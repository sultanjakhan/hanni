// share_routes_products_read.rs — GET /products (catalog from ingredient_catalog)

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

pub async fn list_products(
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
    if ctx.tab != "food" || !(ctx.scope == "all" || ctx.scope == "products") {
        return Err((StatusCode::FORBIDDEN, "Scope does not include products".into()));
    }
    let mut stmt = conn.prepare(
        "SELECT id, name, category, tags, COALESCE(subgroup,'') as subgroup
         FROM ingredient_catalog
         ORDER BY category, subgroup, name"
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "category": r.get::<_, String>(2)?,
            "tags": r.get::<_, String>(3)?,
            "subgroup": r.get::<_, String>(4)?,
        }))
    }).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .filter_map(|r| r.ok()).collect();
    drop(stmt);
    log_activity(&conn, ctx.id, "view", "list_products_catalog", &ip, &ua);

    Ok(Json(serde_json::json!({
        "products": rows, "label": ctx.label, "permissions": ctx.permissions,
    })))
}
