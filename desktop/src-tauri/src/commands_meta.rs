// commands_meta.rs — blocklist, goals, settings, home items, page meta, custom properties, views, UI state
use crate::types::*;
use std::sync::atomic::Ordering;

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
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
        rusqlite::params![key, value],
    ).map_err(|e| format!("DB error: {}", e))?;
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
    let conn = db.read();
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
