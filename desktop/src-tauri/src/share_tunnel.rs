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
    .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {}", e))?;
    let stderr = child.stderr.take().ok_or("no stderr")?;

    // Parse trycloudflare URL from stderr. Cloudflared prints it within ~5s.
    let re = regex::Regex::new(r"https://[a-zA-Z0-9-]+\.trycloudflare\.com")
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stderr).lines();
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
            // POST writes.
            {
                let db = app.state::<crate::types::HanniDb>();
                let conn = db.conn();
                let _ = conn.execute(
                    "INSERT INTO app_settings (key, value) VALUES ('share_tunnel_url', ?1) \
                     ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    rusqlite::params![u],
                );
                crate::sync_share::mark_dirty(&conn, "share_links");
            }
            let _ = app.emit("tunnel-up", u.clone());
            Ok(u)
        }
        None => {
            let _ = child.kill().await;
            Err("cloudflared did not produce a public URL within 20s".into())
        }
    }
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
