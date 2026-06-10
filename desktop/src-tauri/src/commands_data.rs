// commands_data.rs — Life tracker, activities, projects, hobbies, sports, health, media, food, money, training, flywheel
use crate::types::*;
use crate::commands_focus::{start_focus, stop_focus};

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
    let conn = db.read();
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
    let sql = if stage.is_some() { format!("{base} WHERE v.deleted_at IS NULL AND v.stage=?1 ORDER BY v.updated_at DESC") }
              else { format!("{base} WHERE v.deleted_at IS NULL ORDER BY v.updated_at DESC") };
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
    let now = chrono::Local::now().to_rfc3339();
    db.conn().execute("UPDATE job_vacancies SET deleted_at=?1 WHERE id=?2", rusqlite::params![now, id]).map_err(|e| format!("DB error: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn restore_job_vacancy(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("UPDATE job_vacancies SET deleted_at=NULL WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {e}"))?;
    Ok(())
}

// ── Job Stats ──

#[tauri::command]
pub fn get_job_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM job_vacancies WHERE deleted_at IS NULL", [], |r| r.get(0)).unwrap_or(0);
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

// ── v0.32.0: Development Projects / Skills / Cases ──

#[tauri::command]
pub fn get_dev_projects(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, name, icon, sort_order, overview FROM dev_projects ORDER BY sort_order")
        .map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "icon": row.get::<_, String>(2)?, "sort_order": row.get::<_, i32>(3)?,
        "overview": row.get::<_, String>(4)?,
    }))).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_dev_project(name: String, icon: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute("INSERT INTO dev_projects (name, icon, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, icon, now]).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_dev_project(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM dev_projects WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_dev_project(id: i64, name: Option<String>, icon: Option<String>, overview: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates: Vec<String> = vec![];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = icon { updates.push(format!("icon=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = overview { updates.push(format!("overview=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE dev_projects SET {} WHERE id=?{}", updates.join(","), idx);
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_dev_nodes(project_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT n.id, n.parent_id, n.kind, n.name, n.score, n.theory, n.material, n.priority, n.sort_order, n.level,
                (SELECT COUNT(*) FROM dev_cases WHERE node_id=n.id) as case_count
         FROM dev_nodes n WHERE n.project_id=?1 ORDER BY n.sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![project_id], |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "parent_id": row.get::<_, Option<i64>>(1)?,
        "kind": row.get::<_, String>(2)?, "name": row.get::<_, String>(3)?,
        "score": row.get::<_, i32>(4)?, "theory": row.get::<_, String>(5)?,
        "material": row.get::<_, String>(6)?, "priority": row.get::<_, i32>(7)?,
        "sort_order": row.get::<_, i32>(8)?, "level": row.get::<_, String>(9)?,
        "case_count": row.get::<_, i32>(10)?,
    }))).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_dev_node(project_id: i64, parent_id: Option<i64>, kind: String, name: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let sort: i32 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order)+1,0) FROM dev_nodes WHERE project_id=?1 AND IFNULL(parent_id,0)=IFNULL(?2,0)",
        rusqlite::params![project_id, parent_id], |r| r.get(0),
    ).unwrap_or(0);
    conn.execute(
        "INSERT INTO dev_nodes (project_id, parent_id, kind, name, sort_order, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?6)",
        rusqlite::params![project_id, parent_id, kind, name, sort, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_dev_node(id: i64, name: Option<String>, score: Option<i32>, theory: Option<String>, material: Option<String>, priority: Option<i32>, level: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let mut updates = vec!["updated_at=?1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
    let mut idx = 2;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = score { updates.push(format!("score=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = theory { updates.push(format!("theory=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = material { updates.push(format!("material=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = priority { updates.push(format!("priority=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = level { updates.push(format!("level=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    params.push(Box::new(id));
    let sql = format!("UPDATE dev_nodes SET {} WHERE id=?{}", updates.join(","), idx);
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_dev_node(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    // Collect the subtree (node + descendants), then delete it with its cases.
    let mut ids = vec![id];
    let mut frontier = vec![id];
    while let Some(p) = frontier.pop() {
        let mut stmt = conn.prepare("SELECT id FROM dev_nodes WHERE parent_id=?1")
            .map_err(|e| format!("DB error: {}", e))?;
        let children: Vec<i64> = stmt.query_map([p], |r| r.get(0))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        for c in children { ids.push(c); frontier.push(c); }
    }
    for nid in ids {
        conn.execute("DELETE FROM dev_cases WHERE node_id=?1", [nid]).ok();
        conn.execute("DELETE FROM dev_nodes WHERE id=?1", [nid]).ok();
    }
    Ok(())
}

#[tauri::command]
pub fn get_dev_cases(node_id: Option<i64>, project_id: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(nid) = node_id {
        ("SELECT c.id, c.node_id, c.title, c.url, c.description, c.score, c.notes, c.solved_at, c.created_at, n.name \
          FROM dev_cases c JOIN dev_nodes n ON n.id=c.node_id WHERE c.node_id=?1 ORDER BY c.created_at DESC".into(),
         vec![Box::new(nid)])
    } else if let Some(pid) = project_id {
        ("SELECT c.id, c.node_id, c.title, c.url, c.description, c.score, c.notes, c.solved_at, c.created_at, n.name \
          FROM dev_cases c JOIN dev_nodes n ON n.id=c.node_id WHERE n.project_id=?1 ORDER BY c.created_at DESC".into(),
         vec![Box::new(pid)])
    } else {
        ("SELECT c.id, c.node_id, c.title, c.url, c.description, c.score, c.notes, c.solved_at, c.created_at, n.name \
          FROM dev_cases c JOIN dev_nodes n ON n.id=c.node_id ORDER BY c.created_at DESC".into(),
         vec![])
    };
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(refs.as_slice(), |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "node_id": row.get::<_, i64>(1)?,
        "title": row.get::<_, String>(2)?, "url": row.get::<_, String>(3)?,
        "description": row.get::<_, String>(4)?, "score": row.get::<_, i32>(5)?,
        "notes": row.get::<_, String>(6)?, "solved_at": row.get::<_, Option<String>>(7)?,
        "created_at": row.get::<_, String>(8)?, "node_name": row.get::<_, String>(9)?,
    }))).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_dev_case(node_id: i64, title: String, url: String, description: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute("INSERT INTO dev_cases (node_id, title, url, description, created_at) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![node_id, title, url, description, now]).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_dev_case(id: i64, title: Option<String>, url: Option<String>, score: Option<i32>, notes: Option<String>, solved_at: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = title { updates.push(format!("title=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = url { updates.push(format!("url=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = score { updates.push(format!("score=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = notes { updates.push(format!("notes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = solved_at { updates.push(format!("solved_at=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE dev_cases SET {} WHERE id=?{}", updates.join(","), idx);
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_dev_case(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM dev_cases WHERE id=?1", rusqlite::params![id])
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

// ── Schedules commands ──

#[tauri::command]
pub fn create_schedule(title: String, category: String, frequency: String, frequency_days: Option<String>, time_of_day: Option<String>, details: Option<String>, track_overdue: Option<bool>, target_minutes: Option<i64>, tracking_mode: Option<String>, marks_previous_day: Option<bool>, auto_source: Option<String>, visible_from: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let to = track_overdue.unwrap_or(false) as i64;
    let mode = tracking_mode.unwrap_or_else(|| "track".to_string());
    let mpd = marks_previous_day.unwrap_or(false) as i64;
    let src = auto_source.filter(|s| !s.is_empty());
    let vis = visible_from.filter(|s| !s.is_empty());
    let new_id = crate::types::new_uuid_v7();
    conn.execute(
        "INSERT INTO schedules (id, title, category, frequency, frequency_days, time_of_day, details, track_overdue, target_minutes, tracking_mode, marks_previous_day, auto_source, visible_from, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![new_id, title, category, frequency, frequency_days, time_of_day, details.unwrap_or_default(), to, target_minutes, mode, mpd, src, vis, chrono::Local::now().to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(new_id)
}

#[tauri::command]
pub fn get_schedules(category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.read();
    let sql = if category.is_some() {
        "SELECT id, title, category, frequency, frequency_days, time_of_day, details, is_active, created_at, marks_previous_day, until_date, COALESCE(track_overdue,0), target_minutes, COALESCE(tracking_mode,'track'), COALESCE(auto_source,''), COALESCE(visible_from,''), COALESCE(chain_only,0) FROM schedules WHERE category=?1 ORDER BY title"
    } else {
        "SELECT id, title, category, frequency, frequency_days, time_of_day, details, is_active, created_at, marks_previous_day, until_date, COALESCE(track_overdue,0), target_minutes, COALESCE(tracking_mode,'track'), COALESCE(auto_source,''), COALESCE(visible_from,''), COALESCE(chain_only,0) FROM schedules ORDER BY title"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| format!("DB error: {}", e))?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = if let Some(ref cat) = category {
        vec![Box::new(cat.clone())]
    } else { vec![] };
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "frequency": row.get::<_, String>(3)?,
            "frequency_days": row.get::<_, Option<String>>(4)?,
            "time_of_day": row.get::<_, Option<String>>(5)?,
            "details": row.get::<_, String>(6)?,
            "is_active": row.get::<_, i64>(7)? == 1,
            "created_at": row.get::<_, Option<String>>(8)?,
            "marks_previous_day": row.get::<_, i64>(9).unwrap_or(0) == 1,
            "until_date": row.get::<_, Option<String>>(10).unwrap_or(None),
            "track_overdue": row.get::<_, i64>(11).unwrap_or(0) == 1,
            "target_minutes": row.get::<_, Option<i64>>(12).unwrap_or(None),
            "tracking_mode": row.get::<_, String>(13).unwrap_or_else(|_| "track".to_string()),
            "auto_source": row.get::<_, String>(14).unwrap_or_default(),
            "visible_from": row.get::<_, String>(15).unwrap_or_default(),
            "chain_only": row.get::<_, i64>(16).unwrap_or(0) == 1,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_schedule(id: String, title: Option<String>, category: Option<String>, frequency: Option<String>, frequency_days: Option<String>, time_of_day: Option<String>, details: Option<String>, is_active: Option<bool>, marks_previous_day: Option<bool>, until_date: Option<String>, track_overdue: Option<bool>, target_minutes: Option<i64>, tracking_mode: Option<String>, auto_source: Option<String>, visible_from: Option<String>, chain_only: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = title { conn.execute("UPDATE schedules SET title=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = category { conn.execute("UPDATE schedules SET category=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = frequency { conn.execute("UPDATE schedules SET frequency=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = frequency_days { conn.execute("UPDATE schedules SET frequency_days=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = time_of_day { conn.execute("UPDATE schedules SET time_of_day=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = details { conn.execute("UPDATE schedules SET details=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = is_active { conn.execute("UPDATE schedules SET is_active=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    if let Some(v) = marks_previous_day { conn.execute("UPDATE schedules SET marks_previous_day=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    if let Some(v) = until_date {
        let val: Option<String> = if v.is_empty() { None } else { Some(v) };
        conn.execute("UPDATE schedules SET until_date=?1 WHERE id=?2", rusqlite::params![val, id]).ok();
    }
    if let Some(v) = track_overdue { conn.execute("UPDATE schedules SET track_overdue=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    if let Some(v) = target_minutes { conn.execute("UPDATE schedules SET target_minutes=?1 WHERE id=?2", rusqlite::params![if v <= 0 { None } else { Some(v) }, id]).ok(); }
    if let Some(v) = tracking_mode {
        let val = if v == "check" { "check" } else { "track" };
        conn.execute("UPDATE schedules SET tracking_mode=?1 WHERE id=?2", rusqlite::params![val, id]).ok();
    }
    if let Some(v) = auto_source {
        let val: Option<String> = if v.is_empty() { None } else { Some(v) };
        conn.execute("UPDATE schedules SET auto_source=?1 WHERE id=?2", rusqlite::params![val, id]).ok();
    }
    if let Some(v) = visible_from {
        let val: Option<String> = if v.is_empty() { None } else { Some(v) };
        conn.execute("UPDATE schedules SET visible_from=?1 WHERE id=?2", rusqlite::params![val, id]).ok();
    }
    if let Some(v) = chain_only { conn.execute("UPDATE schedules SET chain_only=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    Ok(())
}

#[tauri::command]
pub fn delete_schedule(id: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM schedule_completions WHERE schedule_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM schedules WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_schedule_completion(schedule_id: String, date: String, db: tauri::State<'_, HanniDb>) -> Result<bool, String> {
    let conn = db.conn();
    let existing: Option<i64> = conn.query_row(
        "SELECT completed FROM schedule_completions WHERE schedule_id=?1 AND date=?2",
        rusqlite::params![schedule_id, date], |row| row.get(0),
    ).ok();
    // Keep `status` in sync with `completed`. Otherwise a row marked done via the
    // timer flow (status='done') and later untoggled would stay status='done' and
    // get filtered out of the picker (which excludes status_extra='done').
    match existing {
        Some(1) => {
            conn.execute("UPDATE schedule_completions SET completed=0, completed_at=NULL, status='planned' WHERE schedule_id=?1 AND date=?2",
                rusqlite::params![schedule_id, date]).ok();
            crate::routine_engine::mirror_schedule_to_routine(&conn, &schedule_id, &date, "clear");
            Ok(false)
        }
        Some(_) => {
            conn.execute("UPDATE schedule_completions SET completed=1, completed_at=?3, status='done' WHERE schedule_id=?1 AND date=?2",
                rusqlite::params![schedule_id, date, chrono::Local::now().to_rfc3339()]).ok();
            crate::routine_engine::mirror_schedule_to_routine(&conn, &schedule_id, &date, "done");
            Ok(true)
        }
        None => {
            let new_id = crate::types::new_uuid_v7();
            conn.execute("INSERT INTO schedule_completions (id, schedule_id, date, completed, completed_at, status) VALUES (?1, ?2, ?3, 1, ?4, 'done')",
                rusqlite::params![new_id, schedule_id, date, chrono::Local::now().to_rfc3339()]).ok();
            crate::routine_engine::mirror_schedule_to_routine(&conn, &schedule_id, &date, "done");
            Ok(true)
        }
    }
}

/// Mark a schedule as "skipped today" — user explicitly decided not to do it.
/// Toggle behavior: skipped → planned (cleared). Cycle from done passes through
/// here too. Skipped counts as closed-but-not-completed: not an overdue, not a hit.
#[tauri::command]
pub fn skip_schedule_completion(schedule_id: String, date: String, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let existing: Option<String> = conn.query_row(
        "SELECT COALESCE(status, 'planned') FROM schedule_completions WHERE schedule_id=?1 AND date=?2",
        rusqlite::params![schedule_id, date], |row| row.get(0),
    ).ok();
    let new_status = if existing.as_deref() == Some("skipped") { "planned" } else { "skipped" };
    match existing {
        Some(_) => {
            conn.execute(
                "UPDATE schedule_completions SET completed=0, completed_at=NULL, status=?3 WHERE schedule_id=?1 AND date=?2",
                rusqlite::params![schedule_id, date, new_status],
            ).map_err(|e| format!("DB error: {}", e))?;
        }
        None => {
            let new_id = crate::types::new_uuid_v7();
            conn.execute(
                "INSERT INTO schedule_completions (id, schedule_id, date, completed, completed_at, status) VALUES (?1, ?2, ?3, 0, NULL, ?4)",
                rusqlite::params![new_id, schedule_id, date, new_status],
            ).map_err(|e| format!("DB error: {}", e))?;
        }
    }
    let mirror = if new_status == "skipped" { "skipped" } else { "clear" };
    crate::routine_engine::mirror_schedule_to_routine(&conn, &schedule_id, &date, mirror);
    Ok(new_status.to_string())
}

#[tauri::command]
pub fn get_schedule_completions(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.read();
    let mut stmt = conn.prepare(
        "SELECT sc.schedule_id, sc.completed, s.title, s.category, s.time_of_day, sc.completed_at, COALESCE(s.tracking_mode, 'track'), COALESCE(s.marks_previous_day, 0), COALESCE(sc.status, 'planned')
         FROM schedule_completions sc JOIN schedules s ON s.id = sc.schedule_id
         WHERE sc.date=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![date], |row| {
        Ok(serde_json::json!({
            "schedule_id": row.get::<_, String>(0)?,
            "completed": row.get::<_, i64>(1)? == 1,
            "title": row.get::<_, String>(2)?,
            "category": row.get::<_, String>(3)?,
            "time_of_day": row.get::<_, Option<String>>(4)?,
            "completed_at": row.get::<_, Option<String>>(5)?,
            "tracking_mode": row.get::<_, String>(6)?,
            "marks_previous_day": row.get::<_, i64>(7)? == 1,
            "status": row.get::<_, String>(8)?,
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
        "SELECT id, date, contemplation, pattern_interrupt, vision, integration, notes,
                contemplation_text, vision_text, integration_text
         FROM dan_koe_entries WHERE date=?1",
        rusqlite::params![d], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "date": row.get::<_, String>(1)?,
                "contemplation": row.get::<_, i64>(2)? == 1,
                "pattern_interrupt": row.get::<_, i64>(3)? == 1,
                "vision": row.get::<_, i64>(4)? == 1,
                "integration": row.get::<_, i64>(5)? == 1,
                "notes": row.get::<_, String>(6)?,
                "contemplation_text": row.get::<_, String>(7)?,
                "vision_text": row.get::<_, String>(8)?,
                "integration_text": row.get::<_, String>(9)?,
            }))
        },
    ).ok();
    Ok(entry)
}

#[tauri::command]
pub fn save_dan_koe_entry(
    date: Option<String>,
    pattern_interrupt: Option<bool>,
    contemplation_text: Option<String>,
    vision_text: Option<String>,
    integration_text: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let c_text = contemplation_text.unwrap_or_default();
    let v_text = vision_text.unwrap_or_default();
    let i_text = integration_text.unwrap_or_default();
    let pi = pattern_interrupt.unwrap_or(false);
    // Boolean flags derived from text presence — text = done.
    let c_done = !c_text.trim().is_empty();
    let v_done = !v_text.trim().is_empty();
    let i_done = !i_text.trim().is_empty();
    conn.execute(
        "INSERT INTO dan_koe_entries (date, contemplation, pattern_interrupt, vision, integration,
                                      contemplation_text, vision_text, integration_text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(date) DO UPDATE SET contemplation=?2, pattern_interrupt=?3, vision=?4, integration=?5,
                                          contemplation_text=?6, vision_text=?7, integration_text=?8",
        rusqlite::params![
            d, c_done as i64, pi as i64, v_done as i64, i_done as i64,
            c_text, v_text, i_text, chrono::Local::now().to_rfc3339(),
        ],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_dan_koe_history(days: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let since = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, date, contemplation, pattern_interrupt, vision, integration, notes,
                contemplation_text, vision_text, integration_text
         FROM dan_koe_entries WHERE date>=?1 ORDER BY date DESC"
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
            "contemplation_text": row.get::<_, String>(7)?,
            "vision_text": row.get::<_, String>(8)?,
            "integration_text": row.get::<_, String>(9)?,
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
pub fn log_health(health_type: String, value: f64, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let unit = match health_type.as_str() {
        "sleep" => "hours", "water" => "glasses", "weight" => "kg", "mood" => "1-5", "steps" => "steps",
        _ => "",
    };
    // Upsert: update if same date+type exists. Since Phase 2 id is TEXT (UUIDv7).
    let existing: Option<String> = conn.query_row(
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
        let new_id = crate::types::new_uuid_v7();
        conn.execute(
            "INSERT INTO health_log (id, date, type, value, unit, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![new_id, date, health_type, value, unit, notes.unwrap_or_default(), now.to_rfc3339()],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(new_id)
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
    let conn = db.read();
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
