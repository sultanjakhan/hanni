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
        let mut result = serde_json::json!({
            "sleep": 0, "steps": 0, "heart_rate": 0, "exercise": 0,
            "successful_types": [], "errors": {},
        });
        let mut successful = Vec::new();
        let mut errors = serde_json::Map::new();
        // Sleep
        match handle.0.run_mobile_plugin::<serde_json::Value>("readSleep", &()) {
            Ok(resp) => {
                let sessions = resp.get("sessions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                result["sleep"] = serde_json::json!(import_sleep_sessions(&db, &sessions));
                successful.push("sleep");
            }
            Err(e) => { errors.insert("sleep".into(), serde_json::json!(e.to_string())); }
        }
        // Steps
        match handle.0.run_mobile_plugin::<serde_json::Value>("readSteps", &()) {
            Ok(resp) => {
                let days = resp.get("days").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                result["steps"] = serde_json::json!(import_steps(&db, &days));
                successful.push("steps");
            }
            Err(e) => { errors.insert("steps".into(), serde_json::json!(e.to_string())); }
        }
        // Heart rate
        match handle.0.run_mobile_plugin::<serde_json::Value>("readHeartRate", &()) {
            Ok(resp) => {
                let samples = resp.get("samples").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                result["heart_rate"] = serde_json::json!(import_heart_rate(&db, &samples));
                successful.push("heart_rate");
            }
            Err(e) => { errors.insert("heart_rate".into(), serde_json::json!(e.to_string())); }
        }
        // Exercise
        match handle.0.run_mobile_plugin::<serde_json::Value>("readExercise", &()) {
            Ok(resp) => {
                let sessions = resp.get("sessions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                result["exercise"] = serde_json::json!(import_exercise(&db, &sessions));
                successful.push("exercise");
            }
            Err(e) => { errors.insert("exercise".into(), serde_json::json!(e.to_string())); }
        }
        result["successful_types"] = serde_json::json!(successful);
        result["errors"] = serde_json::Value::Object(errors);
        if successful.is_empty() { return Err("No Health Connect data type could be read".into()); }
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
#[cfg(any(target_os = "android", test))]
const SLEEP_MERGE_GAP_MINUTES: i64 = 180;

/// One sleep, possibly assembled from several Health Connect segments.
#[cfg(any(target_os = "android", test))]
struct SleepNight {
    date: String,
    start_time: String,
    end_time: String,
    start: chrono::DateTime<chrono::FixedOffset>,
    end: chrono::DateTime<chrono::FixedOffset>,
    stages: Vec<serde_json::Value>,
    record_id: String,
}

/// Sort raw HC sleep segments by start instant and merge adjacent ones whose
/// gap is below the threshold (or that overlap) into single nights.
#[cfg(any(target_os = "android", test))]
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
            record_id: s["record_id"].as_str().unwrap_or_default().to_string(),
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

#[cfg(any(target_os = "android", test))]
fn import_sleep_sessions(db: &HanniDb, sessions: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let mut count = 0;
    for night in merge_sleep_segments(sessions) {
        // Wall-clock span so a fragmented night shows as one continuous block.
        let dur = (night.end - night.start).num_minutes().max(0);

        // Prefer the HC ID so corrected start/date updates the same row.
        // Natural-key fallback preserves already imported legacy IDs.
        let existing: Option<String> = conn.query_row(
            "SELECT id FROM sleep_sessions
             WHERE (id=?1 AND ?1<>'') OR (date=?2 AND start_time=?3 AND source='health_connect')
             ORDER BY (id=?1) DESC LIMIT 1",
            rusqlite::params![night.record_id, night.date, night.start_time], |r| r.get(0),
        ).ok();

        let sid: String = if let Some(id) = existing {
            // A UNIQUE conflict must leave this session and its stages intact.
            if conn.execute(
                "UPDATE sleep_sessions SET date=?1,start_time=?2,end_time=?3,duration_minutes=?4
                 WHERE id=?5 AND (date IS NOT ?1 OR start_time IS NOT ?2 OR end_time IS NOT ?3 OR duration_minutes IS NOT ?4)",
                rusqlite::params![night.date, night.start_time, night.end_time, dur, &id],
            ).is_err() { continue; }
            id
        } else {
            let new_id = if night.record_id.is_empty() {
                crate::types::new_uuid_v7()
            } else { night.record_id.clone() };
            if conn.execute(
                "INSERT INTO sleep_sessions (id, date, start_time, end_time, duration_minutes, source)
                 VALUES (?1,?2,?3,?4,?5,'health_connect')",
                rusqlite::params![new_id, night.date, night.start_time, night.end_time, dur],
            ).is_err() { continue; }
            count += 1;
            new_id
        };

        reconcile_sleep_stages(&conn, &sid, &night.stages);
    }
    count
}

#[cfg(any(target_os = "android", test))]
fn reconcile_sleep_stages(
    conn: &rusqlite::Connection,
    session_id: &str,
    stages: &[serde_json::Value],
) {
    use std::collections::{HashMap, HashSet};
    let desired: HashSet<(String, String, String)> = stages.iter().filter_map(|st| {
        Some((st["start_time"].as_str()?.to_string(),
              st["end_time"].as_str()?.to_string(),
              st["stage"].as_str()?.to_string()))
    }).collect();
    let existing: Vec<(String, String, String, String)> = conn.prepare(
        "SELECT id,start_time,end_time,stage FROM sleep_stages WHERE session_id=?1"
    ).and_then(|mut stmt| stmt.query_map([session_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    }).map(|rows| rows.filter_map(Result::ok).collect())).unwrap_or_default();
    let mut kept: HashMap<(String, String, String), String> = HashMap::new();
    for (id, start, end, stage) in existing {
        let key = (start, end, stage);
        if !desired.contains(&key) || kept.contains_key(&key) {
            let _ = conn.execute("DELETE FROM sleep_stages WHERE id=?1", [&id]);
        } else {
            kept.insert(key, id);
        }
    }
    for (start, end, stage) in desired {
        let key = (start.clone(), end.clone(), stage.clone());
        if kept.contains_key(&key) { continue; }
        let id = format!("stage:{session_id}:{start}:{end}:{stage}");
        let _ = conn.execute(
            "INSERT OR IGNORE INTO sleep_stages (id,session_id,start_time,end_time,stage)
             VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![id, session_id, start, end, stage],
        );
    }
}

#[cfg(any(target_os = "android", test))]
fn import_steps(db: &HanniDb, days: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let mut count = 0;
    let now = chrono::Local::now().to_rfc3339();
    for d in days {
        let date = d["date"].as_str().unwrap_or_default();
        let steps = d["steps"].as_f64().unwrap_or(0.0);
        let existing: Option<String> = conn.query_row(
            "SELECT id FROM health_log WHERE date=?1 AND type='steps'", [date], |r| r.get(0),
        ).ok();
        if let Some(id) = existing {
            let _ = conn.execute("UPDATE health_log SET value=?1 WHERE id=?2 AND value IS NOT ?1", rusqlite::params![steps, id]);
        } else {
            let new_id = format!("health:steps:{date}");
            let _ = conn.execute(
                "INSERT INTO health_log (id, date, type, value, unit, notes, created_at)
                 VALUES (?1,?2,'steps',?3,'steps','',?4)",
                rusqlite::params![new_id, date, steps, now],
            );
        }
        count += 1;
    }
    count
}

#[cfg(any(target_os = "android", test))]
fn import_heart_rate(db: &HanniDb, samples: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let mut count = 0;
    for s in samples {
        let date = s["date"].as_str().unwrap_or_default();
        let time = s["time"].as_str().unwrap_or_default();
        let bpm = s["bpm"].as_i64().unwrap_or(0);
        let record_id = s["record_id"].as_str().unwrap_or("");
        let sample_index = s["sample_index"].as_i64().unwrap_or(0);
        let new_id = if record_id.is_empty() {
            format!("health:hr:{date}:{time}")
        } else { format!("health:hr:{record_id}:{sample_index}") };
        if conn.execute(
            "INSERT OR IGNORE INTO heart_rate_samples (id, date, time, bpm) VALUES (?1,?2,?3,?4)",
            rusqlite::params![new_id, date, time, bpm],
        ).is_ok() {
            // INSERT OR IGNORE alone would silently discard corrected samples.
            let _ = conn.execute(
                "UPDATE heart_rate_samples SET date=?1,time=?2,bpm=?3
                 WHERE id=?4 AND (date IS NOT ?1 OR time IS NOT ?2 OR bpm IS NOT ?3)",
                rusqlite::params![date, time, bpm, new_id],
            );
            count += 1;
        }
    }
    count
}

#[cfg(any(target_os = "android", test))]
fn import_exercise(db: &HanniDb, sessions: &[serde_json::Value]) -> usize {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Idempotency: Health Connect returns every session in the window each
    // poll. Upsert by (date, start_time, title) so re-imports refresh in
    // place instead of duplicating. Avoids the old delete-by-date pattern
    // which wiped Mac-synced rows on every phone poll.
    let mut count = 0;
    for s in sessions {
        let date = s["date"].as_str().unwrap_or_default();
        let dur = s["duration_minutes"].as_f64().unwrap_or(0.0);
        let etype = s["type"].as_str().unwrap_or("other");
        let title = s["title"].as_str().unwrap_or("");
        let record_id = s["record_id"].as_str().unwrap_or("");
        // Kotlin readExerciseSessions hands us the per-session start time
        // already formatted "HH:MM" in the local zone — persist it so the
        // Calendar/Timeline syncs can place the block at the real start
        // instead of falling back to a 12:00 default.
        let start_time = s["start_time"].as_str().unwrap_or("");
        let notes = format!("{}: {}", etype, title);
        let stable_id = if record_id.is_empty() {
            format!("health:exercise:{date}:{start_time}:{notes}")
        } else { format!("health:exercise:{record_id}") };
        let existing: Option<String> = conn.query_row(
            "SELECT id FROM health_log
             WHERE id=?1 OR (type='exercise' AND date=?2 AND COALESCE(start_time,'')=?3 AND notes=?4)
             ORDER BY (id=?1) DESC LIMIT 1",
            rusqlite::params![stable_id, date, start_time, notes], |r| r.get(0),
        ).ok();
        if let Some(id) = existing {
            let _ = conn.execute(
                "UPDATE health_log SET value=?1,date=?2,start_time=?3,notes=?4
                 WHERE id=?5 AND (value IS NOT ?1 OR date IS NOT ?2 OR start_time IS NOT ?3 OR notes IS NOT ?4)",
                rusqlite::params![dur, date, start_time, notes, id],
            );
        } else {
            let _ = conn.execute(
                "INSERT INTO health_log (id, date, type, value, unit, notes, start_time, created_at)
                 VALUES (?1,?2,'exercise',?3,'minutes',?4,?5,?6)",
                rusqlite::params![stable_id, date, dur, notes, start_time, now],
            );
        }
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
pub struct HealthPoint {
    pub date: String,
    pub value: f64,
}

#[derive(Serialize)]
pub struct HealthSummary {
    pub avg_sleep_minutes: f64,
    pub avg_steps: f64,
    pub avg_resting_hr: f64,
    pub sleep_sessions: i64,
    pub days_with_steps: i64,
    pub hr_samples: i64,
    pub steps: Vec<HealthPoint>,
}

#[tauri::command]
pub fn get_health_summary(db: State<'_, HanniDb>, days: i64) -> HealthSummary {
    let conn = db.conn();
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let avg_sleep: f64 = conn.query_row(
        "SELECT COALESCE(AVG(duration_minutes),0) FROM sleep_sessions WHERE date>=?1", [&since], |r| r.get(0),
    ).unwrap_or(0.0);
    let (avg_steps, days_steps): (f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(value),0), COUNT(*) FROM (
           SELECT date, MAX(value) AS value FROM health_log
           WHERE type='steps' AND date>=?1 GROUP BY date
         )", [&since], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0));
    let (avg_hr, hr_count): (f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(bpm),0), COUNT(*) FROM heart_rate_samples WHERE date>=?1", [&since], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0));
    let sleep_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sleep_sessions WHERE date>=?1", [&since], |r| r.get(0),
    ).unwrap_or(0);
    let steps = conn.prepare(
        "SELECT date, MAX(value) FROM health_log
         WHERE type='steps' AND date>=?1 GROUP BY date ORDER BY date"
    ).and_then(|mut stmt| {
        let rows = stmt.query_map([&since], |r| Ok(HealthPoint {
            date: r.get(0)?, value: r.get(1)?,
        }))?;
        Ok(rows.filter_map(Result::ok).collect())
    }).unwrap_or_default();
    HealthSummary { avg_sleep_minutes: avg_sleep, avg_steps, avg_resting_hr: avg_hr, sleep_sessions: sleep_count, days_with_steps: days_steps, hr_samples: hr_count, steps }
}

#[cfg(test)]
#[path = "health_import_tests.rs"]
mod tests;
