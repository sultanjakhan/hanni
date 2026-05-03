// sync_owner_auto.rs — background auto-sync loop for owner-side Firestore
// sync. Periodically calls push_inner + pull_inner from sync_owner.
//
// Settings (in app_settings):
//   cloud_owner_auto_enabled — "true" / "false" (default false)
//   cloud_owner_auto_secs    — interval, seconds (default 60, min 30, max 600)
//
// Events emitted to JS:
//   cloud-owner-sync-tick    — { ok, pushed, applied, ts, error? }

use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::sync_owner::{pull_inner, push_inner};
use crate::types::HanniDb;

const DEFAULT_INTERVAL_SECS: u64 = 60;
const MIN_INTERVAL_SECS: u64 = 30;
const MAX_INTERVAL_SECS: u64 = 600;
const BACKOFF_CAP_SECS: u64 = 300;

fn read_settings(db: &HanniDb) -> (bool, u64) {
    let conn = db.conn();
    let get = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            rusqlite::params![key], |r| r.get(0),
        ).ok()
    };
    let enabled = get("cloud_owner_auto_enabled").as_deref() == Some("true");
    let secs = get("cloud_owner_auto_secs")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_INTERVAL_SECS)
        .clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS);
    (enabled, secs)
}

pub fn start_auto_sync_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Small startup delay so app finishes booting before first tick.
        tokio::time::sleep(Duration::from_secs(10)).await;

        let mut backoff_until_secs: u64 = 0;
        loop {
            let db = app.state::<HanniDb>();
            let (enabled, interval) = read_settings(&db);
            // Holding `db` across await would tie a non-Send Mutex guard
            // (via .conn()) to the future; release before sleeping.
            drop(db);

            let sleep_for = if backoff_until_secs > interval { backoff_until_secs } else { interval };
            tokio::time::sleep(Duration::from_secs(sleep_for)).await;

            if !enabled { continue; }

            let db = app.state::<HanniDb>();
            let push_res = push_inner(&db).await;
            let pull_res = pull_inner(&db).await;
            drop(db);

            let ts = chrono::Utc::now().to_rfc3339();
            match (push_res, pull_res) {
                (Ok(p), Ok(q)) => {
                    backoff_until_secs = 0;
                    let _ = app.emit("cloud-owner-sync-tick", json!({
                        "ok": true, "ts": ts, "push": p, "pull": q,
                    }));
                }
                (push_r, pull_r) => {
                    backoff_until_secs = (interval * 2).min(BACKOFF_CAP_SECS);
                    let err = push_r.err().or_else(|| pull_r.err()).unwrap_or_default();
                    let _ = app.emit("cloud-owner-sync-tick", json!({
                        "ok": false, "ts": ts, "error": err,
                    }));
                }
            }
        }
    });
}

#[derive(Debug, serde::Serialize)]
pub struct OwnerAutoCfg {
    pub enabled: bool,
    pub interval_secs: u64,
}

#[tauri::command]
pub fn cloud_owner_get_auto(db: tauri::State<'_, HanniDb>) -> OwnerAutoCfg {
    let (enabled, interval_secs) = read_settings(&db);
    OwnerAutoCfg { enabled, interval_secs }
}

#[tauri::command]
pub fn cloud_owner_set_auto(
    enabled: bool,
    interval_secs: u64,
    db: tauri::State<'_, HanniDb>,
) -> Result<OwnerAutoCfg, String> {
    let interval = interval_secs.clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS);
    let conn = db.conn();
    let upsert = |key: &str, value: &str| {
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![key, value],
        ).map_err(|e| e.to_string())
    };
    upsert("cloud_owner_auto_enabled", if enabled { "true" } else { "false" })?;
    upsert("cloud_owner_auto_secs", &interval.to_string())?;
    Ok(OwnerAutoCfg { enabled, interval_secs: interval })
}
