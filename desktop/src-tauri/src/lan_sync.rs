// lan_sync.rs — direct device-to-device sync over the local network.
//
// When the phone and the Mac are on the same Wi-Fi they exchange SYNC_TABLES
// rows straight over HTTP — no cloud, no Firestore, no quota. A dedicated
// server bound to 0.0.0.0:8244 exposes ONLY the sync endpoint; /auto/eval
// (arbitrary JS = RCE) stays on the loopback-only server and is never exposed.
//
// One POST /lan/sync is a full bidirectional exchange: the caller sends its
// rows changed since its per-table cursors, the callee applies them and
// returns its own rows newer than those cursors. LWW (by `updated_at`) makes
// re-applying idempotent, so there is no cursor-skip race.

use crate::db::SYNC_TABLES;
use crate::sync_owner::{get_setting, row_to_json, set_setting, upsert_row};
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
struct TombItem { tt: String, id: i64 }

#[derive(Serialize, Deserialize)]
struct SyncReq {
    key: String,
    cursors: Map<String, Value>,
    tomb_cursor: String,
    rows: Vec<RowItem>,
    tombs: Vec<TombItem>,
}

#[derive(Serialize, Deserialize, Default)]
struct SyncBatch { rows: Vec<RowItem>, tombs: Vec<TombItem> }

fn cursor_of(cursors: &Map<String, Value>, table: &str) -> String {
    cursors.get(table).and_then(|v| v.as_str()).unwrap_or(EPOCH).to_string()
}

/// Collect rows + tombstones changed since the given per-table cursors.
fn gather(conn: &rusqlite::Connection, cursors: &Map<String, Value>, tomb_cursor: &str)
          -> SyncBatch
{
    let mut rows = Vec::new();
    for table in SYNC_TABLES {
        let since = cursor_of(cursors, table);
        let ids: Vec<i64> = (|| {
            let mut stmt = conn.prepare(&format!(
                "SELECT id FROM {} WHERE updated_at > ?1 ORDER BY updated_at LIMIT {}",
                table, BATCH_LIMIT))?;
            let v = stmt.query_map(rusqlite::params![since], |r| r.get(0))?
                .filter_map(Result::ok).collect();
            Ok::<_, rusqlite::Error>(v)
        })().unwrap_or_default();
        for id in ids {
            if let Ok(Some(Value::Object(mut f))) = row_to_json(conn, table, id) {
                // upsert_row reads `_updated_at` for the LWW comparison.
                if let Some(ua) = f.get("updated_at").cloned() {
                    f.insert("_updated_at".into(), ua);
                }
                rows.push(RowItem { t: (*table).into(), f });
            }
        }
    }
    let tombs: Vec<TombItem> = (|| {
        let mut stmt = conn.prepare(
            "SELECT table_name, row_id FROM sync_tombstones \
             WHERE deleted_at > ?1 ORDER BY deleted_at LIMIT 500")?;
        let v = stmt.query_map(rusqlite::params![tomb_cursor], |r|
            Ok(TombItem { tt: r.get(0)?, id: r.get(1)? }))?
            .filter_map(Result::ok).collect();
        Ok::<_, rusqlite::Error>(v)
    })().unwrap_or_default();
    SyncBatch { rows, tombs }
}

/// Apply a received batch. Table names are validated against SYNC_TABLES
/// before any SQL interpolation.
fn apply_batch(conn: &rusqlite::Connection, batch: &SyncBatch) -> usize {
    let mut applied = 0;
    for item in &batch.rows {
        if !SYNC_TABLES.contains(&item.t.as_str()) { continue; }
        if let Ok(true) = upsert_row(conn, &item.t, &item.f) { applied += 1; }
    }
    for t in &batch.tombs {
        if !SYNC_TABLES.contains(&t.tt.as_str()) { continue; }
        let _ = conn.execute(&format!("DELETE FROM {} WHERE id = ?1", t.tt),
                             rusqlite::params![t.id]);
    }
    applied
}

/// Advance lan_cursor_{table} past every row seen this round (sent + received)
/// so the next sync is incremental and doesn't echo rows back.
fn advance_cursors(conn: &rusqlite::Connection, batches: &[&SyncBatch]) {
    use std::collections::HashMap;
    let mut max: HashMap<&str, String> = HashMap::new();
    for b in batches {
        for r in &b.rows {
            if let Some(ua) = r.f.get("updated_at").and_then(|v| v.as_str()) {
                let e = max.entry(r.t.as_str()).or_default();
                if ua > e.as_str() { *e = ua.to_string(); }
            }
        }
    }
    for (table, ts) in max {
        let key = format!("lan_cursor_{}", table);
        let cur = get_setting(conn, &key).unwrap_or_default();
        if ts > cur { set_setting(conn, &key, &ts); }
    }
}

fn read_cursors(conn: &rusqlite::Connection) -> (Map<String, Value>, String) {
    let mut cursors = Map::new();
    for table in SYNC_TABLES {
        let c = get_setting(conn, &format!("lan_cursor_{}", table))
            .unwrap_or_else(|| EPOCH.into());
        cursors.insert((*table).into(), Value::String(c));
    }
    let tomb = get_setting(conn, "lan_cursor_tombstones").unwrap_or_else(|| EPOCH.into());
    (cursors, tomb)
}

// ── Config ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn lan_sync_get_config(db: State<'_, HanniDb>) -> Value {
    let conn = db.conn();
    json!({
        "peer": get_setting(&conn, "lan_sync_peer").unwrap_or_default(),
        "key":  get_setting(&conn, "lan_sync_key").unwrap_or_default(),
        "enabled": get_setting(&conn, "lan_sync_enabled").as_deref() == Some("true"),
        "port": LAN_PORT,
    })
}

#[tauri::command]
pub fn lan_sync_set_config(peer: String, key: String, enabled: bool,
                           db: State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    set_setting(&conn, "lan_sync_peer", peer.trim());
    set_setting(&conn, "lan_sync_key", key.trim());
    set_setting(&conn, "lan_sync_enabled", if enabled { "true" } else { "false" });
    Ok(())
}

// ── Client ───────────────────────────────────────────────────────────────

/// Run one bidirectional sync against the configured peer.
#[tauri::command]
pub async fn lan_sync_now(db: State<'_, HanniDb>) -> Result<Value, String> {
    let (peer, key, mine, cursors, tomb_cursor) = {
        let conn = db.conn();
        let peer = get_setting(&conn, "lan_sync_peer").unwrap_or_default();
        let key = get_setting(&conn, "lan_sync_key").unwrap_or_default();
        if peer.is_empty() { return Err("LAN peer not configured".into()); }
        let (cursors, tomb_cursor) = read_cursors(&conn);
        let mine = gather(&conn, &cursors, &tomb_cursor);
        (peer, key, mine, cursors, tomb_cursor)
    };

    let req = SyncReq {
        key,
        cursors,
        tomb_cursor,
        rows: mine.rows,
        tombs: mine.tombs,
    };
    let url = format!("http://{}/lan/sync", peer);
    let resp = reqwest::Client::new()
        .post(&url)
        .timeout(std::time::Duration::from_secs(20))
        .json(&req)
        .send().await.map_err(|e| format!("LAN peer unreachable: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("LAN sync {}: {}", resp.status(),
                            resp.text().await.unwrap_or_default()));
    }
    let theirs: SyncBatch = resp.json().await.map_err(|e| format!("bad response: {}", e))?;

    let conn = db.conn();
    let applied = apply_batch(&conn, &theirs);
    let mine_batch = SyncBatch { rows: req.rows, tombs: req.tombs };
    advance_cursors(&conn, &[&mine_batch, &theirs]);
    set_setting(&conn, "lan_cursor_tombstones", &chrono::Local::now().to_rfc3339());

    Ok(json!({ "sent": mine_batch.rows.len(), "received": applied,
               "deletes": theirs.tombs.len() }))
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
        let want = get_setting(&conn, "lan_sync_key").unwrap_or_default();
        if want.is_empty() || req.key != want {
            return Err((StatusCode::UNAUTHORIZED, "bad key".into()));
        }
        apply_batch(&conn, &SyncBatch { rows: req.rows, tombs: req.tombs });
        Ok(Json(gather(&conn, &req.cursors, &req.tomb_cursor)))
    }

    let router = Router::new()
        .route("/lan/sync", post(handle))
        .with_state(app);
    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", LAN_PORT)).await {
        Ok(l) => { let _ = axum::serve(l, router).await; }
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
