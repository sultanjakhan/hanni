// share_tunnel.rs — Cloudflare Tunnel subprocess manager.
// Quick tunnel via trycloudflare.com (no account required, ephemeral URL).

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

pub struct TunnelState {
    pub url: Option<String>,
    pub running: bool,
    pub error: Option<String>,
    pub child: Option<Child>,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self { url: None, running: false, error: None, child: None }
    }
}

pub struct ShareTunnel(pub Mutex<TunnelState>);

impl Default for ShareTunnel {
    fn default() -> Self { Self(Mutex::new(TunnelState::default())) }
}

// ── Binary discovery ──

pub fn find_cloudflared() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/cloudflared",
        "/usr/local/bin/cloudflared",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() { return Some(PathBuf::from(c)); }
    }
    // Fallback: `which`
    std::process::Command::new("which").arg("cloudflared").output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.as_os_str().len() > 0 && p.exists())
}

// ── Start tunnel (subprocess) and wait for the public URL line on stderr ──

/// Probe `tailscale ip -4`. If the host is on a Tailnet, return a stable
/// URL the user's other devices can reach forever — no ephemeral tunnel,
/// no Firestore, no quota. Falls back to None on machines without
/// Tailscale (or on Android, where the CLI doesn't ship).
fn detect_tailscale_url(port: u16) -> Option<String> {
    let candidates = [
        "/usr/local/bin/tailscale",
        "/opt/homebrew/bin/tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "tailscale",
    ];
    for bin in candidates {
        let out = match std::process::Command::new(bin).args(["ip", "-4"]).output() {
            Ok(o) if o.status.success() => o,
            _ => continue,
        };
        let ip = String::from_utf8_lossy(&out.stdout).lines().next()
            .unwrap_or("").trim().to_string();
        if ip.starts_with("100.") {
            return Some(format!("http://{}:{}", ip, port));
        }
    }
    None
}

pub async fn ensure_running(app: AppHandle, port: u16) -> Result<String, String> {
    {
        let st = app.state::<ShareTunnel>();
        let mut guard = st.0.lock().map_err(|e| e.to_string())?;
        // If we have a recorded child, check whether it's still alive;
        // if it exited, clear the state so we spawn a fresh tunnel.
        let still_alive = match guard.child.as_mut() {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        };
        if guard.running && still_alive {
            if let Some(url) = guard.url.clone() { return Ok(url); }
        }
        if !still_alive {
            guard.running = false;
            guard.url = None;
            guard.child = None;
        }
    }

    // Tailscale Funnel: if the user has set a public Funnel URL it survives
    // restarts forever, has no trycloudflare rate-limit, and needs no
    // subprocess. Adopt it and short-circuit — no cloudflared, no gist push.
    let funnel: Option<String> = {
        let db = app.state::<crate::types::HanniDb>();
        let conn = db.conn();
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='share_funnel_url'",
            [], |r| r.get::<_, String>(0),
        ).ok().filter(|s| !s.trim().is_empty())
    };
    if let Some(url) = funnel {
        eprintln!("[tunnel] using Tailscale Funnel (stable): {}", url);
        {
            let st = app.state::<ShareTunnel>();
            let mut guard = st.0.lock().map_err(|e| e.to_string())?;
            guard.running = true;
            guard.url = Some(url.clone());
            guard.error = None;
            guard.child = None;
        }
        // Persist as share_tunnel_url too so existing readers see it.
        {
            let db = app.state::<crate::types::HanniDb>();
            let conn = db.conn();
            let _ = conn.execute(
                "INSERT INTO app_settings (key, value) VALUES ('share_tunnel_url', ?1) \
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                rusqlite::params![url],
            );
        }
        let _ = app.emit("share-tunnel", serde_json::json!({
            "url": &url, "source": "tailscale-funnel"
        }));
        let _ = app.emit("tunnel-up", url.clone());
        return Ok(url);
    }

    // Tailscale: stable peer-to-peer URL for the user's own devices. We
    // record it as `share_tailscale_url` so clients in the same tailnet
    // can skip the public tunnel, but we DO NOT return early — external
    // guests (no Tailscale) still need the public cloudflared endpoint.
    if let Some(ts_url) = detect_tailscale_url(port) {
        eprintln!("[tunnel] Tailscale URL detected (internal): {}", ts_url);
        let db = app.state::<crate::types::HanniDb>();
        let conn = db.conn();
        let _ = conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('share_tailscale_url', ?1) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![ts_url],
        );
        let _ = app.emit("share-tunnel", serde_json::json!({
            "url": &ts_url, "source": "tailscale"
        }));
    }

    let bin = find_cloudflared()
        .ok_or_else(|| "cloudflared not installed. Run: brew install cloudflared".to_string())?;

    eprintln!("[tunnel] spawning cloudflared for 127.0.0.1:{}", port);
    let mut cmd = Command::new(&bin);
    cmd.args([
        "tunnel",
        "--no-autoupdate",
        "--url", &format!("http://127.0.0.1:{}", port),
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    // CRITICAL: without this the cloudflared subprocess survives Hanni
    // quit, so each restart leaks a tunnel (and the next Hanni start
    // can't tell which trycloudflare URL is "ours"). kill_on_drop ties
    // the child's lifetime to the Child handle inside ShareTunnel.
    .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {}", e))?;
    let stderr = child.stderr.take().ok_or("no stderr")?;

    // Parse trycloudflare URL from stderr. Cloudflared prints it within ~5s.
    let re = regex::Regex::new(r"https://[a-zA-Z0-9-]+\.trycloudflare\.com")
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stderr).lines();
    // 429 from trycloudflare = our IP is being rate-limited by Cloudflare
    // (typically after a burst of restarts). Capture it so we can surface a
    // real cause instead of the generic "tunnel did not produce a URL".
    let mut saw_429 = false;
    let url = {
        let mut found: Option<String> = None;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let remaining = match deadline.checked_duration_since(tokio::time::Instant::now()) {
                Some(d) => d,
                None => break,
            };
            let line = tokio::time::timeout(remaining, reader.next_line()).await;
            match line {
                Ok(Ok(Some(l))) => {
                    if l.contains("429") || l.contains("Too Many Requests")
                        || l.contains("error code: 1015") {
                        saw_429 = true;
                    }
                    if let Some(m) = re.find(&l) {
                        found = Some(m.as_str().to_string());
                        break;
                    }
                }
                _ => break,
            }
        }
        found
    };

    match url {
        Some(u) => {
            eprintln!("[tunnel] live at {}", u);
            // Drain remaining stderr in the background so the pipe doesn't fill
            // up and stall cloudflared. (tokio::process::Child does not auto-reap.)
            tokio::spawn(async move {
                let mut r = reader;
                while let Ok(Some(_)) = r.next_line().await {}
            });
            let st = app.state::<ShareTunnel>();
            let mut guard = st.0.lock().map_err(|e| e.to_string())?;
            guard.running = true;
            guard.url = Some(u.clone());
            guard.error = None;
            guard.child = Some(child);
            drop(guard);
            // Persist tunnel URL + mark share_links dirty so the mirror loop
            // pushes the new URL into Firestore — guests on Firebase Hosting
            // read it from share_links/{token}.tunnel_url to know where to
            // POST writes. Also read gist credentials for the always-fresh
            // pointer push below.
            let gist: Option<(String, String)> = {
                let db = app.state::<crate::types::HanniDb>();
                let conn = db.conn();
                let _ = conn.execute(
                    "INSERT INTO app_settings (key, value) VALUES ('share_tunnel_url', ?1) \
                     ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    rusqlite::params![u],
                );
                crate::sync_share::mark_dirty(&conn, "share_links");
                let id: Option<String> = conn.query_row(
                    "SELECT value FROM app_settings WHERE key='share_gist_id'",
                    [], |r| r.get(0)).ok();
                let tok: Option<String> = conn.query_row(
                    "SELECT value FROM app_settings WHERE key='share_gist_token'",
                    [], |r| r.get(0)).ok();
                match (id, tok) {
                    (Some(i), Some(t)) if !i.is_empty() && !t.is_empty() => Some((i, t)),
                    _ => None,
                }
            };
            // Public gist pointer: gives guests a path to recover the live
            // tunnel even when Firestore quota is exhausted. Best-effort —
            // failures don't block tunnel startup.
            if let Some((gist_id, token)) = gist {
                let url_for_push = u.clone();
                tokio::spawn(async move {
                    if let Err(e) = push_tunnel_to_gist(&gist_id, &token, &url_for_push).await {
                        eprintln!("[tunnel] gist push failed: {}", e);
                    } else {
                        eprintln!("[tunnel] gist updated with {}", url_for_push);
                    }
                });
            }
            let _ = app.emit("tunnel-up", u.clone());
            Ok(u)
        }
        None => {
            let _ = child.kill().await;
            let msg = if saw_429 {
                "Cloudflare ограничивает trycloudflare (429). Подожди 15–30 минут, потом попробуй снова. Или поставь named tunnel.".to_string()
            } else {
                "cloudflared did not produce a public URL within 20s".to_string()
            };
            // Surface the cause so tunnel_status reports it accurately.
            let st = app.state::<ShareTunnel>();
            if let Ok(mut g) = st.0.lock() { g.error = Some(msg.clone()); }
            Err(msg)
        }
    }
}

// Push the live tunnel URL into a public GitHub gist so guests can recover
// it client-side without dragging Firestore reads. The gist has a single
// file `hanni-tunnel.json` with shape {"tunnel":"...","updated_at":"..."}.
// Guest fetches the latest-raw URL — that endpoint always serves the
// freshest revision (no SHA pinning).
async fn push_tunnel_to_gist(gist_id: &str, token: &str, url: &str) -> Result<(), String> {
    let payload = serde_json::json!({
        "files": {
            "hanni-tunnel.json": {
                "content": serde_json::json!({
                    "tunnel": url,
                    "updated_at": chrono::Utc::now().to_rfc3339(),
                }).to_string()
            }
        }
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;
    let resp = client.patch(&format!("https://api.github.com/gists/{}", gist_id))
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Hanni")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&payload)
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("gist PATCH {}: {}", resp.status(),
            resp.text().await.unwrap_or_default()));
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn shutdown(app: &AppHandle) {
    let st = app.state::<ShareTunnel>();
    let child_opt = {
        let mut guard = match st.0.lock() { Ok(g) => g, Err(e) => e.into_inner() };
        guard.running = false;
        guard.url = None;
        guard.child.take()
    };
    if let Some(mut child) = child_opt {
        let _ = child.kill().await;
    }
}
