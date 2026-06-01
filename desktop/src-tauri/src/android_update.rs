// android_update.rs — Android-only APK update check + in-app install.
// Tauri's updater plugin is desktop-only, so on Android we poll the GitHub
// Releases API for the latest tag + APK asset, download it in-app, and hand
// it to the OS package installer via a FileProvider content:// URI.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

#[cfg(target_os = "android")]
use tauri::plugin::PluginHandle;

#[cfg(target_os = "android")]
pub struct InstallApkHandle<R: Runtime>(pub PluginHandle<R>);

/// Tauri plugin that bridges to the Kotlin InstallApkPlugin.
pub fn install_apk_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("install-apk")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api.register_android_plugin(
                    "com.sultanjakhan.hanni", "InstallApkPlugin"
                )?;
                app.manage(InstallApkHandle(handle));
            }
            #[cfg(not(target_os = "android"))]
            { let _ = app; }
            Ok(())
        })
        .build()
}

const RELEASES_API: &str = "https://api.github.com/repos/sultanjakhan/hanni/releases/latest";

#[derive(Serialize)]
pub struct ApkUpdate {
    available: bool,
    version: String,
    apk_url: String,
    notes: String,
    /// SHA-256 hex of the APK asset, from GitHub's trusted `digest` field.
    /// Empty when the release predates digest support; download proceeds
    /// unverified in that case (logged).
    sha256: String,
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
    let apk_asset = json
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
        });
    let apk_url = apk_asset
        .and_then(|asset| asset.get("browser_download_url").and_then(|u| u.as_str()))
        .unwrap_or("")
        .to_string();
    // GitHub populates `digest` ("sha256:<hex>") for uploaded assets. It comes
    // from the trusted HTTPS API, so verifying the downloaded APK against it
    // detects a tampered/MITM'd download.
    let sha256 = apk_asset
        .and_then(|asset| asset.get("digest").and_then(|d| d.as_str()))
        .unwrap_or("")
        .trim_start_matches("sha256:")
        .to_string();

    let available = !latest.is_empty() && !apk_url.is_empty() && is_newer(&latest, &current);
    Ok(ApkUpdate { available, version: latest, apk_url, notes, sha256 })
}

/// Opens an APK download URL in the system browser (fallback path; the
/// in-app flow is download_apk + install_apk).
#[tauri::command]
pub async fn open_apk_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only https URLs allowed".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("open failed: {e}"))
}

/// Downloads the APK to the app's cache dir, emitting `apk-download-progress`
/// events with `{loaded, total}` so the banner can show progress. Returns the
/// absolute file path on success.
#[tauri::command]
pub async fn download_apk(app: AppHandle, url: String, version: String, sha256: Option<String>) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err("Only https URLs allowed".into());
    }
    let expected_sha = sha256.unwrap_or_default();
    let cache_dir: PathBuf = app.path().app_cache_dir()
        .map_err(|e| format!("no cache dir: {e}"))?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("mkdir: {e}"))?;
    let safe_ver: String = version.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_').collect();
    let dest = cache_dir.join(format!("hanni-update-{safe_ver}.apk"));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url)
        .header("User-Agent", "Hanni-Android-Updater")
        .send().await
        .map_err(|e| format!("download request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    use std::io::Write;
    use futures_util::StreamExt;
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::create(&dest).map_err(|e| format!("create: {e}"))?;
    let mut hasher = Sha256::new();
    let mut loaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("chunk: {e}"))?;
        file.write_all(&bytes).map_err(|e| format!("write: {e}"))?;
        hasher.update(&bytes);
        loaded += bytes.len() as u64;
        // Emit at most every ~256KB to avoid event spam.
        if loaded - last_emit > 256_000 || loaded == total {
            let _ = app.emit("apk-download-progress",
                serde_json::json!({"loaded": loaded, "total": total}));
            last_emit = loaded;
        }
    }
    file.sync_all().ok();

    // Integrity: refuse to install an APK whose hash doesn't match the value
    // from the trusted GitHub API (MITM / tampered-download defense). If the
    // release carried no digest, proceed but log — can't verify older builds.
    if !expected_sha.is_empty() {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(&expected_sha) {
            let _ = std::fs::remove_file(&dest);
            return Err(format!("APK sha256 mismatch (got {got}, want {expected_sha}) — refusing"));
        }
    } else {
        eprintln!("[android_update] release has no digest — installing unverified APK");
    }
    Ok(dest.to_string_lossy().into_owned())
}

/// Hands the downloaded APK to the OS package installer via FileProvider.
#[tauri::command]
pub async fn install_apk<R: Runtime>(app: AppHandle<R>, path: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // Only ever install an APK we downloaded into our own cache dir — never
        // an arbitrary caller-supplied path handed to the OS package installer.
        let cache_dir = app.path().app_cache_dir().map_err(|e| format!("no cache dir: {e}"))?;
        let p = std::path::Path::new(&path);
        if !(p.starts_with(&cache_dir) && path.to_ascii_lowercase().ends_with(".apk")) {
            return Err("Refusing to install APK outside the app cache dir".into());
        }
        let handle = app.state::<InstallApkHandle<R>>();
        handle.0.run_mobile_plugin::<serde_json::Value>(
            "installApk",
            serde_json::json!({"path": path}),
        ).map_err(|e| format!("{e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    { let _ = (app, path); Err("install_apk is Android-only".into()) }
}

/// True if the user has granted "Install unknown apps" for Hanni. When false,
/// the banner must first open settings via open_install_settings().
#[tauri::command]
pub async fn can_install_apk<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let handle = app.state::<InstallApkHandle<R>>();
        match handle.0.run_mobile_plugin::<serde_json::Value>("canInstall", &()) {
            Ok(v) => Ok(v.get("granted").and_then(|g| g.as_bool()).unwrap_or(false)),
            Err(e) => Err(format!("{e}")),
        }
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Ok(false) }
}

#[tauri::command]
pub async fn open_install_settings<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let handle = app.state::<InstallApkHandle<R>>();
        handle.0.run_mobile_plugin::<serde_json::Value>("openInstallSettings", &())
            .map_err(|e| format!("{e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Err("Android-only".into()) }
}
