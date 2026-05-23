// task_pins.rs — Pin/unpin tasks in the "Запустить таск" picker.
// Pins are keyed by (source_type, source_id), the same identity the picker uses.
use crate::types::HanniDb;

/// Toggle a pin. Returns true if the task is pinned after the call, false if cleared.
#[tauri::command]
pub fn toggle_task_pin(
    source_type: String, source_id: i64, db: tauri::State<'_, HanniDb>,
) -> Result<bool, String> {
    let conn = db.conn();
    let exists: bool = conn.query_row(
        "SELECT 1 FROM task_pins WHERE source_type=?1 AND source_id=?2",
        rusqlite::params![source_type, source_id], |_| Ok(true),
    ).unwrap_or(false);
    if exists {
        conn.execute("DELETE FROM task_pins WHERE source_type=?1 AND source_id=?2",
            rusqlite::params![source_type, source_id]).map_err(|e| format!("DB error: {}", e))?;
        Ok(false)
    } else {
        conn.execute(
            "INSERT INTO task_pins (source_type, source_id, created_at) VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![source_type, source_id]).map_err(|e| format!("DB error: {}", e))?;
        Ok(true)
    }
}

/// All current pins as ["source_type:source_id", ...] for quick client-side lookup.
#[tauri::command]
pub fn get_task_pins(db: tauri::State<'_, HanniDb>) -> Result<Vec<String>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT source_type, source_id FROM task_pins")
        .map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |r| {
        Ok(format!("{}:{}", r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
