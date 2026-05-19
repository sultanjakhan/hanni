// event_categories.rs — CRUD for user-managed calendar event categories
use crate::types::*;

#[tauri::command]
pub fn list_event_categories(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, color, icon, sort_order FROM event_categories ORDER BY sort_order, name"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "color": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "sort_order": row.get::<_, i64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_event_category(name: String, color: String, icon: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Имя категории не может быть пустым".into()); }
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM event_categories", [], |r| r.get(0)
    ).unwrap_or(1);
    // Set updated_at explicitly in RFC3339 — owner-sync compares it as a string
    // against an RFC3339 cursor, so the trigger's datetime('now') format won't do.
    conn.execute(
        "INSERT INTO event_categories (name, color, icon, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![name, color, icon, next_order, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_event_category(id: i64, name: Option<String>, color: Option<String>, icon: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    // Read old name first — needed to cascade rename to events.category.
    let old_name: String = conn.query_row(
        "SELECT name FROM event_categories WHERE id=?1",
        rusqlite::params![id], |r| r.get(0),
    ).map_err(|e| format!("Категория не найдена: {}", e))?;

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    let new_name_for_cascade = name.clone();
    if let Some(v) = name {
        let v = v.trim().to_string();
        if v.is_empty() { return Err("Имя категории не может быть пустым".into()); }
        updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1;
    }
    if let Some(v) = color { updates.push(format!("color=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = icon { updates.push(format!("icon=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    // Bump updated_at in RFC3339 for owner-sync (see create_event_category).
    updates.push(format!("updated_at=?{}", idx));
    params.push(Box::new(chrono::Local::now().to_rfc3339())); idx += 1;
    params.push(Box::new(id));
    let sql = format!("UPDATE event_categories SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;

    // Cascade rename: events.category is a TEXT label, keep it in sync.
    if let Some(new_name) = new_name_for_cascade {
        let new_name = new_name.trim();
        if new_name != old_name {
            conn.execute(
                "UPDATE events SET category=?1 WHERE category=?2",
                rusqlite::params![new_name, old_name],
            ).ok();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn delete_event_category(id: i64, reassign_to: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let name: String = conn.query_row(
        "SELECT name FROM event_categories WHERE id=?1",
        rusqlite::params![id], |r| r.get(0),
    ).map_err(|e| format!("Категория не найдена: {}", e))?;
    if name == "general" {
        return Err("Нельзя удалить базовую категорию 'general'".into());
    }
    let target = reassign_to.unwrap_or_else(|| "general".to_string());
    // Reassign events to target category before delete (preserves history).
    let affected = conn.execute(
        "UPDATE events SET category=?1 WHERE category=?2",
        rusqlite::params![target, name],
    ).map_err(|e| format!("DB error: {}", e))? as i64;
    conn.execute("DELETE FROM event_categories WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(affected)
}
