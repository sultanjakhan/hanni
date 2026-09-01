// sync_github.rs — Tier 3 owner-sync orchestration over a private GitHub repo.
//
// Push batches all dirty rows + tombstones into ONE commit (Git Data API);
// pull diffs commits since the last cursor (a commit SHA) and applies them via
// the reused sync_owner merge layer (LWW / anti-resurrection). REST + codec
// helpers live in sync_github_api.rs. Each device writes only its own outbox
// subdir, so concurrent pushes never collide. Design:
// docs/architecture/firebase-off-tier3-github.md.

use crate::db::SYNC_TABLES;
use crate::sync_github_api::{
    blob_entry, build_doc, fetch_doc, fetch_tarball, gh_get, gh_head, gh_post, gh_req, resolve_gh,
};
use crate::sync_github_replay::prepare_text_id_replay;
use crate::sync_owner::{
    get_setting, row_to_json, set_setting, tombstone_row_id, upsert_row_fail_closed,
};
use crate::types::HanniDb;
use reqwest::Method;
use rusqlite::{types::Value as SqlValue, OptionalExtension};
use serde_json::{json, Map, Value};

const PUSH_LIMIT: usize = 500;
const EPOCH_TS: &str = "1970-01-01T00:00:00Z";

fn dirty_rows(conn: &rusqlite::Connection, table: &str, cursor: &str)
              -> Result<Vec<(SqlValue, String)>, String>
{
    let mut stmt = conn.prepare(&format!(
        "SELECT id, updated_at FROM {} WHERE updated_at > ?1 \
         ORDER BY updated_at ASC LIMIT {}", table, PUSH_LIMIT))
        .map_err(|e| format!("prep {}: {}", table, e))?;
    let rows = stmt.query_map(rusqlite::params![cursor], |row|
        Ok((row.get(0)?, row.get(1)?))
    ).map_err(|e| format!("dirty {}: {}", table, e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("dirty row {}: {}", table, e))?;
    Ok(rows)
}

fn row_label(table: &str, id: &SqlValue) -> Result<String, String> {
    let id = match id {
        SqlValue::Integer(value) => value.to_string(),
        SqlValue::Text(value) => value.clone(),
        other => return Err(format!("unsupported primary key for {}: {:?}", table, other)),
    };
    Ok(format!("row:{}_{}", table, id))
}

pub(crate) async fn gh_push(db: &HanniDb) -> Result<Value, String> {
    let c = resolve_gh(db)?;
    let replayed_text_tables = prepare_text_id_replay(&db.conn(), SYNC_TABLES)?;
    let mut entries: Vec<Value> = Vec::new();
    let mut cursors: Vec<(String, String)> = Vec::new();
    let mut pushed = 0usize;

    {
        let conn = db.conn();
        for table in SYNC_TABLES {
            let ckey = format!("cloud_owner_gh_push_{}", table);
            let cursor = get_setting(&conn, &ckey).unwrap_or_else(|| EPOCH_TS.into());
            let dirty = dirty_rows(&conn, table, &cursor)?;
            let mut max = cursor.clone();
            for (id, ts) in &dirty {
                if let Some(row) = row_to_json(&conn, table, id)? {
                    entries.push(blob_entry(&c, &row_label(table, id)?,
                                            &build_doc(&row, &c.device_id, ts, table))?);
                    if ts > &max { max = ts.clone(); }
                    pushed += 1;
                }
            }
            if max != cursor { cursors.push((ckey, max)); }
        }
        let tcur = get_setting(&conn, "cloud_owner_gh_push_tombstones")
            .unwrap_or_else(|| EPOCH_TS.into());
        let mut stmt = conn.prepare(
            "SELECT table_name, row_id, deleted_at FROM sync_tombstones \
             WHERE deleted_at > ?1 ORDER BY deleted_at ASC LIMIT 500")
            .map_err(|e| format!("prep tombstones: {}", e))?;
        // row_id is TEXT (holds both integer ids and UUID strings) — reading it
        // as i64 made rusqlite error out every row and filter_map silently
        // dropped them all, so tombstones never pushed.
        let tombs: Vec<(String, String, String)> = stmt
            .query_map(rusqlite::params![tcur], |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?)))
            .map_err(|e| format!("tombstones: {}", e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| format!("tombstone row: {}", e))?;
        drop(stmt);
        let mut tmax = tcur.clone();
        for (table, id, ts) in &tombs {
            let doc = build_doc(&json!({ "_target_table": table, "_row_id": id, "_deleted": true }),
                                &c.device_id, ts, "tombstones");
            entries.push(blob_entry(&c, &format!("tomb:{}_{}", table, id), &doc)?);
            if ts > &tmax { tmax = ts.clone(); }
            pushed += 1;
        }
        if tmax != tcur { cursors.push(("cloud_owner_gh_push_tombstones".into(), tmax)); }
    }

    if entries.is_empty() {
        return Ok(json!({ "pushed": 0, "replayed_text_tables": replayed_text_tables }));
    }

    let client = reqwest::Client::new();
    let (parent, base_tree) = gh_head(&client, &c).await?;
    let tree = gh_post(&client, &c, "git/trees",
        &json!({ "base_tree": base_tree, "tree": entries })).await?;
    let tree_sha = tree.get("sha").and_then(|v| v.as_str()).ok_or("no tree sha")?;
    let commit = gh_post(&client, &c, "git/commits", &json!({
        "message": format!("sync {} (+{})", c.device_id, pushed),
        "tree": tree_sha, "parents": [parent],
    })).await?;
    let commit_sha = commit.get("sha").and_then(|v| v.as_str())
        .ok_or("no commit sha")?.to_string();
    let (s, v) = gh_req(&client, &c, Method::PATCH, "git/refs/heads/main",
        Some(&json!({ "sha": commit_sha }))).await?;
    if !(200..300).contains(&s) { return Err(format!("update ref -> {}: {}", s, v)); }

    {
        let conn = db.conn();
        for (k, val) in &cursors { set_setting(&conn, k, val); }
        set_setting(&conn, "cloud_owner_gh_last_push_ts", &chrono::Utc::now().to_rfc3339());
    }
    Ok(json!({ "pushed": pushed, "commit": commit_sha,
               "replayed_text_tables": replayed_text_tables }))
}

pub(crate) async fn gh_pull(db: &HanniDb) -> Result<Value, String> {
    let c = resolve_gh(db)?;
    let client = reqwest::Client::new();
    let (head, _) = gh_head(&client, &c).await?;
    let cursor = get_setting(&db.conn(), "cloud_owner_gh_pull_sha");
    if cursor.as_deref() == Some(head.as_str()) { return Ok(json!({ "applied": 0 })); }

    let own_prefix = format!("{}/", c.device_id);
    let mut applied = 0u64;

    // `compare` returns at most 300 files on its first page. With no cursor
    // (first pull) or a truncated diff, read the whole repo in ONE tarball
    // instead of a per-blob GET storm that would exhaust the account's rate
    // limit and never let the cursor advance. Re-applying is LWW-idempotent.
    let incremental = match &cursor {
        Some(cur) => {
            let files = parse_compare(
                gh_get(&client, &c, &format!("compare/{}...{}", cur, head)).await?);
            if files.len() >= 300 { None } else { Some(files) }
        }
        None => None,
    };

    match incremental {
        Some(files) => {
            for (path, blob_sha) in &files {
                if path.starts_with(&own_prefix) || !path.contains('/') { continue; }
                let doc = fetch_doc(&client, &c, path, blob_sha).await
                    .map_err(|e| format!("GitHub fetch {path}: {e}"))?;
                if apply_doc(&db.conn(), &doc)
                    .map_err(|e| format!("GitHub apply {path}: {e}"))?
                {
                    applied += 1;
                }
            }
        }
        None => {
            for (path, doc) in fetch_tarball(&client, &c, &head).await? {
                if apply_doc(&db.conn(), &doc)
                    .map_err(|e| format!("GitHub apply {path}: {e}"))?
                {
                    applied += 1;
                }
            }
        }
    }

    {
        let conn = db.conn();
        save_pull_head(&conn, &head)?;
        set_setting(&conn, "cloud_owner_gh_last_pull_ts", &chrono::Utc::now().to_rfc3339());
    }
    Ok(json!({ "applied": applied }))
}

fn save_pull_head(conn: &rusqlite::Connection, head: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings(key,value) VALUES(?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params!["cloud_owner_gh_pull_sha", head],
    )
    .map(|_| ())
    .map_err(|e| format!("save GitHub pull head: {e}"))
}

fn apply_tombstone_lww(
    conn: &rusqlite::Connection,
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

    let row_id = tombstone_row_id(Some(id))
        .ok_or_else(|| format!("tombstone {target}: invalid _row_id"))?;
    let row_id_text = match &row_id {
        SqlValue::Integer(value) => value.to_string(),
        SqlValue::Text(value) => value.clone(),
        _ => return Err(format!("tombstone {target}: unsupported _row_id")),
    };

    let known_tombstone: Option<String> = conn.query_row(
        "SELECT deleted_at FROM sync_tombstones WHERE table_name=?1 AND row_id=?2",
        rusqlite::params![target, &row_id_text],
        |row| row.get(0),
    ).optional().map_err(|e| format!("read tombstone {target}: {e}"))?;

    let effective_deleted_at = match known_tombstone.as_deref() {
        Some(timestamp) if timestamp > deleted_at => timestamp,
        _ => deleted_at,
    };

    let local_updated: Option<String> = conn.query_row(
        &format!("SELECT updated_at FROM {target} WHERE id=?1"),
        rusqlite::params![&row_id],
        |row| row.get(0),
    ).optional().map_err(|e| format!("read local row {target}: {e}"))?;

    // Delete wins on equal timestamps; only a strictly newer local row survives.
    if local_updated.as_deref().is_some_and(|timestamp| timestamp > effective_deleted_at) {
        return Ok(false);
    }

    let deleted = conn.execute(
        &format!("DELETE FROM {target} WHERE id=?1"),
        rusqlite::params![&row_id],
    ).map_err(|e| format!("delete {target}: {e}"))?;

    // A local DELETE trigger can stamp local wall-clock time. Preserve the
    // remote logical timestamp so later stale rows cannot resurrect.
    let wrote_tombstone = known_tombstone.as_deref() != Some(effective_deleted_at);
    conn.execute(
        "INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES(?1,?2,?3) \
         ON CONFLICT(table_name,row_id) DO UPDATE SET deleted_at=excluded.deleted_at",
        rusqlite::params![target, &row_id_text, effective_deleted_at],
    )
    .map_err(|e| format!("persist tombstone {target}: {e}"))?;

    Ok(deleted > 0 || wrote_tombstone)
}

/// Apply one decrypted doc: tombstone delete or LWW row upsert via the merge
/// layer. Returns whether a row was changed.
fn apply_doc(conn: &rusqlite::Connection, doc: &Map<String, Value>) -> Result<bool, String> {
    let table = doc.get("_table").and_then(Value::as_str)
        .ok_or("remote document missing _table")?;
    if table == "tombstones" {
        let target = doc.get("_target_table").and_then(Value::as_str)
            .ok_or("tombstone missing _target_table")?;
        let id = doc.get("_row_id").ok_or("tombstone missing _row_id")?;
        let deleted_at = doc.get("_updated_at").and_then(Value::as_str)
            .filter(|timestamp| !timestamp.is_empty())
            .ok_or("tombstone missing _updated_at")?;
        return apply_tombstone_lww(conn, target, id, deleted_at);
    }

    if !SYNC_TABLES.contains(&table) {
        return Err(format!("remote document: unsupported table {table}"));
    }
    upsert_row_fail_closed(conn, table, doc)
}

/// Changed/added blob paths (path, blob_sha) from a `compare` response; git
/// deletions are ignored (our deletes are explicit tombstone blobs).
fn parse_compare(cmp: Value) -> Vec<(String, String)> {
    cmp.get("files").and_then(|f| f.as_array()).map(|arr| arr.iter().filter_map(|f| {
        if f.get("status")?.as_str()? == "removed" { return None; }
        Some((f.get("filename")?.as_str()?.to_string(), f.get("sha")?.as_str()?.to_string()))
    }).collect()).unwrap_or_default()
}

#[cfg(test)]
#[path = "sync_github_tests.rs"]
mod tests;
