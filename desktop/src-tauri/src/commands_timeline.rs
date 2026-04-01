// commands_timeline.rs — CRUD for timeline activity types and blocks
use crate::types::HanniDb;

// ── Activity Types ──

#[tauri::command]
pub fn create_activity_type(name: String, color: String, icon: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0) FROM timeline_activity_types", [], |r| r.get(0)
    ).unwrap_or(0);
    conn.execute(
        "INSERT INTO timeline_activity_types (name, color, icon, is_system, sort_order) VALUES (?1, ?2, ?3, 0, ?4)",
        rusqlite::params![name, color, icon, max_order + 1],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_activity_types(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, color, icon, is_system, sort_order FROM timeline_activity_types ORDER BY sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "color": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "is_system": row.get::<_, i64>(4)? == 1,
            "sort_order": row.get::<_, i64>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_activity_type(id: i64, name: Option<String>, color: Option<String>, icon: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = name { conn.execute("UPDATE timeline_activity_types SET name=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = color { conn.execute("UPDATE timeline_activity_types SET color=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = icon { conn.execute("UPDATE timeline_activity_types SET icon=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    Ok(())
}

#[tauri::command]
pub fn delete_activity_type(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let is_sys: i64 = conn.query_row(
        "SELECT is_system FROM timeline_activity_types WHERE id=?1", rusqlite::params![id], |r| r.get(0)
    ).unwrap_or(1);
    if is_sys == 1 { return Err("Cannot delete system type".into()); }
    conn.execute("DELETE FROM timeline_blocks WHERE type_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM timeline_goals WHERE type_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM timeline_activity_types WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Timeline Blocks ──

#[tauri::command]
pub fn create_timeline_block(type_id: i64, date: String, start_time: String, end_time: String, source: Option<String>, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let dur = calc_duration(&start_time, &end_time);
    let conn = db.conn();
    conn.execute(
        "INSERT INTO timeline_blocks (type_id, date, start_time, end_time, duration_minutes, source, notes) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![type_id, date, start_time, end_time, dur, source.unwrap_or_else(|| "manual".into()), notes.unwrap_or_default()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_timeline_blocks(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT b.id, b.type_id, b.date, b.start_time, b.end_time, b.duration_minutes, b.source, b.notes, t.name, t.color, t.icon
         FROM timeline_blocks b JOIN timeline_activity_types t ON t.id = b.type_id
         WHERE b.date=?1 ORDER BY b.start_time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![date], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type_id": row.get::<_, i64>(1)?,
            "date": row.get::<_, String>(2)?,
            "start_time": row.get::<_, String>(3)?,
            "end_time": row.get::<_, String>(4)?,
            "duration_minutes": row.get::<_, i64>(5)?,
            "source": row.get::<_, String>(6)?,
            "notes": row.get::<_, String>(7)?,
            "type_name": row.get::<_, String>(8)?,
            "type_color": row.get::<_, String>(9)?,
            "type_icon": row.get::<_, String>(10)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_timeline_block(id: i64, type_id: Option<i64>, start_time: Option<String>, end_time: Option<String>, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = type_id { conn.execute("UPDATE timeline_blocks SET type_id=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(ref s) = start_time {
        conn.execute("UPDATE timeline_blocks SET start_time=?1 WHERE id=?2", rusqlite::params![s, id]).ok();
    }
    if let Some(ref e) = end_time {
        conn.execute("UPDATE timeline_blocks SET end_time=?1 WHERE id=?2", rusqlite::params![e, id]).ok();
    }
    if start_time.is_some() || end_time.is_some() {
        let (s, e): (String, String) = conn.query_row(
            "SELECT start_time, end_time FROM timeline_blocks WHERE id=?1", rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?))
        ).map_err(|e| format!("DB error: {}", e))?;
        let dur = calc_duration(&s, &e);
        conn.execute("UPDATE timeline_blocks SET duration_minutes=?1 WHERE id=?2", rusqlite::params![dur, id]).ok();
    }
    if let Some(v) = notes { conn.execute("UPDATE timeline_blocks SET notes=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    Ok(())
}

#[tauri::command]
pub fn delete_timeline_block(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM timeline_blocks WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

pub fn calc_duration(start: &str, end: &str) -> i64 {
    let parse = |t: &str| -> i64 {
        let parts: Vec<&str> = t.split(':').collect();
        if parts.len() == 2 {
            parts[0].parse::<i64>().unwrap_or(0) * 60 + parts[1].parse::<i64>().unwrap_or(0)
        } else { 0 }
    };
    let s = parse(start);
    let e = parse(end);
    if e > s { e - s } else { (24 * 60 - s) + e }
}
