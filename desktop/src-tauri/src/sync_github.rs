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
    blob_entry, build_doc, fetch_doc, gh_get, gh_head, gh_post, gh_req, resolve_gh, GhCreds,
};
use crate::sync_owner::{get_setting, row_to_json, set_setting, upsert_row};
use crate::types::HanniDb;
use reqwest::Method;
use serde_json::{json, Value};

const PUSH_LIMIT: usize = 500;
const EPOCH_TS: &str = "1970-01-01T00:00:00Z";

pub(crate) async fn gh_push(db: &HanniDb) -> Result<Value, String> {
    let c = resolve_gh(db)?;
    let mut entries: Vec<Value> = Vec::new();
    let mut cursors: Vec<(String, String)> = Vec::new();
    let mut pushed = 0usize;

    {
        let conn = db.conn();
        for table in SYNC_TABLES {
            let ckey = format!("cloud_owner_gh_push_{}", table);
            let cursor = get_setting(&conn, &ckey).unwrap_or_else(|| EPOCH_TS.into());
            let mut stmt = conn.prepare(&format!(
                "SELECT id, updated_at FROM {} WHERE updated_at > ?1 \
                 ORDER BY updated_at ASC LIMIT {}", table, PUSH_LIMIT))
                .map_err(|e| format!("prep {}: {}", table, e))?;
            let dirty: Vec<(i64, String)> = stmt
                .query_map(rusqlite::params![cursor], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|e| format!("dirty {}: {}", table, e))?
                .filter_map(Result::ok).collect();
            drop(stmt);
            let mut max = cursor.clone();
            for (id, ts) in &dirty {
                let idv = rusqlite::types::Value::Integer(*id);
                if let Some(row) = row_to_json(&conn, table, &idv)? {
                    entries.push(blob_entry(&c, &format!("row:{}_{}", table, id),
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
        let tombs: Vec<(String, i64, String)> = stmt
            .query_map(rusqlite::params![tcur], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| format!("tombstones: {}", e))?.filter_map(Result::ok).collect();
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

    if entries.is_empty() { return Ok(json!({ "pushed": 0 })); }

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
    Ok(json!({ "pushed": pushed, "commit": commit_sha }))
}

pub(crate) async fn gh_pull(db: &HanniDb) -> Result<Value, String> {
    let c = resolve_gh(db)?;
    let client = reqwest::Client::new();
    let (head, _) = gh_head(&client, &c).await?;
    let cursor = get_setting(&db.conn(), "cloud_owner_gh_pull_sha");
    if cursor.as_deref() == Some(head.as_str()) { return Ok(json!({ "applied": 0 })); }

    let mut files: Vec<(String, String)> = match &cursor {
        Some(cur) => parse_compare(
            gh_get(&client, &c, &format!("compare/{}...{}", cur, head)).await?),
        None => list_tree(&client, &c, &head).await?,
    };
    // `compare` returns at most 300 files on its first page; if we hit that the
    // diff may be truncated and advancing the cursor would skip the rest. Fall
    // back to a full-tree read (re-applying is LWW-idempotent) so nothing is lost.
    if files.len() >= 300 {
        files = list_tree(&client, &c, &head).await?;
    }

    let own_prefix = format!("{}/", c.device_id);
    let mut applied = 0u64;
    for (path, blob_sha) in &files {
        if path.starts_with(&own_prefix) || !path.contains('/') { continue; }
        let doc = match fetch_doc(&client, &c, path, blob_sha).await {
            Ok(d) => d,
            Err(e) => { eprintln!("[sync_github] {}: {}", path, e); continue; }
        };
        let conn = db.conn();
        let table = doc.get("_table").and_then(|v| v.as_str()).unwrap_or("");
        if table == "tombstones" {
            let target = doc.get("_target_table").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(id) = doc.get("_row_id").and_then(|v| v.as_i64()) {
                if SYNC_TABLES.contains(&target) {
                    let _ = conn.execute(&format!("DELETE FROM {} WHERE id = ?1", target),
                                         rusqlite::params![id]);
                    applied += 1;
                }
            }
        } else if SYNC_TABLES.contains(&table) {
            if upsert_row(&conn, table, &doc)? { applied += 1; }
        }
    }

    {
        let conn = db.conn();
        set_setting(&conn, "cloud_owner_gh_pull_sha", &head);
        set_setting(&conn, "cloud_owner_gh_last_pull_ts", &chrono::Utc::now().to_rfc3339());
    }
    Ok(json!({ "applied": applied }))
}

/// Changed/added blob paths (path, blob_sha) from a `compare` response; git
/// deletions are ignored (our deletes are explicit tombstone blobs).
fn parse_compare(cmp: Value) -> Vec<(String, String)> {
    cmp.get("files").and_then(|f| f.as_array()).map(|arr| arr.iter().filter_map(|f| {
        if f.get("status")?.as_str()? == "removed" { return None; }
        Some((f.get("filename")?.as_str()?.to_string(), f.get("sha")?.as_str()?.to_string()))
    }).collect()).unwrap_or_default()
}

/// Every blob (path, sha) in the tree at `head` — used for the first pull and
/// as the truncation-safe fallback when a compare page is capped.
async fn list_tree(client: &reqwest::Client, c: &GhCreds, head: &str)
                   -> Result<Vec<(String, String)>, String> {
    Ok(gh_get(client, c, &format!("git/trees/{}?recursive=1", head)).await?
        .get("tree").and_then(|t| t.as_array()).map(|arr| arr.iter().filter_map(|e| {
            if e.get("type")?.as_str()? != "blob" { return None; }
            Some((e.get("path")?.as_str()?.to_string(), e.get("sha")?.as_str()?.to_string()))
        }).collect()).unwrap_or_default())
}
