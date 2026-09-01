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
    let blocker_config_path = hanni_data_dir().join("blocker_config.json");

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
    let config_path = hanni_data_dir().join("blocker_config.json");

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

// ── Blocklist, Goals, Settings, Home, Contacts, Properties, Views ──

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
    crate::secret_store::set_setting(&conn, &key, &value)?;
    // Sync calendar toggle to static flag
    if key == "apple_calendar_enabled" {
        APPLE_CALENDAR_DISABLED.store(value == "false", Ordering::Relaxed);
    }
    // LLM endpoint/model overrides take effect without restart
    if key == "llm_server_url" { set_llm_base_url(&value); }
    if key == "llm_model" { set_llm_model(&value); }
    Ok(())
}

#[tauri::command]
pub fn get_app_setting(key: String, db: tauri::State<'_, HanniDb>) -> Result<Option<String>, String> {
    if crate::secret_store::is_sensitive_setting(&key) {
        return Err("sensitive settings cannot be read through the generic settings API".into());
    }
    let conn = db.read();
    crate::secret_store::get_setting(&conn, &key)
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
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(c) = category { conditions.push("category=?1".to_string()); params.push(Box::new(c)); }
    if needed_only { conditions.push("needed=1".to_string()); }
    if !conditions.is_empty() { sql += &format!(" WHERE {}", conditions.join(" AND ")); }
    sql += " ORDER BY needed DESC, name ASC";
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows: Vec<serde_json::Value> = stmt.query_map(param_refs.as_slice(), |row| {
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
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = quantity { updates.push(format!("quantity=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = location { updates.push(format!("location=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = notes { updates.push(format!("notes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = needed { updates.push(format!("needed=?{}", idx)); params.push(Box::new(if v { 1 } else { 0 })); idx += 1; }
    params.push(Box::new(id));
    let sql = format!("UPDATE home_items SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| e.to_string())?;
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
    let conn = db.read();
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
    let blocker_config_path = hanni_data_dir().join("blocker_config.json");

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
        .get(llm_models_url())
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    Ok(ModelInfo {
        model_name: llm_model(),
        server_url: llm_chat_url(),
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
        .get(llm_models_url())
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
        mlx_model: llm_model(),
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

pub fn jobs_api_token_path() -> PathBuf {
    hanni_data_dir().join("jobs_api_token.txt")
}

/// Replace the API token file with a fresh UUID. The running server keeps
/// the old token in memory, so a process restart is required for the new
/// one to take effect. Returns the new token so the UI can show it once.
#[tauri::command]
pub fn rotate_api_token() -> Result<String, String> {
    let path = api_token_path();
    let token = uuid::Uuid::new_v4().to_string();
    crate::secret_store::write_file(&path, &token)?;
    Ok(token)
}

#[tauri::command]
pub fn rotate_jobs_api_token() -> Result<String, String> {
    let path = jobs_api_token_path();
    let token = uuid::Uuid::new_v4().to_string();
    crate::secret_store::write_file(&path, &token)?;
    Ok(token)
}

/// Returns the current API token (first 8 chars + ellipsis) for display
/// in Settings. Outside the explicit rotate-and-copy action, the full token
/// never crosses the backend/UI boundary.
#[tauri::command]
pub fn get_api_token_preview() -> Result<String, String> {
    let token = read_token_file(&api_token_path())?;
    if token.len() < 8 { return Ok(token.to_string()); }
    Ok(format!("{}…", &token[..8]))
}

#[tauri::command]
pub fn get_jobs_api_token_preview() -> Result<String, String> {
    let token = read_token_file(&jobs_api_token_path())?;
    if token.len() < 8 { return Ok(token); }
    Ok(format!("{}…", &token[..8]))
}

#[derive(serde::Serialize)]
pub struct AutomationLogRow {
    pub id: i64,
    pub ts: i64,
    pub script_hash: String,
    pub success: bool,
    pub duration_ms: i64,
}

#[tauri::command]
pub fn list_automation_log(limit: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<AutomationLogRow>, String> {
    let conn = db.conn();
    let lim = limit.unwrap_or(100).clamp(1, 1000);
    let mut stmt = conn.prepare(
        "SELECT id, ts, script_hash, success, duration_ms
         FROM automation_log ORDER BY ts DESC LIMIT ?1"
    ).map_err(|e| format!("prepare: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![lim], |r| {
        Ok(AutomationLogRow {
            id: r.get(0)?,
            ts: r.get(1)?,
            script_hash: r.get(2)?,
            success: r.get::<_, i64>(3)? != 0,
            duration_ms: r.get(4)?,
        })
    }).map_err(|e| format!("query: {}", e))?;
    let out: Vec<_> = rows.flatten().collect();
    Ok(out)
}

fn get_or_create_token(path: PathBuf) -> Result<String, String> {
    match read_token_file(&path) {
        Ok(token) => return Ok(token),
        Err(_) if !path.exists() => {}
        Err(e) => return Err(e),
    }
    let token = uuid::Uuid::new_v4().to_string();
    match crate::secret_store::create_file(&path, &token) {
        Ok(()) => Ok(token),
        Err(_) if path.exists() => {
            crate::secure_fs::restrict_file(&path)
                .map_err(|error| format!("secure {}: {error}", path.display()))?;
            read_token_file(&path)
        }
        Err(e) => Err(format!("write {}: {e}", path.display())),
    }
}

fn read_token_file(path: &std::path::Path) -> Result<String, String> {
    let token = crate::secret_store::read_file(path)?;
    let token = token.trim();
    if token.is_empty() {
        return Err(format!("token file is empty: {}", path.display()));
    }
    let parsed = uuid::Uuid::parse_str(token)
        .map_err(|_| format!("token file is not a canonical UUID: {}", path.display()))?;
    let canonical = parsed.hyphenated().to_string();
    if token != canonical {
        return Err(format!("token file is not canonical: {}", path.display()));
    }
    Ok(canonical)
}

pub fn get_or_create_api_token() -> Result<String, String> {
    get_or_create_token(api_token_path())
}

pub fn get_or_create_jobs_api_token() -> Result<String, String> {
    get_or_create_token(jobs_api_token_path())
}

#[cfg(debug_assertions)]
fn dev_reload_token_from_env() -> Result<Option<String>, String> {
    let raw = match std::env::var("HANNI_DEV_RELOAD_TOKEN") {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err("HANNI_DEV_RELOAD_TOKEN is not valid Unicode".into());
        }
    };
    let token = raw.trim();
    let parsed = uuid::Uuid::parse_str(token)
        .map_err(|_| "HANNI_DEV_RELOAD_TOKEN must be a canonical UUID".to_string())?;
    let canonical = parsed.hyphenated().to_string();
    if token != canonical {
        return Err("HANNI_DEV_RELOAD_TOKEN must be a canonical UUID".into());
    }
    Ok(Some(canonical))
}

#[cfg(debug_assertions)]
fn validate_dev_reload_token(
    token: Option<String>,
    api_token: &str,
    jobs_token: &str,
) -> Result<Option<String>, String> {
    if token
        .as_deref()
        .is_some_and(|value| value == api_token || value == jobs_token)
    {
        return Err(
            "HANNI_DEV_RELOAD_TOKEN must differ from the API and Jobs credentials".into(),
        );
    }
    Ok(token)
}

/// Shared state of the local HTTP API (:8236 dev / :8235 prod).
/// Module-level so route handlers can live in other modules (api_jobs.rs).
#[derive(Clone)]
pub struct ApiState {
    pub app: AppHandle,
    pub token: String,
    /// Least-privilege credential accepted only by the Jobs vacancy routes.
    /// It must never authenticate the general local API.
    pub jobs_token: String,
    /// Debug-only reload credential. It is never persisted or accepted by a
    /// release build and is separate from both production API tokens.
    #[cfg(debug_assertions)]
    dev_reload_token: Option<String>,
}

pub fn check_auth(headers: &axum::http::HeaderMap, token: &str) -> Result<(), (axum::http::StatusCode, String)> {
    use subtle::ConstantTimeEq;
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let provided = auth.strip_prefix("Bearer ").unwrap_or(auth);
    // Constant-time compare so byte-by-byte timing can't be used to
    // brute-force the token. Length mismatch short-circuits because
    // ct_eq panics on unequal slices — we want a stable false instead.
    let ok = provided.len() == token.len()
        && bool::from(provided.as_bytes().ct_eq(token.as_bytes()));
    if ok {
        Ok(())
    } else {
        Err((axum::http::StatusCode::UNAUTHORIZED, "Invalid token".into()))
    }
}

pub async fn spawn_api_server(app_handle: AppHandle) -> Result<(), String> {
    use axum::{Router, routing::{get, post}, extract::{State as AxumState, Query, DefaultBodyLimit}, Json, http::{StatusCode, HeaderMap}};

    let api_token = get_or_create_api_token()?;
    let jobs_api_token = get_or_create_jobs_api_token()?;
    #[cfg(debug_assertions)]
    let dev_reload_token = validate_dev_reload_token(
        dev_reload_token_from_env()?,
        &api_token,
        &jobs_api_token,
    )?;

    const JOBS_API_BODY_LIMIT: usize = 16 * 1024;
    // Retention: automation_log rows older than this are pruned lazily.
    const AUTO_LOG_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;

    let state = ApiState {
        app: app_handle.clone(),
        token: api_token,
        jobs_token: jobs_api_token,
        #[cfg(debug_assertions)]
        dev_reload_token,
    };

    // Background retention: prune automation_log once an hour. Kept lazy
    // (no separate scheduler crate) — a single task per server lifetime.
    {
        let app = app_handle.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                tick.tick().await;
                let cutoff = chrono::Utc::now().timestamp() - AUTO_LOG_RETENTION_SECS;
                let db = app.state::<HanniDb>();
                let _ = db.conn().execute(
                    "DELETE FROM automation_log WHERE ts < ?1",
                    rusqlite::params![cutoff],
                );
            }
        });
    }

    #[cfg(debug_assertions)]
    fn log_automation(app: &AppHandle, action: &str, success: bool, duration_ms: i64) {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(action.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let ts = chrono::Utc::now().timestamp();
        let db = app.state::<HanniDb>();
        let _ = db.conn().execute(
            "INSERT INTO automation_log (ts, script_hash, success, duration_ms)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![ts, hash, success as i64, duration_ms],
        );
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
            .get(llm_models_url())
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

    /// Fixed, debug-only reload action. No request data reaches the WebView,
    /// and the credential is separate from every production API token.
    #[cfg(debug_assertions)]
    async fn auto_reload(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
    ) -> Result<StatusCode, (StatusCode, String)> {
        let token = state.dev_reload_token.as_deref().ok_or((
            StatusCode::NOT_FOUND,
            "Debug reload endpoint is disabled".into(),
        ))?;
        check_auth(&headers, token)?;
        let started = std::time::Instant::now();
        let window = state.app.get_webview_window("main").ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "No main webview found".into(),
        ))?;
        match window.reload() {
            Ok(()) => {
                log_automation(
                    &state.app,
                    "debug_webview_reload",
                    true,
                    started.elapsed().as_millis() as i64,
                );
                Ok(StatusCode::NO_CONTENT)
            }
            Err(error) => {
                log_automation(
                    &state.app,
                    "debug_webview_reload",
                    false,
                    started.elapsed().as_millis() as i64,
                );
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("reload error: {error}"),
                ))
            }
        }
    }

    #[derive(Deserialize)]
    struct OauthCallback {
        code:  Option<String>,
        state: Option<String>,
        error: Option<String>,
    }

    pub async fn google_oauth_callback(
        AxumState(state): AxumState<ApiState>,
        Query(q): Query<OauthCallback>,
    ) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], String) {
        // No auth header — Google's redirect can't carry our Bearer token.
        // We rely on the random `state` param (CSRF-protection) inside the handler.
        let html_ok = "<html><body style='font-family:-apple-system,sans-serif;padding:40px;text-align:center'>\
            <h2>✓ Signed in to Hanni</h2>\
            <p>You can close this tab and return to the app.</p></body></html>";
        let ct = (axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8");

        // Escape reflected values — `error` comes straight from the redirect
        // query string (attacker-controllable), so raw interpolation is XSS.
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        if let Some(err) = q.error {
            return (StatusCode::BAD_REQUEST, [ct],
                format!("<h2>OAuth error</h2><pre>{}</pre>", esc(&err)));
        }
        let (code, st) = match (q.code, q.state) {
            (Some(c), Some(s)) => (c, s),
            _ => return (StatusCode::BAD_REQUEST, [ct],
                "<h2>Missing code or state</h2>".into()),
        };
        let db = state.app.state::<HanniDb>();
        match crate::google_auth::handle_oauth_callback(&db, &state.app, &code, &st).await {
            Ok(_) => (StatusCode::OK, [ct], html_ok.into()),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, [ct],
                format!("<h2>Sign-in failed</h2><pre>{}</pre>", esc(&e))),
        }
    }

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/chat", post(api_chat))
        .route("/api/memory/search", get(api_memory_search))
        .route("/api/memory", post(api_memory_add))
        .route("/api/vacancy", get(crate::api_jobs::api_vacancy_lookup).post(crate::api_jobs::api_vacancy_save).layer(DefaultBodyLimit::max(JOBS_API_BODY_LIMIT)))
        .route("/oauth/google/callback", get(google_oauth_callback));
    #[cfg(debug_assertions)]
    let app = if state.dev_reload_token.is_some() {
        app.route("/auto/reload", post(auto_reload))
    } else {
        app
    };
    let app = app.with_state(state);

    let port = if cfg!(debug_assertions) { 8236 } else { 8235 };
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.map_err(|e| format!("API server bind: {e}") )?;
    axum::serve(listener, app).await.map_err(|e| format!("API server: {e}"))
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

pub fn updater_with_headers(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    // Public repo — no auth headers needed. Direct download URLs work without them.
    app.updater_builder()
        .build()
        .map_err(|e| format!("Updater error: {}", e))
}

fn updater_log(msg: &str) {
    use std::io::Write;
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{}] {}\n", ts, msg);
    let path = hanni_data_dir().join("updater.log");
    let Some(parent) = path.parent() else {
        eprintln!("[updater] log path has no parent");
        return;
    };
    if let Err(error) = crate::secure_fs::ensure_private_dir(parent)
        .and_then(|_| crate::secure_fs::restrict_file_if_present(&path))
    {
        eprintln!("[updater] secure log path failed: {error}");
        return;
    }
    let mut f = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!("[updater] open log failed: {error}");
            return;
        }
    };
    if let Err(error) = crate::secure_fs::restrict_file(&path) {
        eprintln!("[updater] secure log file failed: {error}");
        return;
    }
    if let Err(error) = f.write_all(line.as_bytes()) {
        eprintln!("[updater] write log failed: {error}");
    }
}

async fn run_update(
    app: &AppHandle,
    update: tauri_plugin_updater::Update,
) -> Result<String, String> {
    use std::sync::{Arc, Mutex};

    let version = update.version.clone();
    let _ = app.emit("update-available", &version);
    updater_log(&format!("available: v{}", version));

    let downloaded = Arc::new(Mutex::new(0u64));
    let total = Arc::new(Mutex::new(0u64));
    let last_percent = Arc::new(Mutex::new(-1i64));

    let app_chunk = app.clone();
    let dl_r = downloaded.clone();
    let tot_r = total.clone();
    let pct_r = last_percent.clone();

    let app_finish = app.clone();

    let res = update
        .download_and_install(
            move |chunk_len, content_len| {
                let mut d = dl_r.lock().unwrap();
                *d += chunk_len as u64;
                if let Some(t) = content_len {
                    *tot_r.lock().unwrap() = t;
                }
                let t = *tot_r.lock().unwrap();
                let pct = if t > 0 { ((*d * 100) / t) as i64 } else { 0 };
                let mut lp = pct_r.lock().unwrap();
                if pct != *lp {
                    *lp = pct;
                    let _ = app_chunk.emit(
                        "update-progress",
                        serde_json::json!({
                            "downloaded": *d,
                            "total": t,
                            "percent": pct,
                        }),
                    );
                }
            },
            move || {
                updater_log("download finished, installing");
                let _ = app_finish.emit("update-installing", ());
            },
        )
        .await;

    match res {
        Ok(()) => {
            updater_log(&format!("install ok: v{}", version));
            let _ = app.emit("update-ready", &version);
            Ok(format!("Готово — перезапусти Hanni для v{}.", version))
        }
        Err(e) => {
            let msg = format!("Ошибка установки: {}", e);
            updater_log(&msg);
            let _ = app.emit("update-error", &msg);
            Err(msg)
        }
    }
}

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn is_debug_build() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<String, String> {
    let updater = updater_with_headers(&app)?;
    updater_log("check_update: started");
    match updater.check().await {
        Ok(Some(update)) => run_update(&app, update).await,
        Ok(None) => {
            updater_log("check_update: up to date");
            Ok("Вы на последней версии.".into())
        }
        Err(e) => {
            let msg = format!("Не удалось проверить обновления: {}", e);
            updater_log(&msg);
            Err(msg)
        }
    }
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    updater_log("restart_app: triggered by user");
    app.restart();
}

// Called by lib.rs setup — background auto-check at startup.
#[cfg(not(target_os = "android"))]
pub async fn auto_check_on_startup(app: AppHandle) {
    let updater = match updater_with_headers(&app) {
        Ok(u) => u,
        Err(e) => {
            updater_log(&format!("auto: builder error: {}", e));
            return;
        }
    };
    updater_log("auto: startup check");
    match updater.check().await {
        Ok(Some(update)) => {
            let _ = run_update(&app, update).await;
        }
        Ok(None) => updater_log("auto: up to date"),
        Err(e) => updater_log(&format!("auto: check error: {}", e)),
    }
}

#[cfg(all(test, debug_assertions))]
mod dev_reload_security_tests {
    use super::{validate_dev_reload_token, AutomationLogRow};

    const API_TOKEN: &str = "11111111-1111-4111-8111-111111111111";
    const JOBS_TOKEN: &str = "22222222-2222-4222-8222-222222222222";
    const RELOAD_TOKEN: &str = "33333333-3333-4333-8333-333333333333";

    #[test]
    fn reload_token_must_not_reuse_production_credentials() {
        for reused in [API_TOKEN, JOBS_TOKEN] {
            let error = validate_dev_reload_token(
                Some(reused.to_string()),
                API_TOKEN,
                JOBS_TOKEN,
            )
            .expect_err("credential reuse must fail");
            assert!(error.contains("must differ"));
            assert!(!error.contains(reused));
        }
    }

    #[test]
    fn distinct_reload_token_and_disabled_route_are_valid() {
        assert_eq!(
            validate_dev_reload_token(
                Some(RELOAD_TOKEN.to_string()),
                API_TOKEN,
                JOBS_TOKEN,
            )
            .expect("distinct reload token"),
            Some(RELOAD_TOKEN.to_string())
        );
        assert_eq!(
            validate_dev_reload_token(None, API_TOKEN, JOBS_TOKEN)
                .expect("missing token disables route"),
            None
        );
    }

    #[test]
    fn automation_log_json_contains_metadata_only() {
        let value = serde_json::to_value(AutomationLogRow {
            id: 1,
            ts: 2,
            script_hash: "fixed-action-hash".into(),
            success: true,
            duration_ms: 3,
        })
        .expect("serialize automation metadata");
        let object = value.as_object().expect("automation row object");
        assert_eq!(object.len(), 5);
        for key in ["id", "ts", "script_hash", "success", "duration_ms"] {
            assert!(object.contains_key(key), "missing metadata field {key}");
        }
        assert!(!object.contains_key("script_preview"));
        assert!(!object.contains_key("script"));
    }
}
