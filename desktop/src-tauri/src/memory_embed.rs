// memory_embed.rs — Semantic memory helpers (sqlite-vec + fastembed), embeddings, proactive settings
use crate::types::*;
use serde::Deserialize;
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
        // Collapse any line break / control char WITHIN a fact line to a space so
        // one fact stays one line. A synced/MCP fact value with an embedded newline
        // (incl. Unicode LS/PS/NEL that str::lines misses) could otherwise inject
        // extra lines — this builder feeds the proactive prompt raw, and chat wraps
        // its output in quote_for_prompt. Facts remain separated by the join \n.
        lines.iter().map(|l| l.chars().map(|c| match c {
            '\n' | '\r' | '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => ' ',
            c if c.is_control() => ' ',
            c => c,
        }).collect::<String>()).collect::<Vec<_>>().join("\n")
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
