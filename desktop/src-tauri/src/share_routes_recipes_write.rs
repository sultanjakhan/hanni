// share_routes_recipes_write.rs — POST /recipes, PATCH /recipes/:id

use axum::{
    extract::{Path, State as AxumState, ConnectInfo},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tauri::Manager;

use crate::share_auth::{
    load_link, require_perm, rate_limit_check, log_activity, sanitize_author, ua_ip,
    BODY_LIMIT_BYTES,
};
use crate::share_server::ShareServerState;
use crate::types::HanniDb;

const MEAL_TAGS: [&str; 4] = ["breakfast", "lunch", "dinner", "universal"];

fn validated_tags(input: Option<&str>, author: &str) -> Result<String, (StatusCode, String)> {
    let mut out: Vec<&str> = Vec::new();
    for tag in input
        .unwrap_or("universal")
        .split(|c: char| c == ',' || c.is_whitespace())
    {
        let tag = tag.trim();
        if tag.is_empty() || tag.starts_with("shared-by:") {
            continue;
        }
        if !MEAL_TAGS.contains(&tag) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("invalid recipe tag '{tag}'"),
            ));
        }
        if !out.contains(&tag) {
            out.push(tag);
        }
    }
    if out.is_empty() {
        out.push("universal");
    }
    Ok(format!("{},shared-by:{}", out.join(","), author))
}

fn validated_difficulty(value: &str) -> Result<String, (StatusCode, String)> {
    if ["easy", "medium", "hard"].contains(&value) {
        Ok(value.to_string())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "difficulty must be easy|medium|hard".into(),
        ))
    }
}

fn valid_cuisine_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validated_cuisine(
    conn: &rusqlite::Connection,
    value: &str,
) -> Result<String, (StatusCode, String)> {
    if !valid_cuisine_code(value) {
        return Err((StatusCode::BAD_REQUEST, "invalid cuisine code".into()));
    }
    let exists = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM custom_cuisines WHERE code=?1)",
            rusqlite::params![value],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        != 0;
    if !exists {
        return Err((StatusCode::BAD_REQUEST, "unknown cuisine code".into()));
    }
    Ok(value.to_string())
}

fn validate_text(
    value: &str,
    max_chars: usize,
    multiline: bool,
    field: &str,
) -> Result<(), (StatusCode, String)> {
    let bad_control = value
        .chars()
        .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')));
    if value.chars().count() > max_chars || bad_control {
        return Err((StatusCode::BAD_REQUEST, format!("invalid {field}")));
    }
    Ok(())
}

fn validate_i64(
    value: Option<i64>,
    min: i64,
    max: i64,
    field: &str,
) -> Result<(), (StatusCode, String)> {
    if value.is_some_and(|value| !(min..=max).contains(&value)) {
        return Err((StatusCode::BAD_REQUEST, format!("invalid {field}")));
    }
    Ok(())
}

fn validated_recipe_image(value: &str) -> Result<String, (StatusCode, String)> {
    use base64::Engine;
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let encoded = value
        .strip_prefix("data:image/jpeg;base64,")
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "image must be a JPEG data URL".into(),
            )
        })?;
    if encoded.len() > BODY_LIMIT_BYTES {
        return Err((StatusCode::BAD_REQUEST, "recipe image is too large".into()));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "invalid recipe image base64".into(),
            )
        })?;
    if bytes.len() > 192 * 1024
        || !bytes.starts_with(&[0xff, 0xd8, 0xff])
        || !bytes.ends_with(&[0xff, 0xd9])
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid or oversized JPEG image".into(),
        ));
    }
    Ok(value.to_string())
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecipeStepInput {
    text: String,
    #[serde(default)]
    min: u32,
    #[serde(default)]
    ingredients: Vec<String>,
}

fn validated_instructions(value: &str) -> Result<String, (StatusCode, String)> {
    let value = value.trim();
    validate_text(value, 50_000, true, "recipe instructions")?;
    if !value.starts_with('[') {
        return Ok(value.to_string());
    }
    let steps: Vec<RecipeStepInput> = serde_json::from_str(value).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid structured recipe instructions".into(),
        )
    })?;
    if steps.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "too many recipe steps".into()));
    }
    for step in &steps {
        if step.text.trim().is_empty()
            || step.min > 24 * 60
            || validate_text(&step.text, 2_000, true, "recipe step").is_err()
        {
            return Err((StatusCode::BAD_REQUEST, "invalid recipe step".into()));
        }
        if step.ingredients.len() > 100
            || step.ingredients.iter().any(|item| {
                item.trim().is_empty()
                    || item.chars().count() > 200
                    || item.chars().any(|c| c.is_control())
            })
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid recipe step ingredients".into(),
            ));
        }
    }
    serde_json::to_string(&steps).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

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
    image: Option<String>,
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
    validate_text(req.name.trim(), 200, false, "recipe name")?;
    if let Some(value) = &req.description {
        validate_text(value, 10_000, true, "recipe description")?;
    }
    if let Some(value) = &req.ingredients {
        validate_text(value, 50_000, true, "recipe ingredients")?;
    }
    validate_ingredient_items(req.ingredient_items.as_deref().unwrap_or(&[]))?;
    validate_i64(req.prep_time, 0, 10_080, "prep_time")?;
    validate_i64(req.cook_time, 0, 10_080, "cook_time")?;
    validate_i64(req.servings, 1, 1_000, "servings")?;
    for (value, field) in [
        (req.calories, "calories"),
        (req.protein, "protein"),
        (req.fat, "fat"),
        (req.carbs, "carbs"),
    ] {
        validate_i64(value, 0, 1_000_000, field)?;
    }
    validate_i64(req.health_score, 1, 10, "health_score")?;
    validate_i64(req.price_score, 1, 10, "price_score")?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let mut conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    if ctx.tab != "food" || !ctx.has_scope("recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    let now = chrono::Local::now().to_rfc3339();
    let author_tag = sanitize_author(req.author.as_deref(), "guest");
    let tags = validated_tags(req.tags.as_deref(), &format!("link-{}", ctx.id))?;
    let difficulty = validated_difficulty(req.difficulty.as_deref() .unwrap_or("easy"))?;
    let cuisine = validated_cuisine(&conn, req.cuisine.as_deref()
    .unwrap_or("kz")
    )?;
    let image = validated_recipe_image(req.image.as_deref().unwrap_or(""))?;
    let instructions = validated_instructions(req.instructions.as_deref().unwrap_or(""))?;
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
    let tx = conn.transaction()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tx.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time,
            servings, calories, tags, difficulty, cuisine, health_score, price_score,
            protein, fat, carbs, image, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?18)",
        rusqlite::params![
            req.name.trim(), req.description.unwrap_or_default(),
            flat_ingredients,
            instructions,
            req.prep_time.unwrap_or(0), req.cook_time.unwrap_or(0),
            req.servings.unwrap_or(1), req.calories.unwrap_or(0), tags,
            difficulty,
            cuisine,
            req.health_score.unwrap_or(5), req.price_score.unwrap_or(5),
            req.protein.unwrap_or(0), req.fat.unwrap_or(0), req.carbs.unwrap_or(0), image,
            now,
        ],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let recipe_id = tx.last_insert_rowid();

    if let Some(items) = &req.ingredient_items {
        for it in items.iter().filter(|i| !i.name.trim().is_empty()) {
            tx.execute(
                "INSERT INTO recipe_ingredients (recipe_id, name, amount, unit) VALUES (?1,?2,?3,?4)",
                rusqlite::params![recipe_id, it.name.trim(),
                    it.amount.unwrap_or(0.0), it.unit.clone().unwrap_or_else(|| "г".into())],
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        }
    tx.commit()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if req.ingredient_items.is_some() {
        crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    }

    log_activity(&conn, ctx.id, "create_recipe",
        &serde_json::json!({ "recipe_id": recipe_id, "author": author_tag }).to_string(),
        &ip, &ua);

    crate::sync_share::mark_dirty(&conn, "recipes");

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
    tags: Option<String>,
    difficulty: Option<String>,
    cuisine: Option<String>,
    protein: Option<i64>,
    fat: Option<i64>,
    carbs: Option<i64>,
    health_score: Option<i64>,
    price_score: Option<i64>,
    image: Option<String>,
    ingredient_items: Option<Vec<IngredientItem>>,
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
    if let Some(value) = &req.name {
        if value.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "name is required".into()));
        }
        validate_text(value.trim(), 200, false, "recipe name")?;
    }
    if let Some(value) = &req.description {
        validate_text(value, 10_000, true, "recipe description")?;
    }
    if let Some(value) = &req.ingredients {
        validate_text(value, 50_000, true, "recipe ingredients")?;
    }
    validate_ingredient_items(req.ingredient_items.as_deref().unwrap_or(&[]))?;
    validate_i64(req.prep_time, 0, 10_080, "prep_time")?;
    validate_i64(req.cook_time, 0, 10_080, "cook_time")?;
    validate_i64(req.servings, 1, 1_000, "servings")?;
    for (value, field) in [
        (req.calories, "calories"),
        (req.protein, "protein"),
        (req.fat, "fat"),
        (req.carbs, "carbs"),
    ] {
        validate_i64(value, 0, 1_000_000, field)?;
    }
    validate_i64(req.health_score, 1, 10, "health_score")?;
    validate_i64(req.price_score, 1, 10, "price_score")?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let mut conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "edit")?;
    if ctx.tab != "food" || !ctx.has_scope("recipes") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes".into()));
    }
    let now = chrono::Local::now().to_rfc3339();
    let author_tag = sanitize_author(req.author.as_deref(), "guest");
    let attribution = format!("link-{}", ctx.id);
    let tags = req
        .tags
        .as_deref()
        .map(|v| validated_tags(Some(v), &attribution))
        .transpose()?;
    let difficulty = req
        .difficulty
        .as_deref()
        .map(validated_difficulty)
        .transpose()?;
    let cuisine = req
        .cuisine
        .as_deref()
        .map(|v| validated_cuisine(&conn, v))
        .transpose()?;
    let image = req
        .image
        .as_deref()
        .map(validated_recipe_image)
        .transpose()?;
    let instructions = req
        .instructions
        .as_deref()
        .map(validated_instructions)
        .transpose()?;
    let mut updates: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    macro_rules! add { ($col:expr, $val:expr) => {
        if let Some(v) = $val { updates.push(format!("{}=?", $col)); params.push(Box::new(v)); }
    }; }
    // If ingredient_items provided, derive the flat `ingredients` column from
    // them — matches CreateRecipeReq behavior so the legacy text field stays
    // in sync with the structured rows.
    let derived_ingredients: Option<String> = req.ingredient_items.as_ref().map(|items|
        items.iter()
            .filter(|i| !i.name.trim().is_empty())
            .map(|i| format!("{}: {}{}",
                i.name, i.amount.unwrap_or(0.0), i.unit.clone().unwrap_or_else(|| "г".into())))
            .collect::<Vec<_>>().join(", ")
    );
    add!("name", req.name.clone());
    add!("description", req.description.clone());
    add!("ingredients", derived_ingredients.or(req.ingredients.clone()));
    add!("instructions", instructions);
    add!("prep_time", req.prep_time);
    add!("cook_time", req.cook_time);
    add!("servings", req.servings);
    add!("calories", req.calories);
    add!("tags", tags);
    add!("difficulty", difficulty);
    add!("cuisine", cuisine);
    add!("protein", req.protein);
    add!("fat", req.fat);
    add!("carbs", req.carbs);
    add!("health_score", req.health_score);
    add!("price_score", req.price_score);
    add!("image", image);
    let has_field_updates = !updates.is_empty();
    let has_items = req.ingredient_items.is_some();
    if !has_field_updates && !has_items {
        return Err((StatusCode::BAD_REQUEST, "No fields to update".into()));
    }
    let tx = conn
        .transaction()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if has_field_updates {
        updates.push("updated_at=?".into());
        params.push(Box::new(now.clone()));
        params.push(Box::new(id));
        let sql = format!("UPDATE recipes SET {} WHERE id=?", updates.join(", "));
        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let changed = tx
            .execute(&sql, params_ref.as_slice())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if changed == 0 {
            return Err((StatusCode::NOT_FOUND, "Recipe not found".into()));
        }
    }
    // Replace structured ingredients when provided. Matches Hanni-side
    // update_recipe semantics (DELETE + INSERT) so guests can fix ingredient
    // amounts/units without losing the structured rows.
    if let Some(items) = &req.ingredient_items {
        let exists = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM recipes WHERE id=?1)",
                rusqlite::params![id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            != 0;
        if !exists {
            return Err((StatusCode::NOT_FOUND, "Recipe not found".into()));
        }
        tx.execute("DELETE FROM recipe_ingredients WHERE recipe_id=?1",
            rusqlite::params![id]).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for it in items.iter().filter(|i| !i.name.trim().is_empty()) {
            tx.execute(
                "INSERT INTO recipe_ingredients (recipe_id, name, amount, unit) VALUES (?1,?2,?3,?4)",
                rusqlite::params![id, it.name.trim(),
                    it.amount.unwrap_or(0.0), it.unit.clone().unwrap_or_else(|| "г".into())],
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }
        tx.commit()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if has_items {
        crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    }
    log_activity(&conn, ctx.id, "edit_recipe",
        &serde_json::json!({ "recipe_id": id, "author": author_tag }).to_string(),
        &ip, &ua);

    crate::sync_share::mark_dirty(&conn, "recipes");

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
    if ctx.tab != "food" || !ctx.has_scope("recipes") {
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
    crate::sync_share::mark_dirty(&conn, "recipes");
    crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
fn validate_ingredient_items(items: &[IngredientItem]) -> Result<(), (StatusCode, String)> {
    if items.len() > 200 {
        return Err((
            StatusCode::BAD_REQUEST,
            "too many recipe ingredients".into(),
        ));
    }
    for item in items {
        validate_text(item.name.trim(), 200, false, "ingredient name")?;
        if item.name.trim().is_empty()
            || item.amount.is_some_and(|amount| {
                !amount.is_finite() || !(0.0..=1_000_000_000.0).contains(&amount)
            })
        {
            return Err((StatusCode::BAD_REQUEST, "invalid recipe ingredient".into()));
        }
        if let Some(unit) = &item.unit {
            validate_text(unit, 32, false, "ingredient unit")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod share_security_tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn recipe_tags_are_allowlisted_and_attribution_is_server_owned() {
        assert_eq!(
            validated_tags(Some("breakfast,shared-by:forged,lunch"), "link-42").unwrap(),
            "breakfast,lunch,shared-by:link-42"
        );
        assert!(validated_tags(Some("<img onerror=x>"), "guest").is_err());
    }

    #[test]
    fn difficulty_and_cuisine_codes_reject_markup() {
        assert!(validated_difficulty("easy").is_ok());
        assert!(validated_difficulty("easy\" onfocus=\"x").is_err());
        assert!(valid_cuisine_code("central_asia-1"));
        assert!(!valid_cuisine_code("x\" onfocus=\"x"));
    }

    #[test]
    fn cuisine_must_exist() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE custom_cuisines (code TEXT PRIMARY KEY)", [])
            .unwrap();
        conn.execute("INSERT INTO custom_cuisines(code) VALUES ('kz')", [])
            .unwrap();
        assert_eq!(validated_cuisine(&conn, "kz").unwrap(), "kz");
        assert!(validated_cuisine(&conn, "missing").is_err());
    }

    #[test]
    fn recipe_image_accepts_only_small_jpeg_data_urls() {
        let jpeg = [0xff, 0xd8, 0xff, 0xe0, 0xff, 0xd9];
        let value = format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(jpeg)
        );
        assert_eq!(validated_recipe_image(&value).unwrap(), value);
        assert!(validated_recipe_image("x\" onerror=\"alert(1)").is_err());
        assert!(validated_recipe_image("data:image/svg+xml,<svg onload=x>").is_err());
    }

    #[test]
    fn structured_instructions_require_bounded_numeric_minutes() {
        let safe = r#"[{"text":"Boil","min":5,"ingredients":["water"]}]"#;
        assert_eq!(validated_instructions(safe).unwrap(), safe);
        assert!(validated_instructions(
            r#"[{"text":"Boil","min":"</span><img src=x onerror=x>","ingredients":[]}]"#
        )
        .is_err());
        assert!(
            validated_instructions(r#"[{"text":"Boil","min":1441,"ingredients":[]}]"#).is_err()
        );
    }

    #[test]
    fn recipe_fields_and_ingredients_are_bounded() {
        assert!(validate_i64(Some(-1), 0, 10, "score").is_err());
        assert!(validate_text("ok\0bad", 20, false, "name").is_err());
        let invalid = IngredientItem {
            name: "salt".into(),
            amount: Some(-1.0),
            unit: Some("g".into()),
        };
        assert!(validate_ingredient_items(&[invalid]).is_err());
    }
}
