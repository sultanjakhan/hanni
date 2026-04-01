// timeline_stats.rs — Timeline stats, goals CRUD, AFK sync
use crate::types::HanniDb;

// ── Goals CRUD ──

#[tauri::command]
pub fn create_timeline_goal(type_id: i64, operator: String, target_minutes: i64, period: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO timeline_goals (type_id, operator, target_minutes, period) VALUES (?1,?2,?3,?4)",
        rusqlite::params![type_id, operator, target_minutes, period],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_timeline_goals(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT g.id, g.type_id, g.operator, g.target_minutes, g.period, g.active, t.name, t.color, t.icon
         FROM timeline_goals g JOIN timeline_activity_types t ON t.id = g.type_id ORDER BY t.sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type_id": row.get::<_, i64>(1)?,
            "operator": row.get::<_, String>(2)?,
            "target_minutes": row.get::<_, i64>(3)?,
            "period": row.get::<_, String>(4)?,
            "active": row.get::<_, i64>(5)? == 1,
            "type_name": row.get::<_, String>(6)?,
            "type_color": row.get::<_, String>(7)?,
            "type_icon": row.get::<_, String>(8)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_timeline_goal(id: i64, operator: Option<String>, target_minutes: Option<i64>, active: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = operator { conn.execute("UPDATE timeline_goals SET operator=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = target_minutes { conn.execute("UPDATE timeline_goals SET target_minutes=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = active { conn.execute("UPDATE timeline_goals SET active=?1 WHERE id=?2", rusqlite::params![v as i64, id]).ok(); }
    Ok(())
}

#[tauri::command]
pub fn delete_timeline_goal(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM timeline_goals WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Day Stats ──

#[tauri::command]
pub fn get_timeline_day_stats(date: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.icon, COALESCE(SUM(b.duration_minutes),0) as total
         FROM timeline_activity_types t
         LEFT JOIN timeline_blocks b ON b.type_id = t.id AND b.date=?1
         GROUP BY t.id ORDER BY t.sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let per_type: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![date], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "color": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "minutes": row.get::<_, i64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_minutes),0) FROM timeline_blocks WHERE date=?1",
        rusqlite::params![date], |r| r.get(0)
    ).unwrap_or(0);
    Ok(serde_json::json!({ "date": date, "total_minutes": total, "per_type": per_type }))
}

// ── Range Stats (week/month with comparison) ──

#[tauri::command]
pub fn get_timeline_range_stats(start_date: String, end_date: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    // Current period averages
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.icon, COALESCE(SUM(b.duration_minutes),0) as total
         FROM timeline_activity_types t
         LEFT JOIN timeline_blocks b ON b.type_id = t.id AND b.date BETWEEN ?1 AND ?2
         GROUP BY t.id ORDER BY t.sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let current: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![start_date, end_date], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "color": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "total_minutes": row.get::<_, i64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    // Calculate days in range for averages
    let days = {
        let s = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d").ok();
        let e = chrono::NaiveDate::parse_from_str(&end_date, "%Y-%m-%d").ok();
        match (s, e) { (Some(a), Some(b)) => (b - a).num_days().max(1), _ => 7 }
    };
    // Previous period (same length, shifted back)
    let prev_start = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
        .map(|d| (d - chrono::Duration::days(days)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let prev_end = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
        .map(|d| (d - chrono::Duration::days(1)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let mut stmt2 = conn.prepare(
        "SELECT t.id, COALESCE(SUM(b.duration_minutes),0)
         FROM timeline_activity_types t
         LEFT JOIN timeline_blocks b ON b.type_id = t.id AND b.date BETWEEN ?1 AND ?2
         GROUP BY t.id ORDER BY t.sort_order"
    ).map_err(|e| format!("DB error: {}", e))?;
    let previous: Vec<(i64, i64)> = stmt2.query_map(rusqlite::params![prev_start, prev_end], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    let prev_map: std::collections::HashMap<i64, i64> = previous.into_iter().collect();

    Ok(serde_json::json!({
        "start_date": start_date,
        "end_date": end_date,
        "days": days,
        "current": current,
        "previous_totals": prev_map,
    }))
}

// ── AFK Sync (convert activity_snapshots to timeline_blocks) ──

#[tauri::command]
pub fn sync_afk_blocks(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    // Find AFK type id
    let afk_id: i64 = conn.query_row(
        "SELECT id FROM timeline_activity_types WHERE name='АФК' AND is_system=1",
        [], |r| r.get(0)
    ).map_err(|_| "AFK type not found".to_string())?;
    // Remove old auto-detected AFK blocks for this date
    conn.execute(
        "DELETE FROM timeline_blocks WHERE date=?1 AND source='hanni_afk'",
        rusqlite::params![date]
    ).ok();
    // Read activity snapshots for the date
    let mut stmt = conn.prepare(
        "SELECT captured_at, idle_secs, screen_locked FROM activity_snapshots
         WHERE captured_at LIKE ?1 ORDER BY captured_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", date);
    let snaps: Vec<(String, f64, i64)> = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1).unwrap_or(0.0), row.get::<_, i64>(2).unwrap_or(0)))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    let mut count = 0i64;
    let mut afk_start: Option<String> = None;
    for (ts, idle, locked) in &snaps {
        let is_afk = *idle > 300.0 || *locked == 1;
        if is_afk && afk_start.is_none() {
            afk_start = extract_time(ts);
        } else if !is_afk && afk_start.is_some() {
            if let (Some(start), Some(end)) = (afk_start.take(), extract_time(ts)) {
                let s = snap_to_slot(&start);
                let e = snap_to_slot(&end);
                if s != e {
                    let dur = crate::commands_timeline::calc_duration(&s, &e);
                    conn.execute(
                        "INSERT INTO timeline_blocks (type_id,date,start_time,end_time,duration_minutes,source) VALUES (?1,?2,?3,?4,?5,'hanni_afk')",
                        rusqlite::params![afk_id, date, s, e, dur]
                    ).ok();
                    count += 1;
                }
            }
        }
    }
    // Close trailing AFK block
    if let Some(start) = afk_start {
        if let Some(last_ts) = snaps.last().map(|s| &s.0) {
            if let Some(end) = extract_time(last_ts) {
                let s = snap_to_slot(&start);
                let e = snap_to_slot(&end);
                if s != e {
                    let dur = crate::commands_timeline::calc_duration(&s, &e);
                    conn.execute(
                        "INSERT INTO timeline_blocks (type_id,date,start_time,end_time,duration_minutes,source) VALUES (?1,?2,?3,?4,?5,'hanni_afk')",
                        rusqlite::params![afk_id, date, s, e, dur]
                    ).ok();
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

fn extract_time(ts: &str) -> Option<String> {
    // From RFC3339 or "YYYY-MM-DD HH:MM:SS" extract "HH:MM"
    if let Some(t_pos) = ts.find('T').or_else(|| ts.find(' ')) {
        let rest = &ts[t_pos + 1..];
        if rest.len() >= 5 { return Some(rest[..5].to_string()); }
    }
    None
}

fn snap_to_slot(time: &str) -> String {
    // Round to nearest 30-min slot: 08:17 → 08:00, 08:45 → 08:30
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() >= 2 {
        let h = parts[0].parse::<u32>().unwrap_or(0);
        let m = parts[1].parse::<u32>().unwrap_or(0);
        let snapped = if m < 30 { 0 } else { 30 };
        format!("{:02}:{:02}", h, snapped)
    } else {
        time.to_string()
    }
}
