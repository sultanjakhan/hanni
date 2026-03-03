// chat.rs — Chat command, streaming, quality check, OpenClaw proxy
use futures_util::StreamExt;
use crate::types::*;
use crate::prompts::*;
use crate::memory::{build_memory_context_from_db, embed_texts, rerank_facts, search_similar_facts, gather_memory_candidates};
use tauri::{AppHandle, Emitter, Manager};
use std::collections::HashMap;

// ── Chat command ──

#[tauri::command]
pub async fn chat(app: AppHandle, messages: Vec<serde_json::Value>, call_mode: Option<bool>) -> Result<String, String> {
    let llm_state = app.state::<LlmBusy>();
    // Wait for any in-flight LLM call (e.g. proactive) to finish — MLX is single-threaded
    let _permit = tokio::time::timeout(
        std::time::Duration::from_secs(45),
        llm_state.0.acquire(),
    ).await
        .map_err(|_| "LLM busy — timeout after 45s".to_string())?
        .map_err(|_| "LLM semaphore closed".to_string())?;
    let is_call = call_mode.unwrap_or(false);

    // Check if OpenClaw mode is enabled
    let use_openclaw = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='use_openclaw'",
            [], |row| row.get::<_, String>(0),
        ).ok().map(|v| v == "true").unwrap_or(false)
    };

    // Call mode always uses direct MLX (fast ~3s vs 30s+ through OpenClaw)
    let result = if use_openclaw && !is_call {
        chat_openclaw(&app, messages.clone(), is_call).await?
    } else {
        let result = chat_inner(&app, messages.clone(), is_call).await?;

        // Self-critique for complex queries (only in CHAT_FULL mode, no tool calls, opt-in)
        if !is_call && result.tool_calls.is_empty() && result.text.len() > 150 {
            let last_user_msg = messages.iter().rev()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .and_then(|m| m.get("content").and_then(|c| c.as_str()))
                .unwrap_or("");

            if is_complex_query(last_user_msg) {
                let self_refine_enabled = {
                    let db = app.state::<HanniDb>();
                    let conn = db.conn();
                    conn.query_row(
                        "SELECT value FROM app_settings WHERE key='enable_self_refine'",
                        [], |row| row.get::<_, String>(0),
                    ).ok().map(|v| v == "true").unwrap_or(false)
                };

                if self_refine_enabled {
                    let client = &app.state::<HttpClient>().0;
                    if let Ok(Some(correction)) = quality_check_response(client, last_user_msg, &result.text).await {
                        let _ = app.emit("chat-token", TokenPayload {
                            token: format!("\n\n_{}_", correction),
                        });
                    }
                }
            }
        }
        result
    };

    serde_json::to_string(&result).map_err(|e| format!("Serialize error: {}", e))
}

struct ChatModeConfig {
    memory_limit: usize,
    history_limit: usize,
    max_msg_chars: usize,
    max_tokens: u32,
    temperature: f32,
    include_tools: bool,
}

const CHAT_CALL: ChatModeConfig = ChatModeConfig { memory_limit: 8, history_limit: 6, max_msg_chars: 500, max_tokens: 400, temperature: 0.6, include_tools: true };
const CHAT_FULL: ChatModeConfig = ChatModeConfig { memory_limit: 10, history_limit: usize::MAX, max_msg_chars: usize::MAX, max_tokens: 1200, temperature: 0.7, include_tools: true };
const CHAT_LITE: ChatModeConfig = ChatModeConfig { memory_limit: 8, history_limit: 8, max_msg_chars: 500, max_tokens: 400, temperature: 0.6, include_tools: false };

/// Thin proxy to OpenClaw Gateway — sends messages, streams response back to UI.
/// OpenClaw handles: prompt engineering, memory, tools (via MCP), personality (SOUL.md).
/// Hanni just forwards the conversation and streams tokens.
pub async fn chat_openclaw(app: &AppHandle, messages: Vec<serde_json::Value>, _call_mode: bool) -> Result<ChatResult, String> {
    let client = &app.state::<HttpClient>().0;

    // Build simple OpenAI-compatible request — only user/assistant messages, no system prompt
    // OpenClaw agent adds its own system prompt from SOUL.md/AGENTS.md
    let chat_messages: Vec<serde_json::Value> = messages.iter()
        .filter(|m| {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
            role == "user" || role == "assistant"
        })
        .cloned()
        .collect();

    let request_body = serde_json::json!({
        "model": "openclaw:main",
        "messages": chat_messages,
        "stream": true,
        "user": "hanni-app",
    });

    let response = client.post(OPENCLAW_URL)
        .header("Authorization", format!("Bearer {}", OPENCLAW_TOKEN))
        .header("Content-Type", "application/json")
        .header("x-openclaw-agent-id", "main")
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("OpenClaw connection error: {}. Is the gateway running?", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenClaw error {}: {}", status, &body[..body.len().min(200)]));
    }

    // Stream SSE response — same format as MLX (OpenAI-compatible)
    let mut stream = response.bytes_stream();
    let mut full_reply = String::new();
    let mut buffer = String::new();
    let mut finish_reason: Option<String> = None;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        for line in buffer.split('\n').collect::<Vec<_>>() {
            let line = line.trim();
            if !line.starts_with("data: ") { continue; }
            let data = &line[6..];
            if data == "[DONE]" {
                let _ = app.emit("chat-done", ());
                continue;
            }

            if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                if let Some(choice) = chunk.choices.first() {
                    if let Some(ref fr) = choice.finish_reason {
                        finish_reason = Some(fr.clone());
                    }
                    if let Some(delta) = &choice.delta {
                        if let Some(token) = &delta.content {
                            if !token.is_empty() {
                                full_reply.push_str(token);
                                let _ = app.emit("chat-token", TokenPayload {
                                    token: token.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        if let Some(pos) = buffer.rfind('\n') {
            buffer = buffer[pos + 1..].to_string();
        }
    }

    Ok(ChatResult {
        text: full_reply,
        tool_calls: Vec::new(), // OpenClaw handles tool calls internally
        finish_reason,
    })
}

pub async fn chat_inner(app: &AppHandle, messages: Vec<serde_json::Value>, call_mode: bool) -> Result<ChatResult, String> {
    let client = &app.state::<HttpClient>().0;

    // Read thinking mode + web search settings (default: off)
    let (thinking_enabled, web_search_enabled) = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let thinking = conn.query_row(
            "SELECT value FROM app_settings WHERE key='enable_thinking'",
            [], |row| row.get::<_, String>(0),
        ).ok().map(|v| v == "true").unwrap_or(false);
        let web = conn.query_row(
            "SELECT value FROM app_settings WHERE key='enable_web_search'",
            [], |row| row.get::<_, String>(0),
        ).ok().map(|v| v == "true").unwrap_or(false);
        (thinking, web)
    };

    // Build system prompt with current date/time context + full week lookup table
    let now_local = chrono::Local::now();
    let weekday_ru = match now_local.format("%u").to_string().as_str() {
        "1" => "понедельник", "2" => "вторник", "3" => "среда",
        "4" => "четверг", "5" => "пятница", "6" => "суббота",
        "7" => "воскресенье", _ => "",
    };
    // Build next 14 days lookup: "Чт 2026-02-12, Пт 2026-02-13, ..."
    let day_abbr = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
    let mut days_ahead = String::new();
    for i in 1..=14 {
        let d = now_local + chrono::Duration::days(i);
        let wd = d.format("%u").to_string().parse::<usize>().unwrap_or(1) - 1;
        if !days_ahead.is_empty() { days_ahead.push_str(", "); }
        days_ahead.push_str(&format!("{} {}", day_abbr[wd], d.format("%Y-%m-%d")));
    }
    let date_context_base = format!(
        "\n\n[Current context]\nToday: {} ({})\nTime: {}",
        now_local.format("%Y-%m-%d"),
        weekday_ru,
        now_local.format("%H:%M"),
    );
    let date_context_full = format!(
        "{}\nNext 14 days: {}",
        date_context_base, days_ahead,
    );
    // Adaptive prompt: use full prompt only when actions are needed
    let last_user_msg = messages.iter().rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("");
    let use_full = needs_full_prompt(last_user_msg);
    let mode = if call_mode { &CHAT_CALL } else if use_full { &CHAT_FULL } else { &CHAT_LITE };

    let system_content = if call_mode {
        format!(r#"{date_ctx}

[ГОЛОСОВОЙ РЕЖИМ]
Ты — Ханни, голосовой ассистент. Пользователь говорит с тобой через микрофон.

ПРАВИЛА:
1. Короткие, естественные предложения. 1-3 максимум.
2. НИКОГДА не используй markdown, списки, код, эмодзи, форматирование.
3. Числа словами: "пять тысяч", а не "5000".
4. Не повторяй предыдущий ответ. Каждый — новый и разный.
5. Тёплый тон, остроумие — как умный друг. По-русски, на "ты".
6. НЕ выдумывай факты о себе (год, возраст). НЕ грубить пользователю.

ИНСТРУМЕНТЫ: когда просят СДЕЛАТЬ — вызывай. Примеры:
- "купил колу за 500" → add_transaction (expense, 500, food, "кола")
- "запомни что я люблю кофе" → remember
- "завтра встреча в 15:00" → create_event
После инструмента — кратко подтверди."#,
            date_ctx = date_context_full)
    } else if use_full {
        format!("{}{}", SYSTEM_PROMPT, date_context_full)
    } else {
        format!("{}{}", SYSTEM_PROMPT_LITE, date_context_base)
    };

    // C1: Inject user name into system prompt if available
    let system_content = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let user_name: Option<String> = conn.query_row(
            "SELECT value FROM facts WHERE category='user' AND key='name' LIMIT 1",
            [], |row| row.get(0),
        ).ok();
        if let Some(name) = user_name {
            format!("Пользователя зовут {}. Обращайся по имени.\n\n{}", name, system_content)
        } else {
            system_content
        }
    };

    // Append complex-query hint for non-call mode
    let system_content = if !call_mode && is_complex_query(last_user_msg) {
        format!("{}\n\nЭто сложный вопрос. Продумай пошагово. Структурируй ответ если нужно.", system_content)
    } else {
        system_content
    };

    // Thinking mode: no prompt injection needed — we use enable_thinking template
    // and stream the reasoning field as visible "🤔" text

    // Append web search hint when chip is enabled
    let system_content = if web_search_enabled && !call_mode {
        format!("{}\n\nВеб-поиск включён. Для поиска информации используй web_search, для чтения страниц — read_url.", system_content)
    } else {
        system_content
    };

    // We'll build ONE consolidated system message (Qwen3.5 requires all system content at the beginning)
    let mut system_parts: Vec<String> = vec![system_content.clone()];

    // Inject memory context: synthesized profile + relevant facts
    // Step 1: embed user message BEFORE acquiring DB lock (async call)
    // Skip embedding in call_mode — use FTS5 only for faster voice responses
    let mem_user_msg_owned = messages.iter().rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("")
        .to_string();
    let query_embedding: Option<Vec<f32>> = if !call_mode && !mem_user_msg_owned.is_empty() {
        embed_texts(client, &[mem_user_msg_owned.clone()]).await
            .ok()
            .and_then(|mut e| if e.is_empty() { None } else { Some(e.remove(0)) })
    } else {
        None
    };
    // Step 2: acquire DB lock and do sync lookups (gather candidates)
    let (profile, memory_candidates) = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();

        // Semantic search hits from pre-computed embedding
        let semantic_hits: Option<Vec<(i64, f64)>> = query_embedding.as_ref().map(|emb| {
            let hits = search_similar_facts(&conn, emb, 15);
            if hits.is_empty() { return Vec::new(); }
            hits
        }).filter(|h| !h.is_empty());

        // Synthesized user profile (compact, natural language)
        let profile: Option<String> = conn.query_row(
            "SELECT value FROM app_settings WHERE key='user_profile'",
            [], |row| row.get(0),
        ).ok();

        // Gather double-pool of candidates for reranking
        let candidates = gather_memory_candidates(&conn, &mem_user_msg_owned, mode.memory_limit * 2, semantic_hits.as_deref());

        (profile, candidates)
    }; // DB lock dropped here

    // Step 3: Rerank candidates asynchronously (or fallback to original order)
    let facts_ctx = if !memory_candidates.is_empty() && !mem_user_msg_owned.is_empty() {
        match rerank_facts(client, &mem_user_msg_owned, &memory_candidates, mode.memory_limit).await {
            Ok(reranked) => {
                // Build context from reranked results
                let id_map: HashMap<i64, &(i64, String, String, String)> = memory_candidates.iter()
                    .map(|c| (c.0, c))
                    .collect();
                let lines: Vec<String> = reranked.iter()
                    .filter_map(|(id, _score)| id_map.get(id))
                    .map(|(_, cat, key, val)| format!("[{}] {}={}", cat, key, val))
                    .collect();
                lines.join("\n")
            }
            Err(_) => {
                // Fallback: use candidates in original order, truncated to limit
                memory_candidates.iter()
                    .take(mode.memory_limit)
                    .map(|(_, cat, key, val)| format!("[{}] {}={}", cat, key, val))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    } else if !memory_candidates.is_empty() {
        memory_candidates.iter()
            .take(mode.memory_limit)
            .map(|(_, cat, key, val)| format!("[{}] {}={}", cat, key, val))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Ultimate fallback: original build_memory_context_from_db (no candidates gathered)
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        build_memory_context_from_db(&conn, &mem_user_msg_owned, mode.memory_limit, None)
    };

    {
        let mut memory_block = String::new();
        if let Some(ref p) = profile {
            memory_block.push_str("[О пользователе]\n");
            memory_block.push_str(p);
        }
        if !facts_ctx.is_empty() {
            if !memory_block.is_empty() { memory_block.push_str("\n\n"); }
            memory_block.push_str("[Релевантные факты]\n");
            memory_block.push_str(&facts_ctx);
        }
        if !memory_block.is_empty() {
            system_parts.push(memory_block);
        }
    }

    // Inject recent conversation summaries for cross-chat context
    // Only useful at the start of a conversation (few messages so far)
    if messages.len() <= 4 && !call_mode {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let mut summaries = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT summary, started_at FROM conversations
             WHERE summary IS NOT NULL AND summary != ''
             ORDER BY started_at DESC LIMIT 2"
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    // Parse date for display: "2026-02-22T15:30:00+06:00" → "2026-02-22"
                    let date = row.1.get(..10).unwrap_or(&row.1);
                    summaries.push(format!("- {}: {}", date, row.0));
                }
            }
        }
        // Fetch recent insights (decisions & open questions)
        let mut insights_lines = Vec::new();
        if let Ok(mut istmt) = conn.prepare(
            "SELECT insight_type, content, created_at FROM conversation_insights
             WHERE insight_type IN ('decision', 'open_question')
             ORDER BY created_at DESC LIMIT 2"
        ) {
            if let Ok(rows) = istmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            }) {
                for row in rows.flatten() {
                    let date = row.2.get(..10).unwrap_or(&row.2);
                    insights_lines.push(format!("- [{}] {}: {}", row.0, date, row.1));
                }
            }
        }

        let mut context_block = String::new();
        if !summaries.is_empty() {
            summaries.reverse(); // chronological order
            context_block.push_str(&format!("[Недавние разговоры]\n{}", summaries.join("\n")));
        }
        if !insights_lines.is_empty() {
            if !context_block.is_empty() { context_block.push_str("\n\n"); }
            insights_lines.reverse(); // chronological
            context_block.push_str(&format!("[Недавние решения и вопросы]\n{}", insights_lines.join("\n")));
        }
        if !context_block.is_empty() {
            system_parts.push(context_block);
        }
    }

    // Consolidate all system content into ONE system message (Qwen3.5 rejects system messages after user messages)
    let consolidated_system = system_parts.join("\n\n---\n\n");
    let mut chat_messages = vec![ChatMessage::text("system", &consolidated_system)];

    let history_limit = if mode.history_limit == usize::MAX { messages.len() } else { mode.history_limit };
    let skip = messages.len().saturating_sub(history_limit);
    let trimmed: Vec<_> = messages.iter().skip(skip).collect();
    let max_msg_chars = mode.max_msg_chars;
    for msg_val in trimmed.iter() {
        if let Ok(mut cm) = serde_json::from_value::<ChatMessage>((*msg_val).clone()) {
            // Skip system messages from history — Qwen3.5 only allows system at the beginning
            if cm.role == "system" {
                continue;
            }
            // Don't truncate tool results — model needs full context to summarize
            let is_tool = cm.role == "tool";
            if max_msg_chars < usize::MAX && !is_tool {
                if let Some(ref c) = cm.content {
                    if c.len() > max_msg_chars {
                        cm.content = Some(format!("{}...", &c[..c.floor_char_boundary(max_msg_chars)]));
                    }
                }
            }
            chat_messages.push(cm);
        }
    }

    // CH9: Smart context — use last 3 user messages for tool selection, not just the last one
    let tools_param = if mode.include_tools {
        let mut context = String::new();
        let recent_user_msgs: Vec<&str> = messages.iter().rev()
            .filter_map(|m| {
                if m.get("role").and_then(|r| r.as_str()) == Some("user") {
                    m.get("content").and_then(|c| c.as_str())
                } else { None }
            })
            .take(3)
            .collect();
        for msg in recent_user_msgs.iter().rev() {
            if !context.is_empty() { context.push(' '); }
            context.push_str(msg);
        }
        Some(select_relevant_tools(&context))
    } else { None };

    // Force web_search + read_url when web search chip is enabled
    let tools_param = if web_search_enabled {
        let web_tool_names = ["web_search", "read_url"];
        match tools_param {
            Some(mut tools) => {
                // Ensure web tools are present
                let all_defs = build_tool_definitions();
                for name in &web_tool_names {
                    let already = tools.iter().any(|t|
                        t.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) == Some(name)
                    );
                    if !already {
                        if let Some(def) = all_defs.iter().find(|t|
                            t.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) == Some(name)
                        ) {
                            tools.push(def.clone());
                        }
                    }
                }
                Some(tools)
            }
            None => {
                // LITE mode but web search forced — include only web tools
                let all_defs = build_tool_definitions();
                Some(all_defs.into_iter().filter(|t|
                    t.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str())
                        .map(|n| web_tool_names.contains(&n)).unwrap_or(false)
                ).collect())
            }
        }
    } else { tools_param };

    // C5: Adaptive temperature based on query type
    let adaptive_temp = if !call_mode {
        let lower = last_user_msg.to_lowercase();
        if lower.contains("сколько") || lower.contains("когда") || lower.contains("какой")
            || lower.contains("что такое") || lower.contains("кто такой") || lower.contains("найди")
            || lower.contains("статистик") || lower.contains("покажи") {
            0.4 // factual queries → low creativity
        } else if lower.contains("придумай") || lower.contains("напиши стих")
            || lower.contains("история") || lower.contains("расскажи")
            || lower.contains("пошути") || lower.contains("развесел") {
            0.85 // creative queries → high creativity
        } else {
            mode.temperature
        }
    } else {
        mode.temperature
    };

    // ML8: Adaptive max_tokens based on user message length style
    let adaptive_max_tokens = if !call_mode {
        let user_lengths: Vec<usize> = messages.iter()
            .filter_map(|m| {
                if m.get("role").and_then(|r| r.as_str()) == Some("user") {
                    m.get("content").and_then(|c| c.as_str()).map(|s| s.len())
                } else { None }
            })
            .collect();
        if user_lengths.len() >= 2 {
            let avg = user_lengths.iter().sum::<usize>() / user_lengths.len();
            if avg < 30 { mode.max_tokens.min(600) }      // short messages → concise replies
            else if avg > 200 { mode.max_tokens.max(1200) } // long messages → detailed replies
            else { mode.max_tokens }
        } else { mode.max_tokens }
    } else { mode.max_tokens };

    // Thinking mode: bump max_tokens (reasoning tokens count against limit)
    // Model can burn 1000-3000 tokens on reasoning alone
    let adaptive_max_tokens = if thinking_enabled {
        adaptive_max_tokens.max(4096)
    } else {
        adaptive_max_tokens
    };

    let request = ChatRequest {
        model: MODEL.into(),
        messages: chat_messages,
        max_tokens: adaptive_max_tokens,
        stream: true,
        temperature: adaptive_temp,
        repetition_penalty: None,
        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: thinking_enabled },
        tools: tools_param,
    };

    // Retry connection up to 3 times (MLX server may still be loading model or return 404)
    let mut response = None;
    for attempt in 0..3 {
        match client.post(MLX_URL).json(&request).send().await {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    response = Some(r);
                    break;
                } else {
                    // Non-2xx status — read error body and retry
                    let body = r.text().await.unwrap_or_default();
                    eprintln!("[chat_inner] MLX error {}: {}", status, &body[..body.len().min(200)]);
                    if attempt < 2 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    } else {
                        return Err(format!("MLX server error {}: {}", status, &body[..body.len().min(100)]));
                    }
                }
            }
            Err(e) => {
                eprintln!("[chat] MLX connection error (attempt {}): {}", attempt, e);
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                } else {
                    return Err(format!("MLX connection error: {}", e));
                }
            }
        }
    }
    let response = response.ok_or_else(|| "MLX: all retries exhausted".to_string())?;

    let mut stream = response.bytes_stream();
    let mut full_reply = String::new();
    let mut buffer = String::new();
    let mut finish_reason: Option<String> = None;
    let mut reasoning_started = false; // track if we've emitted 🤔 prefix

    // Tool call accumulator: index → (id, name, arguments)
    let mut tc_ids: HashMap<usize, String> = HashMap::new();
    let mut tc_names: HashMap<usize, String> = HashMap::new();
    let mut tc_args: HashMap<usize, String> = HashMap::new();

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
                    // Capture finish_reason
                    if let Some(ref fr) = choice.finish_reason {
                        finish_reason = Some(fr.clone());
                    }

                    if let Some(delta) = &choice.delta {
                        // Accumulate tool call deltas
                        if let Some(ref tcs) = delta.tool_calls {
                            for tc in tcs {
                                let idx = tc.index;
                                if let Some(ref id) = tc.id {
                                    tc_ids.insert(idx, id.clone());
                                }
                                if let Some(ref func) = tc.function {
                                    if let Some(ref name) = func.name {
                                        tc_names.insert(idx, name.clone());
                                    }
                                    if let Some(ref args) = func.arguments {
                                        tc_args.entry(idx).or_default().push_str(args);
                                    }
                                }
                            }
                        }

                        // Stream reasoning tokens (thinking mode)
                        if let Some(ref reasoning) = delta.reasoning {
                            if !reasoning.is_empty() {
                                reasoning_started = true;
                                let _ = app.emit("chat-reasoning", TokenPayload {
                                    token: reasoning.clone(),
                                });
                            }
                        }

                        // Stream content tokens (actual response)
                        if let Some(token) = &delta.content {
                            if !token.is_empty() {
                                if reasoning_started {
                                    reasoning_started = false;
                                    // Signal end of reasoning phase
                                    let _ = app.emit("chat-reasoning-done", ());
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
        }
        if let Some(pos) = buffer.rfind('\n') {
            buffer = buffer[pos + 1..].to_string();
        }
    }

    // Build tool_calls from accumulated deltas
    let mut tool_calls: Vec<ToolCallResult> = Vec::new();
    let mut indices: Vec<usize> = tc_ids.keys().chain(tc_names.keys()).chain(tc_args.keys())
        .copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    indices.sort();
    for idx in indices {
        let id = tc_ids.remove(&idx).unwrap_or_else(|| format!("call_{}", idx));
        let name = tc_names.remove(&idx).unwrap_or_default();
        let arguments = tc_args.remove(&idx).unwrap_or_default();
        tool_calls.push(ToolCallResult {
            id,
            call_type: "function".into(),
            function: ToolCallResultFunction { name, arguments },
        });
    }

    Ok(ChatResult {
        text: full_reply,
        tool_calls,
        finish_reason,
    })
}

/// Self-critique: ask LLM to check its own response for errors.
/// Returns Some(correction) if issues found, None if response is good.
pub async fn quality_check_response(
    client: &reqwest::Client,
    user_msg: &str,
    assistant_response: &str,
) -> Result<Option<String>, String> {
    let check_prompt = format!(
        "Пользователь спросил: \"{}\"\n\nТвой ответ: \"{}\"\n\n\
         Проверь ответ. Если он корректный и полный — напиши только [OK].\n\
         Если есть фактическая ошибка или важное упущение — коротко укажи (1-2 предложения).",
        user_msg, assistant_response
    );

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage::text("system", "Ты — критик ответов. Будь краток. Отвечай на русском."),
            ChatMessage::text("user", &check_prompt),
        ],
        max_tokens: 150,
        stream: false,
        temperature: 0.2,
        repetition_penalty: None,
        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
        tools: None,
    };

    let resp = client.post(MLX_URL)
        .json(&request)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Self-critique request error: {}", e))?;

    let parsed: NonStreamResponse = resp.json().await
        .map_err(|e| format!("Self-critique parse error: {}", e))?;

    let raw = parsed.choices.first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think>
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    if text.contains("[OK]") || text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

// ── File commands ──

#[tauri::command]
pub async fn read_file(path: String) -> Result<String, String> {
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Cannot access {}: {}", path, e))?;

    // Limit to 500KB for text files
    if metadata.len() > 512_000 {
        return Err(format!("File too large: {} bytes (max 500KB)", metadata.len()));
    }

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Cannot read {}: {}", path, e))
}

#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<String>, String> {
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
