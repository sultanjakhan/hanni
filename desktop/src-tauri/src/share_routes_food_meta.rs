// share_routes_food_meta.rs — GET /cuisines, POST /cuisines, POST /catalog

use axum::{
    extract::{Path, State as AxumState, ConnectInfo},
    http::{HeaderMap, StatusCode},
    Json,
};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use std::net::SocketAddr;
use tauri::Manager;

use crate::share_auth::{
    load_link, require_perm, rate_limit_check, log_activity, ua_ip, BODY_LIMIT_BYTES,
};
use crate::share_server::ShareServerState;
use crate::types::HanniDb;

pub(crate) const CATALOG_CATEGORIES: [&str; 13] = [
    "meat", "fish", "veg", "fruit", "grain", "dairy", "legumes", "nuts", "spice", "oil", "bakery",
    "drinks", "other",
];

fn contains_unsafe_text(value: &str) -> bool {
    value
        .chars()
        .any(|c| c.is_control() || "<>&\"'".contains(c))
}

fn validate_cuisine_fields(
    code: &str,
    name: &str,
    emoji: &str,
) -> Result<(String, String, String), (StatusCode, String)> {
    let (code, name, emoji) = (code.trim(), name.trim(), emoji.trim());
    if code.is_empty()
        || code.len() > 32
        || !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err((StatusCode::BAD_REQUEST, "invalid cuisine code".into()));
    }
    if name.is_empty() || name.chars().count() > 80 || contains_unsafe_text(name) {
        return Err((StatusCode::BAD_REQUEST, "invalid cuisine name".into()));
    }
    if emoji.is_empty() || emoji.chars().count() > 16 || contains_unsafe_text(emoji) {
        return Err((StatusCode::BAD_REQUEST, "invalid cuisine emoji".into()));
    }
    Ok((code.to_string(), name.to_string(), emoji.to_string()))
}

fn validated_catalog_category(value: Option<&str>) -> Result<String, (StatusCode, String)> {
    let category = value.unwrap_or("other").trim();
    if CATALOG_CATEGORIES.contains(&category) {
        Ok(category.to_string())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "invalid ingredient category".into(),
        ))
    }
}

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
    // Include id so a guest with delete-perm can reference rows.
    // Include `level` so the guest can split Не ем (hard) / Не люблю (soft) / Люблю (love).
    let mut stmt = conn.prepare(
        "SELECT id, type, value, level, catalog_id, created_at FROM food_blacklist ORDER BY level, type, value"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "type": r.get::<_, String>(1)?,
            "value": r.get::<_, String>(2)?,
            "level": r.get::<_, String>(3).unwrap_or_else(|_| "hard".into()),
            "catalog_id": r.get::<_, Option<i64>>(4).unwrap_or(None),
            "created_at": r.get::<_, String>(5)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    Ok(Json(serde_json::json!({ "blacklist": rows, "label": ctx.label })))
}

#[derive(Deserialize)]
struct AddBlacklistReq {
    #[serde(rename = "type")]
    entry_type: String,
    value: String,
    catalog_id: Option<i64>,
}

pub async fn create_blacklist_item(
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
    let req: AddBlacklistReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    if !["tag", "product", "category"].contains(&req.entry_type.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "type must be tag|product|category".into()));
    }
    let value = req.value.trim().to_lowercase();
    if value.is_empty() || value.len() > 200 || value.chars().any(char::is_control) {
        return Err((StatusCode::BAD_REQUEST, "value is invalid or too long".into(),
        ));
    }
    if req.entry_type == "category" && !CATALOG_CATEGORIES.contains(&value.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "unknown catalog category".into()));
    }
    if req.entry_type != "product" && req.catalog_id.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            "catalog_id is valid only for product entries".into(),
        ));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    check_food_memory_scope(&ctx)?;

    // Mirror Hanni's add_food_blacklist: auto-resolve catalog_id for type=product.
    let cat_id: Option<i64> = match req.catalog_id {
        Some(id) if id > 0 => {
            let catalog_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM ingredient_catalog WHERE id=?1",
                    rusqlite::params![id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if catalog_name
                .as_deref()
                .map(crate::db::normalize_name)
                .as_deref()
                != Some(value.as_str())
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "catalog_id does not match product value".into(),
                ));
            }
            Some(id)}
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
        "catalog_id must be positive".into(),
            ))
        }
        None if req.entry_type == "product" => crate::db::resolve_catalog_id_by_name(&conn, &value),
        None => None,
    };
    conn.execute(
        "INSERT OR IGNORE INTO food_blacklist (type, value, catalog_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![req.entry_type, value, cat_id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let id: i64 = conn.query_row(
        "SELECT id FROM food_blacklist WHERE type=?1 AND value=?2",
        rusqlite::params![req.entry_type, value], |r| r.get(0),
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    log_activity(&conn, ctx.id, "add_blacklist",
        &serde_json::json!({ "id": id, "type": req.entry_type, "value": value }).to_string(),
        &ip, &ua);
    crate::sync_share::mark_dirty(&conn, "food_blacklist");
    Ok(Json(serde_json::json!({ "id": id, "status": "ok" })))
}

pub async fn delete_blacklist_item(
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
    check_food_memory_scope(&ctx)?;
    let affected = conn.execute(
        "DELETE FROM food_blacklist WHERE id=?1",
        rusqlite::params![id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "Blacklist entry not found".into()));
    }
    log_activity(&conn, ctx.id, "delete_blacklist",
        &serde_json::json!({ "id": id }).to_string(), &ip, &ua);
    crate::sync_share::mark_dirty(&conn, "food_blacklist");
    Ok(Json(serde_json::json!({ "status": "ok" })))
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
    let (code, name, em)=
        validate_cuisine_fields(&req.code, &req.name, req.emoji.as_deref() .unwrap_or("🌍"))?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    check_food_recipes_scope(&ctx)?;
    conn.execute(
        "INSERT OR IGNORE INTO custom_cuisines (code, name, emoji, is_default) VALUES (?1,?2,?3,0)",
        rusqlite::params![&code, &name, &em],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    log_activity(&conn, ctx.id, "add_cuisine",
        &serde_json::json!({ "code": code, "name": name }).to_string(), &ip, &ua);
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
    // Either scope qualifies: recipes (auto-add ingredient when authoring a
    // recipe) or products (UI label is "Продукты (каталог)" — extending the
    // catalog is a natural fit for this scope).
    if ctx.tab != "food" || !(ctx.has_scope("recipes") || ctx.has_scope("products")) {
        return Err((StatusCode::FORBIDDEN, "Scope does not include recipes or products".into()));
    }
    let cat = validated_catalog_category(req.category.as_deref())?;
    let trimmed = req.name.trim();
    if trimmed.chars().count() > 120 || trimmed.chars().any(|c| c.is_control()) {
        return Err((StatusCode::BAD_REQUEST, "invalid ingredient name".into()));
    }
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
#[cfg(test)]
mod food_meta_security_tests {
    use super::*;

    #[test]
    fn cuisine_fields_reject_markup_and_control_characters() {
        assert!(validate_cuisine_fields("kz", "Казахская", "🇰🇿").is_ok());
        assert!(validate_cuisine_fields("x", "X", "<img onerror=x>").is_err());
        assert!(validate_cuisine_fields("x\" y", "X", "🌍").is_err());
        assert!(validate_cuisine_fields("x", "bad\nname", "🌍").is_err());
    }

    #[test]
    fn catalog_category_is_an_enum() {
        assert_eq!(validated_catalog_category(Some("veg")).unwrap(), "veg");
        assert_eq!(validated_catalog_category(None).unwrap(), "other");
        assert!(validated_catalog_category(Some("x\" onfocus=\"alert(1)")).is_err());
    }
}
