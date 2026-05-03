// share_routes_products_write.rs — POST /products, PATCH /products/:id, DELETE /products/:id

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

fn check_food_products_scope(ctx: &crate::share_auth::LinkCtx) -> Result<(), (StatusCode, String)> {
    if ctx.tab != "food" || !ctx.has_scope("products") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include products".into()));
    }
    Ok(())
}

#[derive(Deserialize)]
struct CreateProductReq {
    name: String,
    category: Option<String>,
    quantity: Option<f64>,
    unit: Option<String>,
    expiry_date: Option<String>,
    location: Option<String>,
    notes: Option<String>,
    author: Option<String>,
    catalog_id: Option<i64>,
}

pub async fn create_product(
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
    let req: CreateProductReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".into()));
    }
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "add")?;
    check_food_products_scope(&ctx)?;

    let now = chrono::Local::now().to_rfc3339();
    let author_tag = req.author.as_deref().unwrap_or("guest");
    let prev_notes = req.notes.clone().unwrap_or_default();
    let notes = if prev_notes.is_empty() {
        format!("[добавил: {}]", author_tag)
    } else {
        prev_notes
    };
    let trimmed_name = req.name.trim();
    // Match Hanni's add_product: auto-resolve catalog_id and inherit canonical category.
    let resolved_cat_id: Option<i64> = match req.catalog_id {
        Some(id) => Some(id),
        None => crate::db::resolve_catalog_id_by_name(&conn, trimmed_name),
    };
    let final_category: String = match resolved_cat_id {
        Some(id) => conn.query_row(
            "SELECT category FROM ingredient_catalog WHERE id=?1",
            rusqlite::params![id], |r| r.get::<_, String>(0),
        ).unwrap_or_else(|_| req.category.clone().unwrap_or_else(|| "other".into())),
        None => req.category.unwrap_or_else(|| "other".into()),
    };
    conn.execute(
        "INSERT INTO products (name, category, quantity, unit, expiry_date, location, notes, purchased_at, created_at, catalog_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8,?9)",
        rusqlite::params![
            trimmed_name,
            final_category,
            req.quantity.unwrap_or(1.0),
            req.unit.unwrap_or_else(|| "шт".into()),
            req.expiry_date,
            req.location.unwrap_or_else(|| "fridge".into()),
            notes, now,
            resolved_cat_id,
        ],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let product_id = conn.last_insert_rowid();

    log_activity(&conn, ctx.id, "create_product",
        &serde_json::json!({ "product_id": product_id, "author": author_tag }).to_string(),
        &ip, &ua);

    crate::sync_share::mark_dirty(&conn, "products");

    Ok(Json(serde_json::json!({ "status": "ok", "id": product_id })))
}

#[derive(Deserialize)]
struct UpdateProductReq {
    name: Option<String>,
    quantity: Option<f64>,
    expiry_date: Option<String>,
    location: Option<String>,
    notes: Option<String>,
    author: Option<String>,
    catalog_id: Option<i64>,
}

pub async fn update_product(
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
    let req: UpdateProductReq = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    let (ua, ip) = ua_ip(&headers, &addr);

    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let ctx = load_link(&conn, &token)?;
    require_perm(&ctx, "edit")?;
    check_food_products_scope(&ctx)?;

    let mut updates: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    macro_rules! add { ($col:expr, $val:expr) => {
        if let Some(v) = $val { updates.push(format!("{}=?", $col)); params.push(Box::new(v)); }
    }; }
    add!("name", req.name.clone());
    add!("quantity", req.quantity);
    add!("expiry_date", req.expiry_date.clone());
    add!("location", req.location.clone());
    add!("notes", req.notes.clone());
    if let Some(cid) = req.catalog_id {
        updates.push("catalog_id=?".into()); params.push(Box::new(cid));
    } else if let Some(ref nm) = req.name {
        // Auto-resolve catalog_id from new name when caller didn't pass it.
        let auto: Option<i64> = crate::db::resolve_catalog_id_by_name(&conn, nm);
        updates.push("catalog_id=?".into()); params.push(Box::new(auto));
    }
    if updates.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No fields to update".into()));
    }
    params.push(Box::new(id));
    let sql = format!("UPDATE products SET {} WHERE id=?", updates.join(", "));
    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let changed = conn.execute(&sql, params_ref.as_slice())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if changed == 0 {
        return Err((StatusCode::NOT_FOUND, "Product not found".into()));
    }
    let author_tag = req.author.as_deref().unwrap_or("guest");
    log_activity(&conn, ctx.id, "edit_product",
        &serde_json::json!({ "product_id": id, "author": author_tag }).to_string(),
        &ip, &ua);

    crate::sync_share::mark_dirty(&conn, "products");

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn delete_product(
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
    check_food_products_scope(&ctx)?;

    let changed = conn.execute("DELETE FROM products WHERE id=?1", rusqlite::params![id])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if changed == 0 {
        return Err((StatusCode::NOT_FOUND, "Product not found".into()));
    }
    log_activity(&conn, ctx.id, "delete_product",
        &serde_json::json!({ "product_id": id }).to_string(), &ip, &ua);
    crate::sync_share::mark_dirty(&conn, "products");
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
