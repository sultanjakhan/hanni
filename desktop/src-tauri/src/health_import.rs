// health_import.rs — Import health data from Health Connect + analytics
use crate::types::HanniDb;
use serde::Serialize;
use tauri::{Runtime, State};
#[cfg(target_os = "android")]
use tauri::Manager;

/// Import all health data from Health Connect (Android only).
/// On desktop, data arrives via CR-SQLite sync — this is a no-op.
#[tauri::command]
pub async fn import_health_connect_all<R: Runtime>(
    db: State<'_, HanniDb>,
    app: tauri::AppHandle<R>,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        use crate::health_connect_plugin::HealthConnectHandle;
        let handle = app.state::<HealthConnectHandle<R>>();
        let mut result = serde_json::json!({"sleep": 0, "steps": 0, "heart_rate": 0, "exercise": 0});
        // Sleep
        if let Ok(resp) = handle.0.run_mobile_plugin::<serde_json::Value>("readSleep", &()) {
            if let Some(sessions) = resp.get("sessions").and_then(|v| v.as_array()) {
                let count = import_sleep_sessions(&db, sessions);
                result["sleep"] = serde_json::json!(count);
            }
        }
        // Steps
        if let Ok(resp) = handle.0.run_mobile_plugin::<serde_json::Value>("readSteps", &()) {
            if let Some(days) = resp.get("days").and_then(|v| v.as_array()) {
                let count = import_steps(&db, days);
                result["steps"] = serde_json::json!(count);
            }
        }
        // Heart rate
        if let Ok(resp) = handle.0.run_mobile_plugin::<serde_json::Value>("readHeartRate", &()) {
            if let Some(samples) = resp.get("samples").and_then(|v| v.as_array()) {
                let count = import_heart_rate(&db, samples);
                result["heart_rate"] = serde_json::json!(count);
            }
        }
        // Exercise
        if let Ok(resp) = handle.0.run_mobile_plugin::<serde_json::Value>("readExercise", &()) {
            if let Some(sessions) = resp.get("sessions").and_then(|v| v.as_array()) {
                let count = import_exercise(&db, sessions);
                result["exercise"] = serde_json::json!(count);
            }
        }
        Ok(result)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (db, app);
        Err("Health Connect import is only available on Android".into())
    }
}

// Samsung Health writes one night of sleep to Health Connect as several
// separate SleepSessionRecords (split by wake-ups, plus naps). Segments less
// than this many minutes apart are treated as one sleep.
#[cfg(target_os = "android")]
const SLEEP_MERGE_GAP_MINUTES: i64 = 180;

/// One sleep, possibly assembled from several Health Connect segments.
#[cfg(target_os = "android")]
struct SleepNight {
    date: String,
    start_time: String,
    end_time: String,
    start: chrono::DateTime<chrono::FixedOffset>,
    end: chrono::DateTime<chrono::FixedOffset>,
    stages: Vec<serde_json::Value>,
}

/// Sort raw HC sleep segments by start instant and merge adjacent ones whose
/// gap is below the threshold (or that overlap) into single nights.
#[cfg(target_os = "android")]
fn merge_sleep_segments(sessions: &[serde_json::Value]) -> Vec<SleepNight> {
    let mut segs: Vec<SleepNight> = sessions.iter().filter_map(|s| {
        let start = chrono::DateTime::parse_from_rfc3339(s["start_iso"].as_str()?).ok()?;
        let end = chrono::DateTime::parse_from_rfc3339(s["end_iso"].as_str()?).ok()?;
        Some(SleepNight {
            date: s["date"].as_str().unwrap_or_default().to_string(),
            start_time: s["start_time"].as_str().unwrap_or_default().to_string(),
            end_time: s["end_time"].as_str().unwrap_or_default().to_string(),
            start, end,
            stages: s["stages"].as_array().cloned().unwrap_or_default(),
        })
    }).collect();
    segs.sort_by_key(|s| s.start);

    let mut nights: Vec<SleepNight> = Vec::new();
    for seg in segs {
        if let Some(last) = nights.last_mut() {
            let gap = (seg.start - last.end).num_minutes();
            if gap < SLEEP_MERGE_GAP_MINUTES {
                if seg.end > last.end {
                    last.end = seg.end;
                    last.end_time = seg.end_time;
                }
                last.stages.extend(seg.stages);
                continue;
            }
        }
        nights.push(seg);
    }
    nights
}

#[cfg(target_os = "android")]
fn import_sleep_sessions(db: &HanniDb, sessions: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let mut count = 0;
    for night in merge_sleep_segments(sessions) {
        // Wall-clock span so a fragmented night shows as one continuous block.
        let dur = (night.end - night.start).num_minutes().max(0);

        // Idempotency: skip if a session with same (date, start_time, source)
        // already exists. There's no UNIQUE constraint on sleep_sessions, so
        // re-importing would otherwise create duplicates every visibilitychange.
        let existing: Option<i64> = conn.query_row(
            "SELECT id FROM sleep_sessions WHERE date=?1 AND start_time=?2 AND source='health_connect'",
            rusqlite::params![night.date, night.start_time], |r| r.get(0),
        ).ok();

        let sid = if let Some(id) = existing {
            // Refresh stages so any new HC data shows up.
            let _ = conn.execute("DELETE FROM sleep_stages WHERE session_id=?1", rusqlite::params![id]);
            let _ = conn.execute(
                "UPDATE sleep_sessions SET end_time=?1, duration_minutes=?2 WHERE id=?3",
                rusqlite::params![night.end_time, dur, id],
            );
            id
        } else {
            if conn.execute(
                "INSERT INTO sleep_sessions (date, start_time, end_time, duration_minutes, source) VALUES (?1,?2,?3,?4,'health_connect')",
                rusqlite::params![night.date, night.start_time, night.end_time, dur],
            ).is_err() { continue; }
            count += 1;
            conn.last_insert_rowid()
        };

        if sid > 0 {
            for st in &night.stages {
                let _ = conn.execute(
                    "INSERT INTO sleep_stages (session_id, start_time, end_time, stage) VALUES (?1,?2,?3,?4)",
                    rusqlite::params![sid, st["start_time"].as_str().unwrap_or(""), st["end_time"].as_str().unwrap_or(""), st["stage"].as_str().unwrap_or("")],
                );
            }
        }
    }
    count
}

#[cfg(target_os = "android")]
fn import_steps(db: &HanniDb, days: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let mut count = 0;
    let now = chrono::Local::now().to_rfc3339();
    for d in days {
        let date = d["date"].as_str().unwrap_or_default();
        let steps = d["steps"].as_f64().unwrap_or(0.0);
        let existing: Option<i64> = conn.query_row(
            "SELECT id FROM health_log WHERE date=?1 AND type='steps'", [date], |r| r.get(0),
        ).ok();
        if let Some(id) = existing {
            let _ = conn.execute("UPDATE health_log SET value=?1 WHERE id=?2", rusqlite::params![steps, id]);
        } else {
            let _ = conn.execute(
                "INSERT INTO health_log (date, type, value, unit, notes, created_at) VALUES (?1,'steps',?2,'steps','',?3)",
                rusqlite::params![date, steps, now],
            );
        }
        count += 1;
    }
    count
}

#[cfg(target_os = "android")]
fn import_heart_rate(db: &HanniDb, samples: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let mut count = 0;
    for s in samples {
        let date = s["date"].as_str().unwrap_or_default();
        let time = s["time"].as_str().unwrap_or_default();
        let bpm = s["bpm"].as_i64().unwrap_or(0);
        if conn.execute(
            "INSERT OR IGNORE INTO heart_rate_samples (date, time, bpm) VALUES (?1,?2,?3)",
            rusqlite::params![date, time, bpm],
        ).is_ok() { count += 1; }
    }
    count
}

#[cfg(target_os = "android")]
fn import_exercise(db: &HanniDb, sessions: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Idempotency: Health Connect returns every session in the window, so clear
    // this batch's dates first — re-importing then can't pile up duplicates.
    let mut dates: Vec<&str> = sessions.iter().filter_map(|s| s["date"].as_str()).collect();
    dates.sort_unstable();
    dates.dedup();
    for &date in &dates {
        let _ = conn.execute(
            "DELETE FROM health_log WHERE type='exercise' AND date=?1",
            rusqlite::params![date],
        );
    }
    let mut count = 0;
    for s in sessions {
        let date = s["date"].as_str().unwrap_or_default();
        let dur = s["duration_minutes"].as_f64().unwrap_or(0.0);
        let etype = s["type"].as_str().unwrap_or("other");
        let title = s["title"].as_str().unwrap_or("");
        // Kotlin readExerciseSessions hands us the per-session start time
        // already formatted "HH:MM" in the local zone — persist it so the
        // Calendar/Timeline syncs can place the block at the real start
        // instead of falling back to a 12:00 default.
        let start_time = s["start_time"].as_str().unwrap_or("");
        let notes = format!("{}: {}", etype, title);
        let _ = conn.execute(
            "INSERT INTO health_log (date, type, value, unit, notes, start_time, created_at)
             VALUES (?1,'exercise',?2,'minutes',?3,?4,?5)",
            rusqlite::params![date, dur, notes, start_time, now],
        );
        count += 1;
    }
    count
}

#[tauri::command]
pub fn get_heart_rate_samples(db: State<'_, HanniDb>, from: String, to: String) -> Vec<serde_json::Value> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT date, time, bpm FROM heart_rate_samples WHERE date >= ?1 AND date <= ?2 ORDER BY date, time"
    ).unwrap();
    stmt.query_map(rusqlite::params![from, to], |row| {
        Ok(serde_json::json!({
            "date": row.get::<_, String>(0)?,
            "time": row.get::<_, String>(1)?,
            "bpm": row.get::<_, i64>(2)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect()
}

#[derive(Serialize)]
pub struct HealthSummary {
    pub avg_sleep_minutes: f64,
    pub avg_steps: f64,
    pub avg_resting_hr: f64,
    pub sleep_sessions: i64,
    pub days_with_steps: i64,
    pub hr_samples: i64,
}

#[tauri::command]
pub fn get_health_summary(db: State<'_, HanniDb>, days: i64) -> HealthSummary {
    let conn = db.conn();
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let avg_sleep: f64 = conn.query_row(
        "SELECT COALESCE(AVG(duration_minutes),0) FROM sleep_sessions WHERE date>=?1", [&since], |r| r.get(0),
    ).unwrap_or(0.0);
    let (avg_steps, days_steps): (f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(value),0), COUNT(*) FROM health_log WHERE type='steps' AND date>=?1", [&since], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0));
    let (avg_hr, hr_count): (f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(bpm),0), COUNT(*) FROM heart_rate_samples WHERE date>=?1", [&since], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0));
    let sleep_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sleep_sessions WHERE date>=?1", [&since], |r| r.get(0),
    ).unwrap_or(0);
    HealthSummary { avg_sleep_minutes: avg_sleep, avg_steps, avg_resting_hr: avg_hr, sleep_sessions: sleep_count, days_with_steps: days_steps, hr_samples: hr_count }
}
