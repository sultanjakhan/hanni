// commands_share.rs — Tauri commands for managing share-links from the UI

use rand::Rng;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::share_server::share_port;
use crate::share_tunnel::{self, ShareTunnel};
use crate::types::HanniDb;

fn gen_token() -> String {
    // 32 URL-safe chars ≈ 192 bits of entropy
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..32).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect()
}

#[derive(Serialize)]
pub struct ShareLinkRow {
    pub id: i64,
    pub token: String,
    pub tab: String,
    pub scope: String,
    pub permissions: Vec<String>,
    pub label: String,
    pub lifetime: String,
    pub expires_at: Option<String>,
    pub used_count: i64,
    pub revoked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub url: Option<String>,
}

fn tunnel_url(tunnel: &State<'_, ShareTunnel>) -> Option<String> {
    tunnel.0.lock().ok().and_then(|s| s.url.clone())
}

fn full_url(base: &Option<String>, token: &str) -> Option<String> {
    base.as_ref().map(|u| format!("{}/s/{}", u.trim_end_matches('/'), token))
}

#[tauri::command]
pub async fn create_share_link(
    app: AppHandle,
    tab: String,
    scope: String,
    permissions: Vec<String>,
    label: Option<String>,
    lifetime: Option<String>,
    expires_at: Option<String>,
    db: State<'_, HanniDb>,
) -> Result<ShareLinkRow, String> {
    let allowed_perms = ["view", "add", "edit", "comment"];
    for p in &permissions {
        if !allowed_perms.contains(&p.as_str()) {
            return Err(format!("Unknown permission: {}", p));
        }
    }
    if permissions.is_empty() {
        return Err("At least one permission is required".into());
    }
    let lifetime_val = lifetime.unwrap_or_else(|| "permanent".into());
    if !["once", "permanent", "expires"].contains(&lifetime_val.as_str()) {
        return Err(format!("Unknown lifetime: {}", lifetime_val));
    }

    let token = gen_token();
    let label_val = label.unwrap_or_default();
    let perms_json = serde_json::to_string(&permissions).unwrap_or_else(|_| "[]".into());
    let now = chrono::Local::now().to_rfc3339();

    let id: i64 = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO share_links (token, tab, scope, permissions, label, lifetime, expires_at, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
            rusqlite::params![token, tab, scope, perms_json, label_val, lifetime_val, expires_at, now],
        ).map_err(|e| format!("DB error: {}", e))?;
        conn.last_insert_rowid()
    };

    // Lazy-start the Cloudflare tunnel so the link has a reachable URL.
    let base = match share_tunnel::ensure_running(app.clone(), share_port()).await {
        Ok(u) => Some(u),
        Err(e) => {
            eprintln!("[share] tunnel unavailable: {}", e);
            None
        }
    };
    Ok(ShareLinkRow {
        id, token: token.clone(), tab, scope, permissions, label: label_val,
        lifetime: lifetime_val, expires_at,
        used_count: 0, revoked_at: None,
        created_at: now.clone(), updated_at: now,
        url: full_url(&base, &token),
    })
}

#[tauri::command]
pub fn list_share_links(
    tab: Option<String>,
    db: State<'_, HanniDb>,
    tunnel: State<'_, ShareTunnel>,
) -> Result<Vec<ShareLinkRow>, String> {
    let conn = db.conn();
    let base = tunnel_url(&tunnel);
    let rows: Vec<ShareLinkRow> = match tab {
        Some(t) => {
            let mut stmt = conn.prepare(
                "SELECT id, token, tab, scope, permissions, label, lifetime, expires_at,
                        used_count, revoked_at, created_at, updated_at
                 FROM share_links WHERE tab=?1 ORDER BY created_at DESC"
            ).map_err(|e| format!("DB error: {}", e))?;
            let iter = stmt.query_map(rusqlite::params![t], |r| row_to_link(r))
                .map_err(|e| format!("Query error: {}", e))?;
            iter.filter_map(|x| x.ok())
                .map(|mut l| { l.url = full_url(&base, &l.token); l })
                .collect()
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, token, tab, scope, permissions, label, lifetime, expires_at,
                        used_count, revoked_at, created_at, updated_at
                 FROM share_links ORDER BY created_at DESC"
            ).map_err(|e| format!("DB error: {}", e))?;
            let iter = stmt.query_map([], |r| row_to_link(r))
                .map_err(|e| format!("Query error: {}", e))?;
            iter.filter_map(|x| x.ok())
                .map(|mut l| { l.url = full_url(&base, &l.token); l })
                .collect()
        }
    };
    Ok(rows)
}

fn row_to_link(r: &rusqlite::Row) -> Result<ShareLinkRow, rusqlite::Error> {
    let perms_str: String = r.get(4)?;
    let permissions: Vec<String> = serde_json::from_str(&perms_str).unwrap_or_default();
    Ok(ShareLinkRow {
        id: r.get(0)?,
        token: r.get(1)?,
        tab: r.get(2)?,
        scope: r.get(3)?,
        permissions,
        label: r.get(5)?,
        lifetime: r.get(6)?,
        expires_at: r.get(7)?,
        used_count: r.get(8)?,
        revoked_at: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        url: None,
    })
}

#[tauri::command]
pub fn revoke_share_link(id: i64, db: State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE share_links SET revoked_at=?1, updated_at=?1 WHERE id=?2 AND revoked_at IS NULL",
        rusqlite::params![now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_share_activity(
    link_id: i64,
    limit: Option<i64>,
    db: State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(100);
    let mut stmt = conn.prepare(
        "SELECT id, action, payload, guest_ip, user_agent, created_at
         FROM share_activity WHERE link_id=?1 ORDER BY id DESC LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![link_id, max], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "action": r.get::<_, String>(1)?,
            "payload": r.get::<_, String>(2)?,
            "guest_ip": r.get::<_, String>(3)?,
            "user_agent": r.get::<_, String>(4)?,
            "created_at": r.get::<_, String>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
      .filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn tunnel_status(tunnel: State<'_, ShareTunnel>) -> Result<serde_json::Value, String> {
    let s = tunnel.0.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "running": s.running,
        "url": s.url,
        "error": s.error,
    }))
}
