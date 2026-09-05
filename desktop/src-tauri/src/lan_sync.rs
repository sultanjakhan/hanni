// lan_sync.rs — direct device-to-device sync over the local network.
//
// When the phone and the Mac are on the same Wi-Fi they exchange SYNC_TABLES
// rows straight over HTTP — no cloud, no Firestore, no quota. A dedicated
// server bound to 0.0.0.0:8244 exposes ONLY the sync endpoint. The fixed
// debug reload action stays on the loopback-only dev server and arbitrary
// JavaScript execution is not exposed by any Hanni HTTP route.
//
// One POST /lan/sync is a full bidirectional exchange: the caller sends its
// rows changed since its per-table cursors, the callee applies them and
// returns its own rows newer than those cursors. LWW (by `updated_at`) makes
// re-applying idempotent. Push and pull advance independently after commit.

use crate::db::SYNC_TABLES;
use crate::sync_owner::{apply_tombstone_lww, get_setting, row_to_json,
    get_setting_checked, set_setting_checked, upsert_row_lan};
use crate::types::HanniDb;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager, State};

pub const LAN_PORT: u16 = 8244;
const BATCH_LIMIT: usize = 500;
const EPOCH: &str = "1970-01-01T00:00:00";

#[derive(Serialize, Deserialize)]
struct RowItem { t: String, f: Map<String, Value> }

#[derive(Serialize, Deserialize)]
struct TombItem {
    tt: String,
    id: Value,
    #[serde(default)]
    deleted_at: String,
}

#[derive(Serialize, Deserialize)]
struct SyncReq {
    key: String,
    cursors: Map<String, Value>,
    tomb_cursor: String,
    rows: Vec<RowItem>,
    tombs: Vec<TombItem>,
    #[serde(default)]
    push_only: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct SyncBatch {
    rows: Vec<RowItem>,
    tombs: Vec<TombItem>,
    /// Advisory server address. It must not silently change the destination
    /// while the client uses cursors that are not scoped to peer identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peer_hint: Option<String>,
}

/// On macOS/Linux: ask the Tailscale CLI for our advertised IPv4.
/// On Android the CLI doesn't ship; we just return None — the phone is a
/// *client* of LAN sync anyway, so it has nothing to hint with.
/// Cached after first call so we don't spawn `tailscale` per request.
fn detect_my_tailscale_addr() -> Option<String> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<String>> = OnceLock::new();
    CACHED.get_or_init(|| {
        #[cfg(target_os = "android")]
        { None }
        #[cfg(not(target_os = "android"))]
        {
            let candidates = [
                "/usr/local/bin/tailscale",
                "/opt/homebrew/bin/tailscale",
                "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
                "tailscale",
            ];
            for bin in candidates {
                let out = match std::process::Command::new(bin)
                    .args(["ip", "-4"]).output() {
                    Ok(o) if o.status.success() => o,
                    _ => continue,
                };
                let ip = String::from_utf8_lossy(&out.stdout)
                    .lines().next().unwrap_or("").trim().to_string();
                // Sanity: Tailscale CGNAT range is 100.64.0.0/10.
                if ip.starts_with("100.") {
                    return Some(format!("{}:{}", ip, LAN_PORT));
                }
            }
            None
        }
    }).clone()
}

fn cursor_of(cursors: &Map<String, Value>, table: &str) -> String {
    cursors.get(table).and_then(|v| v.as_str()).unwrap_or(EPOCH).to_string()
}

/// Collect rows + tombstones changed since the given per-table cursors.
fn gather(conn: &rusqlite::Connection, cursors: &Map<String, Value>, tomb_cursor: &str)
          -> Result<SyncBatch, String>
{
    let mut rows = Vec::new();
    for table in SYNC_TABLES {
        let since = cursor_of(cursors, table);
        let projection_filter = crate::health_raw_sleep_projection::transport_row_filter(conn, table)?;
        // `id` can be INTEGER (legacy AUTOINCREMENT tables) or TEXT (tables
        // migrated to UUIDv7 in Phase 1+). Read as raw SQLite Value so the
        // same code path serves both.
        let ids: Vec<rusqlite::types::Value> = (|| {
            let mut stmt = conn.prepare(&format!(
                "WITH page AS (SELECT updated_at FROM {0} WHERE updated_at > ?1 AND ({projection_filter})
                 ORDER BY updated_at LIMIT {1})
                 SELECT id FROM {0} WHERE updated_at > ?1 AND ({projection_filter})
                 AND updated_at <= (SELECT MAX(updated_at) FROM page)
                 ORDER BY updated_at, CAST(id AS TEXT)",
                table, BATCH_LIMIT))?;
            let v = stmt.query_map(rusqlite::params![since], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, rusqlite::Error>(v)
        })().map_err(|_| "LAN data read failed")?;
        for id in &ids {
            if let Some(Value::Object(mut f)) = row_to_json(conn, table, id)
                .map_err(|_| "LAN row read failed")? {
                // upsert_row reads `_updated_at` for the LWW comparison.
                if let Some(ua) = f.get("updated_at").cloned() {
                    f.insert("_updated_at".into(), ua);
                }
                rows.push(RowItem { t: (*table).into(), f });
            } else {
                return Err("LAN row disappeared during gather".into());
            }
        }
    }
    let projection_filter = crate::health_raw_sleep_projection::transport_tomb_filter(conn, "sync_tombstones.table_name", "sync_tombstones.row_id")?;
    let tombs: Vec<TombItem> = (|| {
        let mut stmt = conn.prepare(&format!(
            "WITH page AS (SELECT deleted_at FROM sync_tombstones WHERE deleted_at > ?1 AND ({projection_filter})
             ORDER BY deleted_at LIMIT 500)
             SELECT table_name, row_id, deleted_at FROM sync_tombstones
             WHERE deleted_at > ?1 AND ({projection_filter}) AND deleted_at <= (SELECT MAX(deleted_at) FROM page)
             ORDER BY deleted_at, table_name, CAST(row_id AS TEXT)"))?;
        let v = stmt.query_map(rusqlite::params![tomb_cursor], |r|
            Ok(TombItem {
                tt: r.get(0)?,
                id: Value::String(r.get::<_, String>(1)?),
                deleted_at: r.get(2)?,
            }))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok::<_, rusqlite::Error>(v)
    })().map_err(|_| "LAN tombstone read failed")?;
    Ok(SyncBatch { rows, tombs, peer_hint: None })
}

/// Apply a received batch. Table names are validated against SYNC_TABLES
/// before any SQL interpolation.
fn apply_batch(conn: &rusqlite::Connection, batch: &SyncBatch) -> Result<usize, String> {
    // SAVEPOINT composes with the client's outer apply + cursor transaction.
    conn.execute_batch("SAVEPOINT lan_apply").map_err(|_| "LAN transaction failed")?;
    let result = apply_batch_inner(conn, batch);
    if result.is_err() {
        conn.execute_batch("ROLLBACK TO lan_apply; RELEASE lan_apply")
            .map_err(|_| "LAN rollback failed")?;
        return result;
    }
    if conn.execute_batch("RELEASE lan_apply").is_err() {
        conn.execute_batch("ROLLBACK TO lan_apply; RELEASE lan_apply")
            .map_err(|_| "LAN rollback failed")?;
        return Err("LAN commit failed".into());
    }
    result
}

fn apply_batch_inner(conn: &rusqlite::Connection, batch: &SyncBatch) -> Result<usize, String> {
    let mut applied = 0;
    let mut touched_schedules = false;
    let mut touched_health = false;
    for item in &batch.rows {
        if !SYNC_TABLES.contains(&item.t.as_str()) { return Err("Unsupported LAN table".into()); }
        let stored_timestamp = item.f.get("updated_at").and_then(Value::as_str)
            .ok_or("LAN row missing timestamp")?;
        let version_timestamp = item.f.get("_updated_at").and_then(Value::as_str)
            .ok_or("LAN row missing version timestamp")?;
        let normalize = |timestamp: &str| crate::sync_owner::canonical_sync_timestamp(timestamp, "LAN row")
            .map_err(|_| "Invalid LAN row timestamp");
        if normalize(stored_timestamp)? != normalize(version_timestamp)? {
            return Err("LAN row timestamps disagree".into());
        }
        if item.t == "schedules" { touched_schedules = true; }
        if matches!(item.t.as_str(), "health_log" | "events" | "sleep_sessions") {
            touched_health = true;
        }
        if upsert_row_lan(conn, &item.t, &item.f).map_err(|_| "LAN row apply failed")? { applied += 1; }
    }
    for t in &batch.tombs {
        if !SYNC_TABLES.contains(&t.tt.as_str()) { return Err("Unsupported LAN tombstone table".into()); }
        if apply_tombstone_lww(conn, &t.tt, &t.id, &t.deleted_at)
            .map_err(|_| "LAN tombstone apply failed")? {
            applied += 1;
        }
    }
    // Phase 3 follow-up: if this batch added schedules from a peer, two
    // independently-migrated devices can end up with two rows for the same
    // logical schedule. Collapse them right after apply so the next round
    // doesn't echo the duplicates and the user never sees them in UI.
    if touched_schedules { crate::db::dedup_schedules_by_title(conn); }
    // Health data that arrives over LAN (the Android background worker pushes
    // walks/steps/sleep while the phone app is closed) must still auto-✓ the
    // linked schedules — the importer that normally does this only runs on the
    // importing device. Idempotent; cover today + yesterday for the midnight
    // batch (yesterday's totals land just after 00:00).
    if touched_health {
        let now = chrono::Local::now().to_rfc3339();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d").to_string();
        let _ = crate::calendar_health::auto_complete_from_health(conn, &today, &now);
        let _ = crate::calendar_health::auto_complete_from_health(conn, &yesterday, &now);
    }
    Ok(applied)
}

/// Push and pull high-water marks must never acknowledge each other.
fn advance_cursors(conn: &rusqlite::Connection, batch: &SyncBatch, prefix: &str) -> Result<(), String> {
    let mut maximum = std::collections::HashMap::<&str, &str>::new();
    for row in &batch.rows {
        let ts = row.f.get("updated_at").and_then(Value::as_str)
            .ok_or("LAN row missing timestamp")?;
        let current = maximum.entry(row.t.as_str()).or_default();
        if ts > *current { *current = ts; }
    }
    for (table, ts) in maximum {
        if ts > read_cursor(conn, prefix, table)?.as_str() {
            set_setting_checked(conn, &format!("{prefix}{table}"), ts)?;
        }
    }
    if let Some(ts) = batch.tombs.iter().map(|t| t.deleted_at.as_str()).max() {
        if ts > read_cursor(conn, prefix, "tombstones")?.as_str() {
            set_setting_checked(conn, &format!("{prefix}tombstones"), ts)?;
        }
    }
    Ok(())
}

fn read_cursor(conn: &rusqlite::Connection, prefix: &str, table: &str) -> Result<String, String> {
    if let Some(value) = get_setting_checked(conn, &format!("{prefix}{table}"))? {
        return Ok(value);
    }
    // Preserve old progress; historical reconciliation must be explicit.
    let mut legacy = EPOCH.to_string();
    for old in ["lan_cursor_", "background_lan_cursor_", "health_worker_push_cursor_"] {
        if let Some(value) = get_setting_checked(conn, &format!("{old}{table}"))? {
            if value > legacy { legacy = value; }
        }
    }
    Ok(legacy)
}

fn read_cursors(conn: &rusqlite::Connection, prefix: &str) -> Result<(Map<String, Value>, String), String> {
    let mut cursors = Map::new();
    for table in SYNC_TABLES {
        cursors.insert((*table).into(), Value::String(read_cursor(conn, prefix, table)?));
    }
    Ok((cursors, read_cursor(conn, prefix, "tombstones")?))
}

#[tauri::command]
pub fn lan_sync_get_config(db: State<'_, HanniDb>) -> Result<Value, String> {
    let conn = db.conn();
    let key_set = crate::sync_owner::get_setting_checked(&conn, "lan_sync_key")?
        .is_some_and(|key| !key.is_empty());
    Ok(json!({
        "peer": get_setting(&conn, "lan_sync_peer").unwrap_or_default(),
        "key_set": key_set,
        "enabled": get_setting(&conn, "lan_sync_enabled").as_deref() == Some("true"),
        "port": LAN_PORT,
    }))
}

#[tauri::command]
pub fn lan_sync_set_config(peer: String, key: Option<String>, clear_key: Option<bool>, enabled: bool,
                           db: State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    crate::sync_owner::set_setting_checked(&conn, "lan_sync_peer", peer.trim())?;
    if clear_key.unwrap_or(false) {
        crate::sync_owner::set_setting_checked(&conn, "lan_sync_key", "")?;
    } else if let Some(key) = key.filter(|value| !value.trim().is_empty()) {
        crate::sync_owner::set_setting_checked(&conn, "lan_sync_key", key.trim())?;
    }
    crate::sync_owner::set_setting_checked(
        &conn,
        "lan_sync_enabled",
        if enabled { "true" } else { "false" },
    )?;
    Ok(())
}

// ── Client ───────────────────────────────────────────────────────────────

/// Run one bidirectional sync against the configured peer.
#[tauri::command]
pub async fn lan_sync_now(db: State<'_, HanniDb>) -> Result<Value, String> {
    let (peer, key, mine, push_cursors, push_tomb, cursors, tomb_cursor) = {
        let conn = db.conn();
        // Kotlin workers use another SQLite connection. Hold one DB snapshot
        // across selecting the page and serializing its rows, not just a mutex.
        let tx = conn.unchecked_transaction().map_err(|_| "LAN snapshot failed")?;
        let peer = get_setting_checked(&tx, "lan_sync_peer")?.unwrap_or_default();
        let key = get_setting_checked(&tx, "lan_sync_key")?
            .unwrap_or_default();
        if peer.is_empty() { return Err("LAN peer not configured".into()); }
        let (push_cursors, push_tomb) = read_cursors(&tx, "lan_push_cursor_")?;
        let (cursors, tomb_cursor) = read_cursors(&tx, "lan_pull_cursor_")?;
        let mine = gather(&tx, &push_cursors, &push_tomb)?;
        tx.commit().map_err(|_| "LAN snapshot failed")?;
        (peer, key, mine, push_cursors, push_tomb, cursors, tomb_cursor)
    };

    let req = SyncReq {
        key,
        cursors,
        tomb_cursor,
        rows: mine.rows,
        tombs: mine.tombs,
        push_only: false,
    };
    let url = format!("http://{}/lan/sync", peer);
    // Tailscale's direct path goes cold when idle, so the first connect can
    // hang until the timeout. A short connect_timeout fails fast (~5s instead
    // of the full 20s) and a bounded retry warms the path so the exchange
    // succeeds within the same loop tick instead of erroring for ~36s. Only
    // retry connect/timeout errors — an HTTP error status means the peer was
    // reached, so retrying would be pointless.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("LAN client build: {}", e))?;
    let mut resp = None;
    let mut last_err = String::new();
    for attempt in 0..3 {
        match client.post(&url).json(&req).send().await {
            Ok(r) => { resp = Some(r); break; }
            Err(e) => {
                last_err = if e.is_timeout() { "timeout" } else { "connection failed" }.into();
                if !(e.is_connect() || e.is_timeout()) { break; }
                if attempt < 2 {
                    let backoff = 800u64 * (attempt as u64 + 1);
                    tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                }
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("LAN peer unreachable: {}", last_err))?;
    if !resp.status().is_success() {
        return Err(format!("LAN sync HTTP {}", resp.status()));
    }
    let theirs: SyncBatch = resp.json().await.map_err(|_| "Invalid LAN response")?;

    // Apply received batch + advance cursors + record sync time. Done in
    // a scope so the rusqlite Connection (which is !Send) is dropped
    // before the next `.await`.
    let sent_count = req.rows.len();
    let applied;
    {
        let conn = db.conn();
        let tx = conn.unchecked_transaction().map_err(|_| "LAN transaction failed")?;
        if get_setting_checked(&tx, "lan_sync_peer")?.unwrap_or_default() != peer
            || read_cursors(&tx, "lan_push_cursor_")? != (push_cursors, push_tomb)
            || read_cursors(&tx, "lan_pull_cursor_")? != (req.cursors.clone(), req.tomb_cursor.clone()) {
            return Err("LAN state changed during exchange; retry required".into());
        }
        applied = apply_batch(&tx, &theirs)?;
        let mine_batch = SyncBatch { rows: req.rows, tombs: req.tombs, peer_hint: None };
        advance_cursors(&tx, &mine_batch, "lan_push_cursor_")?;
        advance_cursors(&tx, &theirs, "lan_pull_cursor_")?;
        tx.commit().map_err(|_| "LAN commit failed")?;
    }

    // A hint is not proof that another endpoint holds this peer's history.
    // Keep the configured destination stable; cursors are not peer-scoped yet.
    Ok(json!({ "sent": sent_count, "received": applied,
               "deletes": theirs.tombs.len(),
               "peer_adopted": Option::<String>::None }))
}

// ── Server (0.0.0.0:8244, sync endpoint only) ────────────────────────────

pub async fn spawn_lan_sync_server(app: AppHandle) {
    use axum::{Router, routing::post, extract::State as AxumState, Json,
               http::StatusCode};

    async fn handle(
        AxumState(app): AxumState<AppHandle>,
        Json(req): Json<SyncReq>,
    ) -> Result<Json<SyncBatch>, (StatusCode, String)> {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let want = crate::sync_owner::get_setting_checked(&conn, "lan_sync_key")
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "secret store unavailable".into()))?
            .unwrap_or_default();
        if want.is_empty() || req.key != want {
            return Err((StatusCode::UNAUTHORIZED, "bad key".into()));
        }
        let tx = conn.unchecked_transaction()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "LAN transaction failed".into()))?;
        apply_batch(&tx, &SyncBatch { rows: req.rows, tombs: req.tombs, peer_hint: None })
            .map_err(|_| (StatusCode::CONFLICT, "LAN batch was not applied".into()))?;
        if req.push_only {
            tx.commit().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "LAN commit failed".into()))?;
            return Ok(Json(SyncBatch { rows: vec![], tombs: vec![], peer_hint: detect_my_tailscale_addr() }));
        }
        let mut batch = gather(&tx, &req.cursors, &req.tomb_cursor)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "LAN data read failed".into()))?;
        tx.commit().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "LAN commit failed".into()))?;
        batch.peer_hint = detect_my_tailscale_addr();
        Ok(Json(batch))
    }

    let router = Router::new()
        .route("/lan/sync", post(handle))
        .with_state(app);
    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", LAN_PORT)).await {
        Ok(l) => {
            eprintln!("[lan_sync] server on 0.0.0.0:{}", LAN_PORT);
            let r = axum::serve(l, router).await;
            eprintln!("[lan_sync] serve exited: {:?}", r);
        }
        Err(e) => eprintln!("[lan_sync] bind {} failed: {}", LAN_PORT, e),
    }
}

// ── Auto loop ────────────────────────────────────────────────────────────

pub fn start_lan_sync_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            let enabled = {
                let db = app.state::<HanniDb>();
                let conn = db.conn();
                get_setting(&conn, "lan_sync_enabled").as_deref() == Some("true")
                    && !get_setting(&conn, "lan_sync_peer").unwrap_or_default().is_empty()
            };
            if !enabled { continue; }
            let db = app.state::<HanniDb>();
            if let Err(e) = lan_sync_now(db).await {
                eprintln!("[lan_sync] auto: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_settings VALUES('device_id','lan-local');
             CREATE TABLE notes(id INTEGER PRIMARY KEY,title TEXT NOT NULL,updated_at TEXT NOT NULL);
             CREATE TABLE sync_tombstones(
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 table_name TEXT NOT NULL,row_id TEXT NOT NULL,deleted_at TEXT NOT NULL,
                 UNIQUE(table_name,row_id));
             CREATE TABLE sync_row_versions(
                 table_name TEXT NOT NULL,row_id TEXT NOT NULL,updated_at TEXT NOT NULL,
                 device_id TEXT NOT NULL,PRIMARY KEY(table_name,row_id));",
        )
        .unwrap();
        crate::db::migrate_sync_meta(&conn).unwrap();
        conn
    }

    fn gather_conn() -> rusqlite::Connection {
        let conn = sync_conn();
        for table in SYNC_TABLES {
            if *table != "notes" {
                conn.execute_batch(&format!(
                    "CREATE TABLE IF NOT EXISTS {table}(id TEXT PRIMARY KEY,updated_at TEXT NOT NULL);"
                )).unwrap();
            }
        }
        conn
    }

    fn seed_notes(conn: &rusqlite::Connection, count: i64, tied: bool) {
        conn.execute_batch("UPDATE sync_apply_context SET remote_apply=1").unwrap();
        for id in 1..=count {
            let timestamp = if tied { "2026-09-01T00:00:00Z".to_string() } else {
                format!("2026-09-01T00:{:02}:{:02}Z", (id - 1) / 60, (id - 1) % 60)
            };
            conn.execute("INSERT INTO notes(id,title,updated_at) VALUES(?1,'synthetic',?2)",
                rusqlite::params![id, timestamp]).unwrap();
        }
        conn.execute_batch("UPDATE sync_apply_context SET remote_apply=0").unwrap();
    }

    #[test]
    fn timestamp_boundary_keeps_every_row_and_tombstone() {
        let conn = gather_conn();
        seed_notes(&conn, 501, true);
        for id in 1..=501 {
            conn.execute("INSERT INTO sync_tombstones(table_name,row_id,deleted_at)
                VALUES('notes',?1,'2026-09-02T00:00:00Z')", [id.to_string()]).unwrap();
        }
        let batch = gather(&conn, &Map::new(), EPOCH).unwrap();
        assert_eq!(batch.rows.len(), 501);
        assert_eq!(batch.tombs.len(), 501);
        advance_cursors(&conn, &batch, "lan_push_cursor_").unwrap();
        let (cursors, tomb) = read_cursors(&conn, "lan_push_cursor_").unwrap();
        let next = gather(&conn, &cursors, &tomb).unwrap();
        assert!(next.rows.is_empty() && next.tombs.is_empty());
    }

    #[test]
    fn newer_inbound_timestamp_does_not_skip_unsent_local_page() {
        let conn = gather_conn();
        seed_notes(&conn, 601, false);
        let mine = gather(&conn, &Map::new(), EPOCH).unwrap();
        assert_eq!(mine.rows.len(), 500);
        let theirs = row("2099-01-01T00:00:00Z");
        apply_batch(&conn, &theirs).unwrap();
        advance_cursors(&conn, &mine, "lan_push_cursor_").unwrap();
        advance_cursors(&conn, &theirs, "lan_pull_cursor_").unwrap();
        let (push, tomb) = read_cursors(&conn, "lan_push_cursor_").unwrap();
        let remaining = gather(&conn, &push, &tomb).unwrap();
        for id in 501..=601 {
            assert!(remaining.rows.iter().any(|row| row.t == "notes" && row.f["id"] == json!(id)));
        }
    }

    #[test]
    fn malformed_later_row_rolls_back_the_entire_batch() {
        let conn = sync_conn();
        let mut batch = row("2099-01-01T00:00:00Z");
        let invalid = json!({"id":8,"title":null,"updated_at":"2099-01-01T00:00:00Z",
            "_updated_at":"2099-01-01T00:00:00Z"});
        batch.rows.push(RowItem { t: "notes".into(), f: invalid.as_object().unwrap().clone() });
        assert!(apply_batch(&conn, &batch).is_err());
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        assert_eq!(conn.query_row("SELECT remote_apply FROM sync_apply_context", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn cursor_timestamp_must_match_the_applied_row_version() {
        let conn = sync_conn();
        let mut batch = row("2026-09-01T00:00:00Z");
        batch.rows[0].f.insert("updated_at".into(), json!("2099-01-01T00:00:00Z"));
        assert!(apply_batch(&conn, &batch).is_err());
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn missing_source_table_is_an_error_not_an_empty_success() {
        let conn = sync_conn();
        assert!(gather(&conn, &Map::new(), EPOCH).is_err());
    }

    #[test]
    fn migrated_direction_does_not_follow_legacy_cursor_again() {
        let conn = sync_conn();
        set_setting_checked(&conn, "lan_cursor_notes", "2026-09-01T00:00:00Z").unwrap();
        assert_eq!(read_cursor(&conn, "lan_push_cursor_", "notes").unwrap(), "2026-09-01T00:00:00Z");
        advance_cursors(&conn, &row("2026-09-02T00:00:00Z"), "lan_push_cursor_").unwrap();
        set_setting_checked(&conn, "lan_cursor_notes", "2099-01-01T00:00:00Z").unwrap();
        assert_eq!(read_cursor(&conn, "lan_push_cursor_", "notes").unwrap(), "2026-09-02T00:00:00Z");
    }

    fn tombstone(timestamp: &str) -> SyncBatch {
        SyncBatch {
            rows: Vec::new(),
            tombs: vec![TombItem {
                tt: "notes".into(),
                id: json!(7),
                deleted_at: timestamp.into(),
            }],
            peer_hint: None,
        }
    }

    fn row(timestamp: &str) -> SyncBatch {
        SyncBatch {
            rows: vec![RowItem {
                t: "notes".into(),
                f: json!({
                    "id": 7,
                    "title": "remote",
                    "updated_at": timestamp,
                    "_updated_at": timestamp
                })
                .as_object()
                .unwrap()
                .clone(),
            }],
            tombs: Vec::new(),
            peer_hint: None,
        }
    }

    #[test]
    fn lan_future_row_preserves_timestamp_and_observes_global_hlc() {
        let conn = sync_conn();
        let remote = "2099-01-01T00:00:00.000000123Z";
        assert_eq!(apply_batch(&conn, &row(remote)).unwrap(), 1);
        assert_eq!(
            conn.query_row("SELECT updated_at FROM notes WHERE id=7", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            crate::sync_owner::canonical_sync_timestamp(remote, "test").unwrap()
        );

        conn.execute(
            "INSERT INTO notes(id,title,updated_at) VALUES(8,'local','2020-01-01')",
            [],
        )
        .unwrap();
        let local: String = conn
            .query_row("SELECT updated_at FROM notes WHERE id=8", [], |row| row.get(0))
            .unwrap();
        assert!(
            crate::sync_owner::canonical_sync_timestamp(&local, "test").unwrap()
                > crate::sync_owner::canonical_sync_timestamp(remote, "test").unwrap()
        );
    }

    #[test]
    fn lan_future_tombstone_without_row_observes_global_hlc() {
        let conn = sync_conn();
        let remote = "2099-01-01T00:00:00Z";
        assert_eq!(apply_batch(&conn, &tombstone(remote)).unwrap(), 1);
        assert_eq!(
            conn.query_row(
                "SELECT deleted_at FROM sync_tombstones
                 WHERE table_name='notes' AND row_id='7'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            crate::sync_owner::canonical_sync_timestamp(remote, "test").unwrap()
        );

        conn.execute(
            "INSERT INTO notes(id,title,updated_at) VALUES(8,'local','2020-01-01')",
            [],
        )
        .unwrap();
        let local: String = conn
            .query_row("SELECT updated_at FROM notes WHERE id=8", [], |row| row.get(0))
            .unwrap();
        assert!(
            crate::sync_owner::canonical_sync_timestamp(&local, "test").unwrap()
                > crate::sync_owner::canonical_sync_timestamp(remote, "test").unwrap()
        );
    }

    #[test]
    fn lan_older_tombstone_cannot_replace_newer_known_tombstone() {
        let conn = sync_conn();
        let newer = "2099-01-02T00:00:00Z";
        let older = "2099-01-01T00:00:00Z";
        assert_eq!(apply_batch(&conn, &tombstone(newer)).unwrap(), 1);
        assert_eq!(apply_batch(&conn, &tombstone(older)).unwrap(), 0);
        assert_eq!(
            conn.query_row(
                "SELECT deleted_at FROM sync_tombstones
                 WHERE table_name='notes' AND row_id='7'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            crate::sync_owner::canonical_sync_timestamp(newer, "test").unwrap()
        );
    }
}
