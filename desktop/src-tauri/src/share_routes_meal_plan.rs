// share_routes_meal_plan.rs — GET /meal-plan, POST /meal-plan, DELETE /meal-plan/:id

use axum::{
    extract::{Path, Query, State as AxumState, ConnectInfo},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use tauri::Manager;

use crate::share_auth::{
    load_link, require_perm, rate_limit_check, log_activity, ua_ip, BODY_LIMIT_BYTES,
};
use crate::share_server::ShareServerState;
use crate::types::HanniDb;

const MEAL_TYPES: &[&str] = &["breakfast", "lunch", "dinner", "snack"];

fn check_food_meal_scope(ctx: &crate::share_auth::LinkCtx) -> Result<(), (StatusCode, String)> {
    if ctx.tab != "food" || !ctx.has_scope("meal_plan") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include meal plan".into()));
    }
    Ok(())
}

pub async fn list_meal_plan(
    Path(token): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    AxumState(state): AxumState<ShareServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let date = params.get("date").cloned()
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "view")?;
    check_food_meal_scope(&ctx)?;

    let mut stmt = conn.prepare(
        "SELECT mp.id, mp.date, mp.meal_type, mp.recipe_id, mp.notes,
                r.name, r.calories
         FROM meal_plan mp
         LEFT JOIN recipes r ON r.id = mp.recipe_id
         WHERE mp.date = ?1
         ORDER BY CASE mp.meal_type
            WHEN 'breakfast' THEN 1 WHEN 'lunch' THEN 2
            WHEN 'dinner' THEN 3 WHEN 'snack' THEN 4 ELSE 5 END"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let meals: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![date], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "date": r.get::<_, String>(1)?,
            "meal_type": r.get::<_, String>(2)?,
            "recipe_id": r.get::<_, i64>(3)?,
            "notes": r.get::<_, String>(4)?,
            "recipe_name": r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "(удалён)".into()),
            "calories": r.get::<_, Option<i64>>(6)?.unwrap_or(0),
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(stmt);

    let mut idx_stmt = conn.prepare(
        "SELECT id, name FROM recipes ORDER BY name LIMIT 200"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let recipes_index: Vec<serde_json::Value> = idx_stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(idx_stmt);

    log_activity(&conn, ctx.id, "view", &format!("meal_plan:{}", date), &ip, &ua);

    Ok(Json(serde_json::json!({
        "date": date,
        "meals": meals,
        "recipes_index": recipes_index,
        "label": ctx.label,
        "permissions": ctx.permissions,
    })))
}

#[derive(Deserialize)]
struct CreateMealReq {
    date: String,
    meal_type: String,
    recipe_id: i64,
    notes: Option<String>,
    author: Option<String>,
}

pub async fn create_meal_plan(
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
    let req: CreateMealReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    if req.date.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "date is required".into()));
    }
    if !MEAL_TYPES.contains(&req.meal_type.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("Unknown meal_type: {}", req.meal_type)));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    check_food_meal_scope(&ctx)?;

    let recipe_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM recipes WHERE id=?1",
        rusqlite::params![req.recipe_id],
        |r| r.get(0),
    ).unwrap_or(0);
    if recipe_exists == 0 {
        return Err((StatusCode::BAD_REQUEST, "recipe_id does not exist".into()));
    }

    let now = chrono::Local::now().to_rfc3339();
    let author_tag = req.author.as_deref().unwrap_or("guest");
    let prev_notes = req.notes.clone().unwrap_or_default();
    let notes = if prev_notes.is_empty() {
        format!("[добавил: {}]", author_tag)
    } else {
        prev_notes
    };
    conn.execute(
        "INSERT INTO meal_plan (date, meal_type, recipe_id, notes, created_at)
         VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![req.date, req.meal_type, req.recipe_id, notes, now],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let meal_id = conn.last_insert_rowid();

    log_activity(&conn, ctx.id, "create_meal",
        &serde_json::json!({
            "meal_id": meal_id, "date": req.date,
            "meal_type": req.meal_type, "author": author_tag,
        }).to_string(), &ip, &ua);

    crate::sync_share::mark_dirty(&conn, "meal_plan");

    Ok(Json(serde_json::json!({ "status": "ok", "id": meal_id })))
}

pub async fn delete_meal_plan(
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
    require_perm(&ctx, "delete")?;
    check_food_meal_scope(&ctx)?;

    let changed = conn.execute("DELETE FROM meal_plan WHERE id=?1", rusqlite::params![id])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if changed == 0 {
        return Err((StatusCode::NOT_FOUND, "Meal not found".into()));
    }
    log_activity(&conn, ctx.id, "delete_meal",
        &serde_json::json!({ "meal_id": id }).to_string(), &ip, &ua);
    crate::sync_share::mark_dirty(&conn, "meal_plan");
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
