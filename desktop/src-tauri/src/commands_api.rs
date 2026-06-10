// commands_api.rs — HTTP API server, auto-eval roundtrip, API token, automation log
use crate::types::*;
use crate::chat::chat_inner;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use std::process::Command;
use std::path::PathBuf;
use std::collections::HashMap;

/// Global callback map for auto_eval HTTP → JS → Rust roundtrip
pub struct AutoEvalCallbacks(pub std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>);

#[tauri::command]
pub fn auto_eval_callback(cb_id: String, result: String, state: tauri::State<'_, AutoEvalCallbacks>) {
    if let Some(tx) = state.0.lock().unwrap().remove(&cb_id) {
        let _ = tx.send(result);
    }
}

// ── HTTP API Server ──
// ── Phase 4: HTTP API ──

pub fn api_token_path() -> PathBuf {
    hanni_data_dir().join("api_token.txt")
}

/// Write a secret to `path` with owner-only perms applied atomically at
/// creation (mode 0600), closing the brief world-readable window the old
/// write-then-chmod sequence left. set_permissions still runs to repair any
/// pre-existing file created before this change.
fn write_secret_file(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.write_all(contents.as_bytes())?;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        Ok(())
    }
    #[cfg(not(unix))]
    { std::fs::write(path, contents) }
}

/// Replace the API token file with a fresh UUID. The running server keeps
/// the old token in memory, so a process restart is required for the new
/// one to take effect. Returns the new token so the UI can show it once.
#[tauri::command]
pub fn rotate_api_token() -> Result<String, String> {
    let path = api_token_path();
    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
    }
    write_secret_file(&path, &token).map_err(|e| format!("write: {}", e))?;
    Ok(token)
}

/// Returns the current API token (first 8 chars + ellipsis) for display
/// in Settings. We don't ship the full token to JS to keep it out of
/// devtools history — the UI only needs a preview to confirm rotation.
#[tauri::command]
pub fn get_api_token_preview() -> Result<String, String> {
    let path = api_token_path();
    let token = std::fs::read_to_string(&path).map_err(|e| format!("read: {}", e))?;
    let token = token.trim();
    if token.len() < 8 { return Ok(token.to_string()); }
    Ok(format!("{}…", &token[..8]))
}

#[derive(serde::Serialize)]
pub struct AutomationLogRow {
    pub id: i64,
    pub ts: i64,
    pub script_hash: String,
    pub script_preview: String,
    pub success: bool,
    pub duration_ms: i64,
}

#[tauri::command]
pub fn list_automation_log(limit: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<AutomationLogRow>, String> {
    let conn = db.conn();
    let lim = limit.unwrap_or(100).clamp(1, 1000);
    let mut stmt = conn.prepare(
        "SELECT id, ts, script_hash, script_preview, success, duration_ms
         FROM automation_log ORDER BY ts DESC LIMIT ?1"
    ).map_err(|e| format!("prepare: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![lim], |r| {
        Ok(AutomationLogRow {
            id: r.get(0)?,
            ts: r.get(1)?,
            script_hash: r.get(2)?,
            script_preview: r.get(3)?,
            success: r.get::<_, i64>(4)? != 0,
            duration_ms: r.get(5)?,
        })
    }).map_err(|e| format!("query: {}", e))?;
    let out: Vec<_> = rows.flatten().collect();
    Ok(out)
}

pub fn get_or_create_api_token() -> String {
    let path = api_token_path();
    if path.exists() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = write_secret_file(&path, &token);
    token
}

/// Shared state of the local HTTP API (:8236 dev / :8235 prod).
/// Module-level so route handlers can live in other modules (api_jobs.rs).
#[derive(Clone)]
pub struct ApiState {
    pub app: AppHandle,
    pub token: String,
    // (count, window_start_epoch_secs) keyed by source IP.
    pub rate_limit: std::sync::Arc<std::sync::Mutex<HashMap<String, (u32, i64)>>>,
}

pub fn check_auth(headers: &axum::http::HeaderMap, token: &str) -> Result<(), (axum::http::StatusCode, String)> {
    use subtle::ConstantTimeEq;
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let provided = auth.strip_prefix("Bearer ").unwrap_or(auth);
    // Constant-time compare so byte-by-byte timing can't be used to
    // brute-force the token. Length mismatch short-circuits because
    // ct_eq panics on unequal slices — we want a stable false instead.
    let ok = provided.len() == token.len()
        && bool::from(provided.as_bytes().ct_eq(token.as_bytes()));
    if ok {
        Ok(())
    } else {
        Err((axum::http::StatusCode::UNAUTHORIZED, "Invalid token".into()))
    }
}

pub async fn spawn_api_server(app_handle: AppHandle) {
    use axum::{Router, routing::{get, post}, extract::{State as AxumState, Query, DefaultBodyLimit}, Json, http::{StatusCode, HeaderMap}};
    use std::sync::{Arc, Mutex};

    let api_token = get_or_create_api_token();

    // /auto/eval body cap: a single eval script over this size is almost
    // certainly malicious or a runaway log dump. 256 KiB matches the
    // share-server body limit (share_auth::BODY_LIMIT_BYTES).
    const AUTO_EVAL_BODY_LIMIT: usize = 256 * 1024;
    // Rate-limit per IP: same posture as share_auth::RATE_LIMIT_PER_MINUTE.
    // Server is loopback-only so the IP is effectively always 127.0.0.1,
    // but keying by IP keeps the door open for future tunneling.
    const AUTO_EVAL_RATE_PER_MINUTE: u32 = 100;
    // Retention: automation_log rows older than this are pruned lazily.
    const AUTO_LOG_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;

    let state = ApiState {
        app: app_handle.clone(),
        token: api_token,
        rate_limit: Arc::new(Mutex::new(HashMap::new())),
    };

    // Background retention: prune automation_log once an hour. Kept lazy
    // (no separate scheduler crate) — a single task per server lifetime.
    {
        let app = app_handle.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                tick.tick().await;
                let cutoff = chrono::Utc::now().timestamp() - AUTO_LOG_RETENTION_SECS;
                let db = app.state::<HanniDb>();
                let _ = db.conn().execute(
                    "DELETE FROM automation_log WHERE ts < ?1",
                    rusqlite::params![cutoff],
                );
            }
        });
    }

    fn rate_limit_check(state: &ApiState, key: &str) -> Result<(), (StatusCode, String)> {
        let now = chrono::Utc::now().timestamp();
        let mut map = state.rate_limit.lock().unwrap();
        // Bound map growth so spammed distinct keys can't exhaust memory
        // (matches share_auth::rate_limit_check cap).
        if map.len() > 10_000 {
            map.retain(|_, (_, started)| now - *started < 60);
        }
        let entry = map.entry(key.to_string()).or_insert((0, now));
        if now - entry.1 >= 60 { *entry = (0, now); }
        entry.0 += 1;
        if entry.0 > AUTO_EVAL_RATE_PER_MINUTE {
            Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".into()))
        } else { Ok(()) }
    }

    fn log_automation(app: &AppHandle, script: &str, success: bool, duration_ms: i64) {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(script.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let preview: String = script.chars().take(200).collect();
        let ts = chrono::Utc::now().timestamp();
        let db = app.state::<HanniDb>();
        let _ = db.conn().execute(
            "INSERT INTO automation_log (ts, script_hash, script_preview, success, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![ts, hash, preview, success as i64, duration_ms],
        );
    }

    #[derive(Deserialize)]
    struct ChatReq {
        message: String,
        history: Option<Vec<serde_json::Value>>,
    }

    #[derive(Deserialize)]
    struct SearchQuery {
        q: String,
        limit: Option<usize>,
    }

    #[derive(Deserialize)]
    struct RememberReq {
        category: String,
        key: String,
        value: String,
    }

    pub async fn api_status(
        AxumState(state): AxumState<ApiState>,
    ) -> Json<serde_json::Value> {
        // No auth required for status — allows frontend health check
        let busy = state.app.state::<LlmBusy>().0.available_permits() == 0;
        let focus_active = state.app.state::<FocusManager>().0.lock().unwrap_or_else(|e| e.into_inner()).active;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let model_online = client
            .get(llm_models_url())
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        Json(serde_json::json!({
            "status": "ok",
            "model_online": model_online,
            "llm_busy": busy,
            "focus_active": focus_active,
        }))
    }

    pub async fn api_chat(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<ChatReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let mut messages = req.history.unwrap_or_default();
        messages.push(serde_json::json!({"role": "user", "content": req.message}));

        match chat_inner(&state.app, messages, false).await {
            Ok(result) => Ok(Json(serde_json::json!({ "reply": result.text, "tool_calls": result.tool_calls }))),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
        }
    }

    pub async fn api_memory_search(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Query(params): Query<SearchQuery>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let max = params.limit.unwrap_or(20) as i64;

        let words: Vec<&str> = params.q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
        let mut results = Vec::new();

        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        if results.is_empty() {
            let like_pattern = format!("%{}%", params.q);
            if let Ok(mut stmt) = conn.prepare(
                "SELECT category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        Ok(Json(serde_json::json!({ "results": results })))
    }

    pub async fn api_memory_add(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<RememberReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO facts (category, key, value, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'api', ?4, ?4)
             ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
            rusqlite::params![req.category, req.key, req.value, now],
        ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

        Ok(Json(serde_json::json!({ "status": "ok" })))
    }

    // ── Automation endpoints (eval JS in WebView, works even minimized) ──

    #[derive(Deserialize)]
    struct EvalReq {
        script: String,
    }

    pub async fn auto_eval(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<EvalReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;
        // Loopback-only server: a single rate-limit bucket is enough.
        // If we ever expose this beyond 127.0.0.1, switch to per-IP keying.
        rate_limit_check(&state, "loopback")?;

        let started = std::time::Instant::now();
        let script = req.script.clone();

        let cb_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();

        // Register callback in global map
        state.app.state::<AutoEvalCallbacks>()
            .0.lock().unwrap()
            .insert(cb_id.clone(), tx);

        // Wrap script to invoke Tauri command with result
        let wrapped = format!(
            r#"(async () => {{
                try {{
                    const __r = await (async () => {{ {script} }})();
                    await window.__TAURI__.core.invoke('auto_eval_callback', {{ cbId: '{cb_id}', result: JSON.stringify(__r ?? null) }});
                }} catch(e) {{
                    await window.__TAURI__.core.invoke('auto_eval_callback', {{ cbId: '{cb_id}', result: JSON.stringify({{ __error: e.message }}) }});
                }}
            }})()"#,
            script = req.script, cb_id = cb_id
        );

        if let Some(win) = state.app.get_webview_window("main") {
            if let Err(e) = win.eval(&wrapped) {
                let dur = started.elapsed().as_millis() as i64;
                log_automation(&state.app, &script, false, dur);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("eval error: {}", e)));
            }
        } else {
            let dur = started.elapsed().as_millis() as i64;
            log_automation(&state.app, &script, false, dur);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "No main webview found".into()));
        }

        let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), rx).await;
        let dur = started.elapsed().as_millis() as i64;
        match outcome {
            Ok(Ok(result)) => {
                let inner: serde_json::Value = serde_json::from_str(&result)
                    .unwrap_or(serde_json::Value::String(result));
                let script_failed = inner.get("__error").is_some();
                log_automation(&state.app, &script, !script_failed, dur);
                Ok(Json(serde_json::json!({ "result": inner })))
            }
            Ok(Err(_)) => {
                log_automation(&state.app, &script, false, dur);
                Err((StatusCode::INTERNAL_SERVER_ERROR, "Channel closed".into()))
            }
            Err(_) => {
                log_automation(&state.app, &script, false, dur);
                Err((StatusCode::REQUEST_TIMEOUT, "Script timed out after 10s".into()))
            }
        }
    }

    #[derive(Deserialize)]
    struct OauthCallback {
        code:  Option<String>,
        state: Option<String>,
        error: Option<String>,
    }

    pub async fn google_oauth_callback(
        AxumState(state): AxumState<ApiState>,
        Query(q): Query<OauthCallback>,
    ) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], String) {
        // No auth header — Google's redirect can't carry our Bearer token.
        // We rely on the random `state` param (CSRF-protection) inside the handler.
        let html_ok = "<html><body style='font-family:-apple-system,sans-serif;padding:40px;text-align:center'>\
            <h2>✓ Signed in to Hanni</h2>\
            <p>You can close this tab and return to the app.</p>\
            <script>setTimeout(()=>window.close(),1500)</script></body></html>";
        let ct = (axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8");

        // Escape reflected values — `error` comes straight from the redirect
        // query string (attacker-controllable), so raw interpolation is XSS.
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        if let Some(err) = q.error {
            return (StatusCode::BAD_REQUEST, [ct],
                format!("<h2>OAuth error</h2><pre>{}</pre>", esc(&err)));
        }
        let (code, st) = match (q.code, q.state) {
            (Some(c), Some(s)) => (c, s),
            _ => return (StatusCode::BAD_REQUEST, [ct],
                "<h2>Missing code or state</h2>".into()),
        };
        let db = state.app.state::<HanniDb>();
        match crate::google_auth::handle_oauth_callback(&db, &state.app, &code, &st).await {
            Ok(_) => (StatusCode::OK, [ct], html_ok.into()),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, [ct],
                format!("<h2>Sign-in failed</h2><pre>{}</pre>", esc(&e))),
        }
    }

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/chat", post(api_chat))
        .route("/api/memory/search", get(api_memory_search))
        .route("/api/memory", post(api_memory_add))
        .route("/api/vacancy", get(crate::api_jobs::api_vacancy_lookup).post(crate::api_jobs::api_vacancy_save))
        .route(
            "/auto/eval",
            post(auto_eval).layer(DefaultBodyLimit::max(AUTO_EVAL_BODY_LIMIT)),
        )
        .route("/oauth/google/callback", get(google_oauth_callback))
        .with_state(state);

    let port = if cfg!(debug_assertions) { 8236 } else { 8235 };
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await;
    match listener {
        Ok(listener) => {
            let _ = axum::serve(listener, app).await;
        }
        Err(e) => {
            eprintln!("Failed to start API server: {}", e);
        }
    }
}

pub fn find_python() -> Option<String> {
    // Try common locations for python3 with mlx_lm
    let candidates = [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/usr/bin/python3",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

const VOICE_SERVER_URL: &str = "http://127.0.0.1:8237";

pub fn escape_plist_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn ensure_voice_server_launchagent() {
    let python = match find_python() {
        Some(p) => p,
        None => { eprintln!("[voice] No python3 found"); return; }
    };

    // Extract embedded voice_server.py to data dir (always overwrite to keep in sync with binary)
    let script = hanni_data_dir().join("voice_server.py");
    let embedded = include_str!("../../voice_server.py");
    if let Err(e) = std::fs::write(&script, embedded) {
        eprintln!("[voice] Failed to write voice_server.py: {}", e);
        return;
    }

    let log_path = hanni_data_dir().join("voice_server.log");
    let plist_path = match dirs::home_dir() {
        Some(h) => h.join("Library/LaunchAgents/com.hanni.voice-server.plist"),
        None => { eprintln!("[voice] Cannot determine home dir"); return; }
    };
    // XML-escape all interpolated paths to prevent plist injection
    let python_esc = escape_plist_xml(&python);
    let script_esc = escape_plist_xml(&script.to_string_lossy());
    let log_esc = escape_plist_xml(&log_path.to_string_lossy());

    let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>com.hanni.voice-server</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>{}</string>
	</array>
	<key>KeepAlive</key>
	<true/>
	<key>RunAtLoad</key>
	<true/>
	<key>StandardErrorPath</key>
	<string>{}</string>
	<key>StandardOutPath</key>
	<string>{}</string>
</dict>
</plist>"#, python_esc, script_esc, log_esc, log_esc);

    // Check if plist already exists with same content
    let needs_update = match std::fs::read_to_string(&plist_path) {
        Ok(existing) => existing != plist_content,
        Err(_) => true,
    };

    if needs_update {
        // Unload old version if exists
        let _ = Command::new("launchctl").args(["unload", &plist_path.to_string_lossy()]).output();
        if let Err(e) = std::fs::write(&plist_path, &plist_content) {
            eprintln!("[voice] Failed to write LaunchAgent: {}", e);
            return;
        }
        let _ = Command::new("launchctl").args(["load", &plist_path.to_string_lossy()]).output();
        eprintln!("[voice] LaunchAgent installed and loaded");
    } else {
        // Just make sure it's running
        let check = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build();
        let running = check.ok()
            .and_then(|c| c.get(&format!("{}/health", VOICE_SERVER_URL)).send().ok())
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if !running {
            let _ = Command::new("launchctl").args(["unload", &plist_path.to_string_lossy()]).output();
            let _ = Command::new("launchctl").args(["load", &plist_path.to_string_lossy()]).output();
            eprintln!("[voice] LaunchAgent reloaded");
        } else {
            eprintln!("[voice] LaunchAgent already running");
        }
    }
}
