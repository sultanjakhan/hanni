// sync_owner.rs — Stage D: snapshot-based owner sync via Firestore.
//
// Replaces the cr-sqlite changeset model (rejected by cr-sqlite for tables
// with INTEGER PRIMARY KEY AUTOINCREMENT — see memory
// `tech/project_crsqlite_pk_constraint.md`). Each device pushes its own
// dirty rows of the 7 sync-target tables to Firestore and pulls back any
// rows touched by other devices since the last cursor.
//
// Layout: owners/{owner_uid}/data/{opaque_document_id}. Rows and tombstones
// share one collection so a pull can page every sync table in one query.
//
// Conflict resolution: last-write-wins on `_updated_at` (UTC ISO-8601).
// Echoes are filtered out via `_device_id` (each install has a stable UUID
// in app_settings). Push and pull cursors use deterministic tuple tie-breakers.

use crate::db::{column_is_text, SYNC_TABLES};
use crate::google_auth::{load_config as load_google_config, load_session as load_google_session};
use crate::sync_share::{
    firestore_host, get_access_token, json_to_field, load_config as load_share_config,
};
use crate::types::HanniDb;
use rusqlite::{types::Value as SqlValue, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;

static FIRESTORE_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
        let cfg = load_share_config(&conn)?
            .ok_or_else(|| "cloud-share not configured (need service account)".to_string())?;
        let session =
            load_google_session(&conn)?.ok_or_else(|| "Sign in with Google first".to_string())?;
        let google_cfg =
            load_google_config(&conn)?.ok_or_else(|| "Google auth not configured".to_string())?;
        (cfg, session.uid, google_cfg.project_id)
    };
    let token = get_access_token(&cfg).await?;
    Ok((token, uid, project_id))
}

const PULL_LIMIT: i32 = 500;
const PUSH_LIMIT: usize = 500;
const EPOCH_TS: &str = "1970-01-01T00:00:00Z";

pub(crate) fn canonical_sync_timestamp(raw: &str, context: &str) -> Result<String, String> {
    let raw = raw.trim();
    if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Ok(timestamp
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true));
    }
    for format in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
        if let Ok(timestamp) = chrono::NaiveDateTime::parse_from_str(raw, format) {
            return Ok(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                timestamp,
                chrono::Utc,
            )
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true));
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        let timestamp = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| format!("{context}: invalid date"))?;
        return Ok(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            timestamp,
            chrono::Utc,
        )
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true));
    }
    Err(format!(
        "{context}: timestamp is not a supported UTC/local SQLite format"
    ))
}

#[derive(Debug, Serialize)]
pub struct OwnerSyncStatus {
    pub configured: bool,
    pub device_id: String,
    pub last_push_ts: Option<String>,
    pub last_pull_ts: Option<String>,
    pub owner_uid: Option<String>,
}

// ── Settings helpers ─────────────────────────────────────────────────────

pub(crate) fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    get_setting_checked(conn, key).ok().flatten()
}

pub(crate) fn get_setting_checked(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    crate::secret_store::get_setting(conn, key)
}

pub(crate) fn set_setting(conn: &Connection, key: &str, value: &str) {
    if let Err(error) = set_setting_checked(conn, key, value) {
        eprintln!("[sync_owner] setting write failed for {key}: {error}");
    }
}

pub(crate) fn set_setting_checked(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    crate::secret_store::set_setting(conn, key, value)
}

fn device_id(conn: &Connection) -> Result<String, String> {
    get_setting_checked(conn, "device_id")?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "owner sync device_id is missing".to_string())
}

fn push_cursor_key(table: &str) -> String {
    format!("cloud_owner_v2_push_{}", table)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowCursor {
    pub timestamp: String,
    pub id: Option<SqlValue>,
}

fn row_cursor_id_key(timestamp_key: &str) -> String {
    format!("{timestamp_key}_id")
}

fn encode_cursor_id(id: &SqlValue) -> Result<String, String> {
    match id {
        SqlValue::Integer(value) => Ok(format!("i:{value}")),
        SqlValue::Text(value) => Ok(format!("t:{value}")),
        other => Err(format!("unsupported sync cursor id: {other:?}")),
    }
}

fn decode_cursor_id(encoded: &str) -> Result<SqlValue, String> {
    if let Some(value) = encoded.strip_prefix("i:") {
        return value
            .parse::<i64>()
            .map(SqlValue::Integer)
            .map_err(|_| "invalid integer sync cursor id".to_string());
    }
    if let Some(value) = encoded.strip_prefix("t:") {
        return Ok(SqlValue::Text(value.to_string()));
    }
    Err("invalid sync cursor id encoding".to_string())
}

pub(crate) fn load_row_cursor(conn: &Connection, timestamp_key: &str) -> Result<RowCursor, String> {
    let timestamp = get_setting_checked(conn, timestamp_key)?;
    let encoded_id = get_setting_checked(conn, &row_cursor_id_key(timestamp_key))?;
    if timestamp.as_deref() == Some("") || encoded_id.as_deref() == Some("") {
        return Err(format!("sync cursor {timestamp_key} is empty"));
    }
    if timestamp.is_none() && encoded_id.is_some() {
        return Err(format!(
            "sync cursor {timestamp_key} has an id without a timestamp"
        ));
    }
    Ok(RowCursor {
        timestamp: match timestamp {
            Some(timestamp) => {
                canonical_sync_timestamp(&timestamp, &format!("sync cursor {timestamp_key}"))?
            }
            None => canonical_sync_timestamp(EPOCH_TS, "sync epoch")?,
        },
        id: encoded_id.as_deref().map(decode_cursor_id).transpose()?,
    })
}

pub(crate) fn save_row_cursor(
    conn: &Connection,
    timestamp_key: &str,
    cursor: &RowCursor,
) -> Result<(), String> {
    let id = cursor
        .id
        .as_ref()
        .ok_or_else(|| format!("sync cursor {timestamp_key} has no row id"))?;
    let timestamp =
        canonical_sync_timestamp(&cursor.timestamp, &format!("sync cursor {timestamp_key}"))?;
    set_setting_checked(conn, timestamp_key, &timestamp)?;
    set_setting_checked(
        conn,
        &row_cursor_id_key(timestamp_key),
        &encode_cursor_id(id)?,
    )
}

pub(crate) fn dirty_rows_after(
    conn: &Connection,
    table: &str,
    cursor: &RowCursor,
    limit: usize,
) -> Result<Vec<(SqlValue, String)>, String> {
    let cursor_timestamp =
        canonical_sync_timestamp(&cursor.timestamp, &format!("sync cursor for {table}"))?;
    let mut stmt = conn
        .prepare(&format!("SELECT id, updated_at FROM {table}"))
        .map_err(|error| format!("prepare dirty rows for {table}: {error}"))?;
    let mapped = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| format!("query dirty rows for {table}: {error}"))?;
    let mut rows = Vec::new();
    for row in mapped {
        let (id, timestamp) =
            row.map_err(|error| format!("decode dirty row for {table}: {error}"))?;
        if !matches!(id, SqlValue::Integer(_) | SqlValue::Text(_)) {
            return Err(format!("dirty row for {table} has an unsupported id"));
        }
        rows.push((
            id,
            canonical_sync_timestamp(&timestamp, &format!("{table}.updated_at"))?,
        ));
    }
    rows.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| match (&left.0, &right.0) {
                (SqlValue::Integer(left), SqlValue::Integer(right)) => left.cmp(right),
                (SqlValue::Text(left), SqlValue::Text(right)) => left.cmp(right),
                (SqlValue::Integer(_), SqlValue::Text(_)) => std::cmp::Ordering::Less,
                (SqlValue::Text(_), SqlValue::Integer(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    });
    let mut dirty = Vec::new();
    for row in rows {
        let after = if row.1 > cursor_timestamp {
            true
        } else if row.1 < cursor_timestamp {
            false
        } else if let Some(cursor_id) = &cursor.id {
            match (&row.0, cursor_id) {
                (SqlValue::Integer(row_id), SqlValue::Integer(cursor_id)) => row_id > cursor_id,
                (SqlValue::Text(row_id), SqlValue::Text(cursor_id)) => row_id > cursor_id,
                _ => {
                    return Err(format!(
                        "sync cursor for {table} has a primary-key type mismatch"
                    ));
                }
            }
        } else {
            true
        };
        if after {
            dirty.push(row);
            if dirty.len() == limit {
                break;
            }
        }
    }
    Ok(dirty)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TombstoneCursor {
    pub timestamp: String,
    pub table: Option<String>,
    pub row_id: Option<String>,
}

pub(crate) fn load_tombstone_cursor(
    conn: &Connection,
    timestamp_key: &str,
) -> Result<TombstoneCursor, String> {
    let timestamp = get_setting_checked(conn, timestamp_key)?;
    let table = get_setting_checked(conn, &format!("{timestamp_key}_table"))?;
    let row_id = get_setting_checked(conn, &format!("{timestamp_key}_row_id"))?;
    if timestamp.as_deref() == Some("")
        || table.as_deref() == Some("")
        || row_id.as_deref() == Some("")
        || table.is_some() != row_id.is_some()
        || (timestamp.is_none() && table.is_some())
    {
        return Err(format!("sync cursor {timestamp_key} is incomplete"));
    }
    Ok(TombstoneCursor {
        timestamp: match timestamp {
            Some(timestamp) => {
                canonical_sync_timestamp(&timestamp, &format!("sync cursor {timestamp_key}"))?
            }
            None => canonical_sync_timestamp(EPOCH_TS, "sync epoch")?,
        },
        table,
        row_id,
    })
}

pub(crate) fn save_tombstone_cursor(
    conn: &Connection,
    timestamp_key: &str,
    cursor: &TombstoneCursor,
) -> Result<(), String> {
    let table = cursor
        .table
        .as_deref()
        .ok_or_else(|| format!("sync cursor {timestamp_key} has no table"))?;
    let row_id = cursor
        .row_id
        .as_deref()
        .ok_or_else(|| format!("sync cursor {timestamp_key} has no row id"))?;
    let timestamp =
        canonical_sync_timestamp(&cursor.timestamp, &format!("sync cursor {timestamp_key}"))?;
    set_setting_checked(conn, timestamp_key, &timestamp)?;
    set_setting_checked(conn, &format!("{timestamp_key}_table"), table)?;
    set_setting_checked(conn, &format!("{timestamp_key}_row_id"), row_id)
}

pub(crate) fn dirty_tombstones_after(
    conn: &Connection,
    cursor: &TombstoneCursor,
    limit: usize,
) -> Result<Vec<(String, String, String)>, String> {
    let cursor_timestamp = canonical_sync_timestamp(&cursor.timestamp, "tombstone sync cursor")?;
    let mut rows = Vec::new();
    let mut stmt = conn
        .prepare("SELECT table_name, row_id, deleted_at FROM sync_tombstones")
        .map_err(|error| format!("prepare dirty tombstones: {error}"))?;
    let mapped = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|error| format!("query dirty tombstones: {error}"))?;
    for row in mapped {
        let (table, row_id, timestamp): (String, String, String) =
            row.map_err(|error| format!("decode dirty tombstone: {error}"))?;
        rows.push((
            table,
            row_id,
            canonical_sync_timestamp(&timestamp, "sync tombstone deleted_at")?,
        ));
    }
    rows.sort_by(|left, right| {
        left.2
            .cmp(&right.2)
            .then_with(|| left.0.cmp(&right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    Ok(rows
        .into_iter()
        .filter(|(table, row_id, timestamp)| {
            if timestamp > &cursor_timestamp {
                return true;
            }
            if timestamp < &cursor_timestamp {
                return false;
            }
            match (&cursor.table, &cursor.row_id) {
                (Some(cursor_table), Some(cursor_row_id)) => {
                    table > cursor_table || (table == cursor_table && row_id > cursor_row_id)
                }
                _ => true,
            }
        })
        .take(limit)
        .collect())
}

// ── Row ↔ Firestore document codec ───────────────────────────────────────

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| format!("table_info {}: {}", table, e))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| format!("query_map: {}", e))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("row: {}", e))?);
    }
    Ok(out)
}

pub(crate) fn row_to_json(
    conn: &Connection,
    table: &str,
    id: &rusqlite::types::Value,
) -> Result<Option<Value>, String> {
    let cols = table_columns(conn, table)?;
    let select = cols.join(", ");
    let sql = format!("SELECT {} FROM {} WHERE id = ?1", select, table);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prep {}: {}", table, e))?;
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
        Err(e) => Err(format!("row_to_json {} #{:?}: {}", table, id, e)),
    }
}

fn sqlite_to_json(v: rusqlite::types::Value) -> Value {
    use rusqlite::types::Value as SV;
    match v {
        SV::Null => Value::Null,
        SV::Integer(i) => Value::Number(i.into()),
        SV::Real(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        SV::Text(s) => Value::String(s),
        SV::Blob(b) => Value::String(format!("blob:{}", b.len())),
    }
}

fn encode_doc(row: &Value, device_id: &str, updated_at: &str, table: &str) -> Value {
    let mut obj = match row {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    obj.insert("_device_id".into(), Value::String(device_id.into()));
    obj.insert("_updated_at".into(), Value::String(updated_at.into()));
    // `_synced_at` is added by patch_doc as a Firestore REQUEST_TIME transform.
    // A client wall clock is not a safe global cursor across multiple devices.
    // _table is what makes the collectionGroup query routable on pull —
    // without it we can't tell which table a row should be applied to.
    obj.insert("_table".into(), Value::String(table.into()));
    let mut fields = serde_json::Map::new();
    for (k, v) in obj {
        fields.insert(k, json_to_field(&v));
    }
    json!({ "fields": fields })
}

fn decode_field(field: &Value, context: &str) -> Result<Value, String> {
    let object = field
        .as_object()
        .ok_or_else(|| format!("Firestore field {context} is not an object"))?;
    if object.len() != 1 {
        return Err(format!(
            "Firestore field {context} has an invalid type wrapper"
        ));
    }
    if let Some(value) = object.get("nullValue") {
        if value.is_null() {
            return Ok(Value::Null);
        }
        return Err(format!("Firestore field {context} has invalid nullValue"));
    }
    if let Some(value) = object.get("stringValue") {
        return value
            .as_str()
            .map(|value| Value::String(value.to_string()))
            .ok_or_else(|| format!("Firestore field {context} has invalid stringValue"));
    }
    if let Some(value) = object.get("timestampValue") {
        let timestamp = value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("Firestore field {context} has invalid timestampValue"))?;
        return canonical_sync_timestamp(timestamp, &format!("Firestore field {context}"))
            .map(Value::String);
    }
    if let Some(value) = object.get("integerValue") {
        let integer = if let Some(value) = value.as_str() {
            value.parse::<i64>().ok()
        } else {
            value.as_i64()
        };
        return integer
            .map(|value| Value::Number(value.into()))
            .ok_or_else(|| format!("Firestore field {context} has invalid integerValue"));
    }
    if let Some(value) = object.get("doubleValue") {
        return value
            .as_f64()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| format!("Firestore field {context} has invalid doubleValue"));
    }
    if let Some(value) = object.get("booleanValue") {
        return value
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| format!("Firestore field {context} has invalid booleanValue"));
    }
    if let Some(value) = object.get("arrayValue") {
        let array = value
            .as_object()
            .ok_or_else(|| format!("Firestore field {context} has invalid arrayValue"))?;
        let values = match array.get("values") {
            Some(values) => values
                .as_array()
                .ok_or_else(|| format!("Firestore field {context} has invalid array values"))?,
            None => return Ok(Value::Array(Vec::new())),
        };
        let mut decoded = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            decoded.push(decode_field(value, &format!("{context}[{index}]"))?);
        }
        return Ok(Value::Array(decoded));
    }
    if let Some(value) = object.get("mapValue") {
        let map = value
            .as_object()
            .ok_or_else(|| format!("Firestore field {context} has invalid mapValue"))?;
        let fields = match map.get("fields") {
            Some(fields) => fields
                .as_object()
                .ok_or_else(|| format!("Firestore field {context} has invalid map fields"))?,
            None => return Ok(Value::Object(serde_json::Map::new())),
        };
        let mut decoded = serde_json::Map::new();
        for (name, value) in fields {
            decoded.insert(name.clone(), decode_field(value, name)?);
        }
        return Ok(Value::Object(decoded));
    }
    Err(format!(
        "Firestore field {context} uses an unsupported value type"
    ))
}

#[derive(Clone, Debug, PartialEq)]
struct FirestoreDocument {
    name: String,
    update_time: String,
    fields: serde_json::Map<String, Value>,
}

const FIRESTORE_CURSOR_MIGRATION_REQUIRED: &str = "Firestore cursor migration required";

fn decode_doc(doc: &Value, index: usize) -> Result<FirestoreDocument, String> {
    let object = doc
        .as_object()
        .ok_or_else(|| format!("Firestore document {index} is not an object"))?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Firestore document {index} is missing name"))?
        .to_string();
    let update_time = object
        .get("updateTime")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Firestore document {index} is missing updateTime"))?;
    let update_time = canonical_sync_timestamp(
        update_time,
        &format!("Firestore document {index} updateTime"),
    )?;
    let fields = object
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Firestore document {index} is missing fields"))?;
    let source_synced_at = match fields
        .get("_synced_at")
        .and_then(Value::as_object)
    {
        Some(wrapper) if wrapper.len() == 1 && wrapper.contains_key("timestampValue") => wrapper
                .get("timestampValue")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "{FIRESTORE_CURSOR_MIGRATION_REQUIRED}: document {index} has an invalid timestampValue"
                    )
                })?
                .to_string(),
        _ => {
            return Err(format!(
                "{FIRESTORE_CURSOR_MIGRATION_REQUIRED}: document {index} does not have a server timestamp"
            ));
        }
    };
    let mut decoded = serde_json::Map::new();
    for (field_name, value) in fields {
        decoded.insert(field_name.clone(), decode_field(value, field_name)?);
    }
    for field_name in ["_synced_at", "_device_id", "_table"] {
        if !decoded
            .get(field_name)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            return Err(format!(
                "Firestore document {index} is missing {field_name}"
            ));
        }
    }
    let synced_at = canonical_sync_timestamp(
        &source_synced_at,
        &format!("Firestore document {index} _synced_at"),
    )
    .map_err(|error| format!("{FIRESTORE_CURSOR_MIGRATION_REQUIRED}: {error}"))?;
    if synced_at > update_time {
        return Err(format!(
            "{FIRESTORE_CURSOR_MIGRATION_REQUIRED}: document {index} _synced_at is newer than updateTime"
        ));
    }
    decoded.insert("_synced_at".into(), Value::String(synced_at));
    let table = decoded
        .get("_table")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if table != "tombstones" && !SYNC_TABLES.contains(&table) {
        return Err(format!(
            "Firestore document {index} has an unsupported table"
        ));
    }
    Ok(FirestoreDocument {
        name,
        update_time,
        fields: decoded,
    })
}

fn parse_run_query_payload(
    payload: Value,
    expected_document_prefix: &str,
) -> Result<Vec<FirestoreDocument>, String> {
    let entries = payload
        .as_array()
        .ok_or_else(|| "Firestore runQuery response is not an array".to_string())?;
    if entries.is_empty() {
        return Err("Firestore runQuery response is empty".into());
    }
    let mut documents = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| format!("Firestore runQuery entry {index} is not an object"))?;
        if let Some(skipped) = object.get("skippedResults") {
            let skipped = skipped.as_u64().ok_or_else(|| {
                format!("Firestore runQuery entry {index} has invalid skippedResults")
            })?;
            if skipped != 0 {
                return Err(format!(
                    "Firestore runQuery entry {index} unexpectedly skipped results"
                ));
            }
        }
        if let Some(read_time) = object.get("readTime") {
            if !read_time.as_str().is_some_and(|value| !value.is_empty()) {
                return Err(format!(
                    "Firestore runQuery entry {index} has invalid readTime"
                ));
            }
        }
        if let Some(document) = object.get("document") {
            let decoded = decode_doc(document, index)?;
            decoded
                .name
                .strip_prefix(expected_document_prefix)
                .filter(|value| !value.is_empty() && !value.contains('/'))
                .ok_or_else(|| {
                    format!("Firestore document {index} is outside the requested collection")
                })?;
            documents.push(decoded);
        } else if !object
            .get("readTime")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            return Err(format!(
                "Firestore runQuery entry {index} has neither document nor readTime"
            ));
        }
    }
    if documents.len() > PULL_LIMIT as usize {
        return Err("Firestore runQuery response exceeds the requested page limit".into());
    }
    Ok(documents)
}

// ── Firestore I/O ────────────────────────────────────────────────────────

fn firestore_commit_body(
    project_id: &str,
    path: &str,
    doc_id: &str,
    body: &Value,
) -> Result<Value, String> {
    let fields = body
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("PATCH {path}: document body is missing fields"))?;
    let document_name =
        format!("projects/{project_id}/databases/(default)/documents/{path}/{doc_id}");
    Ok(firestore_commit_body_for_name(&document_name, fields))
}

fn firestore_commit_body_for_name(
    document_name: &str,
    fields: &serde_json::Map<String, Value>,
) -> Value {
    json!({
        "writes": [{
            "update": {
                "name": document_name,
                "fields": fields
            },
            "updateTransforms": [{
                "fieldPath": "_synced_at",
                "setToServerValue": "REQUEST_TIME"
            }]
        }]
    })
}

fn firestore_synced_at_transform_body(document_name: &str, update_time: &str) -> Value {
    json!({
        "writes": [{
            "transform": {
                "document": document_name,
                "fieldTransforms": [{
                    "fieldPath": "_synced_at",
                    "setToServerValue": "REQUEST_TIME"
                }]
            },
            "currentDocument": {"updateTime": update_time}
        }]
    })
}

async fn commit_firestore_document(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    document_name: &str,
    fields: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents:commit",
        firestore_host(),
        project_id,
    );
    let commit = firestore_commit_body_for_name(document_name, fields);
    let response = client
        .post(&url)
        .bearer_auth(token)
        .json(&commit)
        .send()
        .await
        .map_err(|error| format!("Firestore commit request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Firestore commit failed with HTTP {}",
            response.status()
        ));
    }
    Ok(())
}

async fn transform_firestore_synced_at(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    document_name: &str,
    update_time: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents:commit",
        firestore_host(),
        project_id,
    );
    let response = client
        .post(&url)
        .bearer_auth(token)
        .json(&firestore_synced_at_transform_body(document_name, update_time))
        .send()
        .await
        .map_err(|error| format!("Firestore cursor transform request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Firestore cursor transform failed with HTTP {}",
            response.status()
        ));
    }
    Ok(())
}

async fn patch_doc(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    path: &str,
    doc_id: &str,
    body: &Value,
) -> Result<(), String> {
    let commit = firestore_commit_body(project_id, path, doc_id, body)?;
    let document = commit
        .pointer("/writes/0/update")
        .and_then(Value::as_object)
        .ok_or_else(|| "Firestore commit body lost its update".to_string())?;
    let document_name = document
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Firestore commit body lost its document name".to_string())?;
    let fields = document
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| "Firestore commit body lost its fields".to_string())?;
    commit_firestore_document(client, token, project_id, document_name, fields).await
}

const FIRESTORE_PULL_TS_KEY: &str = "cloud_owner_v2_pull_synced";
const FIRESTORE_PULL_NAME_KEY: &str = "cloud_owner_v2_pull_name";

#[derive(Clone, Debug, PartialEq, Eq)]
struct FirestoreCursor {
    synced_at: String,
    document_name: Option<String>,
}

fn load_firestore_cursor(conn: &Connection) -> Result<FirestoreCursor, String> {
    let synced_at = get_setting_checked(conn, FIRESTORE_PULL_TS_KEY)?;
    let document_name = get_setting_checked(conn, FIRESTORE_PULL_NAME_KEY)?;
    match (synced_at, document_name) {
        (Some(synced_at), Some(document_name))
            if !synced_at.is_empty() && !document_name.is_empty() =>
        {
            Ok(FirestoreCursor {
                synced_at: canonical_sync_timestamp(&synced_at, "Firestore pull cursor")?,
                document_name: Some(document_name),
            })
        }
        // A timestamp-only cursor was written by the permissive one-page
        // implementation. It may have advanced past a malformed skipped doc,
        // so replay once from the epoch; LWW makes this idempotent.
        (Some(_), None) | (None, None) => Ok(FirestoreCursor {
            synced_at: canonical_sync_timestamp(EPOCH_TS, "sync epoch")?,
            document_name: None,
        }),
        _ => Err("Firestore pull cursor is incomplete or empty".into()),
    }
}

fn save_firestore_cursor(conn: &Connection, cursor: &FirestoreCursor) -> Result<(), String> {
    let document_name = cursor
        .document_name
        .as_deref()
        .ok_or_else(|| "Firestore pull cursor has no document name".to_string())?;
    let synced_at = canonical_sync_timestamp(&cursor.synced_at, "Firestore pull cursor")?;
    set_setting_checked(conn, FIRESTORE_PULL_TS_KEY, &synced_at)?;
    set_setting_checked(conn, FIRESTORE_PULL_NAME_KEY, document_name)
}

fn firestore_overlap_cursor(cursor: &FirestoreCursor) -> Result<FirestoreCursor, String> {
    let timestamp = chrono::DateTime::parse_from_rfc3339(&cursor.synced_at)
        .map_err(|_| "Firestore pull cursor is not RFC3339".to_string())?
        .with_timezone(&chrono::Utc);
    let epoch = chrono::DateTime::parse_from_rfc3339(EPOCH_TS)
        .map_err(|_| "sync epoch is not RFC3339".to_string())?
        .with_timezone(&chrono::Utc);
    let overlapped = timestamp
        .checked_sub_signed(chrono::Duration::milliseconds(1))
        .unwrap_or_else(|| epoch.clone());
    let overlapped = if overlapped < epoch { epoch } else { overlapped };
    Ok(FirestoreCursor {
        synced_at: overlapped.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        document_name: None,
    })
}

fn firestore_query_body(
    collection_id: &str,
    cursor: &FirestoreCursor,
    all_descendants: bool,
) -> Value {
    let mut structured_query = json!({
        "from": [{
            "collectionId": collection_id,
            "allDescendants": all_descendants,
        }],
        "where": {"fieldFilter": {
            "field": {"fieldPath": "_synced_at"},
            "op": "GREATER_THAN_OR_EQUAL",
            "value": {"timestampValue": cursor.synced_at}
        }},
        "orderBy": [
            {"field": {"fieldPath": "_synced_at"}, "direction": "ASCENDING"},
            {"field": {"fieldPath": "__name__"}, "direction": "ASCENDING"}
        ],
        "limit": PULL_LIMIT
    });
    if let Some(document_name) = &cursor.document_name {
        structured_query["startAt"] = json!({
            "values": [
                {"timestampValue": cursor.synced_at},
                {"referenceValue": document_name}
            ],
            "before": false
        });
    }
    json!({ "structuredQuery": structured_query })
}

async fn run_query(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    parent_path: &str,
    collection_id: &str,
    cursor: &FirestoreCursor,
    all_descendants: bool,
) -> Result<Vec<FirestoreDocument>, String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents/{}:runQuery",
        firestore_host(),
        project_id,
        parent_path
    );
    let body = firestore_query_body(collection_id, cursor, all_descendants);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("runQuery {}: {}", collection_id, e))?;
    let status = resp.status();
    let payload: Value = resp
        .json()
        .await
        .map_err(|_| format!("runQuery {collection_id}: invalid JSON response"))?;
    if !status.is_success() {
        return Err(format!(
            "runQuery {collection_id} failed with HTTP {status}"
        ));
    }
    let expected_prefix = format!(
        "projects/{project_id}/databases/(default)/documents/{parent_path}/{collection_id}/"
    );
    parse_run_query_payload(payload, &expected_prefix)
}

// ── Per-table push ───────────────────────────────────────────────────────

// Flat layout: owners/{uid}/data/{table}_{row_id}. Lets us pull every
// table with a single non-collectionGroup query (REST forbids cgroups
// outside the root parent).
#[derive(Clone, Debug, PartialEq)]
struct FirestoreListPage {
    documents: Vec<Value>,
    next_page_token: Option<String>,
}

fn parse_list_documents_payload(payload: Value) -> Result<FirestoreListPage, String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "Firestore listDocuments response is not an object".to_string())?;
    let documents = match object.get("documents") {
        Some(documents) => documents
            .as_array()
            .ok_or_else(|| "Firestore listDocuments documents is not an array".to_string())?
            .clone(),
        None => Vec::new(),
    };
    if documents.len() > PULL_LIMIT as usize {
        return Err("Firestore listDocuments response exceeds the page limit".into());
    }
    let next_page_token = object
        .get("nextPageToken")
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| "Firestore listDocuments has an invalid nextPageToken".to_string())
        })
        .transpose()?;
    Ok(FirestoreListPage {
        documents,
        next_page_token,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScannedFirestoreDocument {
    name: String,
    update_time: String,
    needs_server_timestamp: bool,
}

fn scan_firestore_document(
    document: &Value,
    index: usize,
    expected_document_prefix: &str,
) -> Result<ScannedFirestoreDocument, String> {
    let object = document
        .as_object()
        .ok_or_else(|| format!("Firestore scanned document {index} is not an object"))?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Firestore scanned document {index} is missing name"))?;
    name.strip_prefix(expected_document_prefix)
        .filter(|value| !value.is_empty() && !value.contains('/'))
        .ok_or_else(|| format!("Firestore scanned document {index} is outside the collection"))?;
    let update_time = object
        .get("updateTime")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Firestore scanned document {index} is missing updateTime"))?;
    let update_time = canonical_sync_timestamp(
        update_time,
        &format!("Firestore scanned document {index} updateTime"),
    )?;
    let fields = object
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Firestore scanned document {index} is missing fields"))?;
    let server_timestamp = fields
        .get("_synced_at")
        .and_then(Value::as_object)
        .filter(|wrapper| wrapper.len() == 1)
        .and_then(|wrapper| wrapper.get("timestampValue"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .and_then(|value| canonical_sync_timestamp(value, "Firestore scanned _synced_at").ok());
    let has_valid_server_timestamp = server_timestamp
        .as_deref()
        .is_some_and(|synced_at| synced_at <= update_time.as_str());

    // Validate every non-cursor field using the same strict decoder as the
    // incremental pull. A broken document must not be hidden by the marker.
    let mut normalized = document.clone();
    if !has_valid_server_timestamp {
        normalized
            .get_mut("fields")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("Firestore scanned document {index} lost fields"))?
            .insert("_synced_at".into(), json!({"timestampValue": EPOCH_TS}));
    }
    decode_doc(&normalized, index)?;
    Ok(ScannedFirestoreDocument {
        name: name.to_string(),
        update_time,
        needs_server_timestamp: !has_valid_server_timestamp,
    })
}

fn parse_begin_transaction_payload(payload: Value) -> Result<String, String> {
    payload
        .as_object()
        .and_then(|object| object.get("transaction"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "Firestore beginTransaction response is missing transaction".to_string())
}

async fn begin_firestore_read_transaction(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents:beginTransaction",
        firestore_host(),
        project_id,
    );
    let response = client
        .post(&url)
        .bearer_auth(token)
        .json(&json!({"options": {"readOnly": {}}}))
        .send()
        .await
        .map_err(|error| format!("Firestore beginTransaction request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Firestore beginTransaction failed with HTTP {status}"
        ));
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|_| "Firestore beginTransaction returned invalid JSON".to_string())?;
    parse_begin_transaction_payload(payload)
}

async fn rollback_firestore_transaction(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    transaction: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents:rollback",
        firestore_host(),
        project_id,
    );
    let response = client
        .post(&url)
        .bearer_auth(token)
        .json(&json!({"transaction": transaction}))
        .send()
        .await
        .map_err(|error| format!("Firestore rollback request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Firestore rollback failed with HTTP {}",
            response.status()
        ));
    }
    Ok(())
}

fn firestore_list_query(
    transaction: &str,
    page_token: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("pageSize", PULL_LIMIT.to_string()),
        ("orderBy", "__name__".to_string()),
        ("transaction", transaction.to_string()),
    ];
    if let Some(page_token) = page_token {
        query.push(("pageToken", page_token.to_string()));
    }
    query
}

async fn list_firestore_documents_page(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
    transaction: &str,
    page_token: Option<&str>,
) -> Result<FirestoreListPage, String> {
    let url = format!(
        "{}/projects/{}/databases/(default)/documents/owners/{}/data",
        firestore_host(),
        project_id,
        owner_uid,
    );
    let response = client
        .get(&url)
        .bearer_auth(token)
        .query(&firestore_list_query(transaction, page_token))
        .send()
        .await
        .map_err(|error| format!("Firestore listDocuments request failed: {error}"))?;
    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|_| "Firestore listDocuments returned invalid JSON".to_string())?;
    if !status.is_success() {
        return Err(format!("Firestore listDocuments failed with HTTP {status}"));
    }
    parse_list_documents_payload(payload)
}

async fn scan_firestore_cursor_candidates(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<Vec<ScannedFirestoreDocument>, String> {
    let transaction = begin_firestore_read_transaction(client, token, project_id).await?;
    let expected_prefix =
        format!("projects/{project_id}/databases/(default)/documents/owners/{owner_uid}/data/");
    let scan_result: Result<Vec<ScannedFirestoreDocument>, String> = async {
        let mut page_token: Option<String> = None;
        let mut seen_tokens = std::collections::HashSet::new();
        let mut previous_document_name: Option<String> = None;
        let mut scanned_documents = Vec::new();
        for page_index in 0..1_000usize {
            if let Some(token) = &page_token {
                if !seen_tokens.insert(token.clone()) {
                    return Err("Firestore listDocuments repeated a page token".into());
                }
            }
            let page = list_firestore_documents_page(
                client,
                token,
                project_id,
                owner_uid,
                &transaction,
                page_token.as_deref(),
            )
            .await?;
            for (document_index, document) in page.documents.iter().enumerate() {
                let scanned = scan_firestore_document(
                    document,
                    page_index * PULL_LIMIT as usize + document_index,
                    &expected_prefix,
                )?;
                if previous_document_name
                    .as_deref()
                    .is_some_and(|previous| previous >= scanned.name.as_str())
                {
                    return Err("Firestore listDocuments is not strictly ordered by name".into());
                }
                previous_document_name = Some(scanned.name.clone());
                scanned_documents.push(scanned);
            }
            match page.next_page_token {
                Some(next_page_token) => page_token = Some(next_page_token),
                None => return Ok(scanned_documents),
            }
        }
        Err("Firestore listDocuments exceeded the 1000-page safety limit".into())
    }
    .await;
    let rollback_result =
        rollback_firestore_transaction(client, token, project_id, &transaction).await;
    match (scan_result, rollback_result) {
        (Ok(scanned), Ok(())) => Ok(scanned),
        (Err(scan_error), Ok(())) => Err(scan_error),
        (Ok(_), Err(rollback_error)) => Err(rollback_error),
        (Err(scan_error), Err(rollback_error)) => Err(format!(
            "{scan_error}; Firestore snapshot cleanup also failed: {rollback_error}"
        )),
    }
}

async fn scan_and_migrate_firestore_cursors(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<usize, String> {
    let scanned_documents =
        scan_firestore_cursor_candidates(client, token, project_id, owner_uid).await?;
    let mut transformed = 0usize;
    for scanned in scanned_documents {
        if scanned.needs_server_timestamp {
            transform_firestore_synced_at(
                client,
                token,
                project_id,
                &scanned.name,
                &scanned.update_time,
            )
            .await?;
            transformed += 1;
        }
    }
    Ok(transformed)
}

async fn migrate_firestore_cursor_generation(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<usize, String> {
    let mut total = 0usize;
    // A clean second scan is the migration proof. Extra passes tolerate a
    // concurrent old client once; continued legacy writes fail retryably.
    for _pass in 0..3usize {
        let transformed =
            scan_and_migrate_firestore_cursors(client, token, project_id, owner_uid).await?;
        total += transformed;
        if transformed == 0 {
            return Ok(total);
        }
    }
    Err("Firestore cursor migration did not converge after three full scans".into())
}

fn data_collection(owner_uid: &str) -> String {
    format!("owners/{}/data", owner_uid)
}
fn data_doc_id(table: &str, id: &SqlValue, row: &Value, device_id: &str) -> Result<String, String> {
    let logical_id = if table == "event_categories" {
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "event_categories row is missing its logical name".to_string())?;
        format!("name:{name}")
    } else {
        match id {
            SqlValue::Integer(value) => format!("i:{value}"),
            SqlValue::Text(value) => format!("t:{value}"),
            other => {
                return Err(format!(
                    "unsupported Firestore primary key for {table}: {other:?}"
                ));
            }
        }
    };
    let mut hasher = Sha256::new();
    for component in [device_id, table, logical_id.as_str()] {
        hasher.update(component.as_bytes());
        hasher.update([0]);
    }
    Ok(format!("row_{}", hex::encode(hasher.finalize())))
}
fn tombstone_doc_id(table: &str, id: &str, device_id: &str) -> String {
    let mut hasher = Sha256::new();
    for component in [device_id, table, id] {
        hasher.update(component.as_bytes());
        hasher.update([0]);
    }
    format!("delete_{}", hex::encode(hasher.finalize()))
}

/// Bind a pulled tombstone's `_row_id` without table schema context.
pub(crate) fn tombstone_row_id(v: Option<&Value>) -> Option<rusqlite::types::Value> {
    match v {
        Some(Value::Number(n)) => n.as_i64().map(rusqlite::types::Value::Integer),
        Some(Value::String(s)) => Some(match s.parse::<i64>() {
            Ok(n) => rusqlite::types::Value::Integer(n),
            Err(_) => rusqlite::types::Value::Text(s.clone()),
        }),
        _ => None,
    }
}

fn tombstone_row_id_for_table(conn: &Connection, table: &str, value: &Value) -> Option<SqlValue> {
    if column_is_text(conn, table, "id") {
        return match value {
            Value::String(value) => Some(SqlValue::Text(value.clone())),
            Value::Number(value) => Some(SqlValue::Text(value.to_string())),
            _ => None,
        };
    }
    tombstone_row_id(Some(value))
}

fn with_remote_sync_apply<T>(
    conn: &Connection,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    const SAVEPOINT: &str = "hanni_remote_sync_apply";
    conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(|error| format!("start remote sync apply: {error}"))?;
    let result = (|| -> Result<T, String> {
        let changed = conn
            .execute(
                "UPDATE sync_apply_context SET remote_apply=1
                 WHERE singleton=1 AND remote_apply=0 AND stamp_depth=0",
                [],
            )
            .map_err(|error| format!("enter remote sync apply: {error}"))?;
        if changed != 1 {
            return Err("remote sync apply context is missing or already active".into());
        }
        let value = operation()?;
        let changed = conn
            .execute(
                "UPDATE sync_apply_context SET remote_apply=0
                 WHERE singleton=1 AND remote_apply=1 AND stamp_depth=0",
                [],
            )
            .map_err(|error| format!("leave remote sync apply: {error}"))?;
        if changed != 1 {
            return Err("remote sync apply context was not restored".into());
        }
        Ok(value)
    })();

    match result {
        Ok(value) => match conn.execute_batch(&format!("RELEASE {SAVEPOINT}")) {
            Ok(()) => Ok(value),
            Err(error) => {
                let _ = conn.execute_batch(&format!(
                    "ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"
                ));
                Err(format!("commit remote sync apply: {error}"))
            }
        },
        Err(error) => {
            let cleanup = conn.execute_batch(&format!(
                "ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"
            ));
            match cleanup {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(format!(
                    "{error}; rollback remote sync apply: {cleanup_error}"
                )),
            }
        }
    }
}

pub(crate) fn apply_tombstone_lww(
    conn: &Connection,
    target: &str,
    id: &Value,
    deleted_at: &str,
) -> Result<bool, String> {
    with_remote_sync_apply(conn, || {
        apply_tombstone_lww_inner(conn, target, id, deleted_at)
    })
}

fn apply_tombstone_lww_inner(
    conn: &Connection,
    target: &str,
    id: &Value,
    deleted_at: &str,
) -> Result<bool, String> {
    if !SYNC_TABLES.contains(&target) {
        return Err(format!("tombstone: unsupported table {target}"));
    }
    if deleted_at.is_empty() {
        return Err(format!("tombstone {target}: missing _updated_at"));
    }
    let deleted_at =
        canonical_sync_timestamp(deleted_at, &format!("tombstone {target} _updated_at"))?;

    let (row_id, row_id_text, lookup_column) = if target == "event_categories" {
        let Some(name) = id
            .as_str()
            .and_then(|value| value.strip_prefix("name:"))
            .filter(|value| !value.is_empty())
        else {
            // Legacy category tombstones contain device-local integer ids.
            // Applying them could delete an unrelated category on a peer, so
            // preserve data and advance past this known-unsafe legacy record.
            eprintln!(
                "[sync_owner] ignored legacy event_categories tombstone without logical name"
            );
            return Ok(false);
        };
        (
            SqlValue::Text(name.to_string()),
            format!("name:{name}"),
            "name",
        )
    } else {
        let row_id = tombstone_row_id_for_table(conn, target, id)
            .ok_or_else(|| format!("tombstone {target}: invalid _row_id"))?;
        let row_id_text = match &row_id {
            SqlValue::Integer(value) => value.to_string(),
            SqlValue::Text(value) => value.clone(),
            _ => return Err(format!("tombstone {target}: unsupported _row_id")),
        };
        (row_id, row_id_text, "id")
    };

    let known_tombstone_raw: Option<String> = conn
        .query_row(
            "SELECT deleted_at FROM sync_tombstones WHERE table_name=?1 AND row_id=?2",
            rusqlite::params![target, &row_id_text],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("read tombstone {target}: {error}"))?;
    let known_tombstone = known_tombstone_raw
        .as_deref()
        .map(|timestamp| {
            canonical_sync_timestamp(timestamp, &format!("stored tombstone {target} deleted_at"))
        })
        .transpose()?;
    let effective_deleted_at = match known_tombstone.as_deref() {
        Some(timestamp) if timestamp > deleted_at.as_str() => timestamp.to_string(),
        _ => deleted_at,
    };

    let local_updated_raw: Option<String> = conn
        .query_row(
            &format!("SELECT updated_at FROM {target} WHERE {lookup_column}=?1"),
            rusqlite::params![&row_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("read local row {target}: {error}"))?;
    let local_updated = local_updated_raw
        .as_deref()
        .map(|timestamp| canonical_sync_timestamp(timestamp, &format!("local {target}.updated_at")))
        .transpose()?;
    // Delete wins on equal timestamps; only a strictly newer local row survives.
    if local_updated
        .as_deref()
        .is_some_and(|timestamp| timestamp > effective_deleted_at.as_str())
    {
        return Ok(false);
    }

    let deleted = conn
        .execute(
            &format!("DELETE FROM {target} WHERE {lookup_column}=?1"),
            rusqlite::params![&row_id],
        )
        .map_err(|error| format!("delete {target}: {error}"))?;

    // Local DELETE triggers can stamp wall-clock time. Persist the remote
    // logical timestamp (or a newer known tombstone) to prevent resurrection.
    let wrote_tombstone = known_tombstone.as_deref() != Some(effective_deleted_at.as_str());
    conn.execute(
        "INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES(?1,?2,?3) \
         ON CONFLICT(table_name,row_id) DO UPDATE SET deleted_at=excluded.deleted_at",
        rusqlite::params![target, &row_id_text, &effective_deleted_at],
    )
    .map_err(|error| format!("persist tombstone {target}: {error}"))?;
    crate::db::observe_sync_hlc_timestamp(conn, &effective_deleted_at)?;

    Ok(deleted > 0 || wrote_tombstone)
}

async fn push_table(
    db: &HanniDb,
    table: &str,
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<usize, String> {
    let (rows, original_cursor, next_cursor, dev_id) = {
        let conn = db.conn();
        let cursor_key = push_cursor_key(table);
        let cursor = load_row_cursor(&conn, &cursor_key)?;
        let dev = device_id(&conn)?;
        let dirty = dirty_rows_after(&conn, table, &cursor, PUSH_LIMIT)?;
        let mut payloads: Vec<(SqlValue, String, Value)> = Vec::new();
        for (id, ts) in &dirty {
            let row = row_to_json(&conn, table, id)?
                .ok_or_else(|| format!("dirty row disappeared from {table}"))?;
            payloads.push((id.clone(), ts.clone(), row));
        }
        let next = payloads.last().map(|(id, timestamp, _)| RowCursor {
            timestamp: timestamp.clone(),
            id: Some(id.clone()),
        });
        (payloads, cursor, next, dev)
    };

    let path = data_collection(owner_uid);
    let mut pushed = 0usize;
    for (id, ts, row) in &rows {
        let body = encode_doc(row, &dev_id, ts, table);
        let doc_id = data_doc_id(table, id, row, &dev_id)?;
        patch_doc(client, token, project_id, &path, &doc_id, &body).await?;
        pushed += 1;
    }
    if let Some(next_cursor) = next_cursor {
        let mut conn = db.conn();
        let transaction = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("start {table} push cursor transaction: {error}"))?;
        let cursor_key = push_cursor_key(table);
        if load_row_cursor(&transaction, &cursor_key)? != original_cursor {
            return Err(format!("{table} push cursor changed during upload"));
        }
        save_row_cursor(&transaction, &cursor_key, &next_cursor)?;
        transaction
            .commit()
            .map_err(|error| format!("commit {table} push cursor: {error}"))?;
    }
    Ok(pushed)
}

async fn push_tombstones(
    db: &HanniDb,
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<usize, String> {
    const CURSOR_KEY: &str = "cloud_owner_v2_push_tombstones";
    let (rows, original_cursor, next_cursor, dev_id) = {
        let conn = db.conn();
        let cursor = load_tombstone_cursor(&conn, CURSOR_KEY)?;
        let dirty = dirty_tombstones_after(&conn, &cursor, PUSH_LIMIT)?;
        let next = dirty
            .last()
            .map(|(table, row_id, timestamp)| TombstoneCursor {
                timestamp: timestamp.clone(),
                table: Some(table.clone()),
                row_id: Some(row_id.clone()),
            });
        (dirty, cursor, next, device_id(&conn)?)
    };

    let path = data_collection(owner_uid);
    let mut pushed = 0usize;
    for (table, id, ts) in &rows {
        let row = json!({ "_target_table": table, "_row_id": id, "_deleted": true });
        let body = encode_doc(&row, &dev_id, ts, "tombstones");
        patch_doc(
            client,
            token,
            project_id,
            &path,
            &tombstone_doc_id(table, id, &dev_id),
            &body,
        )
        .await?;
        pushed += 1;
    }
    if let Some(next_cursor) = next_cursor {
        let mut conn = db.conn();
        let transaction = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("start tombstone push cursor transaction: {error}"))?;
        if load_tombstone_cursor(&transaction, CURSOR_KEY)? != original_cursor {
            return Err("tombstone push cursor changed during upload".into());
        }
        save_tombstone_cursor(&transaction, CURSOR_KEY, &next_cursor)?;
        transaction
            .commit()
            .map_err(|error| format!("commit tombstone push cursor: {error}"))?;
    }
    Ok(pushed)
}

// ── Per-table pull ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum UpsertMode {
    BestEffort,
    FailClosed,
}

pub(crate) fn upsert_row(
    conn: &Connection,
    table: &str,
    fields: &serde_json::Map<String, Value>,
) -> Result<bool, String> {
    with_remote_sync_apply(conn, || {
        upsert_row_inner(conn, table, fields, UpsertMode::BestEffort)
    })
}

pub(crate) fn upsert_row_fail_closed(
    conn: &Connection,
    table: &str,
    fields: &serde_json::Map<String, Value>,
) -> Result<bool, String> {
    with_remote_sync_apply(conn, || {
        upsert_row_inner(conn, table, fields, UpsertMode::FailClosed)
    })
}

fn local_writer_for_equal_timestamp(
    conn: &Connection,
    table: &str,
    row_id: &str,
    local_timestamp: &str,
) -> Result<String, String> {
    let local_device = get_setting_checked(conn, "device_id")?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "sync row comparison requires a local device_id".to_string())?;
    let stored: Option<(String, String)> = conn
        .query_row(
            "SELECT updated_at, device_id FROM sync_row_versions
             WHERE table_name=?1 AND row_id=?2",
            rusqlite::params![table, row_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("read sync version for {table}/{row_id}: {error}"))?;
    if let Some((timestamp, device_id)) = stored {
        let timestamp = canonical_sync_timestamp(
            &timestamp,
            &format!("stored sync version for {table}/{row_id}"),
        )?;
        if timestamp == local_timestamp && !device_id.is_empty() {
            return Ok(device_id);
        }
    }
    Ok(local_device)
}

fn record_row_version(
    conn: &Connection,
    table: &str,
    row_id: &str,
    timestamp: &str,
    device_id: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sync_row_versions(table_name,row_id,updated_at,device_id)
         VALUES(?1,?2,?3,?4)
         ON CONFLICT(table_name,row_id) DO UPDATE SET
             updated_at=excluded.updated_at,
             device_id=excluded.device_id",
        rusqlite::params![table, row_id, timestamp, device_id],
    )
    .map(|_| ())
    .map_err(|error| format!("record sync version for {table}/{row_id}: {error}"))
}

fn upsert_row_inner(
    conn: &Connection,
    table: &str,
    fields: &serde_json::Map<String, Value>,
    mode: UpsertMode,
) -> Result<bool, String> {
    let id_value = fields
        .get("id")
        .cloned()
        .ok_or_else(|| format!("{}: row missing id", table))?;
    // Bind only the declared primary-key type. Generic JSON coercion could map
    // `true` to INTEGER 1 while version/tombstone metadata used the string
    // "true", corrupting two identities at once.
    let (id_sql, id_str) = if column_is_text(conn, table, "id") {
        match &id_value {
            Value::String(value) if !value.is_empty() => {
                (SqlValue::Text(value.clone()), value.clone())
            }
            _ => return Err(format!("{table}: TEXT row id must be a non-empty string")),
        }
    } else {
        match &id_value {
            Value::Number(value) => {
                let value = value
                    .as_i64()
                    .ok_or_else(|| format!("{table}: INTEGER row id is invalid"))?;
                (SqlValue::Integer(value), value.to_string())
            }
            _ => return Err(format!("{table}: INTEGER row id must be an integer")),
        }
    };
    let remote_ts_raw = fields
        .get("_updated_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    if remote_ts_raw.is_empty() {
        if mode == UpsertMode::FailClosed {
            return Err(format!("{}: row missing _updated_at", table));
        }
        return Ok(false);
    }
    let remote_ts =
        match canonical_sync_timestamp(remote_ts_raw, &format!("remote {table}._updated_at")) {
            Ok(timestamp) => timestamp,
            Err(error) if mode == UpsertMode::FailClosed => return Err(error),
            Err(_) => return Ok(false),
        };
    let remote_device = fields
        .get("_device_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    if mode == UpsertMode::FailClosed && remote_device.is_none() {
        return Err(format!("{table}: row missing _device_id"));
    }

    // event_categories has no stable cross-device id — resolve it by name.
    if table == "event_categories" {
        return upsert_event_category(conn, fields, &remote_ts, remote_device.unwrap_or(""), mode);
    }

    // Tolerate missing tables — devices may diverge if one shipped earlier
    // schema. We still advance the pull cursor so we don't loop forever.
    let cols = match table_columns(conn, table) {
        Ok(c) => c,
        Err(error) if mode == UpsertMode::FailClosed => return Err(error),
        Err(_) => {
            eprintln!("[sync_owner] skip remote row for missing table {}", table);
            return Ok(false);
        }
    };

    // Don't resurrect locally-deleted rows: skip if a tombstone post-dates the
    // remote write. Without this, a deletion gets clobbered by the next pull of
    // the still-present remote row (LWW alone can't tell delete from absence).
    let tomb_ts_raw: Option<String> = match conn
        .query_row(
            "SELECT deleted_at FROM sync_tombstones WHERE table_name=?1 AND row_id=?2",
            rusqlite::params![table, &id_str],
            |r| r.get(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(error) if mode == UpsertMode::FailClosed => {
            return Err(format!("{}: read tombstone: {}", table, error));
        }
        Err(_) => None,
    };
    if let Some(tomb) = &tomb_ts_raw {
        let tomb = match canonical_sync_timestamp(tomb, &format!("{table} tombstone")) {
            Ok(timestamp) => timestamp,
            Err(error) if mode == UpsertMode::FailClosed => return Err(error),
            Err(_) => return Ok(false),
        };
        if tomb >= remote_ts {
            return Ok(false);
        }
    }

    // LWW uses a deterministic writer-id tie-break when timestamps are equal.
    let local_ts_raw: Option<String> = match conn
        .query_row(
            &format!("SELECT updated_at FROM {} WHERE id = ?1", table),
            rusqlite::params![&id_sql],
            |r| r.get(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(error) if mode == UpsertMode::FailClosed => {
            return Err(format!("{}: read local row: {}", table, error));
        }
        Err(_) => None,
    };
    if let Some(local) = &local_ts_raw {
        let local = match canonical_sync_timestamp(local, &format!("local {table}.updated_at")) {
            Ok(timestamp) => timestamp,
            Err(error) if mode == UpsertMode::FailClosed => return Err(error),
            Err(_) => return Ok(false),
        };
        if local > remote_ts {
            return Ok(false);
        }
        if local == remote_ts {
            if mode != UpsertMode::FailClosed {
                return Ok(false);
            }
            let local_writer = local_writer_for_equal_timestamp(conn, table, &id_str, &local)?;
            if local_writer.as_str() >= remote_device.unwrap_or("") {
                return Ok(false);
            }
        }
    }

    let cols: Vec<&str> = cols
        .iter()
        .map(|s| s.as_str())
        .filter(|c| fields.contains_key(*c) || *c == "updated_at")
        .collect();
    if cols.is_empty() {
        if mode == UpsertMode::FailClosed {
            return Err(format!("{}: row has no applicable columns", table));
        }
        return Ok(false);
    }

    let placeholders = (1..=cols.len())
        .map(|i| format!("?{}", i))
        .collect::<Vec<_>>()
        .join(",");
    let updates = cols
        .iter()
        .filter(|c| **c != "id")
        .map(|c| format!("{0} = excluded.{0}", c))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) \
         ON CONFLICT(id) DO UPDATE SET {}",
        table,
        cols.join(","),
        placeholders,
        updates
    );

    let params: Vec<rusqlite::types::Value> = cols
        .iter()
        .map(|c| {
            if *c == "updated_at" {
                SqlValue::Text(remote_ts.clone())
            } else {
                json_to_sqlite(fields.get(*c).unwrap_or(&Value::Null))
            }
        })
        .collect();
    let refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    if let Err(e) = conn.execute(&sql, refs.as_slice()) {
        if mode == UpsertMode::FailClosed {
            return Err(format!("{}: upsert row {}: {}", table, id_str, e));
        }
        // Don't fail the whole pull on one bad row — log and move on.
        eprintln!("[sync_owner] upsert {} #{}: {}", table, id_str, e);
        return Ok(false);
    }
    // The production bump trigger treats an equal-timestamp remote winner as
    // a local edit because NEW.updated_at == OLD.updated_at. Restore the exact
    // remote value before recording its writer; the second UPDATE has a
    // changed timestamp and therefore does not trigger another bump.
    if let Err(error) = conn.execute(
        &format!(
            "UPDATE {table} SET updated_at=?1
             WHERE id=?2 AND updated_at<>?1"
        ),
        rusqlite::params![&remote_ts, &id_sql],
    ) {
        if mode == UpsertMode::FailClosed {
            return Err(format!(
                "{table}: restore remote timestamp for {id_str}: {error}"
            ));
        }
        eprintln!(
            "[sync_owner] restore remote timestamp {} #{}: {}",
            table, id_str, error
        );
    }
    if mode == UpsertMode::FailClosed {
        record_row_version(
            conn,
            table,
            &id_str,
            &remote_ts,
            remote_device.unwrap_or(""),
        )?;
    }
    crate::db::observe_sync_hlc_timestamp(conn, &remote_ts)?;
    Ok(true)
}

/// event_categories uses an AUTOINCREMENT `id` that diverges across devices,
/// but `name` is UNIQUE and is what `events.category` references. Sync by name:
/// ignore the remote id (local AUTOINCREMENT owns it), conflict-resolve on name.
fn upsert_event_category(
    conn: &Connection,
    fields: &serde_json::Map<String, Value>,
    remote_ts: &str,
    remote_device: &str,
    mode: UpsertMode,
) -> Result<bool, String> {
    let name = match fields.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        _ if mode == UpsertMode::FailClosed => {
            return Err("event_categories: row missing name".into());
        }
        _ => return Ok(false),
    };

    let logical_row_id = format!("name:{name}");
    let tombstone: Option<String> = match conn
        .query_row(
            "SELECT deleted_at FROM sync_tombstones \
             WHERE table_name='event_categories' AND row_id=?1",
            [&logical_row_id],
            |row| row.get(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(error) if mode == UpsertMode::FailClosed => {
            return Err(format!("event_categories: read tombstone: {error}"));
        }
        Err(_) => None,
    };
    if let Some(timestamp) = tombstone.as_deref() {
        let timestamp = match canonical_sync_timestamp(timestamp, "event_categories tombstone") {
            Ok(timestamp) => timestamp,
            Err(error) if mode == UpsertMode::FailClosed => return Err(error),
            Err(_) => return Ok(false),
        };
        if timestamp.as_str() >= remote_ts {
            return Ok(false);
        }
    }

    // LWW by name, then by writer id when timestamps are equal.
    let local_ts_raw: Option<String> = match conn
        .query_row(
            "SELECT updated_at FROM event_categories WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(error) if mode == UpsertMode::FailClosed => {
            return Err(format!("event_categories: read local row: {}", error));
        }
        Err(_) => None,
    };
    if let Some(local) = &local_ts_raw {
        let local = match canonical_sync_timestamp(local, "local event_categories.updated_at") {
            Ok(timestamp) => timestamp,
            Err(error) if mode == UpsertMode::FailClosed => return Err(error),
            Err(_) => return Ok(false),
        };
        if local.as_str() > remote_ts {
            return Ok(false);
        }
        if local.as_str() == remote_ts {
            if mode != UpsertMode::FailClosed {
                return Ok(false);
            }
            let local_writer = local_writer_for_equal_timestamp(
                conn,
                "event_categories",
                &logical_row_id,
                &local,
            )?;
            if local_writer.as_str() >= remote_device {
                return Ok(false);
            }
        }
    }

    let table_cols = match table_columns(conn, "event_categories") {
        Ok(c) => c,
        Err(error) if mode == UpsertMode::FailClosed => return Err(error),
        Err(_) => return Ok(false),
    };
    // Drop `id` — the local AUTOINCREMENT owns it.
    let cols: Vec<&str> = table_cols
        .iter()
        .map(|s| s.as_str())
        .filter(|c| *c != "id" && (fields.contains_key(*c) || *c == "updated_at"))
        .collect();
    if cols.is_empty() {
        if mode == UpsertMode::FailClosed {
            return Err("event_categories: row has no applicable columns".into());
        }
        return Ok(false);
    }

    let placeholders = (1..=cols.len())
        .map(|i| format!("?{}", i))
        .collect::<Vec<_>>()
        .join(",");
    let updates = cols
        .iter()
        .filter(|c| **c != "name")
        .map(|c| format!("{0} = excluded.{0}", c))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO event_categories ({}) VALUES ({}) \
         ON CONFLICT(name) DO UPDATE SET {}",
        cols.join(","),
        placeholders,
        updates
    );

    let params: Vec<rusqlite::types::Value> = cols
        .iter()
        .map(|c| {
            if *c == "updated_at" {
                SqlValue::Text(remote_ts.to_string())
            } else {
                json_to_sqlite(fields.get(*c).unwrap_or(&Value::Null))
            }
        })
        .collect();
    let refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    if let Err(e) = conn.execute(&sql, refs.as_slice()) {
        if mode == UpsertMode::FailClosed {
            return Err(format!("event_categories: upsert {}: {}", name, e));
        }
        eprintln!("[sync_owner] upsert event_categories '{}': {}", name, e);
        return Ok(false);
    }
    if let Err(error) = conn.execute(
        "UPDATE event_categories SET updated_at=?1
         WHERE name=?2 AND updated_at<>?1",
        rusqlite::params![remote_ts, name],
    ) {
        if mode == UpsertMode::FailClosed {
            return Err(format!(
                "event_categories: restore remote timestamp for {name}: {error}"
            ));
        }
        eprintln!(
            "[sync_owner] restore remote timestamp event_categories '{}': {}",
            name, error
        );
    }
    if mode == UpsertMode::FailClosed {
        record_row_version(
            conn,
            "event_categories",
            &logical_row_id,
            remote_ts,
            remote_device,
        )?;
    }
    crate::db::observe_sync_hlc_timestamp(conn, remote_ts)?;
    Ok(true)
}

fn json_to_sqlite(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as SV;
    match v {
        Value::Null => SV::Null,
        Value::Bool(b) => SV::Integer(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SV::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SV::Real(f)
            } else {
                SV::Text(n.to_string())
            }
        }
        Value::String(s) => SV::Text(s.clone()),
        other => SV::Text(other.to_string()),
    }
}

fn firestore_cursor_after(candidate: &FirestoreCursor, previous: &FirestoreCursor) -> bool {
    if candidate.synced_at != previous.synced_at {
        return candidate.synced_at > previous.synced_at;
    }
    match (&candidate.document_name, &previous.document_name) {
        (Some(candidate), Some(previous)) => candidate > previous,
        (Some(_), None) => true,
        _ => false,
    }
}

fn apply_firestore_document(
    conn: &Connection,
    document: &FirestoreDocument,
    local_device_id: &str,
) -> Result<Option<String>, String> {
    if document.fields.get("_device_id").and_then(Value::as_str) == Some(local_device_id) {
        return Ok(None);
    }
    let table = document
        .fields
        .get("_table")
        .and_then(Value::as_str)
        .ok_or_else(|| "Firestore document lost _table after decode".to_string())?;
    let changed = if table == "tombstones" {
        if document.fields.get("_deleted").and_then(Value::as_bool) != Some(true) {
            return Err("Firestore tombstone is missing _deleted=true".into());
        }
        let target = document
            .fields
            .get("_target_table")
            .and_then(Value::as_str)
            .ok_or_else(|| "Firestore tombstone is missing _target_table".to_string())?;
        let row_id = document
            .fields
            .get("_row_id")
            .ok_or_else(|| "Firestore tombstone is missing _row_id".to_string())?;
        let deleted_at = document
            .fields
            .get("_updated_at")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Firestore tombstone is missing _updated_at".to_string())?;
        apply_tombstone_lww(conn, target, row_id, deleted_at)?
    } else {
        upsert_row_fail_closed(conn, table, &document.fields)?
    };
    Ok(changed.then(|| table.to_string()))
}

fn apply_firestore_page(
    conn: &mut Connection,
    persisted_cursor: &FirestoreCursor,
    page_cursor: &FirestoreCursor,
    documents: &[FirestoreDocument],
    local_device_id: &str,
) -> Result<
    (
        serde_json::Map<String, Value>,
        FirestoreCursor,
        FirestoreCursor,
    ),
    String,
> {
    if documents.is_empty() {
        return Ok((
            serde_json::Map::new(),
            persisted_cursor.clone(),
            page_cursor.clone(),
        ));
    }

    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("start Firestore page transaction: {error}"))?;
    if load_firestore_cursor(&transaction)? != *persisted_cursor {
        return Err("Firestore pull cursor changed during fetch".into());
    }

    let mut totals = serde_json::Map::new();
    let mut next_persisted_cursor = persisted_cursor.clone();
    let mut next_page_cursor = page_cursor.clone();
    for document in documents {
        let synced_at = document
            .fields
            .get("_synced_at")
            .and_then(Value::as_str)
            .ok_or_else(|| "Firestore document lost _synced_at after decode".to_string())?;
        let candidate = FirestoreCursor {
            synced_at: canonical_sync_timestamp(synced_at, "Firestore document _synced_at")?,
            document_name: Some(document.name.clone()),
        };
        if !firestore_cursor_after(&candidate, &next_page_cursor) {
            return Err("Firestore page is not strictly ordered after its cursor".into());
        }
        next_page_cursor = candidate.clone();
        if firestore_cursor_after(&candidate, &next_persisted_cursor) {
            next_persisted_cursor = candidate;
        }

        if let Some(table) = apply_firestore_document(&transaction, document, local_device_id)? {
            let count = totals.get(&table).and_then(Value::as_u64).unwrap_or(0);
            totals.insert(table, json!(count + 1));
        }
    }

    if next_persisted_cursor != *persisted_cursor {
        save_firestore_cursor(&transaction, &next_persisted_cursor)?;
    }
    transaction
        .commit()
        .map_err(|error| format!("commit Firestore page: {error}"))?;
    Ok((totals, next_persisted_cursor, next_page_cursor))
}

/// Fetch all owner-sync documents page-by-page. Network awaits never hold the
/// SQLite writer. Each decoded page and its tuple cursor commit atomically.
async fn pull_all(
    db: &HanniDb,
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<serde_json::Map<String, Value>, String> {
    let (mut persisted_cursor, dev_id) = {
        let conn = db.conn();
        (load_firestore_cursor(&conn)?, device_id(&conn)?)
    };
    let mut page_cursor = firestore_overlap_cursor(&persisted_cursor)?;
    let parent = format!("owners/{owner_uid}");
    let mut totals = serde_json::Map::new();

    for _page in 0..1_000usize {
        let documents =
            run_query(
                client,
                token,
                project_id,
                &parent,
                "data",
                &page_cursor,
                false,
            )
            .await?;
        let page_len = documents.len();
        let (page_totals, next_persisted_cursor, next_page_cursor) = {
            let mut conn = db.conn();
            apply_firestore_page(
                &mut conn,
                &persisted_cursor,
                &page_cursor,
                &documents,
                &dev_id,
            )?
        };
        for (table, value) in page_totals {
            let count = totals.get(&table).and_then(Value::as_u64).unwrap_or(0);
            totals.insert(table, json!(count + value.as_u64().unwrap_or(0)));
        }
        persisted_cursor = next_persisted_cursor;
        page_cursor = next_page_cursor;
        if page_len < PULL_LIMIT as usize {
            return Ok(totals);
        }
    }
    Err("Firestore pull exceeded the 1000-page safety limit".into())
}

// Rollout precondition: stop or upgrade every legacy Firestore writer before
// the first v3 migration. A clean scan is conclusive only without an old
// client writing non-server cursors afterward; mixed-generation sync is not
// supported by this protocol generation.
const FIRESTORE_CURSOR_V3_MARKER: &str = "cloud_owner_firestore_server_cursor_v1";
const FIRESTORE_SCOPE_KEY: &str = "cloud_owner_firestore_scope_v1";

fn sync_scope_fingerprint(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

fn prepare_firestore_cursor_v3_replay(
    conn: &Connection,
    scope_fingerprint: &str,
) -> Result<bool, String> {
    let transaction = conn
        .unchecked_transaction()
        .map_err(|error| format!("start Firestore cursor migration: {error}"))?;
    if get_setting_checked(&transaction, FIRESTORE_CURSOR_V3_MARKER)?.is_some()
        && get_setting_checked(&transaction, FIRESTORE_SCOPE_KEY)?.as_deref()
            == Some(scope_fingerprint)
    {
        return Ok(false);
    }
    for table in SYNC_TABLES {
        let timestamp_key = push_cursor_key(table);
        let id_key = row_cursor_id_key(&timestamp_key);
        transaction
            .execute(
                "DELETE FROM app_settings WHERE key IN (?1,?2)",
                rusqlite::params![timestamp_key, id_key],
            )
            .map_err(|error| format!("reset Firestore push cursor for {table}: {error}"))?;
    }
    for key in [
        "cloud_owner_v2_push_tombstones",
        "cloud_owner_v2_push_tombstones_table",
        "cloud_owner_v2_push_tombstones_row_id",
        FIRESTORE_PULL_TS_KEY,
        FIRESTORE_PULL_NAME_KEY,
    ] {
        transaction
            .execute("DELETE FROM app_settings WHERE key=?1", [key])
            .map_err(|error| format!("reset Firestore cursor {key}: {error}"))?;
    }
    set_setting_checked(
        &transaction,
        FIRESTORE_CURSOR_V3_MARKER,
        &chrono::Utc::now().to_rfc3339(),
    )?;
    set_setting_checked(&transaction, FIRESTORE_SCOPE_KEY, scope_fingerprint)?;
    transaction
        .commit()
        .map_err(|error| format!("commit Firestore cursor migration: {error}"))?;
    Ok(true)
}

async fn ensure_firestore_cursor_v3(
    db: &HanniDb,
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<(bool, usize), String> {
    let local_device_id = device_id(&db.conn())?;
    let scope_fingerprint = sync_scope_fingerprint(&[project_id, owner_uid, &local_device_id]);
    if get_setting_checked(&db.conn(), FIRESTORE_CURSOR_V3_MARKER)?.is_some()
        && get_setting_checked(&db.conn(), FIRESTORE_SCOPE_KEY)?.as_deref()
            == Some(scope_fingerprint.as_str())
    {
        return Ok((false, 0));
    }
    let migrated =
        migrate_firestore_cursor_generation(client, token, project_id, owner_uid).await?;
    let replayed = prepare_firestore_cursor_v3_replay(&db.conn(), &scope_fingerprint)?;
    Ok((replayed, migrated))
}

fn invalidate_firestore_cursor_marker(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "DELETE FROM app_settings WHERE key=?1",
        [FIRESTORE_CURSOR_V3_MARKER],
    )
    .map_err(|error| format!("invalidate Firestore cursor marker: {error}"))?;
    Ok(())
}

async fn repair_firestore_cursor_generation(
    db: &HanniDb,
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    owner_uid: &str,
) -> Result<(bool, usize), String> {
    let local_device_id = device_id(&db.conn())?;
    let scope_fingerprint = sync_scope_fingerprint(&[project_id, owner_uid, &local_device_id]);
    let migrated =
        migrate_firestore_cursor_generation(client, token, project_id, owner_uid).await?;
    // Keep the old marker and every cursor until the remote clean scan passes.
    // Deleting only the marker after that makes a crash retry the safe path.
    invalidate_firestore_cursor_marker(&db.conn())?;
    let replayed = prepare_firestore_cursor_v3_replay(&db.conn(), &scope_fingerprint)?;
    Ok((replayed, migrated))
}

// ── Top-level push/pull ──────────────────────────────────────────────────

pub(crate) async fn push_inner(db: &HanniDb) -> Result<Value, String> {
    {
        let conn = db.conn();
        crate::db::verify_sync_schema(&conn)?;
    }
    let backend = get_setting_checked(&db.conn(), "cloud_owner_backend")?;
    if backend.as_deref() == Some("github") {
        return crate::sync_github::gh_push(db).await;
    }
    let _firestore_guard = FIRESTORE_SYNC_LOCK.lock().await;
    let (token, owner_uid, project_id) = resolve_creds(db).await?;
    let client = reqwest::Client::new();
    let (replayed_firestore_cursor_v3, migrated_firestore_documents) =
        ensure_firestore_cursor_v3(db, &client, &token, &project_id, &owner_uid).await?;
    let mut totals = serde_json::Map::new();
    let mut total = 0usize;
    for table in SYNC_TABLES {
        let n = push_table(db, table, &client, &token, &project_id, &owner_uid)
            .await
            .map_err(|e| format!("push {}: {}", table, e))?;
        totals.insert((*table).into(), json!(n));
        total += n;
    }
    let n = push_tombstones(db, &client, &token, &project_id, &owner_uid).await?;
    totals.insert("tombstones".into(), json!(n));
    total += n;
    {
        let conn = db.conn();
        set_setting_checked(
            &conn,
            "cloud_owner_v2_last_push_ts",
            &chrono::Utc::now().to_rfc3339(),
        )?;
    }
    Ok(json!({
        "pushed": total,
        "by_table": Value::Object(totals),
        "replayed_firestore_cursor_v3": replayed_firestore_cursor_v3,
        "migrated_firestore_documents": migrated_firestore_documents
    }))
}

pub(crate) async fn pull_inner(db: &HanniDb) -> Result<Value, String> {
    {
        let conn = db.conn();
        crate::db::verify_sync_schema(&conn)?;
    }
    let backend = get_setting_checked(&db.conn(), "cloud_owner_backend")?;
    if backend.as_deref() == Some("github") {
        return crate::sync_github::gh_pull(db).await;
    }
    let _firestore_guard = FIRESTORE_SYNC_LOCK.lock().await;
    let (token, owner_uid, project_id) = resolve_creds(db).await?;
    let client = reqwest::Client::new();
    let (mut replayed_firestore_cursor_v3, mut migrated_firestore_documents) =
        ensure_firestore_cursor_v3(db, &client, &token, &project_id, &owner_uid).await?;
    let current_totals = match pull_all(db, &client, &token, &project_id, &owner_uid).await {
        Ok(totals) => totals,
        Err(error) if error.contains(FIRESTORE_CURSOR_MIGRATION_REQUIRED) => {
            let (replayed, migrated) =
                repair_firestore_cursor_generation(db, &client, &token, &project_id, &owner_uid)
                    .await?;
            replayed_firestore_cursor_v3 |= replayed;
            migrated_firestore_documents += migrated;
            pull_all(db, &client, &token, &project_id, &owner_uid)
                .await
                .map_err(|retry_error| {
                    format!("pull after Firestore cursor migration: {retry_error}")
                })?
        }
        Err(error) => return Err(format!("pull: {error}")),
    };
    let mut totals = serde_json::Map::new();
    for (table, value) in current_totals {
        let count = totals.get(&table).and_then(Value::as_u64).unwrap_or(0);
        totals.insert(table, json!(count + value.as_u64().unwrap_or(0)));
    }
    let total: u64 = totals.values().filter_map(|v| v.as_u64()).sum();
    {
        let conn = db.conn();
        set_setting_checked(
            &conn,
            "cloud_owner_v2_last_pull_ts",
            &chrono::Utc::now().to_rfc3339(),
        )?;
    }
    Ok(json!({
        "applied": total,
        "by_table": Value::Object(totals),
        "replayed_firestore_cursor_v3": replayed_firestore_cursor_v3,
        "migrated_firestore_documents": migrated_firestore_documents
    }))
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
pub async fn debug_owner_list(table: String, db: State<'_, HanniDb>) -> Result<Value, String> {
    let (token, owner_uid, project_id) = resolve_creds(&db).await?;
    let client = reqwest::Client::new();
    let parent = format!("owners/{}/v2/{}", owner_uid, table);
    let url = format!(
        "{}/projects/{}/databases/(default)/documents/{}",
        firestore_host(),
        project_id,
        parent
    );
    let resp = client
        .get(&url)
        .bearer_auth(&token)
        .query(&[("pageSize", "50")])
        .send()
        .await
        .map_err(|e| format!("get: {}", e))?;
    let status = resp.status();
    let body: Value = resp.json().await.map_err(|e| format!("body: {}", e))?;
    let names: Vec<String> = body
        .get("documents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|d| d.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
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
pub fn cloud_owner_status(db: State<'_, HanniDb>) -> Result<OwnerSyncStatus, String> {
    let conn = db.conn();
    let session = load_google_session(&conn)?;
    let cfg = load_google_config(&conn)?;
    let configured = cfg.is_some() && session.is_some();
    Ok(OwnerSyncStatus {
        configured,
        device_id: device_id(&conn)?,
        last_push_ts: get_setting_checked(&conn, "cloud_owner_v2_last_push_ts")?,
        last_pull_ts: get_setting_checked(&conn, "cloud_owner_v2_last_pull_ts")?,
        owner_uid: session.map(|s| s.uid),
    })
}

#[cfg(test)]
#[path = "sync_owner_tests.rs"]
mod tests;
