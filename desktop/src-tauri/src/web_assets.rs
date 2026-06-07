// web_assets.rs — Android OTA web-asset serving + update.
//
// Serves the frontend (HTML/JS/CSS/vendor) through a custom URI scheme so it
// can be swapped at runtime from an OTA bundle in app_data_dir/web/current/
// without reinstalling the ~106MB APK. When no valid bundle is present, every
// request falls back to the APK-embedded assets via `asset_resolver()`, so a
// missing/partial/corrupt bundle can never brick the app.
//
// The protocol + window wiring are Android-only; the OTA commands compile on
// all platforms (no-op-ish on desktop, which keeps its tauri-plugin-updater).

use std::path::PathBuf;
use tauri::{Manager, Runtime};

/// Written last by `apply` once an extract is complete + validated. Its absence
/// makes the protocol ignore a half-written dir and serve embedded assets.
pub const READY_MARKER: &str = ".ready";

/// A freshly-applied bundle carries this until the frontend confirms it booted
/// (`web_ota_boot_ok`). An unconfirmed trial is reverted on the next launch.
pub const TRIAL_MARKER: &str = ".trial";
/// Set on the first boot that serves a trial bundle; if it survives to the next
/// boot the trial never confirmed (white-screened) → the bundle is reverted.
const PENDING_MARKER: &str = ".trial_pending";

/// Custom scheme for OTA-served frontend.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub const SCHEME: &str = "hanniweb";

/// The URL wry exposes our custom scheme at — the form differs per platform:
/// Android/Linux serve it as `http://<scheme>.localhost/`, while macOS/iOS use
/// `<scheme>://localhost/`. Navigating to the wrong form white-screens.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn nav_url() -> String {
    #[cfg(target_os = "macos")]
    { format!("{}://localhost/index.html", SCHEME) }
    #[cfg(not(target_os = "macos"))]
    { format!("http://{}.localhost/index.html", SCHEME) }
}

fn web_base<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    app.path()
        .app_data_dir()
        .ok()
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(format!("/data/data/{}/files", app.config().identifier)))
        .join("web")
}
fn current_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("current") }
fn staging_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("staging") }
fn version_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("version.txt") }

// ───────────────────────── protocol (Android only) ─────────────────────────

#[cfg(any(target_os = "android", target_os = "macos"))]
fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "glb" => "model/gltf-binary",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

/// URI → asset-relative path (no leading slash; "" → index.html; strips query).
#[cfg(any(target_os = "android", target_os = "macos"))]
fn rel_path(uri: &str) -> String {
    let after_scheme = uri.splitn(2, "://").nth(1).unwrap_or(uri);
    let path = after_scheme.splitn(2, '/').nth(1).unwrap_or("");
    let path = path.split(['?', '#']).next().unwrap_or("");
    if path.is_empty() { "index.html".to_string() } else { path.to_string() }
}

/// Resolve to (bytes, mime, csp): OTA dir first (when ready + safe), else the
/// APK-embedded assets. Rejects `..` traversal out of the bundle dir.
#[cfg(any(target_os = "android", target_os = "macos"))]
fn resolve<R: Runtime>(app: &tauri::AppHandle<R>, rel: &str) -> Option<(Vec<u8>, String, Option<String>)> {
    let traversal = rel.split('/').any(|c| c == "..");
    if !traversal {
        let dir = current_dir(app);
        if dir.join(READY_MARKER).exists() {
            if let Ok(bytes) = std::fs::read(dir.join(rel)) {
                return Some((bytes, mime_for(rel).to_string(), embedded_csp(app)));
            }
        }
    }
    app.asset_resolver()
        .get(rel.to_string())
        .map(|a| (a.bytes, a.mime_type, a.csp_header))
}

/// CSP from the embedded index.html so OTA-served HTML carries the same policy.
#[cfg(any(target_os = "android", target_os = "macos"))]
fn embedded_csp<R: Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.asset_resolver().get("index.html".to_string()).and_then(|a| a.csp_header)
}

#[cfg(any(target_os = "android", target_os = "macos"))]
fn not_found() -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder().status(404).body(b"not found".to_vec()).unwrap()
}

/// Registers the OTA web-asset protocol on the builder (Android only).
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    use tauri::http::{Request, Response};
    use tauri::UriSchemeContext;
    builder.register_uri_scheme_protocol(SCHEME, move |ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>| {
        let app = ctx.app_handle();
        let rel = rel_path(&request.uri().to_string());
        match resolve(app, &rel) {
            Some((bytes, mime, csp)) => {
                let mut b = Response::builder()
                    .status(200)
                    .header("Content-Type", mime)
                    .header("Access-Control-Allow-Origin", "*")
                    // No caching: the WebView persistently caches custom-protocol
                    // responses, which would defeat OTA updates (an applied bundle
                    // wouldn't show until cache eviction). Local disk reads are
                    // cheap, so always re-serve the current bytes.
                    .header("Cache-Control", "no-store");
                if let Some(csp) = csp {
                    b = b.header("Content-Security-Policy", csp);
                }
                b.body(bytes).unwrap_or_else(|_| not_found())
            }
            None => not_found(),
        }
    })
}

// ───────────────────────── OTA update (all platforms) ──────────────────────

/// True if `a` >= `b` comparing numeric dot-parts ("0.91.3" >= "0.91.2").
fn version_gte(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect()
    };
    let (a, b) = (parse(a), parse(b));
    for i in 0..a.len().max(b.len()) {
        let (av, bv) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if av != bv { return av > bv; }
    }
    true // equal
}

#[derive(serde::Serialize)]
pub struct WebUpdate {
    pub available: bool,
    pub web_version: String,
    pub url: String,
    pub sha256: String,
}

const RELEASES_API: &str = "https://api.github.com/repos/sultanjakhan/hanni/releases/latest";

/// Applied web-bundle version + the native (APK) version.
#[tauri::command]
pub async fn web_ota_status<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value, String> {
    let applied = std::fs::read_to_string(version_file(&app)).unwrap_or_default().trim().to_string();
    Ok(serde_json::json!({
        "applied": applied,
        "native": app.package_info().version.to_string(),
    }))
}

/// Check the latest GitHub release for a newer, compatible web bundle.
/// Reads a `web-manifest.json` asset: {web_version, min_native_version, sha256, asset}.
#[tauri::command]
pub async fn web_ota_check<R: Runtime>(app: tauri::AppHandle<R>) -> Result<WebUpdate, String> {
    let none = || WebUpdate { available: false, web_version: String::new(), url: String::new(), sha256: String::new() };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build().map_err(|e| e.to_string())?;
    let rel: serde_json::Value = client.get(RELEASES_API)
        .header("User-Agent", "Hanni-Web-OTA")
        .header("Accept", "application/vnd.github+json")
        .send().await.map_err(|e| format!("release fetch: {e}"))?
        .json().await.map_err(|e| e.to_string())?;
    let assets = rel.get("assets").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let find_url = |name_suffix: &str| assets.iter().find(|a|
        a.get("name").and_then(|n| n.as_str()).map(|n| n.ends_with(name_suffix)).unwrap_or(false)
    ).and_then(|a| a.get("browser_download_url").and_then(|u| u.as_str())).map(String::from);

    let manifest_url = match find_url("web-manifest.json") { Some(u) => u, None => return Ok(none()) };
    let manifest: serde_json::Value = client.get(&manifest_url)
        .header("User-Agent", "Hanni-Web-OTA").send().await.map_err(|e| format!("manifest: {e}"))?
        .json().await.map_err(|e| e.to_string())?;
    let web_version = manifest.get("web_version").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let min_native = manifest.get("min_native_version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
    let sha256 = manifest.get("sha256").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let asset_name = manifest.get("asset").and_then(|v| v.as_str()).unwrap_or("");
    let url = match find_url(asset_name) { Some(u) => u, None => return Ok(none()) };

    let native = app.package_info().version.to_string();
    let applied = std::fs::read_to_string(version_file(&app)).unwrap_or_default().trim().to_string();
    // Available iff: native shell new enough AND bundle newer than what's applied.
    let compatible = version_gte(&native, min_native);
    let newer = applied.is_empty() || (version_gte(&web_version, &applied) && web_version != applied);
    let available = compatible && newer && !web_version.is_empty() && !sha256.is_empty() && !url.is_empty();
    Ok(WebUpdate { available, web_version, url, sha256 })
}

/// Download → verify sha256 → extract → atomic swap into web/current.
/// On any failure the existing bundle (or embedded fallback) stays intact.
#[tauri::command]
pub async fn web_ota_apply<R: Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
    web_version: String,
    sha256: String,
) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1")) {
        return Err("only https (or localhost http for testing) allowed".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build().map_err(|e| e.to_string())?;
    let bytes = client.get(&url).header("User-Agent", "Hanni-Web-OTA")
        .send().await.map_err(|e| format!("download: {e}"))?
        .error_for_status().map_err(|e| format!("download status: {e}"))?
        .bytes().await.map_err(|e| e.to_string())?;

    // Verify integrity before touching disk.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(&sha256) {
        return Err(format!("sha256 mismatch: got {got}, want {sha256}"));
    }

    let staging = staging_dir(&app);
    let current = current_dir(&app);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("mkdir staging: {e}"))?;

    // Extract tar.gz (must be flat: index.html at root, no top-level dir).
    // Reject symlinks/hardlinks and any entry escaping staging — defense in depth
    // on top of sha256 so a tampered bundle can't drop a link out of the dir.
    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries().map_err(|e| format!("read archive: {e}"))? {
        let mut entry = entry.map_err(|e| format!("entry: {e}"))?;
        match entry.header().entry_type() {
            tar::EntryType::Regular | tar::EntryType::Directory => {
                if !entry.unpack_in(&staging).map_err(|e| format!("extract: {e}"))? {
                    let _ = std::fs::remove_dir_all(&staging);
                    return Err("bundle entry escapes staging dir".into());
                }
            }
            _ => continue, // skip symlinks, hardlinks, devices, fifos
        }
    }

    // Validate the bundle has an entry point before swapping it in.
    if !staging.join("index.html").exists() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("bundle missing index.html".into());
    }
    std::fs::write(staging.join(READY_MARKER), b"").map_err(|e| format!("ready marker: {e}"))?;
    // Trial: the bundle serves on next launch but must prove it boots (the
    // frontend calls web_ota_boot_ok) or verify_trial_on_boot reverts it.
    std::fs::write(staging.join(TRIAL_MARKER), web_version.as_bytes())
        .map_err(|e| format!("trial marker: {e}"))?;

    // Atomic-ish swap. If we crash between remove and rename, `current` is gone
    // → the protocol serves embedded assets (safe), and next check re-applies.
    let _ = std::fs::remove_dir_all(&current);
    std::fs::rename(&staging, &current).map_err(|e| format!("swap: {e}"))?;
    std::fs::write(version_file(&app), web_version.as_bytes()).ok();
    Ok(())
}

/// Trial-boot safety net — call once at startup BEFORE navigating to the OTA
/// bundle. If the current bundle is a trial that already had its one boot
/// attempt (the pending marker survived), it white-screened last launch → drop
/// it so this launch falls back to embedded assets. version.txt keeps the
/// version so we don't immediately re-download the same bad bundle.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn verify_trial_on_boot<R: Runtime>(app: &tauri::AppHandle<R>) {
    let current = current_dir(app);
    if !current.join(TRIAL_MARKER).exists() {
        return;
    }
    if current.join(PENDING_MARKER).exists() {
        let _ = std::fs::remove_dir_all(&current);
    } else {
        let _ = std::fs::write(current.join(PENDING_MARKER), b"");
    }
}

/// Reconcile the applied OTA bundle against the embedded assets at startup. A
/// native update ships fresh embedded assets; if the currently-applied bundle is
/// OLDER than the native version (or there is none), drop it so the newer
/// embedded assets serve instead of being shadowed by a stale bundle, and set
/// the baseline to the native version so web_ota_check won't re-download a bundle
/// identical to what just shipped. Call at startup, AFTER verify_trial_on_boot.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn reconcile_native_baseline<R: Runtime>(app: &tauri::AppHandle<R>) {
    let native = app.package_info().version.to_string();
    let applied = std::fs::read_to_string(version_file(app)).unwrap_or_default().trim().to_string();
    let has_bundle = current_dir(app).join(READY_MARKER).exists();
    // A genuine web update (>= native) sitting on top of this shell → keep it.
    if has_bundle && version_gte(&applied, &native) {
        return;
    }
    if has_bundle {
        let _ = std::fs::remove_dir_all(current_dir(app));
    }
    if applied != native {
        let _ = std::fs::create_dir_all(web_base(app));
        let _ = std::fs::write(version_file(app), native.as_bytes());
    }
}

/// Called by the frontend once it has loaded successfully. Confirms a trial
/// bundle by clearing its markers so it's kept permanently. No-op otherwise.
#[tauri::command]
pub fn web_ota_boot_ok<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let current = current_dir(&app);
    let _ = std::fs::remove_file(current.join(TRIAL_MARKER));
    let _ = std::fs::remove_file(current.join(PENDING_MARKER));
    Ok(())
}

// ──────── origin migration (tauri://localhost → hanniweb://localhost) ────────
//
// Serving the frontend through the custom scheme changes the document origin,
// which partitions localStorage. To carry the user's UI prefs across the switch
// we stage it: launch 1 stays on the old origin and exports localStorage; from
// launch 2 we navigate to the scheme and the frontend re-imports the dump.
// A switch that never boots (the frontend never calls web_origin_ok — e.g. a
// white screen) accrues strikes in the pending file; after MAX_UNCONFIRMED
// consecutive misses we disable it and serve embedded assets. One unlucky
// quick-quit in the ~1s boot window therefore can't kill a working channel, and
// a disabled channel auto-recovers after the next native update (a new shell may
// fix whatever broke it).

/// Tolerate this many consecutive unconfirmed switches before disabling.
const MAX_UNCONFIRMED: u32 = 3;

fn origin_stage_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("origin_stage") }
fn ls_dump_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("ls_dump.json") }
fn origin_pending_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("origin_pending") }
/// Native version recorded when the switch was disabled — used to auto-retry
/// once a newer shell is installed.
fn origin_native_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf { web_base(app).join("origin_native") }

fn read_origin_stage<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    std::fs::read_to_string(origin_stage_file(app)).unwrap_or_default().trim().to_string()
}
fn write_origin_stage<R: Runtime>(app: &tauri::AppHandle<R>, stage: &str) {
    let _ = std::fs::create_dir_all(web_base(app));
    let _ = std::fs::write(origin_stage_file(app), stage.as_bytes());
}
/// Consecutive unconfirmed-boot strikes (absent file = 0). web_origin_ok clears
/// it each successful launch, so only repeated failures accumulate.
fn read_pending_strikes<R: Runtime>(app: &tauri::AppHandle<R>) -> u32 {
    std::fs::read_to_string(origin_pending_file(app)).ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Decide whether to navigate the main window to the custom scheme this launch.
/// Drives the staged migration and self-heals a switch that white-screened.
pub fn prepare_origin<R: Runtime>(app: &tauri::AppHandle<R>) -> bool {
    let native = app.package_info().version.to_string();
    // Auto-recover a disabled switch once a newer native shell is installed (it
    // may fix whatever broke the switch). Legacy disables (no recorded version)
    // also get one retry. Otherwise stay on embedded assets.
    if read_origin_stage(app) == "disabled" {
        let at = std::fs::read_to_string(origin_native_file(app)).unwrap_or_default().trim().to_string();
        if at.is_empty() || at != native {
            let _ = std::fs::remove_file(origin_pending_file(app));
            write_origin_stage(app, "exported");
        } else {
            return false;
        }
    }
    match read_origin_stage(app).as_str() {
        // Ready to switch (or already switched). Navigate, recording a strike; a
        // switch that boots clears it (web_origin_ok). Only after
        // MAX_UNCONFIRMED consecutive unconfirmed boots do we disable + fall back.
        "exported" | "live" => {
            let strikes = read_pending_strikes(app);
            if strikes >= MAX_UNCONFIRMED {
                let _ = std::fs::remove_file(origin_pending_file(app));
                write_origin_stage(app, "disabled");
                let _ = std::fs::write(origin_native_file(app), native.as_bytes());
                eprintln!("[hanni] web_assets: switch unconfirmed {strikes}× → embedded fallback (retries after next native update)");
                false
            } else {
                let _ = std::fs::create_dir_all(web_base(app));
                let _ = std::fs::write(origin_pending_file(app), (strikes + 1).to_string().as_bytes());
                true
            }
        }
        // Pristine (first launch of an OTA-capable build): stay on the default
        // origin until the frontend exports localStorage.
        _ => false,
    }
}

/// Frontend (old origin) hands us its localStorage so the new origin can restore
/// it. Advances the migration to "exported" so the next launch switches.
#[tauri::command]
pub fn web_ls_export<R: Runtime>(app: tauri::AppHandle<R>, json: String) -> Result<(), String> {
    let _ = std::fs::create_dir_all(web_base(&app));
    std::fs::write(ls_dump_file(&app), json.as_bytes()).map_err(|e| e.to_string())?;
    if read_origin_stage(&app).is_empty() {
        write_origin_stage(&app, "exported");
    }
    Ok(())
}

/// Frontend (new origin) asks for the exported localStorage to repopulate it.
#[tauri::command]
pub fn web_ls_import<R: Runtime>(app: tauri::AppHandle<R>) -> Result<Option<String>, String> {
    Ok(std::fs::read_to_string(ls_dump_file(&app)).ok())
}

/// Frontend confirms it booted on the custom-scheme origin. Clears the pending
/// strikes (and any stale disable record) so the switch is kept, and marks the
/// migration "live".
#[tauri::command]
pub fn web_origin_ok<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let _ = std::fs::remove_file(origin_pending_file(&app));
    let _ = std::fs::remove_file(origin_native_file(&app));
    write_origin_stage(&app, "live");
    Ok(())
}
