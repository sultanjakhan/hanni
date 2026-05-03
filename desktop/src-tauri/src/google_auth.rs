// google_auth.rs — Sign in with Google → Firebase Auth (Stage C.1).
//
// Replaces the service-account JWT flow for owner-side cloud sync.
// The user clicks "Sign in with Google" once per device; we open the
// system browser with an OAuth URL pointing at our local HTTP server
// (`/oauth/google/callback` on 8235 prod / 8236 dev). After Google
// redirects back with an auth code, we:
//   1. exchange code → Google id_token (oauth2.googleapis.com/token)
//   2. exchange Google id_token → Firebase id_token (identitytoolkit signInWithIdp)
//   3. persist {id_token, refresh_token, uid, email, expires_at}
//
// `get_firebase_id_token()` is the single entry point used by sync
// code: it returns a valid id_token, refreshing transparently when
// the cached one is within 60s of expiry.

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
    pub firebase_id_token: String,
    pub firebase_refresh_token: String,
    pub expires_at: i64,
    pub uid: String,
    pub email: String,
    /// Google OAuth access token with `cloud-platform` scope — used for
    /// Firestore Admin / Firebase Management REST APIs (database setup,
    /// rules deployment). Distinct from `firebase_id_token` which is for
    /// Firestore data access under Auth rules.
    #[serde(default)]
    pub google_access_token: String,
    #[serde(default)]
    pub google_refresh_token: String,
    #[serde(default)]
    pub google_expires_at: i64,
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

    let client = reqwest::Client::new();

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
        .ok_or_else(|| format!("no Google id_token in response: {}", google_resp))?;
    let google_access_token = google_resp.get("access_token").and_then(|v| v.as_str())
        .unwrap_or("").to_string();
    let google_refresh_token = google_resp.get("refresh_token").and_then(|v| v.as_str())
        .unwrap_or("").to_string();
    let google_expires_in: i64 = google_resp.get("expires_in").and_then(|v| v.as_i64())
        .unwrap_or(3600);

    // 2. Exchange Google ID token → Firebase ID token via signInWithIdp
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

    let id_token = fb_resp.get("idToken").and_then(|v| v.as_str())
        .ok_or_else(|| format!("no firebase idToken: {}", fb_resp))?;
    let refresh_token = fb_resp.get("refreshToken").and_then(|v| v.as_str())
        .ok_or_else(|| "no firebase refreshToken".to_string())?;
    let local_id = fb_resp.get("localId").and_then(|v| v.as_str())
        .ok_or_else(|| "no localId in firebase response".to_string())?;
    let email = fb_resp.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let expires_in: i64 = fb_resp.get("expiresIn").and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok()).unwrap_or(3600);

    let session = GoogleAuthSession {
        firebase_id_token: id_token.to_string(),
        firebase_refresh_token: refresh_token.to_string(),
        expires_at: now_secs() + expires_in,
        uid: local_id.to_string(),
        email: email.clone(),
        google_access_token,
        google_refresh_token,
        google_expires_at: now_secs() + google_expires_in,
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

// ── Used by sync_owner ────────────────────────────────────────────────────

/// Returns a valid Google OAuth access token (cloud-platform scope),
/// refreshing via refresh_token if necessary. Used for Firestore Admin /
/// Firebase Management REST APIs.
pub async fn get_google_access_token(db: &HanniDb) -> Result<String, String> {
    let (cfg, mut session) = {
        let conn = db.conn();
        let cfg = load_config(&conn).ok_or_else(|| "Google Auth not configured".to_string())?;
        let session = load_session(&conn).ok_or_else(|| "Not signed in with Google".to_string())?;
        (cfg, session)
    };
    if session.google_access_token.is_empty() || session.google_refresh_token.is_empty() {
        return Err("Google access token missing — sign in again to grant cloud-platform scope".into());
    }
    if session.google_expires_at > now_secs() + 60 {
        return Ok(session.google_access_token);
    }
    let body = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        enc(&cfg.client_id), enc(&cfg.client_secret), enc(&session.google_refresh_token),
    );
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client.post("https://oauth2.googleapis.com/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send().await.map_err(|e| format!("google refresh: {}", e))?
        .json().await.map_err(|e| format!("google refresh parse: {}", e))?;
    let new_token = resp.get("access_token").and_then(|v| v.as_str())
        .ok_or_else(|| format!("google refresh: no access_token: {}", resp))?;
    let expires_in: i64 = resp.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(3600);
    session.google_access_token = new_token.to_string();
    session.google_expires_at = now_secs() + expires_in;
    save_session(&db.conn(), &session)?;
    Ok(session.google_access_token)
}

/// Returns (firebase_id_token, uid, project_id), refreshing the token
/// transparently if it's within 60 seconds of expiry.
pub async fn get_firebase_id_token(db: &HanniDb) -> Result<(String, String, String), String> {
    let (cfg, mut session) = {
        let conn = db.conn();
        let cfg = load_config(&conn).ok_or_else(|| "Google Auth not configured".to_string())?;
        let session = load_session(&conn).ok_or_else(|| "Not signed in with Google".to_string())?;
        (cfg, session)
    };

    if session.expires_at <= now_secs() + 60 {
        let url = format!("https://securetoken.googleapis.com/v1/token?key={}", cfg.api_key);
        let client = reqwest::Client::new();
        let body = format!("grant_type=refresh_token&refresh_token={}",
                           enc(&session.firebase_refresh_token));
        let resp: serde_json::Value = client.post(&url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send().await.map_err(|e| format!("token refresh: {}", e))?
            .json().await.map_err(|e| format!("refresh parse: {}", e))?;

        let new_id = resp.get("id_token").and_then(|v| v.as_str())
            .ok_or_else(|| format!("refresh: no id_token: {}", resp))?;
        let new_refresh = resp.get("refresh_token").and_then(|v| v.as_str())
            .unwrap_or(&session.firebase_refresh_token).to_string();
        let expires_in: i64 = resp.get("expires_in").and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok()).unwrap_or(3600);

        session.firebase_id_token = new_id.to_string();
        session.firebase_refresh_token = new_refresh;
        session.expires_at = now_secs() + expires_in;

        let conn = db.conn();
        save_session(&conn, &session)?;
    }

    Ok((session.firebase_id_token, session.uid, cfg.project_id))
}
