// share_server.rs — Public HTTP server for share-links (guest-facing).
// Runs on 127.0.0.1:8239 (prod) / 8240 (dev). Cloudflare Tunnel exposes it to the internet.

use axum::{
    extract::{Path, Request, State as AxumState},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, Response},
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
use crate::share_routes_food_meta::{create_blacklist_item, create_catalog_item, create_cuisine, delete_blacklist_item, list_blacklist, list_cuisines, list_fridge};
use crate::share_routes_meal_plan::{create_meal_plan, delete_meal_plan, list_meal_plan};
use crate::share_routes_products_read::list_products;
use crate::share_routes_products_write::{create_product, delete_product, update_product};
use crate::share_routes_recipes_read::{get_recipe, list_recipes};
use crate::share_routes_recipes_write::{create_recipe, update_recipe, delete_recipe};
use crate::share_static::{
    asset_css, asset_js, asset_js_fridge, asset_js_fridge_shared,
    asset_js_meal_plan, asset_js_memory, asset_js_products, asset_js_recipe_add,
    asset_js_recipe_shared, asset_js_recipe_shared_ingredients, asset_js_recipe_shared_steps,
    asset_js_recipes,
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
        .route("/s/{token}/blacklist", get(list_blacklist).post(create_blacklist_item))
        .route("/s/{token}/blacklist/{id}", delete(delete_blacklist_item))
        .route("/s/{token}/fridge", get(list_fridge))
        .route("/s/{token}/assets/guest.css", get(asset_css))
        .route("/s/{token}/assets/guest.js", get(asset_js))
        .route("/s/{token}/assets/guest_recipes.js", get(asset_js_recipes))
        .route("/s/{token}/assets/recipe-shared.js", get(asset_js_recipe_shared))
        .route("/s/{token}/assets/recipe-shared-ingredients.js", get(asset_js_recipe_shared_ingredients))
        .route("/s/{token}/assets/recipe-shared-steps.js", get(asset_js_recipe_shared_steps))
        .route("/s/{token}/assets/guest_recipe_add.js", get(asset_js_recipe_add))
        .route("/s/{token}/assets/guest_products.js", get(asset_js_products))
        .route("/s/{token}/assets/guest_meal_plan.js", get(asset_js_meal_plan))
        .route("/s/{token}/assets/guest_memory.js", get(asset_js_memory))
        .route("/s/{token}/assets/fridge-shared.js", get(asset_js_fridge_shared))
        .route("/s/{token}/assets/guest_fridge.js", get(asset_js_fridge))
        .with_state(state)
        // Strip Referer on outbound navigation — share-link tokens live in
        // URL paths, leaking them via the Referer header to any external
        // page a guest happens to open is a token-exposure risk.
        .layer(middleware::from_fn(add_security_headers))
        // Allow same-host guest origins only: localhost/loopback for dev
        // tooling, and any Tailscale CGNAT origin (100.64.0.0/10) — guests in
        // the same tailnet hit the server directly at http://100.x.x.x:8240/...
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    origin.to_str().map(|s|
                        s.starts_with("http://127.0.0.1")
                        || s.starts_with("http://localhost")
                        || s.starts_with("http://100.")
                    ).unwrap_or(false)
                }))
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE])
        );

    let port = share_port();
    // Bind on 0.0.0.0 (was 127.0.0.1) so guests on the same Tailnet can
    // reach this directly at http://<our-tailscale-ip>:8240/s/<token>.
    // Tokens are 192-bit URL-safe ids generated by gen_token() and
    // required on every route, so exposing the listener beyond loopback
    // is safe — without the token there is no way in.
    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(listener) => {
            eprintln!("[share] public server on 0.0.0.0:{}", port);
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

async fn add_security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "referrer-policy",
        HeaderValue::from_static("no-referrer"),
    );
    response
}

async fn landing(
    Path(token): Path<String>,
    AxumState(state): AxumState<ShareServerState>,
) -> Result<Html<String>, (StatusCode, String)> {
    rate_limit_check(&state, &token)?;
    let ctx = {
        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        load_link(&conn, &token)?
    };
    let html = include_str!("share_assets/guest.html")
        .replace("{{LABEL}}", &html_escape(&ctx.label))
        .replace("{{TAB}}", &html_escape(&ctx.tab))
        .replace("{{SCOPE}}", &html_escape(&ctx.scope))
        .replace("{{TOKEN}}", &html_escape(&token))
        .replace("{{PERMS}}", &serde_json::to_string(&ctx.permissions).unwrap_or("[]".into()));
    Ok(Html(html))
}
