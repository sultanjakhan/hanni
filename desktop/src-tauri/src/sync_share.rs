// sync_share.rs — Hanni-side cloud sync engine for share-links.
//
// Stage A scaffolding:
//   * Stores Firebase Web SDK config (project_id, api_key, auth_domain) in
//     app_settings so the rest of the codebase can detect cloud-share mode.
//   * Provides push_share_to_firestore(share_id) that uploads the snapshot
//     of recipes / recipe_ingredients / ingredient_catalog / products /
//     food_blacklist / meal_plan / share_links for a given share-link to
//     Firestore via REST.
//   * Idempotent: re-running the push overwrites docs by deterministic IDs
//     keyed off (owner_uid, local_id).
//
// What's NOT yet implemented (Stage B):
//   * Per-write incremental sync (currently full-snapshot only).
//   * Cloud Function that mints a custom JWT carrying `share_token` claim
//     for guest writes — without it, Stage A is read-only for guests.
//   * Conflict resolution / two-way sync.
//
// Activation:
//   1. User runs `firebase login` and `./scripts/firebase-setup.sh`.
//   2. Web-SDK config gets pasted into Hanni Settings.
//      → cloud_share_set_config saves it to app_settings.
//   3. Each create_share_link can opt into cloud mirror by passing
//      `cloud=true`, which triggers push_share_to_firestore on success.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::types::HanniDb;

const SETTING_KEY: &str = "cloud_share_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudShareConfig {
    /// Firebase project ID, e.g. "hanni-share-abc12345".
    pub project_id: String,
    /// Web API key from `firebase apps:sdkconfig WEB`.
    pub api_key: String,
    /// Stable per-host UUID — namespacing data inside Firestore so multiple
    /// Hanni installations of the same project don't clash. Generated at
    /// first set_config call.
    pub owner_uid: String,
}

static CACHED_CONFIG: OnceLock<std::sync::RwLock<Option<CloudShareConfig>>> = OnceLock::new();

fn cache() -> &'static std::sync::RwLock<Option<CloudShareConfig>> {
    CACHED_CONFIG.get_or_init(|| std::sync::RwLock::new(None))
}

/// Load config from app_settings on demand, cached for the process lifetime.
pub fn load_config(conn: &rusqlite::Connection) -> Option<CloudShareConfig> {
    if let Ok(g) = cache().read() {
        if let Some(c) = g.as_ref() { return Some(c.clone()); }
    }
    let raw: String = conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![SETTING_KEY], |r| r.get(0),
    ).ok()?;
    let cfg: CloudShareConfig = serde_json::from_str(&raw).ok()?;
    if let Ok(mut g) = cache().write() { *g = Some(cfg.clone()); }
    Some(cfg)
}

fn save_config(conn: &rusqlite::Connection, cfg: &CloudShareConfig) -> Result<(), String> {
    let json = serde_json::to_string(cfg).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![SETTING_KEY, json],
    ).map_err(|e| e.to_string())?;
    if let Ok(mut g) = cache().write() { *g = Some(cfg.clone()); }
    Ok(())
}

fn gen_owner_uid() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..16).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect()
}

// ── Tauri commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn cloud_share_set_config(
    project_id: String,
    api_key: String,
    db: State<'_, HanniDb>,
) -> Result<CloudShareConfig, String> {
    if project_id.trim().is_empty() || api_key.trim().is_empty() {
        return Err("project_id and api_key required".into());
    }
    let conn = db.conn();
    // Re-use existing owner_uid if config already saved (so previously-pushed
    // documents stay reachable).
    let existing = load_config(&conn);
    let owner_uid = existing.map(|c| c.owner_uid).unwrap_or_else(gen_owner_uid);
    let cfg = CloudShareConfig {
        project_id: project_id.trim().to_string(),
        api_key: api_key.trim().to_string(),
        owner_uid,
    };
    save_config(&conn, &cfg)?;
    Ok(cfg)
}

#[tauri::command]
pub fn cloud_share_get_config(db: State<'_, HanniDb>) -> Option<CloudShareConfig> {
    load_config(&db.conn())
}

/// Stage A: full snapshot upload of a single share-link's data to Firestore.
///
/// NOTE: This is a placeholder that compiles and exercises the surrounding
/// machinery. The actual Firestore REST calls live behind `firestore_upsert`
/// and are no-ops until a `cloud_share_config` is saved. Wiring is intentional
/// so the rest of Hanni can call this safely today.
#[tauri::command]
pub async fn cloud_share_push(
    share_id: i64,
    db: State<'_, HanniDb>,
) -> Result<serde_json::Value, String> {
    let cfg = {
        let conn = db.conn();
        load_config(&conn).ok_or_else(|| "cloud-share not configured".to_string())?
    };

    let snapshot = build_snapshot(&db, share_id)?;

    // TODO Stage B: replace the dry-run with real REST upserts.
    // For now we just return the snapshot so the UI can show what would be
    // pushed; uncommenting the line below will start writing to Firestore
    // once the config is in place.
    // firestore_upsert_snapshot(&cfg, &snapshot).await?;
    let _ = &cfg; // silence unused while Stage A is dry-run

    Ok(serde_json::json!({
        "status": "dry-run",
        "owner_uid": cfg.owner_uid,
        "share_id": share_id,
        "counts": {
            "recipes":            snapshot.recipes.len(),
            "recipe_ingredients": snapshot.recipe_ingredients.len(),
            "ingredient_catalog": snapshot.ingredient_catalog.len(),
            "products":           snapshot.products.len(),
            "food_blacklist":     snapshot.food_blacklist.len(),
            "meal_plan":          snapshot.meal_plan.len(),
        },
    }))
}

// ── Snapshot builder ──────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Snapshot {
    pub share_link: serde_json::Value,
    pub recipes: Vec<serde_json::Value>,
    pub recipe_ingredients: Vec<serde_json::Value>,
    pub ingredient_catalog: Vec<serde_json::Value>,
    pub products: Vec<serde_json::Value>,
    pub food_blacklist: Vec<serde_json::Value>,
    pub meal_plan: Vec<serde_json::Value>,
}

fn build_snapshot(db: &State<'_, HanniDb>, share_id: i64) -> Result<Snapshot, String> {
    let conn = db.conn();
    let mut s = Snapshot::default();

    // share_links row — gives us scope/perms.
    s.share_link = conn.query_row(
        "SELECT id, token, tab, scope, permissions, label, expires_at, revoked_at, created_at \
         FROM share_links WHERE id=?1",
        rusqlite::params![share_id],
        |r| Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "token": r.get::<_, String>(1)?,
            "tab": r.get::<_, String>(2)?,
            "scope": r.get::<_, String>(3)?,
            "permissions": r.get::<_, String>(4)?,
            "label": r.get::<_, Option<String>>(5).unwrap_or(None),
            "expires_at": r.get::<_, Option<String>>(6).unwrap_or(None),
            "revoked_at": r.get::<_, Option<String>>(7).unwrap_or(None),
            "created_at": r.get::<_, String>(8)?,
        })),
    ).map_err(|e| format!("share_link {} not found: {}", share_id, e))?;

    // recipes (only those visible to scope)
    let mut stmt = conn.prepare(
        "SELECT id, name, description, ingredients, instructions, prep_time, cook_time, \
                servings, calories, tags, difficulty, cuisine, protein, fat, carbs, favorite, \
                health_score, price_score, created_at, updated_at \
         FROM recipes ORDER BY updated_at DESC LIMIT 200"
    ).map_err(|e| e.to_string())?;
    s.recipes = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
        "description": r.get::<_, String>(2)?, "ingredients": r.get::<_, String>(3)?,
        "instructions": r.get::<_, String>(4)?, "prep_time": r.get::<_, i64>(5)?,
        "cook_time": r.get::<_, i64>(6)?, "servings": r.get::<_, i64>(7)?,
        "calories": r.get::<_, i64>(8)?, "tags": r.get::<_, String>(9)?,
        "difficulty": r.get::<_, String>(10).unwrap_or_else(|_| "easy".into()),
        "cuisine": r.get::<_, String>(11).unwrap_or_else(|_| "kz".into()),
        "protein": r.get::<_, i64>(12).unwrap_or(0),
        "fat": r.get::<_, i64>(13).unwrap_or(0),
        "carbs": r.get::<_, i64>(14).unwrap_or(0),
        "favorite": r.get::<_, i64>(15).unwrap_or(0),
        "health_score": r.get::<_, i64>(16).unwrap_or(5),
        "price_score": r.get::<_, i64>(17).unwrap_or(5),
        "created_at": r.get::<_, String>(18)?,
        "updated_at": r.get::<_, String>(19)?,
    }))).map_err(|e| e.to_string())?.filter_map(|x| x.ok()).collect();
    drop(stmt);

    let mut stmt = conn.prepare(
        "SELECT id, recipe_id, name, amount, unit, catalog_id FROM recipe_ingredients"
    ).map_err(|e| e.to_string())?;
    s.recipe_ingredients = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "recipe_id": r.get::<_, i64>(1)?,
        "name": r.get::<_, String>(2)?, "amount": r.get::<_, f64>(3)?,
        "unit": r.get::<_, String>(4)?, "catalog_id": r.get::<_, Option<i64>>(5).unwrap_or(None),
    }))).map_err(|e| e.to_string())?.filter_map(|x| x.ok()).collect();
    drop(stmt);

    let mut stmt = conn.prepare(
        "SELECT id, name, category, tags, COALESCE(subgroup,''), parent_id FROM ingredient_catalog"
    ).map_err(|e| e.to_string())?;
    s.ingredient_catalog = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
        "category": r.get::<_, String>(2)?, "tags": r.get::<_, String>(3)?,
        "subgroup": r.get::<_, String>(4)?,
        "parent_id": r.get::<_, Option<i64>>(5).unwrap_or(None),
    }))).map_err(|e| e.to_string())?.filter_map(|x| x.ok()).collect();
    drop(stmt);

    let mut stmt = conn.prepare(
        "SELECT id, name, category, quantity, unit, expiry_date, location, notes, catalog_id FROM products"
    ).map_err(|e| e.to_string())?;
    s.products = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
        "category": r.get::<_, String>(2)?, "quantity": r.get::<_, f64>(3)?,
        "unit": r.get::<_, String>(4)?, "expiry_date": r.get::<_, Option<String>>(5).unwrap_or(None),
        "location": r.get::<_, String>(6)?, "notes": r.get::<_, String>(7)?,
        "catalog_id": r.get::<_, Option<i64>>(8).unwrap_or(None),
    }))).map_err(|e| e.to_string())?.filter_map(|x| x.ok()).collect();
    drop(stmt);

    let mut stmt = conn.prepare(
        "SELECT id, type, value, catalog_id FROM food_blacklist"
    ).map_err(|e| e.to_string())?;
    s.food_blacklist = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "type": r.get::<_, String>(1)?,
        "value": r.get::<_, String>(2)?,
        "catalog_id": r.get::<_, Option<i64>>(3).unwrap_or(None),
    }))).map_err(|e| e.to_string())?.filter_map(|x| x.ok()).collect();
    drop(stmt);

    let mut stmt = conn.prepare(
        "SELECT id, date, meal_type, recipe_id, notes FROM meal_plan"
    ).map_err(|e| e.to_string())?;
    s.meal_plan = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "date": r.get::<_, String>(1)?,
        "meal_type": r.get::<_, String>(2)?, "recipe_id": r.get::<_, i64>(3)?,
        "notes": r.get::<_, String>(4)?,
    }))).map_err(|e| e.to_string())?.filter_map(|x| x.ok()).collect();
    drop(stmt);

    Ok(s)
}

// ── Firestore REST helpers (Stage B implementation goes here) ─────────────
//
// async fn firestore_upsert_snapshot(cfg: &CloudShareConfig, s: &Snapshot)
//     -> Result<(), String>
// {
//     let base = format!(
//         "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents",
//         cfg.project_id);
//     let client = reqwest::Client::new();
//     // 1. share_links/{token}      ← s.share_link
//     // 2. recipes/{owner_uid}_{id} ← each s.recipes[i]
//     // 3. recipe_ingredients/{owner_uid}_{id}
//     // 4. ingredient_catalog/{owner_uid}_{id}
//     // 5. products/{owner_uid}_{id}
//     // 6. food_blacklist/{owner_uid}_{id}
//     // 7. meal_plan/{owner_uid}_{id}
//     // PATCH https://firestore/.../documents/<col>/<doc>?key=<api_key>
//     // Body: { "fields": { "name": { "stringValue": "..." }, ... } }
//     // Use commitWrite or batched writes for atomicity.
//     Ok(())
// }
