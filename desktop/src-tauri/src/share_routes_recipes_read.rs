// share_routes_recipes_read.rs — GET /recipes, GET /recipes/:id

use axum::{
    extract::{Path, State as AxumState, ConnectInfo},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::net::SocketAddr;
use tauri::Manager;

use crate::share_auth::{load_link, require_perm, rate_limit_check, log_activity, ua_ip};
use crate::share_server::ShareServerState;
use crate::types::HanniDb;

pub async fn list_recipes(
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

pub async fn get_recipe(
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
