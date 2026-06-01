// commands_share.rs — Tauri commands for managing share-links from the UI

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
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

// Build the guest-facing URL for a share token.
//
// Prefer a user-configured Tailscale Funnel (or any public https tunnel) as
// the entry point: axum serves the guest frontend itself, so the link needs
// neither Firebase Hosting nor a `?t=` param — nothing in the path can go
// stale or depend on Firestore.
//
// Fall back to the legacy Firebase Hosting form only when no public funnel is
// set. There `?t=<tunnel>` lets the guest reach axum directly; without it the
// guest hits the dead Firestore read-path (Firestore is off post firebase-off).
fn cloud_url_with_tunnel(conn: &rusqlite::Connection, token: &str) -> String {
    let setting = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            rusqlite::params![key], |r| r.get::<_, String>(0),
        ).ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
    };

    if let Some(base) = setting("share_funnel_url").filter(|s| s.starts_with("https://")) {
        return format!("{}/s/{}", base, token);
    }

    let legacy = format!("https://hanni-2e5d0.web.app/s/{}", token);
    match setting("share_tunnel_url") {
        Some(t) => format!("{}?t={}", legacy, utf8_percent_encode(&t, NON_ALPHANUMERIC)),
        None => legacy,
    }
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
    let allowed_perms = ["view", "add", "edit", "delete", "comment"];
    for p in &permissions {
        if !allowed_perms.contains(&p.as_str()) {
            return Err(format!("Unknown permission: {}", p));
        }
    }
    if permissions.is_empty() {
        return Err("At least one permission is required".into());
    }
    // Scope is "all" or a CSV of known keys for the tab. We don't enforce
    // tab-specific keys here — share_routes check ctx.has_scope() at request time.
    let allowed_scope_parts = ["all", "recipes", "products", "fridge", "meal_plan", "memory"];
    let scope_trimmed = scope.trim();
    if scope_trimmed.is_empty() {
        return Err("Scope is required".into());
    }
    for part in scope_trimmed.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !allowed_scope_parts.contains(&part) {
            return Err(format!("Unknown scope: {}", part));
        }
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
        // Capture rowid BEFORE mark_dirty — mark_dirty's app_settings INSERT
        // would otherwise become last_insert_rowid().
        let new_id = conn.last_insert_rowid();
        // Backfill cloud mirror: mark every relevant table dirty so the
        // background loop pushes a full snapshot for this new share-link.
        for tbl in &["recipes", "recipe_ingredients", "ingredient_catalog",
                     "products", "food_blacklist", "meal_plan"] {
            crate::sync_share::mark_dirty(&conn, tbl);
        }
        new_id
    };

    // Lazy-start the Cloudflare tunnel so writes from the guest can reach
    // axum (read-only access doesn't need it — Firebase Hosting + Firestore).
    // Tunnel URL is also persisted via share_tunnel.rs and mirrored to
    // Firestore so guests on Firebase Hosting know where to POST.
    if let Err(e) = share_tunnel::ensure_running(app.clone(), share_port()).await {
        eprintln!("[share] tunnel unavailable: {}", e);
    }

    // Static cloud URL with live tunnel embedded — guest reads via axum
    // (zero Firestore reads) when the host is online.
    let url = { let conn = db.conn(); cloud_url_with_tunnel(&conn, &token) };

    Ok(ShareLinkRow {
        id, token: token.clone(), tab, scope, permissions, label: label_val,
        lifetime: lifetime_val, expires_at,
        used_count: 0, revoked_at: None,
        created_at: now.clone(), updated_at: now,
        url: Some(url),
    })
}

#[tauri::command]
pub fn list_share_links(
    tab: Option<String>,
    db: State<'_, HanniDb>,
    _tunnel: State<'_, ShareTunnel>,
) -> Result<Vec<ShareLinkRow>, String> {
    let conn = db.conn();
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
                .map(|mut l| { l.url = Some(cloud_url_with_tunnel(&conn, &l.token)); l })
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
                .map(|mut l| { l.url = Some(cloud_url_with_tunnel(&conn, &l.token)); l })
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
pub fn delete_share_link(id: i64, db: State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM share_activity WHERE link_id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error (activity): {}", e))?;
    conn.execute("DELETE FROM share_comments WHERE link_id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error (comments): {}", e))?;
    conn.execute("DELETE FROM share_links WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error (link): {}", e))?;
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
