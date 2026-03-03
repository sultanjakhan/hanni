// memory.rs — Semantic memory, embedding, conversation management
use crate::types::*;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use std::path::PathBuf;

// ── Semantic memory helpers (sqlite-vec + fastembed) ──

pub async fn embed_texts(client: &reqwest::Client, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let resp = client
        .post(&format!("{}/embed", VOICE_SERVER_URL))
        .json(&serde_json::json!({ "texts": texts }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Embed request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Embed server returned {}", resp.status()));
    }
    #[derive(Deserialize)]
    struct EmbedResponse {
        embeddings: Vec<Vec<f32>>,
    }
    let body: EmbedResponse = resp.json().await
        .map_err(|e| format!("Embed parse error: {}", e))?;
    Ok(body.embeddings)
}

pub fn store_fact_embedding(conn: &rusqlite::Connection, fact_id: i64, embedding: &[f32]) {
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            embedding.as_ptr() as *const u8,
            embedding.len() * std::mem::size_of::<f32>(),
        )
    };
    let _ = conn.execute(
        "INSERT OR REPLACE INTO vec_facts(fact_id, embedding) VALUES (?1, ?2)",
        rusqlite::params![fact_id, bytes],
    );
}

pub fn search_similar_facts(conn: &rusqlite::Connection, query_embedding: &[f32], limit: usize) -> Vec<(i64, f64)> {
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            query_embedding.as_ptr() as *const u8,
            query_embedding.len() * std::mem::size_of::<f32>(),
        )
    };
    let mut results = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT fact_id, distance FROM vec_facts WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![bytes, limit as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        }) {
            for row in rows.flatten() {
                results.push(row);
            }
        }
    }
    results
}

pub fn build_memory_context_from_db(conn: &rusqlite::Connection, user_msg: &str, limit: usize, semantic_hits: Option<&[(i64, f64)]>) -> String {
    let mut lines = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 0. Semantic search tier — pre-computed vector similarity hits
    if let Some(hits) = semantic_hits {
        for &(fact_id, _distance) in hits {
            if let Ok(row) = conn.query_row(
                "SELECT id, category, key, value FROM facts WHERE id=?1",
                rusqlite::params![fact_id],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            ) {
                if seen_ids.insert(row.0) {
                    lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                }
            }
        }
    }

    // 1. Always include core user/preferences facts (top 20), ordered by decay score
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, category, key, value FROM facts
         WHERE category IN ('user', 'preferences')
         ORDER BY (COALESCE(access_count,0) * 0.5 + CASE WHEN last_accessed IS NOT NULL
           THEN (julianday('now') - julianday(last_accessed)) * -0.05 ELSE -3 END) DESC,
           updated_at DESC LIMIT 20"
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                seen_ids.insert(row.0);
                lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
            }
        }
    }

    // 2. FTS5 search matching user's latest message (top 20 more)
    let remaining = limit.saturating_sub(lines.len());
    if remaining > 0 && !user_msg.is_empty() {
        // Build FTS query: split words, join with OR
        let words: Vec<&str> = user_msg.split_whitespace()
            .filter(|w| w.len() > 2)
            .take(10)
            .collect();
        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.id, f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1
                 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, remaining as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        if seen_ids.insert(row.0) {
                            lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                        }
                    }
                }
            }
        }
    }

    // 3. Fill remaining with most recent facts (exclude observations)
    let remaining = limit.saturating_sub(lines.len());
    if remaining > 0 {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, category, key, value FROM facts
             WHERE category != 'observation'
             ORDER BY updated_at DESC LIMIT ?1"
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![remaining as i64 + seen_ids.len() as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    if lines.len() >= limit {
                        break;
                    }
                    if seen_ids.insert(row.0) {
                        lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                    }
                }
            }
        }
    }

    // 4. Add up to 2 observation facts (low priority, for proactive personalization)
    let obs_remaining = 2.min(limit.saturating_sub(lines.len()));
    if obs_remaining > 0 {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, category, key, value FROM facts
             WHERE category = 'observation'
             ORDER BY updated_at DESC LIMIT ?1"
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![obs_remaining as i64 + seen_ids.len() as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    if lines.len() >= limit {
                        break;
                    }
                    if seen_ids.insert(row.0) {
                        lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                    }
                }
            }
        }
    }

    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n")
    }
}

/// Gather all memory candidates from 4 tiers (semantic, core, FTS, recent) into a single pool.
/// Returns Vec<(fact_id, category, key, value)>.
pub fn gather_memory_candidates(
    conn: &rusqlite::Connection,
    user_msg: &str,
    pool_size: usize,
    semantic_hits: Option<&[(i64, f64)]>,
) -> Vec<(i64, String, String, String)> {
    let mut candidates = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 0. Semantic search tier (exclude observation facts — they pollute context)
    if let Some(hits) = semantic_hits {
        for &(fact_id, _) in hits {
            if let Ok(row) = conn.query_row(
                "SELECT id, category, key, value FROM facts WHERE id=?1 AND category != 'observation'",
                rusqlite::params![fact_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
            ) {
                if seen_ids.insert(row.0) {
                    candidates.push(row);
                }
            }
        }
    }

    // 1. Core user/preferences facts
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, category, key, value FROM facts WHERE category IN ('user', 'preferences') ORDER BY updated_at DESC LIMIT 20"
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
        }) {
            for row in rows.flatten() {
                if seen_ids.insert(row.0) { candidates.push(row); }
            }
        }
    }

    // 2. FTS5 search
    if !user_msg.is_empty() {
        let words: Vec<&str> = user_msg.split_whitespace().filter(|w| w.len() > 2).take(10).collect();
        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.id, f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid WHERE facts_fts MATCH ?1 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, pool_size as i64], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
                }) {
                    for row in rows.flatten() {
                        if seen_ids.insert(row.0) { candidates.push(row); }
                    }
                }
            }
        }
    }

    // 3. Recent facts to fill pool (exclude observations)
    let remaining = pool_size.saturating_sub(candidates.len());
    if remaining > 0 {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, category, key, value FROM facts WHERE category != 'observation' ORDER BY updated_at DESC LIMIT ?1"
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![remaining as i64 + seen_ids.len() as i64], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
            }) {
                for row in rows.flatten() {
                    if candidates.len() >= pool_size { break; }
                    if seen_ids.insert(row.0) { candidates.push(row); }
                }
            }
        }
    }

    candidates
}

/// Call voice_server /rerank endpoint to rerank facts by relevance to query.
/// Returns top_k (fact_id, score) pairs sorted by score desc.
pub async fn rerank_facts(
    client: &reqwest::Client,
    query: &str,
    facts: &[(i64, String, String, String)],
    top_k: usize,
) -> Result<Vec<(i64, f64)>, String> {
    let passages: Vec<serde_json::Value> = facts.iter()
        .map(|(id, cat, key, val)| serde_json::json!({"id": id, "text": format!("[{}] {}={}", cat, key, val)}))
        .collect();

    let body = serde_json::json!({
        "query": query,
        "passages": passages,
        "top_k": top_k,
    });

    let resp = client.post("http://127.0.0.1:8237/rerank")
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Rerank request error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Rerank HTTP {}", resp.status()));
    }

    #[derive(Deserialize)]
    struct RerankResponse {
        results: Vec<RerankResult>,
    }
    #[derive(Deserialize)]
    struct RerankResult {
        id: serde_json::Value,
        score: f64,
    }

    let parsed: RerankResponse = resp.json().await.map_err(|e| format!("Rerank parse error: {}", e))?;
    Ok(parsed.results.iter().map(|r| {
        let id = r.id.as_i64().unwrap_or(0);
        (id, r.score)
    }).collect())
}

pub fn proactive_settings_path() -> PathBuf {
    hanni_data_dir().join("proactive_settings.json")
}

pub fn load_proactive_settings() -> ProactiveSettings {
    let path = proactive_settings_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        ProactiveSettings::default()
    }
}

pub fn save_proactive_settings(settings: &ProactiveSettings) -> Result<(), String> {
    let path = proactive_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Cannot write: {}", e))
}

// ── Memory commands (SQLite) ──

#[tauri::command]
pub fn memory_remember(
    category: String,
    key: String,
    value: String,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO facts (category, key, value, source, created_at, updated_at, access_count, last_accessed)
         VALUES (?1, ?2, ?3, 'user', ?4, ?4, 1, ?4)
         ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4, access_count=access_count+1, last_accessed=?4",
        rusqlite::params![category, key, value, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(format!("Remembered {}/{}={}", category, key, value))
}

#[tauri::command]
pub fn memory_recall(
    category: String,
    key: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    match key {
        Some(k) => {
            let result: Result<String, _> = conn.query_row(
                "SELECT value FROM facts WHERE category=?1 AND key=?2",
                rusqlite::params![category, k],
                |row| row.get(0),
            );
            match result {
                Ok(val) => {
                    // ME1: Update access tracking on recall
                    let _ = conn.execute(
                        "UPDATE facts SET access_count=access_count+1, last_accessed=?3 WHERE category=?1 AND key=?2",
                        rusqlite::params![category, k, now],
                    );
                    Ok(format!("{}={}", k, val))
                },
                Err(_) => Ok(format!("No memory for {}/{}", category, k)),
            }
        }
        None => {
            // ME1: Sort by decay score — frequently accessed + recently updated facts first
            let mut stmt = conn.prepare(
                "SELECT key, value FROM facts WHERE category=?1
                 ORDER BY (access_count * 0.5 + CASE WHEN last_accessed IS NOT NULL
                   THEN (julianday('now') - julianday(last_accessed)) * -0.05 ELSE -3 END) DESC,
                 updated_at DESC"
            ).map_err(|e| format!("DB error: {}", e))?;
            let pairs: Vec<String> = stmt.query_map(rusqlite::params![category], |row| {
                Ok(format!("{}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("DB error: {}", e))?
            .flatten()
            .collect();
            if pairs.is_empty() {
                Ok(format!("No memories in category '{}'", category))
            } else {
                Ok(pairs.join(", "))
            }
        }
    }
}

#[tauri::command]
pub fn memory_forget(
    category: String,
    key: String,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    // Clean up vector embedding before deleting the fact
    let _ = conn.execute(
        "DELETE FROM vec_facts WHERE fact_id IN (SELECT id FROM facts WHERE category=?1 AND key=?2)",
        rusqlite::params![category, key],
    );
    let deleted = conn.execute(
        "DELETE FROM facts WHERE category=?1 AND key=?2",
        rusqlite::params![category, key],
    ).map_err(|e| format!("DB error: {}", e))?;
    if deleted > 0 {
        Ok(format!("Forgot {}/{}", category, key))
    } else {
        Ok(format!("No memory for {}/{}", category, key))
    }
}

#[tauri::command]
pub fn memory_search(
    query: String,
    limit: Option<usize>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(20) as i64;

    // Try FTS5 MATCH first
    let words: Vec<&str> = query.split_whitespace()
        .filter(|w| w.len() > 1)
        .take(10)
        .collect();
    if !words.is_empty() {
        let fts_query = words.join(" OR ");
        if let Ok(mut stmt) = conn.prepare(
            "SELECT f.category, f.key, f.value FROM facts_fts fts
             JOIN facts f ON f.id = fts.rowid
             WHERE facts_fts MATCH ?1
             ORDER BY rank LIMIT ?2"
        ) {
            let results: Vec<String> = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                Ok(format!("[{}] {}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| format!("DB error: {}", e))?
            .flatten()
            .collect();
            if !results.is_empty() {
                return Ok(results.join("\n"));
            }
        }
    }

    // Fallback: LIKE search
    let like_pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT category, key, value FROM facts
         WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1
         ORDER BY updated_at DESC LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let results: Vec<String> = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
        Ok(format!("[{}] {}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })
    .map_err(|e| format!("DB error: {}", e))?
    .flatten()
    .collect();

    if results.is_empty() {
        Ok("No memories found.".into())
    } else {
        Ok(results.join("\n"))
    }
}

#[tauri::command]
pub fn save_conversation(
    messages: Vec<serde_json::Value>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let msg_count = messages.len() as i64;
    conn.execute(
        "INSERT INTO conversations (started_at, message_count, messages) VALUES (?1, ?2, ?3)",
        rusqlite::params![now, msg_count, messages_json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_conversation(
    id: i64,
    messages: Vec<serde_json::Value>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let msg_count = messages.len() as i64;
    conn.execute(
        "UPDATE conversations SET messages=?1, message_count=?2, ended_at=?3 WHERE id=?4",
        rusqlite::params![messages_json, msg_count, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_conversations(
    limit: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(30);
    let mut stmt = conn.prepare(
        "SELECT id, started_at, summary, message_count FROM conversations
         ORDER BY started_at DESC LIMIT ?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![max], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "summary": row.get::<_, Option<String>>(2)?,
            "message_count": row.get::<_, i64>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_conversation(
    id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let (messages_json, summary, started_at): (String, Option<String>, String) = conn.query_row(
        "SELECT messages, summary, started_at FROM conversations WHERE id=?1",
        rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    let messages: serde_json::Value = serde_json::from_str(&messages_json)
        .map_err(|e| format!("Parse error: {}", e))?;
    Ok(serde_json::json!({
        "id": id,
        "started_at": started_at,
        "summary": summary,
        "messages": messages,
    }))
}

#[tauri::command]
pub fn delete_conversation(
    id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM conversations WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn search_conversations(
    query: String,
    limit: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(20);
    let words: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
    if words.is_empty() {
        return Ok(vec![]);
    }
    let fts_query = words.join(" OR ");
    let mut stmt = conn.prepare(
        "SELECT c.id, c.started_at, c.summary, c.message_count
         FROM conversations_fts fts
         JOIN conversations c ON c.id = fts.rowid
         WHERE conversations_fts MATCH ?1
         ORDER BY rank LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![fts_query, max], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "summary": row.get::<_, Option<String>>(2)?,
            "message_count": row.get::<_, i64>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
pub async fn process_conversation_end(
    messages: Vec<serde_json::Value>,
    conversation_id: i64,
    app: AppHandle,
) -> Result<(), String> {
    // Acquire LLM semaphore — MLX is single-threaded, prevent concurrent inference
    let llm_state = app.state::<LlmBusy>();
    let _permit = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        llm_state.0.acquire(),
    ).await
        .map_err(|_| "LLM busy — timeout".to_string())?
        .map_err(|_| "LLM semaphore closed".to_string())?;
    let client = &app.state::<HttpClient>().0;

    // Build a compact version of the conversation for the LLM
    // ONLY include user messages — assistant messages cause the model to extract
    // hallucinated "facts" from its own responses (e.g. "user doubts their skills"
    // from a proactive message saying "maybe you doubt yourself?")
    let conv_text: String = messages.iter()
        .filter(|m| {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
            role == "user"
        })
        .map(|m| {
            let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
            format!("user: {}", content)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Извлеки личные факты о пользователе из этого разговора.\n\n\
        ПРАВИЛА:\n\
        - Ниже ТОЛЬКО сообщения пользователя. Извлекай факты СТРОГО из того, что он написал.\n\
        - Записывай на том же языке, на котором пишет пользователь\n\
        - Каждый факт должен быть самодостаточным (понятен без контекста)\n\
        - НЕ извлекай: приветствия, общие знания, одноразовые действия (покупки, еда), временные состояния\n\
        - Дата: {today}\n\n\
        ЧТО извлекать:\n\
        1. user: имя, возраст, город, университет, работа, национальность\n\
        2. preferences: что нравится/не нравится, вкусы, привычки\n\
        3. people: друзья, семья, коллеги — имена и отношения\n\
        4. habits: рутины, спорт, режим сна, диета\n\
        5. goals: цели, планы, дедлайны\n\
        6. work: проекты, навыки, карьера\n\n\
        ЧТО НЕ извлекать:\n\
        - \"Привет\" → {{\"facts\": []}} (приветствие, не факт)\n\
        - \"Купил колу за 500\" → {{\"facts\": []}} (одноразовая покупка)\n\
        - \"Сейчас устал\" → {{\"facts\": []}} (временное состояние)\n\
        - \"Земля вращается вокруг Солнца\" → {{\"facts\": []}} (общее знание)\n\
        - НЕ извлекай мета-факты о самооценке, психологии, уверенности (\"сомневается в навыках\", \"беспомощный\")\n\
        - НЕ извлекай факты из ПРИМЕРОВ этого промпта — только из реального разговора\n\
        - НЕ извлекай названия моделей/инструментов из контекста (\"изучает Qwen\", \"использует Claude\")\n\n\
        ПРИМЕРЫ:\n\
        \"Меня зовут Дима, учусь в КазНУ на CS\" → \
        {{\"facts\": [{{\"category\":\"user\",\"key\":\"имя\",\"value\":\"Дима\"}},{{\"category\":\"user\",\"key\":\"университет\",\"value\":\"Учится в КазНУ на CS\"}}]}}\n\
        \"Артём — мой лучший друг, мы вместе кодим\" → \
        {{\"facts\": [{{\"category\":\"people\",\"key\":\"Артём\",\"value\":\"Лучший друг, вместе программируют\"}}]}}\n\n\
        Верни ТОЛЬКО JSON: {{\"summary\": \"1-2 предложения\", \"category\": \"chat|work|health|money|food|hobby|planning|personal\", \"facts\": [...], \"insights\": [{{\"type\": \"decision|goal|open_question\", \"content\": \"...\"}}]}}\n\n\
        Разговор:\n{conv}\n/no_think",
        today = chrono::Local::now().format("%Y-%m-%d"),
        conv = conv_text
    );

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage::text("system", "Ты извлекаешь структурированные данные из разговоров. Верни только валидный JSON."),
            ChatMessage::text("user", &prompt),
        ],
        max_tokens: 1000,
        stream: false,
        temperature: 0.3,
        repetition_penalty: None,
        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
        tools: None,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("MLX memory extraction error {}: {}", status, &body[..body.len().min(200)]));
    }

    let parsed: NonStreamResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let raw = parsed.choices.first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think> tags
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    // Extract JSON from response — handles ```json blocks and surrounding text
    let json_str = {
        let mut s = text.as_str();
        // Strip markdown code blocks first
        if let Some(fence) = s.find("```") {
            let after = &s[fence + 3..];
            let inner = after.strip_prefix("json").unwrap_or(after);
            if let Some(end_fence) = inner.find("```") {
                let candidate = inner[..end_fence].trim();
                if candidate.starts_with('{') { s = candidate; }
            }
        }
        // Find first balanced JSON object via brace counting
        if !s.starts_with('{') {
            if let Some(start) = s.find('{') {
                let bytes = s.as_bytes();
                let (mut depth, mut in_str, mut esc, mut end) = (0i32, false, false, start);
                for i in start..bytes.len() {
                    if esc { esc = false; continue; }
                    match bytes[i] {
                        b'\\' if in_str => esc = true,
                        b'"' => in_str = !in_str,
                        b'{' if !in_str => depth += 1,
                        b'}' if !in_str => { depth -= 1; if depth == 0 { end = i; break; } }
                        _ => {}
                    }
                }
                if depth == 0 && end > start { s = &s[start..=end]; }
            }
        }
        s
    };

    #[derive(Deserialize)]
    struct ExtractionResult {
        summary: Option<String>,
        category: Option<String>,
        #[serde(default)]
        facts: Vec<ExtractedFact>,
        #[serde(default)]
        insights: Vec<ExtractedInsight>,
    }
    #[derive(Deserialize)]
    struct ExtractedFact {
        category: String,
        key: String,
        value: String,
    }
    #[derive(Deserialize)]
    struct ExtractedInsight {
        #[serde(rename = "type")]
        insight_type: String,
        content: String,
    }

    if let Ok(result) = serde_json::from_str::<ExtractionResult>(json_str) {
        let now = chrono::Local::now().to_rfc3339();

        // Update conversation summary + category (scoped DB access)
        {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            if let Some(summary) = &result.summary {
                let _ = conn.execute(
                    "UPDATE conversations SET summary=?1, ended_at=?2, category=?3 WHERE id=?4",
                    rusqlite::params![summary, now, result.category, conversation_id],
                );
            }
        } // conn dropped here

        if result.facts.is_empty() {
            return Ok(());
        }

        // ── Mem0-style dedup pipeline ──
        // 1. Embed extracted facts (async — no DB lock held)
        let fact_texts: Vec<String> = result.facts.iter()
            .map(|f| format!("[{}] {}: {}", f.category, f.key, f.value))
            .collect();
        let embeddings = embed_texts(client, &fact_texts).await.ok();

        // 2. Find similar existing facts for each extracted fact (scoped DB access)
        struct DedupCandidate {
            index: usize,
            similar: Vec<(i64, String, String, String, String, f64)>, // id, cat, key, val, text, distance
        }
        let (dedup_batch, no_similar) = {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            let mut dedup_batch: Vec<DedupCandidate> = Vec::new();
            let mut no_similar: Vec<usize> = Vec::new();

            if let Some(ref embs) = embeddings {
                for (i, fact) in result.facts.iter().enumerate() {
                    if let Some(emb) = embs.get(i) {
                        let hits = search_similar_facts(&conn, emb, 5);
                        let similar: Vec<(i64, String, String, String, String, f64)> = hits.iter()
                            .filter(|(_, dist)| *dist < 0.35)
                            .filter_map(|(fid, dist)| {
                                conn.query_row(
                                    "SELECT id, category, key, value FROM facts WHERE id=?1",
                                    rusqlite::params![fid],
                                    |row| Ok((
                                        row.get::<_, i64>(0)?,
                                        row.get::<_, String>(1)?,
                                        row.get::<_, String>(2)?,
                                        row.get::<_, String>(3)?,
                                    ))
                                ).ok().map(|(id, cat, k, v)| {
                                    let text = format!("[{}] {}={}", cat, k, v);
                                    (id, cat, k, v, text, *dist)
                                })
                            })
                            .collect();

                        if similar.is_empty() {
                            no_similar.push(i);
                        } else {
                            let exact_match = similar.iter().any(|(_, cat, k, _, _, _)| {
                                cat == &fact.category && k == &fact.key
                            });
                            if exact_match {
                                no_similar.push(i);
                            } else {
                                dedup_batch.push(DedupCandidate { index: i, similar });
                            }
                        }
                    } else {
                        no_similar.push(i);
                    }
                }
            } else {
                no_similar = (0..result.facts.len()).collect();
            }
            (dedup_batch, no_similar)
        }; // conn dropped here

        // 3. Direct insert for facts with no similar matches (scoped DB access)
        // ME7: Detect conflicts when existing value differs from new value
        {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            for &idx in &no_similar {
                let fact = &result.facts[idx];
                // Check for conflict: existing fact with same key but different value
                let old_value: Option<String> = conn.query_row(
                    "SELECT value FROM facts WHERE category=?1 AND key=?2",
                    rusqlite::params![fact.category, fact.key],
                    |row| row.get(0),
                ).ok();
                if let Some(ref old_val) = old_value {
                    if old_val != &fact.value {
                        // Memory conflict detected — log it
                        let _ = conn.execute(
                            "INSERT INTO conversation_insights (conversation_id, insight_type, content, created_at)
                             VALUES (?1, 'memory_conflict', ?2, ?3)",
                            rusqlite::params![
                                conversation_id,
                                format!("[{}] {}: '{}' → '{}'", fact.category, fact.key, old_val, fact.value),
                                now
                            ],
                        );
                    }
                }
                let inserted = conn.execute(
                    "INSERT INTO facts (category, key, value, source, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'auto', ?4, ?4)
                     ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
                    rusqlite::params![fact.category, fact.key, fact.value, now],
                );
                if inserted.is_ok() {
                    if let Some(ref embs) = embeddings {
                        if let Some(emb) = embs.get(idx) {
                            if let Ok(fid) = conn.query_row(
                                "SELECT id FROM facts WHERE category=?1 AND key=?2",
                                rusqlite::params![fact.category, fact.key],
                                |row| row.get::<_, i64>(0),
                            ) {
                                store_fact_embedding(&conn, fid, emb);
                            }
                        }
                    }
                }
            }
        } // conn dropped here

        // 4. Batch LLM dedup call for facts with similar matches (async — no DB lock)
        if !dedup_batch.is_empty() {
            let mut prompt_parts = String::from(
                "Сравни новые факты с существующей памятью. Для каждого нового факта реши:\n\
                 - ADD: действительно новая информация — добавить как есть\n\
                 - UPDATE #N: та же тема что у факта #N — объединить значения\n\
                 - NOOP: уже известно — пропустить\n\n\
                 Новые факты:\n"
            );
            for (batch_idx, cand) in dedup_batch.iter().enumerate() {
                let fact = &result.facts[cand.index];
                prompt_parts.push_str(&format!(
                    "{}. [{}] {}: {}\n",
                    batch_idx + 1, fact.category, fact.key, fact.value
                ));
            }
            prompt_parts.push_str("\nСуществующие похожие факты:\n");
            for (batch_idx, cand) in dedup_batch.iter().enumerate() {
                let sim_str: Vec<String> = cand.similar.iter()
                    .map(|(id, _, _, _, text, dist)| {
                        format!("{{id: {}, {} (similarity: {:.0}%)}}", id, text, (1.0 - dist) * 100.0)
                    })
                    .collect();
                prompt_parts.push_str(&format!(
                    "Для #{}: {}\n",
                    batch_idx + 1,
                    sim_str.join(", ")
                ));
            }
            prompt_parts.push_str(
                "\nВерни ТОЛЬКО JSON массив, без другого текста:\n\
                 [{\"index\":1,\"decision\":\"UPDATE\",\"target_id\":5,\"value\":\"объединённое значение\"}, ...]\n\
                 Решения: ADD (вставить новый), UPDATE (обновить target_id с value), NOOP (пропустить)\n\
                 /no_think"
            );

            let dedup_request = ChatRequest {
                model: MODEL.into(),
                messages: vec![
                    ChatMessage::text("system", "Ты дедуплицируешь факты памяти. Верни только валидный JSON массив."),
                    ChatMessage::text("user", &prompt_parts),
                ],
                max_tokens: 400,
                stream: false,
                temperature: 0.2,
                repetition_penalty: None,
                chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
                tools: None,
            };

            // Async LLM call — no DB lock held (30s timeout)
            if let Ok(resp) = client.post(MLX_URL).json(&dedup_request).timeout(std::time::Duration::from_secs(30)).send().await {
                if !resp.status().is_success() { eprintln!("[dedup] MLX error {}", resp.status()); }
                else if let Ok(parsed) = resp.json::<NonStreamResponse>().await {
                    let raw_dedup = parsed.choices.first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_default();

                    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
                    let clean = re.replace_all(&raw_dedup, "").trim().to_string();
                    let json_arr = if let Some(start) = clean.find('[') {
                        if let Some(end) = clean.rfind(']') {
                            &clean[start..=end]
                        } else { &clean }
                    } else { &clean };

                    #[derive(Deserialize)]
                    struct DedupDecision {
                        index: usize,
                        decision: String,
                        #[serde(default)]
                        target_id: Option<i64>,
                        #[serde(default)]
                        value: Option<String>,
                    }

                    if let Ok(decisions) = serde_json::from_str::<Vec<DedupDecision>>(json_arr) {
                        // Execute decisions (scoped DB access)
                        let db = app.state::<HanniDb>();
                        let conn = db.conn();
                        for dec in &decisions {
                            let batch_idx = dec.index.saturating_sub(1);
                            if batch_idx >= dedup_batch.len() { continue; }
                            let fact_idx = dedup_batch[batch_idx].index;
                            let fact = &result.facts[fact_idx];

                            match dec.decision.to_uppercase().as_str() {
                                "ADD" => {
                                    let _ = conn.execute(
                                        "INSERT INTO facts (category, key, value, source, created_at, updated_at)
                                         VALUES (?1, ?2, ?3, 'auto', ?4, ?4)
                                         ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
                                        rusqlite::params![fact.category, fact.key, fact.value, now],
                                    );
                                    if let Some(ref embs) = embeddings {
                                        if let Some(emb) = embs.get(fact_idx) {
                                            if let Ok(fid) = conn.query_row(
                                                "SELECT id FROM facts WHERE category=?1 AND key=?2",
                                                rusqlite::params![fact.category, fact.key],
                                                |row| row.get::<_, i64>(0),
                                            ) {
                                                store_fact_embedding(&conn, fid, emb);
                                            }
                                        }
                                    }
                                }
                                "UPDATE" => {
                                    if let Some(tid) = dec.target_id {
                                        let merged_value = dec.value.as_deref().unwrap_or(&fact.value);
                                        let _ = conn.execute(
                                            "UPDATE facts SET value=?1, updated_at=?2 WHERE id=?3",
                                            rusqlite::params![merged_value, now, tid],
                                        );
                                        if let Some(ref embs) = embeddings {
                                            if let Some(emb) = embs.get(fact_idx) {
                                                store_fact_embedding(&conn, tid, emb);
                                            }
                                        }
                                    }
                                }
                                _ => {} // NOOP or unknown — skip
                            }
                        }
                    }
                }
            }
        }

        // Save conversation insights
        if !result.insights.is_empty() {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            for insight in &result.insights {
                let itype = insight.insight_type.as_str();
                if matches!(itype, "decision" | "open_question" | "topic" | "action_taken") {
                    let _ = conn.execute(
                        "INSERT INTO conversation_insights (conversation_id, insight_type, content, created_at)
                         VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![conversation_id, itype, insight.content, now],
                    );
                }
            }
        }

        // ME8: Trigger profile re-synthesis if new facts were extracted (with 45s timeout)
        let app2 = app.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(45),
                synthesize_user_profile(&app2),
            ).await;
        });
    }

    Ok(())
}

/// Synthesize a natural-language user profile from all stored facts.
/// Stores result in app_settings as 'user_profile'.
pub async fn synthesize_user_profile(app: &AppHandle) -> Result<(), String> {
    let facts_text = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();

        // Collect all facts
        let mut facts = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT category, key, value FROM facts ORDER BY category, updated_at DESC"
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            }) {
                for row in rows.flatten() {
                    facts.push(format!("[{}] {} = {}", row.0, row.1, row.2));
                }
            }
        }

        if facts.is_empty() { return Ok(()); }
        facts.join("\n")
    };

    let client = &app.state::<HttpClient>().0;
    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage::text("system",
                "Ты синтезируешь факты о пользователе в краткий профиль. Пиши на русском. \
                 Верни ТОЛЬКО текст профиля — без JSON, без разметки, без заголовков. \
                 Пиши как будто описываешь друга: естественно, тепло, 3-5 предложений."),
            ChatMessage::text("user", &format!(
                "Собери эти факты в один связный абзац — профиль пользователя:\n\n{}\n/no_think", facts_text)),
        ],
        max_tokens: 400,
        stream: false,
        temperature: 0.4,
        repetition_penalty: Some(1.1),
        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
        tools: None,
    };

    let response = client.post(MLX_URL).json(&request).timeout(std::time::Duration::from_secs(30)).send().await
        .map_err(|e| format!("Profile synthesis error: {}", e))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Profile synthesis MLX error {}: {}", status, &body[..body.len().min(200)]));
    }
    let parsed: NonStreamResponse = response.json().await
        .map_err(|e| format!("Profile parse error: {}", e))?;

    let profile = parsed.choices.first()
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_default();

    if !profile.is_empty() {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let _ = conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('user_profile', ?1) \
             ON CONFLICT(key) DO UPDATE SET value=?1",
            rusqlite::params![profile],
        );
    }

    Ok(())
}
