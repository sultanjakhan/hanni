// android_update.rs — Android-only APK update check.
// Tauri's updater plugin is desktop-only, so on Android we poll the GitHub
// Releases API for the latest tag + APK asset and let the user download it
// via the system browser (manual install). No native install intent.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

const RELEASES_API: &str = "https://api.github.com/repos/sultanjakhan/hanni/releases/latest";

#[derive(Serialize)]
pub struct ApkUpdate {
    available: bool,
    version: String,
    apk_url: String,
    notes: String,
}

/// True if `latest` is strictly newer than `current`, comparing numeric
/// dot-separated parts (e.g. "0.81.5" > "0.81.4"). Leading 'v' tolerated.
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (l, c) = (parse(latest), parse(current));
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

#[tauri::command]
pub async fn check_apk_update(app: AppHandle) -> Result<ApkUpdate, String> {
    let current = app.package_info().version.to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(RELEASES_API)
        .header("User-Agent", "Hanni-Android-Updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API status {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let latest = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    let notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let apk_url = json
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find(|asset| {
                asset
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.to_ascii_lowercase().ends_with(".apk"))
                    .unwrap_or(false)
            })
        })
        .and_then(|asset| asset.get("browser_download_url").and_then(|u| u.as_str()))
        .unwrap_or("")
        .to_string();

    let available = !latest.is_empty() && !apk_url.is_empty() && is_newer(&latest, &current);
    Ok(ApkUpdate { available, version: latest, apk_url, notes })
}

/// Opens an APK download URL in the system browser (cross-platform via the
/// opener plugin — macos.rs::open_url shells out to `open`, which is unavailable
/// on Android).
#[tauri::command]
pub async fn open_apk_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only https URLs allowed".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("open failed: {e}"))
}
