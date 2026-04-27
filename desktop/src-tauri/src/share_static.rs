// share_static.rs — guest.css / guest.js static responses

use axum::{http::header, response::IntoResponse};

const JS: &str = "application/javascript; charset=utf-8";

pub async fn asset_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], include_str!("share_assets/guest.css"))
}

pub async fn asset_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest.js"))
}

pub async fn asset_js_recipes() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_recipes.js"))
}

pub async fn asset_js_products() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_products.js"))
}

pub async fn asset_js_meal_plan() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_meal_plan.js"))
}

pub async fn asset_js_memory() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_memory.js"))
}

pub async fn asset_js_recipe_add() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_recipe_add.js"))
}

// Shared add-recipe modal — same source file as Hanni (desktop/src/js/recipe-shared.js).
// Compile-time inlined; both frontends always serve identical bytes.
pub async fn asset_js_recipe_shared() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("../../src/js/recipe-shared.js"))
}
