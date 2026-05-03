// sync_owner.rs — Stage D: snapshot-based owner sync via Firestore.
//
// Replaces the cr-sqlite changeset model (rejected by cr-sqlite for tables
// with INTEGER PRIMARY KEY AUTOINCREMENT — see memory
// `tech/project_crsqlite_pk_constraint.md`). Each device pushes its own
// dirty rows of the 7 sync-target tables to Firestore and pulls back any
// rows touched by other devices since the last cursor.
//
// Layout:
//   owners/{owner_uid}/v2/{table}/rows/{row_id}        — row mirror, latest copy
//   owners/{owner_uid}/v2/tombstones/rows/{table}_{id} — delete record
//
// Conflict resolution: last-write-wins on `_updated_at` (UTC ISO-8601).
// Echoes are filtered out via `_device_id` (each install has a stable UUID
// in app_settings). Cursors are per-table strings stored in app_settings.

use crate::db::SYNC_TABLES;
use crate::google_auth::{load_session as load_google_session,
                          load_config as load_google_config};
use crate::sync_share::{firestore_host, get_access_token, json_to_field,
                         load_config as load_share_config};
use crate::types::HanniDb;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

/// Resolve `(service_account_token, owner_uid, project_id)` for owner sync.
/// Uses the same Firebase service-account JWT path as `sync_share`, which
/// bypasses Firestore security rules — so users don't need to deploy custom
/// rules just to get multi-device sync working. Path-isolation by `owner_uid`
/// is still enforced because every document lives under `/owners/{uid}/...`.
///
/// project_id MUST come from google_auth (Sign-in-with-Google) — that's the
/// project the user actually authenticated against and where the cloud
/// owner_uid lives. cloud_share_config.project_id can point at a different
/// Firebase project (e.g. an older Stage-A test project on Android), which
/// would silently send pushes/pulls to a project where no other device looks.
async fn resolve_creds(db: &HanniDb) -> Result<(String, String, String), String> {
    let (cfg, uid, project_id) = {
        let conn = db.conn();
        let cfg = load_share_config(&conn)
            .ok_or_else(|| "cloud-share not configured (need service account)".to_string())?;
        let session = load_google_session(&conn)
            .ok_or_else(|| "Sign in with Google first".to_string())?;
        let google_cfg = load_google_config(&conn)
            .ok_or_else(|| "Google auth not configured".to_string())?;
        (cfg, session.uid, google_cfg.project_id)
    };
    let token = get_access_token(&cfg).await?;
    Ok((token, uid, project_id))
}

const PULL_LIMIT: i32 = 500;
const PUSH_LIMIT: usize = 500;
const EPOCH_TS: &str = "1970-01-01T00:00:00Z";

#[derive(Debug, Serialize)]
pub struct OwnerSyncStatus {
    pub configured: bool,
    pub device_id: String,
    pub last_push_ts: Option<String>,
    pub last_pull_ts: Option<String>,
    pub owner_uid: Option<String>,
}

// ── Settings helpers ─────────────────────────────────────────────────────

fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![key], |r| r.get(0),
    ).ok()
}

fn set_setting(conn: &Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    );
}

fn device_id(conn: &Connection) -> String {
    get_setting(conn, "device_id").unwrap_or_else(|| "unknown".into())
}

fn push_cursor_key(table: &str) -> String { format!("cloud_owner_v2_push_{}", table) }
fn pull_cursor_key(table: &str) -> String { format!("cloud_owner_v2_pull_{}", table) }

// ── Row ↔ Firestore document codec ───────────────────────────────────────

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| format!("table_info {}: {}", table, e))?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| format!("query_map: {}", e))?;
    let mut out = Vec::new();
    for r in rows { out.push(r.map_err(|e| format!("row: {}", e))?); }
    Ok(out)
}

fn row_to_json(conn: &Connection, table: &str, id: i64) -> Result<Option<Value>, String> {
    let cols = table_columns(conn, table)?;
    let select = cols.join(", ");
    let sql = format!("SELECT {} FROM {} WHERE id = ?1", select, table);
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prep {}: {}", table, e))?;
    let row = stmt.query_row(rusqlite::params![id], |r| {
        let mut obj = serde_json::Map::new();
        for (i, name) in cols.iter().enumerate() {
            let v: rusqlite::types::Value = r.get(i)?;
            obj.insert(name.clone(), sqlite_to_json(v));
        }
        Ok(Value::Object(obj))
    });
    match row {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("row_to_json {} #{}: {}", table, id, e)),
    }
}

fn sqlite_to_json(v: rusqlite::types::Value) -> Value {
    use rusqlite::types::Value as SV;
    match v {
        SV::Null      => Value::Null,
        SV::Integer(i) => Value::Number(i.into()),
        SV::Real(f)    => serde_json::Number::from_f64(f)
            .map(Value::Number).unwrap_or(Value::Null),
        SV::Text(s)    => Value::String(s),
        SV::Blob(b)    => Value::String(format!("blob:{}", b.len())),
    }
}

fn encode_doc(row: &Value, device_id: &str, updated_at: &str, table: &str) -> Value {
    let mut obj = match row {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    obj.insert("_device_id".into(), Value::String(device_id.into()));
    obj.insert("_updated_at".into(), Value::String(updated_at.into()));
    // _table is what makes the collectionGroup query routable on pull —
    // without it we can't tell which table a row should be applied to.
    obj.insert("_table".into(), Value::String(table.into()));
    let mut fields = serde_json::Map::new();
    for (k, v) in obj { fields.insert(k, json_to_field(&v)); }
    json!({ "fields": fields })
}

fn decode_field(f: &Value) -> Value {
    if let Some(s) = f.get("stringValue").and_then(|v| v.as_str()) { return Value::String(s.into()); }
    if let Some(s) = f.get("integerValue").and_then(|v| v.as_str()) {
        if let Ok(i) = s.parse::<i64>() { return Value::Number(i.into()); }
    }
    if let Some(i) = f.get("integerValue").and_then(|v| v.as_i64()) { return Value::Number(i.into()); }
    if let Some(d) = f.get("doubleValue").and_then(|v| v.as_f64()) {
        return serde_json::Number::from_f64(d).map(Value::Number).unwrap_or(Value::Null);
    }
    if let Some(b) = f.get("booleanValue").and_then(|v| v.as_bool()) { return Value::Bool(b); }
    Value::Null
}

fn decode_doc(doc: &Value) -> serde_json::Map<String, Value> {
    let empty = serde_json::Map::new();
    let fields = doc.get("fields").and_then(|v| v.as_object()).unwrap_or(&empty);
    let mut out = serde_json::Map::new();
    for (k, v) in fields { out.insert(k.clone(), decode_field(v)); }
    out
}

// ── Firestore I/O ────────────────────────────────────────────────────────

async fn patch_doc(client: &reqwest::Client, token: &str, project_id: &str,
                    path: &str, doc_id: &str, body: &Value) -> Result<(), String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents/{}/{}",
        firestore_host(), project_id, path, doc_id,
    );
    let resp = client.patch(&url).bearer_auth(token).json(body)
        .send().await.map_err(|e| format!("PATCH {}: {}", path, e))?;
    if !resp.status().is_success() {
        let s = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("PATCH {} {}: {} — {}", path, doc_id, s, txt));
    }
    Ok(())
}

async fn run_query(client: &reqwest::Client, token: &str, project_id: &str,
                   parent_path: &str, collection_id: &str, since_ts: &str,
                   all_descendants: bool)
                   -> Result<Vec<Value>, String> {
    // Single-field filter avoids Firestore's composite-index requirement.
    // Echo-filtering by `_device_id` happens in the caller after the fetch.
    let url = format!("{}/projects/{}/databases/(default)/documents/{}:runQuery",
        firestore_host(), project_id, parent_path);
    let body = json!({
        "structuredQuery": {
            "from": [{
                "collectionId": collection_id,
                "allDescendants": all_descendants,
            }],
            "where": {"fieldFilter": {
                "field": {"fieldPath": "_updated_at"},
                "op": "GREATER_THAN",
                "value": {"stringValue": since_ts}
            }},
            "orderBy": [{"field": {"fieldPath": "_updated_at"}, "direction": "ASCENDING"}],
            "limit": PULL_LIMIT
        }
    });
    let resp = client.post(&url).bearer_auth(token).json(&body)
        .send().await.map_err(|e| format!("runQuery {}: {}", collection_id, e))?;
    let status = resp.status();
    let payload: Value = resp.json().await.map_err(|e| format!("body: {}", e))?;
    if !status.is_success() {
        return Err(format!("runQuery {} {}: {}", collection_id, status, payload));
    }
    Ok(payload.as_array().cloned().unwrap_or_default())
}

// ── Per-table push ───────────────────────────────────────────────────────

async fn push_table(db: &HanniDb, table: &str, client: &reqwest::Client,
                    token: &str, project_id: &str, owner_uid: &str)
                    -> Result<usize, String>
{
    let (rows, max_ts, dev_id) = {
        let conn = db.conn();
        let cursor = get_setting(&conn, &push_cursor_key(table))
            .unwrap_or_else(|| EPOCH_TS.into());
        let dev = device_id(&conn);
        let mut stmt = conn.prepare(&format!(
            "SELECT id, updated_at FROM {} WHERE updated_at > ?1 \
             ORDER BY updated_at ASC LIMIT {}", table, PUSH_LIMIT))
            .map_err(|e| format!("prep dirty {}: {}", table, e))?;
        let dirty: Vec<(i64, String)> = stmt.query_map(rusqlite::params![cursor], |r|
            Ok((r.get(0)?, r.get(1)?))
        ).map_err(|e| format!("dirty {}: {}", table, e))?
            .filter_map(Result::ok).collect();
        drop(stmt);
        let mut payloads: Vec<(i64, String, Value)> = Vec::new();
        let mut max = cursor.clone();
        for (id, ts) in &dirty {
            if let Some(row) = row_to_json(&conn, table, *id)? {
                payloads.push((*id, ts.clone(), row));
                if ts > &max { max = ts.clone(); }
            }
        }
        (payloads, max, dev)
    };

    let path = format!("owners/{}/v2/{}/rows", owner_uid, table);
    let mut pushed = 0usize;
    for (id, ts, row) in &rows {
        let body = encode_doc(row, &dev_id, ts, table);
        patch_doc(client, token, project_id, &path, &id.to_string(), &body).await?;
        pushed += 1;
    }
    if pushed > 0 {
        let conn = db.conn();
        set_setting(&conn, &push_cursor_key(table), &max_ts);
    }
    Ok(pushed)
}

async fn push_tombstones(db: &HanniDb, client: &reqwest::Client,
                         token: &str, project_id: &str, owner_uid: &str)
                         -> Result<usize, String>
{
    let (rows, max_ts, dev_id) = {
        let conn = db.conn();
        let cursor = get_setting(&conn, "cloud_owner_v2_push_tombstones")
            .unwrap_or_else(|| EPOCH_TS.into());
        let dev = device_id(&conn);
        let mut stmt = conn.prepare(
            "SELECT table_name, row_id, deleted_at FROM sync_tombstones \
             WHERE deleted_at > ?1 ORDER BY deleted_at ASC LIMIT 500"
        ).map_err(|e| format!("prep tombstones: {}", e))?;
        let mut max = cursor.clone();
        let dirty: Vec<(String, i64, String)> = stmt.query_map(
            rusqlite::params![cursor],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
        ).map_err(|e| format!("tombstones: {}", e))?
            .filter_map(Result::ok)
            .inspect(|(_, _, ts)| { if ts > &max { max = ts.clone(); } })
            .collect();
        (dirty, max, dev)
    };

    let path = format!("owners/{}/v2/tombstones/rows", owner_uid);
    let mut pushed = 0usize;
    for (table, id, ts) in &rows {
        let row = json!({ "_target_table": table, "_row_id": id, "_deleted": true });
        let doc_id = format!("{}_{}", table, id);
        // Use "tombstones" as the _table marker so the collectionGroup pull
        // can distinguish tombstone docs from regular row docs.
        let body = encode_doc(&row, &dev_id, ts, "tombstones");
        patch_doc(client, token, project_id, &path, &doc_id, &body).await?;
        pushed += 1;
    }
    if pushed > 0 {
        let conn = db.conn();
        set_setting(&conn, "cloud_owner_v2_push_tombstones", &max_ts);
    }
    Ok(pushed)
}

// ── Per-table pull ───────────────────────────────────────────────────────

fn upsert_row(conn: &Connection, table: &str, fields: &serde_json::Map<String, Value>)
              -> Result<bool, String>
{
    let id = fields.get("id").and_then(|v| v.as_i64())
        .ok_or_else(|| format!("{}: row missing integer id", table))?;
    let remote_ts = fields.get("_updated_at").and_then(|v| v.as_str())
        .unwrap_or("");

    // LWW: skip if local is newer-or-equal.
    let local_ts: Option<String> = conn.query_row(
        &format!("SELECT updated_at FROM {} WHERE id = ?1", table),
        rusqlite::params![id], |r| r.get(0),
    ).ok();
    if let Some(local) = &local_ts {
        if local.as_str() >= remote_ts { return Ok(false); }
    }

    let cols = table_columns(conn, table)?;
    let cols: Vec<&str> = cols.iter().map(|s| s.as_str())
        .filter(|c| fields.contains_key(*c)).collect();
    if cols.is_empty() { return Ok(false); }

    let placeholders = (1..=cols.len()).map(|i| format!("?{}", i)).collect::<Vec<_>>().join(",");
    let updates = cols.iter().filter(|c| **c != "id")
        .map(|c| format!("{0} = excluded.{0}", c)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) \
         ON CONFLICT(id) DO UPDATE SET {}",
        table, cols.join(","), placeholders, updates
    );

    let params: Vec<rusqlite::types::Value> = cols.iter().map(|c| {
        json_to_sqlite(fields.get(*c).unwrap_or(&Value::Null))
    }).collect();
    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    conn.execute(&sql, refs.as_slice())
        .map_err(|e| format!("upsert {} #{}: {}", table, id, e))?;
    Ok(true)
}

fn json_to_sqlite(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as SV;
    match v {
        Value::Null      => SV::Null,
        Value::Bool(b)   => SV::Integer(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() { SV::Integer(i) }
            else if let Some(f) = n.as_f64() { SV::Real(f) }
            else { SV::Text(n.to_string()) }
        }
        Value::String(s) => SV::Text(s.clone()),
        other            => SV::Text(other.to_string()),
    }
}

/// Single collectionGroup pull — fetches every row touched by any device since
/// our cursor, across ALL sync tables, in one Firestore query. Without this
/// pull would do 57 queries/tick and blow past the Spark-plan free quota.
/// Each doc carries its own `_table` field so we know where to apply it.
async fn pull_all(db: &HanniDb, client: &reqwest::Client,
                  token: &str, project_id: &str, owner_uid: &str)
                  -> Result<serde_json::Map<String, Value>, String>
{
    let (since, dev_id) = {
        let conn = db.conn();
        let s = get_setting(&conn, "cloud_owner_v2_pull_global")
            .unwrap_or_else(|| EPOCH_TS.into());
        (s, device_id(&conn))
    };
    let parent = format!("owners/{}", owner_uid);
    let docs = run_query(client, token, project_id, &parent, "rows", &since, true).await?;

    let allowed: std::collections::HashSet<&str> = SYNC_TABLES.iter().copied().collect();
    let mut totals = serde_json::Map::new();
    let mut max_ts = since.clone();
    let conn = db.conn();

    for row_doc in &docs {
        let Some(doc) = row_doc.get("document") else { continue };
        let fields = decode_doc(doc);
        if let Some(ts) = fields.get("_updated_at").and_then(|v| v.as_str()) {
            if ts > max_ts.as_str() { max_ts = ts.into(); }
        }
        if fields.get("_device_id").and_then(|v| v.as_str()) == Some(dev_id.as_str()) {
            continue;
        }
        let Some(table) = fields.get("_table").and_then(|v| v.as_str()) else { continue };

        if table == "tombstones" {
            let Some(target) = fields.get("_target_table").and_then(|v| v.as_str()) else { continue };
            let Some(id)    = fields.get("_row_id").and_then(|v| v.as_i64()) else { continue };
            if !allowed.contains(target) { continue; }
            let _ = conn.execute(
                &format!("DELETE FROM {} WHERE id = ?1", target),
                rusqlite::params![id],
            );
            *totals.entry("tombstones".to_string()).or_insert(json!(0))
                = json!(totals.get("tombstones").and_then(|v| v.as_u64()).unwrap_or(0) + 1);
        } else {
            if !allowed.contains(table) { continue; }
            if upsert_row(&conn, table, &fields)? {
                let cur = totals.get(table).and_then(|v| v.as_u64()).unwrap_or(0);
                totals.insert(table.to_string(), json!(cur + 1));
            }
        }
    }
    if max_ts != since {
        set_setting(&conn, "cloud_owner_v2_pull_global", &max_ts);
    }
    Ok(totals)
}

// ── Top-level push/pull ──────────────────────────────────────────────────

pub(crate) async fn push_inner(db: &HanniDb) -> Result<Value, String> {
    let (token, owner_uid, project_id) = resolve_creds(db).await?;
    let client = reqwest::Client::new();
    let mut totals = serde_json::Map::new();
    let mut total = 0usize;
    for table in SYNC_TABLES {
        let n = push_table(db, table, &client, &token, &project_id, &owner_uid).await
            .map_err(|e| format!("push {}: {}", table, e))?;
        totals.insert((*table).into(), json!(n));
        total += n;
    }
    let n = push_tombstones(db, &client, &token, &project_id, &owner_uid).await?;
    totals.insert("tombstones".into(), json!(n));
    total += n;
    {
        let conn = db.conn();
        set_setting(&conn, "cloud_owner_v2_last_push_ts", &chrono::Utc::now().to_rfc3339());
    }
    Ok(json!({ "pushed": total, "by_table": Value::Object(totals) }))
}

pub(crate) async fn pull_inner(db: &HanniDb) -> Result<Value, String> {
    let (token, owner_uid, project_id) = resolve_creds(db).await?;
    let client = reqwest::Client::new();
    let totals = pull_all(db, &client, &token, &project_id, &owner_uid).await
        .map_err(|e| format!("pull: {e}"))?;
    let total: u64 = totals.values().filter_map(|v| v.as_u64()).sum();
    {
        let conn = db.conn();
        set_setting(&conn, "cloud_owner_v2_last_pull_ts", &chrono::Utc::now().to_rfc3339());
    }
    Ok(json!({ "applied": total, "by_table": Value::Object(totals) }))
}

// ── Tauri commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_owner_push(db: State<'_, HanniDb>) -> Result<Value, String> {
    push_inner(&db).await
}

#[tauri::command]
pub async fn cloud_owner_pull(db: State<'_, HanniDb>) -> Result<Value, String> {
    pull_inner(&db).await
}

/// Debug helper — runs a raw Firestore query for one collection and returns
/// the document count + list of names so we can see whether pull is empty
/// because the cloud is empty or because of a query/auth bug.
#[tauri::command]
pub async fn debug_owner_list(table: String, db: State<'_, HanniDb>)
    -> Result<Value, String>
{
    let (token, owner_uid, project_id) = resolve_creds(&db).await?;
    let client = reqwest::Client::new();
    let parent = format!("owners/{}/v2/{}", owner_uid, table);
    let url = format!("{}/projects/{}/databases/(default)/documents/{}",
        firestore_host(), project_id, parent);
    let resp = client.get(&url).bearer_auth(&token)
        .query(&[("pageSize", "50")])
        .send().await.map_err(|e| format!("get: {}", e))?;
    let status = resp.status();
    let body: Value = resp.json().await.map_err(|e| format!("body: {}", e))?;
    let names: Vec<String> = body.get("documents").and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|d| d.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
        .unwrap_or_default();
    Ok(json!({
        "url": url,
        "status": status.as_u16(),
        "count": names.len(),
        "first": names.into_iter().take(5).collect::<Vec<_>>(),
        "raw_keys": body.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

#[tauri::command]
pub fn cloud_owner_status(db: State<'_, HanniDb>) -> OwnerSyncStatus {
    let conn = db.conn();
    let session = load_google_session(&conn);
    let cfg = load_google_config(&conn);
    let configured = cfg.is_some() && session.is_some();
    OwnerSyncStatus {
        configured,
        device_id: device_id(&conn),
        last_push_ts: get_setting(&conn, "cloud_owner_v2_last_push_ts"),
        last_pull_ts: get_setting(&conn, "cloud_owner_v2_last_pull_ts"),
        owner_uid: session.map(|s| s.uid),
    }
}
