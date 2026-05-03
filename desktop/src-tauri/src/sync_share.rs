// sync_share.rs — Hanni-side cloud sync engine for share-links.
//
// Stage A (initial): full-snapshot push to Firestore via REST, signed with
// the user's Firebase service-account JSON. Service-account scope `datastore`
// bypasses Firestore security rules — appropriate because Hanni is the data
// owner.
//
// Stage C-1 (current): mirror writes into per-share-link sub-collections
// `share_links/{token}/{table}/{id}` so Firestore rules can grant `allow read`
// on the whole sub-tree using the URL token as the only secret (no Firebase
// Auth needed). Background loop in sync_share_auto.rs polls the dirty-flag
// queue every few seconds; write-commands in commands_data.rs / commands_share.rs
// call `mark_dirty(&conn, "recipes")` after a successful DB op, the loop
// pushes affected share-links whose scope covers the dirty table.

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

/// Deterministic owner_uid = first 16 hex chars of sha256(client_email).
/// Two devices using the same Firebase service account get the same UID,
/// so they share `owners/{uid}/changes/` automatically.
pub(crate) fn derive_owner_uid_from_email(email: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(email.as_bytes());
    hex::encode(&digest[..8])
}

/// Reset push/pull bookmarks so the next sync re-uploads everything under
/// the new UID and re-pulls the foreign collection from scratch. Called
/// whenever owner_uid changes (deterministic-derive or manual import).
pub(crate) fn reset_sync_bookmarks(conn: &rusqlite::Connection) {
    let _ = conn.execute(
        "DELETE FROM app_settings WHERE key IN \
         ('cloud_owner_last_push_ver','cloud_owner_last_pull_ts')",
        [],
    );
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

/// List doc IDs currently in `share_links/{token}/{coll}` on Firestore.
/// Used by mirror to compute the orphan set (docs the snapshot no longer
/// contains, so they should be deleted from cloud).
async fn list_collection_ids(
    client: &reqwest::Client, cfg: &CloudShareConfig, token: &str, path: &str,
) -> Result<Vec<(String, i64)>, String> {
    // Returns Vec<(full_resource_name, parsed_id)>. Walks pageToken so we
    // see every doc, not just the first page.
    let mut out = Vec::new();
    let mut page_token: Option<String> = None;
    for _ in 0..20 { // hard cap: 20 pages × 300 = 6000 docs
        let mut url = format!(
            "{}/projects/{}/databases/(default)/documents/{}?pageSize=300",
            FIRESTORE_HOST, cfg.project_id, path,
        );
        if let Some(t) = &page_token {
            url.push_str(&format!("&pageToken={}", t));
        }
        let resp = client.get(&url).bearer_auth(token).send().await
            .map_err(|e| format!("LIST {}: {}", path, e))?;
        if !resp.status().is_success() {
            // 404 on empty sub-collection is normal for fresh share-links.
            return Ok(out);
        }
        let body: serde_json::Value = resp.json().await
            .map_err(|e| format!("LIST {} parse: {}", path, e))?;
        if let Some(arr) = body.get("documents").and_then(|v| v.as_array()) {
            for d in arr {
                if let Some(name) = d.get("name").and_then(|v| v.as_str()) {
                    let id_str = name.rsplit('/').next().unwrap_or("");
                    if let Ok(id) = id_str.parse::<i64>() {
                        out.push((name.to_string(), id));
                    }
                }
            }
        }
        page_token = body.get("nextPageToken").and_then(|v| v.as_str()).map(String::from);
        if page_token.is_none() { break; }
    }
    Ok(out)
}

async fn delete_doc_by_name(
    client: &reqwest::Client, token: &str, full_name: &str,
) -> Result<(), String> {
    // Firestore delete URL: https://firestore.googleapis.com/v1/{full_name}
    let url = format!("https://firestore.googleapis.com/v1/{}", full_name);
    let resp = client.delete(&url).bearer_auth(token).send().await
        .map_err(|e| format!("DELETE {}: {}", full_name, e))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("Firestore DELETE {} {}: {}", full_name, status, txt));
    }
    Ok(())
}

async fn firestore_upsert_snapshot(cfg: &CloudShareConfig, snap: &Snapshot)
    -> Result<serde_json::Value, String>
{
    let token = get_access_token(cfg).await?;
    let client = reqwest::Client::new();
    let mut written = std::collections::BTreeMap::<String, usize>::new();
    let mut deleted = std::collections::BTreeMap::<String, usize>::new();

    let token_id = snap.share_link.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if token_id.is_empty() {
        return Err("share_link.token missing".into());
    }
    let scope = snap.share_link.get("scope").and_then(|v| v.as_str()).unwrap_or("all").to_string();

    // share_links/{token} — top-level doc with scope/permissions/expires.
    // Guest fetches this first to discover what views to render.
    patch_doc(&client, cfg, &token, "share_links", &token_id,
              doc_payload(&snap.share_link, &cfg.owner_uid)).await?;
    written.insert("share_links".into(), 1);

    // Sub-collections live under share_links/{token}/{coll}/{id}. Path is
    // built once per collection, doc-id is just the local SQLite rowid
    // (uniqueness is per share-link, not global).
    //
    // Sync semantics:
    //   1. Upsert every row from the snapshot (PATCH).
    //   2. List Firestore-side docs and DELETE any whose id isn't in the
    //      snapshot — that handles deletions on the Hanni side.
    macro_rules! sync_sub {
        ($coll:literal, $rows:expr) => {{
            let path = format!("share_links/{}/{}", token_id, $coll);
            let mut kept_ids = std::collections::HashSet::<i64>::new();
            for row in $rows {
                let id = row.get("id").and_then(|v| v.as_i64())
                    .ok_or_else(|| format!("{} row missing id", $coll))?;
                patch_doc(&client, cfg, &token, &path, &id.to_string(),
                          doc_payload(row, &cfg.owner_uid)).await?;
                kept_ids.insert(id);
            }
            written.insert($coll.into(), kept_ids.len());
            // Orphan sweep
            let existing = list_collection_ids(&client, cfg, &token, &path).await?;
            let mut del_n = 0usize;
            for (name, id) in existing {
                if !kept_ids.contains(&id) {
                    delete_doc_by_name(&client, &token, &name).await?;
                    del_n += 1;
                }
            }
            if del_n > 0 { deleted.insert($coll.into(), del_n); }
        }};
    }

    let need_recipes   = matches!(scope.as_str(), "all" | "recipes" | "meal_plan");
    let need_products  = matches!(scope.as_str(), "all" | "products" | "fridge");
    let need_blacklist = matches!(scope.as_str(), "all" | "memory");
    let need_meal_plan = matches!(scope.as_str(), "all" | "meal_plan");
    // ingredient_catalog provides categories/colors for ingredient tags;
    // useful anywhere that lists ingredients.
    let need_catalog   = need_recipes || need_products;

    if need_recipes {
        sync_sub!("recipes", &snap.recipes);
        sync_sub!("recipe_ingredients", &snap.recipe_ingredients);
    }
    if need_catalog {
        sync_sub!("ingredient_catalog", &snap.ingredient_catalog);
    }
    if need_products {
        sync_sub!("products", &snap.products);
    }
    if need_blacklist {
        sync_sub!("food_blacklist", &snap.food_blacklist);
    }
    if need_meal_plan {
        sync_sub!("meal_plan", &snap.meal_plan);
    }

    Ok(serde_json::json!({
        "status": "ok", "scope": scope,
        "written": written, "deleted": deleted,
    }))
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
    let parsed_email: Option<String> = if let Some(ref sa) = service_account_json {
        let parsed: ServiceAccount = serde_json::from_str(sa)
            .map_err(|e| format!("service_account_json invalid: {}", e))?;
        Some(parsed.client_email)
    } else { None };
    let conn = db.conn();
    let existing = load_config(&conn);
    let prev_uid = existing.as_ref().map(|c| c.owner_uid.clone());
    // Owner UID priority: deterministic from email (if SA provided) >
    // existing UID (preserve) > random fallback (true fresh install).
    let owner_uid = parsed_email
        .as_deref()
        .map(derive_owner_uid_from_email)
        .or_else(|| prev_uid.clone())
        .unwrap_or_else(gen_owner_uid);
    // Preserve previously-saved service_account if caller didn't pass a new one.
    let service_account_json = service_account_json
        .or_else(|| existing.and_then(|c| c.service_account_json));
    let cfg = CloudShareConfig {
        project_id: project_id.trim().to_string(),
        api_key: api_key.trim().to_string(),
        owner_uid: owner_uid.clone(),
        service_account_json,
    };
    save_config(&conn, &cfg)?;
    if prev_uid.as_deref() != Some(owner_uid.as_str()) {
        reset_sync_bookmarks(&conn);
    }
    Ok(cfg)
}

/// Manually override owner_uid (UID-import flow: paste UID copied from
/// another device that shares the same Firebase project). Resets push/pull
/// bookmarks so the next sync round re-pushes/re-pulls everything.
#[tauri::command]
pub fn cloud_owner_set_uid(uid: String, db: State<'_, HanniDb>) -> Result<String, String> {
    let uid = uid.trim().to_string();
    if uid.is_empty() { return Err("uid is empty".into()); }
    let conn = db.conn();
    let mut cfg = load_config(&conn)
        .ok_or_else(|| "cloud-share not configured (set project_id/api_key first)".to_string())?;
    if cfg.owner_uid == uid { return Ok(uid); }
    cfg.owner_uid = uid.clone();
    save_config(&conn, &cfg)?;
    reset_sync_bookmarks(&conn);
    Ok(uid)
}

#[tauri::command]
pub fn cloud_owner_get_uid(db: State<'_, HanniDb>) -> Option<String> {
    load_config(&db.conn()).map(|c| c.owner_uid)
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
    let snapshot = build_snapshot(db.inner(), share_id)?;

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

fn build_snapshot(db: &HanniDb, share_id: i64) -> Result<Snapshot, String> {
    let conn = db.conn();
    let mut s = Snapshot::default();

    // Latest tunnel URL — guests on Firebase Hosting read this from
    // share_links/{token}.tunnel_url and POST writes there. Null when
    // cloudflared isn't running (host offline → guest goes read-only).
    let tunnel_url: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key='share_tunnel_url'",
        [], |r| r.get(0),
    ).ok();

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
            "tunnel_url": tunnel_url,
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

// ── Dirty-flag mirror queue (Stage C-1) ──────────────────────────────────
// Write-paths in commands_data.rs / commands_share.rs call mark_dirty(...)
// after a successful DB op. The background loop in sync_share_auto.rs picks
// up dirty flags every few seconds and pushes affected share-links.

const DIRTY_PREFIX: &str = "cloud_share_dirty_";

pub fn mark_dirty(conn: &rusqlite::Connection, table: &str) {
    let key = format!("{}{}", DIRTY_PREFIX, table);
    let _ = conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, '1') \
         ON CONFLICT(key) DO UPDATE SET value='1'",
        rusqlite::params![key],
    );
}

fn list_dirty_tables(conn: &rusqlite::Connection) -> Vec<String> {
    let pattern = format!("{}%", DIRTY_PREFIX);
    let mut stmt = match conn.prepare(
        "SELECT key FROM app_settings WHERE key LIKE ?1 AND value='1'"
    ) { Ok(s) => s, Err(_) => return Vec::new() };
    stmt.query_map(rusqlite::params![pattern], |r| r.get::<_, String>(0))
        .map(|it| it.filter_map(|x| x.ok())
            .map(|k| k.trim_start_matches(DIRTY_PREFIX).to_string()).collect())
        .unwrap_or_default()
}

fn clear_dirty(conn: &rusqlite::Connection) {
    let pattern = format!("{}%", DIRTY_PREFIX);
    let _ = conn.execute(
        "UPDATE app_settings SET value='0' WHERE key LIKE ?1",
        rusqlite::params![pattern],
    );
}

/// Returns true if a share-link with `scope` mirrors writes to `table`.
fn scope_covers(scope: &str, table: &str) -> bool {
    // share_links metadata (incl. tunnel_url) is pushed for every active link
    // regardless of scope — guests on Firebase Hosting read it to discover
    // where to POST writes.
    if table == "share_links" { return true; }
    match (scope, table) {
        ("all", _) => true,
        ("recipes", "recipes" | "recipe_ingredients" | "ingredient_catalog") => true,
        ("products", "products" | "ingredient_catalog") => true,
        ("fridge", "products" | "ingredient_catalog") => true,
        ("meal_plan", "meal_plan" | "recipes" | "recipe_ingredients" | "ingredient_catalog") => true,
        ("memory", "food_blacklist") => true,
        _ => false,
    }
}

/// Push every active share-link whose scope covers any currently-dirty table.
/// On full success, clears dirty flags. On any error, leaves them set so the
/// next loop tick retries.
pub(crate) async fn mirror_pending(db: &HanniDb) -> Result<serde_json::Value, String> {
    let dirty: Vec<String> = list_dirty_tables(&db.conn());
    if dirty.is_empty() {
        return Ok(serde_json::json!({ "status": "clean" }));
    }

    let active: Vec<(i64, String)> = {
        let conn = db.conn();
        let now_iso = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, scope FROM share_links \
             WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?1)"
        ).map_err(|e| e.to_string())?;
        let mapped = stmt.query_map(rusqlite::params![now_iso],
                          |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        let rows: Vec<(i64, String)> = mapped.filter_map(|x| x.ok()).collect();
        rows
    };

    if active.is_empty() {
        clear_dirty(&db.conn());
        return Ok(serde_json::json!({ "status": "no-active-links", "dirty": dirty }));
    }

    let cfg = match load_config(&db.conn()) {
        Some(c) if c.service_account_json.is_some() => c,
        _ => return Ok(serde_json::json!({ "status": "not-configured" })),
    };

    let mut pushed = Vec::new();
    let mut errors = Vec::new();
    for (share_id, scope) in &active {
        if !dirty.iter().any(|t| scope_covers(scope, t)) { continue; }
        let snap = match build_snapshot(db, *share_id) {
            Ok(s) => s,
            Err(e) => { errors.push(serde_json::json!({"share_id": share_id, "error": e})); continue; }
        };
        match firestore_upsert_snapshot(&cfg, &snap).await {
            Ok(v) => pushed.push(serde_json::json!({"share_id": share_id, "scope": scope, "result": v})),
            Err(e) => errors.push(serde_json::json!({"share_id": share_id, "scope": scope, "error": e})),
        }
    }

    if errors.is_empty() {
        clear_dirty(&db.conn());
    }

    Ok(serde_json::json!({
        "status": if errors.is_empty() { "ok" } else { "partial" },
        "dirty": dirty,
        "pushed": pushed,
        "errors": errors,
    }))
}

