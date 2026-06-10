// commands_contacts.rs — People/Contacts CRUD + per-contact site/app blocks
use crate::types::*;

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
