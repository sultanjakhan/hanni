// sync_owner.rs — Owner-side CRDT sync via Firestore.
//
// Push/pull cr-sqlite changes between devices that share the same
// owner_uid (e.g. Mac + Android of the same user). Reuses Firebase
// service-account auth from sync_share.rs.
//
// Collection layout:  owners/{owner_uid}/changes/{site_id}_{db_v}_{cl}_{seq}
//
// Push:  current device's local crsql_changes since last_push_ver are
//        PATCH'd as Firestore documents. Idempotent — same doc-id =
//        upsert.
// Pull:  list all docs in the changes collection, skip ones whose
//        site_id matches our own (those are echoes), apply the rest
//        through apply_remote_changes (which is also idempotent thanks
//        to cr-sqlite's CRDT semantics).

use crate::sync::{Change, get_local_changes, apply_remote_changes, get_db_version, get_site_id};
use crate::sync_share::{CloudShareConfig, get_access_token, load_config, json_to_field, firestore_host};
use crate::types::HanniDb;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct OwnerSyncStatus {
    pub configured: bool,
    pub site_id: String,
    pub last_push_ver: i64,
    pub last_pull_ts: Option<String>,
    pub pending_changes: usize,
    pub owner_uid: Option<String>,
}

// ── Settings helpers ─────────────────────────────────────────────────────

fn get_setting(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![key], |r| r.get(0),
    ).ok()
}

fn set_setting(conn: &rusqlite::Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    );
}

// ── Firestore document encoding/decoding ────────────────────────────────

fn change_to_doc(c: &Change) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    let s = |x: &str| serde_json::Value::String(x.into());
    let i = |x: i64| serde_json::Value::Number(x.into());
    fields.insert("table".into(),       json_to_field(&s(&c.table)));
    fields.insert("pk".into(),          json_to_field(&s(&c.pk)));
    fields.insert("cid".into(),         json_to_field(&s(&c.cid)));
    if let Some(v) = c.val.as_ref() {
        let val_str = match v { serde_json::Value::String(x) => x.clone(), other => other.to_string() };
        fields.insert("val".into(),     json_to_field(&serde_json::Value::String(val_str)));
    }
    fields.insert("col_version".into(), json_to_field(&i(c.col_version)));
    fields.insert("db_version".into(),  json_to_field(&i(c.db_version)));
    fields.insert("site_id".into(),     json_to_field(&s(&c.site_id)));
    fields.insert("cl".into(),          json_to_field(&i(c.cl)));
    fields.insert("seq".into(),         json_to_field(&i(c.seq)));
    fields.insert("created_at".into(),  json_to_field(&s(&chrono::Utc::now().to_rfc3339())));
    serde_json::json!({ "fields": fields })
}

fn field_str(doc: &serde_json::Value, name: &str) -> Option<String> {
    doc.get("fields")?.get(name)?.get("stringValue")?.as_str().map(String::from)
}
fn field_int(doc: &serde_json::Value, name: &str) -> Option<i64> {
    let f = doc.get("fields")?.get(name)?;
    if let Some(s) = f.get("integerValue").and_then(|v| v.as_str()) { return s.parse().ok(); }
    f.get("integerValue").and_then(|v| v.as_i64())
}

fn doc_to_change(doc: &serde_json::Value) -> Option<Change> {
    Some(Change {
        table:       field_str(doc, "table")?,
        pk:          field_str(doc, "pk")?,
        cid:         field_str(doc, "cid")?,
        val:         field_str(doc, "val").map(serde_json::Value::String),
        col_version: field_int(doc, "col_version")?,
        db_version:  field_int(doc, "db_version")?,
        site_id:     field_str(doc, "site_id")?,
        cl:          field_int(doc, "cl")?,
        seq:         field_int(doc, "seq")?,
    })
}

// ── Tauri commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_owner_push(db: State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let cfg = {
        let conn = db.conn();
        load_config(&conn).ok_or_else(|| "cloud-share not configured".to_string())?
    };
    let token = get_access_token(&cfg).await?;
    let client = reqwest::Client::new();

    let (changes, db_ver, site_id) = {
        let conn = db.conn();
        let last_push: i64 = get_setting(&conn, "cloud_owner_last_push_ver")
            .and_then(|s| s.parse().ok()).unwrap_or(0);
        let ch = get_local_changes(&conn, last_push);
        (ch, get_db_version(&conn), get_site_id(&conn))
    };

    let collection = format!("owners/{}/changes", cfg.owner_uid);
    let mut pushed = 0usize;
    for c in &changes {
        let doc_id = format!("{}_{}_{}_{}", c.site_id, c.db_version, c.cl, c.seq);
        let url = format!(
            "{}/projects/{}/databases/(default)/documents/{}/{}",
            firestore_host(), cfg.project_id, collection, doc_id
        );
        let resp = client.patch(&url)
            .bearer_auth(&token)
            .json(&change_to_doc(c))
            .send().await.map_err(|e| format!("PATCH change: {}", e))?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("Firestore push: {}", txt));
        }
        pushed += 1;
    }

    {
        let conn = db.conn();
        set_setting(&conn, "cloud_owner_last_push_ver", &db_ver.to_string());
        set_setting(&conn, "cloud_owner_site_id", &site_id);
    }

    Ok(serde_json::json!({ "pushed": pushed, "db_version": db_ver, "site_id": site_id }))
}

#[tauri::command]
pub async fn cloud_owner_pull(db: State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let (cfg, my_site_id) = {
        let conn = db.conn();
        let cfg = load_config(&conn).ok_or_else(|| "cloud-share not configured".to_string())?;
        (cfg, get_site_id(&conn))
    };
    let token = get_access_token(&cfg).await?;
    let client = reqwest::Client::new();

    let collection = format!("owners/{}/changes", cfg.owner_uid);
    let url = format!(
        "{}/projects/{}/databases/(default)/documents/{}?pageSize=300",
        firestore_host(), cfg.project_id, collection
    );
    let resp = client.get(&url).bearer_auth(&token)
        .send().await.map_err(|e| format!("GET changes: {}", e))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("body: {}", e))?;
    if !status.is_success() {
        return Err(format!("Firestore pull {}: {}", status, body));
    }

    let docs = body.get("documents").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut applied = 0usize;
    let mut skipped = 0usize;
    {
        let conn = db.conn();
        for doc in &docs {
            if let Some(c) = doc_to_change(doc) {
                if c.site_id == my_site_id { skipped += 1; continue; }
                apply_remote_changes(&conn, std::slice::from_ref(&c))?;
                applied += 1;
            }
        }
        set_setting(&conn, "cloud_owner_last_pull_ts", &chrono::Utc::now().to_rfc3339());
    }

    Ok(serde_json::json!({ "applied": applied, "skipped_own": skipped, "total": docs.len() }))
}

#[tauri::command]
pub fn cloud_owner_status(db: State<'_, HanniDb>) -> OwnerSyncStatus {
    let conn = db.conn();
    let cfg = load_config(&conn);
    let configured = cfg.as_ref().and_then(|c| c.service_account_json.as_ref()).is_some();
    let last_push: i64 = get_setting(&conn, "cloud_owner_last_push_ver")
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    OwnerSyncStatus {
        configured,
        site_id: get_site_id(&conn),
        last_push_ver: last_push,
        last_pull_ts: get_setting(&conn, "cloud_owner_last_pull_ts"),
        pending_changes: get_local_changes(&conn, last_push).len(),
        owner_uid: cfg.map(|c| c.owner_uid),
    }
}
