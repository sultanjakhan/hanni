// firestore_admin.rs — One-shot Firestore project setup (Stage C.1).
//
// Uses the Google OAuth access token (cloud-platform scope, captured by
// google_auth.rs) to:
//   1. Create the (default) Firestore database in a chosen region, if
//      missing. Creating a DB also flips the API from disabled → enabled,
//      which is what unblocks subsequent reads/writes from sync_owner.
//   2. Deploy a per-user security ruleset:
//        match /owners/{uid}/changes/{doc} {
//          allow read, write: if request.auth.uid == uid;
//        }
//      and bind it to the cloud.firestore release.
//
// Idempotent: running it twice on an already-configured project just
// re-deploys rules (cheap, no charge).

use serde_json::json;
use tauri::State;

use crate::google_auth::{get_google_access_token, load_config};
use crate::types::HanniDb;

const FIRESTORE_API: &str = "https://firestore.googleapis.com/v1";
const RULES_API: &str = "https://firebaserules.googleapis.com/v1";

const RULES_TEXT: &str = r#"rules_version = '2';
service cloud.firestore {
  match /databases/{database}/documents {
    match /owners/{uid}/changes/{doc} {
      allow read, write: if request.auth != null && request.auth.uid == uid;
    }
  }
}
"#;

async fn ensure_database(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
    location: &str,
) -> Result<&'static str, String> {
    // Probe existing (default) DB.
    let probe_url = format!("{}/projects/{}/databases/(default)", FIRESTORE_API, project_id);
    let probe = client.get(&probe_url).bearer_auth(token).send().await
        .map_err(|e| format!("probe db: {}", e))?;
    if probe.status().is_success() {
        return Ok("already-exists");
    }

    // Create.
    let create_url = format!(
        "{}/projects/{}/databases?databaseId=(default)",
        FIRESTORE_API, project_id
    );
    let create_body = json!({
        "locationId": location,
        "type": "FIRESTORE_NATIVE",
    });
    let resp = client.post(&create_url).bearer_auth(token).json(&create_body).send().await
        .map_err(|e| format!("create db: {}", e))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
    if !status.is_success() {
        return Err(format!("create db {}: {}", status, body));
    }

    // Long-running operation; poll until done (≤60s in practice).
    let op_name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !op_name.is_empty() {
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let op_url = format!("{}/{}", FIRESTORE_API, op_name);
            let op = client.get(&op_url).bearer_auth(token).send().await
                .map_err(|e| format!("op poll: {}", e))?
                .json::<serde_json::Value>().await.unwrap_or(json!({}));
            if op.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                if let Some(err) = op.get("error") {
                    return Err(format!("create db op failed: {}", err));
                }
                return Ok("created");
            }
        }
    }
    Ok("created-pending")
}

async fn deploy_rules(
    client: &reqwest::Client,
    token: &str,
    project_id: &str,
) -> Result<(), String> {
    // 1. Create ruleset.
    let create_url = format!("{}/projects/{}/rulesets", RULES_API, project_id);
    let body = json!({
        "source": { "files": [{ "name": "firestore.rules", "content": RULES_TEXT }] }
    });
    let resp = client.post(&create_url).bearer_auth(token).json(&body).send().await
        .map_err(|e| format!("create ruleset: {}", e))?;
    let status = resp.status();
    let payload: serde_json::Value = resp.json().await.unwrap_or(json!({}));
    if !status.is_success() {
        return Err(format!("create ruleset {}: {}", status, payload));
    }
    let ruleset_name = payload.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| format!("ruleset response missing name: {}", payload))?
        .to_string();

    // 2. Update existing release (or create if missing).
    let release_id = format!("projects/{}/releases/cloud.firestore", project_id);
    let update_url = format!("{}/{}", RULES_API, release_id);
    let update_body = json!({ "release": { "name": release_id, "rulesetName": ruleset_name } });
    let upd = client.patch(&update_url).bearer_auth(token).json(&update_body).send().await
        .map_err(|e| format!("bind release: {}", e))?;
    let upd_status = upd.status();
    if !upd_status.is_success() {
        // First-time projects have no release yet → create it.
        let create_release_url = format!("{}/projects/{}/releases", RULES_API, project_id);
        let create_release_body = json!({ "name": release_id, "rulesetName": ruleset_name });
        let cr = client.post(&create_release_url).bearer_auth(token)
            .json(&create_release_body).send().await
            .map_err(|e| format!("create release: {}", e))?;
        let cr_status = cr.status();
        if !cr_status.is_success() {
            let txt = cr.text().await.unwrap_or_default();
            return Err(format!("release {} (after PATCH {}): {}",
                cr_status, upd_status, txt));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn firestore_setup(
    location: Option<String>,
    db: State<'_, HanniDb>,
) -> Result<serde_json::Value, String> {
    let location = location.unwrap_or_else(|| "eur3".to_string());
    let project_id = {
        let conn = db.conn();
        load_config(&conn).map(|c| c.project_id)
            .ok_or_else(|| "Google Auth not configured".to_string())?
    };
    let token = get_google_access_token(&db).await?;
    let client = reqwest::Client::new();

    let db_status = ensure_database(&client, &token, &project_id, &location).await?;
    deploy_rules(&client, &token, &project_id).await?;

    Ok(json!({
        "ok": true,
        "project_id": project_id,
        "location": location,
        "database": db_status,
        "rules": "deployed",
    }))
}
