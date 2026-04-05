// timeline_afk.rs — Auto-sync activity_snapshots → timeline_blocks (AFK + Focus)
use crate::types::HanniDb;

/// Sync today's activity_snapshots into timeline blocks for AFK and Focus.
/// Called automatically when Timeline tab opens.
#[tauri::command]
pub fn sync_timeline_auto(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();

    let afk_id = get_type_id(&conn, "АФК")?;
    let focus_id = get_type_id(&conn, "Фокус")?;

    // Delete old auto-generated blocks for this date
    conn.execute(
        "DELETE FROM timeline_blocks WHERE date=?1 AND source IN ('auto_afk','auto_focus')",
        rusqlite::params![date],
    ).ok();

    // Fetch snapshots
    let mut stmt = conn.prepare(
        "SELECT captured_at, idle_secs, screen_locked FROM activity_snapshots
         WHERE captured_at LIKE ?1 ORDER BY captured_at"
    ).map_err(|e| format!("DB: {e}"))?;
    let pattern = format!("{date}%");
    let snaps: Vec<(String, f64, i64)> = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok((row.get(0)?, row.get::<_, f64>(1).unwrap_or(0.0), row.get::<_, i64>(2).unwrap_or(0)))
    }).map_err(|e| format!("Query: {e}"))?.filter_map(|r| r.ok()).collect();

    if snaps.is_empty() { return Ok(0); }

    // Build segments: consecutive AFK or Focus periods
    let mut count = 0i64;
    let mut seg_start: Option<String> = None;
    let mut seg_is_afk: Option<bool> = None;

    for (ts, idle, locked) in &snaps {
        let is_afk = *idle > 300.0 || *locked == 1;

        match seg_is_afk {
            None => {
                seg_start = extract_time(ts);
                seg_is_afk = Some(is_afk);
            }
            Some(prev_afk) if prev_afk != is_afk => {
                // State changed — flush previous segment
                if let (Some(start), Some(end)) = (seg_start.take(), extract_time(ts)) {
                    count += insert_block(&conn, prev_afk, afk_id, focus_id, &date, &start, &end);
                }
                seg_start = extract_time(ts);
                seg_is_afk = Some(is_afk);
            }
            _ => {} // same state, continue
        }
    }
    // Flush last segment
    if let (Some(start), Some(is_afk)) = (seg_start, seg_is_afk) {
        if let Some(end) = snaps.last().and_then(|s| extract_time(&s.0)) {
            count += insert_block(&conn, is_afk, afk_id, focus_id, &date, &start, &end);
        }
    }
    Ok(count)
}

// Keep old command name for compatibility
#[tauri::command]
pub fn sync_afk_blocks(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    sync_timeline_auto(date, db)
}

fn get_type_id(conn: &rusqlite::Connection, name: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT id FROM timeline_activity_types WHERE name=?1 AND is_system=1",
        rusqlite::params![name], |r| r.get(0)
    ).map_err(|_| format!("Type '{name}' not found"))
}

fn insert_block(conn: &rusqlite::Connection, is_afk: bool, afk_id: i64, focus_id: i64, date: &str, start: &str, end: &str) -> i64 {
    let s = snap_to_slot(start);
    let e = snap_to_slot(end);
    if s == e { return 0; }
    let dur = crate::commands_timeline::calc_duration(&s, &e);
    if dur < 5 { return 0; } // skip segments shorter than 5 min
    let (type_id, source) = if is_afk { (afk_id, "auto_afk") } else { (focus_id, "auto_focus") };
    conn.execute(
        "INSERT INTO timeline_blocks (type_id,date,start_time,end_time,duration_minutes,source) VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![type_id, date, s, e, dur, source],
    ).ok();
    1
}

fn extract_time(ts: &str) -> Option<String> {
    let t_pos = ts.find('T').or_else(|| ts.find(' '))?;
    let rest = &ts[t_pos + 1..];
    if rest.len() >= 5 { Some(rest[..5].to_string()) } else { None }
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
