// notes.rs — Notes CRUD, custom pages, focus overlay
use crate::types::*;
use tauri::{AppHandle, Manager};

// ── v0.7.0: Notes commands ──

#[tauri::command]
pub fn create_note(
    title: String, content: String, tags: String,
    tab_name: Option<String>, status: Option<String>,
    due_date: Option<String>, reminder_at: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let st = status.unwrap_or_else(|| "note".to_string());
    conn.execute(
        "INSERT INTO notes (title, content, tags, tab_name, status, due_date, reminder_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        rusqlite::params![title, content, tags, tab_name, st, due_date, reminder_at, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_note(
    id: i64, title: String, content: String, tags: String,
    pinned: Option<bool>, archived: Option<bool>,
    tab_name: Option<String>, status: Option<String>,
    due_date: Option<String>, reminder_at: Option<String>,
    content_blocks: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let (cur_pinned, cur_archived, cur_tab, cur_status, cur_due, cur_reminder, cur_blocks): (i32, i32, Option<String>, String, Option<String>, Option<String>, Option<String>) = conn.query_row(
        "SELECT pinned, archived, tab_name, status, due_date, reminder_at, content_blocks FROM notes WHERE id=?1", rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, String>(3).unwrap_or_else(|_| "note".into()), row.get(4)?, row.get(5)?, row.get(6)?)),
    ).unwrap_or((0, 0, None, "note".into(), None, None, None));
    let p = pinned.map(|v| v as i32).unwrap_or(cur_pinned);
    let a = archived.map(|v| v as i32).unwrap_or(cur_archived);
    let tn = if tab_name.is_some() { tab_name } else { cur_tab };
    let st = status.unwrap_or(cur_status);
    let dd = if due_date.is_some() { due_date } else { cur_due };
    let ra = if reminder_at.is_some() { reminder_at } else { cur_reminder };
    let cb = if content_blocks.is_some() { content_blocks } else { cur_blocks };
    conn.execute(
        "UPDATE notes SET title=?1, content=?2, tags=?3, pinned=?4, archived=?5, tab_name=?6, status=?7, due_date=?8, reminder_at=?9, updated_at=?10, content_blocks=?11 WHERE id=?12",
        rusqlite::params![title, content, tags, p, a, tn, st, dd, ra, now, cb, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_note(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM notes WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_note_pin(id: i64, db: tauri::State<'_, HanniDb>) -> Result<bool, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute("UPDATE notes SET pinned = 1 - pinned, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id]).map_err(|e| format!("DB error: {}", e))?;
    let new_val: bool = conn.query_row("SELECT pinned != 0 FROM notes WHERE id=?1", rusqlite::params![id],
        |row| row.get(0)).unwrap_or(false);
    Ok(new_val)
}

#[tauri::command]
pub fn toggle_note_archive(id: i64, db: tauri::State<'_, HanniDb>) -> Result<bool, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute("UPDATE notes SET archived = 1 - archived, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id]).map_err(|e| format!("DB error: {}", e))?;
    let new_val: bool = conn.query_row("SELECT archived != 0 FROM notes WHERE id=?1", rusqlite::params![id],
        |row| row.get(0)).unwrap_or(false);
    Ok(new_val)
}

#[tauri::command]
pub fn get_notes(filter: Option<String>, search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let rows = if let Some(q) = search {
        if q.trim().is_empty() { get_notes_all(&conn)? }
        else {
            let words: Vec<&str> = q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
            if words.is_empty() { get_notes_all(&conn)? }
            else {
                let fts_query = words.join(" OR ");
                let mut stmt = conn.prepare(
                    "SELECT n.id, n.title, n.content, n.tags, n.pinned, n.archived, n.created_at, n.updated_at, n.tab_name, n.status, n.due_date, n.reminder_at, n.sort_order
                     FROM notes_fts fts JOIN notes n ON n.id = fts.rowid
                     WHERE notes_fts MATCH ?1 ORDER BY rank LIMIT 50"
                ).map_err(|e| format!("DB error: {}", e))?;
                let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![fts_query], |row| note_from_row(row))
                    .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
                result
            }
        }
    } else if let Some(ref f) = filter {
        if f == "tasks" {
            let mut stmt = conn.prepare(
                "SELECT id, title, content, tags, pinned, archived, created_at, updated_at, tab_name, status, due_date, reminder_at, sort_order, content_blocks FROM notes
                 WHERE status IN ('task','done') AND archived=0 ORDER BY CASE WHEN status='done' THEN 1 ELSE 0 END, due_date ASC NULLS LAST, updated_at DESC LIMIT 200"
            ).map_err(|e| format!("DB error: {}", e))?;
            let result: Vec<serde_json::Value> = stmt.query_map([], |row| note_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
            result
        } else if let Some(tab) = f.strip_prefix("tab:") {
            let mut stmt = conn.prepare(
                "SELECT id, title, content, tags, pinned, archived, created_at, updated_at, tab_name, status, due_date, reminder_at, sort_order, content_blocks FROM notes
                 WHERE tab_name=?1 AND archived=0 ORDER BY pinned DESC, sort_order ASC, updated_at DESC LIMIT 200"
            ).map_err(|e| format!("DB error: {}", e))?;
            let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![tab], |row| note_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
            result
        } else {
            get_notes_all(&conn)?
        }
    } else {
        get_notes_all(&conn)?
    };
    Ok(rows)
}

pub fn get_notes_all(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at, tab_name, status, due_date, reminder_at, sort_order, content_blocks FROM notes
         ORDER BY pinned DESC, updated_at DESC LIMIT 200"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| note_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

pub fn note_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "title": row.get::<_, String>(1)?,
        "content": row.get::<_, String>(2)?,
        "tags": row.get::<_, String>(3)?,
        "pinned": row.get::<_, i32>(4)? != 0,
        "archived": row.get::<_, i32>(5)? != 0,
        "created_at": row.get::<_, String>(6)?,
        "updated_at": row.get::<_, String>(7)?,
        "tab_name": row.get::<_, Option<String>>(8)?,
        "status": row.get::<_, String>(9)?,
        "due_date": row.get::<_, Option<String>>(10)?,
        "reminder_at": row.get::<_, Option<String>>(11)?,
        "sort_order": row.get::<_, i32>(12)?,
        "content_blocks": row.get::<_, Option<String>>(13)?,
    }))
}

#[tauri::command]
pub fn get_note(id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    conn.query_row(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at, tab_name, status, due_date, reminder_at, sort_order, content_blocks FROM notes WHERE id=?1",
        rusqlite::params![id],
        |row| note_from_row(row),
    ).map_err(|e| format!("Not found: {}", e))
}

#[tauri::command]
pub fn update_note_status(id: i64, status: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE notes SET status=?1, updated_at=?2 WHERE id=?3",
        rusqlite::params![status, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn reorder_notes(ids: Vec<i64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE notes SET sort_order=?1 WHERE id=?2",
            rusqlite::params![i as i32, id],
        ).map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_note_tags(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT name, color FROM note_tags ORDER BY name")
        .map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "name": row.get::<_, String>(0)?,
            "color": row.get::<_, String>(1)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn set_note_tag_color(name: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO note_tags (name, color) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE SET color=?2",
        rusqlite::params![name, color],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_notes_for_tab(tab_name: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at, tab_name, status, due_date, reminder_at, sort_order, content_blocks FROM notes
         WHERE tab_name=?1 AND archived=0 ORDER BY pinned DESC, sort_order ASC, updated_at DESC LIMIT 100"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![tab_name], |row| note_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.24.0: Custom Pages commands ──

#[tauri::command]
pub fn create_custom_page(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().to_rfc3339();
    let sort_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM custom_pages", [],
        |row| row.get(0),
    ).unwrap_or(0);
    conn.execute(
        "INSERT INTO custom_pages (id, title, icon, description, content, sub_tabs, sort_order, created_at, updated_at) VALUES (?1, 'Новая страница', '📄', '', '', '[]', ?2, ?3, ?3)",
        rusqlite::params![id, sort_order, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(serde_json::json!({
        "id": id,
        "title": "Новая страница",
        "icon": "📄",
        "description": "",
        "content": "",
        "sub_tabs": "[]",
        "sort_order": sort_order,
        "created_at": now,
        "updated_at": now,
    }))
}

#[tauri::command]
pub fn get_custom_pages(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, icon, description, content, sub_tabs, sort_order, created_at, updated_at, content_blocks FROM custom_pages ORDER BY sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "icon": row.get::<_, String>(2)?,
            "description": row.get::<_, String>(3)?,
            "content": row.get::<_, String>(4)?,
            "sub_tabs": row.get::<_, String>(5)?,
            "sort_order": row.get::<_, i32>(6)?,
            "created_at": row.get::<_, String>(7)?,
            "updated_at": row.get::<_, String>(8)?,
            "content_blocks": row.get::<_, Option<String>>(9)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_custom_page(id: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    conn.query_row(
        "SELECT id, title, icon, description, content, sub_tabs, sort_order, created_at, updated_at, content_blocks FROM custom_pages WHERE id=?1",
        rusqlite::params![id],
        |row| Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "icon": row.get::<_, String>(2)?,
            "description": row.get::<_, String>(3)?,
            "content": row.get::<_, String>(4)?,
            "sub_tabs": row.get::<_, String>(5)?,
            "sort_order": row.get::<_, i32>(6)?,
            "created_at": row.get::<_, String>(7)?,
            "updated_at": row.get::<_, String>(8)?,
            "content_blocks": row.get::<_, Option<String>>(9)?,
        })),
    ).map_err(|e| format!("Not found: {}", e))
}

#[tauri::command]
pub fn update_custom_page(
    id: String, title: Option<String>, icon: Option<String>,
    description: Option<String>, content: Option<String>,
    content_blocks: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Get current values for fields not provided
    let (cur_title, cur_icon, cur_desc, cur_content, cur_blocks): (String, String, String, String, Option<String>) = conn.query_row(
        "SELECT title, icon, description, content, content_blocks FROM custom_pages WHERE id=?1",
        rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    conn.execute(
        "UPDATE custom_pages SET title=?1, icon=?2, description=?3, content=?4, content_blocks=?5, updated_at=?6 WHERE id=?7",
        rusqlite::params![
            title.unwrap_or(cur_title),
            icon.unwrap_or(cur_icon),
            description.unwrap_or(cur_desc),
            content.unwrap_or(cur_content),
            if content_blocks.is_some() { content_blocks } else { cur_blocks },
            now, id
        ],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_custom_page(id: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM custom_pages WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Focus Overlay Window ──

#[tauri::command]
pub fn toggle_focus_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("focus-overlay") {
        let _ = win.close();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "focus-overlay",
        tauri::WebviewUrl::App("focus-overlay.html".into()),
    )
    .title("Focus")
    .inner_size(220.0, 56.0)
    .decorations(false)
    .always_on_top(true)
    .resizable(false)
    .skip_taskbar(true)
    .build()
    .map_err(|e| format!("Window error: {}", e))?;
    Ok(())
}

