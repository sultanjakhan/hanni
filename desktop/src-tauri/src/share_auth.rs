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

impl LinkCtx {
    /// `scope` may be a single keyword (`"all"`, `"recipes"`) or a CSV
    /// (`"recipes,fridge"`). `"all"` covers everything; otherwise membership
    /// is by explicit listing.
    pub fn has_scope(&self, target: &str) -> bool {
        let s = self.scope.trim();
        s == "all" || s.split(',').any(|part| part.trim() == target)
    }
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
    // rate_limit_check runs BEFORE token validation, so an internet attacker
    // spamming distinct random tokens could otherwise grow this HashMap without
    // bound (memory-exhaustion DoS). Prune expired entries on every call — the
    // map holds at most a minute's worth of active tokens, so retain() is cheap.
    map.retain(|_, (_, started)| now - *started < 60);
    // Hard cap: if still saturated with fresh distinct tokens, shed new ones
    // rather than grow. 5_000 active tokens/min dwarfs any real multi-guest use.
    if map.len() >= 5_000 && !map.contains_key(token) {
        return Err((StatusCode::TOO_MANY_REQUESTS, "Server busy".into()));
    }
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
    // Trust forwarded-IP headers ONLY when the TCP peer is loopback — i.e. the
    // request came through the local Cloudflare Tunnel, which sets CF-Connecting-IP.
    // The server now binds 0.0.0.0 (Tailnet reach), so a direct (non-loopback)
    // peer could otherwise spoof CF-Connecting-IP / X-Forwarded-For to poison
    // share_activity.guest_ip. For direct peers, use the real TCP source.
    let ip = if addr.ip().is_loopback() {
        headers.get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| headers.get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()))
            .unwrap_or_else(|| addr.ip().to_string())
    } else {
        addr.ip().to_string()
    };
    (ua, ip)
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;").replace('\'', "&#39;")
}

/// Sanitize a guest-supplied display name. Strips HTML-relevant and control
/// characters, caps length at 30. Keeps Cyrillic, emoji, common punctuation —
/// only removes what could break renderers or enable stored XSS.
/// Returns `default_name` if input is None / empty / all-stripped.
pub fn sanitize_author(input: Option<&str>, default_name: &str) -> String {
    let raw = input.unwrap_or("").trim();
    let cleaned: String = raw.chars()
        .filter(|c| !matches!(c, '<' | '>' | '"' | '\'' | '&' | '\0'..='\x1f' | '\x7f'))
        .take(30)
        .collect();
    if cleaned.is_empty() { default_name.to_string() } else { cleaned }
}
