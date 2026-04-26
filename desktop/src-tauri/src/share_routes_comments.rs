// share_routes_comments.rs — GET + POST /recipes/:id/comments

use axum::{
    extract::{Path, State as AxumState, ConnectInfo},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use std::net::SocketAddr;
use tauri::Manager;

use crate::share_auth::{
    load_link, require_perm, rate_limit_check, log_activity, ua_ip, BODY_LIMIT_BYTES,
};
use crate::share_server::ShareServerState;
use crate::types::HanniDb;

pub async fn list_comments(
    Path((token, id)): Path<(String, i64)>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    let mut stmt = conn.prepare(
        "SELECT id, author, text, created_at FROM share_comments
         WHERE entity_type='recipe' AND entity_id=?1 ORDER BY id DESC LIMIT 100"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let iter = stmt.query_map(rusqlite::params![id], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "author": r.get::<_, String>(1)?,
            "text": r.get::<_, String>(2)?,
            "created_at": r.get::<_, String>(3)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = iter.filter_map(|r| r.ok()).collect();
    Ok(Json(serde_json::json!({ "comments": rows })))
}

#[derive(Deserialize)]
struct CommentReq { text: String, author: Option<String> }

pub async fn create_comment(
    Path((token, id)): Path<(String, i64)>,
    AxumState(state): AxumState<ShareServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    if body.len() > BODY_LIMIT_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "Body too large".into()));
    }
    let req: CommentReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    let text = req.text.trim();
    if text.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "text is required".into()));
    }
    if text.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "text too long".into()));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "comment")?;
    let author = req.author.as_deref().unwrap_or("Guest").trim();
    let author = if author.is_empty() { "Guest" } else { author };
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO share_comments (link_id, entity_type, entity_id, author, text, created_at)
         VALUES (?1,'recipe',?2,?3,?4,?5)",
        rusqlite::params![ctx.id, id, author, text, now],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let comment_id = conn.last_insert_rowid();
    log_activity(&conn, ctx.id, "comment",
        &serde_json::json!({ "recipe_id": id, "comment_id": comment_id }).to_string(),
        &ip, &ua);
    Ok(Json(serde_json::json!({ "status": "ok", "id": comment_id })))
}
