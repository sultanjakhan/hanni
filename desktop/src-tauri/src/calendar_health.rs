// calendar_health.rs — Sync sleep_sessions + exercise → events (Calendar Day-view).

/// Idempotent dedup for auto_health events: keeps MIN(id) per
/// (date, title, time, duration_minutes). Mirrors the startup migration
/// in db.rs so a long-running Mac dev instance can clean accumulated
/// duplicates without a restart — JS calls this on Calendar refresh.
#[tauri::command]
pub fn dedup_auto_health_events(db: tauri::State<'_, crate::types::HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let before: i64 = conn.query_row(
        "SELECT count(*) FROM events WHERE source='auto_health'", [], |r| r.get(0)
    ).map_err(|e| format!("count: {e}"))?;
    conn.execute(
        "DELETE FROM events WHERE source='auto_health' AND id NOT IN (
            SELECT MIN(id) FROM events WHERE source='auto_health'
            GROUP BY date, title, time, duration_minutes
        )", [],
    ).map_err(|e| format!("delete: {e}"))?;
    let after: i64 = conn.query_row(
        "SELECT count(*) FROM events WHERE source='auto_health'", [], |r| r.get(0)
    ).map_err(|e| format!("count: {e}"))?;
    Ok(before - after)
}

// Uses upsert by content-derived external_id so re-sync doesn't churn auto-
// increment ids — without that LAN-sync ends up with stale tombstones and
// the Mac accumulates duplicates of every sleep/walk.

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

/// UPDATE the existing auto_health event for (date, title, time), or INSERT
/// a new one if none exists. Looking up by content rather than external_id
/// is what keeps the row stable: prior versions of this code used different
/// external_id formats, so an external_id lookup would have missed and we'd
/// have churned out new auto-increment ids on every sync — exactly the
/// thing that caused the Mac duplicates to begin with.
fn upsert_event(
    conn: &rusqlite::Connection,
    external_id: &str, title: &str, desc: &str,
    date: &str, time: &str, dur_min: i64, color: &str, now: &str,
) -> bool {
    let updated = conn.execute(
        "UPDATE events
            SET description=?1, duration_minutes=?2, color=?3,
                external_id=?4, updated_at=?5
          WHERE date=?6 AND title=?7 AND time=?8 AND source='auto_health'",
        rusqlite::params![desc, dur_min, color, external_id, now, date, title, time],
    ).unwrap_or(0);
    if updated > 0 { return true; }
    conn.execute(
        "INSERT INTO events (title, description, date, time, duration_minutes,
            category, color, source, external_id, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,'health',?6,'auto_health',?7,?8,?8)",
        rusqlite::params![title, desc, date, time, dur_min, color, external_id, now],
    ).is_ok()
}

/// Sync sleep + exercise for a given date into the events table so they
/// appear in Calendar Day-view. Upserts by content-derived external_id so
/// re-syncs don't create duplicate rows; auto_health rows for the date
/// whose external_id isn't produced by the current import get deleted
/// (handles a Watch session disappearing from Health Connect).
#[tauri::command]
pub fn sync_health_to_calendar(date: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let mut count = 0i64;
    let mut wanted: Vec<String> = Vec::new();

    // Collapse any historical duplicates left over from the previous
    // DELETE+INSERT scheme — idempotent, fast, runs only for this date.
    let _ = conn.execute(
        "DELETE FROM events WHERE source='auto_health' AND date=?1
         AND id NOT IN (
           SELECT min(id) FROM events WHERE source='auto_health' AND date=?1
           GROUP BY title, time, duration_minutes
         )",
        rusqlite::params![date],
    );

    // Sleep sessions → events
    if let Ok(mut stmt) = conn.prepare(
        "SELECT start_time, duration_minutes FROM sleep_sessions
         WHERE date=?1 AND source='health_connect'"
    ) {
        let sessions: Vec<(String, i64)> = stmt.query_map(
            rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?))
        ).map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_default();
        for (start, dur_min) in sessions {
            if dur_min < 5 { continue; }
            let time = normalize_time(&start);
            // Content-derived id: same sleep produces the same external_id
            // across re-imports → upsert hits the existing row.
            let ext_id = format!("sleep:{date}:{time}:{dur_min}");
            wanted.push(ext_id.clone());
            // Blue — visually distinct from the green walking blocks and the
            // ~purple "scheduled" tone elsewhere; matches user request.
            if upsert_event(&conn, &ext_id, "Сон", "", &date, &time, dur_min, "#3b82f6", &now) {
                count += 1;
            }
        }
    }

    // Exercise (walking/running/etc) → events. Uses the per-session start
    // time persisted by import_exercise; only falls back to a fanned-out
    // 12:00 slot for rows imported before the migration added the column.
    if let Ok(mut stmt) = conn.prepare(
        "SELECT value, notes, COALESCE(start_time,'') FROM health_log
         WHERE date=?1 AND type='exercise' ORDER BY start_time, rowid"
    ) {
        let rows: Vec<(f64, String, String)> = stmt.query_map(
            rusqlite::params![date], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).map(|rs| rs.filter_map(|r| r.ok()).collect()).unwrap_or_default();
        let mut slot = 12 * 60i64; // fallback start at 12:00
        let mut idx = 0;
        for (dur, notes, st) in rows {
            let dur_min = dur as i64;
            if dur_min < 5 { continue; }
            let etype = notes.split(':').next().unwrap_or("").trim();
            let detail = notes.splitn(2, ':').nth(1).unwrap_or("").trim();
            let title = exercise_title(etype);
            let time = if st.len() >= 5 && st.chars().nth(2) == Some(':') {
                st[..5].to_string()
            } else {
                let t = format!("{:02}:{:02}", slot / 60, slot % 60);
                slot += 1;
                t
            };
            // Content-derived id: stable across re-imports for the same
            // walk on the same date in the same slot.
            let ext_id = format!("exercise:{date}:{idx}:{dur_min}");
            idx += 1;
            wanted.push(ext_id.clone());
            if upsert_event(&conn, &ext_id, title, detail, &date, &time, dur_min, "#34d399", &now) {
                count += 1;
            }
        }
    }

    // Drop auto_health rows for this date that don't match anything in the
    // current import — handles HC sessions that vanished from the source.
    if wanted.is_empty() {
        let _ = conn.execute(
            "DELETE FROM events WHERE date=?1 AND source='auto_health'",
            rusqlite::params![date],
        );
    } else {
        let placeholders = (0..wanted.len()).map(|i| format!("?{}", i + 2)).collect::<Vec<_>>().join(",");
        let sql = format!(
            "DELETE FROM events WHERE date=?1 AND source='auto_health'
             AND (external_id IS NULL OR external_id NOT IN ({placeholders}))"
        );
        let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(1 + wanted.len());
        params.push(date.clone().into());
        for w in &wanted { params.push(w.clone().into()); }
        let _ = conn.execute(&sql, rusqlite::params_from_iter(params));
    }

    Ok(count)
}
