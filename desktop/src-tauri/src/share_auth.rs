// share_auth.rs — Link lookup, rate limiting, activity logging, shared helpers.
// All DB ops take an existing &Connection to avoid re-locking the Mutex.

use axum::http::{HeaderMap, StatusCode, header};
use std::net::SocketAddr;

use crate::share_server::ShareServerState;

const RATE_LIMIT_PER_MINUTE: u32 = 100;
pub const BODY_LIMIT_BYTES: usize = 256 * 1024;

pub struct LinkCtx {
    pub id: i64,
    pub scope: String,
    pub permissions: Vec<String>,
    pub label: String,
    pub tab: String,
}

pub fn load_link(conn: &rusqlite::Connection, token: &str) -> Result<LinkCtx, (StatusCode, String)> {
    let row: Result<(i64, String, String, String, String, Option<String>, Option<String>), _> = conn.query_row(
        "SELECT id, tab, scope, permissions, label, expires_at, revoked_at
         FROM share_links WHERE token=?1",
        rusqlite::params![token],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    );
    match row {
        Ok((id, tab, scope, perms, label, expires_at, revoked_at)) => {
            if revoked_at.is_some() {
                return Err((StatusCode::GONE, "Link revoked".into()));
            }
            if let Some(exp) = expires_at {
                if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&exp) {
                    if chrono::Local::now() > t.with_timezone(&chrono::Local) {
                        return Err((StatusCode::GONE, "Link expired".into()));
                    }
                }
            }
            let permissions: Vec<String> = serde_json::from_str(&perms).unwrap_or_default();
            Ok(LinkCtx { id, scope, permissions, label, tab })
        }
        Err(_) => Err((StatusCode::NOT_FOUND, "Link not found".into())),
    }
}

pub fn require_perm(ctx: &LinkCtx, needed: &str) -> Result<(), (StatusCode, String)> {
    if ctx.permissions.iter().any(|p| p == needed) { Ok(()) } else {
        Err((StatusCode::FORBIDDEN, format!("Permission '{}' denied", needed)))
    }
}

pub fn rate_limit_check(state: &ShareServerState, token: &str) -> Result<(), (StatusCode, String)> {
    let now = chrono::Local::now().timestamp();
    let mut map = state.rate_limit.lock().unwrap();
    let entry = map.entry(token.to_string()).or_insert((0, now));
    if now - entry.1 >= 60 { *entry = (0, now); }
    entry.0 += 1;
    if entry.0 > RATE_LIMIT_PER_MINUTE {
        Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".into()))
    } else { Ok(()) }
}

pub fn log_activity(conn: &rusqlite::Connection, link_id: i64, action: &str, payload: &str, ip: &str, ua: &str) {
    let now = chrono::Local::now().to_rfc3339();
    let _ = conn.execute(
        "INSERT INTO share_activity (link_id, action, payload, guest_ip, user_agent, created_at)
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![link_id, action, payload, ip, ua, now],
    );
    let _ = conn.execute(
        "UPDATE share_links SET used_count=used_count+1, updated_at=?1 WHERE id=?2",
        rusqlite::params![now, link_id],
    );
    if action != "view" {
        let _ = conn.execute(
            "UPDATE share_links SET revoked_at=?1 WHERE id=?2 AND lifetime='once' AND revoked_at IS NULL",
            rusqlite::params![now, link_id],
        );
    }
}

pub fn ua_ip(headers: &HeaderMap, addr: &SocketAddr) -> (String, String) {
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let ip = addr.ip().to_string();
    (ua, ip)
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;").replace('\'', "&#39;")
}
