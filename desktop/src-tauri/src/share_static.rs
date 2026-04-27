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

pub async fn asset_js_recipe_add() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_recipe_add.js"))
}

pub async fn asset_js_recipe_ingredients() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_recipe_ingredients.js"))
}

pub async fn asset_js_recipe_steps() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_recipe_steps.js"))
}
