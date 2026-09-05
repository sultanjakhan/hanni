//! Durable encrypted health delivery. The same core runs on desktop and in an
//! Android WorkManager JNI call; no Tauri/WebView is required by the core.
#[path = "cloud_relay_checkpoint.rs"]
mod checkpoint;
#[path = "cloud_relay_fragments.rs"]
mod fragments;
#[path = "cloud_relay_identity.rs"]
mod identity;
#[path = "cloud_relay_raw.rs"]
mod raw;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, time::Duration};

pub(crate) const TABLES: &[&str] = &[
    "health_log",
    "sleep_sessions",
    "sleep_stages",
    "heart_rate_samples",
    "health_records",
];
const PLAIN_LIMIT: usize = 60_000;
const RESPONSE_LIMIT: usize = 600 * 1024;
static RUN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Never derive Debug: this object contains the device bearer and the E2E key.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RelayConfig {
    pub v: u8,
    pub endpoint: String,
    pub device_id: String,
    pub key_id: String,
    pub token: String,
    pub key: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sleep_source_store_id: Option<String>,
}

fn opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn decode(value: &str, length: usize) -> Result<Vec<u8>, String> {
    let bytes = B64.decode(value).map_err(|_| "relay_invalid_encoding")?;
    if bytes.len() != length || B64.encode(&bytes) != value {
        return Err("relay_invalid_encoding".into());
    }
    Ok(bytes)
}
impl RelayConfig {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("relay_invalid_config".into());
        }
        let cfg: Self = serde_json::from_str(raw).map_err(|_| "relay_invalid_config")?;
        let url = reqwest::Url::parse(&cfg.endpoint).map_err(|_| "relay_invalid_endpoint")?;
        let secure = url.scheme() == "https";
        #[cfg(test)]
        let secure = secure || (url.scheme() == "http" && url.host_str() == Some("127.0.0.1"));
        if cfg.v != 1
            || !secure
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || !opaque_id(&cfg.device_id)
            || !opaque_id(&cfg.key_id)
        {
            return Err("relay_invalid_config".into());
        }
        decode(&cfg.token, 32)?;
        decode(&cfg.key, 32)?;
        if cfg.sleep_source_store_id.as_ref().is_some_and(|id| {
            uuid::Uuid::parse_str(id)
                .ok()
                .map(|v| v.to_string())
                .as_deref()
                != Some(id.as_str())
        }) {
            return Err("relay_invalid_sleep_source".into());
        }
        Ok(cfg)
    }
    fn scope(&self) -> String {
        // Credential rotation is a separate operation: never reuse a cursor or
        // silently discard an outbox when destination, identity or key changes.
        hash(
            &serde_json::to_vec(&json!([
                self.endpoint.trim_end_matches('/'),
                self.device_id,
                self.key_id,
                self.key,
                self.sleep_source_store_id
            ]))
            .expect("JSON strings serialize"),
        )
    }
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint.trim_end_matches('/'), path)
    }
}

// Field order is the server's canonical-envelope hashing contract.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    v: u8,
    alg: String,
    key_id: String,
    nonce: String,
    ciphertext: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    client_seq: i64,
    batch_id: String,
    envelope: Envelope,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    t: String,
    f: Map<String, Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tomb {
    tt: String,
    id: Value,
    deleted_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    identity: Option<identity::TombIdentity>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    v: u8,
    kind: String,
    applied_seq: i64,
    rows: Vec<Row>,
    tombs: Vec<Tomb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fragment: Option<fragments::Fragment>,
}
#[derive(Deserialize)]
struct StoredBatch {
    seq: i64,
    client_seq: i64,
    sender_device_id: String,
    batch_id: String,
    envelope: Envelope,
    envelope_sha256: String,
}
#[derive(Deserialize)]
struct Page {
    batches: Vec<StoredBatch>,
    next_cursor: i64,
    latest_seq: i64,
    has_more: bool,
}
#[derive(Deserialize)]
struct Ack {
    seq: i64,
    client_seq: i64,
    sender_device_id: String,
    batch_id: String,
    envelope_sha256: String,
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn envelope_hash(envelope: &Envelope) -> Result<String, String> {
    Ok(hash(
        &serde_json::to_vec(envelope).map_err(|_| "relay_encode_failed")?,
    ))
}
fn aad(sender: &str, batch: &str, key_id: &str, client_seq: i64) -> Vec<u8> {
    serde_json::to_vec(&json!([
        "hanni-relay-v2",
        sender,
        batch,
        key_id,
        client_seq
    ]))
    .expect("JSON strings serialize")
}
fn encrypt(cfg: &RelayConfig, payload: &Payload, client_seq: i64) -> Result<Batch, String> {
    if !(1..=9_007_199_254_740_991).contains(&client_seq) {
        return Err("relay_client_sequence_invalid".into());
    }
    let plain = serde_json::to_vec(payload).map_err(|_| "relay_encode_failed")?;
    if plain.len() > PLAIN_LIMIT {
        return Err("relay_record_too_large".into());
    }
    let key: [u8; 32] = decode(&cfg.key, 32)?
        .try_into()
        .map_err(|_| "relay_invalid_key")?;
    let id = uuid::Uuid::new_v4().to_string();
    let blob = crate::sync_crypto::seal(
        &key,
        &aad(&cfg.device_id, &id, &cfg.key_id, client_seq),
        &plain,
    )?;
    Ok(Batch {
        client_seq,
        batch_id: id,
        envelope: Envelope {
            v: 1,
            alg: "XChaCha20-Poly1305".into(),
            key_id: cfg.key_id.clone(),
            nonce: B64.encode(&blob[..24]),
            ciphertext: B64.encode(&blob[24..]),
        },
    })
}
fn decrypt(cfg: &RelayConfig, batch: &StoredBatch) -> Result<Payload, String> {
    let env = &batch.envelope;
    if env.v != 1
        || !(1..=9_007_199_254_740_991).contains(&batch.client_seq)
        || env.alg != "XChaCha20-Poly1305"
        || env.key_id != cfg.key_id
        || !opaque_id(&batch.sender_device_id)
        || uuid::Uuid::parse_str(&batch.batch_id)
            .map(|id| id.to_string())
            .ok()
            .as_deref()
            != Some(batch.batch_id.as_str())
        || envelope_hash(env)? != batch.envelope_sha256
    {
        return Err("relay_invalid_envelope".into());
    }
    let mut blob = decode(&env.nonce, 24)?;
    let ct = B64
        .decode(&env.ciphertext)
        .map_err(|_| "relay_invalid_encoding")?;
    if ct.len() < 16 || ct.len() > 65536 || B64.encode(&ct) != env.ciphertext {
        return Err("relay_invalid_envelope".into());
    }
    blob.extend(ct);
    let key: [u8; 32] = decode(&cfg.key, 32)?
        .try_into()
        .map_err(|_| "relay_invalid_key")?;
    let plain = crate::sync_crypto::open(
        &key,
        &aad(
            &batch.sender_device_id,
            &batch.batch_id,
            &env.key_id,
            batch.client_seq,
        ),
        &blob,
    )
    .map_err(|_| "relay_authentication_failed")?;
    let payload: Payload = serde_json::from_slice(&plain).map_err(|_| "relay_invalid_payload")?;
    if payload.v != 1
        || !matches!(payload.kind.as_str(), "changes" | "receipt" | "fragment")
        || payload.applied_seq < 0
        || payload.applied_seq >= batch.seq
        || (payload.kind == "receipt" && (!payload.rows.is_empty() || !payload.tombs.is_empty()))
        || ((payload.kind == "fragment") != payload.fragment.is_some())
        || (payload.fragment.is_some() && (!payload.rows.is_empty() || !payload.tombs.is_empty()))
    {
        return Err("relay_invalid_payload".into());
    }
    Ok(payload)
}

fn sql<T>(result: rusqlite::Result<T>) -> Result<T, String> {
    result.map_err(|_| "relay_database_failed".into())
}
fn scalar(conn: &Connection, statement: &str) -> Result<i64, String> {
    sql(conn.query_row(statement, [], |row| row.get(0)))
}
fn initialize(conn: &mut Connection, cfg: &RelayConfig) -> Result<(), String> {
    let tx = sql(conn.transaction_with_behavior(TransactionBehavior::Immediate))?;
    sql(tx.execute_batch("CREATE TABLE IF NOT EXISTS cloud_relay_state(
        id INTEGER PRIMARY KEY CHECK(id=1),scope TEXT NOT NULL,receive_seq INTEGER NOT NULL DEFAULT 0,
        receipt_needed INTEGER NOT NULL DEFAULT 0,not_before INTEGER NOT NULL DEFAULT 0,
        upload_not_before INTEGER NOT NULL DEFAULT 0,pull_not_before INTEGER NOT NULL DEFAULT 0,
        last_ok TEXT,last_error TEXT,upload_error TEXT,pull_error TEXT);
      CREATE TABLE IF NOT EXISTS cloud_relay_control(id INTEGER PRIMARY KEY CHECK(id=1),applying INTEGER NOT NULL);
      INSERT OR IGNORE INTO cloud_relay_control VALUES(1,0);
      CREATE TABLE IF NOT EXISTS cloud_relay_dirty(seq INTEGER PRIMARY KEY AUTOINCREMENT,
        table_name TEXT NOT NULL,row_id TEXT NOT NULL,UNIQUE(table_name,row_id));
      CREATE INDEX IF NOT EXISTS cloud_relay_dirty_type_seq ON cloud_relay_dirty(table_name,seq);
      CREATE TABLE IF NOT EXISTS cloud_relay_selection(singleton INTEGER PRIMARY KEY CHECK(singleton=1),
        urgent_next INTEGER NOT NULL CHECK(urgent_next IN (0,1)));
      INSERT OR IGNORE INTO cloud_relay_selection VALUES(1,1);
      CREATE TABLE IF NOT EXISTS cloud_relay_outbox(local_seq INTEGER PRIMARY KEY AUTOINCREMENT,
        batch_id TEXT NOT NULL UNIQUE,body TEXT NOT NULL,envelope_hash TEXT NOT NULL,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS cloud_relay_receipts(device_id TEXT PRIMARY KEY,applied_seq INTEGER NOT NULL,received_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS cloud_relay_sender_watermarks(device_id TEXT PRIMARY KEY,
        client_seq INTEGER NOT NULL CHECK(client_seq>0),server_seq INTEGER NOT NULL CHECK(server_seq>0));
      CREATE TABLE IF NOT EXISTS cloud_relay_freshness(table_name TEXT PRIMARY KEY,record_updated_at TEXT NOT NULL,received_at TEXT NOT NULL);"))?;
    raw::initialize(&tx)?;
    identity::initialize(&tx)?;
    fragments::initialize(&tx)?;
    checkpoint::initialize(&tx)?;
    let existing: Option<String> = sql(tx
        .query_row("SELECT scope FROM cloud_relay_state WHERE id=1", [], |r| {
            r.get(0)
        })
        .optional())?;
    if existing.as_ref().is_some_and(|scope| scope != &cfg.scope()) {
        return Err("relay_pairing_changed".into());
    }
    if existing.is_none() {
        sql(tx.execute(
            "INSERT INTO cloud_relay_state(id,scope) VALUES(1,?1)",
            [cfg.scope()],
        ))?;
        for table in TABLES {
            let projection_filter = crate::health_raw_sleep_projection::transport_row_filter(&tx, table)?;
            // The initial scan repairs old timestamp-cursor gaps. Thereafter a
            // journal records each changed key, including equal timestamps.
            sql(tx.execute(
                &format!(
                    "INSERT OR IGNORE INTO cloud_relay_dirty(table_name,row_id)
                SELECT ?1,CAST(id AS TEXT) FROM {table} WHERE ({projection_filter}) ORDER BY id"
                ),
                [table],
            ))?;
            let projection_filter = crate::health_raw_sleep_projection::transport_tomb_filter(&tx, "sync_tombstones.table_name", "sync_tombstones.row_id")?;
            sql(tx.execute(
                &format!("INSERT OR IGNORE INTO cloud_relay_dirty(table_name,row_id)
                SELECT table_name,row_id FROM sync_tombstones WHERE table_name=?1 AND ({projection_filter})"),
                [table],
            ))?;
        }
    }
    for table in TABLES {
        if *table == raw::TABLE {
            continue;
        } // Shared raw schema owns its journal; deletion is a source revision.
        for (action, reference) in [("INSERT", "NEW"), ("UPDATE", "NEW"), ("DELETE", "OLD")] {
            sql(tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS relay_{table}_{action}
                AFTER {action} ON {table} WHEN (SELECT applying FROM cloud_relay_control WHERE id=1)=0
                BEGIN INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id)
                VALUES('{table}',CAST({reference}.id AS TEXT)); END;")))?;
        }
    }
    for action in ["INSERT", "UPDATE"] {
        sql(tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS relay_tomb_{action}
            AFTER {action} ON sync_tombstones
            WHEN NEW.table_name IN ('health_log','sleep_sessions','sleep_stages','heart_rate_samples')
            AND (SELECT applying FROM cloud_relay_control WHERE id=1)=0
            BEGIN INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id)
            VALUES(NEW.table_name,CAST(NEW.row_id AS TEXT)); END;")))?;
    }
    sql(tx.commit())
}

fn enqueue(conn: &mut Connection, cfg: &RelayConfig) -> Result<bool, String> {
    let tx = sql(conn.transaction_with_behavior(TransactionBehavior::Immediate))?;
    if scalar(&tx, "SELECT COUNT(*) FROM cloud_relay_outbox")? > 0 {
        return Ok(true);
    }
    let mut payload = Payload {
        v: 1,
        kind: "changes".into(),
        applied_seq: fragments::applied_cursor(&tx)?,
        rows: vec![],
        tombs: vec![],
        fragment: None,
    };
    // Historical local materialization keys are discardable, immutable outboxes are not.
    let projection_filter = crate::health_raw_sleep_projection::transport_tomb_filter(&tx, "cloud_relay_dirty.table_name", "cloud_relay_dirty.row_id")?;
    sql(tx.execute(&format!("DELETE FROM cloud_relay_dirty WHERE NOT ({projection_filter})"), []))?;
    let mut entries: Vec<(i64, String, String)> = {
        let mut stmt = sql(tx.prepare(
            "SELECT seq,table_name,row_id FROM cloud_relay_dirty
            ORDER BY CASE table_name WHEN 'health_log' THEN 0 WHEN 'sleep_sessions' THEN 1
            WHEN 'sleep_stages' THEN 2 ELSE 3 END,seq LIMIT 256",
        ))?;
        let mapped = sql(stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))?;
        sql(mapped.collect())?
    };
    // Only unencrypted raw keys can jump ahead. Raw archive rows have no
    // foreign-key dependency; the remaining legacy candidates retain their
    // existing parent-before-child order. Alternating an oldest-only turn
    // prevents even a continuous stream of large urgent rows starving history.
    if scalar(&tx, "SELECT urgent_next FROM cloud_relay_selection WHERE singleton=1")? == 1 {
        let urgent: Option<(i64,String,String)> = sql(tx.query_row(
            "SELECT seq,table_name,row_id FROM cloud_relay_dirty WHERE table_name='health_records' ORDER BY seq DESC LIMIT 1",
            [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).optional())?;
        if let Some(urgent) = urgent {
            entries.retain(|(seq,_,_)|*seq!=urgent.0);
            entries.insert(0,urgent);
            entries.truncate(256);
        }
    }
    let writer: String = sql(tx.query_row(
        "SELECT value FROM app_settings WHERE key='device_id'",
        [],
        |r| r.get(0),
    ))?;
    if writer.is_empty() {
        return Err("relay_missing_writer".into());
    }
    let mut captured = vec![];
    for (seq, table, id) in entries {
        if !TABLES.contains(&table.as_str()) {
            return Err("relay_invalid_table".into());
        }
        let raw_id = rusqlite::types::Value::Text(id.clone());
        if let Some(Value::Object(mut fields)) =
            crate::sync_owner::row_to_json(&tx, &table, &raw_id)
                .map_err(|_| "relay_row_read_failed")?
        {
            let timestamp = fields
                .get("updated_at")
                .and_then(Value::as_str)
                .ok_or("relay_missing_timestamp")?;
            let timestamp = crate::sync_owner::canonical_sync_timestamp(timestamp, "relay")
                .map_err(|_| "relay_invalid_timestamp")?;
            let version: Option<(String, String)> = sql(tx
                .query_row(
                    "SELECT updated_at,device_id
                FROM sync_row_versions WHERE table_name=?1 AND row_id=?2",
                    params![table, id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional())?;
            let origin = version
                .filter(|(ts, _)| {
                    crate::sync_owner::canonical_sync_timestamp(ts, "relay")
                        .ok()
                        .as_ref()
                        == Some(&timestamp)
                })
                .map(|(_, device)| device)
                .unwrap_or_else(|| writer.clone());
            fields.insert("updated_at".into(), json!(timestamp));
            fields.insert("_updated_at".into(), json!(timestamp));
            fields.insert("_device_id".into(), json!(origin));
            payload.rows.push(Row {
                t: table,
                f: fields,
            });
            if serde_json::to_vec(&payload)
                .map_err(|_| "relay_encode_failed")?
                .len()
                > PLAIN_LIMIT
            {
                let oversized = payload.rows.pop().expect("the current row was appended");
                if captured.is_empty() {
                    fragments::enqueue(&tx, cfg, &oversized, payload.applied_seq, seq)?;
                    rotate_selection(&tx)?;
                    sql(tx.commit())?;
                    return Ok(true);
                }
                break;
            }
        } else {
            let deleted_at: String = sql(tx.query_row(
                "SELECT deleted_at FROM sync_tombstones WHERE table_name=?1 AND row_id=?2",
                params![table, id],
                |r| r.get(0),
            ))?;
            payload.tombs.push(Tomb {
                identity: identity::tomb_identity(&tx, &table, &json!(id))?,
                tt: table,
                id: json!(id),
                deleted_at,
            });
            if serde_json::to_vec(&payload)
                .map_err(|_| "relay_encode_failed")?
                .len()
                > PLAIN_LIMIT
            {
                payload.tombs.pop();
                break;
            }
        }
        captured.push(seq);
    }
    if captured.is_empty() {
        if scalar(
            &tx,
            "SELECT receipt_needed FROM cloud_relay_state WHERE id=1",
        )? == 0
        {
            sql(tx.commit())?;
            return Ok(false);
        }
        payload.kind = "receipt".into();
    }
    persist_payload(&tx, cfg, &payload)?;
    for seq in captured {
        sql(tx.execute("DELETE FROM cloud_relay_dirty WHERE seq=?1", [seq]))?;
    }
    sql(tx.execute(
        "UPDATE cloud_relay_state SET receipt_needed=0 WHERE id=1",
        [],
    ))?;
    rotate_selection(&tx)?;
    sql(tx.commit())?;
    Ok(true)
}

fn rotate_selection(conn: &Connection) -> Result<(),String> {
    if sql(conn.execute("UPDATE cloud_relay_selection SET urgent_next=1-urgent_next WHERE singleton=1",[]))?!=1 {
        return Err("relay_selection_state_missing".into());
    }
    Ok(())
}

fn persist_payload(conn: &Connection, cfg: &RelayConfig, payload: &Payload) -> Result<(), String> {
    // AUTOINCREMENT is a durable per-device sequence, assigned in the same
    // transaction as encryption and journal capture. Upload never skips an ACK.
    let seq = scalar(
        conn,
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='cloud_relay_outbox'),0)+1",
    )?;
    let batch = encrypt(cfg, payload, seq)?;
    let body = serde_json::to_string(&batch).map_err(|_| "relay_encode_failed")?;
    sql(conn.execute("INSERT INTO cloud_relay_outbox(local_seq,batch_id,body,envelope_hash,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![seq,batch.batch_id,body,envelope_hash(&batch.envelope)?,chrono::Utc::now().to_rfc3339()]))?;
    Ok(())
}

fn read_response(
    conn: &Connection,
    response: reqwest::blocking::Response,
    upload: bool,
) -> Result<Vec<u8>, String> {
    let status = response.status();
    if !status.is_success() {
        let retry = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(if status.as_u16() == 507 { 3600 } else { 60 })
            .clamp(1, 86400);
        let column = if upload {
            "upload_not_before"
        } else {
            "pull_not_before"
        };
        sql(conn.execute(
            &format!("UPDATE cloud_relay_state SET {column}=MAX({column},?1) WHERE id=1"),
            [chrono::Utc::now().timestamp() + retry],
        ))?;
        #[cfg(test)]
        {
            let mut diagnostic = String::new();
            let _ = response.take(512).read_to_string(&mut diagnostic);
            let parsed = serde_json::from_str::<Value>(&diagnostic).ok();
            let code = parsed
                .as_ref()
                .and_then(|v| v.get("error"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            if code.len() <= 64 && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                eprintln!("synthetic relay HTTP {}: {}", status.as_u16(), code);
            }
        }
        return Err(format!("relay_http_{}", status.as_u16()));
    }
    let mut body = vec![];
    response
        .take((RESPONSE_LIMIT + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| "relay_response_failed")?;
    if body.len() > RESPONSE_LIMIT {
        return Err("relay_response_too_large".into());
    }
    Ok(body)
}

fn upload(
    conn: &mut Connection,
    cfg: &RelayConfig,
    http: &reqwest::blocking::Client,
) -> Result<usize, String> {
    let out: Option<(String, String, String, i64)> = sql(conn
        .query_row(
            "SELECT batch_id,body,envelope_hash,local_seq FROM cloud_relay_outbox ORDER BY local_seq LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?,r.get(3)?)),
        )
        .optional())?;
    let Some((id, body, digest, client_seq)) = out else {
        return Ok(0);
    };
    let response = http
        .post(cfg.url("/v1/batches"))
        .bearer_auth(&cfg.token)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|_| "relay_network_unavailable")?;
    let ack: Ack = serde_json::from_slice(&read_response(conn, response, true)?)
        .map_err(|_| "relay_invalid_ack")?;
    if ack.seq < 1
        || ack.client_seq != client_seq
        || ack.sender_device_id != cfg.device_id
        || ack.batch_id != id
        || ack.envelope_sha256 != digest
    {
        return Err("relay_invalid_ack".into());
    }
    // No local timestamp cursor is advanced here; the immutable encrypted
    // batch was durable before its journal keys were removed.
    sql(conn.execute(
        "DELETE FROM cloud_relay_outbox WHERE batch_id=?1 AND envelope_hash=?2",
        params![id, digest],
    ))?;
    sql(conn.execute(
        "UPDATE cloud_relay_state SET upload_error=NULL WHERE id=1",
        [],
    ))?;
    Ok(1)
}

fn apply_authenticated_row(conn: &Connection, mut row: Row, now: &str) -> Result<usize, String> {
    let mut applied = 0;
    if !TABLES.contains(&row.t.as_str()) {
        return Err("relay_invalid_table".into());
    }
    let timestamp = row
        .f
        .get("updated_at")
        .and_then(Value::as_str)
        .ok_or("relay_missing_timestamp")?;
    let version = row
        .f
        .get("_updated_at")
        .and_then(Value::as_str)
        .ok_or("relay_missing_timestamp")?
        .to_owned();
    if crate::sync_owner::canonical_sync_timestamp(timestamp, "relay")
        .map_err(|_| "relay_invalid_timestamp")?
        != crate::sync_owner::canonical_sync_timestamp(&version, "relay")
            .map_err(|_| "relay_invalid_timestamp")?
    {
        return Err("relay_timestamp_mismatch".into());
    }
    if row.t == raw::TABLE {
        if raw::apply(conn, &row.f)? {
            applied += 1;
        }
    } else {
        let Some(fields) = identity::translate_row(conn, &row.t, &row.f)? else {
            return Ok(0);
        };
        row.f = fields;
        if crate::sync_owner::upsert_row_fail_closed(conn, &row.t, &row.f)
            .map_err(|_| "relay_row_apply_failed")?
        {
            applied += 1;
        }
    }
    let freshness_type = if row.t == raw::TABLE {
        format!(
            "health_records:{}",
            row.f
                .get("record_type")
                .and_then(Value::as_str)
                .ok_or("relay_archive_invalid_field")?
        )
    } else if row.t == "health_log" {
        match row.f.get("type").and_then(Value::as_str) {
            Some("steps") => "health_log:steps".to_owned(),
            Some("exercise") => "health_log:exercise".to_owned(),
            _ => row.t.clone(),
        }
    } else {
        row.t.clone()
    };
    let version = crate::sync_owner::canonical_sync_timestamp(&version, "relay")
        .map_err(|_| "relay_invalid_timestamp")?;
    sql(conn.execute("INSERT INTO cloud_relay_freshness VALUES(?1,?2,?3) ON CONFLICT(table_name)
        DO UPDATE SET record_updated_at=MAX(record_updated_at,excluded.record_updated_at),received_at=excluded.received_at",
        params![freshness_type,version,now]))?;
    Ok(applied)
}

fn apply_authenticated_tomb(
    conn: &Connection,
    tomb: &Tomb,
    source_seq: i64,
) -> Result<bool, String> {
    if !TABLES.contains(&tomb.tt.as_str()) || tomb.tt == raw::TABLE {
        return Err("relay_invalid_table".into());
    }
    identity::apply_tombstone_with_identity(
        conn,
        &tomb.tt,
        &tomb.id,
        &tomb.deleted_at,
        tomb.identity.as_ref(),
        source_seq,
    )
    .map_err(|_| "relay_delete_apply_failed".into())
}

fn apply_page(
    conn: &mut Connection,
    cfg: &RelayConfig,
    before: i64,
    page: Page,
) -> Result<usize, String> {
    let last = page.batches.last().map(|b| b.seq).unwrap_or(before);
    if page.next_cursor != last
        || page.latest_seq < last
        || (page.has_more && page.latest_seq <= last)
        || (!page.has_more && page.latest_seq != last)
        || page.batches.len() > 32
    {
        return Err("relay_invalid_page".into());
    }
    let tx = sql(conn.transaction_with_behavior(TransactionBehavior::Immediate))?;
    if scalar(&tx, "SELECT receive_seq FROM cloud_relay_state WHERE id=1")? != before {
        return Err("relay_stale_cursor".into());
    }
    sql(tx.execute("UPDATE cloud_relay_control SET applying=1 WHERE id=1", []))?;
    let mut expected = before + 1;
    let mut applied = 0;
    let now = chrono::Utc::now().to_rfc3339();
    for batch in page.batches {
        if batch.seq != expected {
            return Err("relay_sequence_gap".into());
        }
        expected += 1;
        let mut payload = decrypt(cfg, &batch)?;
        // A checkpoint carries these authenticated sender positions. Binding the
        // sequence in AEAD prevents the relay replaying an old packet as new.
        let previous: i64 = sql(tx.query_row(
            "SELECT COALESCE((SELECT client_seq FROM cloud_relay_sender_watermarks WHERE device_id=?1),0)",
            [&batch.sender_device_id], |r| r.get(0),
        ))?;
        if batch.client_seq != previous + 1 {
            return Err("relay_sender_sequence_gap".into());
        }
        sql(tx.execute("INSERT INTO cloud_relay_sender_watermarks VALUES(?1,?2,?3)
            ON CONFLICT(device_id) DO UPDATE SET client_seq=excluded.client_seq,server_seq=excluded.server_seq",
            params![batch.sender_device_id,batch.client_seq,batch.seq]))?;
        if let Some(part) = payload.fragment.take() {
            if let Some(row) = fragments::accept(&tx, &batch.sender_device_id, batch.seq, part)? {
                payload.rows.push(row);
                payload.kind = "changes".into();
            }
        }
        // Keep even our authenticated partial prefix for checkpoint recovery.
        // The complete local row already exists, so no own echo is applied.
        if batch.sender_device_id == cfg.device_id {
            continue;
        }
        payload.rows.sort_by_key(|row| {
            TABLES
                .iter()
                .position(|t| *t == row.t)
                .unwrap_or(usize::MAX)
        });
        for row in payload.rows {
            applied += apply_authenticated_row(&tx, row, &now)?;
        }
        for tomb in payload.tombs {
            if apply_authenticated_tomb(&tx, &tomb, batch.seq)? {
                applied += 1;
            }
        }
        sql(tx.execute("INSERT INTO cloud_relay_receipts VALUES(?1,?2,?3) ON CONFLICT(device_id)
            DO UPDATE SET applied_seq=MAX(applied_seq,excluded.applied_seq),received_at=excluded.received_at",
            params![batch.sender_device_id,payload.applied_seq,now]))?;
        // Receipts do not generate receipts. An applied changes batch always
        // generates an automatic durable receipt, including idempotent rows.
        if payload.kind == "changes" {
            sql(tx.execute(
                "UPDATE cloud_relay_state SET receipt_needed=1 WHERE id=1",
                [],
            ))?;
        }
    }
    sql(tx.execute("UPDATE cloud_relay_control SET applying=0 WHERE id=1", []))?;
    sql(tx.execute(
        "UPDATE cloud_relay_state SET receive_seq=?1,last_ok=?2,pull_error=NULL WHERE id=1",
        params![last, now],
    ))?;
    sql(tx.commit())?;
    Ok(applied)
}

fn sync_once(conn: &mut Connection, cfg: &RelayConfig) -> Result<Value, String> {
    initialize(conn, cfg)?;
    // Local views must progress even while HTTP is unavailable or rate limited.
    let mut projection = project_local_status(conn, cfg);
    let now = chrono::Utc::now().timestamp();
    let not_before = scalar(conn, "SELECT not_before FROM cloud_relay_state WHERE id=1")?;
    if not_before > now {
        return Ok(json!({"more_pending":true,"deferred":true,"retry_after":not_before-now,"projection":projection}));
    }
    let http = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "relay_http_init_failed")?;
    let mut uploaded = 0;
    let mut errors: Vec<String> = vec![];
    // Bound a WorkManager run and preserve pending work for its next retry.
    for _ in 0..4 {
        if scalar(
            conn,
            "SELECT upload_not_before FROM cloud_relay_state WHERE id=1",
        )? > now
        {
            break;
        }
        match enqueue(conn, cfg).and_then(|pending| {
            if pending {
                upload(conn, cfg, &http)
            } else {
                Ok(0)
            }
        }) {
            Ok(0) => break,
            Ok(count) => uploaded += count,
            Err(error) => {
                sql(conn.execute(
                    "UPDATE cloud_relay_state SET upload_error=?1 WHERE id=1",
                    [&error],
                ))?;
                errors.push(error);
                break;
            }
        }
    }
    // A full upload store must remain readable. Push errors/backoff never
    // acknowledge or prevent independent inbound catch-up.
    let mut applied = 0;
    let mut more = false;
    let mut caught_up = false;
    if scalar(
        conn,
        "SELECT pull_not_before FROM cloud_relay_state WHERE id=1",
    )? <= now
    {
        let pull = checkpoint::pull(conn, cfg, &http);
        match pull {
            Ok((count, pending)) => {
                applied = count;
                more = pending;
                caught_up = !pending;
            }
            Err(error) => {
                sql(conn.execute(
                    "UPDATE cloud_relay_state SET pull_error=?1 WHERE id=1",
                    [&error],
                ))?;
                errors.push(error);
            }
        }
    } else {
        more = true;
    }
    // Includes records restored by an encrypted checkpoint. Projection failure
    // never rolls back or falsely acknowledges the independent raw archive.
    let prior_records = projection["records"].as_u64().unwrap_or(0);
    projection = project_local_status(conn, cfg);
    if let Some(records) = projection["records"].as_u64() {
        projection["records"] = json!(records + prior_records);
    }
    // Ship the receipt in this run so the sender need not wait 15 minutes.
    if scalar(
        conn,
        "SELECT upload_not_before FROM cloud_relay_state WHERE id=1",
    )? <= now
        && errors.is_empty()
    {
        match enqueue(conn, cfg).and_then(|pending| {
            if pending {
                upload(conn, cfg, &http)
            } else {
                Ok(0)
            }
        }) {
            Ok(count) => uploaded += count,
            Err(error) => {
                sql(conn.execute(
                    "UPDATE cloud_relay_state SET upload_error=?1 WHERE id=1",
                    [&error],
                ))?;
                errors.push(error);
            }
        }
    }
    // Compaction must continue even when the append store is full. Capturing
    // local pending data never acknowledges or replaces the normal outbox.
    match checkpoint::maintain(conn, cfg, &http, caught_up) {
        Ok(pending) => more |= pending,
        Err(error) => {
            sql(conn.execute(
                "UPDATE cloud_relay_checkpoint_state SET last_error=?1 WHERE id=1",
                [&error],
            ))?;
            errors.push(error);
        }
    }
    let pending = scalar(
        conn,
        "SELECT (SELECT COUNT(*) FROM cloud_relay_dirty)+(SELECT COUNT(*) FROM cloud_relay_outbox)",
    )?;
    sql(conn.execute(
        "UPDATE cloud_relay_state SET last_error=COALESCE(upload_error,pull_error,(SELECT last_error FROM cloud_relay_checkpoint_state WHERE id=1)) WHERE id=1",
        [],
    ))?;
    let error: Option<String> = sql(conn.query_row(
        "SELECT last_error FROM cloud_relay_state WHERE id=1",
        [],
        |r| r.get(0),
    ))?;
    Ok(
        json!({"uploaded_batches":uploaded,"applied_rows":applied,"pending_keys":pending,
        "checkpoint_retry_after":checkpoint::retry_after(conn)?,
        "applied_seq":fragments::applied_cursor(conn)?,"unresolved_deletions":identity::unresolved_tomb_count(conn)?,
        "error_code":error,"more_pending":more || pending>0 || !errors.is_empty() || projection["more_pending"] == true,
        "projection":projection}),
    )
}

pub(crate) fn open_existing(path: &str) -> Result<crate::worker_connection::WorkerConnection, String> {
    static EXTENSIONS: std::sync::Once = std::sync::Once::new();
    EXTENSIONS.call_once(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
    let conn = sql(Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )).map_err(|code| { eprintln!("[hanni-worker] relay_bootstrap=open_failed"); code })?;
    // Own cleanup before any schema check or extension initialization can fail.
    let mut conn = crate::worker_connection::WorkerConnection::new(conn);
    sql(conn.busy_timeout(Duration::from_secs(5)))?;
    let mode: String = sql(conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)))?;
    if mode != "wal" {
        eprintln!("[hanni-worker] relay_bootstrap=wal_required");
        return Err("relay_database_not_ready".into());
    }
    sql(conn.pragma_update(None, "foreign_keys", "ON"))?;
    // Only existing initialized databases are accepted. No app migrations,
    // calendar cleanup, seeds or silent creation happen in a headless worker.
    for table in [
        "app_settings",
        "sync_tombstones",
        "sync_row_versions",
        "sync_apply_context",
        "sync_hlc_state",
    ] {
        let count: i64 = sql(conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        ))?;
        if count != 1 {
            eprintln!("[hanni-worker] relay_bootstrap=sync_schema_missing");
            return Err("relay_database_not_ready".into());
        }
    }
    for (table, columns) in [
        (
            "health_log",
            &[
                "id",
                "date",
                "type",
                "value",
                "unit",
                "notes",
                "start_time",
                "updated_at",
            ][..],
        ),
        (
            "sleep_sessions",
            &[
                "id",
                "date",
                "start_time",
                "end_time",
                "source",
                "updated_at",
            ][..],
        ),
        (
            "sleep_stages",
            &[
                "id",
                "session_id",
                "start_time",
                "end_time",
                "stage",
                "updated_at",
            ][..],
        ),
        (
            "heart_rate_samples",
            &["id", "date", "time", "bpm", "source", "updated_at"][..],
        ),
    ] {
        let mut stmt = sql(conn.prepare(&format!("PRAGMA table_info({table})")))?;
        let fields =
            sql(stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?))))?;
        let fields: std::collections::HashMap<String, String> = sql(fields.collect())?;
        if columns.iter().any(|name| !fields.contains_key(*name))
            || fields
                .get("id")
                .is_none_or(|kind| !kind.eq_ignore_ascii_case("TEXT"))
        {
            eprintln!("[hanni-worker] relay_bootstrap=projection_schema_mismatch");
            return Err("relay_database_not_ready".into());
        }
    }
    let crsql: i64 = scalar(
        &conn,
        "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE '%__crsql_%'",
    )?;
    if crsql > 0 {
        unsafe {
            let guard = sql(rusqlite::LoadExtensionGuard::new(&conn))?;
            sql(conn.load_extension(crate::crsqlite_lib_path(), Some("sqlite3_crsqlite_init")))
                .map_err(|code| { eprintln!("[hanni-worker] relay_bootstrap=crsqlite_load_failed"); code })?;
            drop(guard);
        }
        // Only a successful load keeps the extension library mapped.
        conn.mark_crsqlite_loaded();
    }
    Ok(conn)
}

fn project_local(conn: &mut Connection, cfg: &RelayConfig) -> Result<crate::health_raw_sleep_projection::ProjectionStatus, String> {
    let tx = sql(conn.transaction_with_behavior(TransactionBehavior::Immediate))?;
    raw::initialize(&tx)?;
    let prior_remote = scalar(&tx, "SELECT remote_apply FROM sync_apply_context WHERE singleton=1")?;
    let prior_applying = scalar(&tx, "SELECT applying FROM cloud_relay_control WHERE id=1")?;
    sql(tx.execute("UPDATE sync_apply_context SET remote_apply=1 WHERE singleton=1", []))?;
    sql(tx.execute("UPDATE cloud_relay_control SET applying=1 WHERE id=1", []))?;
    let result = crate::health_raw_sleep_projection::reconcile_pending(&tx, cfg.sleep_source_store_id.as_deref(), 100)?;
    sql(tx.execute("UPDATE sync_apply_context SET remote_apply=?1 WHERE singleton=1", [prior_remote]))?;
    sql(tx.execute("UPDATE cloud_relay_control SET applying=?1 WHERE id=1", [prior_applying]))?;
    sql(tx.commit())?;
    Ok(result)
}

fn project_local_status(conn: &mut Connection, cfg: &RelayConfig) -> Value {
    match project_local(conn, cfg) {
        Ok(result) => serde_json::to_value(result).expect("projection status serializes"),
        Err(_) => json!({"status":"projection_deferred","records":0,"more_pending":false,"retry_needed":true,"error_code":"hc_sleep_projection_failed"}),
    }
}

/// Local-only materialization: no HTTP, no enabled/network gate, no secrets returned.
pub(crate) fn run_headless_projection_once(db_path: &str, config_json: &str) -> Result<String, String> {
    let cfg = RelayConfig::parse(config_json)?;
    let mut conn = open_existing(db_path)?;
    Ok(serde_json::to_string(&project_local(&mut conn, &cfg)?).map_err(|_| "hc_sleep_projection_failed")?)
}

pub(crate) fn run_headless_once(db_path: &str, config_json: &str) -> Result<String, String> {
    let cfg = RelayConfig::parse(config_json)?;
    if !cfg.enabled {
        return Ok(json!({"enabled":false,"more_pending":false}).to_string());
    }
    let _guard = RUN_LOCK.try_lock().map_err(|_| "relay_already_running")?;
    let mut conn = open_existing(db_path)?;
    let result = sync_once(&mut conn, &cfg);
    if let Err(ref code) = result {
        // Codes only: no URL, key, row identifiers or raw database/HTTP errors.
        let _ = conn.execute(
            "UPDATE cloud_relay_state SET last_error=?1 WHERE id=1",
            [code],
        );
    }
    result.map(|value| value.to_string())
}

/// Shared, read-only aggregate status for the Android and desktop interfaces.
/// Receipt progress deliberately differs from fetched progress while assembly
/// or an unresolved historical deletion remains outstanding.
pub(crate) fn database_status(conn: &Connection) -> Result<Value, String> {
    let projection = (|| -> Result<Value,String> {
        if scalar(conn,"SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hc_sleep_projection_config'")? == 0 {
            return Ok(json!({"status":"projection_not_initialized","projection_revision":"0"}));
        }
        let authority: Option<String> = sql(conn.query_row("SELECT source_store_id FROM hc_sleep_projection_config WHERE singleton=1",[],|r|r.get(0)).optional())?;
        serde_json::to_value(crate::health_raw_sleep_projection::database_status(conn,authority.as_deref())?)
            .map_err(|_|"hc_sleep_projection_failed".into())
    })().unwrap_or_else(|_|json!({"status":"projection_deferred","retry_needed":true}));
    let mut source_import = Vec::<Value>::new();
    if scalar(conn, "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hc_raw_import_state'")? == 1 {
        // Source capabilities and last successful read differ from cloud delivery.
        // Tokens, record values and source identifiers never enter diagnostics.
        let mut query = sql(conn.prepare("SELECT record_type,phase,status,last_attempt_at,last_success_at,history_coverage,deletion_gap,needs_catchup FROM hc_raw_import_state ORDER BY record_type"))?;
        let rows = sql(query.query_map([], |r| Ok(json!({
            "type":r.get::<_,String>(0)?,"phase":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,
            "last_attempt_at":r.get::<_,Option<String>>(3)?,"last_success_at":r.get::<_,Option<String>>(4)?,
            "history_coverage":r.get::<_,String>(5)?,"deletion_gap":r.get::<_,bool>(6)?,"more_pending":r.get::<_,bool>(7)?
        }))))?;
        source_import = sql(rows.collect())?;
    }
    let exists = scalar(
        conn,
        "SELECT COUNT(*) FROM sqlite_master WHERE name='cloud_relay_state'",
    )?;
    if exists == 0 {
        return Ok(json!({"initializing":true,"source_import":source_import,"projection":projection}));
    }
    let (cursor, last_ok, error): (i64, Option<String>, Option<String>) = sql(conn.query_row(
        "SELECT receive_seq,last_ok,last_error FROM cloud_relay_state WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ))?;
    let pending = scalar(
        conn,
        "SELECT (SELECT COUNT(*) FROM cloud_relay_dirty)+(SELECT COUNT(*) FROM cloud_relay_outbox)",
    )?;
    let mut stmt=sql(conn.prepare("SELECT table_name,record_updated_at,received_at FROM cloud_relay_freshness ORDER BY table_name"))?;
    let freshness=sql(stmt.query_map([],|r|Ok(json!({"type":r.get::<_,String>(0)?,"record_updated_at":r.get::<_,String>(1)?,"received_at":r.get::<_,String>(2)?}))))?;
    let freshness: Vec<Value> = sql(freshness.collect())?;
    let mut stmt = sql(conn.prepare(
        "SELECT device_id,applied_seq,received_at FROM cloud_relay_receipts ORDER BY device_id",
    ))?;
    let receipts=sql(stmt.query_map([],|r|Ok(json!({"device_id":r.get::<_,String>(0)?,"applied_seq":r.get::<_,i64>(1)?,"received_at":r.get::<_,String>(2)?}))))?;
    let receipts: Vec<Value> = sql(receipts.collect())?;
    Ok(
        json!({"pending_keys":pending,"received_seq":cursor,"applied_seq":fragments::applied_cursor(conn)?,
        "unresolved_deletions":identity::unresolved_tomb_count(conn)?,
        "incomplete_parts":scalar(conn,"SELECT COUNT(*) FROM cloud_relay_fragments")?,
        "last_ok":last_ok,"error_code":error,"freshness":freshness,"source_import":source_import,"projection":projection,"device_receipts":receipts}),
    )
}

#[cfg(not(target_os = "android"))]
pub(crate) fn start_background_sync(app: &tauri::AppHandle) {
    crate::cloud_relay_runtime::start(app);
}

#[cfg(test)]
#[path = "cloud_relay_tests.rs"]
mod tests;
