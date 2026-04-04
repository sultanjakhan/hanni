// timeline_afk.rs — AFK sync: convert activity_snapshots to timeline_blocks
use crate::types::HanniDb;

#[tauri::command]
pub fn sync_afk_blocks(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let afk_id: i64 = conn.query_row(
        "SELECT id FROM timeline_activity_types WHERE name='АФК' AND is_system=1",
        [], |r| r.get(0)
    ).map_err(|_| "AFK type not found".to_string())?;
    conn.execute(
        "DELETE FROM timeline_blocks WHERE date=?1 AND source='hanni_afk'",
        rusqlite::params![date]
    ).ok();
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
                count += insert_afk_block(&conn, afk_id, &date, &start, &end);
            }
        }
    }
    if let Some(start) = afk_start {
        if let Some(last_ts) = snaps.last().map(|s| &s.0) {
            if let Some(end) = extract_time(last_ts) {
                count += insert_afk_block(&conn, afk_id, &date, &start, &end);
            }
        }
    }
    Ok(count)
}

fn insert_afk_block(conn: &rusqlite::Connection, afk_id: i64, date: &str, start: &str, end: &str) -> i64 {
    let s = snap_to_slot(start);
    let e = snap_to_slot(end);
    if s == e { return 0; }
    let dur = crate::commands_timeline::calc_duration(&s, &e);
    conn.execute(
        "INSERT INTO timeline_blocks (type_id,date,start_time,end_time,duration_minutes,source) VALUES (?1,?2,?3,?4,?5,'hanni_afk')",
        rusqlite::params![afk_id, date, s, e, dur]
    ).ok();
    1
}

fn extract_time(ts: &str) -> Option<String> {
    if let Some(t_pos) = ts.find('T').or_else(|| ts.find(' ')) {
        let rest = &ts[t_pos + 1..];
        if rest.len() >= 5 { return Some(rest[..5].to_string()); }
    }
    None
}

fn snap_to_slot(time: &str) -> String {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() >= 2 {
        let h = parts[0].parse::<u32>().unwrap_or(0);
        let m = parts[1].parse::<u32>().unwrap_or(0);
        format!("{:02}:{:02}", h, if m < 30 { 0 } else { 30 })
    } else {
        time.to_string()
    }
}
