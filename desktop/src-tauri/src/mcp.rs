// mcp.rs — MCP client manager: connects to MCP servers, exposes tools to LLM
use std::collections::HashMap;
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, Tool},
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde::Deserialize;
use tauri::Manager;
use tokio::process::Command;

type McpClient = RunningService<rmcp::service::RoleClient, ()>;

// ── Config (mirrors .mcp.json / ~/.hanni/mcp.json format) ──

#[derive(Deserialize, Clone, Debug)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize, Debug)]
pub struct McpConfig {
    #[serde(alias = "mcpServers")]
    pub servers: HashMap<String, McpServerConfig>,
}

// ── Manager ──

pub struct McpManager {
    clients: HashMap<String, McpClient>,
    /// tool_name → server_name (for routing calls)
    tool_index: HashMap<String, String>,
    /// Cached OpenAI-format tool definitions
    openai_tools: Vec<serde_json::Value>,
}

impl McpManager {
    /// Connect to all enabled MCP servers from config file.
    pub async fn from_config(config_path: &str) -> Self {
        let mut mgr = McpManager {
            clients: HashMap::new(),
            tool_index: HashMap::new(),
            openai_tools: Vec::new(),
        };

        let config_str = match std::fs::read_to_string(config_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[mcp] Config not found at {}: {}", config_path, e);
                return mgr;
            }
        };
        let config: McpConfig = match serde_json::from_str(&config_str) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[mcp] Invalid config: {}", e);
                return mgr;
            }
        };

        for (name, server) in &config.servers {
            if !server.enabled { continue; }
            match connect_server(server).await {
                Ok(client) => {
                    eprintln!("[mcp] Connected to '{}'", name);
                    // Fetch and index tools
                    match client.list_all_tools().await {
                        Ok(tools) => {
                            for tool in &tools {
                                let prefixed = format!("{}_{}", name, tool.name);
                                mgr.tool_index.insert(prefixed.clone(), name.clone());
                                mgr.openai_tools.push(mcp_tool_to_openai(&prefixed, tool));
                            }
                            eprintln!("[mcp] '{}': {} tools registered", name, tools.len());
                        }
                        Err(e) => eprintln!("[mcp] '{}' list_tools failed: {}", name, e),
                    }
                    mgr.clients.insert(name.clone(), client);
                }
                Err(e) => eprintln!("[mcp] Failed to connect '{}': {}", name, e),
            }
        }
        mgr
    }

    /// Get all MCP tools as OpenAI function-calling format.
    pub fn tools_as_openai(&self) -> &[serde_json::Value] {
        &self.openai_tools
    }

    /// Check if a tool name belongs to an MCP server.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tool_index.contains_key(name)
    }

    /// Call an MCP tool by prefixed name. Returns text result.
    pub async fn call_tool(&self, prefixed_name: &str, arguments: serde_json::Value) -> Result<String, String> {
        let server_name = self.tool_index.get(prefixed_name)
            .ok_or_else(|| format!("Unknown MCP tool: {}", prefixed_name))?;

        let client = self.clients.get(server_name)
            .ok_or_else(|| format!("MCP server '{}' not connected", server_name))?;

        // Strip server prefix to get original tool name
        let original_name = prefixed_name.strip_prefix(&format!("{}_", server_name))
            .unwrap_or(prefixed_name);

        let args = match arguments {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };

        let result = client.call_tool(
            CallToolRequestParams::new(original_name.to_string()).with_arguments(args)
        ).await.map_err(|e| format!("MCP call failed: {}", e))?;

        // Extract text from result content
        let mut output = String::new();
        for content in &result.content {
            if let Some(text) = content.as_text() {
                if !output.is_empty() { output.push('\n'); }
                output.push_str(&text.text);
            }
        }
        if result.is_error.unwrap_or(false) {
            return Err(output);
        }
        Ok(output)
    }

    /// Graceful shutdown of all MCP servers.
    pub async fn shutdown(self) {
        for (name, client) in self.clients {
            if let Err(e) = client.cancel().await {
                eprintln!("[mcp] Error shutting down '{}': {}", name, e);
            }
        }
    }
}

// ── Tauri commands ──

#[tauri::command]
pub async fn mcp_call_tool(
    app: tauri::AppHandle,
    name: String,
    arguments: serde_json::Value,
) -> Result<String, String> {
    eprintln!("[mcp] call_tool: {} args={}", name, arguments);
    let state = app.state::<McpState>();
    let mgr = state.0.lock().await;
    let result = mgr.call_tool(&name, arguments).await;
    match &result {
        Ok(s) => eprintln!("[mcp] call_tool OK: {}...", &s[..s.len().min(200)]),
        Err(e) => eprintln!("[mcp] call_tool ERROR: {}", e),
    }
    result
}

#[tauri::command]
pub async fn mcp_list_tools(app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let state = app.state::<McpState>();
    let mgr = state.0.lock().await;
    Ok(mgr.tools_as_openai().to_vec())
}

/// Wrapper for Tauri managed state (Arc for cloning into async tasks)
#[derive(Clone)]
pub struct McpState(pub std::sync::Arc<tokio::sync::Mutex<McpManager>>);

impl McpState {
    pub fn empty() -> Self {
        McpState(std::sync::Arc::new(tokio::sync::Mutex::new(McpManager {
            clients: HashMap::new(),
            tool_index: HashMap::new(),
            openai_tools: Vec::new(),
        })))
    }
}

// ── Helpers ──

async fn connect_server(config: &McpServerConfig) -> Result<McpClient, String> {
    let transport = TokioChildProcess::new(
        Command::new(&config.command).configure(|cmd| {
            cmd.args(&config.args);
            for (k, v) in &config.env {
                cmd.env(k, v);
            }
        })
    ).map_err(|e| format!("spawn failed: {}", e))?;

    let client = ().serve(transport).await
        .map_err(|e| format!("MCP handshake failed: {}", e))?;
    Ok(client)
}

/// Convert MCP Tool → OpenAI function-calling JSON.
fn mcp_tool_to_openai(prefixed_name: &str, tool: &Tool) -> serde_json::Value {
    let description = tool.description.as_deref().unwrap_or("").to_string();
    let parameters = serde_json::to_value(&*tool.input_schema)
        .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

    serde_json::json!({
        "type": "function",
        "function": {
            "name": prefixed_name,
            "description": description,
            "parameters": parameters
        }
    })
}
