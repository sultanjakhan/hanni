// share_server.rs — Public HTTP server for share-links (guest-facing).
// Runs on 127.0.0.1:8239 (prod) / 8240 (dev). Cloudflare Tunnel exposes it to the internet.

use axum::{
    extract::{Path, State as AxumState},
    http::{HeaderValue, Method, StatusCode, header},
    response::Html,
    routing::{delete, get, patch},
    Json, Router,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::share_auth::{html_escape, load_link, rate_limit_check};
use crate::share_routes_comments::{create_comment, list_comments};
use crate::share_routes_food_meta::{create_catalog_item, create_cuisine, list_blacklist, list_cuisines, list_fridge};
use crate::share_routes_meal_plan::{create_meal_plan, delete_meal_plan, list_meal_plan};
use crate::share_routes_products_read::list_products;
use crate::share_routes_products_write::{create_product, delete_product, update_product};
use crate::share_routes_recipes_read::{get_recipe, list_recipes};
use crate::share_routes_recipes_write::{create_recipe, update_recipe, delete_recipe};
use crate::share_static::{
    asset_css, asset_js, asset_js_firestore, asset_js_fridge, asset_js_fridge_shared,
    asset_js_meal_plan, asset_js_memory, asset_js_products, asset_js_recipe_add,
    asset_js_recipe_shared, asset_js_recipe_shared_ingredients, asset_js_recipes,
};
use crate::types::HanniDb;

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
        .route("/s/{token}/recipes/{id}", get(get_recipe).patch(update_recipe).delete(delete_recipe))
        .route("/s/{token}/recipes/{id}/comments", get(list_comments).post(create_comment))
        .route("/s/{token}/products", get(list_products).post(create_product))
        .route("/s/{token}/products/{id}", patch(update_product).delete(delete_product))
        .route("/s/{token}/meal-plan", get(list_meal_plan).post(create_meal_plan))
        .route("/s/{token}/meal-plan/{id}", delete(delete_meal_plan))
        .route("/s/{token}/cuisines", get(list_cuisines).post(create_cuisine))
        .route("/s/{token}/catalog", axum::routing::post(create_catalog_item))
        .route("/s/{token}/blacklist", get(list_blacklist))
        .route("/s/{token}/fridge", get(list_fridge))
        .route("/s/{token}/assets/guest.css", get(asset_css))
        .route("/s/{token}/assets/guest.js", get(asset_js))
        .route("/s/{token}/assets/guest_firestore.js", get(asset_js_firestore))
        .route("/s/{token}/assets/guest_recipes.js", get(asset_js_recipes))
        .route("/s/{token}/assets/recipe-shared.js", get(asset_js_recipe_shared))
        .route("/s/{token}/assets/recipe-shared-ingredients.js", get(asset_js_recipe_shared_ingredients))
        .route("/s/{token}/assets/guest_recipe_add.js", get(asset_js_recipe_add))
        .route("/s/{token}/assets/guest_products.js", get(asset_js_products))
        .route("/s/{token}/assets/guest_meal_plan.js", get(asset_js_meal_plan))
        .route("/s/{token}/assets/guest_memory.js", get(asset_js_memory))
        .route("/s/{token}/assets/fridge-shared.js", get(asset_js_fridge_shared))
        .route("/s/{token}/assets/guest_fridge.js", get(asset_js_fridge))
        .with_state(state)
        // Allow Firebase-Hosted guest UI to POST/PATCH/DELETE writes here.
        // Static origin is .web.app / .firebaseapp.com — both belong to the
        // same Firebase project. Localhost stays open for dev tooling.
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    origin.to_str().map(|s|
                        s.ends_with(".web.app")
                        || s.ends_with(".firebaseapp.com")
                        || s.starts_with("http://127.0.0.1")
                        || s.starts_with("http://localhost")
                    ).unwrap_or(false)
                }))
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE])
        );

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

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "share" }))
}

async fn landing(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Html<String>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let (ctx, firestore_json) = {
        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let ctx = load_link(&conn, &token)?;
        // Firestore config travels to the guest so the JS client can read
        // share_links/{token}/* from the cloud directly. Stripped of the
        // service-account JSON (which never leaves Hanni). When cloud-share
        // isn't configured we send `null` and the guest falls back to axum.
        let fs_json = crate::sync_share::load_config(&conn)
            .map(|c| serde_json::json!({
                "project_id": c.project_id,
                "api_key":    c.api_key,
            }).to_string())
            .unwrap_or_else(|| "null".into());
        (ctx, fs_json)
    };
    let html = include_str!("share_assets/guest.html")
        .replace("{{LABEL}}", &html_escape(&ctx.label))
        .replace("{{TAB}}", &html_escape(&ctx.tab))
        .replace("{{SCOPE}}", &html_escape(&ctx.scope))
        .replace("{{TOKEN}}", &html_escape(&token))
        .replace("{{PERMS}}", &serde_json::to_string(&ctx.permissions).unwrap_or("[]".into()))
        .replace("{{FIRESTORE}}", &firestore_json);
    Ok(Html(html))
}
