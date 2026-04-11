// timeline_health.rs — Sync sleep_sessions + health_log → timeline_blocks
use crate::types::HanniDb;

/// Sync sleep sessions and exercise into timeline blocks for a given date.
/// Called alongside sync_timeline_auto when Timeline tab opens.
#[tauri::command]
pub fn sync_health_to_timeline(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let mut count = 0i64;

    // Get system type IDs
    let sleep_id = get_type_id(&conn, "Сон")?;
    let sport_id = get_type_id(&conn, "Спорт")?;

    // Remove old auto_health blocks for this date
    conn.execute(
        "DELETE FROM timeline_blocks WHERE date=?1 AND source='auto_health'",
        rusqlite::params![date],
    ).ok();

    // Sync sleep sessions → timeline blocks
    count += sync_sleep(&conn, &date, sleep_id);

    // Sync exercise sessions → timeline blocks
    count += sync_exercise(&conn, &date, sport_id);

    Ok(count)
}

fn sync_sleep(conn: &rusqlite::Connection, date: &str, type_id: i64) -> i64 {
    let mut stmt = match conn.prepare(
        "SELECT start_time, end_time, duration_minutes FROM sleep_sessions
         WHERE date=?1 AND source='health_connect'"
    ) { Ok(s) => s, Err(_) => return 0 };

    let sessions: Vec<(String, String, i64)> = stmt.query_map(
        rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    ).map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_default();

    let mut count = 0i64;
    for (start, end, dur) in sessions {
        let s = normalize_time(&start);
        let e = normalize_time(&end);
        if s == e || dur < 5 { continue; }
        conn.execute(
            "INSERT INTO timeline_blocks (type_id,date,start_time,end_time,duration_minutes,source,notes)
             VALUES (?1,?2,?3,?4,?5,'auto_health','Samsung Health')",
            rusqlite::params![type_id, date, s, e, dur],
        ).ok();
        count += 1;
    }
    count
}

fn sync_exercise(conn: &rusqlite::Connection, date: &str, type_id: i64) -> i64 {
    // Exercise is stored in health_log as type='exercise', notes='running: Morning jog'
    let mut stmt = match conn.prepare(
        "SELECT value, notes FROM health_log WHERE date=?1 AND type='exercise'"
    ) { Ok(s) => s, Err(_) => return 0 };

    let exercises: Vec<(f64, String)> = stmt.query_map(
        rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?))
    ).map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_default();

    let mut count = 0i64;
    for (dur_min, notes) in exercises {
        if dur_min < 5.0 { continue; }
        // We don't have exact times for exercise from health_log,
        // so place it at a default slot (12:00) with the known duration
        let start = "12:00".to_string();
        let end_min = 12 * 60 + dur_min as i64;
        let end = format!("{:02}:{:02}", end_min / 60, end_min % 60);
        conn.execute(
            "INSERT INTO timeline_blocks (type_id,date,start_time,end_time,duration_minutes,source,notes)
             VALUES (?1,?2,?3,?4,?5,'auto_health',?6)",
            rusqlite::params![type_id, date, start, end, dur_min as i64, notes],
        ).ok();
        count += 1;
    }
    count
}

fn get_type_id(conn: &rusqlite::Connection, name: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT id FROM timeline_activity_types WHERE name=?1 AND is_system=1",
        rusqlite::params![name], |r| r.get(0),
    ).map_err(|_| format!("Activity type '{}' not found", name))
}

/// Normalize time: HH:MM from various formats (ISO, HH:MM, etc.)
fn normalize_time(t: &str) -> String {
    if t.len() == 5 && t.contains(':') { return t.to_string(); }
    if let Some(pos) = t.find('T') {
        let rest = &t[pos + 1..];
        if rest.len() >= 5 { return rest[..5].to_string(); }
    }
    if t.len() >= 5 { t[..5].to_string() } else { t.to_string() }
}
