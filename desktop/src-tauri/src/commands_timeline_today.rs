// commands_timeline_today.rs — Today view: planned tasks from Calendar+Schedule+Notes, start/stop tracking
use crate::types::HanniDb;
use chrono::Datelike;

// Returns 1=Mon..7=Sun
fn day_of_week(date: &str) -> Result<u32, String> {
    let dt = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date: {}", e))?;
    Ok(dt.weekday().number_from_monday())
}

#[tauri::command]
pub fn get_today_planned(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut items: Vec<serde_json::Value> = Vec::new();

    // ── 1. Calendar events on date
    let mut e_stmt = conn.prepare(
        "SELECT id, title, time, duration_minutes, category, color, completed
         FROM events WHERE date=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let events_iter = e_stmt.query_map(rusqlite::params![date], |row| {
        let id: i64 = row.get(0)?;
        let title: String = row.get(1)?;
        let time: String = row.get(2)?;
        let dur: Option<i64> = row.get(3)?;
        let category: String = row.get(4)?;
        let color: String = row.get(5)?;
        let completed: i64 = row.get(6)?;
        let planned_time = if time.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(time) };
        Ok(serde_json::json!({
            "source_type": "event",
            "source_id": id,
            "title": title,
            "planned_time": planned_time,
            "duration_minutes": dur,
            "category": category,
            "color": color,
            "completed": completed == 1,
            "status_extra": if completed == 1 { "done" } else { "planned" },
        }))
    }).map_err(|e| format!("Query error: {}", e))?;
    for it in events_iter.flatten() { items.push(it); }

    // ── 2. Schedules matching today + completion status
    let dow = day_of_week(&date)?;
    let mut s_stmt = conn.prepare(
        "SELECT s.id, s.title, s.category, s.frequency, s.frequency_days, s.time_of_day, s.until_date,
                COALESCE(sc.completed, 0), COALESCE(sc.status, 'planned')
         FROM schedules s
         LEFT JOIN schedule_completions sc ON sc.schedule_id = s.id AND sc.date = ?1
         WHERE s.is_active = 1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let schedules_iter = s_stmt.query_map(rusqlite::params![date.clone()], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, String>(8)?,
        ))
    }).map_err(|e| format!("Query error: {}", e))?;

    for tup in schedules_iter.flatten() {
        let (id, title, category, frequency, frequency_days, time_of_day, until_date, completed, sc_status) = tup;
        // until_date filter
        if let Some(ref ud) = until_date {
            if !ud.is_empty() && date.as_str() > ud.as_str() { continue; }
        }
        // frequency filter
        let matches = match frequency.as_str() {
            "daily" => true,
            "weekly" | "custom" => frequency_days
                .as_deref()
                .map(|fd| fd.split(',').filter_map(|d| d.trim().parse::<u32>().ok()).any(|d| d == dow))
                .unwrap_or(false),
            _ => false,
        };
        if !matches { continue; }

        items.push(serde_json::json!({
            "source_type": "schedule",
            "source_id": id,
            "title": title,
            "planned_time": time_of_day,
            "duration_minutes": serde_json::Value::Null,
            "category": category,
            "color": "#3b82f6",
            "completed": completed == 1,
            "status_extra": sc_status,
        }));
    }

    // ── 3. Notes tasks with due_date = date
    let mut n_stmt = conn.prepare(
        "SELECT id, title, status FROM notes
         WHERE archived = 0 AND status IN ('task', 'done', 'skipped') AND due_date = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let notes_iter = n_stmt.query_map(rusqlite::params![date], |row| {
        let id: i64 = row.get(0)?;
        let title: String = row.get(1)?;
        let status: String = row.get(2)?;
        Ok(serde_json::json!({
            "source_type": "note",
            "source_id": id,
            "title": title,
            "planned_time": serde_json::Value::Null,
            "duration_minutes": serde_json::Value::Null,
            "category": "task",
            "color": "#9ca3af",
            "completed": status == "done",
            "status_extra": status,
        }))
    }).map_err(|e| format!("Query error: {}", e))?;
    for it in notes_iter.flatten() { items.push(it); }

    // ── 4. Merge actual blocks from timeline_blocks (by source_type, source_id)
    let mut b_stmt = conn.prepare(
        "SELECT id, source_type, source_id, start_time, end_time, duration_minutes, is_active
         FROM timeline_blocks
         WHERE date = ?1 AND source_type IS NOT NULL AND source_id IS NOT NULL"
    ).map_err(|e| format!("DB error: {}", e))?;
    let blocks_iter = b_stmt.query_map(rusqlite::params![date], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    }).map_err(|e| format!("Query error: {}", e))?;
    let blocks: Vec<(i64, String, i64, String, String, i64, i64)> = blocks_iter.flatten().collect();

    for it in items.iter_mut() {
        let st = it.get("source_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let sid = it.get("source_id").and_then(|v| v.as_i64()).unwrap_or(0);
        for (block_id, bst, bsid, start, end, dur, active) in &blocks {
            if bst == &st && *bsid == sid {
                if let Some(obj) = it.as_object_mut() {
                    obj.insert("block_id".into(), serde_json::json!(block_id));
                    obj.insert("actual_start".into(), serde_json::json!(start));
                    obj.insert("is_active".into(), serde_json::json!(*active == 1));
                    if *active == 1 {
                        obj.insert("actual_end".into(), serde_json::Value::Null);
                        obj.insert("actual_duration".into(), serde_json::Value::Null);
                    } else {
                        obj.insert("actual_end".into(), serde_json::json!(end));
                        obj.insert("actual_duration".into(), serde_json::json!(dur));
                    }
                }
                break;
            }
        }
    }

    Ok(items)
}

#[tauri::command]
pub fn start_task_block(
    source_type: String,
    source_id: i64,
    type_id: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let now_hm = now.format("%H:%M").to_string();

    // Close any currently active block (across all dates)
    if let Ok((bid, bdate, bstart)) = conn.query_row::<(i64, String, String), _, _>(
        "SELECT id, date, start_time FROM timeline_blocks WHERE is_active = 1 LIMIT 1",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ) {
        let close_end = if bdate == date { now_hm.clone() } else { "23:59".to_string() };
        let dur = crate::commands_timeline::calc_duration(&bstart, &close_end);
        conn.execute(
            "UPDATE timeline_blocks SET is_active=0, end_time=?1, duration_minutes=?2 WHERE id=?3",
            rusqlite::params![close_end, dur, bid],
        ).ok();
    }

    // Resolve type_id (default = "Запланировано" system type)
    let resolved_type_id = match type_id {
        Some(t) => t,
        None => conn.query_row(
            "SELECT id FROM timeline_activity_types WHERE name='Запланировано' AND is_system=1 LIMIT 1",
            [], |r| r.get::<_, i64>(0),
        ).map_err(|e| format!("Default type missing: {}", e))?,
    };

    // Insert new active block (end_time sentinel '--:--' since column is NOT NULL)
    conn.execute(
        "INSERT INTO timeline_blocks (type_id, date, start_time, end_time, duration_minutes, source, notes, is_active, source_type, source_id)
         VALUES (?1, ?2, ?3, '--:--', 0, 'task', '', 1, ?4, ?5)",
        rusqlite::params![resolved_type_id, date, now_hm, source_type, source_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn complete_task_block(
    block_id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let now_hm = now.format("%H:%M").to_string();
    let now_rfc = now.to_rfc3339();
    let date_today = now.format("%Y-%m-%d").to_string();

    let (start_time, source_type, source_id, block_date): (String, Option<String>, Option<i64>, String) = conn.query_row(
        "SELECT start_time, source_type, source_id, date FROM timeline_blocks WHERE id=?1",
        rusqlite::params![block_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    ).map_err(|e| format!("Block not found: {}", e))?;

    let close_end = if block_date == date_today { now_hm } else { "23:59".to_string() };
    let dur = crate::commands_timeline::calc_duration(&start_time, &close_end);
    conn.execute(
        "UPDATE timeline_blocks SET is_active=0, end_time=?1, duration_minutes=?2 WHERE id=?3",
        rusqlite::params![close_end, dur, block_id],
    ).map_err(|e| format!("DB error: {}", e))?;

    // Auto-mark source as done
    if let (Some(st), Some(sid)) = (source_type, source_id) {
        match st.as_str() {
            "event" => {
                conn.execute("UPDATE events SET completed=1 WHERE id=?1", rusqlite::params![sid]).ok();
            }
            "schedule" => {
                conn.execute(
                    "INSERT INTO schedule_completions (schedule_id, date, completed, completed_at, status)
                     VALUES (?1, ?2, 1, ?3, 'done')
                     ON CONFLICT(schedule_id, date) DO UPDATE SET completed=1, completed_at=?3, status='done'",
                    rusqlite::params![sid, block_date, now_rfc],
                ).ok();
            }
            "note" => {
                conn.execute(
                    "UPDATE notes SET status='done', updated_at=?1 WHERE id=?2",
                    rusqlite::params![now_rfc, sid],
                ).ok();
            }
            _ => {}
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_active_block(db: tauri::State<'_, HanniDb>) -> Result<Option<serde_json::Value>, String> {
    let conn = db.conn();
    let row = conn.query_row(
        "SELECT b.id, b.date, b.start_time, b.source_type, b.source_id, t.name, t.color, t.icon
         FROM timeline_blocks b JOIN timeline_activity_types t ON t.id = b.type_id
         WHERE b.is_active = 1 LIMIT 1",
        [], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?,
                "date": r.get::<_, String>(1)?,
                "start_time": r.get::<_, String>(2)?,
                "source_type": r.get::<_, Option<String>>(3)?,
                "source_id": r.get::<_, Option<i64>>(4)?,
                "type_name": r.get::<_, String>(5)?,
                "type_color": r.get::<_, String>(6)?,
                "type_icon": r.get::<_, String>(7)?,
            }))
        }
    ).ok();
    Ok(row)
}

// Sweep on app startup: close orphan active blocks from previous days.
pub fn auto_close_orphan_blocks(conn: &rusqlite::Connection) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "UPDATE timeline_blocks
         SET is_active=0, end_time='23:59',
             duration_minutes = (
                 (CAST(SUBSTR('23:59',1,2) AS INTEGER) * 60 + CAST(SUBSTR('23:59',4,2) AS INTEGER)) -
                 (CAST(SUBSTR(start_time,1,2) AS INTEGER) * 60 + CAST(SUBSTR(start_time,4,2) AS INTEGER))
             )
         WHERE is_active = 1 AND date < ?1",
        rusqlite::params![today],
    ).ok();
}
