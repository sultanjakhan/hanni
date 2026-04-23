// share_server.rs — Public HTTP server for share-links (guest-facing)
// Runs on 127.0.0.1:8239 (prod) / 8240 (dev). Cloudflare Tunnel exposes it to the internet.

use axum::{
    extract::{Path, State as AxumState, ConnectInfo},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

use crate::types::HanniDb;

const RATE_LIMIT_PER_MINUTE: u32 = 100;
const BODY_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub struct ShareServerState {
    pub app: AppHandle,
    pub rate_limit: Arc<Mutex<HashMap<String, (u32, i64)>>>,
}

pub fn share_port() -> u16 {
    if cfg!(debug_assertions) { 8240 } else { 8239 }
}

pub async fn spawn_share_server(app_handle: AppHandle) {
    let state = ShareServerState {
        app: app_handle,
        rate_limit: Arc::new(Mutex::new(HashMap::new())),
    };

    // axum 0.8: path params use `{name}`, not `:name`.
    let app = Router::new()
        .route("/share/health", get(health))
        .route("/s/{token}", get(landing))
        .route("/s/{token}/recipes", get(list_recipes).post(create_recipe))
        .route("/s/{token}/recipes/{id}",
            get(get_recipe).patch(update_recipe))
        .route("/s/{token}/recipes/{id}/comments",
            get(list_comments).post(create_comment))
        .route("/s/{token}/assets/guest.css", get(asset_css))
        .route("/s/{token}/assets/guest.js", get(asset_js))
        .with_state(state);

    let port = share_port();
    match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(listener) => {
            eprintln!("[share] public server on 127.0.0.1:{}", port);
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            ).await;
        }
        Err(e) => eprintln!("[share] bind {} failed: {}", port, e),
    }
}

// ── Link lookup & auth (all DB ops take an existing &Connection to avoid re-locking the Mutex) ──

struct LinkCtx {
    id: i64,
    scope: String,
    permissions: Vec<String>,
    label: String,
    tab: String,
}

fn load_link(conn: &rusqlite::Connection, token: &str) -> Result<LinkCtx, (StatusCode, String)> {
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

fn require_perm(ctx: &LinkCtx, needed: &str) -> Result<(), (StatusCode, String)> {
    if ctx.permissions.iter().any(|p| p == needed) { Ok(()) } else {
        Err((StatusCode::FORBIDDEN, format!("Permission '{}' denied", needed)))
    }
}

fn rate_limit_check(state: &ShareServerState, token: &str) -> Result<(), (StatusCode, String)> {
    let now = chrono::Local::now().timestamp();
    let mut map = state.rate_limit.lock().unwrap();
    let entry = map.entry(token.to_string()).or_insert((0, now));
    if now - entry.1 >= 60 { *entry = (0, now); }
    entry.0 += 1;
    if entry.0 > RATE_LIMIT_PER_MINUTE {
        Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".into()))
    } else { Ok(()) }
}

fn log_activity(conn: &rusqlite::Connection, link_id: i64, action: &str, payload: &str, ip: &str, ua: &str) {
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

fn ua_ip(headers: &HeaderMap, addr: &SocketAddr) -> (String, String) {
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let ip = addr.ip().to_string();
    (ua, ip)
}

// ── Routes ──

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "share" }))
}

async fn landing(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Html<String>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let ctx = {
        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        load_link(&conn, &token)?
    };
    let html = include_str!("share_assets/guest.html")
        .replace("{{LABEL}}", &html_escape(&ctx.label))
        .replace("{{TAB}}", &html_escape(&ctx.tab))
        .replace("{{SCOPE}}", &html_escape(&ctx.scope))
        .replace("{{TOKEN}}", &html_escape(&token))
        .replace("{{PERMS}}", &serde_json::to_string(&ctx.permissions).unwrap_or("[]".into()));
    Ok(Html(html))
}

async fn list_recipes(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    if ctx.tab != "food" || !(ctx.scope == "all" || ctx.scope == "recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    let mut stmt = conn.prepare(
        "SELECT id, name, description, prep_time, cook_time, servings, calories, tags,
                difficulty, cuisine, protein, fat, carbs
         FROM recipes ORDER BY updated_at DESC LIMIT 50"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "description": r.get::<_, String>(2)?,
            "prep_time": r.get::<_, i64>(3)?,
            "cook_time": r.get::<_, i64>(4)?,
            "servings": r.get::<_, i64>(5)?,
            "calories": r.get::<_, i64>(6)?,
            "tags": r.get::<_, String>(7)?,
            "difficulty": r.get::<_, String>(8).unwrap_or_else(|_| "easy".into()),
            "cuisine": r.get::<_, String>(9).unwrap_or_else(|_| "kz".into()),
            "protein": r.get::<_, i64>(10).unwrap_or(0),
            "fat": r.get::<_, i64>(11).unwrap_or(0),
            "carbs": r.get::<_, i64>(12).unwrap_or(0),
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(stmt);
    log_activity(&conn, ctx.id, "view", "list_recipes", &ip, &ua);

    Ok(Json(serde_json::json!({
        "recipes": rows, "label": ctx.label, "permissions": ctx.permissions,
    })))
}

async fn get_recipe(
    Path((token, id)): Path<(String, i64)>,
    AxumState(state): AxumState<ShareServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    let recipe = conn.query_row(
        "SELECT id, name, description, ingredients, instructions, prep_time, cook_time,
                servings, calories, tags, difficulty, cuisine, protein, fat, carbs
         FROM recipes WHERE id=?1",
        rusqlite::params![id],
        |r| Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "description": r.get::<_, String>(2)?,
            "ingredients": r.get::<_, String>(3)?,
            "instructions": r.get::<_, String>(4)?,
            "prep_time": r.get::<_, i64>(5)?,
            "cook_time": r.get::<_, i64>(6)?,
            "servings": r.get::<_, i64>(7)?,
            "calories": r.get::<_, i64>(8)?,
            "tags": r.get::<_, String>(9)?,
            "difficulty": r.get::<_, String>(10).unwrap_or_else(|_| "easy".into()),
            "cuisine": r.get::<_, String>(11).unwrap_or_else(|_| "kz".into()),
            "protein": r.get::<_, i64>(12).unwrap_or(0),
            "fat": r.get::<_, i64>(13).unwrap_or(0),
            "carbs": r.get::<_, i64>(14).unwrap_or(0),
        }))
    ).map_err(|_| (StatusCode::NOT_FOUND, "Recipe not found".into()))?;

    log_activity(&conn, ctx.id, "view", &format!("recipe:{}", id), &ip, &ua);
    Ok(Json(recipe))
}

#[derive(Deserialize)]
struct CreateRecipeReq {
    name: String,
    description: Option<String>,
    ingredients: Option<String>,
    instructions: Option<String>,
    prep_time: Option<i64>,
    cook_time: Option<i64>,
    servings: Option<i64>,
    calories: Option<i64>,
    tags: Option<String>,
    difficulty: Option<String>,
    cuisine: Option<String>,
    protein: Option<i64>,
    fat: Option<i64>,
    carbs: Option<i64>,
    author: Option<String>,
}

async fn create_recipe(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    if body.len() > BODY_LIMIT_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "Body too large".into()));
    }
    let req: CreateRecipeReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".into()));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    if ctx.tab != "food" || !(ctx.scope == "all" || ctx.scope == "recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    let now = chrono::Local::now().to_rfc3339();
    let author_tag = req.author.as_deref().unwrap_or("guest");
    let prev_tags = req.tags.clone().unwrap_or_default();
    let tags = if prev_tags.is_empty() {
        format!("shared-by:{}", author_tag)
    } else {
        format!("{} shared-by:{}", prev_tags, author_tag)
    };
    conn.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time,
            servings, calories, tags, difficulty, cuisine, health_score, price_score,
            protein, fat, carbs, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,5,5,?12,?13,?14,?15,?15)",
        rusqlite::params![
            req.name, req.description.unwrap_or_default(),
            req.ingredients.unwrap_or_default(), req.instructions.unwrap_or_default(),
            req.prep_time.unwrap_or(0), req.cook_time.unwrap_or(0),
            req.servings.unwrap_or(1), req.calories.unwrap_or(0), tags,
            req.difficulty.unwrap_or_else(|| "easy".into()),
            req.cuisine.unwrap_or_else(|| "kz".into()),
            req.protein.unwrap_or(0), req.fat.unwrap_or(0), req.carbs.unwrap_or(0), now,
        ],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let recipe_id = conn.last_insert_rowid();

    log_activity(&conn, ctx.id, "create_recipe",
        &serde_json::json!({ "recipe_id": recipe_id, "author": author_tag }).to_string(),
        &ip, &ua);

    Ok(Json(serde_json::json!({ "status": "ok", "id": recipe_id })))
}

#[derive(Deserialize)]
struct UpdateRecipeReq {
    name: Option<String>,
    description: Option<String>,
    ingredients: Option<String>,
    instructions: Option<String>,
    prep_time: Option<i64>,
    cook_time: Option<i64>,
    servings: Option<i64>,
    calories: Option<i64>,
    author: Option<String>,
}

async fn update_recipe(
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
    let req: UpdateRecipeReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "edit")?;
    if ctx.tab != "food" || !(ctx.scope == "all" || ctx.scope == "recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    let now = chrono::Local::now().to_rfc3339();
    let mut updates: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    macro_rules! add { ($col:expr, $val:expr) => {
        if let Some(v) = $val { updates.push(format!("{}=?", $col)); params.push(Box::new(v)); }
    }; }
    add!("name", req.name.clone());
    add!("description", req.description.clone());
    add!("ingredients", req.ingredients.clone());
    add!("instructions", req.instructions.clone());
    add!("prep_time", req.prep_time);
    add!("cook_time", req.cook_time);
    add!("servings", req.servings);
    add!("calories", req.calories);
    if updates.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No fields to update".into()));
    }
    updates.push("updated_at=?".into());
    params.push(Box::new(now.clone()));
    params.push(Box::new(id));
    let sql = format!("UPDATE recipes SET {} WHERE id=?", updates.join(", "));
    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let changed = conn.execute(&sql, params_ref.as_slice())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if changed == 0 {
        return Err((StatusCode::NOT_FOUND, "Recipe not found".into()));
    }
    let author_tag = req.author.as_deref().unwrap_or("guest");
    log_activity(&conn, ctx.id, "edit_recipe",
        &serde_json::json!({ "recipe_id": id, "author": author_tag }).to_string(),
        &ip, &ua);

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_comments(
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

async fn create_comment(
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

// ── Static assets ──

async fn asset_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], include_str!("share_assets/guest.css"))
}

async fn asset_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("share_assets/guest.js"))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;").replace('\'', "&#39;")
}
