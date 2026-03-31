// agent.rs — Headless agentic loop for background tasks (no UI streaming)
// Sends prompt to LLM, handles tool_calls (MCP + internal), loops until done.

use crate::types::*;
use crate::mcp::McpState;
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

const MAX_AGENT_ITERATIONS: usize = 25;

/// Context overrides for tool execution (e.g. force project_id for vacancy tasks).
pub type AgentContext = HashMap<String, serde_json::Value>;

/// Run a headless agent task: LLM + tool loop, returns final text.
pub async fn run_agent_task(
    app: &AppHandle,
    system_prompt: &str,
    user_prompt: &str,
    tools: Vec<serde_json::Value>,
    context: AgentContext,
) -> Result<String, String> {
    let client = &app.state::<HttpClient>().0;
    let llm_state = app.state::<LlmBusy>();

    let mut messages: Vec<ChatMessage> = vec![
        ChatMessage::text("system", system_prompt),
        ChatMessage::text("user", user_prompt),
    ];

    for iteration in 0..MAX_AGENT_ITERATIONS {
        // Acquire LLM semaphore (MLX is single-threaded)
        let _permit = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            llm_state.0.acquire(),
        ).await
            .map_err(|_| "Agent: LLM busy timeout".to_string())?
            .map_err(|_| "Agent: LLM semaphore closed".to_string())?;

        let request = ChatRequest {
            model: MODEL.into(),
            messages: messages.clone(),
            max_tokens: 2048,
            stream: false,
            temperature: 0.3,
            repetition_penalty: None,
            chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
            tools: if tools.is_empty() { None } else { Some(tools.clone()) },
        };

        let resp = client.post(MLX_URL)
            .json(&request)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await
            .map_err(|e| format!("Agent MLX error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Agent MLX {}: {}", status, &body[..body.len().min(200)]));
        }

        let parsed: AgentResponse = resp.json().await
            .map_err(|e| format!("Agent parse error: {}", e))?;

        let choice = parsed.choices.into_iter().next()
            .ok_or("Agent: no choices")?;

        let content = choice.message.content.unwrap_or_default();
        let tool_calls = choice.message.tool_calls.unwrap_or_default();

        // No tool calls — we're done
        if tool_calls.is_empty() {
            let preview: String = content.chars().take(100).collect();
            eprintln!("[agent] Done after {} iterations, response: {}...",
                iteration + 1, preview);
            return Ok(content);
        }

        // Add assistant message with tool_calls to history
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: if content.is_empty() { None } else { Some(content) },
            tool_call_id: None,
            name: None,
            tool_calls: Some(tool_calls.iter().map(|tc| ToolCallResult {
                id: tc.id.clone(),
                call_type: "function".into(),
                function: ToolCallResultFunction {
                    name: tc.function.name.clone(),
                    arguments: tc.function.arguments.clone(),
                },
            }).collect()),
        });

        // Execute each tool call
        for tc in &tool_calls {
            let result = execute_tool_call(app, &tc.function.name, &tc.function.arguments, &context).await;
            let preview: String = result.chars().take(150).collect();
            eprintln!("[agent] tool {} → {}", tc.function.name, preview);

            messages.push(ChatMessage {
                role: "tool".into(),
                content: Some(result),
                tool_call_id: Some(tc.id.clone()),
                name: Some(tc.function.name.clone()),
                tool_calls: None,
            });
        }
    }

    Err("Agent: max iterations reached".into())
}

/// Execute a single tool call — routes to MCP or internal Tauri commands.
async fn execute_tool_call(app: &AppHandle, name: &str, arguments_raw: &str, context: &AgentContext) -> String {
    let args: serde_json::Value = serde_json::from_str(arguments_raw)
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    // Try MCP first
    {
        let mcp_state = app.state::<McpState>();
        let mgr = mcp_state.0.lock().await;
        if mgr.has_tool(name) {
            return match mgr.call_tool(name, args).await {
                Ok(s) => s,
                Err(e) => format!("MCP error: {}", e),
            };
        }
    }

    // Internal commands
    match name {
        "create_task" | "create_project_task" => {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            // Use context override for project_id if provided, else from args, else 1
            let project_id = context.get("project_id")
                .and_then(|v| v.as_i64())
                .or_else(|| args.get("project_id").and_then(|v| v.as_i64()))
                .unwrap_or(1);
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("normal");
            let due_date = args.get("due_date").and_then(|v| v.as_str());
            let now = chrono::Local::now().to_rfc3339();
            match conn.execute(
                "INSERT INTO tasks (project_id, title, description, status, priority, due_date, created_at) VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6)",
                rusqlite::params![project_id, title, description, priority, due_date, now],
            ) {
                Ok(_) => format!("Task created: {}", title),
                Err(e) => format!("DB error: {}", e),
            }
        }
        "save_vacancy" => {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            let company = args.get("company").and_then(|v| v.as_str()).unwrap_or("");
            let position = args.get("position").and_then(|v| v.as_str()).unwrap_or("");
            // Check blacklist
            let company_lower = company.to_lowercase();
            let bl: Vec<String> = conn.prepare("SELECT value FROM facts WHERE category='jobs_blacklist'")
                .and_then(|mut s| { let r: Vec<String> = s.query_map([], |row| row.get(0))?.flatten().collect(); Ok(r) })
                .unwrap_or_default();
            if bl.iter().any(|b| company_lower.contains(&b.to_lowercase())) {
                return format!("Skipped (blacklisted): {}", company);
            }
            let salary = args.get("salary").and_then(|v| v.as_str()).unwrap_or("");
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");
            let now = chrono::Local::now().to_rfc3339();
            match conn.execute(
                "INSERT INTO job_vacancies (company, position, salary, url, stage, source, notes, found_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'found', ?5, ?6, ?7, ?7)",
                rusqlite::params![company, position, salary, url, source, notes, now],
            ) {
                Ok(_) => format!("Vacancy saved: {} — {}", company, position),
                Err(e) => format!("DB error: {}", e),
            }
        }
        "web_search" | "read_url" => {
            format!("Tool {} not available in agent mode. Use playwright browser tools instead.", name)
        }
        _ => {
            format!("Unknown tool: {}", name)
        }
    }
}
