// sync_share_auto.rs — background mirror loop for Stage C-1.
//
// Wakes every MIRROR_TICK_SECS, asks sync_share::mirror_pending to push any
// dirty tables to Firestore for all active share-links. No settings, no UI —
// it just runs as long as Hanni is open.
//
// 429 backoff: Firestore Spark plan has a 20k writes/day quota. When we hit
// it, retrying every 3s burns nothing but logs and Firebase metrics. After
// a 429 we suspend pushes for QUOTA_BACKOFF_SECS, then probe once. Backoff
// clears on the first successful push (status=ok|partial without 429s).

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::sync_share::mirror_pending;
use crate::types::HanniDb;

const MIRROR_TICK_SECS: u64 = 3;
const STARTUP_DELAY_SECS: u64 = 8;
const QUOTA_BACKOFF_SECS: u64 = 1800; // 30 min

static BACKOFF_UNTIL: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

fn backoff_slot() -> &'static Mutex<Option<Instant>> {
    BACKOFF_UNTIL.get_or_init(|| Mutex::new(None))
}

fn is_backed_off() -> bool {
    let mut g = backoff_slot().lock().unwrap_or_else(|e| e.into_inner());
    match *g {
        Some(until) if Instant::now() < until => true,
        Some(_) => { *g = None; false }, // expired — clear and retry
        None => false,
    }
}

fn arm_backoff() {
    let until = Instant::now() + Duration::from_secs(QUOTA_BACKOFF_SECS);
    *backoff_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(until);
}

fn clear_backoff() {
    *backoff_slot().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn saw_quota_429(v: &serde_json::Value) -> bool {
    v.get("errors").and_then(|e| e.as_array()).map_or(false, |arr| {
        arr.iter().any(|err| {
            err.get("error").and_then(|e| e.as_str()).map_or(false, |s| {
                s.contains("429") || s.contains("RESOURCE_EXHAUSTED")
                    || s.contains("Quota exceeded")
            })
        })
    })
}

pub fn start_mirror_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(STARTUP_DELAY_SECS)).await;

        loop {
            tokio::time::sleep(Duration::from_secs(MIRROR_TICK_SECS)).await;

            if is_backed_off() { continue; }

            let db = app.state::<HanniDb>();
            let result = mirror_pending(&db).await;
            drop(db);

            match result {
                Ok(v) => {
                    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    if saw_quota_429(&v) {
                        arm_backoff();
                        eprintln!("[mirror] Firestore 429 — sleeping {}s", QUOTA_BACKOFF_SECS);
                        let _ = app.emit("cloud-share-mirror-tick", json!({
                            "ok": false,
                            "ts": chrono::Utc::now().to_rfc3339(),
                            "error": "quota_exceeded_backoff",
                            "backoff_secs": QUOTA_BACKOFF_SECS,
                        }));
                    } else if status == "ok" || status == "partial" {
                        // First successful push since a backoff — clear it so
                        // next tick runs at the normal cadence.
                        clear_backoff();
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
