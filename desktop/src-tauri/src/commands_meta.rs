// commands_meta.rs — Mindset, blocklist, goals, settings, home, contacts, properties, views, integrations, model info, health check, updater, HTTP API, focus
use crate::types::*;
use crate::chat::chat_inner;
use crate::macos::run_osascript;
use crate::commands_data::load_tracker_data;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use std::sync::atomic::Ordering;
use std::process::{Command, Child};
use std::path::PathBuf;
use std::collections::HashMap;

/// Global callback map for auto_eval HTTP → JS → Rust roundtrip
pub struct AutoEvalCallbacks(pub std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>);

#[tauri::command]
pub fn auto_eval_callback(cb_id: String, result: String, state: tauri::State<'_, AutoEvalCallbacks>) {
    if let Some(tx) = state.0.lock().unwrap().remove(&cb_id) {
        let _ = tx.send(result);
    }
}

// ── Focus Commands ──

#[tauri::command]
pub fn start_focus(
    duration_minutes: u64,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());

    if state.active {
        return Err("Focus mode is already active".into());
    }

    // Load default config if not provided
    let blocker_config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    let default_apps = vec!["Telegram".to_string(), "Discord".to_string(), "Slack".to_string()];
    let default_sites = vec![
        "youtube.com".to_string(), "twitter.com".to_string(), "x.com".to_string(),
        "instagram.com".to_string(), "facebook.com".to_string(), "tiktok.com".to_string(),
        "reddit.com".to_string(), "vk.com".to_string(), "netflix.com".to_string(),
    ];

    let block_apps = apps.unwrap_or_else(|| {
        if blocker_config_path.exists() {
            std::fs::read_to_string(&blocker_config_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|cfg| cfg["apps"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }))
                .unwrap_or_else(|| default_apps.clone())
        } else {
            default_apps.clone()
        }
    });

    let block_sites = sites.unwrap_or_else(|| {
        if blocker_config_path.exists() {
            std::fs::read_to_string(&blocker_config_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|cfg| cfg["sites"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }))
                .unwrap_or_else(|| default_sites.clone())
        } else {
            default_sites.clone()
        }
    });

    // Sanitize site names — only allow valid hostname chars
    let safe_site = |s: &str| -> String {
        s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect()
    };
    // Build hosts entries
    let mut hosts_entries = String::new();
    for site in &block_sites {
        let s = safe_site(site);
        if s.is_empty() { continue; }
        hosts_entries.push_str(&format!("127.0.0.1 {}\n127.0.0.1 www.{}\n", s, s));
    }

    // Write to /etc/hosts using osascript for sudo
    let hosts_block = format!(
        "# === HANNI FOCUS BLOCKER ===\n{}# === END HANNI FOCUS BLOCKER ===",
        hosts_entries
    );

    let script = format!(
        "do shell script \"printf '\\n{}' >> /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges",
        hosts_block.replace("'", "'\\''").replace("\n", "\\n")
    );
    run_osascript(&script).map_err(|e| format!("Failed to set focus mode (admin needed): {}", e))?;

    // Quit blocked apps — sanitize names to prevent AppleScript injection
    let safe_app = |s: &str| -> String {
        s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '.').collect()
    };
    for app_name in &block_apps {
        let name = safe_app(app_name);
        if name.is_empty() { continue; }
        let _ = run_osascript(&format!(
            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
            name, name
        ));
    }

    let end_time = chrono::Local::now() + chrono::Duration::minutes(duration_minutes as i64);
    state.active = true;
    state.end_time = Some(end_time);
    state.blocked_apps = block_apps;
    state.blocked_sites = block_sites;
    state.monitor_running.store(true, Ordering::Relaxed);

    Ok(format!("Focus mode started for {} minutes", duration_minutes))
}

#[tauri::command]
pub fn stop_focus(focus: tauri::State<'_, FocusManager>) -> Result<String, String> {
    let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());

    if !state.active {
        return Ok("Focus mode is not active".into());
    }

    // Remove HANNI FOCUS BLOCKER section from /etc/hosts
    let script = "do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges";
    let _ = run_osascript(script);

    state.active = false;
    state.end_time = None;
    state.blocked_apps.clear();
    state.blocked_sites.clear();
    state.monitor_running.store(false, Ordering::Relaxed);

    Ok("Focus mode stopped".into())
}

#[tauri::command]
pub fn get_focus_status(focus: tauri::State<'_, FocusManager>) -> Result<FocusStatus, String> {
    let state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
    let remaining = if let Some(end) = state.end_time {
        let diff = end - chrono::Local::now();
        if diff.num_seconds() > 0 { diff.num_seconds() as u64 } else { 0 }
    } else {
        0
    };
    Ok(FocusStatus {
        active: state.active,
        remaining_seconds: remaining,
        blocked_apps: state.blocked_apps.clone(),
        blocked_sites: state.blocked_sites.clone(),
    })
}

#[tauri::command]
pub fn update_blocklist(apps: Option<Vec<String>>, sites: Option<Vec<String>>) -> Result<String, String> {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Dir error: {}", e))?;
    }

    let mut config: serde_json::Value = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(a) = apps {
        config["apps"] = serde_json::json!(a);
    }
    if let Some(s) = sites {
        config["sites"] = serde_json::json!(s);
    }

    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {}", e))?;
    Ok("Blocklist updated".into())
}

// ── Mindset, Blocklist, Goals, Settings, Home, Contacts, Properties, Views ──
// ── v0.8.0: Mindset commands ──

#[tauri::command]
pub fn save_journal_entry(
    date: Option<String>, mood: i32, energy: i32, stress: i32,
    gratitude: Option<String>, reflection: Option<String>,
    wins: Option<String>, struggles: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO journal_entries (date, mood, energy, stress, gratitude, reflection, wins, struggles, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(date) DO UPDATE SET mood=?2, energy=?3, stress=?4, gratitude=?5, reflection=?6, wins=?7, struggles=?8",
        rusqlite::params![d, mood, energy, stress, gratitude.unwrap_or_default(),
            reflection.unwrap_or_default(), wins.unwrap_or_default(), struggles.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_journal_entries(period: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let days = period.unwrap_or(30);
    let since = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, date, mood, energy, stress, gratitude, reflection, wins, struggles FROM journal_entries WHERE date>=?1 ORDER BY date DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![since], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
            "mood": row.get::<_, i32>(2)?, "energy": row.get::<_, i32>(3)?,
            "stress": row.get::<_, i32>(4)?, "gratitude": row.get::<_, String>(5)?,
            "reflection": row.get::<_, String>(6)?, "wins": row.get::<_, String>(7)?,
            "struggles": row.get::<_, String>(8)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_journal_entry(date: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    conn.query_row(
        "SELECT id, date, mood, energy, stress, gratitude, reflection, wins, struggles FROM journal_entries WHERE date=?1",
        rusqlite::params![date], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
                "mood": row.get::<_, i32>(2)?, "energy": row.get::<_, i32>(3)?,
                "stress": row.get::<_, i32>(4)?, "gratitude": row.get::<_, String>(5)?,
                "reflection": row.get::<_, String>(6)?, "wins": row.get::<_, String>(7)?,
                "struggles": row.get::<_, String>(8)?,
            }))
        },
    ).map_err(|e| format!("Not found: {}", e))
}

#[tauri::command]
pub fn log_mood(mood: i32, note: Option<String>, trigger: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    conn.execute(
        "INSERT INTO mood_log (date, time, mood, note, trigger_text, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![now.format("%Y-%m-%d").to_string(), now.format("%H:%M").to_string(),
            mood, note.unwrap_or_default(), trigger.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_mood_history(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = days.unwrap_or(7);
    let since = (chrono::Local::now() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, date, time, mood, note, trigger_text FROM mood_log WHERE date>=?1 ORDER BY date DESC, time DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![since], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
            "time": row.get::<_, String>(2)?, "mood": row.get::<_, i32>(3)?,
            "note": row.get::<_, String>(4)?, "trigger": row.get::<_, String>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_principle(title: String, description: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO principles (title, description, category, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![title, description.unwrap_or_default(), category.unwrap_or_else(|| "discipline".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_principles(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, description, category, active FROM principles ORDER BY category, created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?, "category": row.get::<_, String>(3)?,
            "active": row.get::<_, i32>(4)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_principle(id: i64, active: Option<bool>, title: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(a) = active { conn.execute("UPDATE principles SET active=?1 WHERE id=?2", rusqlite::params![a as i32, id]).map_err(|e| format!("DB error: {}", e))?; }
    if let Some(t) = title { conn.execute("UPDATE principles SET title=?1 WHERE id=?2", rusqlite::params![t, id]).map_err(|e| format!("DB error: {}", e))?; }
    Ok(())
}

#[tauri::command]
pub fn delete_principle(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM principles WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_mindset_check(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let (avg_mood, avg_energy, avg_stress, journal_count): (f64, f64, f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(mood),3), COALESCE(AVG(energy),3), COALESCE(AVG(stress),3), COUNT(*)
         FROM journal_entries WHERE date>=?1",
        rusqlite::params![week_ago], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap_or((3.0, 3.0, 3.0, 0));
    let principles_count: i64 = conn.query_row("SELECT COUNT(*) FROM principles WHERE active=1", [], |row| row.get(0)).unwrap_or(0);
    Ok(serde_json::json!({
        "avg_mood": format!("{:.1}", avg_mood), "avg_energy": format!("{:.1}", avg_energy),
        "avg_stress": format!("{:.1}", avg_stress), "journal_streak": journal_count,
        "active_principles": principles_count,
    }))
}

// ── v0.8.0: Blocklist commands ──

#[tauri::command]
pub fn add_to_blocklist(block_type: String, value: String, schedule: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO blocklist (type, value, schedule, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![block_type, value, schedule, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn remove_from_blocklist(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM blocklist WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_blocklist(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, type, value, schedule, active FROM blocklist ORDER BY type, value"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "type": row.get::<_, String>(1)?,
            "value": row.get::<_, String>(2)?, "schedule": row.get::<_, Option<String>>(3)?,
            "active": row.get::<_, i32>(4)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn toggle_blocklist_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE blocklist SET active = 1 - active WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.8.0: Goals & Settings commands ──

#[tauri::command]
pub fn create_goal(tab_name: String, title: String, target_value: f64, unit: Option<String>, deadline: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tab_goals (tab_name, title, target_value, unit, deadline, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![tab_name, title, target_value, unit.unwrap_or_default(), deadline, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_goals(tab_name: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(t) = tab_name {
        let mut stmt = conn.prepare(
            "SELECT id, tab_name, title, target_value, current_value, unit, deadline, status FROM tab_goals WHERE tab_name=?1 AND status='active' ORDER BY created_at"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![t], |row| goal_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, tab_name, title, target_value, current_value, unit, deadline, status FROM tab_goals WHERE status='active' ORDER BY tab_name, created_at"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| goal_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn goal_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    let target: f64 = row.get(3)?;
    let current: f64 = row.get(4)?;
    let pct = if target > 0.0 { (current / target * 100.0).min(100.0) } else { 0.0 };
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "tab_name": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?, "target_value": target,
        "current_value": current, "unit": row.get::<_, String>(5)?,
        "deadline": row.get::<_, Option<String>>(6)?, "status": row.get::<_, String>(7)?,
        "percent": format!("{:.0}", pct),
    }))
}

#[tauri::command]
pub fn update_goal(id: i64, current_value: Option<f64>, status: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = current_value { conn.execute("UPDATE tab_goals SET current_value=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {}", e))?; }
    if let Some(s) = status { conn.execute("UPDATE tab_goals SET status=?1 WHERE id=?2", rusqlite::params![s, id]).map_err(|e| format!("DB error: {}", e))?; }
    Ok(())
}

#[tauri::command]
pub fn delete_goal(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM tab_goals WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn set_app_setting(key: String, value: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
        rusqlite::params![key, value],
    ).map_err(|e| format!("DB error: {}", e))?;
    // Sync calendar toggle to static flag
    if key == "apple_calendar_enabled" {
        APPLE_CALENDAR_DISABLED.store(value == "false", Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_setting(key: String, db: tauri::State<'_, HanniDb>) -> Result<Option<String>, String> {
    let conn = db.conn();
    let result: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1", rusqlite::params![key], |row| row.get(0),
    ).ok();
    Ok(result)
}

// ── Home Items ──

#[tauri::command]
pub fn add_home_item(name: String, category: String, quantity: Option<f64>, unit: Option<String>, location: String, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("INSERT INTO home_items (name,category,quantity,unit,location,notes) VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![name, category, quantity, unit, location, notes]).map_err(|e| e.to_string())?;
    Ok("added".into())
}

#[tauri::command]
pub fn get_home_items(category: Option<String>, needed_only: bool, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut sql = "SELECT id,name,category,quantity,unit,location,needed,notes,created_at FROM home_items".to_string();
    let mut conditions = Vec::new();
    if let Some(ref c) = category { conditions.push(format!("category='{}'", c)); }
    if needed_only { conditions.push("needed=1".to_string()); }
    if !conditions.is_empty() { sql += &format!(" WHERE {}", conditions.join(" AND ")); }
    sql += " ORDER BY needed DESC, name ASC";
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,i64>(0)?, "name": row.get::<_,String>(1)?,
            "category": row.get::<_,String>(2)?, "quantity": row.get::<_,Option<f64>>(3)?,
            "unit": row.get::<_,Option<String>>(4)?, "location": row.get::<_,String>(5)?,
            "needed": row.get::<_,i64>(6)? != 0, "notes": row.get::<_,Option<String>>(7)?,
            "created_at": row.get::<_,String>(8)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_home_item(id: i64, name: Option<String>, category: Option<String>, quantity: Option<f64>, location: Option<String>, notes: Option<String>, needed: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let mut updates = vec!["updated_at=datetime('now')".to_string()];
    if let Some(v) = &name { updates.push(format!("name='{}'", v)); }
    if let Some(v) = &category { updates.push(format!("category='{}'", v)); }
    if let Some(v) = quantity { updates.push(format!("quantity={}", v)); }
    if let Some(v) = &location { updates.push(format!("location='{}'", v)); }
    if let Some(v) = &notes { updates.push(format!("notes='{}'", v)); }
    if let Some(v) = needed { updates.push(format!("needed={}", if v { 1 } else { 0 })); }
    conn.execute(&format!("UPDATE home_items SET {} WHERE id=?1", updates.join(",")), rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("updated".into())
}

#[tauri::command]
pub fn delete_home_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("DELETE FROM home_items WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("deleted".into())
}

#[tauri::command]
pub fn toggle_home_item_needed(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE home_items SET needed = CASE WHEN needed=1 THEN 0 ELSE 1 END, updated_at=datetime('now') WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

// ── People / Contacts ──

#[tauri::command]
pub fn add_contact(
    name: String,
    phone: Option<String>,
    email: Option<String>,
    category: Option<String>,
    relationship: Option<String>,
    notes: Option<String>,
    blocked: Option<bool>,
    block_reason: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO contacts (name, phone, email, category, relationship, notes, blocked, block_reason, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,datetime('now'),datetime('now'))",
        rusqlite::params![name, phone, email, category.unwrap_or("other".into()), relationship, notes, blocked.unwrap_or(false) as i32, block_reason],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_contacts(category: Option<String>, blocked: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut sql = "SELECT id, name, phone, email, category, relationship, notes, blocked, block_reason, favorite, created_at, updated_at FROM contacts WHERE 1=1".to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref cat) = category {
        sql.push_str(&format!(" AND category=?{}", params.len() + 1));
        params.push(Box::new(cat.clone()));
    }
    if let Some(b) = blocked {
        sql.push_str(&format!(" AND blocked=?{}", params.len() + 1));
        params.push(Box::new(b as i32));
    }
    sql.push_str(" ORDER BY favorite DESC, name ASC");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "phone": row.get::<_, Option<String>>(2)?,
            "email": row.get::<_, Option<String>>(3)?,
            "category": row.get::<_, String>(4)?,
            "relationship": row.get::<_, Option<String>>(5)?,
            "notes": row.get::<_, Option<String>>(6)?,
            "blocked": row.get::<_, i32>(7)? != 0,
            "block_reason": row.get::<_, Option<String>>(8)?,
            "favorite": row.get::<_, i32>(9)? != 0,
            "created_at": row.get::<_, String>(10)?,
            "updated_at": row.get::<_, String>(11)?,
        }))
    }).map_err(|e| e.to_string())?;
    let items: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!(items))
}

#[tauri::command]
pub fn update_contact(
    id: i64,
    name: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    category: Option<String>,
    relationship: Option<String>,
    notes: Option<String>,
    blocked: Option<bool>,
    block_reason: Option<String>,
    favorite: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    if let Some(v) = name { conn.execute("UPDATE contacts SET name=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = phone { conn.execute("UPDATE contacts SET phone=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = email { conn.execute("UPDATE contacts SET email=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = category { conn.execute("UPDATE contacts SET category=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = relationship { conn.execute("UPDATE contacts SET relationship=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = notes { conn.execute("UPDATE contacts SET notes=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = blocked { conn.execute("UPDATE contacts SET blocked=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v as i32, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = block_reason { conn.execute("UPDATE contacts SET block_reason=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = favorite { conn.execute("UPDATE contacts SET favorite=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v as i32, id]).map_err(|e| e.to_string())?; }
    Ok("updated".into())
}

#[tauri::command]
pub fn delete_contact(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("DELETE FROM contacts WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("deleted".into())
}

#[tauri::command]
pub fn toggle_contact_blocked(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE contacts SET blocked = CASE WHEN blocked=1 THEN 0 ELSE 1 END, updated_at=datetime('now') WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

#[tauri::command]
pub fn toggle_contact_favorite(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE contacts SET favorite = CASE WHEN favorite=1 THEN 0 ELSE 1 END, updated_at=datetime('now') WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

// ── Contact blocks (per-person site/app blocking) ──

#[tauri::command]
pub fn add_contact_block(
    contact_id: i64,
    block_type: Option<String>,
    value: String,
    reason: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO contact_blocks (contact_id, block_type, value, reason) VALUES (?1,?2,?3,?4)",
        rusqlite::params![contact_id, block_type.unwrap_or("site".into()), value, reason],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_contact_blocks(contact_id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, contact_id, block_type, value, reason, active, created_at FROM contact_blocks WHERE contact_id=?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![contact_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "contact_id": row.get::<_, i64>(1)?,
            "block_type": row.get::<_, String>(2)?,
            "value": row.get::<_, String>(3)?,
            "reason": row.get::<_, Option<String>>(4)?,
            "active": row.get::<_, i32>(5)? != 0,
            "created_at": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| e.to_string())?;
    let items: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!(items))
}

#[tauri::command]
pub fn delete_contact_block(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("DELETE FROM contact_blocks WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("deleted".into())
}

#[tauri::command]
pub fn toggle_contact_block_active(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE contact_blocks SET active = CASE WHEN active=1 THEN 0 ELSE 1 END WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

// ── v0.9.0: Page Meta & Custom Properties ──

#[tauri::command]
pub fn get_page_meta(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let result = conn.query_row(
        "SELECT tab_id, emoji, title, description, updated_at FROM page_meta WHERE tab_id=?1",
        rusqlite::params![tab_id],
        |row| Ok(serde_json::json!({
            "tab_id": row.get::<_, String>(0)?,
            "emoji": row.get::<_, Option<String>>(1)?,
            "title": row.get::<_, Option<String>>(2)?,
            "description": row.get::<_, Option<String>>(3)?,
            "updated_at": row.get::<_, String>(4)?,
        }))
    );
    match result {
        Ok(v) => Ok(v),
        Err(_) => Ok(serde_json::json!(null)),
    }
}

#[tauri::command]
pub fn update_page_meta(tab_id: String, emoji: Option<String>, title: Option<String>, description: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO page_meta (tab_id, emoji, title, description, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(tab_id) DO UPDATE SET
         emoji=COALESCE(?2, emoji), title=COALESCE(?3, title),
         description=COALESCE(?4, description), updated_at=?5",
        rusqlite::params![tab_id, emoji, title, description, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_property_definitions(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, tab_id, name, type, position, color, options, default_value, visible
         FROM property_definitions WHERE tab_id=?1 ORDER BY position"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![tab_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "tab_id": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "type": row.get::<_, String>(3)?,
            "position": row.get::<_, i64>(4)?,
            "color": row.get::<_, Option<String>>(5)?,
            "options": row.get::<_, Option<String>>(6)?,
            "default_value": row.get::<_, Option<String>>(7)?,
            "visible": row.get::<_, i64>(8)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_property_definition(tab_id: String, name: String, prop_type: String, position: Option<i64>, color: Option<String>, options: Option<String>, default_value: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let pos = position.unwrap_or_else(|| {
        conn.query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM property_definitions WHERE tab_id=?1",
            rusqlite::params![tab_id], |row| row.get::<_, i64>(0)).unwrap_or(0)
    });
    conn.execute(
        "INSERT INTO property_definitions (tab_id, name, type, position, color, options, default_value, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![tab_id, name, prop_type, pos, color, options, default_value, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_property_definition(id: i64, name: Option<String>, prop_type: Option<String>, position: Option<i64>, color: Option<String>, options: Option<String>, visible: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(n) = name { conn.execute("UPDATE property_definitions SET name=?1 WHERE id=?2", rusqlite::params![n, id]).map_err(|e| e.to_string())?; }
    if let Some(t) = prop_type { conn.execute("UPDATE property_definitions SET type=?1 WHERE id=?2", rusqlite::params![t, id]).map_err(|e| e.to_string())?; }
    if let Some(p) = position { conn.execute("UPDATE property_definitions SET position=?1 WHERE id=?2", rusqlite::params![p, id]).map_err(|e| e.to_string())?; }
    if let Some(c) = color { conn.execute("UPDATE property_definitions SET color=?1 WHERE id=?2", rusqlite::params![c, id]).map_err(|e| e.to_string())?; }
    if let Some(o) = options { conn.execute("UPDATE property_definitions SET options=?1 WHERE id=?2", rusqlite::params![o, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = visible { conn.execute("UPDATE property_definitions SET visible=?1 WHERE id=?2", rusqlite::params![v as i32, id]).map_err(|e| e.to_string())?; }
    Ok(())
}

#[tauri::command]
pub fn delete_property_definition(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM property_values WHERE property_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM property_definitions WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_property_values(record_table: String, record_ids: Vec<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if record_ids.is_empty() { return Ok(vec![]); }
    let placeholders: Vec<String> = record_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
    let sql = format!(
        "SELECT pv.id, pv.record_id, pv.record_table, pv.property_id, pv.value, pd.name, pd.type
         FROM property_values pv JOIN property_definitions pd ON pd.id = pv.property_id
         WHERE pv.record_table=?1 AND pv.record_id IN ({})",
        placeholders.join(",")
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params.push(Box::new(record_table));
    for id in &record_ids { params.push(Box::new(*id)); }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "record_id": row.get::<_, i64>(1)?,
            "record_table": row.get::<_, String>(2)?,
            "property_id": row.get::<_, i64>(3)?,
            "value": row.get::<_, Option<String>>(4)?,
            "prop_name": row.get::<_, String>(5)?,
            "prop_type": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn set_property_value(record_id: i64, record_table: String, property_id: i64, value: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO property_values (record_id, record_table, property_id, value)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(record_id, record_table, property_id) DO UPDATE SET value=?4",
        rusqlite::params![record_id, record_table, property_id, value],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_property_value(record_id: i64, record_table: String, property_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM property_values WHERE record_id=?1 AND record_table=?2 AND property_id=?3",
        rusqlite::params![record_id, record_table, property_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_view_configs(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, tab_id, name, view_type, filter_json, sort_json, visible_columns, is_default, position
         FROM view_configs WHERE tab_id=?1 ORDER BY position"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![tab_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "tab_id": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "view_type": row.get::<_, String>(3)?,
            "filter_json": row.get::<_, Option<String>>(4)?,
            "sort_json": row.get::<_, Option<String>>(5)?,
            "visible_columns": row.get::<_, Option<String>>(6)?,
            "is_default": row.get::<_, i64>(7)? != 0,
            "position": row.get::<_, Option<i64>>(8)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_view_config(tab_id: String, name: String, view_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let vt = view_type.unwrap_or_else(|| "table".into());
    conn.execute(
        "INSERT INTO view_configs (tab_id, name, view_type, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![tab_id, name, vt, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_view_config(id: i64, filter_json: Option<String>, sort_json: Option<String>, visible_columns: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(f) = filter_json { conn.execute("UPDATE view_configs SET filter_json=?1 WHERE id=?2", rusqlite::params![f, id]).map_err(|e| e.to_string())?; }
    if let Some(s) = sort_json { conn.execute("UPDATE view_configs SET sort_json=?1 WHERE id=?2", rusqlite::params![s, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = visible_columns { conn.execute("UPDATE view_configs SET visible_columns=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    Ok(())
}

// ── UI State (persistent key-value, replaces localStorage) ──

#[tauri::command]
pub fn get_ui_state(key: String, db: tauri::State<'_, HanniDb>) -> Result<Option<String>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT value FROM ui_state WHERE key=?1").map_err(|e| e.to_string())?;
    let val = stmt.query_row(rusqlite::params![key], |r| r.get::<_, String>(0)).ok();
    Ok(val)
}

#[tauri::command]
pub fn set_ui_state(key: String, value: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("INSERT INTO ui_state (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", rusqlite::params![key, value]).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Integrations, Model Info, Health Check ──
// ── Integrations info ──

#[tauri::command]
pub async fn get_integrations() -> Result<IntegrationsInfo, String> {
    // ── Access ──
    let tracker_path = data_file_path();
    let tracker_exists = tracker_path.exists();
    let access = vec![
        IntegrationItem {
            name: "Life Tracker".into(),
            status: if tracker_exists { "active" } else { "inactive" }.into(),
            detail: if tracker_exists {
                "~/Library/Application Support/Hanni/life-tracker-data.json".into()
            } else {
                "Файл не найден".into()
            },
        },
        IntegrationItem {
            name: "File System".into(),
            status: "active".into(),
            detail: "$HOME/** — чтение файлов".into(),
        },
        IntegrationItem {
            name: "Shell".into(),
            status: "active".into(),
            detail: "Выполнение команд".into(),
        },
    ];

    // ── Tracking ──
    let tracking = if tracker_exists {
        let data = load_tracker_data().unwrap_or(TrackerData {
            purchases: vec![], time_entries: vec![], goals: vec![], notes: vec![],
            settings: serde_json::Value::Null,
        });
        vec![
            IntegrationItem {
                name: "Расходы".into(),
                status: "active".into(),
                detail: format!("{} записей", data.purchases.len()),
            },
            IntegrationItem {
                name: "Время".into(),
                status: "active".into(),
                detail: format!("{} записей", data.time_entries.len()),
            },
            IntegrationItem {
                name: "Цели".into(),
                status: "active".into(),
                detail: format!("{} целей", data.goals.len()),
            },
            IntegrationItem {
                name: "Заметки".into(),
                status: "active".into(),
                detail: format!("{} заметок", data.notes.len()),
            },
        ]
    } else {
        vec![IntegrationItem {
            name: "Life Tracker".into(),
            status: "inactive".into(),
            detail: "Не подключен".into(),
        }]
    };

    // ── Blocker config ──
    let blocker_config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    let default_apps = vec!["Telegram", "Discord", "Slack", "Safari"];
    let default_sites = vec![
        "youtube.com", "twitter.com", "x.com", "instagram.com",
        "facebook.com", "tiktok.com", "reddit.com", "vk.com", "netflix.com",
    ];

    let (apps, sites) = if blocker_config_path.exists() {
        let content = std::fs::read_to_string(&blocker_config_path).unwrap_or_default();
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
            let apps: Vec<String> = cfg["apps"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_apps.iter().map(|s| s.to_string()).collect());
            let sites: Vec<String> = cfg["sites"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_sites.iter().map(|s| s.to_string()).collect());
            (apps, sites)
        } else {
            (default_apps.iter().map(|s| s.to_string()).collect(),
             default_sites.iter().map(|s| s.to_string()).collect())
        }
    } else {
        (default_apps.iter().map(|s| s.to_string()).collect(),
         default_sites.iter().map(|s| s.to_string()).collect())
    };

    // Check if blocking is active via /etc/hosts
    let blocker_active = std::fs::read_to_string("/etc/hosts")
        .map(|c| c.contains("# === HANNI FOCUS BLOCKER ==="))
        .unwrap_or(false);

    let blocked_apps = apps.iter().map(|a| IntegrationItem {
        name: a.clone(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: format!("/Applications/{}.app", a),
    }).collect();

    // Deduplicate sites (remove www. variants for display)
    let unique_sites: Vec<&String> = sites.iter()
        .filter(|s| !s.starts_with("www."))
        .collect();

    let blocked_sites = unique_sites.iter().map(|s| IntegrationItem {
        name: s.to_string(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: if blocker_active { "Заблокирован" } else { "Не заблокирован" }.into(),
    }).collect();

    // ── macOS integrations ──
    let macos = vec![
        IntegrationItem {
            name: "Screen Time".into(),
            status: "ready".into(),
            detail: "knowledgeC.db · по запросу".into(),
        },
        IntegrationItem {
            name: "Календарь".into(),
            status: "ready".into(),
            detail: "Calendar.app · по запросу".into(),
        },
        IntegrationItem {
            name: "Музыка".into(),
            status: "ready".into(),
            detail: "Music / Spotify · по запросу".into(),
        },
        IntegrationItem {
            name: "Браузер".into(),
            status: "ready".into(),
            detail: "Safari / Chrome / Arc · по запросу".into(),
        },
    ];

    Ok(IntegrationsInfo {
        access,
        tracking,
        blocked_apps,
        blocked_sites,
        blocker_active,
        macos,
    })
}

// ── Model info ──

#[tauri::command]
pub async fn get_model_info() -> Result<ModelInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    let online = client
        .get("http://127.0.0.1:8234/v1/models")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    Ok(ModelInfo {
        model_name: MODEL.to_string(),
        server_url: MLX_URL.to_string(),
        server_online: online,
    })
}

// ── Health Check (C4) ──

#[tauri::command]
pub async fn health_check(app: AppHandle) -> Result<HealthStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    // MLX server check
    let mlx_online = client
        .get("http://127.0.0.1:8234/v1/models")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    // Voice server check
    let voice_server_online = client
        .get(format!("{}/health", VOICE_SERVER_URL))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    // DB checks
    let (db_ok, db_tables, db_facts, db_conversations, db_size_mb) = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();

        let tables: usize = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [], |row| row.get(0),
        ).unwrap_or(0);

        let facts: usize = conn.query_row(
            "SELECT count(*) FROM facts", [], |row| row.get(0),
        ).unwrap_or(0);

        let convs: usize = conn.query_row(
            "SELECT count(*) FROM conversations", [], |row| row.get(0),
        ).unwrap_or(0);

        // DB file size
        let size: f64 = conn.query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [], |row| row.get::<_, i64>(0),
        ).map(|bytes| bytes as f64 / 1_048_576.0).unwrap_or(0.0);

        let integrity: String = conn.query_row(
            "PRAGMA integrity_check", [], |row| row.get(0),
        ).unwrap_or_else(|_| "error".into());

        (integrity == "ok", tables, facts, convs, size)
    };

    Ok(HealthStatus {
        mlx_online,
        mlx_model: MODEL.to_string(),
        voice_server_online,
        db_ok,
        db_tables,
        db_facts,
        db_conversations,
        db_size_mb,
    })
}

// ── HTTP API Server ──
// ── Phase 4: HTTP API ──

pub fn api_token_path() -> PathBuf {
    hanni_data_dir().join("api_token.txt")
}

pub fn get_or_create_api_token() -> String {
    let path = api_token_path();
    if path.exists() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &token);
    token
}

pub async fn spawn_api_server(app_handle: AppHandle) {
    use axum::{Router, routing::{get, post}, extract::{State as AxumState, Query}, Json, http::{StatusCode, HeaderMap}};

    let api_token = get_or_create_api_token();

    #[derive(Clone)]
    struct ApiState {
        app: AppHandle,
        token: String,
    }

    let state = ApiState {
        app: app_handle,
        token: api_token,
    };

    pub fn check_auth(headers: &HeaderMap, token: &str) -> Result<(), (StatusCode, String)> {
        let auth = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let provided = auth.strip_prefix("Bearer ").unwrap_or(auth);
        if provided == token {
            Ok(())
        } else {
            Err((StatusCode::UNAUTHORIZED, "Invalid token".into()))
        }
    }

    #[derive(Deserialize)]
    struct ChatReq {
        message: String,
        history: Option<Vec<serde_json::Value>>,
    }

    #[derive(Deserialize)]
    struct SearchQuery {
        q: String,
        limit: Option<usize>,
    }

    #[derive(Deserialize)]
    struct RememberReq {
        category: String,
        key: String,
        value: String,
    }

    pub async fn api_status(
        AxumState(state): AxumState<ApiState>,
    ) -> Json<serde_json::Value> {
        // No auth required for status — allows frontend health check
        let busy = state.app.state::<LlmBusy>().0.available_permits() == 0;
        let focus_active = state.app.state::<FocusManager>().0.lock().unwrap_or_else(|e| e.into_inner()).active;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let model_online = client
            .get("http://127.0.0.1:8234/v1/models")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        Json(serde_json::json!({
            "status": "ok",
            "model_online": model_online,
            "llm_busy": busy,
            "focus_active": focus_active,
        }))
    }

    pub async fn api_chat(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<ChatReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let mut messages = req.history.unwrap_or_default();
        messages.push(serde_json::json!({"role": "user", "content": req.message}));

        match chat_inner(&state.app, messages, false).await {
            Ok(result) => Ok(Json(serde_json::json!({ "reply": result.text, "tool_calls": result.tool_calls }))),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
        }
    }

    pub async fn api_memory_search(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Query(params): Query<SearchQuery>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let max = params.limit.unwrap_or(20) as i64;

        let words: Vec<&str> = params.q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
        let mut results = Vec::new();

        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        if results.is_empty() {
            let like_pattern = format!("%{}%", params.q);
            if let Ok(mut stmt) = conn.prepare(
                "SELECT category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        Ok(Json(serde_json::json!({ "results": results })))
    }

    pub async fn api_memory_add(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<RememberReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO facts (category, key, value, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'api', ?4, ?4)
             ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
            rusqlite::params![req.category, req.key, req.value, now],
        ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

        Ok(Json(serde_json::json!({ "status": "ok" })))
    }

    // ── Automation endpoints (eval JS in WebView, works even minimized) ──

    #[derive(Deserialize)]
    struct EvalReq {
        script: String,
    }

    pub async fn auto_eval(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<EvalReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let cb_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();

        // Register callback in global map
        state.app.state::<AutoEvalCallbacks>()
            .0.lock().unwrap()
            .insert(cb_id.clone(), tx);

        // Wrap script to invoke Tauri command with result
        let wrapped = format!(
            r#"(async () => {{
                try {{
                    const __r = await (async () => {{ {script} }})();
                    await window.__TAURI__.core.invoke('auto_eval_callback', {{ cbId: '{cb_id}', result: JSON.stringify(__r ?? null) }});
                }} catch(e) {{
                    await window.__TAURI__.core.invoke('auto_eval_callback', {{ cbId: '{cb_id}', result: JSON.stringify({{ __error: e.message }}) }});
                }}
            }})()"#,
            script = req.script, cb_id = cb_id
        );

        if let Some(win) = state.app.get_webview_window("main") {
            win.eval(&wrapped).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("eval error: {}", e)))?;
        } else {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "No main webview found".into()));
        }

        match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
            Ok(Ok(result)) => {
                let inner: serde_json::Value = serde_json::from_str(&result)
                    .unwrap_or(serde_json::Value::String(result));
                Ok(Json(serde_json::json!({ "result": inner })))
            }
            Ok(Err(_)) => Err((StatusCode::INTERNAL_SERVER_ERROR, "Channel closed".into())),
            Err(_) => Err((StatusCode::REQUEST_TIMEOUT, "Script timed out after 10s".into())),
        }
    }

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/chat", post(api_chat))
        .route("/api/memory/search", get(api_memory_search))
        .route("/api/memory", post(api_memory_add))
        .route("/auto/eval", post(auto_eval))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8235").await;
    match listener {
        Ok(listener) => {
            let _ = axum::serve(listener, app).await;
        }
        Err(e) => {
            eprintln!("Failed to start API server: {}", e);
        }
    }
}

pub fn find_python() -> Option<String> {
    // Try common locations for python3 with mlx_lm
    let candidates = [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/usr/bin/python3",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

pub fn start_mlx_server() -> Option<Child> {
    let python = match find_python() {
        Some(p) => p,
        None => {
            eprintln!("[mlx] No python3 found — cannot start MLX server");
            return None;
        }
    };

    // Check if server is already running
    let check = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    if check.get("http://127.0.0.1:8234/v1/models").send().map(|r| r.status().is_success()).unwrap_or(false) {
        eprintln!("[mlx] Server already running on port 8234");
        return None;
    }

    let args = vec![
        "-m", "mlx_lm", "server",
        "--model", MODEL,
        "--port", "8234",
        "--chat-template-args", r#"{"enable_thinking":false}"#,
    ];
    eprintln!("[mlx] Starting MLX server: {} {:?}", python, args);

    // Log MLX stderr to file for debugging
    let log_path = hanni_data_dir().join("mlx_server.log");
    let stderr_file = std::fs::File::create(&log_path)
        .map(std::process::Stdio::from)
        .unwrap_or_else(|_| std::process::Stdio::null());
    let child = Command::new(&python)
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(stderr_file)
        .spawn();

    match child {
        Ok(child) => {
            eprintln!("[mlx] Server process spawned (pid {})", child.id());
            Some(child)
        }
        Err(e) => {
            eprintln!("[mlx] Failed to spawn server: {}", e);
            None
        }
    }
}

/// Ensure OpenClaw gateway is running. Checks health first; if down, starts as subprocess.
pub fn ensure_openclaw_gateway() -> Option<Child> {
    // Check if gateway is already running (LaunchAgent or manual)
    let check = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    if check.get("http://127.0.0.1:18789/")
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
    {
        eprintln!("[openclaw] Gateway already running on port 18789");
        return None;
    }

    // Try to find openclaw binary
    let openclaw_path = which_binary("openclaw");
    let openclaw = match openclaw_path {
        Some(p) => p,
        None => {
            eprintln!("[openclaw] openclaw not found in PATH — gateway will not start");
            return None;
        }
    };

    eprintln!("[openclaw] Starting gateway as subprocess: {}", openclaw);
    let log_path = hanni_data_dir().join("openclaw_gateway.log");
    let stderr_file = std::fs::File::create(&log_path)
        .map(std::process::Stdio::from)
        .unwrap_or_else(|_| std::process::Stdio::null());
    let child = Command::new(&openclaw)
        .args(["gateway", "--port", "18789"])
        .stdout(std::process::Stdio::null())
        .stderr(stderr_file)
        .spawn();

    match child {
        Ok(child) => {
            eprintln!("[openclaw] Gateway spawned (pid {})", child.id());
            Some(child)
        }
        Err(e) => {
            eprintln!("[openclaw] Failed to spawn gateway: {}", e);
            None
        }
    }
}

fn which_binary(name: &str) -> Option<String> {
    // Check common paths + PATH
    let common = [
        format!("/opt/homebrew/bin/{}", name),
        format!("/usr/local/bin/{}", name),
    ];
    for path in &common {
        if std::path::Path::new(path).exists() {
            return Some(path.clone());
        }
    }
    // Fallback: use `which` command
    std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else { None }
        })
}

const VOICE_SERVER_URL: &str = "http://127.0.0.1:8237";

pub fn escape_plist_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn ensure_voice_server_launchagent() {
    let python = match find_python() {
        Some(p) => p,
        None => { eprintln!("[voice] No python3 found"); return; }
    };

    // Extract embedded voice_server.py to data dir (always overwrite to keep in sync with binary)
    let script = hanni_data_dir().join("voice_server.py");
    let embedded = include_str!("../../voice_server.py");
    if let Err(e) = std::fs::write(&script, embedded) {
        eprintln!("[voice] Failed to write voice_server.py: {}", e);
        return;
    }

    let log_path = hanni_data_dir().join("voice_server.log");
    let plist_path = match dirs::home_dir() {
        Some(h) => h.join("Library/LaunchAgents/com.hanni.voice-server.plist"),
        None => { eprintln!("[voice] Cannot determine home dir"); return; }
    };
    // XML-escape all interpolated paths to prevent plist injection
    let python_esc = escape_plist_xml(&python);
    let script_esc = escape_plist_xml(&script.to_string_lossy());
    let log_esc = escape_plist_xml(&log_path.to_string_lossy());

    let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>com.hanni.voice-server</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>{}</string>
	</array>
	<key>KeepAlive</key>
	<true/>
	<key>RunAtLoad</key>
	<true/>
	<key>StandardErrorPath</key>
	<string>{}</string>
	<key>StandardOutPath</key>
	<string>{}</string>
</dict>
</plist>"#, python_esc, script_esc, log_esc, log_esc);

    // Check if plist already exists with same content
    let needs_update = match std::fs::read_to_string(&plist_path) {
        Ok(existing) => existing != plist_content,
        Err(_) => true,
    };

    if needs_update {
        // Unload old version if exists
        let _ = Command::new("launchctl").args(["unload", &plist_path.to_string_lossy()]).output();
        if let Err(e) = std::fs::write(&plist_path, &plist_content) {
            eprintln!("[voice] Failed to write LaunchAgent: {}", e);
            return;
        }
        let _ = Command::new("launchctl").args(["load", &plist_path.to_string_lossy()]).output();
        eprintln!("[voice] LaunchAgent installed and loaded");
    } else {
        // Just make sure it's running
        let check = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build();
        let running = check.ok()
            .and_then(|c| c.get(&format!("{}/health", VOICE_SERVER_URL)).send().ok())
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if !running {
            let _ = Command::new("launchctl").args(["unload", &plist_path.to_string_lossy()]).output();
            let _ = Command::new("launchctl").args(["load", &plist_path.to_string_lossy()]).output();
            eprintln!("[voice] LaunchAgent reloaded");
        } else {
            eprintln!("[voice] LaunchAgent already running");
        }
    }
}


// ── Updater ──
// ── Updater ──

pub fn updater_with_headers(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    // Public repo — no auth headers needed. Direct download URLs work without them.
    app.updater_builder()
        .build()
        .map_err(|e| format!("Updater error: {}", e))
}

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<String, String> {
    let updater = updater_with_headers(&app)?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let _ = app.emit("update-available", &version);
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| format!("Install error: {}", e))?;
            app.restart();
        }
        Ok(None) => Ok("Вы на последней версии.".into()),
        Err(e) => Err(format!("Не удалось проверить обновления: {}", e)),
    }
}

