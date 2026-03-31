// commands_data.rs — Life tracker, activities, projects, hobbies, sports, health, media, food, money, training, flywheel
use crate::types::*;
use crate::prompts::SYSTEM_PROMPT;
use crate::commands_meta::{start_focus, stop_focus};
use chrono::Timelike;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::io::Write;

// ── Life Tracker ──
// ── Life Tracker commands ──

pub fn load_tracker_data() -> Result<TrackerData, String> {
    let path = data_file_path();
    if !path.exists() {
        return Err("Life Tracker data file not found".into());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read tracker data: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Cannot parse tracker data: {}", e))
}

pub fn save_tracker_data(data: &TrackerData) -> Result<(), String> {
    let path = data_file_path();
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Cannot write: {}", e))
}

#[tauri::command]
pub async fn tracker_add_purchase(amount: f64, category: String, description: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("p_{}", now.timestamp_millis()),
        "date": now.format("%Y-%m-%d").to_string(),
        "amount": amount,
        "currency": "KZT",
        "category": category,
        "description": description,
        "tags": [],
        "source": "hanni"
    });
    data.purchases.push(entry.clone());
    save_tracker_data(&data)?;
    Ok(format!("Added purchase: {} KZT — {}", amount, description))
}

#[tauri::command]
pub async fn tracker_add_time(activity: String, duration: u32, category: String, productive: bool) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("t_{}", now.timestamp_millis()),
        "date": now.format("%Y-%m-%d").to_string(),
        "duration": duration,
        "activity": activity,
        "category": category,
        "productive": productive,
        "notes": "",
        "source": "hanni"
    });
    data.time_entries.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added time: {} min — {}", duration, activity))
}

#[tauri::command]
pub async fn tracker_add_goal(title: String, category: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("g_{}", now.timestamp_millis()),
        "title": title,
        "description": "",
        "category": category,
        "progress": 0,
        "milestones": [],
        "status": "active",
        "createdAt": now.to_rfc3339()
    });
    data.goals.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added goal: {}", title))
}

#[tauri::command]
pub async fn tracker_add_note(title: String, content: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("n_{}", now.timestamp_millis()),
        "title": title,
        "content": content,
        "tags": [],
        "pinned": false,
        "archived": false,
        "createdAt": now.to_rfc3339(),
        "updatedAt": now.to_rfc3339()
    });
    data.notes.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added note: {}", title))
}

#[tauri::command]
pub async fn tracker_get_stats() -> Result<String, String> {
    let data = load_tracker_data()?;
    let today = chrono::Local::now().format("%Y-%m").to_string();

    let month_purchases: f64 = data.purchases.iter()
        .filter(|p| p["date"].as_str().unwrap_or("").starts_with(&today))
        .map(|p| p["amount"].as_f64().unwrap_or(0.0))
        .sum();

    let month_time: u64 = data.time_entries.iter()
        .filter(|t| t["date"].as_str().unwrap_or("").starts_with(&today))
        .map(|t| t["duration"].as_u64().unwrap_or(0))
        .sum();

    let active_goals = data.goals.iter()
        .filter(|g| g["status"].as_str().unwrap_or("") == "active")
        .count();

    let total_notes = data.notes.len();

    Ok(format!(
        "📊 Статистика за {}:\n• Расходы: {:.0} KZT ({} записей)\n• Время: {} мин ({} записей)\n• Активных целей: {}\n• Заметок: {}",
        today, month_purchases, data.purchases.len(),
        month_time, data.time_entries.len(),
        active_goals, total_notes
    ))
}

#[tauri::command]
pub async fn tracker_get_recent(entry_type: String, limit: usize) -> Result<String, String> {
    let data = load_tracker_data()?;
    let entries: Vec<&serde_json::Value> = match entry_type.as_str() {
        "purchases" => data.purchases.iter().rev().take(limit).collect(),
        "time" => data.time_entries.iter().rev().take(limit).collect(),
        "goals" => data.goals.iter().rev().take(limit).collect(),
        "notes" => data.notes.iter().rev().take(limit).collect(),
        _ => return Err(format!("Unknown type: {}", entry_type)),
    };
    serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Serialize error: {}", e))
}


// ── Activities ──
// ── v0.7.0: Activities (Focus) commands ──

#[tauri::command]
pub fn start_activity(
    title: String,
    category: String,
    focus_mode: bool,
    duration: Option<u64>,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    db: tauri::State<'_, HanniDb>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO activities (title, category, started_at, focus_mode, created_at) VALUES (?1, ?2, ?3, ?4, ?3)",
        rusqlite::params![title, category, now, focus_mode as i32],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id = conn.last_insert_rowid();

    // Optionally start focus blocking
    if focus_mode {
        drop(conn);
        let dur = duration.unwrap_or(120);
        let _ = start_focus(dur, apps, sites, focus);
    }
    Ok(id)
}

#[tauri::command]
pub fn stop_activity(
    db: tauri::State<'_, HanniDb>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Find current (unfinished) activity
    let result: Result<(i64, String), _> = conn.query_row(
        "SELECT id, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok((id, started_at)) => {
            if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let duration = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                conn.execute(
                    "UPDATE activities SET ended_at=?1, duration_minutes=?2 WHERE id=?3",
                    rusqlite::params![now, duration, id],
                ).map_err(|e| format!("DB error: {}", e))?;
            } else {
                conn.execute(
                    "UPDATE activities SET ended_at=?1 WHERE id=?2",
                    rusqlite::params![now, id],
                ).map_err(|e| format!("DB error: {}", e))?;
            }
            // Stop focus if active
            drop(conn);
            let _ = stop_focus(focus);
            Ok("Activity stopped".into())
        }
        Err(_) => Ok("No active activity".into()),
    }
}

#[tauri::command]
pub fn get_current_activity(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let result: Result<(i64, String, String, String), _> = conn.query_row(
        "SELECT id, title, category, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );
    match result {
        Ok((id, title, category, started_at)) => {
            let elapsed = if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let mins = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                let h = mins / 60;
                let m = mins % 60;
                if h > 0 { format!("{}ч {}м", h, m) } else { format!("{}м", m) }
            } else { String::new() };
            Ok(serde_json::json!({ "id": id, "title": title, "category": category, "started_at": started_at, "elapsed": elapsed }))
        }
        Err(_) => Err("No active activity".into()),
    }
}

#[tauri::command]
pub fn get_activity_log(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn.prepare(
        "SELECT id, title, category, started_at, ended_at, duration_minutes FROM activities
         WHERE started_at LIKE ?1 ORDER BY started_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", target_date);
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| {
        let started: String = row.get(3)?;
        let time = if started.len() >= 16 { started[11..16].to_string() } else { String::new() };
        let dur_min: Option<i64> = row.get(5)?;
        let duration = dur_min.map(|m| if m >= 60 { format!("{}ч {}м", m/60, m%60) } else { format!("{}м", m) }).unwrap_or_default();
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "time": time,
            "duration": duration,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}


// ── Projects, Tasks, Learning, Hobbies, Workouts, Health, Dashboard, Memory Browser, Media, Food, Money ──
// ── v0.7.0: Projects & Tasks (Work) commands ──

// ── Job Sources ──

#[tauri::command]
pub fn get_job_sources(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, name, type, url, active, created_at FROM job_sources ORDER BY name")
        .map_err(|e| format!("DB error: {e}"))?;
    let rows = stmt.query_map([], |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "type": row.get::<_, String>(2)?, "url": row.get::<_, String>(3)?,
        "active": row.get::<_, i64>(4)? == 1, "created_at": row.get::<_, String>(5)?,
    }))).map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_job_source(name: String, source_type: String, url: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute("INSERT INTO job_sources (name, type, url) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, source_type, url]).map_err(|e| format!("DB error: {e}"))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_job_source(id: i64, name: Option<String>, source_type: Option<String>, url: Option<String>, active: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = name { conn.execute("UPDATE job_sources SET name=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = source_type { conn.execute("UPDATE job_sources SET type=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = url { conn.execute("UPDATE job_sources SET url=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = active { conn.execute("UPDATE job_sources SET active=?1 WHERE id=?2", rusqlite::params![v as i64, id]).map_err(|e| format!("DB error: {e}"))?; }
    Ok(())
}

#[tauri::command]
pub fn delete_job_source(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM job_sources WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {e}"))?;
    Ok(())
}

// ── Job Roles ──

#[tauri::command]
pub fn get_job_roles(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, title, keywords, salary_min, priority, created_at FROM job_roles ORDER BY priority, title")
        .map_err(|e| format!("DB error: {e}"))?;
    let rows = stmt.query_map([], |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "title": row.get::<_, String>(1)?,
        "keywords": row.get::<_, String>(2)?, "salary_min": row.get::<_, Option<i64>>(3)?,
        "priority": row.get::<_, String>(4)?, "created_at": row.get::<_, String>(5)?,
    }))).map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_job_role(title: String, keywords: String, salary_min: Option<i64>, priority: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute("INSERT INTO job_roles (title, keywords, salary_min, priority) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![title, keywords, salary_min, priority]).map_err(|e| format!("DB error: {e}"))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_job_role(id: i64, title: Option<String>, keywords: Option<String>, salary_min: Option<i64>, priority: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = title { conn.execute("UPDATE job_roles SET title=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = keywords { conn.execute("UPDATE job_roles SET keywords=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = salary_min { conn.execute("UPDATE job_roles SET salary_min=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = priority { conn.execute("UPDATE job_roles SET priority=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {e}"))?; }
    Ok(())
}

#[tauri::command]
pub fn delete_job_role(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM job_roles WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {e}"))?;
    Ok(())
}

// ── Job Vacancies ──

#[tauri::command]
pub fn get_job_vacancies(stage: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let base = "SELECT v.id, v.company, v.position, v.url, v.stage, v.contact, v.applied_at, v.source, v.notes, v.found_at, v.updated_at, v.salary
         FROM job_vacancies v";
    let sql = if stage.is_some() { format!("{base} WHERE v.stage=?1 ORDER BY v.updated_at DESC") }
              else { format!("{base} ORDER BY v.updated_at DESC") };
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {e}"))?;
    let rows: Vec<serde_json::Value> = if let Some(ref s) = stage {
        stmt.query_map(rusqlite::params![s], |row| vacancy_row(row))
            .map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).collect()
    } else {
        stmt.query_map([], |row| vacancy_row(row))
            .map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).collect()
    };
    Ok(rows)
}

fn vacancy_row(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "company": row.get::<_, String>(1)?,
        "position": row.get::<_, String>(2)?, "url": row.get::<_, String>(3)?,
        "stage": row.get::<_, String>(4)?, "contact": row.get::<_, String>(5)?,
        "applied_at": row.get::<_, Option<String>>(6)?, "source": row.get::<_, String>(7)?,
        "notes": row.get::<_, String>(8)?, "found_at": row.get::<_, String>(9)?,
        "updated_at": row.get::<_, String>(10)?, "salary": row.get::<_, String>(11)?,
    }))
}

#[tauri::command]
pub fn add_job_vacancy(company: String, position: String, url: String, stage: String, contact: Option<String>, source: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO job_vacancies (company, position, url, stage, contact, source) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![company, position, url, stage, contact.unwrap_or_default(), source.unwrap_or_default()],
    ).map_err(|e| format!("DB error: {e}"))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_job_vacancy(id: i64, company: Option<String>, position: Option<String>, url: Option<String>, stage: Option<String>, contact: Option<String>, applied_at: Option<String>, source: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    if let Some(v) = company { conn.execute("UPDATE job_vacancies SET company=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = position { conn.execute("UPDATE job_vacancies SET position=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = url { conn.execute("UPDATE job_vacancies SET url=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = stage { conn.execute("UPDATE job_vacancies SET stage=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = contact { conn.execute("UPDATE job_vacancies SET contact=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = applied_at { conn.execute("UPDATE job_vacancies SET applied_at=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    if let Some(v) = source { conn.execute("UPDATE job_vacancies SET source=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id]).map_err(|e| format!("DB error: {e}"))?; }
    Ok(())
}

#[tauri::command]
pub fn delete_job_vacancy(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM job_vacancies WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {e}"))?;
    Ok(())
}

// ── Job Stats ──

#[tauri::command]
pub fn get_job_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM job_vacancies", [], |r| r.get(0)).unwrap_or(0);
    let applied_week: i64 = conn.query_row(
        "SELECT COUNT(*) FROM job_vacancies WHERE stage='applied' AND updated_at >= date('now', '-7 days')", [], |r| r.get(0)
    ).unwrap_or(0);
    let sources: i64 = conn.query_row("SELECT COUNT(*) FROM job_sources WHERE active=1", [], |r| r.get(0)).unwrap_or(0);
    let mut by_stage = serde_json::Map::new();
    let mut stmt = conn.prepare("SELECT stage, COUNT(*) FROM job_vacancies GROUP BY stage").map_err(|e| format!("DB error: {e}"))?;
    let _ = stmt.query_map([], |row| {
        let stage: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        by_stage.insert(stage, serde_json::json!(count));
        Ok(())
    }).map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).count();
    Ok(serde_json::json!({ "total": total, "by_stage": by_stage, "applied_this_week": applied_week, "sources_count": sources }))
}

// ── Job Search Log ──

#[tauri::command]
pub fn add_job_search_log(source_id: Option<i64>, found_count: i64, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute("INSERT INTO job_search_log (source_id, found_count, notes) VALUES (?1, ?2, ?3)",
        rusqlite::params![source_id, found_count, notes]).map_err(|e| format!("DB error: {e}"))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_job_search_log(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT l.id, l.source_id, l.searched_at, l.found_count, l.notes, COALESCE(s.name, '') as source_name
         FROM job_search_log l LEFT JOIN job_sources s ON l.source_id=s.id ORDER BY l.searched_at DESC LIMIT 100"
    ).map_err(|e| format!("DB error: {e}"))?;
    let rows = stmt.query_map([], |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "source_id": row.get::<_, Option<i64>>(1)?,
        "searched_at": row.get::<_, String>(2)?, "found_count": row.get::<_, i64>(3)?,
        "notes": row.get::<_, String>(4)?, "source_name": row.get::<_, String>(5)?,
    }))).map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Learning Items (Development) commands ──

#[tauri::command]
pub fn create_learning_item(item_type: String, title: String, description: String, url: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO learning_items (type, title, description, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![item_type, title, description, url, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_learning_items(type_filter: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let rows = if let Some(t) = type_filter {
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, url, progress, status, category FROM learning_items WHERE type=?1 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![t], |row| learning_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        result
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, url, progress, status, category FROM learning_items ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result: Vec<serde_json::Value> = stmt.query_map([], |row| learning_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        result
    };
    Ok(rows)
}

pub fn learning_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "type": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?,
        "description": row.get::<_, String>(3)?,
        "url": row.get::<_, String>(4)?,
        "progress": row.get::<_, i32>(5)?,
        "status": row.get::<_, String>(6)?,
        "category": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
pub fn update_learning_item_status(id: i64, status: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE learning_items SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_learning_item(id: i64, title: Option<String>, item_type: Option<String>, status: Option<String>, progress: Option<i32>, url: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let mut updates = vec![format!("updated_at=?1")];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
    let mut idx = 2;
    if let Some(v) = title { updates.push(format!("title=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = item_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = status { updates.push(format!("status=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = progress { updates.push(format!("progress=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = url { updates.push(format!("url=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    params.push(Box::new(id));
    let sql = format!("UPDATE learning_items SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_learning_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM learning_items WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.7.0: Hobbies commands ──

#[tauri::command]
pub fn create_hobby(name: String, category: String, icon: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO hobbies (name, category, icon, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, category, icon, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_hobbies(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT h.id, h.name, h.category, h.icon, h.color,
                COALESCE((SELECT SUM(duration_minutes) FROM hobby_entries WHERE hobby_id=h.id), 0) / 60.0 as total_hours
         FROM hobbies h ORDER BY h.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "color": row.get::<_, String>(4)?,
            "total_hours": format!("{:.1}", row.get::<_, f64>(5)?),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn log_hobby_entry(hobby_id: i64, duration_minutes: i64, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO hobby_entries (hobby_id, date, duration_minutes, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![hobby_id, date, duration_minutes, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_hobby_entries(hobby_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, date, duration_minutes, notes FROM hobby_entries WHERE hobby_id=?1 ORDER BY date DESC LIMIT 30"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![hobby_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "date": row.get::<_, String>(1)?,
            "duration_minutes": row.get::<_, i64>(2)?,
            "notes": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Workouts (Sports) commands ──

#[tauri::command]
pub fn create_workout(workout_type: String, title: String, duration_minutes: i64, calories: Option<i64>, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO workouts (type, title, date, duration_minutes, calories, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![workout_type, title, date, duration_minutes, calories, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_workouts(_date_range: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, type, title, date, duration_minutes, calories, notes FROM workouts ORDER BY date DESC, created_at DESC LIMIT 50"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, String>(1)?,
            "title": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "duration_minutes": row.get::<_, i64>(4)?,
            "calories": row.get::<_, Option<i64>>(5)?,
            "notes": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_workout_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let (count, total_min, total_cal): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(duration_minutes), 0), COALESCE(SUM(calories), 0) FROM workouts WHERE date >= ?1",
        rusqlite::params![week_ago],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0, 0));
    Ok(serde_json::json!({ "count": count, "total_minutes": total_min, "total_calories": total_cal }))
}

#[tauri::command]
pub fn delete_workout(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM workouts WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_workout(id: i64, title: Option<String>, workout_type: Option<String>, duration_minutes: Option<i64>, calories: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = title { updates.push(format!("title=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = workout_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = duration_minutes { updates.push(format!("duration_minutes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = calories { updates.push(format!("calories=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE workouts SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Schedules commands ──

#[tauri::command]
pub fn create_schedule(title: String, category: String, frequency: String, frequency_days: Option<String>, time_of_day: Option<String>, details: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO schedules (title, category, frequency, frequency_days, time_of_day, details, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![title, category, frequency, frequency_days, time_of_day, details.unwrap_or_default(), chrono::Local::now().to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_schedules(category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let sql = if category.is_some() {
        "SELECT id, title, category, frequency, frequency_days, time_of_day, details, is_active, created_at, marks_previous_day FROM schedules WHERE category=?1 ORDER BY title"
    } else {
        "SELECT id, title, category, frequency, frequency_days, time_of_day, details, is_active, created_at, marks_previous_day FROM schedules ORDER BY title"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| format!("DB error: {}", e))?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = if let Some(ref cat) = category {
        vec![Box::new(cat.clone())]
    } else { vec![] };
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "frequency": row.get::<_, String>(3)?,
            "frequency_days": row.get::<_, Option<String>>(4)?,
            "time_of_day": row.get::<_, Option<String>>(5)?,
            "details": row.get::<_, String>(6)?,
            "is_active": row.get::<_, i64>(7)? == 1,
            "created_at": row.get::<_, Option<String>>(8)?,
            "marks_previous_day": row.get::<_, i64>(9).unwrap_or(0) == 1,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_schedule(id: i64, title: Option<String>, category: Option<String>, frequency: Option<String>, frequency_days: Option<String>, time_of_day: Option<String>, details: Option<String>, is_active: Option<bool>, marks_previous_day: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = title { conn.execute("UPDATE schedules SET title=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = category { conn.execute("UPDATE schedules SET category=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = frequency { conn.execute("UPDATE schedules SET frequency=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = frequency_days { conn.execute("UPDATE schedules SET frequency_days=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = time_of_day { conn.execute("UPDATE schedules SET time_of_day=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = details { conn.execute("UPDATE schedules SET details=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = is_active { conn.execute("UPDATE schedules SET is_active=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    if let Some(v) = marks_previous_day { conn.execute("UPDATE schedules SET marks_previous_day=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    Ok(())
}

#[tauri::command]
pub fn delete_schedule(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM schedule_completions WHERE schedule_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM schedules WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_schedule_completion(schedule_id: i64, date: String, db: tauri::State<'_, HanniDb>) -> Result<bool, String> {
    let conn = db.conn();
    let existing: Option<i64> = conn.query_row(
        "SELECT completed FROM schedule_completions WHERE schedule_id=?1 AND date=?2",
        rusqlite::params![schedule_id, date], |row| row.get(0),
    ).ok();
    match existing {
        Some(1) => {
            conn.execute("UPDATE schedule_completions SET completed=0, completed_at=NULL WHERE schedule_id=?1 AND date=?2",
                rusqlite::params![schedule_id, date]).ok();
            Ok(false)
        }
        Some(_) => {
            conn.execute("UPDATE schedule_completions SET completed=1, completed_at=?3 WHERE schedule_id=?1 AND date=?2",
                rusqlite::params![schedule_id, date, chrono::Local::now().to_rfc3339()]).ok();
            Ok(true)
        }
        None => {
            conn.execute("INSERT INTO schedule_completions (schedule_id, date, completed, completed_at) VALUES (?1, ?2, 1, ?3)",
                rusqlite::params![schedule_id, date, chrono::Local::now().to_rfc3339()]).ok();
            Ok(true)
        }
    }
}

#[tauri::command]
pub fn get_schedule_completions(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT sc.schedule_id, sc.completed, s.title, s.category, s.time_of_day, sc.completed_at
         FROM schedule_completions sc JOIN schedules s ON s.id = sc.schedule_id
         WHERE sc.date=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![date], |row| {
        Ok(serde_json::json!({
            "schedule_id": row.get::<_, i64>(0)?,
            "completed": row.get::<_, i64>(1)? == 1,
            "title": row.get::<_, String>(2)?,
            "category": row.get::<_, String>(3)?,
            "time_of_day": row.get::<_, Option<String>>(4)?,
            "completed_at": row.get::<_, Option<String>>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_schedule_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM schedules WHERE is_active=1", [], |r| r.get(0)).unwrap_or(0);
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let completed_week: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedule_completions WHERE completed=1 AND date>=?1", rusqlite::params![week_ago], |r| r.get(0)
    ).unwrap_or(0);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let completed_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedule_completions WHERE completed=1 AND date=?1", rusqlite::params![today], |r| r.get(0)
    ).unwrap_or(0);
    Ok(serde_json::json!({ "total_active": total, "completed_week": completed_week, "completed_today": completed_today }))
}

// ── Dan Koe Protocol commands ──

#[tauri::command]
pub fn get_dan_koe_entry(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Option<serde_json::Value>, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let entry = conn.query_row(
        "SELECT id, date, contemplation, pattern_interrupt, vision, integration, notes FROM dan_koe_entries WHERE date=?1",
        rusqlite::params![d], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "date": row.get::<_, String>(1)?,
                "contemplation": row.get::<_, i64>(2)? == 1,
                "pattern_interrupt": row.get::<_, i64>(3)? == 1,
                "vision": row.get::<_, i64>(4)? == 1,
                "integration": row.get::<_, i64>(5)? == 1,
                "notes": row.get::<_, String>(6)?,
            }))
        },
    ).ok();
    Ok(entry)
}

#[tauri::command]
pub fn save_dan_koe_entry(date: Option<String>, contemplation: bool, pattern_interrupt: bool, vision: bool, integration: bool, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO dan_koe_entries (date, contemplation, pattern_interrupt, vision, integration, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(date) DO UPDATE SET contemplation=?2, pattern_interrupt=?3, vision=?4, integration=?5, notes=?6",
        rusqlite::params![d, contemplation as i64, pattern_interrupt as i64, vision as i64, integration as i64, notes.unwrap_or_default(), chrono::Local::now().to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_dan_koe_history(days: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let since = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, date, contemplation, pattern_interrupt, vision, integration, notes FROM dan_koe_entries WHERE date>=?1 ORDER BY date DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![since], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "date": row.get::<_, String>(1)?,
            "contemplation": row.get::<_, i64>(2)? == 1,
            "pattern_interrupt": row.get::<_, i64>(3)? == 1,
            "vision": row.get::<_, i64>(4)? == 1,
            "integration": row.get::<_, i64>(5)? == 1,
            "notes": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_dan_koe_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let total_week: i64 = conn.query_row("SELECT COUNT(*) FROM dan_koe_entries WHERE date>=?1", rusqlite::params![week_ago], |r| r.get(0)).unwrap_or(0);
    let full_week: i64 = conn.query_row(
        "SELECT COUNT(*) FROM dan_koe_entries WHERE date>=?1 AND contemplation=1 AND pattern_interrupt=1 AND vision=1 AND integration=1",
        rusqlite::params![week_ago], |r| r.get(0)
    ).unwrap_or(0);
    // Current streak
    let mut stmt = conn.prepare("SELECT date FROM dan_koe_entries WHERE contemplation=1 AND pattern_interrupt=1 AND vision=1 AND integration=1 ORDER BY date DESC")
        .map_err(|e| format!("DB error: {}", e))?;
    let dates: Vec<String> = stmt.query_map([], |row| row.get(0)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    let mut streak = 0i64;
    let today = chrono::Local::now().date_naive();
    for (i, d) in dates.iter().enumerate() {
        if let Ok(parsed) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            let expected = today - chrono::Duration::days(i as i64);
            if parsed == expected { streak += 1; } else { break; }
        }
    }
    Ok(serde_json::json!({ "week_entries": total_week, "week_complete": full_week, "streak": streak }))
}

// ── v0.7.0: Health & Habits commands ──

#[tauri::command]
pub fn log_health(health_type: String, value: f64, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let unit = match health_type.as_str() {
        "sleep" => "hours", "water" => "glasses", "weight" => "kg", "mood" => "1-5", "steps" => "steps",
        _ => "",
    };
    // Upsert: update if same date+type exists
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM health_log WHERE date=?1 AND type=?2 LIMIT 1",
        rusqlite::params![date, health_type],
        |row| row.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE health_log SET value=?1, notes=?2 WHERE id=?3",
            rusqlite::params![value, notes.unwrap_or_default(), id],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO health_log (date, type, value, unit, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![date, health_type, value, unit, notes.unwrap_or_default(), now.to_rfc3339()],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(conn.last_insert_rowid())
    }
}

#[tauri::command]
pub fn get_health_today(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT type, value FROM health_log WHERE date=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let mut result = serde_json::json!({});
    let rows = stmt.query_map(rusqlite::params![today], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?;
    for row in rows.flatten() {
        result[row.0] = serde_json::json!(row.1);
    }
    Ok(result)
}

#[tauri::command]
pub fn create_habit(name: String, icon: String, frequency: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO habits (name, icon, frequency, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, icon, frequency, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn check_habit(habit_id: i64, date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let now = chrono::Local::now().to_rfc3339();
    // Toggle: if exists, delete; else insert
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM habit_checks WHERE habit_id=?1 AND date=?2",
        rusqlite::params![habit_id, target_date],
        |row| row.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute("DELETE FROM habit_checks WHERE id=?1", rusqlite::params![id])
            .map_err(|e| format!("DB error: {}", e))?;
    } else {
        conn.execute(
            "INSERT INTO habit_checks (habit_id, date, completed, created_at) VALUES (?1, ?2, 1, ?3)",
            rusqlite::params![habit_id, target_date, now],
        ).map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn update_habit(id: i64, name: Option<String>, frequency: Option<String>, icon: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = frequency { updates.push(format!("frequency=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = icon { updates.push(format!("icon=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE habits SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_habit(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM habit_checks WHERE habit_id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    conn.execute("DELETE FROM habits WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Activities: get_all, update, delete (for Focus DatabaseView) ──

#[tauri::command]
pub fn get_all_activities(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, category, started_at, ended_at, duration_minutes, focus_mode, notes
         FROM activities ORDER BY started_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "started_at": row.get::<_, String>(3)?,
            "ended_at": row.get::<_, Option<String>>(4)?,
            "duration_minutes": row.get::<_, Option<i64>>(5)?,
            "focus_mode": row.get::<_, i64>(6)?,
            "notes": row.get::<_, Option<String>>(7)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_activity(id: i64, title: Option<String>, category: Option<String>, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = title { updates.push(format!("title=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = notes { updates.push(format!("notes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE activities SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_activity(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM activities WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_habits_today(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT h.id, h.name, h.icon, h.frequency,
                (SELECT COUNT(*) FROM habit_checks WHERE habit_id=h.id AND date=?1) as checked,
                (SELECT COUNT(*) FROM habit_checks hc WHERE hc.habit_id=h.id AND hc.date >= date(?1, '-30 days')) as streak_approx
         FROM habits h ORDER BY h.created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![today], |row| {
        // Simple streak calc: count consecutive days backward
        let checked: i64 = row.get(4)?;
        let streak_approx: i64 = row.get(5)?;
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "icon": row.get::<_, String>(2)?,
            "frequency": row.get::<_, String>(3)?,
            "completed": checked > 0,
            "streak": streak_approx,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Dashboard aggregate command ──

#[tauri::command]
pub fn get_dashboard_data(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today_pattern = format!("{}%", today);

    // Current activity
    let current_activity: Option<serde_json::Value> = conn.query_row(
        "SELECT title, category, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            let started: String = row.get(2)?;
            let elapsed = if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started) {
                let mins = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                format!("{}м", mins)
            } else { String::new() };
            Ok(serde_json::json!({ "title": row.get::<_, String>(0)?, "category": row.get::<_, String>(1)?, "elapsed": elapsed }))
        },
    ).ok();

    // Activities count today
    let activities_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activities WHERE started_at LIKE ?1", rusqlite::params![today_pattern], |row| row.get(0),
    ).unwrap_or(0);

    // Focus minutes today
    let focus_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_minutes), 0) FROM activities WHERE started_at LIKE ?1 AND ended_at IS NOT NULL",
        rusqlite::params![today_pattern], |row| row.get(0),
    ).unwrap_or(0);

    // Notes count
    let notes_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes WHERE archived=0", [], |row| row.get(0)).unwrap_or(0);

    // Events today
    let mut events_stmt = conn.prepare(
        "SELECT title, time FROM events WHERE date=?1 ORDER BY time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let events: Vec<serde_json::Value> = events_stmt.query_map(rusqlite::params![today], |row| {
        Ok(serde_json::json!({ "title": row.get::<_, String>(0)?, "time": row.get::<_, String>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    // Recent notes
    let mut notes_stmt = conn.prepare(
        "SELECT title FROM notes WHERE archived=0 ORDER BY updated_at DESC LIMIT 3"
    ).map_err(|e| format!("DB error: {}", e))?;
    let recent_notes: Vec<serde_json::Value> = notes_stmt.query_map([], |row| {
        Ok(serde_json::json!({ "title": row.get::<_, String>(0)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    Ok(serde_json::json!({
        "current_activity": current_activity,
        "activities_today": activities_today,
        "focus_minutes": focus_minutes,
        "notes_count": notes_count,
        "events_today": events.len(),
        "events": events,
        "recent_notes": recent_notes,
    }))
}

// ── Notification widget data ──

#[tauri::command]
pub fn get_notifications(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let current_time = now.format("%H:%M").to_string();
    let soon_time = (now + chrono::Duration::hours(2)).format("%H:%M").to_string();

    // Upcoming events (next 2 hours)
    let mut upcoming: Vec<serde_json::Value> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title, time FROM events WHERE date = ?1 AND time >= ?2 AND time <= ?3 ORDER BY time"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![&today, &current_time, &soon_time], |row| {
            let title: String = row.get(0)?;
            let time: String = row.get(1)?;
            Ok(serde_json::json!({"type": "event", "title": title, "time": time}))
        }) {
            upcoming.extend(rows.flatten());
        }
    }

    // Overdue tasks
    let mut overdue: Vec<serde_json::Value> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title, due_date FROM tasks WHERE due_date < ?1 AND due_date != '' AND completed_at IS NULL AND status != 'done' ORDER BY due_date DESC LIMIT 5"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![&today], |row| {
            let title: String = row.get(0)?;
            let due: String = row.get(1)?;
            Ok(serde_json::json!({"type": "overdue", "title": title, "due_date": due}))
        }) {
            overdue.extend(rows.flatten());
        }
    }

    let total = upcoming.len() + overdue.len();
    Ok(serde_json::json!({
        "upcoming": upcoming,
        "overdue": overdue,
        "total": total,
        "now": current_time,
    }))
}

// ── Proactive messages ──

#[tauri::command]
pub fn save_proactive_message(text: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO proactive_messages (text) VALUES (?1)",
        rusqlite::params![text],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_proactive_messages(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, text, created_at, read, rating FROM proactive_messages WHERE archived = 0 ORDER BY id DESC LIMIT 5"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "text": row.get::<_, String>(1)?,
            "created_at": row.get::<_, String>(2)?,
            "read": row.get::<_, bool>(3)?,
            "rating": row.get::<_, i64>(4).unwrap_or(0),
        }))
    }).map_err(|e| format!("DB error: {}", e))?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn rate_proactive_message(id: i64, rating: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute(
        "UPDATE proactive_messages SET rating = ?1 WHERE id = ?2",
        rusqlite::params![rating, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn mark_proactive_read(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE proactive_messages SET read = 1 WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn archive_old_proactive(db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let cutoff = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let count = conn.execute(
        "UPDATE proactive_messages SET archived = 1 WHERE archived = 0 AND created_at < ?1",
        rusqlite::params![cutoff],
    ).map_err(|e| format!("DB error: {}", e))? as i64;
    Ok(count)
}

// ── v0.7.0: Memory browser command ──

#[tauri::command]
pub fn get_all_memories(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(q) = search {
        if !q.trim().is_empty() {
            let like = format!("%{}%", q);
            let mut stmt = conn.prepare(
                "SELECT id, category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1 ORDER BY updated_at DESC LIMIT 100"
            ).map_err(|e| format!("DB error: {}", e))?;
            let rows = stmt.query_map(rusqlite::params![like], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "category": row.get::<_, String>(1)?,
                    "key": row.get::<_, String>(2)?,
                    "value": row.get::<_, String>(3)?,
                }))
            }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
            return Ok(rows);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT id, category, key, value FROM facts ORDER BY category, updated_at DESC LIMIT 200"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "category": row.get::<_, String>(1)?,
            "key": row.get::<_, String>(2)?,
            "value": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_memory(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let _ = conn.execute("DELETE FROM vec_facts WHERE fact_id=?1", rusqlite::params![id]);
    conn.execute("DELETE FROM facts WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_memory(id: i64, category: Option<String>, key: Option<String>, value: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    if let Some(cat) = &category {
        conn.execute("UPDATE facts SET category=?1, updated_at=?2 WHERE id=?3", rusqlite::params![cat, now, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(k) = &key {
        conn.execute("UPDATE facts SET key=?1, updated_at=?2 WHERE id=?3", rusqlite::params![k, now, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(v) = &value {
        conn.execute("UPDATE facts SET value=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

/// Clean up memory: remove duplicates (same key, keep newest), remove stale facts
/// (not accessed in 60+ days with low access_count), and remove very short/vague entries.
#[tauri::command]
pub fn memory_cleanup(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut removed = 0u32;
    let merged = 0u32;

    // 1. Remove exact duplicates (same category+key, keep the one with most recent updated_at)
    let dup_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM facts WHERE id NOT IN (
            SELECT MAX(id) FROM facts GROUP BY category, key
        )",
        [], |row| row.get(0),
    ).unwrap_or(0);
    if dup_count > 0 {
        // Delete embeddings for duplicates first
        let _ = conn.execute(
            "DELETE FROM vec_facts WHERE fact_id NOT IN (
                SELECT MAX(id) FROM facts GROUP BY category, key
            )", [],
        );
        let _ = conn.execute(
            "DELETE FROM facts WHERE id NOT IN (
                SELECT MAX(id) FROM facts GROUP BY category, key
            )", [],
        );
        removed += dup_count as u32;
    }

    // 2. Remove stale facts: not accessed in 90+ days, never accessed (access_count=0), source = 'auto'
    let stale: i64 = conn.query_row(
        "SELECT COUNT(*) FROM facts
         WHERE source = 'auto'
           AND COALESCE(access_count, 0) = 0
           AND (last_accessed IS NULL OR julianday('now') - julianday(last_accessed) > 90)
           AND julianday('now') - julianday(updated_at) > 90",
        [], |row| row.get(0),
    ).unwrap_or(0);
    if stale > 0 {
        let _ = conn.execute(
            "DELETE FROM vec_facts WHERE fact_id IN (
                SELECT id FROM facts
                WHERE source = 'auto'
                  AND COALESCE(access_count, 0) = 0
                  AND (last_accessed IS NULL OR julianday('now') - julianday(last_accessed) > 90)
                  AND julianday('now') - julianday(updated_at) > 90
            )", [],
        );
        let _ = conn.execute(
            "DELETE FROM facts
             WHERE source = 'auto'
               AND COALESCE(access_count, 0) = 0
               AND (last_accessed IS NULL OR julianday('now') - julianday(last_accessed) > 90)
               AND julianday('now') - julianday(updated_at) > 90",
            [],
        );
        removed += stale as u32;
    }

    // 3. Remove very short values (less than 3 chars — likely noise)
    let short: i64 = conn.query_row(
        "SELECT COUNT(*) FROM facts WHERE LENGTH(value) < 3",
        [], |row| row.get(0),
    ).unwrap_or(0);
    if short > 0 {
        let _ = conn.execute("DELETE FROM vec_facts WHERE fact_id IN (SELECT id FROM facts WHERE LENGTH(value) < 3)", []);
        let _ = conn.execute("DELETE FROM facts WHERE LENGTH(value) < 3", []);
        removed += short as u32;
    }

    // Report total facts remaining
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM facts", [], |row| row.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "removed": removed,
        "merged": merged,
        "total_remaining": total,
    }))
}

// ── v0.8.0: Media Items (Hobbies collections) ──

#[tauri::command]
pub fn add_media_item(
    media_type: String, title: String, original_title: Option<String>, year: Option<i32>,
    description: Option<String>, cover_url: Option<String>, status: Option<String>,
    rating: Option<i32>, progress: Option<i32>, total_episodes: Option<i32>,
    notes: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO media_items (media_type, title, original_title, year, description, cover_url, status, rating, progress, total_episodes, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        rusqlite::params![
            media_type, title, original_title.unwrap_or_default(), year,
            description.unwrap_or_default(), cover_url.unwrap_or_default(),
            status.unwrap_or_else(|| "planned".into()), rating.unwrap_or(0),
            progress.unwrap_or(0), total_episodes,
            notes.unwrap_or_default(), now
        ],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_media_item(
    id: i64, status: Option<String>, rating: Option<i32>, progress: Option<i32>,
    notes: Option<String>, title: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Build dynamic update
    let (cur_status, cur_rating, cur_progress, cur_notes, cur_title): (String, i32, i32, String, String) = conn.query_row(
        "SELECT status, rating, progress, notes, title FROM media_items WHERE id=?1",
        rusqlite::params![id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    let new_status = status.unwrap_or(cur_status);
    let completed_at = if new_status == "completed" { Some(now.clone()) } else { None };
    conn.execute(
        "UPDATE media_items SET status=?1, rating=?2, progress=?3, notes=?4, title=?5, completed_at=?6, updated_at=?7 WHERE id=?8",
        rusqlite::params![new_status, rating.unwrap_or(cur_rating), progress.unwrap_or(cur_progress),
            notes.unwrap_or(cur_notes), title.unwrap_or(cur_title), completed_at, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_media_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM list_items WHERE media_item_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM media_items WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_media_items(media_type: String, status: Option<String>, show_hidden: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let hidden = if show_hidden.unwrap_or(false) { 1 } else { 0 };
    if let Some(s) = status {
        let mut stmt = conn.prepare(
            "SELECT id, media_type, title, original_title, year, status, rating, progress, total_episodes, cover_url, notes, hidden, created_at
             FROM media_items WHERE media_type=?1 AND status=?2 AND hidden<=?3 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![media_type, s, hidden], |row| media_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, media_type, title, original_title, year, status, rating, progress, total_episodes, cover_url, notes, hidden, created_at
             FROM media_items WHERE media_type=?1 AND hidden<=?2 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![media_type, hidden], |row| media_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn media_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "media_type": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?,
        "original_title": row.get::<_, String>(3)?,
        "year": row.get::<_, Option<i32>>(4)?,
        "status": row.get::<_, String>(5)?,
        "rating": row.get::<_, i32>(6)?,
        "progress": row.get::<_, i32>(7)?,
        "total_episodes": row.get::<_, Option<i32>>(8)?,
        "cover_url": row.get::<_, String>(9)?,
        "notes": row.get::<_, String>(10)?,
        "hidden": row.get::<_, i32>(11)? != 0,
    }))
}

#[tauri::command]
pub fn hide_media_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE media_items SET hidden=1 WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn unhide_media_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE media_items SET hidden=0 WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn create_user_list(name: String, description: Option<String>, color: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO user_lists (name, description, color, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, description.unwrap_or_default(), color.unwrap_or_else(|| "#818cf8".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_user_lists(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT ul.id, ul.name, ul.description, ul.color,
                (SELECT COUNT(*) FROM list_items WHERE list_id=ul.id) as item_count
         FROM user_lists ul ORDER BY ul.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "color": row.get::<_, String>(3)?,
            "item_count": row.get::<_, i64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_to_list(list_id: i64, media_item_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO list_items (list_id, media_item_id, added_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![list_id, media_item_id, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn remove_from_list(list_id: i64, media_item_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM list_items WHERE list_id=?1 AND media_item_id=?2",
        rusqlite::params![list_id, media_item_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_list_items(list_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT m.id, m.media_type, m.title, m.status, m.rating, m.cover_url
         FROM list_items li JOIN media_items m ON m.id = li.media_item_id
         WHERE li.list_id=?1 ORDER BY li.position, li.added_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![list_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "media_type": row.get::<_, String>(1)?,
            "title": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "rating": row.get::<_, i32>(4)?,
            "cover_url": row.get::<_, String>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_media_stats(media_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    if let Some(mt) = media_type {
        let (total, completed, in_progress, avg_rating): (i64, i64, i64, f64) = conn.query_row(
            "SELECT COUNT(*), SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status='in_progress' THEN 1 ELSE 0 END),
                    COALESCE(AVG(CASE WHEN rating>0 THEN rating END), 0)
             FROM media_items WHERE media_type=?1 AND hidden=0",
            rusqlite::params![mt], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap_or((0, 0, 0, 0.0));
        Ok(serde_json::json!({ "total": total, "completed": completed, "in_progress": in_progress, "avg_rating": format!("{:.1}", avg_rating) }))
    } else {
        let mut stmt = conn.prepare(
            "SELECT media_type, COUNT(*), SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END)
             FROM media_items WHERE hidden=0 GROUP BY media_type"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "media_type": row.get::<_, String>(0)?,
                "total": row.get::<_, i64>(1)?,
                "completed": row.get::<_, i64>(2)?,
            }))
        }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(serde_json::json!({ "by_type": rows }))
    }
}

// ── v0.8.0: Food commands ──

#[tauri::command]
pub fn log_food(
    date: Option<String>, meal_type: String, name: String,
    calories: Option<i64>, protein: Option<f64>, carbs: Option<f64>, fat: Option<f64>,
    notes: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO food_log (date, meal_type, name, calories, protein, carbs, fat, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![d, meal_type, name, calories.unwrap_or(0), protein.unwrap_or(0.0),
            carbs.unwrap_or(0.0), fat.unwrap_or(0.0), notes.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_food_log(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn.prepare(
        "SELECT id, meal_type, name, calories, protein, carbs, fat, notes FROM food_log WHERE date=?1 ORDER BY created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![d], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "meal_type": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?, "calories": row.get::<_, i64>(3)?,
            "protein": row.get::<_, f64>(4)?, "carbs": row.get::<_, f64>(5)?,
            "fat": row.get::<_, f64>(6)?, "notes": row.get::<_, String>(7)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_food_entry(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM food_log WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_food_entry(id: i64, name: Option<String>, meal_type: Option<String>, calories: Option<i64>, protein: Option<f64>, carbs: Option<f64>, fat: Option<f64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = meal_type { updates.push(format!("meal_type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = calories { updates.push(format!("calories=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = protein { updates.push(format!("protein=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = carbs { updates.push(format!("carbs=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = fat { updates.push(format!("fat=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE food_log SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_food_stats(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let d = days.unwrap_or(7);
    let since = (chrono::Local::now() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let (total_cal, avg_cal, total_protein): (i64, f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(calories),0), COALESCE(AVG(daily_cal),0), COALESCE(SUM(protein),0)
         FROM (SELECT date, SUM(calories) as daily_cal, SUM(protein) as protein FROM food_log WHERE date>=?1 GROUP BY date)",
        rusqlite::params![since], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0.0, 0.0));
    Ok(serde_json::json!({ "total_calories": total_cal, "avg_daily_calories": format!("{:.0}", avg_cal), "total_protein": format!("{:.1}", total_protein), "days": d }))
}

#[tauri::command]
pub fn create_recipe(
    name: String, description: Option<String>, ingredients: String, instructions: String,
    prep_time: Option<i64>, cook_time: Option<i64>, servings: Option<i64>,
    calories: Option<i64>, tags: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time, servings, calories, tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        rusqlite::params![name, description.unwrap_or_default(), ingredients, instructions,
            prep_time.unwrap_or(0), cook_time.unwrap_or(0), servings.unwrap_or(1),
            calories.unwrap_or(0), tags.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_recipes(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(q) = search {
        let like = format!("%{}%", q);
        let mut stmt = conn.prepare(
            "SELECT id, name, description, prep_time, cook_time, servings, calories, tags FROM recipes WHERE name LIKE ?1 OR tags LIKE ?1 ORDER BY updated_at DESC LIMIT 50"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![like], |row| recipe_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, description, prep_time, cook_time, servings, calories, tags FROM recipes ORDER BY updated_at DESC LIMIT 50"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| recipe_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn recipe_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "description": row.get::<_, String>(2)?, "prep_time": row.get::<_, i64>(3)?,
        "cook_time": row.get::<_, i64>(4)?, "servings": row.get::<_, i64>(5)?,
        "calories": row.get::<_, i64>(6)?, "tags": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
pub fn update_recipe(id: i64, name: Option<String>, prep_time: Option<i64>, cook_time: Option<i64>, servings: Option<i64>, calories: Option<i64>, tags: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = prep_time { updates.push(format!("prep_time=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = cook_time { updates.push(format!("cook_time=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = servings { updates.push(format!("servings=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = calories { updates.push(format!("calories=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = tags { updates.push(format!("tags=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE recipes SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_recipe(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM recipes WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn add_product(
    name: String, category: Option<String>, quantity: Option<f64>, unit: Option<String>,
    expiry_date: Option<String>, location: Option<String>, notes: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO products (name, category, quantity, unit, expiry_date, location, notes, purchased_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        rusqlite::params![name, category.unwrap_or_else(|| "other".into()), quantity.unwrap_or(1.0),
            unit.unwrap_or_else(|| "шт".into()), expiry_date,
            location.unwrap_or_else(|| "fridge".into()), notes.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_products(location: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(loc) = location {
        let mut stmt = conn.prepare(
            "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products WHERE location=?1 ORDER BY expiry_date NULLS LAST"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![loc], |row| product_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products ORDER BY expiry_date NULLS LAST"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| product_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn product_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "category": row.get::<_, String>(2)?, "quantity": row.get::<_, f64>(3)?,
        "unit": row.get::<_, String>(4)?, "expiry_date": row.get::<_, Option<String>>(5)?,
        "location": row.get::<_, String>(6)?, "notes": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
pub fn update_product(id: i64, name: Option<String>, quantity: Option<f64>, expiry_date: Option<String>, location: Option<String>, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = quantity { updates.push(format!("quantity=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = expiry_date { updates.push(format!("expiry_date=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = location { updates.push(format!("location=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = notes { updates.push(format!("notes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE products SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_product(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM products WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_expiring_products(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = days.unwrap_or(3);
    let deadline = (chrono::Local::now() + chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products
         WHERE expiry_date IS NOT NULL AND expiry_date <= ?1 ORDER BY expiry_date"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![deadline], |row| product_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.8.0: Money commands ──

#[tauri::command]
pub fn add_transaction(
    date: Option<String>, transaction_type: String, amount: f64, currency: Option<String>,
    category: String, description: Option<String>, recurring: Option<bool>,
    recurring_period: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO transactions (date, type, amount, currency, category, description, recurring, recurring_period, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![d, transaction_type, amount, currency.unwrap_or_else(|| "KZT".into()),
            category, description.unwrap_or_default(), recurring.unwrap_or(false) as i32,
            recurring_period, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_transactions(month: Option<String>, transaction_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let prefix = month.unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let pattern = format!("{}%", prefix);
    if let Some(t) = transaction_type {
        let mut stmt = conn.prepare(
            "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE date LIKE ?1 AND type=?2 ORDER BY date DESC, created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern, t], |row| tx_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE date LIKE ?1 ORDER BY date DESC, created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| tx_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn tx_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
        "type": row.get::<_, String>(2)?, "amount": row.get::<_, f64>(3)?,
        "currency": row.get::<_, String>(4)?, "category": row.get::<_, String>(5)?,
        "description": row.get::<_, String>(6)?,
    }))
}

#[tauri::command]
pub fn update_transaction(id: i64, amount: Option<f64>, category: Option<String>, description: Option<String>, tx_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = amount { updates.push(format!("amount=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = description { updates.push(format!("description=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = tx_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE transactions SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_transaction(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM transactions WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_transaction_stats(month: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let prefix = month.unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let pattern = format!("{}%", prefix);
    let (total_expense, total_income): (f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN type='expense' THEN amount END), 0),
                COALESCE(SUM(CASE WHEN type='income' THEN amount END), 0)
         FROM transactions WHERE date LIKE ?1",
        rusqlite::params![pattern], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0.0, 0.0));
    // By category
    let mut stmt = conn.prepare(
        "SELECT category, SUM(amount) FROM transactions WHERE date LIKE ?1 AND type='expense' GROUP BY category ORDER BY SUM(amount) DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let by_cat: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({ "category": row.get::<_, String>(0)?, "amount": row.get::<_, f64>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({ "total_expense": total_expense, "total_income": total_income, "balance": total_income - total_expense, "by_category": by_cat }))
}

#[tauri::command]
pub fn create_budget(category: String, amount: f64, period: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let p = period.unwrap_or_else(|| "monthly".into());
    conn.execute(
        "INSERT INTO budgets (category, amount, period, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(category, period) DO UPDATE SET amount=?2",
        rusqlite::params![category, amount, p, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_budgets(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let pattern = format!("{}%", month);
    let mut stmt = conn.prepare(
        "SELECT b.id, b.category, b.amount, b.period,
                COALESCE((SELECT SUM(amount) FROM transactions WHERE category=b.category AND type='expense' AND date LIKE ?1), 0) as spent
         FROM budgets b ORDER BY b.category"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "category": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "period": row.get::<_, String>(3)?,
            "spent": row.get::<_, f64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_budget(id: i64, category: Option<String>, amount: Option<f64>, period: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = amount { updates.push(format!("amount=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = period { updates.push(format!("period=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE budgets SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_budget(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM budgets WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn create_savings_goal(name: String, target_amount: f64, deadline: Option<String>, color: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO savings_goals (name, target_amount, deadline, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, target_amount, deadline, color.unwrap_or_else(|| "#818cf8".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_savings_goals(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, target_amount, current_amount, deadline, color FROM savings_goals ORDER BY created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        let target: f64 = row.get(2)?;
        let current: f64 = row.get(3)?;
        let pct = if target > 0.0 { (current / target * 100.0).min(100.0) } else { 0.0 };
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "target_amount": target, "current_amount": current,
            "deadline": row.get::<_, Option<String>>(4)?, "color": row.get::<_, String>(5)?,
            "percent": format!("{:.0}", pct),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_savings_goal(id: i64, add_amount: Option<f64>, target_amount: Option<f64>, name: Option<String>, deadline: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(add) = add_amount {
        conn.execute("UPDATE savings_goals SET current_amount = current_amount + ?1 WHERE id=?2", rusqlite::params![add, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(target) = target_amount {
        conn.execute("UPDATE savings_goals SET target_amount=?1 WHERE id=?2", rusqlite::params![target, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(v) = name {
        conn.execute("UPDATE savings_goals SET name=?1 WHERE id=?2", rusqlite::params![v, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(v) = deadline {
        conn.execute("UPDATE savings_goals SET deadline=?1 WHERE id=?2", rusqlite::params![v, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_savings_goal(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM savings_goals WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn add_subscription(name: String, amount: f64, currency: Option<String>, period: Option<String>, next_payment: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO subscriptions (name, amount, currency, period, next_payment, category, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, amount, currency.unwrap_or_else(|| "KZT".into()), period.unwrap_or_else(|| "monthly".into()),
            next_payment, category.unwrap_or_else(|| "other".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_subscriptions(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, currency, period, next_payment, category, active FROM subscriptions ORDER BY active DESC, name"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "currency": row.get::<_, String>(3)?,
            "period": row.get::<_, String>(4)?, "next_payment": row.get::<_, Option<String>>(5)?,
            "category": row.get::<_, String>(6)?, "active": row.get::<_, i32>(7)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_subscription(id: i64, active: Option<bool>, amount: Option<f64>, name: Option<String>, period: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = active { updates.push(format!("active=?{}", idx)); params.push(Box::new(v as i32)); idx += 1; }
    if let Some(v) = amount { updates.push(format!("amount=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = period { updates.push(format!("period=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE subscriptions SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_subscription(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM subscriptions WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn add_debt(name: String, debt_type: String, amount: f64, interest_rate: Option<f64>, due_date: Option<String>, description: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO debts (name, type, amount, remaining, interest_rate, due_date, description, created_at) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, debt_type, amount, interest_rate.unwrap_or(0.0), due_date, description.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_debts(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, type, amount, remaining, interest_rate, due_date, description FROM debts WHERE remaining > 0 ORDER BY due_date NULLS LAST"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        let amt: f64 = row.get(3)?;
        let rem: f64 = row.get(4)?;
        let pct = if amt > 0.0 { ((amt - rem) / amt * 100.0).min(100.0) } else { 0.0 };
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?, "amount": amt, "remaining": rem,
            "interest_rate": row.get::<_, f64>(5)?, "due_date": row.get::<_, Option<String>>(6)?,
            "description": row.get::<_, String>(7)?, "paid_percent": format!("{:.0}", pct),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_debt(id: i64, pay_amount: Option<f64>, name: Option<String>, remaining: Option<f64>, debt_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(pay) = pay_amount {
        conn.execute("UPDATE debts SET remaining = MAX(0, remaining - ?1) WHERE id=?2", rusqlite::params![pay, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = remaining { updates.push(format!("remaining=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = debt_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE debts SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_debt(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM debts WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

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
    let conn = db.conn();
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
    let conn = db.0.lock().map_err(|e| e.to_string())?;
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
    let conn = db.0.lock().map_err(|e| e.to_string())?;
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
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM body_records WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_body_zones_summary(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
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
