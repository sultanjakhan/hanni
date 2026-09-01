// share_server.rs — Public HTTP server for share-links (guest-facing).
// Runs on 127.0.0.1:8239 (prod) / 8240 (dev). Cloudflare Tunnel exposes it to the internet.

use axum::{
    extract::{DefaultBodyLimit, Path, Request, State as AxumState},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, Response},
    routing::{delete, get, patch},
    Json, Router,
};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::share_auth::{html_escape, load_link, rate_limit_check, BODY_LIMIT_BYTES};
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

/// Accept browser origins only from an exact loopback host or the Tailscale
/// CGNAT range (100.64.0.0/10). Prefix checks are unsafe here: hosts such as
/// `localhost.evil.example` and public `100.128.0.0/9` addresses must not be
/// treated as trusted local origins.
fn is_allowed_share_origin(origin: &HeaderValue) -> bool {
    let Ok(raw) = origin.to_str() else {
        return false;
    };
    let Ok(uri) = raw.parse::<axum::http::Uri>() else {
        return false;
    };
    if uri.scheme_str() != Some("http") || uri.path() != "/" || uri.query().is_some() {
        return false;
    }
    let Some(authority) = uri.authority() else {
        return false;
    };
    if authority.as_str().contains('@') {
        return false;
    }

    let host = authority.host().trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) if ip.is_loopback() => true,
        Ok(IpAddr::V4(ip)) => {
            let octets = ip.octets();
            octets[0] == 100 && (64..=127).contains(&octets[1])
        }
        Ok(IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => false,
    }
}

/// Single production CORS contract for the guest share server. Keep tests on
/// this layer itself so route-level refactors cannot silently broaden origins,
/// methods, headers, or credential behavior.
fn share_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            is_allowed_share_origin(origin)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::CONTENT_TYPE])
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
        // Reject oversized bodies before Bytes/Json extractors allocate them.
        // Handler-level checks remain as defense in depth.
        .layer(DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        // Strip Referer on outbound navigation — share-link tokens live in
        // URL paths, leaking them via the Referer header to any external
        // page a guest happens to open is a token-exposure risk.
        .layer(middleware::from_fn(add_security_headers))
        // Allow same-host guest origins only: localhost/loopback for dev
        // tooling, and any Tailscale CGNAT origin (100.64.0.0/10) — guests in
        // the same tailnet hit the server directly at http://100.x.x.x:8240/...
        .layer(share_cors_layer());

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

#[cfg(test)]
mod tests {
    use super::{is_allowed_share_origin, share_cors_layer};
    use axum::{
        body::Body,
        http::{header, HeaderValue, Method, Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn allowed(origin: &'static str) -> bool {
        is_allowed_share_origin(&HeaderValue::from_static(origin))
    }

    fn cors_test_router() -> Router {
        Router::new()
            .route("/probe", get(|| async { StatusCode::NO_CONTENT }))
            .layer(share_cors_layer())
    }

    async fn preflight(
        origin: &'static str,
        requested_method: Method,
        requested_headers: &'static str,
    ) -> axum::response::Response {
        cors_test_router()
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/probe")
                    .header(header::ORIGIN, origin)
                    .header(
                        header::ACCESS_CONTROL_REQUEST_METHOD,
                        requested_method.as_str(),
                    )
                    .header(header::ACCESS_CONTROL_REQUEST_HEADERS, requested_headers)
                    .body(Body::empty())
                    .expect("build preflight request"),
            )
            .await
            .expect("preflight response")
    }

    async fn actual(origin: Option<&'static str>) -> axum::response::Response {
        let mut request = Request::builder().method(Method::GET).uri("/probe");
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        cors_test_router()
            .oneshot(
                request
                    .body(Body::empty())
                    .expect("build actual request"),
            )
            .await
            .expect("actual response")
    }

    fn assert_default_cors_vary(response: &axum::response::Response) {
        let vary = response
            .headers()
            .get(header::VARY)
            .expect("CORS Vary header")
            .to_str()
            .expect("Vary text");
        for expected in [
            "origin",
            "access-control-request-method",
            "access-control-request-headers",
        ] {
            assert!(
                vary.split(',').any(|value| value.trim() == expected),
                "missing {expected} in Vary: {vary}"
            );
        }
    }

    #[test]
    fn cors_accepts_exact_loopback_and_tailscale_origins() {
        assert!(allowed("http://localhost:8239"));
        assert!(allowed("http://127.0.0.1:8239"));
        assert!(allowed("http://127.42.0.7"));
        assert!(allowed("http://[::1]:8239"));
        assert!(allowed("http://100.64.0.1:8239"));
        assert!(allowed("http://100.127.255.254"));
    }

    #[test]
    fn cors_rejects_prefix_confusion_and_non_tailscale_hosts() {
        assert!(!allowed("http://localhost.evil.example:8239"));
        assert!(!allowed("http://127.0.0.1.evil.example"));
        assert!(!allowed("http://100.63.255.255"));
        assert!(!allowed("http://100.128.0.1"));
        assert!(!allowed("http://100.example"));
        assert!(!allowed("https://localhost:8239"));
        assert!(!allowed("http://user@localhost:8239"));
        assert!(!allowed("http://localhost:8239/path"));
        assert!(!allowed("http://localhost:8239/?q=1"));
        assert!(!allowed("null"));
    }

    #[tokio::test]
    async fn cors_preflight_advertises_only_the_production_contract() {
        let origin = "http://100.64.0.1:8240";
        let response = preflight(origin, Method::PATCH, "content-type").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static(origin))
        );
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_METHODS),
            Some(&HeaderValue::from_static("GET,POST,PATCH,DELETE"))
        );
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_HEADERS),
            Some(&HeaderValue::from_static("content-type"))
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
        assert_default_cors_vary(&response);
    }

    #[tokio::test]
    async fn cors_preflight_rejects_hostile_origins_in_the_actual_middleware() {
        for origin in [
            "http://localhost.evil.example:8240",
            "http://user@localhost:8240",
            "http://100.63.255.255:8240",
            "http://100.128.0.1:8240",
            "https://localhost:8240",
        ] {
            let response = preflight(origin, Method::PATCH, "content-type").await;

            assert_eq!(response.status(), StatusCode::OK, "origin {origin}");
            assert!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .is_none(),
                "origin {origin}"
            );
            assert_eq!(
                response.headers().get(header::ACCESS_CONTROL_ALLOW_METHODS),
                Some(&HeaderValue::from_static("GET,POST,PATCH,DELETE"))
            );
            assert_eq!(
                response.headers().get(header::ACCESS_CONTROL_ALLOW_HEADERS),
                Some(&HeaderValue::from_static("content-type"))
            );
            assert!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                    .is_none()
            );
        }
    }

    #[tokio::test]
    async fn cors_preflight_does_not_advertise_put() {
        let origin = "http://127.0.0.1:8240";
        let response = preflight(origin, Method::PUT, "content-type").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static(origin))
        );
        let methods = response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .expect("fixed allow-methods")
            .to_str()
            .expect("allow-methods text");
        assert!(!methods.split(',').any(|method| method.trim() == "PUT"));
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_HEADERS),
            Some(&HeaderValue::from_static("content-type"))
        );
    }

    #[tokio::test]
    async fn cors_preflight_does_not_advertise_authorization() {
        let origin = "http://127.0.0.1:8240";
        let response = preflight(origin, Method::POST, "authorization").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static(origin))
        );
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_METHODS),
            Some(&HeaderValue::from_static("GET,POST,PATCH,DELETE"))
        );
        let headers = response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("fixed allow-headers")
            .to_str()
            .expect("allow-headers text");
        assert!(!headers
            .split(',')
            .any(|name| name.trim() == "authorization"));
    }

    #[tokio::test]
    async fn cors_actual_response_echoes_only_an_allowed_origin() {
        for (origin, expected_allow_origin) in [
            ("http://127.0.0.1:8240", true),
            ("https://localhost:8240", false),
            ("http://100.128.0.1:8240", false),
        ] {
            let response = actual(Some(origin)).await;

            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            if expected_allow_origin {
                assert_eq!(
                    response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
                    Some(&HeaderValue::from_static(origin)),
                    "origin {origin}"
                );
            } else {
                assert!(
                    response
                        .headers()
                        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                        .is_none(),
                    "origin {origin}"
                );
            }
            assert!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                    .is_none()
            );
            assert!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                    .is_none()
            );
            assert!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                    .is_none()
            );
            assert_default_cors_vary(&response);
        }
    }

    #[tokio::test]
    async fn cors_actual_response_without_origin_is_an_ordinary_response() {
        let response = actual(None).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none()
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .is_none()
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .is_none()
        );
        assert_default_cors_vary(&response);
    }
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
