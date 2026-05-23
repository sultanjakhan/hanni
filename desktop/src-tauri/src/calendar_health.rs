// calendar_health.rs — Sync sleep_sessions + exercise → events (Calendar Day-view)
use crate::types::HanniDb;
use crate::timeline_health::normalize_time;

/// Russian label + emoji for a Health Connect exercise type. Used as event
/// title when syncing exercise from health_log into the events table.
fn exercise_title(etype: &str) -> &'static str {
    match etype {
        "walking" => "🚶 Прогулка",
        "running" => "🏃 Пробежка",
        "biking" | "cycling" => "🚴 Велосипед",
        "swimming" => "🏊 Плавание",
        "strength_training" | "weightlifting" => "🏋 Силовая",
        "yoga" => "🧘 Йога",
        "hiking" => "🥾 Поход",
        _ => "💪 Тренировка",
    }
}

/// Sync sleep + exercise for a given date into the events table so they
/// appear in Calendar Day-view alongside manual events. Idempotent: removes
/// prior auto_health rows for the date before inserting current ones.
#[tauri::command]
pub fn sync_health_to_calendar(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();

    conn.execute(
        "DELETE FROM events WHERE date=?1 AND source='auto_health'",
        rusqlite::params![date],
    ).ok();

    let mut count = 0i64;
    let now = chrono::Local::now().to_rfc3339();

    // Sleep sessions → events
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, start_time, duration_minutes FROM sleep_sessions
         WHERE date=?1 AND source='health_connect'"
    ) {
        let sessions: Vec<(i64, String, i64)> = stmt.query_map(
            rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_default();
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
    }

    // Exercise (walking/running/etc) → events. We don't have a real start
    // time in health_log yet, so we fan them out from 12:00 in 1-minute
    // increments so multiple walks on the same day don't collide on the same
    // visual slot. notes format from import_exercise: "{etype}: {title}".
    if let Ok(mut stmt) = conn.prepare(
        "SELECT rowid, value, notes FROM health_log
         WHERE date=?1 AND type='exercise' ORDER BY rowid"
    ) {
        let rows: Vec<(i64, f64, String)> = stmt.query_map(
            rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).map(|rs| rs.filter_map(|r| r.ok()).collect()).unwrap_or_default();
        let mut slot = 12 * 60i64; // start at 12:00
        for (rid, dur, notes) in rows {
            let dur_min = dur as i64;
            if dur_min < 5 { continue; }
            let etype = notes.split(':').next().unwrap_or("").trim();
            let detail = notes.splitn(2, ':').nth(1).unwrap_or("").trim();
            let title = exercise_title(etype);
            let time = format!("{:02}:{:02}", slot / 60, slot % 60);
            slot += 1;
            let ext_id = format!("exercise:{rid}");
            let res = conn.execute(
                "INSERT INTO events (title, description, date, time, duration_minutes,
                    category, color, source, external_id, created_at)
                 VALUES (?1,?2,?3,?4,?5,'health','#34d399','auto_health',?6,?7)",
                rusqlite::params![title, detail, date, time, dur_min, ext_id, now],
            );
            if res.is_ok() { count += 1; }
        }
    }

    Ok(count)
}
