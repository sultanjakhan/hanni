// calendar_health.rs — Sync sleep_sessions → events (Calendar Day-view)
use crate::types::HanniDb;
use crate::timeline_health::normalize_time;

/// Sync sleep sessions for a given date into the events table so they appear
/// in Calendar Day-view alongside manual events. Idempotent: removes prior
/// auto_health rows for the date before inserting current ones.
#[tauri::command]
pub fn sync_health_to_calendar(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();

    conn.execute(
        "DELETE FROM events WHERE date=?1 AND source='auto_health'",
        rusqlite::params![date],
    ).ok();

    let mut stmt = match conn.prepare(
        "SELECT id, start_time, duration_minutes FROM sleep_sessions
         WHERE date=?1 AND source='health_connect'"
    ) { Ok(s) => s, Err(e) => return Err(format!("prepare: {e}")) };

    let sessions: Vec<(i64, String, i64)> = stmt.query_map(
        rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    ).map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_default();

    let now = chrono::Local::now().to_rfc3339();
    let mut count = 0i64;
    for (sid, start, dur_min) in sessions {
        if dur_min < 5 { continue; }
        let time = normalize_time(&start);
        let ext_id = format!("sleep:{sid}");
        let res = conn.execute(
            "INSERT INTO events (title, description, date, time, duration_minutes,
                category, color, source, external_id, created_at)
             VALUES ('Сон','',?1,?2,?3,'health','#a78bfa','auto_health',?4,?5)",
            rusqlite::params![date, time, dur_min, ext_id, now],
        );
        if res.is_ok() { count += 1; }
    }
    Ok(count)
}
