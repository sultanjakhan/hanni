// sync_github_api.rs — GitHub REST + codec helpers for Tier 3 owner-sync.
// Credentials, the encrypt→tree-entry codec, and thin Git Data API wrappers.
// Orchestration (cursors, push/pull flow) lives in sync_github.rs.

use crate::sync_crypto::{doc_name, open, seal};
use crate::sync_owner::get_setting;
use crate::types::HanniDb;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use reqwest::Method;
use serde_json::{json, Map, Value};

pub(crate) const API: &str = "https://api.github.com";

pub(crate) struct GhCreds {
    pub pat: String,
    pub repo: String,
    pub key: [u8; 32],
    pub device_id: String,
}

pub(crate) fn resolve_gh(db: &HanniDb) -> Result<GhCreds, String> {
    let conn = db.conn();
    let pat = get_setting(&conn, "cloud_owner_gh_pat").ok_or("GitHub sync: PAT not set")?;
    let repo = get_setting(&conn, "cloud_owner_gh_repo").ok_or("GitHub sync: repo not set")?;
    let key_hex = get_setting(&conn, "cloud_owner_gh_key")
        .ok_or("GitHub sync: shared key not set")?;
    let key: [u8; 32] = hex::decode(&key_hex).ok().and_then(|b| b.try_into().ok())
        .ok_or("GitHub sync: key must be 32-byte hex")?;
    let device_id = get_setting(&conn, "device_id").unwrap_or_else(|| "unknown".into());
    Ok(GhCreds { pat, repo, key, device_id })
}

/// Plain-JSON change-set doc (NOT Firestore field-format — this gets encrypted).
pub(crate) fn build_doc(row: &Value, device_id: &str, updated_at: &str, table: &str) -> Value {
    let mut obj = match row { Value::Object(m) => m.clone(), _ => Map::new() };
    obj.insert("_device_id".into(), json!(device_id));
    obj.insert("_updated_at".into(), json!(updated_at));
    obj.insert("_synced_at".into(), json!(chrono::Utc::now().to_rfc3339()));
    obj.insert("_table".into(), json!(table));
    Value::Object(obj)
}

/// Seal a doc and wrap it as a Git tree entry under this device's outbox dir.
pub(crate) fn blob_entry(c: &GhCreds, label: &str, doc: &Value) -> Result<Value, String> {
    let path = format!("{}/{}", c.device_id, doc_name(&c.key, label));
    let blob = seal(&c.key, path.as_bytes(), doc.to_string().as_bytes())?;
    Ok(json!({ "path": path, "mode": "100644", "type": "blob", "content": B64.encode(&blob) }))
}

pub(crate) async fn gh_req(client: &reqwest::Client, c: &GhCreds, method: Method, path: &str,
                           body: Option<&Value>) -> Result<(u16, Value), String> {
    let url = format!("{}/repos/{}/{}", API, c.repo, path);
    let mut rb = client.request(method, &url)
        .header("User-Agent", "Hanni")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .bearer_auth(&c.pat);
    if let Some(b) = body { rb = rb.json(b); }
    let resp = rb.send().await.map_err(|e| format!("{}: {}", path, e))?;
    let status = resp.status().as_u16();
    let val = resp.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, val))
}

pub(crate) async fn gh_get(client: &reqwest::Client, c: &GhCreds, path: &str)
                           -> Result<Value, String> {
    let (s, v) = gh_req(client, c, Method::GET, path, None).await?;
    if (200..300).contains(&s) { Ok(v) } else { Err(format!("GET {} -> {}: {}", path, s, v)) }
}

pub(crate) async fn gh_post(client: &reqwest::Client, c: &GhCreds, path: &str, body: &Value)
                            -> Result<Value, String> {
    let (s, v) = gh_req(client, c, Method::POST, path, Some(body)).await?;
    if (200..300).contains(&s) { Ok(v) } else { Err(format!("POST {} -> {}: {}", path, s, v)) }
}

/// (parent_commit_sha, base_tree_sha). The repo is seeded with an initial
/// commit, so the ref always exists.
pub(crate) async fn gh_head(client: &reqwest::Client, c: &GhCreds)
                            -> Result<(String, String), String> {
    let r = gh_get(client, c, "git/ref/heads/main").await?;
    let commit_sha = r.pointer("/object/sha").and_then(|v| v.as_str())
        .ok_or("no ref sha")?.to_string();
    let commit = gh_get(client, c, &format!("git/commits/{}", commit_sha)).await?;
    let tree_sha = commit.pointer("/tree/sha").and_then(|v| v.as_str())
        .ok_or("no tree sha")?.to_string();
    Ok((commit_sha, tree_sha))
}

/// Fetch a blob, undo the double base64 (git's wrapper + our encoding), decrypt.
pub(crate) async fn fetch_doc(client: &reqwest::Client, c: &GhCreds, path: &str, blob_sha: &str)
                              -> Result<Map<String, Value>, String> {
    let b = gh_get(client, c, &format!("git/blobs/{}", blob_sha)).await?;
    let git_b64: String = b.get("content").and_then(|v| v.as_str()).unwrap_or("")
        .split_whitespace().collect();
    let file_bytes = B64.decode(git_b64).map_err(|_| "git blob base64")?;
    let blob = B64.decode(&file_bytes).map_err(|_| "inner base64")?;
    let plain = open(&c.key, path.as_bytes(), &blob)?;
    serde_json::from_slice(&plain).map_err(|e| format!("doc parse: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds() -> GhCreds {
        GhCreds { pat: "x".into(), repo: "o/r".into(), key: [42u8; 32], device_id: "devA".into() }
    }

    #[test]
    fn codec_roundtrip() {
        let c = creds();
        let row = json!({ "id": 5, "name": "soup", "calories": 120 });
        let doc = build_doc(&row, &c.device_id, "2026-05-31T00:00:00Z", "recipes");
        let entry = blob_entry(&c, "row:recipes_5", &doc).unwrap();
        let path = entry.get("path").unwrap().as_str().unwrap();
        let content = entry.get("content").unwrap().as_str().unwrap();
        // Mirror fetch_doc's inner decode (git's outer base64 wrapper is added /
        // stripped by the API and is validated separately end-to-end).
        let blob = B64.decode(content).unwrap();
        let plain = open(&c.key, path.as_bytes(), &blob).unwrap();
        let parsed: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(parsed, doc); // exact round-trip through seal/base64/open/parse
        assert!(path.starts_with("devA/")); // own outbox dir
        assert!(!path.contains("recipes")); // table name not leaked into the name
    }

    #[test]
    fn open_with_wrong_path_fails() {
        let c = creds();
        let doc = build_doc(&json!({ "id": 1 }), &c.device_id, "t", "notes");
        let entry = blob_entry(&c, "row:notes_1", &doc).unwrap();
        let blob = B64.decode(entry.get("content").unwrap().as_str().unwrap()).unwrap();
        // AAD = path binds ciphertext to its slot: a wrong path must fail to open.
        assert!(open(&c.key, b"devA/wrongname", &blob).is_err());
    }
}
