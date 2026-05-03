// share_static.rs — guest.css / guest.js static responses

use axum::{http::header, response::IntoResponse};

const JS: &str = "application/javascript; charset=utf-8";

pub async fn asset_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], include_str!("share_assets/guest.css"))
}

pub async fn asset_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest.js"))
}

// Stage C-1 Firestore REST client. Loaded before guest.js so view-modules
// can read window.HanniGuest.firestore from the moment they mount.
pub async fn asset_js_firestore() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_firestore.js"))
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

pub async fn asset_js_fridge() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_fridge.js"))
}

// Shared fridge UI — same source file as Hanni.
pub async fn asset_js_fridge_shared() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("../../src/js/fridge-shared.js"))
}

pub async fn asset_js_recipe_add() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("share_assets/guest_recipe_add.js"))
}

// Shared add-recipe modal — same source file as Hanni (desktop/src/js/recipe-shared.js).
// Compile-time inlined; both frontends always serve identical bytes.
pub async fn asset_js_recipe_shared() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("../../src/js/recipe-shared.js"))
}

// Ingredient-row helpers; loaded BEFORE recipe-shared.js (registers HanniRecipe.ingredients).
pub async fn asset_js_recipe_shared_ingredients() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, JS)], include_str!("../../src/js/recipe-shared-ingredients.js"))
}
