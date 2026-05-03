// share_routes_food_meta.rs — GET /cuisines, POST /cuisines, POST /catalog

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

fn check_food_recipes_scope(ctx: &crate::share_auth::LinkCtx) -> Result<(), (StatusCode, String)> {
    if ctx.tab != "food" || !ctx.has_scope("recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    Ok(())
}

fn check_food_memory_scope(ctx: &crate::share_auth::LinkCtx) -> Result<(), (StatusCode, String)> {
    if ctx.tab != "food" || !ctx.has_scope("memory") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include memory".into()));
    }
    Ok(())
}

pub async fn list_fridge(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    if ctx.tab != "food" || !ctx.has_scope("fridge") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include fridge".into()));
    }
    let mut stmt = conn.prepare(
        "SELECT id, name, category, quantity, unit, expiry_date, location, notes
         FROM products
         ORDER BY (expiry_date IS NULL), expiry_date, name
         LIMIT 500"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "category": r.get::<_, String>(2)?,
            "quantity": r.get::<_, f64>(3)?,
            "unit": r.get::<_, String>(4)?,
            "expiry_date": r.get::<_, Option<String>>(5)?,
            "location": r.get::<_, String>(6)?,
            "notes": r.get::<_, String>(7)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    Ok(Json(serde_json::json!({ "items": rows, "label": ctx.label })))
}

pub async fn list_blacklist(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    check_food_memory_scope(&ctx)?;
    let mut stmt = conn.prepare(
        "SELECT type, value, created_at FROM food_blacklist ORDER BY type, value"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "type": r.get::<_, String>(0)?,
            "value": r.get::<_, String>(1)?,
            "created_at": r.get::<_, String>(2)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    Ok(Json(serde_json::json!({ "blacklist": rows, "label": ctx.label })))
}

pub async fn list_cuisines(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    check_food_recipes_scope(&ctx)?;
    let mut stmt = conn.prepare(
        "SELECT code, name, emoji, is_default FROM custom_cuisines ORDER BY is_default DESC, name"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            // Match Hanni JS expectation: id == code (string).
            "id": r.get::<_, String>(0)?,
            "code": r.get::<_, String>(0)?,
            "name": r.get::<_, String>(1)?,
            "emoji": r.get::<_, String>(2)?,
            "is_default": r.get::<_, i64>(3)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    Ok(Json(serde_json::json!({ "cuisines": rows })))
}

#[derive(Deserialize)]
struct AddCuisineReq { code: String, name: String, emoji: Option<String> }

pub async fn create_cuisine(
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
    let req: AddCuisineReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    if req.code.trim().is_empty() || req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "code + name required".into()));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    check_food_recipes_scope(&ctx)?;
    let em = req.emoji.unwrap_or_else(|| "🌍".into());
    conn.execute(
        "INSERT OR IGNORE INTO custom_cuisines (code, name, emoji, is_default) VALUES (?1,?2,?3,0)",
        rusqlite::params![req.code, req.name, em],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    log_activity(&conn, ctx.id, "add_cuisine",
        &serde_json::json!({ "code": req.code, "name": req.name }).to_string(), &ip, &ua);
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

#[derive(Deserialize)]
struct AddCatalogReq { name: String, category: Option<String> }

pub async fn create_catalog_item(
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
    let req: AddCatalogReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name required".into()));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    check_food_recipes_scope(&ctx)?;
    let cat = req.category.unwrap_or_else(|| "other".into());
    let trimmed = req.name.trim();
    conn.execute(
        "INSERT OR IGNORE INTO ingredient_catalog (name, category, tags) VALUES (?1,?2,'')",
        rusqlite::params![trimmed, cat],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let id: i64 = conn.query_row(
        "SELECT id FROM ingredient_catalog WHERE name=?1 COLLATE NOCASE",
        rusqlite::params![trimmed], |r| r.get(0),
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    log_activity(&conn, ctx.id, "add_catalog_item",
        &serde_json::json!({ "name": req.name, "category": cat }).to_string(), &ip, &ua);
    crate::sync_share::mark_dirty(&conn, "ingredient_catalog");
    Ok(Json(serde_json::json!({ "id": id, "status": "ok" })))
}
