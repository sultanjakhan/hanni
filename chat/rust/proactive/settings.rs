// proactive/settings.rs — Tauri commands for proactive settings management
use crate::types::*;
use crate::memory::save_proactive_settings;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn get_proactive_settings(state: tauri::State<'_, Arc<Mutex<ProactiveState>>>) -> Result<ProactiveSettings, String> {
    let state = state.lock().await;
    Ok(state.settings.clone())
}

#[tauri::command]
pub async fn set_proactive_settings(
    settings: ProactiveSettings,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    save_proactive_settings(&settings)?;
    let mut state = state.lock().await;
    state.settings = settings;
    Ok(())
}

#[tauri::command]
pub async fn set_user_typing(
    typing: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.user_is_typing = typing;
    Ok(())
}

#[tauri::command]
pub async fn set_recording_state(
    recording: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    state.lock().await.is_recording = recording;
    Ok(())
}

#[tauri::command]
pub async fn report_proactive_engagement(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    // Mark the last proactive message as replied
    if let Some(pid) = pstate.last_proactive_id {
        {
                let conn = db.conn();
            let delay = pstate.last_message_time
                .map(|t| (chrono::Local::now() - t).num_seconds())
                .unwrap_or(0);
            let _ = conn.execute(
                "UPDATE proactive_history SET user_replied = 1, reply_delay_secs = ?1 WHERE id = ?2",
                rusqlite::params![delay, pid],
            );
        }
    }
    // Recompute engagement rate: rolling avg of last 20 proactive messages
    {
                let conn = db.conn();
        let rate: f64 = conn.query_row(
            "SELECT COALESCE(AVG(CAST(user_replied AS REAL)), 0.5) FROM (SELECT user_replied FROM proactive_history ORDER BY id DESC LIMIT 20)",
            [], |row| row.get(0),
        ).unwrap_or(0.5);
        pstate.engagement_rate = rate;
    }
    Ok(())
}

#[tauri::command]
pub async fn report_user_chat_activity(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    pstate.last_user_chat_time = Some(chrono::Local::now());
    Ok(())
}

#[tauri::command]
pub fn rate_proactive(db: tauri::State<'_, HanniDb>, proactive_id: i64, rating: i64) -> Result<(), String> {
    db.conn().execute(
        "UPDATE proactive_history SET rating = ?1 WHERE id = ?2",
        rusqlite::params![rating, proactive_id],
    ).map_err(|e| format!("DB: {}", e))?;
    Ok(())
}
