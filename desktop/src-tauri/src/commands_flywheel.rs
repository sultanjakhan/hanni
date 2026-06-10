// commands_flywheel.rs — Data flywheel, training export, finetune, message ratings, tab blocks, activity timeline/weekly, body records
use crate::types::*;
use crate::prompts::SYSTEM_PROMPT;
use chrono::Timelike;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::io::Write;

// ── Data Flywheel ──
// ── v0.18.0 Wave 3: Data Flywheel (ML7) ──

#[tauri::command]
pub fn get_flywheel_status(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    // Count accumulated thumbs-up pairs
    let thumbs_up: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_feedback WHERE rating = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let exported: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_feedback WHERE rating = 1 AND exported = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let new_pairs = thumbs_up - exported;
    // Last cycle
    let last_cycle: Option<(String, String, i64, Option<f64>)> = conn
        .query_row(
            "SELECT started_at, status, train_pairs, eval_score FROM flywheel_cycles ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok();
    // Total cycles
    let total_cycles: i64 = conn
        .query_row("SELECT COUNT(*) FROM flywheel_cycles", [], |row| row.get(0))
        .unwrap_or(0);
    // Adapter status
    let adapter_dir = hanni_data_dir().join("lora-adapter");
    let adapter_exists = adapter_dir.exists();
    Ok(serde_json::json!({
        "thumbs_up_total": thumbs_up,
        "exported": exported,
        "new_pairs": new_pairs,
        "total_cycles": total_cycles,
        "adapter_exists": adapter_exists,
        "ready_to_train": new_pairs >= 20,
        "last_cycle": last_cycle.map(|(date, status, pairs, score)| serde_json::json!({
            "date": date, "status": status, "train_pairs": pairs, "eval_score": score,
        })),
    }))
}

#[tauri::command]
pub fn get_flywheel_history(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare("SELECT id, started_at, finished_at, status, train_pairs, eval_score, notes FROM flywheel_cycles ORDER BY id DESC LIMIT 20")
        .map_err(|e| format!("DB: {}", e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "started_at": row.get::<_, String>(1)?,
                "finished_at": row.get::<_, Option<String>>(2)?,
                "status": row.get::<_, String>(3)?,
                "train_pairs": row.get::<_, i64>(4)?,
                "eval_score": row.get::<_, Option<f64>>(5)?,
                "notes": row.get::<_, Option<String>>(6)?,
            }))
        })
        .map_err(|e| format!("DB: {}", e))?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    Ok(results)
}

#[tauri::command]
pub async fn run_flywheel_cycle(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    // Create cycle record
    let cycle_id: i64 = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO flywheel_cycles (started_at, status) VALUES (?1, 'running')",
            rusqlite::params![now],
        )
        .map_err(|e| format!("DB: {}", e))?;
        conn.last_insert_rowid()
    };
    // Step 1: Export training data
    let export_result = {
        let conn = db.conn();
        // Reuse export logic inline — count available pairs
        let train_pairs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_feedback WHERE rating = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        train_pairs
    };
    // Update cycle with pair count
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE flywheel_cycles SET train_pairs = ?1 WHERE id = ?2",
            rusqlite::params![export_result, cycle_id],
        )
        .map_err(|e| format!("DB: {}", e))?;
    }
    // Step 2: Run finetune.py (reuse existing logic)
    let finetune_output = match tokio::task::spawn_blocking(|| {
        let script = hanni_data_dir().join("finetune.py");
        if !script.exists() {
            // Try relative path
            let cwd_script = std::env::current_dir()
                .map(|d| d.join("finetune.py"))
                .unwrap_or_default();
            if cwd_script.exists() {
                return std::process::Command::new("python3")
                    .arg(cwd_script)
                    .output()
                    .map_err(|e| format!("Run: {}", e));
            }
            return Err("finetune.py not found".into());
        }
        std::process::Command::new("python3")
            .arg(script)
            .output()
            .map_err(|e| format!("Run: {}", e))
    })
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                Ok(format!("{}\n{}", stdout, stderr))
            } else {
                Err(format!("Finetune failed: {}", stderr))
            }
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("Task: {}", e)),
    };
    // Update cycle status
    let finished = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let status = if finetune_output.is_ok() {
        "completed"
    } else {
        "failed"
    };
    let notes = match &finetune_output {
        Ok(s) => s.chars().take(500).collect::<String>(),
        Err(e) => e.chars().take(500).collect::<String>(),
    };
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE flywheel_cycles SET finished_at = ?1, status = ?2, notes = ?3 WHERE id = ?4",
            rusqlite::params![finished, status, notes, cycle_id],
        )
        .map_err(|e| format!("DB: {}", e))?;
    }
    Ok(serde_json::json!({
        "cycle_id": cycle_id,
        "status": status,
        "train_pairs": export_result,
        "notes": notes,
    }))
}

// ── Training Data Export ──
// ── Phase 3: Training Data Export ──

#[tauri::command]
pub fn get_training_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();

    let conv_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations WHERE message_count >= 4",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let total_messages: i64 = conn.query_row(
        "SELECT COALESCE(SUM(message_count), 0) FROM conversations WHERE message_count >= 4",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let date_range: (String, String) = conn.query_row(
        "SELECT COALESCE(MIN(started_at), ''), COALESCE(MAX(started_at), '') FROM conversations WHERE message_count >= 4",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).unwrap_or(("".into(), "".into()));

    Ok(serde_json::json!({
        "conversations": conv_count,
        "total_messages": total_messages,
        "earliest": date_range.0,
        "latest": date_range.1,
    }))
}

#[tauri::command]
pub fn export_training_data(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();

    // Load all feedback ratings into a map: conversation_id -> { message_index -> rating }
    let mut feedback_map: HashMap<i64, HashMap<i64, i64>> = HashMap::new();
    {
        let mut fb_stmt = conn.prepare(
            "SELECT conversation_id, message_index, rating FROM message_feedback"
        ).map_err(|e| format!("DB error: {}", e))?;
        let fb_rows = fb_stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        }).map_err(|e| format!("Query error: {}", e))?;
        for row in fb_rows.filter_map(|r| r.ok()) {
            feedback_map.entry(row.0).or_default().insert(row.1, row.2);
        }
    }

    let mut stmt = conn.prepare(
        "SELECT id, messages, summary FROM conversations WHERE message_count >= 4 ORDER BY started_at"
    ).map_err(|e| format!("DB error: {}", e))?;

    let rows: Vec<(i64, String, Option<String>)> = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
    })
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let mut rated_examples: Vec<serde_json::Value> = Vec::new();
    let mut unrated_examples: Vec<serde_json::Value> = Vec::new();

    for (conv_id, messages_json, _summary) in &rows {
        let messages: Vec<(String, String)> = match serde_json::from_str(messages_json) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let ratings = feedback_map.get(conv_id);
        let has_positive = ratings.map_or(false, |r| r.values().any(|&v| v == 1));

        // Filter: skip if fewer than 2 real messages
        let real_msgs: Vec<&(String, String)> = messages.iter()
            .filter(|(role, content)| {
                (role == "user" || role == "assistant")
                && !content.starts_with("[Action result:")
                && !content.contains("```action")
            })
            .collect();

        if real_msgs.len() < 2 {
            continue;
        }

        let mut chat_msgs = vec![serde_json::json!({
            "role": "system",
            "content": SYSTEM_PROMPT
        })];

        for (idx, (role, content)) in messages.iter().enumerate() {
            if role == "user" || role == "assistant" {
                // Skip assistant messages with negative ratings
                if role == "assistant" {
                    if let Some(r) = ratings {
                        if r.get(&(idx as i64)) == Some(&-1) {
                            continue;
                        }
                    }
                }
                let clean = content.trim_end_matches(" /no_think").to_string();
                chat_msgs.push(serde_json::json!({
                    "role": role,
                    "content": clean,
                }));
            }
        }

        let example = serde_json::json!({ "messages": chat_msgs });
        if has_positive {
            rated_examples.push(example);
        } else {
            unrated_examples.push(example);
        }
    }

    // Prioritize rated conversations: rated first, then unrated
    let mut training_examples = rated_examples;
    training_examples.extend(unrated_examples);

    if training_examples.is_empty() {
        return Err("No conversations suitable for training".into());
    }

    // 80/10/10 split (mlx_lm wants train/valid/test)
    let total = training_examples.len();
    let train_end = (total as f64 * 0.8).ceil() as usize;
    let valid_end = train_end + (total as f64 * 0.1).ceil() as usize;
    let train = &training_examples[..train_end];
    let valid = &training_examples[train_end..valid_end.min(total)];
    let test = &training_examples[valid_end.min(total)..];

    // Write files
    let output_dir = hanni_data_dir().join("training");
    std::fs::create_dir_all(&output_dir).map_err(|e| format!("Dir error: {}", e))?;

    let train_path = output_dir.join("train.jsonl");
    let valid_path = output_dir.join("valid.jsonl");
    let test_path = output_dir.join("test.jsonl");

    for (path, data) in [(&train_path, train), (&valid_path, valid), (&test_path, test)] {
        let mut f = std::fs::File::create(path).map_err(|e| format!("File error: {}", e))?;
        for example in data {
            writeln!(f, "{}", serde_json::to_string(example).unwrap_or_default())
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }

    // Mark feedback as exported
    conn.execute("UPDATE message_feedback SET exported = 1 WHERE exported = 0", [])
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::json!({
        "train_path": train_path.to_string_lossy(),
        "valid_path": valid_path.to_string_lossy(),
        "test_path": test_path.to_string_lossy(),
        "train_count": train.len(),
        "valid_count": valid.len(),
        "test_count": test.len(),
        "total": total,
    }))
}

#[tauri::command]
pub fn get_adapter_status() -> Result<serde_json::Value, String> {
    let adapter_dir = hanni_data_dir().join("lora-adapter");
    let meta_path = adapter_dir.join("hanni_meta.json");
    let adapter_exists = adapter_dir.join("adapters.safetensors").exists()
        || adapter_dir.join("adapter_config.json").exists();

    let meta: Option<serde_json::Value> = if meta_path.exists() {
        std::fs::read_to_string(&meta_path).ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    } else {
        None
    };

    Ok(serde_json::json!({
        "exists": adapter_exists,
        "meta": meta,
    }))
}

#[tauri::command]
pub async fn run_finetune() -> Result<String, String> {
    let finetune_script = std::env::current_dir()
        .unwrap_or_default()
        .join("finetune.py");

    // Also check relative to the binary
    let script_path = if finetune_script.exists() {
        finetune_script
    } else {
        // In packaged .app, try next to the Resources dir
        let alt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../finetune.py");
        if alt.exists() { alt } else { finetune_script }
    };

    if !script_path.exists() {
        return Err(format!("finetune.py not found at {}", script_path.display()));
    }

    let output = Command::new("python3")
        .arg(&script_path)
        .output()
        .map_err(|e| format!("Failed to start finetune: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(format!("{}\n{}", stdout, stderr))
    } else {
        Err(format!("Fine-tuning failed:\n{}\n{}", stdout, stderr))
    }
}

#[tauri::command]
pub fn rate_message(db: tauri::State<'_, HanniDb>, conversation_id: i64, message_index: i64, rating: i64) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR REPLACE INTO message_feedback (conversation_id, message_index, rating, created_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![conversation_id, message_index, rating],
    ).map_err(|e| format!("DB error: {}", e))?;

    // ML1: On thumbs-up, export training pair to JSONL for future fine-tuning
    if rating == 1 {
        if let Ok(messages_json) = conn.query_row(
            "SELECT messages FROM conversations WHERE id=?1",
            rusqlite::params![conversation_id],
            |row| row.get::<_, String>(0),
        ) {
            if let Ok(msgs) = serde_json::from_str::<Vec<serde_json::Value>>(&messages_json) {
                let idx = message_index as usize;
                if idx < msgs.len() && msgs[idx].get("role").and_then(|r| r.as_str()) == Some("assistant") {
                    // Find preceding user message
                    let user_msg = (0..idx).rev().find_map(|i| {
                        if msgs[i].get("role").and_then(|r| r.as_str()) == Some("user") {
                            msgs[i].get("content").and_then(|c| c.as_str()).map(|s| s.to_string())
                        } else { None }
                    });
                    if let (Some(user), Some(assistant)) = (user_msg, msgs[idx].get("content").and_then(|c| c.as_str())) {
                        let training_path = hanni_data_dir().join("training_pairs.jsonl");
                        let entry = serde_json::json!({
                            "messages": [
                                {"role": "user", "content": user},
                                {"role": "assistant", "content": assistant}
                            ],
                            "timestamp": chrono::Local::now().to_rfc3339()
                        });
                        if let Ok(line) = serde_json::to_string(&entry) {
                            let _ = std::fs::OpenOptions::new()
                                .create(true).append(true)
                                .open(&training_path)
                                .and_then(|mut f| {
                                    use std::io::Write;
                                    writeln!(f, "{}", line)
                                });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_message_ratings(db: tauri::State<'_, HanniDb>, conversation_id: i64) -> Result<Vec<(i64, i64)>, String> {
    let conn = db.read();
    let mut stmt = conn.prepare(
        "SELECT message_index, rating FROM message_feedback WHERE conversation_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![conversation_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

// ── Tab Page Blocks ──

#[tauri::command]
pub fn get_tab_blocks(db: tauri::State<'_, HanniDb>, tab_id: String, sub_tab: String) -> Result<Option<String>, String> {
    let conn = db.conn();
    let result = conn.query_row(
        "SELECT blocks_json FROM tab_page_blocks WHERE tab_id = ?1 AND sub_tab = ?2",
        rusqlite::params![tab_id, sub_tab],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(json) => Ok(Some(json)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("DB error: {}", e)),
    }
}

#[tauri::command]
pub fn save_tab_blocks(db: tauri::State<'_, HanniDb>, tab_id: String, sub_tab: String, blocks_json: String) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tab_page_blocks (tab_id, sub_tab, blocks_json, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(tab_id, sub_tab) DO UPDATE SET blocks_json = ?3, updated_at = ?4",
        rusqlite::params![tab_id, sub_tab, blocks_json, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Activity Tracking ──

#[tauri::command]
pub fn get_activity_timeline(db: tauri::State<'_, HanniDb>, date: Option<String>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    // Get all snapshots for the day
    let mut stmt = conn.prepare(
        "SELECT captured_at, frontmost_app, browser_url, window_title, category, idle_secs, music_playing, productive_min, distraction_min, screen_locked
         FROM activity_snapshots
         WHERE captured_at LIKE ?1
         ORDER BY captured_at ASC"
    ).map_err(|e| e.to_string())?;

    let snapshots: Vec<serde_json::Value> = stmt.query_map(
        [format!("{}%", target_date)],
        |row| {
            Ok(serde_json::json!({
                "time": row.get::<_, String>(0).unwrap_or_default(),
                "app": row.get::<_, String>(1).unwrap_or_default(),
                "url": row.get::<_, String>(2).unwrap_or_default(),
                "title": row.get::<_, String>(3).unwrap_or_default(),
                "category": row.get::<_, String>(4).unwrap_or("other".into()),
                "idle": row.get::<_, f64>(5).unwrap_or(0.0),
                "music": row.get::<_, String>(6).unwrap_or_default(),
                "productive": row.get::<_, f64>(7).unwrap_or(0.0),
                "distraction": row.get::<_, f64>(8).unwrap_or(0.0),
                "screen_locked": row.get::<_, i32>(9).unwrap_or(0),
            }))
        }
    ).map_err(|e| e.to_string())?.flatten().collect();

    // Aggregate by category and app, separating active vs AFK
    let mut by_category: HashMap<String, f64> = HashMap::new();
    let mut by_app: HashMap<String, f64> = HashMap::new();
    let interval_min = 0.5_f64; // each 30-sec snapshot = 0.5 min

    let mut active_minutes = 0.0_f64;
    let mut idle_minutes = 0.0_f64;
    let mut locked_minutes = 0.0_f64;

    for s in &snapshots {
        let idle = s["idle"].as_f64().unwrap_or(0.0);
        let locked = s["screen_locked"].as_i64().unwrap_or(0) == 1;
        let cat = s["category"].as_str().unwrap_or("other").to_string();
        let app = s["app"].as_str().unwrap_or("").to_string();

        if locked {
            locked_minutes += interval_min;
        } else if cat == "afk" || idle >= 120.0 {
            idle_minutes += interval_min;
        } else {
            active_minutes += interval_min;
            *by_category.entry(cat).or_insert(0.0) += interval_min;
            if !app.is_empty() {
                *by_app.entry(app).or_insert(0.0) += interval_min;
            }
        }
    }

    let prod_min: f64 = snapshots.iter().map(|s| s["productive"].as_f64().unwrap_or(0.0)).sum();
    let dist_min: f64 = snapshots.iter().map(|s| s["distraction"].as_f64().unwrap_or(0.0)).sum();
    let total_min = active_minutes + idle_minutes + locked_minutes;
    // Unknown = time of day elapsed minus tracked time
    let day_elapsed_min = if target_date == chrono::Local::now().format("%Y-%m-%d").to_string() {
        let now = chrono::Local::now();
        (now.hour() as f64) * 60.0 + now.minute() as f64
    } else {
        1440.0 // full day
    };
    let unknown_minutes = (day_elapsed_min - total_min).max(0.0);

    // Sort apps by time descending
    let mut top_apps: Vec<(String, f64)> = by_app.into_iter().collect();
    top_apps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    top_apps.truncate(10);

    Ok(serde_json::json!({
        "date": target_date,
        "total_minutes": total_min,
        "active_minutes": active_minutes,
        "idle_minutes": idle_minutes,
        "locked_minutes": locked_minutes,
        "productive_minutes": prod_min,
        "distraction_minutes": dist_min,
        "unknown_minutes": unknown_minutes,
        "snapshots_count": snapshots.len(),
        "categories": by_category,
        "top_apps": top_apps.iter().map(|(app, min)| serde_json::json!({"app": app, "minutes": min})).collect::<Vec<_>>(),
        "timeline": snapshots,
    }))
}

#[tauri::command]
pub fn get_activity_weekly(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut days = Vec::new();

    for i in 0..7 {
        let date = (chrono::Local::now() - chrono::Duration::days(i)).format("%Y-%m-%d").to_string();
        let (prod, dist, count): (f64, f64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(productive_min), 0), COALESCE(SUM(distraction_min), 0), COUNT(*)
             FROM activity_snapshots WHERE captured_at LIKE ?1",
            [format!("{}%", date)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap_or((0.0, 0.0, 0));
        let total = prod + dist;

        // Top category for the day
        let top_cat: String = conn.query_row(
            "SELECT category FROM activity_snapshots WHERE captured_at LIKE ?1
             GROUP BY category ORDER BY COUNT(*) DESC LIMIT 1",
            [format!("{}%", date)],
            |row| row.get(0),
        ).unwrap_or_else(|_| "none".into());

        days.push(serde_json::json!({
            "date": date,
            "total_minutes": total,
            "productive_minutes": prod,
            "distraction_minutes": dist,
            "snapshots": count,
            "top_category": top_cat,
        }));
    }

    Ok(serde_json::json!({ "days": days }))
}

// ── Body Records (3D Body Tab) ──

#[tauri::command]
pub async fn create_body_record(
    db: tauri::State<'_, HanniDb>,
    zone: String,
    zone_label: String,
    record_type: String,
    intensity: Option<i32>,
    pain_type: Option<String>,
    goal_type: Option<String>,
    value: Option<f64>,
    unit: Option<String>,
    treatment_type: Option<String>,
    note: Option<String>,
    date: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let n = note.unwrap_or_default();
    conn.execute(
        "INSERT INTO body_records (zone, zone_label, record_type, intensity, pain_type, goal_type, value, unit, treatment_type, note, date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![zone, zone_label, record_type, intensity, pain_type, goal_type, value, unit, treatment_type, n, d],
    ).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(serde_json::json!({ "id": id }))
}

#[tauri::command]
pub async fn get_body_records(
    db: tauri::State<'_, HanniDb>,
    zone: Option<String>,
    record_type: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = db.read();
    let mut sql = "SELECT id, zone, zone_label, record_type, intensity, pain_type, goal_type, value, unit, treatment_type, note, date, created_at FROM body_records WHERE 1=1".to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
    if let Some(z) = &zone {
        sql.push_str(&format!(" AND zone = ?{}", params.len() + 1));
        params.push(Box::new(z.clone()));
    }
    if let Some(rt) = &record_type {
        sql.push_str(&format!(" AND record_type = ?{}", params.len() + 1));
        params.push(Box::new(rt.clone()));
    }
    sql.push_str(" ORDER BY date DESC, created_at DESC LIMIT 200");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "zone": row.get::<_, String>(1)?,
            "zone_label": row.get::<_, String>(2)?,
            "record_type": row.get::<_, String>(3)?,
            "intensity": row.get::<_, Option<i32>>(4)?,
            "pain_type": row.get::<_, Option<String>>(5)?,
            "goal_type": row.get::<_, Option<String>>(6)?,
            "value": row.get::<_, Option<f64>>(7)?,
            "unit": row.get::<_, Option<String>>(8)?,
            "treatment_type": row.get::<_, Option<String>>(9)?,
            "note": row.get::<_, String>(10)?,
            "date": row.get::<_, String>(11)?,
            "created_at": row.get::<_, String>(12)?,
        }))
    }).map_err(|e| e.to_string())?;
    let records: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!(records))
}

#[tauri::command]
pub async fn delete_body_record(db: tauri::State<'_, HanniDb>, id: i64) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM body_records WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_body_zones_summary(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.read();
    let mut stmt = conn.prepare(
        "SELECT zone, zone_label, record_type, COUNT(*) as cnt,
                MAX(CASE WHEN record_type='pain' THEN intensity ELSE NULL END) as max_intensity
         FROM body_records GROUP BY zone, record_type"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "zone": row.get::<_, String>(0)?,
            "zone_label": row.get::<_, String>(1)?,
            "record_type": row.get::<_, String>(2)?,
            "count": row.get::<_, i64>(3)?,
            "max_intensity": row.get::<_, Option<i32>>(4)?,
        }))
    }).map_err(|e| e.to_string())?;
    let records: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!(records))
}
