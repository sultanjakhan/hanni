// sync_commands.rs — Tauri commands for sync UI + auto-sync background loop
use crate::sync::*;
use crate::types::HanniDb;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct SyncConfig {
    pub enabled: bool,
    pub relay_url: String,
    pub device_token: String,
    pub device_name: String,
    pub auto_sync_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub last_sync: Option<String>,
    pub last_push_version: i64,
    pub last_pull_version: i64,
    pub pending_changes: usize,
    pub site_id: String,
    pub device_name: String,
}

fn load_sync_config(db: &HanniDb) -> Result<SyncConfig, String> {
    let conn = db.conn();
    let get = |key: &str, default: &str| -> Result<String, String> {
        Ok(crate::secret_store::get_setting(&conn, key)?
            .unwrap_or_else(|| default.to_string()))
    };
    Ok(SyncConfig {
        enabled: get("sync_enabled", "false")? == "true",
        relay_url: get("sync_relay_url", "")?,
        device_token: get("sync_device_token", "")?,
        device_name: get("sync_device_name", "")?,
        auto_sync_secs: get("sync_auto_secs", "30")?.parse().unwrap_or(30),
    })
}

fn save_sync_setting(db: &HanniDb, key: &str, value: &str) -> Result<(), String> {
    let conn = db.conn();
    crate::secret_store::set_setting(&conn, key, value)
}

#[tauri::command]
pub fn get_sync_status(db: State<'_, HanniDb>) -> Result<SyncStatus, String> {
    let config = load_sync_config(&db)?;
    let conn = db.conn();
    let site_id = get_site_id(&conn);
    let last_push: i64 = conn.query_row(
        "SELECT CAST(COALESCE((SELECT value FROM app_settings WHERE key='sync_last_push_ver'), '0') AS INTEGER)",
        [], |r| r.get(0),
    ).unwrap_or(0);
    let last_pull: i64 = conn.query_row(
        "SELECT CAST(COALESCE((SELECT value FROM app_settings WHERE key='sync_last_pull_ver'), '0') AS INTEGER)",
        [], |r| r.get(0),
    ).unwrap_or(0);
    let pending = get_local_changes(&conn, last_push).len();
    let last_sync: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key='sync_last_time'",
        [], |r| r.get(0),
    ).ok();

    Ok(SyncStatus {
        enabled: config.enabled,
        last_sync,
        last_push_version: last_push,
        last_pull_version: last_pull,
        pending_changes: pending,
        site_id,
        device_name: config.device_name,
    })
}

#[tauri::command]
pub fn set_sync_config(
    db: State<'_, HanniDb>,
    enabled: bool, relay_url: String, device_token: String,
    device_name: String,
) -> Result<(), String> {
    save_sync_setting(&db, "sync_enabled", if enabled { "true" } else { "false" })?;
    save_sync_setting(&db, "sync_relay_url", &relay_url)?;
    save_sync_setting(&db, "sync_device_token", &device_token)?;
    save_sync_setting(&db, "sync_device_name", &device_name)
}

#[tauri::command]
pub async fn sync_now(db: State<'_, HanniDb>) -> Result<String, String> {
    let config = load_sync_config(&db)?;
    if !config.enabled || config.relay_url.is_empty() {
        return Err("Sync not configured".into());
    }

    // 1. Push local changes
    let (changes, db_ver, site_id, last_push) = {
        let conn = db.conn();
        let last_push: i64 = conn.query_row(
            "SELECT CAST(COALESCE((SELECT value FROM app_settings WHERE key='sync_last_push_ver'), '0') AS INTEGER)",
            [], |r| r.get(0),
        ).unwrap_or(0);
        let changes = get_local_changes(&conn, last_push);
        let db_ver = get_db_version(&conn);
        let site_id = get_site_id(&conn);
        (changes, db_ver, site_id, last_push)
    };

    if !changes.is_empty() {
        let payload = SyncPayload {
            device_id: site_id.clone(),
            changes,
            db_version: db_ver,
        };
        push_changes(&config.relay_url, &config.device_token, &payload).await?;
        save_sync_setting(&db, "sync_last_push_ver", &db_ver.to_string())?;
    }

    // 2. Pull remote changes
    let last_pull: i64 = {
        let conn = db.conn();
        conn.query_row(
            "SELECT CAST(COALESCE((SELECT value FROM app_settings WHERE key='sync_last_pull_ver'), '0') AS INTEGER)",
            [], |r| r.get(0),
        ).unwrap_or(0)
    };
    let pull_resp = pull_changes(
        &config.relay_url, &config.device_token, last_pull,
    ).await?;

    if !pull_resp.changes.is_empty() {
        let conn = db.conn();
        apply_remote_changes(&conn, &pull_resp.changes)?;
        drop(conn);
        save_sync_setting(&db, "sync_last_pull_ver", &pull_resp.server_version.to_string())?;
    }

    // Update last sync time
    let now = chrono::Utc::now().to_rfc3339();
    save_sync_setting(&db, "sync_last_time", &now)?;

    Ok(format!("Synced: pushed {} ver, pulled {} changes", db_ver, pull_resp.changes.len()))
}
