// sync_github_cmds.rs — Tauri commands to configure GitHub owner-sync (Tier 3).
// Provisioning: PAT + repo, the shared E2E key, and the backend feature-flag.

use crate::sync_owner::set_setting;
use crate::types::HanniDb;
use rand::RngCore;
use tauri::State;

#[tauri::command]
pub fn cloud_owner_gh_set_config(pat: String, repo: String, db: State<'_, HanniDb>)
                                 -> Result<(), String> {
    let conn = db.conn();
    set_setting(&conn, "cloud_owner_gh_pat", pat.trim());
    set_setting(&conn, "cloud_owner_gh_repo", repo.trim());
    Ok(())
}

/// Generate a fresh 32-byte shared key, store it, and return it as hex to hand
/// to the other device over the existing LAN-pairing channel (QR / short code).
#[tauri::command]
pub fn cloud_owner_gh_gen_key(db: State<'_, HanniDb>) -> Result<String, String> {
    let mut k = [0u8; 32];
    rand::rng().fill_bytes(&mut k);
    let hx = hex::encode(k);
    set_setting(&db.conn(), "cloud_owner_gh_key", &hx);
    Ok(hx)
}

/// Set the shared key received from the other device (64 hex chars).
#[tauri::command]
pub fn cloud_owner_gh_set_key(key_hex: String, db: State<'_, HanniDb>) -> Result<(), String> {
    let h = key_hex.trim();
    let bytes = hex::decode(h).map_err(|_| "key must be hex".to_string())?;
    if bytes.len() != 32 {
        return Err("key must be 32 bytes (64 hex chars)".into());
    }
    set_setting(&db.conn(), "cloud_owner_gh_key", h);
    Ok(())
}

/// Switch the owner-sync backend: "firestore" (default) or "github".
#[tauri::command]
pub fn cloud_owner_backend_set(backend: String, db: State<'_, HanniDb>) -> Result<(), String> {
    if backend != "firestore" && backend != "github" {
        return Err("backend must be 'firestore' or 'github'".into());
    }
    set_setting(&db.conn(), "cloud_owner_backend", &backend);
    Ok(())
}
