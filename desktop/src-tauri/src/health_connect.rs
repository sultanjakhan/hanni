// health_connect.rs — Sleep data from Samsung Health / Health Connect API
// On Android: calls Kotlin plugin via Tauri mobile plugin bridge
// On desktop: returns stub data or manual entries from SQLite
use crate::types::HanniDb;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SleepStage {
    pub start_time: String,
    pub end_time: String,
    pub stage: String, // awake, light, deep, rem, sleeping, out_of_bed
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SleepSession {
    pub id: Option<String>, // UUIDv7 since Phase 1 migration
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: i64,
    pub stages: Vec<SleepStage>,
    pub source: String, // health_connect, manual
    pub quality_score: Option<i64>,
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SleepStats {
    pub avg_duration_minutes: f64,
    pub avg_deep_minutes: f64,
    pub avg_rem_minutes: f64,
    pub total_sessions: i64,
}

#[tauri::command]
pub fn get_sleep_sessions(
    db: State<'_, HanniDb>, from: String, to: String,
) -> Vec<SleepSession> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, date, start_time, end_time, duration_minutes, source, quality_score, notes
         FROM sleep_sessions WHERE date >= ?1 AND date <= ?2 ORDER BY date DESC"
    ).unwrap();
    let sessions: Vec<SleepSession> = stmt.query_map(
        rusqlite::params![from, to], |row| {
            Ok(SleepSession {
                id: row.get(0)?,
                date: row.get(1)?,
                start_time: row.get(2)?,
                end_time: row.get(3)?,
                duration_minutes: row.get(4)?,
                stages: vec![],
                source: row.get(5)?,
                quality_score: row.get(6)?,
                notes: row.get(7)?,
            })
        }
    ).unwrap().filter_map(|r| r.ok()).collect();

    // Load stages for each session
    sessions.into_iter().map(|mut s| {
        if let Some(sid) = &s.id {
            let mut st = conn.prepare(
                "SELECT start_time, end_time, stage FROM sleep_stages WHERE session_id=?1"
            ).unwrap();
            s.stages = st.query_map(rusqlite::params![sid], |row| Ok(SleepStage {
                start_time: row.get(0)?,
                end_time: row.get(1)?,
                stage: row.get(2)?,
            })).unwrap().filter_map(|r| r.ok()).collect();
        }
        s
    }).collect()
}

#[tauri::command]
pub fn add_sleep_session(db: State<'_, HanniDb>, session: SleepSession) -> Result<String, String> {
    let conn = db.conn();
    // INSERT OR REPLACE on (date, start_time, source) — if a session with the
    // same triple already exists, we get its id back instead of creating a
    // duplicate. Generate a fresh UUIDv7 for the new path; ignored on REPLACE.
    let new_id = crate::types::new_uuid_v7();
    conn.execute(
        "INSERT INTO sleep_sessions
         (id, date, start_time, end_time, duration_minutes, source, quality_score, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(date, start_time, source) DO UPDATE SET
             end_time = excluded.end_time,
             duration_minutes = excluded.duration_minutes,
             quality_score = excluded.quality_score,
             notes = excluded.notes",
        rusqlite::params![
            new_id, session.date, session.start_time, session.end_time,
            session.duration_minutes, session.source, session.quality_score, session.notes
        ],
    ).map_err(|e| e.to_string())?;
    // After upsert, fetch the canonical id (existing-row id wins on conflict).
    let id: String = conn.query_row(
        "SELECT id FROM sleep_sessions WHERE date=?1 AND start_time=?2 AND source=?3",
        rusqlite::params![session.date, session.start_time, session.source], |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    // Refresh stages: drop any existing for this session, then re-insert.
    conn.execute("DELETE FROM sleep_stages WHERE session_id=?1",
                 rusqlite::params![id]).map_err(|e| e.to_string())?;
    for stage in &session.stages {
        let stage_id = crate::types::new_uuid_v7();
        conn.execute(
            "INSERT INTO sleep_stages (id, session_id, start_time, end_time, stage)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![stage_id, id, stage.start_time, stage.end_time, stage.stage],
        ).map_err(|e| e.to_string())?;
    }
    Ok(id)
}

#[tauri::command]
pub fn delete_sleep_session(db: State<'_, HanniDb>, id: String) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM sleep_stages WHERE session_id=?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM sleep_sessions WHERE id=?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_sleep_stats(db: State<'_, HanniDb>, days: i64) -> SleepStats {
    let conn = db.conn();
    let since = chrono::Utc::now() - chrono::Duration::days(days);
    let since_str = since.format("%Y-%m-%d").to_string();
    let (avg_dur, total): (f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(duration_minutes), 0), COUNT(*) FROM sleep_sessions WHERE date >= ?1",
        [&since_str], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0));
    // sleep_stages.start_time / end_time stored as HH:MM (time-of-day,
    // no date). When a stage crosses midnight (start=23:30, end=01:00),
    // julianday(end)-julianday(start) goes negative. Add 24 h whenever
    // end < start so the duration is always positive.
    let stage_dur =
        "(julianday(ss.end_time) - julianday(ss.start_time) \
         + (julianday(ss.end_time) < julianday(ss.start_time))) * 1440";
    let avg_deep: f64 = conn.query_row(
        &format!(
            "SELECT COALESCE(AVG(mins), 0) FROM (
                SELECT SUM({stage_dur}) as mins
                FROM sleep_stages ss JOIN sleep_sessions s ON ss.session_id = s.id
                WHERE s.date >= ?1 AND ss.stage = 'deep' GROUP BY s.id)"
        ),
        [&since_str], |r| r.get(0),
    ).unwrap_or(0.0);
    let avg_rem: f64 = conn.query_row(
        &format!(
            "SELECT COALESCE(AVG(mins), 0) FROM (
                SELECT SUM({stage_dur}) as mins
                FROM sleep_stages ss JOIN sleep_sessions s ON ss.session_id = s.id
                WHERE s.date >= ?1 AND ss.stage = 'rem' GROUP BY s.id)"
        ),
        [&since_str], |r| r.get(0),
    ).unwrap_or(0.0);
    SleepStats { avg_duration_minutes: avg_dur, avg_deep_minutes: avg_deep, avg_rem_minutes: avg_rem, total_sessions: total }
}

/// Import sleep sessions from Health Connect (Android only).
/// Delegates to import_health_connect_all for full import.
#[tauri::command]
pub async fn import_health_connect_sleep<R: tauri::Runtime>(
    db: State<'_, HanniDb>,
    app: tauri::AppHandle<R>,
) -> Result<String, String> {
    crate::health_import::import_health_connect_all(db, app).await
        .map(|v| format!("Imported: {}", v))
}
