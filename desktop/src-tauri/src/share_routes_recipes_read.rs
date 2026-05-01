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
        "SELECT id, name, description, ingredients, prep_time, cook_time, servings, calories, tags,
                difficulty, cuisine, protein, fat, carbs, favorite, health_score, price_score
         FROM recipes ORDER BY updated_at DESC LIMIT 50"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "description": r.get::<_, String>(2)?,
            "ingredients": r.get::<_, String>(3)?,
            "prep_time": r.get::<_, i64>(4)?,
            "cook_time": r.get::<_, i64>(5)?,
            "servings": r.get::<_, i64>(6)?,
            "calories": r.get::<_, i64>(7)?,
            "tags": r.get::<_, String>(8)?,
            "difficulty": r.get::<_, String>(9).unwrap_or_else(|_| "easy".into()),
            "cuisine": r.get::<_, String>(10).unwrap_or_else(|_| "kz".into()),
            "protein": r.get::<_, i64>(11).unwrap_or(0),
            "fat": r.get::<_, i64>(12).unwrap_or(0),
            "carbs": r.get::<_, i64>(13).unwrap_or(0),
            "favorite": r.get::<_, i64>(14).unwrap_or(0),
            "health_score": r.get::<_, i64>(15).unwrap_or(5),
            "price_score": r.get::<_, i64>(16).unwrap_or(5),
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(stmt);

    // Ingredient catalog (id, name, category, tags) — small enough to ship inline
    let mut cat_stmt = conn.prepare("SELECT id, name, category, COALESCE(tags,'') FROM ingredient_catalog")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let catalog: Vec<serde_json::Value> = cat_stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "category": r.get::<_, String>(2)?,
            "tags": r.get::<_, String>(3)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(cat_stmt);

    log_activity(&conn, ctx.id, "view", "list_recipes", &ip, &ua);

    Ok(Json(serde_json::json!({
        "recipes": rows, "catalog": catalog,
        "label": ctx.label, "permissions": ctx.permissions,
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
    let mut recipe = conn.query_row(
        "SELECT id, name, description, ingredients, instructions, prep_time, cook_time,
                servings, calories, tags, difficulty, cuisine, protein, fat, carbs,
                favorite, health_score, price_score
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
            "favorite": r.get::<_, i64>(15).unwrap_or(0),
            "health_score": r.get::<_, i64>(16).unwrap_or(5),
            "price_score": r.get::<_, i64>(17).unwrap_or(5),
        }))
    ).map_err(|_| (StatusCode::NOT_FOUND, "Recipe not found".into()))?;

    // Structured ingredient items + catalog metadata (when linked).
    let mut ing_stmt = conn.prepare(
        "SELECT ri.name, ri.amount, ri.unit, ri.catalog_id, \
                COALESCE(c.category,''), COALESCE(c.tags,'') \
         FROM recipe_ingredients ri \
         LEFT JOIN ingredient_catalog c ON c.id = ri.catalog_id \
         WHERE ri.recipe_id=?1"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let items: Vec<serde_json::Value> = ing_stmt.query_map(rusqlite::params![id], |r| {
        Ok(serde_json::json!({
            "name": r.get::<_, String>(0)?,
            "amount": r.get::<_, f64>(1)?,
            "unit": r.get::<_, String>(2)?,
            "catalog_id": r.get::<_, Option<i64>>(3).unwrap_or(None),
            "catalog_category": r.get::<_, String>(4)?,
            "catalog_tags": r.get::<_, String>(5)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(ing_stmt);
    if let Some(obj) = recipe.as_object_mut() {
        obj.insert("ingredient_items".into(), serde_json::Value::Array(items));
    }

    log_activity(&conn, ctx.id, "view", &format!("recipe:{}", id), &ip, &ua);
    Ok(Json(recipe))
}
