// sync_share_auto.rs — background mirror loop for Stage C-1.
//
// Wakes every MIRROR_TICK_SECS, asks sync_share::mirror_pending to push any
// dirty tables to Firestore for all active share-links. No settings, no UI —
// it just runs as long as Hanni is open.

use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::sync_share::mirror_pending;
use crate::types::HanniDb;

const MIRROR_TICK_SECS: u64 = 3;
const STARTUP_DELAY_SECS: u64 = 8;

pub fn start_mirror_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(STARTUP_DELAY_SECS)).await;

        loop {
            tokio::time::sleep(Duration::from_secs(MIRROR_TICK_SECS)).await;

            let db = app.state::<HanniDb>();
            let result = mirror_pending(&db).await;
            drop(db);

            match result {
                Ok(v) => {
                    // Only emit a tick when something actually happened, to keep
                    // the JS console quiet during idle periods.
                    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    if status == "ok" || status == "partial" {
                        let _ = app.emit("cloud-share-mirror-tick", json!({
                            "ok": status == "ok",
                            "ts": chrono::Utc::now().to_rfc3339(),
                            "result": v,
                        }));
                    }
                }
                Err(e) => {
                    let _ = app.emit("cloud-share-mirror-tick", json!({
                        "ok": false,
                        "ts": chrono::Utc::now().to_rfc3339(),
                        "error": e,
                    }));
                }
            }
        }
    });
}
