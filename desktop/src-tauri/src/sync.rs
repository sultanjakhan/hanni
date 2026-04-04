// sync.rs — CR-SQLite sync core: changeset extraction, application, relay communication
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Change {
    pub table: String,
    pub pk: String,
    pub cid: String,
    pub val: Option<serde_json::Value>,
    pub col_version: i64,
    pub db_version: i64,
    pub site_id: String, // hex-encoded
    pub cl: i64,
    pub seq: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPayload {
    pub device_id: String,
    pub changes: Vec<Change>,
    pub db_version: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub changes: Vec<Change>,
    pub server_version: i64,
}

/// Get local db_version (last change version)
pub fn get_db_version(conn: &Connection) -> i64 {
    conn.query_row("SELECT crsql_db_version()", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Get this device's site_id as hex string
pub fn get_site_id(conn: &Connection) -> String {
    conn.query_row(
        "SELECT lower(hex(crsql_site_id()))", [], |r| r.get::<_, String>(0),
    ).unwrap_or_default()
}

/// Extract local changes since `since_version`
pub fn get_local_changes(conn: &Connection, since_version: i64) -> Vec<Change> {
    let mut stmt = conn.prepare(
        "SELECT \"table\", \"pk\", \"cid\", \"val\", \"col_version\",
                \"db_version\", COALESCE(lower(hex(\"site_id\")), ''), \"cl\", \"seq\"
         FROM crsql_changes
         WHERE db_version > ?1 AND site_id IS NULL"
    ).unwrap();

    stmt.query_map([since_version], |row| {
        Ok(Change {
            table: row.get(0)?,
            pk: row.get(1)?,
            cid: row.get(2)?,
            val: row.get::<_, Option<String>>(3)?
                .map(|v| serde_json::Value::String(v)),
            col_version: row.get(4)?,
            db_version: row.get(5)?,
            site_id: row.get(6)?,
            cl: row.get(7)?,
            seq: row.get(8)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect()
}

/// Apply remote changes to local database
pub fn apply_remote_changes(conn: &Connection, changes: &[Change]) -> Result<(), String> {
    let mut stmt = conn.prepare(
        "INSERT INTO crsql_changes
            (\"table\", \"pk\", \"cid\", \"val\", \"col_version\",
             \"db_version\", \"site_id\", \"cl\", \"seq\")
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, unhex(?7), ?8, ?9)"
    ).map_err(|e| e.to_string())?;

    for c in changes {
        let val_str = c.val.as_ref().map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        });
        stmt.execute(rusqlite::params![
            c.table, c.pk, c.cid, val_str,
            c.col_version, c.db_version, c.site_id, c.cl, c.seq
        ]).map_err(|e| format!("apply change to {}: {}", c.table, e))?;
    }
    Ok(())
}

/// Push local changes to relay server
pub async fn push_changes(
    relay_url: &str, device_token: &str, payload: &SyncPayload,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client.post(format!("{}/sync/push", relay_url))
        .header("Authorization", format!("Bearer {}", device_token))
        .json(payload)
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Push failed: {}", resp.status()));
    }
    Ok(())
}

/// Pull remote changes from relay server
pub async fn pull_changes(
    relay_url: &str, device_token: &str, since_version: i64,
) -> Result<PullResponse, String> {
    let client = reqwest::Client::new();
    let resp = client.get(format!("{}/sync/pull?since={}", relay_url, since_version))
        .header("Authorization", format!("Bearer {}", device_token))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Pull failed: {}", resp.status()));
    }
    resp.json::<PullResponse>().await.map_err(|e| e.to_string())
}

/// Call crsql_finalize() — must be called before closing DB
pub fn finalize_crsql(conn: &Connection) {
    let _ = conn.execute_batch("SELECT crsql_finalize();");
}
