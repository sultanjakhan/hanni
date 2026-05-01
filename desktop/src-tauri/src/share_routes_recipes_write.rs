// share_routes_recipes_write.rs — POST /recipes, PATCH /recipes/:id

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

#[derive(Deserialize)]
struct IngredientItem {
    name: String,
    amount: Option<f64>,
    unit: Option<String>,
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
    health_score: Option<i64>,
    price_score: Option<i64>,
    ingredient_items: Option<Vec<IngredientItem>>,
    author: Option<String>,
}

pub async fn create_recipe(
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
    // Build flat ingredients string from items if provided (for legacy `ingredients` column).
    let flat_ingredients = if let Some(items) = &req.ingredient_items {
        items.iter()
            .filter(|i| !i.name.trim().is_empty())
            .map(|i| format!("{}: {}{}",
                i.name, i.amount.unwrap_or(0.0), i.unit.clone().unwrap_or_else(|| "г".into())))
            .collect::<Vec<_>>().join(", ")
    } else {
        req.ingredients.clone().unwrap_or_default()
    };
    conn.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time,
            servings, calories, tags, difficulty, cuisine, health_score, price_score,
            protein, fat, carbs, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)",
        rusqlite::params![
            req.name.trim(), req.description.unwrap_or_default(),
            flat_ingredients, req.instructions.unwrap_or_default(),
            req.prep_time.unwrap_or(0), req.cook_time.unwrap_or(0),
            req.servings.unwrap_or(1), req.calories.unwrap_or(0), tags,
            req.difficulty.unwrap_or_else(|| "easy".into()),
            req.cuisine.unwrap_or_else(|| "kz".into()),
            req.health_score.unwrap_or(5), req.price_score.unwrap_or(5),
            req.protein.unwrap_or(0), req.fat.unwrap_or(0), req.carbs.unwrap_or(0), now,
        ],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let recipe_id = conn.last_insert_rowid();

    if let Some(items) = req.ingredient_items {
        for it in items.iter().filter(|i| !i.name.trim().is_empty()) {
            let _ = conn.execute(
                "INSERT INTO recipe_ingredients (recipe_id, name, amount, unit) VALUES (?1,?2,?3,?4)",
                rusqlite::params![recipe_id, it.name.trim(),
                    it.amount.unwrap_or(0.0), it.unit.clone().unwrap_or_else(|| "г".into())],
            );
        }
    }

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

pub async fn update_recipe(
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

pub async fn delete_recipe(
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
    if ctx.tab != "food" || !(ctx.scope == "all" || ctx.scope == "recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    // recipe_ingredients and meal_plan rows are cleared by ON DELETE CASCADE.
    let changed = conn.execute("DELETE FROM recipes WHERE id=?1", rusqlite::params![id])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if changed == 0 {
        return Err((StatusCode::NOT_FOUND, "Recipe not found".into()));
    }
    log_activity(&conn, ctx.id, "delete_recipe",
        &serde_json::json!({ "recipe_id": id }).to_string(), &ip, &ua);
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
