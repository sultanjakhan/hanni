use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const MLX_URL: &str = "http://127.0.0.1:8234/v1/chat/completions";
const MODEL: &str = "mlx-community/Qwen3-30B-A3B-4bit";
const SYSTEM_PROMPT: &str = "You are Hanni, a helpful AI assistant. Answer concisely and directly. Use the same language as the user. /no_think";

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Option<Delta>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Clone, Serialize)]
struct TokenPayload {
    token: String,
}

#[tauri::command]
async fn chat(app: AppHandle, messages: Vec<(String, String)>) -> Result<String, String> {
    let client = reqwest::Client::new();

    let mut chat_messages = vec![ChatMessage {
        role: "system".into(),
        content: SYSTEM_PROMPT.into(),
    }];

    for (role, content) in &messages {
        chat_messages.push(ChatMessage {
            role: role.clone(),
            content: content.clone(),
        });
    }

    let request = ChatRequest {
        model: MODEL.into(),
        messages: chat_messages,
        max_tokens: 1024,
        stream: true,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("MLX connection error: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut full_reply = String::new();
    let mut in_think = false;
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        for line in buffer.split('\n').collect::<Vec<_>>() {
            let line = line.trim();
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data == "[DONE]" {
                let _ = app.emit("chat-done", ());
                continue;
            }

            if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                if let Some(choice) = chunk.choices.first() {
                    if let Some(delta) = &choice.delta {
                        if let Some(token) = &delta.content {
                            if token.contains("<think>") {
                                in_think = true;
                                continue;
                            }
                            if token.contains("</think>") {
                                in_think = false;
                                continue;
                            }
                            if in_think {
                                continue;
                            }
                            full_reply.push_str(token);
                            let _ = app.emit("chat-token", TokenPayload {
                                token: token.clone(),
                            });
                        }
                    }
                }
            }
        }
        // Keep only the last incomplete line in buffer
        if let Some(pos) = buffer.rfind('\n') {
            buffer = buffer[pos + 1..].to_string();
        }
    }

    Ok(full_reply)
}

#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Cannot read {}: {}", path, e))
}

#[tauri::command]
async fn list_dir(path: String) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| format!("Cannot read dir {}: {}", path, e))?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        if let Some(name) = entry.file_name().to_str() {
            entries.push(name.to_string());
        }
    }
    Ok(entries)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![chat, read_file, list_dir])
        .run(tauri::generate_context!())
        .expect("error while running Hanni");
}
