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
use crate::sync_github_replay::prepare_cursor_v2_replay;
use crate::sync_owner::{
    apply_tombstone_lww, dirty_rows_after, dirty_tombstones_after, get_setting_checked,
    load_row_cursor, load_tombstone_cursor, row_to_json, save_row_cursor, save_tombstone_cursor,
    set_setting_checked, upsert_row_fail_closed, RowCursor, TombstoneCursor,
};
use crate::types::HanniDb;
use reqwest::Method;
use rusqlite::{types::Value as SqlValue, Connection, TransactionBehavior};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const PUSH_LIMIT: usize = 500;
const EPOCH_TS: &str = "1970-01-01T00:00:00Z";

fn github_sync_scope(c: &crate::sync_github_api::GhCreds) -> String {
    let mut hasher = Sha256::new();
    hasher.update(c.repo.as_bytes());
    hasher.update([0]);
    hasher.update(c.key);
    hasher.update([0]);
    hasher.update(c.device_id.as_bytes());
    hex::encode(hasher.finalize())
}

fn dirty_rows(conn: &rusqlite::Connection, table: &str, cursor: &str,
)
              -> Result<Vec<(SqlValue, String)>, String>
{
    dirty_rows_after(
        conn, table,
        &RowCursor {
            timestamp: cursor.to_string(),
            id: None,
        },
        PUSH_LIMIT,
    )
}

fn row_label(table: &str, id: &SqlValue) -> Result<String, String> {
    let id = match id {
        SqlValue::Integer(value) => value.to_string(),
        SqlValue::Text(value) => value.clone(),
        other => {
            return Err(format!("unsupported primary key for {}: {:?}", table, other))
        }
    };
    Ok(format!("row:{}_{}", table, id))
}

pub(crate) async fn gh_push(db: &HanniDb) -> Result<Value, String> {
    let c = resolve_gh(db)?;
    let scope = github_sync_scope(&c);
    let replayed_cursor_tables = prepare_cursor_v2_replay(&db.conn(), SYNC_TABLES, &scope)?;
    let mut entries: Vec<Value> = Vec::new();
    let mut row_cursor_updates: Vec<(String, RowCursor, RowCursor)> = Vec::new();
    let mut tombstone_cursor_update: Option<(String, TombstoneCursor, TombstoneCursor)> = None;
    let mut pushed = 0usize;

    {
        let conn = db.conn();
        for table in SYNC_TABLES {
            let ckey = format!("cloud_owner_gh_push_{}", table);
            let cursor = load_row_cursor(&conn, &ckey)?;
            let dirty = dirty_rows_after(&conn, table, &cursor, PUSH_LIMIT)?;
            for (id, ts) in &dirty {
                let row = row_to_json(&conn, table, id)?
                    .ok_or_else(|| format!("dirty row disappeared from {table}"))?;
                entries.push(blob_entry(&c, &row_label(table, id)?,
                                            &build_doc(&row, &c.device_id, ts, table),
                )?);
                pushed += 1;
                }
            if let Some((id, timestamp)) = dirty.last() {
                row_cursor_updates.push((
                    ckey,
                    cursor,
                    RowCursor {
                        timestamp: timestamp.clone(),
                        id: Some(id.clone()),
                    },
                ));
            }
        }
        let cursor_key = "cloud_owner_gh_push_tombstones".to_string();
        let tcur = load_tombstone_cursor(&conn, &cursor_key)?;
        let tombs = dirty_tombstones_after(&conn, &tcur, PUSH_LIMIT)?;
        for (table, id, ts) in &tombs {
            let doc = build_doc(&json!({ "_target_table": table, "_row_id": id, "_deleted": true }),
                                &c.device_id, ts, "tombstones",
            );
            entries.push(blob_entry(&c, &format!("tomb:{}_{}", table, id), &doc)?);
            pushed += 1;
        }
        if let Some((table, row_id, timestamp)) = tombs.last() {
            tombstone_cursor_update = Some((
                cursor_key,
                tcur,
                TombstoneCursor {
                    timestamp: timestamp.clone(),
                    table: Some(table.clone()),
                    row_id: Some(row_id.clone()),
                },
            )); }
    }

    if entries.is_empty() {
        return Ok(json!({ "pushed": 0, "replayed_cursor_tables": replayed_cursor_tables
        }));
    }

    let client = reqwest::Client::new();
    let (parent, base_tree) = gh_head(&client, &c).await?;
    let tree = gh_post(&client, &c, "git/trees",
        &json!({ "base_tree": base_tree, "tree": entries }),
    ).await?;
    let tree_sha = tree.get("sha").and_then(|v| v.as_str()).ok_or("no tree sha")?;
    let commit = gh_post(&client, &c, "git/commits", &json!({
        "message": format!("sync {} (+{})", c.device_id, pushed),
        "tree": tree_sha, "parents": [parent],
    }),
    ).await?;
    let commit_sha = commit.get("sha").and_then(|v| v.as_str())
        .ok_or("no commit sha")?.to_string();
    let (s, v) = gh_req(&client, &c, Method::PATCH, "git/refs/heads/main",
        Some(&json!({ "sha": commit_sha })),
    ).await?;
    if !(200..300).contains(&s) { return Err(format!("update ref -> {}: {}", s, v)); }

    {
        let mut conn = db.conn();
        let transaction = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("start GitHub push cursor transaction: {error}"))?;
        for (key, original, next) in &row_cursor_updates {
            if load_row_cursor(&transaction, key)? != *original {
                return Err(format!("GitHub push cursor changed during upload: {key}"));
            }
            save_row_cursor(&transaction, key, next)?;
        }
        if let Some((key, original, next)) = &tombstone_cursor_update {
            if load_tombstone_cursor(&transaction, key)? != *original {
                return Err("GitHub tombstone cursor changed during upload".into());
            }
            save_tombstone_cursor(&transaction, key, next)?;
        }
        set_setting_checked(
            &transaction, "cloud_owner_gh_last_push_ts", &chrono::Utc::now().to_rfc3339(),
        )?;
        transaction
            .commit()
            .map_err(|error| format!("commit GitHub push cursors: {error}"))?;
    }
    Ok(json!({ "pushed": pushed, "commit": commit_sha,
               "replayed_cursor_tables": replayed_cursor_tables }))
}

pub(crate) async fn gh_pull(db: &HanniDb) -> Result<Value, String> {
    let c = resolve_gh(db)?;
    let scope = github_sync_scope(&c);
    let replayed_cursor_tables = prepare_cursor_v2_replay(&db.conn(), SYNC_TABLES, &scope)?;
    let client = reqwest::Client::new();
    let (head, _) = gh_head(&client, &c).await?;
    let cursor = get_setting_checked(&db.conn(), "cloud_owner_gh_pull_sha")?;
    if cursor.as_deref() == Some(head.as_str()) { return Ok(json!({ "applied": 0,
            "replayed_cursor_tables": replayed_cursor_tables
        })); }

    let own_prefix = format!("{}/", c.device_id);

    // `compare` returns at most 300 files on its first page. With no cursor
    // (first pull) or a truncated diff, read the whole repo in ONE tarball
    // instead of a per-blob GET storm that would exhaust the account's rate
    // limit and never let the cursor advance. Re-applying is LWW-idempotent.
    let incremental = match &cursor {
        Some(cur) => {
            match parse_compare(gh_get(&client, &c, &format!("compare/{}...{}", cur, head)).await?)?
            {
                ComparePlan::Incremental(files) => Some(files),
                ComparePlan::FullSnapshot => None,
    }
        }
        None => None,
    };

    let documents = match incremental {
        Some(files) => {
            let mut documents = Vec::new();
            for (path, blob_sha) in &files {
                if path.starts_with(&own_prefix) || !path.contains('/') { continue; }
                let doc = fetch_doc(&client, &c, path, blob_sha).await
                    .map_err(|e| format!("GitHub fetch {path}: {e}"))?;
                documents.push((path.clone(), doc));
            }
            documents
        }
        None => fetch_tarball(&client, &c, &head).await?,
    };

    let applied = apply_github_documents(
        &mut db.conn(), &cursor, &head,
        &documents, &chrono::Utc::now().to_rfc3339(),
    )?;
    Ok(json!({ "applied": applied,
        "replayed_cursor_tables": replayed_cursor_tables
    }))
}

fn apply_github_documents(conn: &mut Connection,
    requested_cursor: &Option<String>,
    head: &str,
    documents: &[(String, Map<String, Value>)],
    pulled_at: &str,
) -> Result<u64, String> {
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("start GitHub pull transaction: {error}"))?;
    if get_setting_checked(&transaction, "cloud_owner_gh_pull_sha")? != *requested_cursor {
        return Err("GitHub pull cursor changed during fetch".into());
    }
    let mut applied = 0u64;
    for (path, document) in documents {
        if apply_doc(&transaction, document)
            .map_err(|error| format!("GitHub apply {path}: {error}"))?
        {
            applied += 1;
        }
    }
    save_pull_head(&transaction, head)?;
    set_setting_checked(&transaction, "cloud_owner_gh_last_pull_ts", pulled_at)?;
    transaction
        .commit()
        .map_err(|error| format!("commit GitHub pull: {error}"))?;
    Ok(applied)
}

fn save_pull_head(conn: &rusqlite::Connection, head: &str) -> Result<(), String> {
    set_setting_checked(conn, "cloud_owner_gh_pull_sha", head).map_err(|error| format!("save GitHub pull head: {error}"))
}

/// Apply one decrypted doc: tombstone delete or LWW row upsert via the merge
/// layer. Returns whether a row was changed.
fn apply_doc(conn: &rusqlite::Connection, doc: &Map<String, Value>) -> Result<bool, String> {
    let table = doc.get("_table").and_then(Value::as_str)
        .ok_or("remote document missing _table")?;
    if table == "tombstones" {
        if doc.get("_deleted").and_then(Value::as_bool) != Some(true) {
            return Err("tombstone missing _deleted=true".into());
        }
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

#[derive(Debug, PartialEq)]
enum ComparePlan {
    Incremental(Vec<(String, String)>),
    FullSnapshot,
}

/// Strictly decode changed/added blobs from GitHub compare. The API caps the
/// first page at 300 files, so the raw count selects a full tarball before any
/// removed entries are filtered. Malformed responses never permit HEAD save.
fn parse_compare(cmp: Value) -> Result<ComparePlan, String> {
    let status = cmp
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "GitHub compare response is missing status".to_string())?;
    if matches!(status, "behind" | "diverged") {
        return Ok(ComparePlan::FullSnapshot);
    }
    if !matches!(status, "ahead" | "identical") {
        return Err("GitHub compare response has an unsupported status".into());
    }
    let files = cmp
        .get("files").and_then(Value::as_array)
        .ok_or_else(|| "GitHub compare response is missing files".to_string())?;
    if files.len() >= 300 {
        return Ok(ComparePlan::FullSnapshot);
    }

    let mut changed = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let filename = file
            .get("filename")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("GitHub compare file {index} is missing filename"))?;
        let status = file
            .get("status")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("GitHub compare file {index} is missing status"))?;
        if !matches!(
            status,
            "added" | "removed" | "modified" | "renamed" | "copied" | "changed" | "unchanged"
        ) {
            return Err(format!(
                "GitHub compare file {index} has an unsupported status"
            ));
        }
        if status == "removed" {
            continue;
        }
        let sha = file
            .get("sha")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("GitHub compare file {index} is missing sha"))?;
        changed.push((filename.to_string(), sha.to_string()));
    }
    Ok(ComparePlan::Incremental(changed))
}

#[cfg(test)]
#[path = "sync_github_tests.rs"]
mod tests;
