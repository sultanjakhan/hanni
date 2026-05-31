// google_auth.rs — Sign in with Google (Stage C.1).
//
// Provides a stable per-user identity for owner-side cloud sync. The
// user clicks "Sign in with Google" once per device; we open the system
// browser with an OAuth URL pointing at our local HTTP server
// (`/oauth/google/callback` on 8235 prod / 8236 dev). After Google
// redirects back with an auth code, we:
//   1. exchange code → Google id_token (oauth2.googleapis.com/token)
//   2. resolve a stable localId/email from it (identitytoolkit signInWithIdp)
//   3. persist {uid, email, expires_at}
//
// sync_owner reads only `uid` (via load_session) + `project_id` (via
// load_config) to scope its documents.

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::types::HanniDb;

const SETTING_CONFIG: &str = "google_auth_config";
const SETTING_SESSION: &str = "google_auth_session";
const SETTING_PENDING_STATE: &str = "google_auth_pending_state";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub project_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAuthSession {
    pub expires_at: i64,
    pub uid: String,
    pub email: String,
}

fn get_setting(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![key], |r| r.get(0)).ok()
}

fn set_setting(conn: &rusqlite::Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    );
}

fn del_setting(conn: &rusqlite::Connection, key: &str) {
    let _ = conn.execute("DELETE FROM app_settings WHERE key=?1", rusqlite::params![key]);
}

pub fn load_config(conn: &rusqlite::Connection) -> Option<GoogleAuthConfig> {
    get_setting(conn, SETTING_CONFIG).and_then(|s| serde_json::from_str(&s).ok())
}

pub fn load_session(conn: &rusqlite::Connection) -> Option<GoogleAuthSession> {
    get_setting(conn, SETTING_SESSION).and_then(|s| serde_json::from_str(&s).ok())
}

fn save_session(conn: &rusqlite::Connection, sess: &GoogleAuthSession) -> Result<(), String> {
    let json = serde_json::to_string(sess).map_err(|e| e.to_string())?;
    set_setting(conn, SETTING_SESSION, &json);
    Ok(())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

pub fn redirect_uri() -> String {
    let port = if cfg!(debug_assertions) { 8236 } else { 8235 };
    format!("http://127.0.0.1:{}/oauth/google/callback", port)
}

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

/// Shared HTTP client with a 30s timeout. Without one, a slow Google
/// endpoint would hang the sign-in flow indefinitely.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

// ── Tauri commands ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GoogleAuthStatus {
    pub configured: bool,
    pub authenticated: bool,
    pub email: Option<String>,
    pub uid: Option<String>,
    pub expires_at: Option<i64>,
    pub project_id: Option<String>,
    pub redirect_uri: String,
}

#[tauri::command]
pub fn google_auth_status(db: State<'_, HanniDb>) -> GoogleAuthStatus {
    let conn = db.conn();
    let cfg = load_config(&conn);
    let session = load_session(&conn);
    GoogleAuthStatus {
        configured: cfg.is_some(),
        authenticated: session.is_some(),
        email: session.as_ref().map(|s| s.email.clone()),
        uid: session.as_ref().map(|s| s.uid.clone()),
        expires_at: session.as_ref().map(|s| s.expires_at),
        project_id: cfg.map(|c| c.project_id),
        redirect_uri: redirect_uri(),
    }
}

#[tauri::command]
pub fn google_auth_set_config(
    client_id: String,
    client_secret: String,
    project_id: String,
    api_key: String,
    db: State<'_, HanniDb>,
) -> Result<(), String> {
    let cfg = GoogleAuthConfig { client_id, client_secret, project_id, api_key };
    let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    set_setting(&db.conn(), SETTING_CONFIG, &json);
    Ok(())
}

#[tauri::command]
pub fn google_auth_signout(db: State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    del_setting(&conn, SETTING_SESSION);
    del_setting(&conn, SETTING_PENDING_STATE);
    // Reset sync bookmarks — next sync starts fresh under whatever uid signs in next.
    let _ = conn.execute(
        "DELETE FROM app_settings WHERE key IN \
         ('cloud_owner_last_push_ver','cloud_owner_last_pull_ts')",
        [],
    );
    Ok(())
}

/// Returns the OAuth authorization URL. Caller (JS) opens it in the
/// system browser; user signs in; Google redirects to our callback.
#[tauri::command]
pub fn google_auth_start_signin(db: State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let cfg = load_config(&conn).ok_or_else(|| "Google Auth not configured".to_string())?;
    let state = uuid::Uuid::new_v4().to_string();
    set_setting(&conn, SETTING_PENDING_STATE, &state);
    // cloud-platform scope: Firestore Admin REST + Firebase Management REST.
    // Lets us create/configure Firestore databases & rules without UI.
    let scopes = "openid email profile https://www.googleapis.com/auth/cloud-platform";
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={cid}\
         &redirect_uri={ru}\
         &response_type=code\
         &scope={scope}\
         &state={state}\
         &access_type=offline\
         &prompt=consent",
        cid   = enc(&cfg.client_id),
        ru    = enc(&redirect_uri()),
        scope = enc(scopes),
        state = enc(&state),
    );
    Ok(url)
}

// ── HTTP callback handler (called from API server) ───────────────────────

pub async fn handle_oauth_callback(
    db: &HanniDb,
    app: &AppHandle,
    code: &str,
    state: &str,
) -> Result<(), String> {
    let cfg = {
        let conn = db.conn();
        let pending = get_setting(&conn, SETTING_PENDING_STATE)
            .ok_or_else(|| "no pending oauth state — start sign-in again".to_string())?;
        if pending != state { return Err("oauth state mismatch — replay or CSRF".into()); }
        load_config(&conn).ok_or_else(|| "Google Auth not configured".to_string())?
    };

    let client = http_client();

    // 1. Exchange auth code → Google ID token
    let body = format!(
        "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
        enc(code), enc(&cfg.client_id), enc(&cfg.client_secret), enc(&redirect_uri()),
    );
    let google_resp: serde_json::Value = client.post("https://oauth2.googleapis.com/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send().await.map_err(|e| format!("google token exchange: {}", e))?
        .json().await.map_err(|e| format!("google token parse: {}", e))?;
    let google_id_token = google_resp.get("id_token").and_then(|v| v.as_str())
        .ok_or_else(|| {
            eprintln!("[google_auth] code-exchange missing id_token: {}", google_resp);
            "Google sign-in failed — see app logs".to_string()
        })?;

    // 2. Resolve a stable localId/email via signInWithIdp
    let url = format!(
        "https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp?key={}",
        cfg.api_key
    );
    let body = serde_json::json!({
        "postBody": format!("id_token={}&providerId=google.com", google_id_token),
        "requestUri": "http://localhost",
        "returnSecureToken": true,
    });
    let fb_resp: serde_json::Value = client.post(&url).json(&body)
        .send().await.map_err(|e| format!("firebase signInWithIdp: {}", e))?
        .json().await.map_err(|e| format!("firebase parse: {}", e))?;

    let local_id = fb_resp.get("localId").and_then(|v| v.as_str())
        .ok_or_else(|| {
            eprintln!("[google_auth] signInWithIdp missing localId: {}", fb_resp);
            "Google sign-in failed — see app logs".to_string()
        })?;
    let email = fb_resp.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let expires_in: i64 = fb_resp.get("expiresIn").and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok()).unwrap_or(3600);

    let session = GoogleAuthSession {
        expires_at: now_secs() + expires_in,
        uid: local_id.to_string(),
        email: email.clone(),
    };

    {
        let conn = db.conn();
        save_session(&conn, &session)?;
        del_setting(&conn, SETTING_PENDING_STATE);
        // Reset bookmarks so first sync after sign-in re-pushes everything
        // under the new (Firebase) uid.
        let _ = conn.execute(
            "DELETE FROM app_settings WHERE key IN \
             ('cloud_owner_last_push_ver','cloud_owner_last_pull_ts')",
            [],
        );
    }

    let _ = app.emit("google-auth-changed", serde_json::json!({
        "authenticated": true, "email": email, "uid": local_id,
    }));
    Ok(())
}
