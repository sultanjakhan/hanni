// sync_share.rs — Hanni-side cloud sync engine for share-links.
//
// Stage A (this commit): full-snapshot push to Firestore via REST, signed
// with the user's Firebase service-account JSON. The service-account scope
// `datastore` bypasses Firestore security rules — appropriate because Hanni
// is the data owner. Guests read through the Web SDK with rules that gate
// access by share_token in the document path.
//
// Activation
//   1. `firebase login` + `./scripts/firebase-setup.sh`         (one-shot)
//   2. Firebase Console → Project settings → Service accounts →
//      «Generate new private key» → save the JSON.
//   3. Hanni DevTools (or Settings UI when wired):
//        await __TAURI__.core.invoke('cloud_share_set_config', {
//          projectId, apiKey,
//          serviceAccountJson: '<paste full file contents>'
//        });
//   4. await __TAURI__.core.invoke('cloud_share_push', { shareId: 1 });
//      Pushes a snapshot for that share_link to Firestore.
//
// Stage B (later) will add per-write incremental sync + a Cloud Function
// that mints custom JWTs so guests can write. This module is intentionally
// kept self-contained so Stage B can plug in without churn.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::types::HanniDb;

const SETTING_KEY: &str = "cloud_share_config";
const FIRESTORE_HOST: &str = "https://firestore.googleapis.com/v1";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/datastore";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudShareConfig {
    pub project_id: String,
    pub api_key: String,
    pub owner_uid: String,
    /// Raw service-account JSON (the file Firebase Console gives you).
    /// Contains client_email + private_key + token_uri.
    pub service_account_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(default)]
    token_uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

static CACHED_CONFIG: OnceLock<std::sync::RwLock<Option<CloudShareConfig>>> = OnceLock::new();
static CACHED_TOKEN: OnceLock<std::sync::RwLock<Option<(String, u64)>>> = OnceLock::new();

fn cfg_cache() -> &'static std::sync::RwLock<Option<CloudShareConfig>> {
    CACHED_CONFIG.get_or_init(|| std::sync::RwLock::new(None))
}
fn token_cache() -> &'static std::sync::RwLock<Option<(String, u64)>> {
    CACHED_TOKEN.get_or_init(|| std::sync::RwLock::new(None))
}

pub(crate) fn firestore_host() -> &'static str { FIRESTORE_HOST }

pub fn load_config(conn: &rusqlite::Connection) -> Option<CloudShareConfig> {
    if let Ok(g) = cfg_cache().read() {
        if let Some(c) = g.as_ref() { return Some(c.clone()); }
    }
    let raw: String = conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![SETTING_KEY], |r| r.get(0),
    ).ok()?;
    let cfg: CloudShareConfig = serde_json::from_str(&raw).ok()?;
    if let Ok(mut g) = cfg_cache().write() { *g = Some(cfg.clone()); }
    Some(cfg)
}

fn save_config(conn: &rusqlite::Connection, cfg: &CloudShareConfig) -> Result<(), String> {
    let json = serde_json::to_string(cfg).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![SETTING_KEY, json],
    ).map_err(|e| e.to_string())?;
    if let Ok(mut g) = cfg_cache().write() { *g = Some(cfg.clone()); }
    if let Ok(mut g) = token_cache().write() { *g = None; }
    Ok(())
}

fn gen_owner_uid() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..16).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect()
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ── OAuth2 access-token via service-account JWT ───────────────────────────

pub(crate) async fn get_access_token(cfg: &CloudShareConfig) -> Result<String, String> {
    if let Ok(g) = token_cache().read() {
        if let Some((tok, exp)) = g.as_ref() {
            if *exp > now_secs() + 30 { return Ok(tok.clone()); }
        }
    }
    let sa_raw = cfg.service_account_json.as_deref()
        .ok_or_else(|| "service_account_json missing — paste it via cloud_share_set_config".to_string())?;
    let sa: ServiceAccount = serde_json::from_str(sa_raw)
        .map_err(|e| format!("service_account JSON invalid: {}", e))?;

    let iat = now_secs();
    let exp = iat + 3500;
    let aud = sa.token_uri.as_deref().unwrap_or(TOKEN_URL);
    let claims = JwtClaims { iss: &sa.client_email, scope: SCOPE, aud, iat, exp };
    let header = Header::new(Algorithm::RS256);
    let key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
        .map_err(|e| format!("private_key parse: {}", e))?;
    let jwt = encode(&header, &claims, &key).map_err(|e| format!("JWT sign: {}", e))?;

    let client = reqwest::Client::new();
    // OAuth2 token endpoint expects application/x-www-form-urlencoded.
    // Both `grant_type` value (encoded `:` → `%3A`) and the JWT (base64url —
    // already URL-safe) are pre-encoded inline.
    let body_str = format!(
        "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={}",
        jwt
    );
    let resp = client.post(aud)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body_str)
        .send().await.map_err(|e| format!("token request: {}", e))?;
    let status = resp.status();
    let body: String = resp.text().await.map_err(|e| format!("token body: {}", e))?;
    if !status.is_success() {
        return Err(format!("token endpoint {}: {}", status, body));
    }
    let tr: TokenResponse = serde_json::from_str(&body)
        .map_err(|e| format!("token parse: {} — {}", e, body))?;
    let new_exp = now_secs() + tr.expires_in;
    if let Ok(mut g) = token_cache().write() { *g = Some((tr.access_token.clone(), new_exp)); }
    Ok(tr.access_token)
}

// ── Firestore document encoding ───────────────────────────────────────────
// Firestore REST wants every value tagged: { "stringValue": "..." }, etc.

pub(crate) fn json_to_field(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Null         => serde_json::json!({ "nullValue": null }),
        Value::Bool(b)      => serde_json::json!({ "booleanValue": b }),
        Value::String(s)    => serde_json::json!({ "stringValue": s }),
        Value::Number(n)    => {
            if let Some(i) = n.as_i64() { serde_json::json!({ "integerValue": i.to_string() }) }
            else if let Some(f) = n.as_f64() { serde_json::json!({ "doubleValue": f }) }
            else { serde_json::json!({ "stringValue": n.to_string() }) }
        }
        Value::Array(arr)   => serde_json::json!({
            "arrayValue": { "values": arr.iter().map(json_to_field).collect::<Vec<_>>() }
        }),
        Value::Object(obj)  => {
            let mut fields = serde_json::Map::new();
            for (k, val) in obj { fields.insert(k.clone(), json_to_field(val)); }
            serde_json::json!({ "mapValue": { "fields": fields } })
        }
    }
}

fn doc_payload(row: &serde_json::Value, owner_uid: &str) -> serde_json::Value {
    let mut obj = match row {
        serde_json::Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    obj.insert("owner_uid".into(), serde_json::Value::String(owner_uid.to_string()));
    obj.insert("synced_at".into(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));
    let mut fields = serde_json::Map::new();
    for (k, v) in obj { fields.insert(k, json_to_field(&v)); }
    serde_json::json!({ "fields": fields })
}

pub(crate) async fn patch_doc(
    client: &reqwest::Client,
    cfg: &CloudShareConfig,
    token: &str,
    collection: &str,
    doc_id: &str,
    body: serde_json::Value,
) -> Result<(), String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents/{}/{}",
        FIRESTORE_HOST, cfg.project_id, collection, doc_id
    );
    let resp = client.patch(&url)
        .bearer_auth(token)
        .json(&body)
        .send().await.map_err(|e| format!("PATCH {}: {}", collection, e))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("Firestore PATCH {} {}: {} — {}", collection, doc_id, status, txt));
    }
    Ok(())
}

async fn firestore_upsert_snapshot(cfg: &CloudShareConfig, snap: &Snapshot)
    -> Result<serde_json::Value, String>
{
    let token = get_access_token(cfg).await?;
    let client = reqwest::Client::new();
    let mut written = std::collections::BTreeMap::<&str, usize>::new();

    // share_links — keyed by token (path-friendly for guest reads).
    let token_id = snap.share_link.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !token_id.is_empty() {
        patch_doc(&client, cfg, &token, "share_links", &token_id,
                  doc_payload(&snap.share_link, &cfg.owner_uid)).await?;
        *written.entry("share_links").or_default() += 1;
    }

    async fn push_collection(
        client: &reqwest::Client, cfg: &CloudShareConfig, token: &str,
        collection: &str, rows: &[serde_json::Value], owner_uid: &str,
    ) -> Result<usize, String> {
        let mut n = 0;
        for row in rows {
            let id = row.get("id").and_then(|v| v.as_i64())
                .ok_or_else(|| format!("{} row missing id", collection))?;
            let doc_id = format!("{}_{}", owner_uid, id);
            patch_doc(client, cfg, token, collection, &doc_id,
                      doc_payload(row, owner_uid)).await?;
            n += 1;
        }
        Ok(n)
    }

    written.insert("recipes",
        push_collection(&client, cfg, &token, "recipes",            &snap.recipes,            &cfg.owner_uid).await?);
    written.insert("recipe_ingredients",
        push_collection(&client, cfg, &token, "recipe_ingredients", &snap.recipe_ingredients, &cfg.owner_uid).await?);
    written.insert("ingredient_catalog",
        push_collection(&client, cfg, &token, "ingredient_catalog", &snap.ingredient_catalog, &cfg.owner_uid).await?);
    written.insert("products",
        push_collection(&client, cfg, &token, "products",           &snap.products,           &cfg.owner_uid).await?);
    written.insert("food_blacklist",
        push_collection(&client, cfg, &token, "food_blacklist",     &snap.food_blacklist,     &cfg.owner_uid).await?);
    written.insert("meal_plan",
        push_collection(&client, cfg, &token, "meal_plan",          &snap.meal_plan,          &cfg.owner_uid).await?);

    Ok(serde_json::json!({ "status": "ok", "written": written }))
}

// ── Tauri commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn cloud_share_set_config(
    project_id: String,
    api_key: String,
    service_account_json: Option<String>,
    db: State<'_, HanniDb>,
) -> Result<CloudShareConfig, String> {
    if project_id.trim().is_empty() || api_key.trim().is_empty() {
        return Err("project_id and api_key required".into());
    }
    if let Some(ref sa) = service_account_json {
        // Sanity-check it parses.
        let _: ServiceAccount = serde_json::from_str(sa)
            .map_err(|e| format!("service_account_json invalid: {}", e))?;
    }
    let conn = db.conn();
    let existing = load_config(&conn);
    let owner_uid = existing.as_ref().map(|c| c.owner_uid.clone()).unwrap_or_else(gen_owner_uid);
    // Preserve previously-saved service_account if caller didn't pass a new one.
    let service_account_json = service_account_json
        .or_else(|| existing.and_then(|c| c.service_account_json));
    let cfg = CloudShareConfig {
        project_id: project_id.trim().to_string(),
        api_key: api_key.trim().to_string(),
        owner_uid,
        service_account_json,
    };
    save_config(&conn, &cfg)?;
    Ok(cfg)
}

#[tauri::command]
pub fn cloud_share_get_config(db: State<'_, HanniDb>) -> Option<CloudShareConfig> {
    // Strip the service-account JSON from the response so it never leaves the
    // backend — the UI just needs to know it's set.
    load_config(&db.conn()).map(|mut c| {
        if c.service_account_json.is_some() {
            c.service_account_json = Some("<set>".into());
        }
        c
    })
}

#[tauri::command]
pub async fn cloud_share_push(
    share_id: i64,
    dry_run: Option<bool>,
    db: State<'_, HanniDb>,
) -> Result<serde_json::Value, String> {
    let cfg = load_config(&db.conn())
        .ok_or_else(|| "cloud-share not configured".to_string())?;
    let snapshot = build_snapshot(&db, share_id)?;

    if dry_run.unwrap_or(false) {
        return Ok(serde_json::json!({
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
        }));
    }

    firestore_upsert_snapshot(&cfg, &snapshot).await
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
