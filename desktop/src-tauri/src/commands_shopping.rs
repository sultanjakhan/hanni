// commands_shopping.rs — Shopping-list CRUD for the "🛒 Закупка" event template.
// Backs the multi-select picker that fills the event description with items
// and marks them bought_at when the event is saved.

use serde::{Deserialize, Serialize};

use crate::types::HanniDb;

#[derive(Serialize, Deserialize)]
pub struct ShoppingItem {
    pub id: i64,
    pub name: String,
    pub qty: String,
    pub note: String,
    pub added_at: String,
    pub bought_at: Option<String>,
}

#[tauri::command]
pub fn list_shopping_items(
    include_bought: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<ShoppingItem>, String> {
    let conn = db.conn();
    let sql = if include_bought.unwrap_or(false) {
        "SELECT id, name, qty, note, added_at, bought_at FROM shopping_list ORDER BY added_at DESC"
    } else {
        "SELECT id, name, qty, note, added_at, bought_at FROM shopping_list WHERE bought_at IS NULL ORDER BY added_at DESC"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |r| Ok(ShoppingItem {
        id: r.get(0)?, name: r.get(1)?, qty: r.get(2)?, note: r.get(3)?,
        added_at: r.get(4)?, bought_at: r.get(5)?,
    })).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_shopping_item(
    name: String, qty: Option<String>, note: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let trimmed = name.trim();
    if trimmed.is_empty() { return Err("Название обязательно".into()); }
    // Re-open a previously-bought entry with the same name instead of
    // creating duplicates — keeps the list tidy if the user re-stocks.
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM shopping_list WHERE bought_at IS NOT NULL AND lower(name)=lower(?1) ORDER BY id DESC LIMIT 1",
        rusqlite::params![trimmed], |r| r.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE shopping_list SET bought_at=NULL, qty=?2, note=?3, added_at=?4 WHERE id=?1",
            rusqlite::params![id, qty.unwrap_or_default(), note.unwrap_or_default(), now],
        ).map_err(|e| format!("DB error: {}", e))?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO shopping_list (name, qty, note, added_at) VALUES (?1,?2,?3,?4)",
        rusqlite::params![trimmed, qty.unwrap_or_default(), note.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn mark_shopping_bought(
    ids: Vec<i64>, db: tauri::State<'_, HanniDb>,
) -> Result<usize, String> {
    if ids.is_empty() { return Ok(0); }
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i + 1)).collect();
    let sql = format!("UPDATE shopping_list SET bought_at=?1 WHERE id IN ({})", placeholders.join(","));
    let mut params: Vec<rusqlite::types::Value> = vec![now.into()];
    for id in &ids { params.push((*id).into()); }
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let n = conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(n)
}

#[tauri::command]
pub fn delete_shopping_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM shopping_list WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}
