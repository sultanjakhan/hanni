// web_assets.rs — Android OTA web-asset serving + update.
//
// Serves the frontend (HTML/JS/CSS/vendor) through a custom URI scheme so it
// can be swapped at runtime from an OTA bundle in
// app_data_dir/web/signed-ota-v1/current/
// without reinstalling the ~106MB APK. When no valid bundle is present, every
// request falls back to the APK-embedded assets via `asset_resolver()`, so a
// missing/partial/corrupt bundle can never brick the app.
//
// The protocol + window wiring are Android-only; the OTA commands compile on
// all platforms (no-op-ish on desktop, which keeps its tauri-plugin-updater).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tauri::{Manager, Runtime};

static WEB_OTA_APPLY_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static WEB_OTA_STATE_LOCK: Mutex<()> = Mutex::new(());
static WEB_OTA_SERVING_DISABLED: AtomicBool = AtomicBool::new(false);
static WEB_OTA_PROCESS_LEASE: OnceLock<std::fs::File> = OnceLock::new();
#[cfg(any(target_os = "android", target_os = "macos"))]
static VOLATILE_CACHE_EPOCH: OnceLock<String> = OnceLock::new();

fn owns_ota_process_lease() -> bool {
    WEB_OTA_PROCESS_LEASE.get().is_some()
}

fn lock_ota_state() -> MutexGuard<'static, ()> {
    WEB_OTA_STATE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Hold an OS-level exclusive lease for the process lifetime. A second Hanni
/// instance falls back to embedded assets instead of racing trial markers or
/// swapping a directory beneath the first instance's WebView.
pub fn acquire_ota_process_lease<R: Runtime>(app: &tauri::AppHandle<R>) -> bool {
    if WEB_OTA_PROCESS_LEASE.get().is_some() {
        return !WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst);
    }
    let base = web_base(app);
    if let Err(e) = create_dir_all_synced(&base, "OTA lease") {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        eprintln!("[hanni] web_assets: cannot create OTA lease directory: {e}");
        return false;
    }
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(base.join(".process.lock"))
    {
        Ok(file) => file,
        Err(e) => {
            WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
            eprintln!("[hanni] web_assets: cannot open OTA process lease: {e}");
            return false;
        }
    };
    if let Err(e) = file.try_lock() {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        eprintln!("[hanni] web_assets: another process owns OTA state: {e}");
        return false;
    }
    let _ = WEB_OTA_PROCESS_LEASE.set(file);
    if let Err(e) = ensure_secure_ota_state(app) {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        eprintln!("[hanni] web_assets: cannot establish signed OTA state: {e}");
        return false;
    }
    true
}

/// Written last by `apply` once an extract is complete + validated. Its absence
/// makes the protocol ignore a half-written dir and serve embedded assets.
pub const READY_MARKER: &str = ".ready";
const BUNDLE_VERSION_MARKER: &str = ".version";
const SECURE_SCHEMA_MARKER: &str = ".signed-ota-v1";
const SECURE_SCHEMA_CONTENTS: &[u8] = b"hanni.signed-web-ota.v1\n";
const CACHE_EPOCH_MARKER: &str = ".cache-epoch";
const INITIALIZING_MARKER: &str = ".initializing-v1";

/// A freshly-applied bundle carries this until the frontend confirms it booted
/// (`web_ota_boot_ok`). An unconfirmed trial is reverted on the next launch.
pub const TRIAL_MARKER: &str = ".trial";
/// Set on the first boot that serves a trial bundle; if it survives to the next
/// boot the trial never confirmed (white-screened) → the bundle is reverted.
const PENDING_MARKER: &str = ".trial_pending";

/// Per-launch flag the frontend flips via `web_ota_boot_ok` once it has actually
/// painted. The boot watchdog reads it: if it stays false past the window while
/// an OTA bundle is live, the bundle white-screened → revert. In-memory (not a
/// file) so it resets every launch, independent of the persisted trial markers
/// (those can be cleared eagerly; this is the authoritative "did we paint?" bit).
#[derive(Default)]
pub struct BootGuard {
    confirmed: AtomicBool,
    expected: Mutex<Option<String>>,
    nonce: Mutex<Option<String>>,
    loaded_nonce: Mutex<Option<String>>,
}

/// How long to wait for the frontend to confirm a real paint before assuming the
/// served OTA bundle white-screened. Generous so a slow cold start never trips it
/// (the confirm fires as soon as the tab bar paints, typically well under 2s).
#[cfg(any(target_os = "android", target_os = "macos"))]
const WATCHDOG_SECS: u64 = 12;

/// Custom scheme for OTA-served frontend.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub const SCHEME: &str = "hanniweb";

/// The URL wry exposes our custom scheme at — the form differs per platform:
/// Android/Linux serve it as `http://<scheme>.localhost/`, while macOS/iOS use
/// `<scheme>://localhost/`. Navigating to the wrong form white-screens.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn nav_url<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    #[cfg(target_os = "macos")]
    let base = { format!("{}://localhost/index.html", SCHEME) };
    #[cfg(not(target_os = "macos"))]
    let base = { format!("http://{}.localhost/index.html", SCHEME) };
    match selected_boot_nonce(app) {
        Some(nonce) => format!("{base}#hanni-ota-boot={nonce}"),
        None => base,
    }
}

fn web_root<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    app.path()
        .app_data_dir()
        .ok()
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(format!("/data/data/{}/files", app.config().identifier)))
        .join("web")
}
fn web_base<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_root(app).join("signed-ota-v1")
}
fn current_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("current")
}
fn next_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("next")
}
fn staging_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("staging")
}
fn version_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("version.txt")
}
fn watermark_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("high-watermark.txt")
}
fn watermark_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("high-watermarks")
}
fn rejected_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join("rejected-versions")
}
fn secure_schema_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join(SECURE_SCHEMA_MARKER)
}
fn cache_epoch_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join(CACHE_EPOCH_MARKER)
}
fn initializing_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_base(app).join(INITIALIZING_MARKER)
}

fn read_cache_epoch<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<String, String> {
    let raw = std::fs::read_to_string(cache_epoch_file(app))
        .map_err(|e| format!("OTA cache epoch read: {e}"))?;
    let value = raw.trim();
    let parsed = uuid::Uuid::parse_str(value).map_err(|_| "OTA cache epoch is invalid")?;
    if parsed.get_version_num() != 4 || parsed.hyphenated().to_string() != value {
        return Err("OTA cache epoch is not a canonical v4 UUID".into());
    }
    Ok(value.to_string())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    std::fs::File::open(path)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| format!("sync directory {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Create a directory chain and durably publish every new directory entry on
/// Unix. In particular, the append-only rollback floor must not disappear
/// because its top-level `web/` directory was never synced.
fn create_dir_all_synced(path: &Path, label: &str) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        missing.push(cursor.to_path_buf());
        cursor = cursor
            .parent()
            .ok_or_else(|| format!("{label} has no existing parent"))?;
    }
    std::fs::create_dir_all(path).map_err(|e| format!("{label} directory: {e}"))?;
    for created in missing.iter().rev() {
        sync_directory(created)?;
        if let Some(parent) = created.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

fn write_new_synced(path: &Path, contents: &[u8], label: &str) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{label} create: {e}"))?;
    if let Err(e) = file.write_all(contents) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("{label} write: {e}"));
    }
    if let Err(e) = file.sync_all() {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("{label} sync: {e}"));
    }
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn persist_append_marker(
    dir: &Path,
    name: &str,
    contents: &[u8],
    label: &str,
) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| format!("{label} directory: {e}"))?;
    let path = dir.join(name);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(contents).and_then(|_| file.sync_all()) {
                drop(file);
                let _ = std::fs::remove_file(&path);
                let _ = sync_directory(dir);
                return Err(format!("{label} persist: {e}"));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let file =
                std::fs::File::open(&path).map_err(|e| format!("{label} repair open: {e}"))?;
            if !file
                .metadata()
                .map_err(|e| format!("{label} repair metadata: {e}"))?
                .is_file()
            {
                return Err(format!("{label} marker is not a regular file"));
            }
            file.sync_all()
                .map_err(|e| format!("{label} repair sync: {e}"))?;
        }
        Err(e) => return Err(format!("{label} create: {e}")),
    }
    sync_directory(dir)?;
    if let Some(parent) = dir.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

/// Flush every extracted regular file before READY becomes durable. The archive
/// already has an entry cap; this second cap protects against unexpected local
/// filesystem contents if cleanup was interrupted.
fn sync_tree(root: &Path) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    let mut nodes = 0usize;
    while let Some(dir) = stack.pop() {
        nodes += 1;
        if nodes > 8192 {
            return Err("bundle tree exceeds sync safety limit".into());
        }
        directories.push(dir.clone());
        for entry in std::fs::read_dir(&dir).map_err(|e| format!("sync tree read: {e}"))? {
            let entry = entry.map_err(|e| format!("sync tree entry: {e}"))?;
            let kind = entry
                .file_type()
                .map_err(|e| format!("sync tree type: {e}"))?;
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                nodes += 1;
                if nodes > 8192 {
                    return Err("bundle tree exceeds sync safety limit".into());
                }
                std::fs::File::open(entry.path())
                    .and_then(|file| file.sync_all())
                    .map_err(|e| format!("sync bundle file: {e}"))?;
            } else {
                return Err("bundle tree contains a non-regular entry".into());
            }
        }
    }
    for dir in directories.into_iter().rev() {
        sync_directory(&dir)?;
    }
    Ok(())
}

fn normalized_version(value: &str) -> Option<String> {
    Some(
        version_parts(value)?
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn read_watermark<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<String, String> {
    let mut best = match std::fs::read_to_string(watermark_file(app)) {
        Ok(value) => normalized_version(value.trim())
            .ok_or_else(|| "legacy OTA watermark is invalid".to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("legacy OTA watermark read: {e}")),
    };
    let entries = match std::fs::read_dir(watermark_dir(app)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(best),
        Err(e) => return Err(format!("OTA watermark directory read: {e}")),
    };
    for entry in entries {
        let entry = entry.map_err(|e| format!("OTA watermark entry: {e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("OTA watermark type: {e}"))?
            .is_file()
        {
            return Err("OTA watermark entry is not a regular file".into());
        }
        let name = entry.file_name();
        let name = name
            .to_str()
            .and_then(normalized_version)
            .ok_or_else(|| "OTA watermark entry has an invalid version".to_string())?;
        if best.is_empty() || version_gt(&name, &best) {
            best = name;
        }
    }
    Ok(best)
}

fn bundle_version_at(dir: &Path) -> String {
    if !dir.join(READY_MARKER).is_file() || !dir.join("index.html").is_file() {
        return String::new();
    }
    std::fs::read_to_string(dir.join(BUNDLE_VERSION_MARKER))
        .ok()
        .and_then(|value| normalized_version(value.trim()))
        .unwrap_or_default()
}

fn bundle_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    bundle_version_at(&current_dir(app))
}

fn next_bundle_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    bundle_version_at(&next_dir(app))
}

fn marker_version_at(dir: &Path, marker: &str) -> String {
    std::fs::read_to_string(dir.join(marker))
        .ok()
        .and_then(|value| normalized_version(value.trim()))
        .unwrap_or_default()
}

fn current_rejection_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    let version = marker_version_at(&current_dir(app), BUNDLE_VERSION_MARKER);
    if version.is_empty() {
        marker_version_at(&current_dir(app), TRIAL_MARKER)
    } else {
        version
    }
}

fn next_rejection_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    let version = marker_version_at(&next_dir(app), BUNDLE_VERSION_MARKER);
    if version.is_empty() {
        marker_version_at(&next_dir(app), TRIAL_MARKER)
    } else {
        version
    }
}

fn highest_installed_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    let current = bundle_version(app);
    let next = next_bundle_version(app);
    if current.is_empty() || (!next.is_empty() && version_gt(&next, &current)) {
        next
    } else {
        current
    }
}

/// Only return a version that the resolver may serve. Rejected, stale, and
/// not-yet-durably-started trial bundles fail closed even if cleanup fails.
fn current_bundle_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    let current = current_dir(app);
    let version = bundle_version(app);
    let trial = current.join(TRIAL_MARKER);
    let pending = current.join(PENDING_MARKER);
    let trial_version = marker_version_at(&current, TRIAL_MARKER);
    if WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst)
        || version.is_empty()
        || !web_version_compatible(&version, &app.package_info().version.to_string())
        || !version_gte(&version, &app.package_info().version.to_string())
        || is_rejected(app, &version)
        || (trial.exists()
            && (!trial.is_file()
                || !pending.is_file()
                || version_cmp(&trial_version, &version) != Some(std::cmp::Ordering::Equal)))
    {
        return String::new();
    }
    version
}

fn is_rejected<R: Runtime>(app: &tauri::AppHandle<R>, version: &str) -> bool {
    let Some(version) = normalized_version(version) else {
        return true;
    };
    match std::fs::metadata(rejected_dir(app).join(version)) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => true,
    }
}

fn reject_version<R: Runtime>(app: &tauri::AppHandle<R>, version: &str) -> Result<(), String> {
    let version =
        normalized_version(version).ok_or_else(|| "invalid rejected OTA version".to_string())?;
    persist_append_marker(
        &rejected_dir(app),
        &version,
        version.as_bytes(),
        "rejected-version",
    )
}

fn advance_watermark<R: Runtime>(app: &tauri::AppHandle<R>, candidate: &str) -> Result<(), String> {
    let candidate =
        normalized_version(candidate).ok_or_else(|| "invalid OTA watermark version".to_string())?;
    let current = read_watermark(app)?;
    if current.is_empty() || version_gte(&candidate, &current) {
        persist_append_marker(
            &watermark_dir(app),
            &candidate,
            candidate.as_bytes(),
            "watermark",
        )?;
    }
    Ok(())
}

/// Move live bytes out of the resolver's fixed path before best-effort cleanup.
/// A locked file may leave a quarantined directory behind, but cannot keep
/// rejected code live under `web/current`.
fn detach_bundle_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    target: &Path,
    label: &str,
) -> Result<(), String> {
    if !target.exists() {
        return Ok(());
    }
    let base = web_base(app);
    std::fs::create_dir_all(&base).map_err(|e| format!("web directory: {e}"))?;
    let quarantine = base.join(format!("quarantine-{label}-{}", uuid::Uuid::new_v4()));
    match std::fs::rename(target, &quarantine) {
        Ok(()) => {
            sync_directory(&base)?;
            let _ = std::fs::remove_dir_all(&quarantine);
            let _ = sync_directory(&base);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("detach {label} web bundle: {e}")),
    }
}

fn detach_current_bundle<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    detach_bundle_dir(app, &current_dir(app), "current")
}

fn detach_next_bundle<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    detach_bundle_dir(app, &next_dir(app), "next")
}

fn ensure_secure_ota_state<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    let _state_guard = lock_ota_state();
    let sentinel = secure_schema_file(app);
    match std::fs::read(&sentinel) {
        Ok(contents) if contents == SECURE_SCHEMA_CONTENTS => {
            read_cache_epoch(app)?;
            match std::fs::remove_file(initializing_file(app)) {
                Ok(()) => sync_directory(&web_base(app))?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("clear OTA initialization marker: {e}")),
            }
            return Ok(());
        }
        Ok(_) => return Err("signed OTA schema marker is invalid".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("signed OTA schema marker read: {e}")),
    }

    let initializing = initializing_file(app);
    match std::fs::read(&initializing) {
        Ok(contents) if contents == SECURE_SCHEMA_CONTENTS => {}
        Ok(_) => return Err("OTA initialization marker is invalid".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            for entry in std::fs::read_dir(web_base(app))
                .map_err(|e| format!("inspect uninitialized OTA namespace: {e}"))?
            {
                let entry = entry.map_err(|e| format!("inspect OTA namespace entry: {e}"))?;
                if entry.file_name().to_str() != Some(".process.lock") {
                    return Err("signed OTA sentinel is missing from a non-empty namespace".into());
                }
            }
            write_new_synced(
                &initializing,
                SECURE_SCHEMA_CONTENTS,
                "OTA initialization marker",
            )?;
        }
        Err(e) => return Err(format!("OTA initialization marker read: {e}")),
    }

    for (target, label) in [
        (current_dir(app), "incomplete-secure-current"),
        (next_dir(app), "incomplete-secure-next"),
        (staging_dir(app), "incomplete-secure-staging"),
        (watermark_dir(app), "incomplete-secure-watermarks"),
        (rejected_dir(app), "incomplete-secure-rejections"),
        (web_root(app).join("current"), "unsigned-current"),
        (web_root(app).join("next"), "unsigned-next"),
        (web_root(app).join("staging"), "unsigned-staging"),
        (web_root(app).join("high-watermarks"), "unsigned-watermarks"),
        (
            web_root(app).join("rejected-versions"),
            "unsigned-rejections",
        ),
    ] {
        detach_bundle_dir(app, &target, label)?;
    }
    for path in [
        version_file(app),
        watermark_file(app),
        cache_epoch_file(app),
        web_root(app).join("version.txt"),
        web_root(app).join("high-watermark.txt"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                if let Some(parent) = path.parent() {
                    sync_directory(parent)?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "remove legacy OTA metadata {}: {e}",
                    path.display()
                ))
            }
        }
    }
    let epoch = uuid::Uuid::new_v4().hyphenated().to_string();
    write_new_synced(&cache_epoch_file(app), epoch.as_bytes(), "OTA cache epoch")?;
    write_new_synced(
        &sentinel,
        SECURE_SCHEMA_CONTENTS,
        "signed OTA schema marker",
    )?;
    std::fs::remove_file(&initializing)
        .map_err(|e| format!("clear OTA initialization marker: {e}"))?;
    sync_directory(&web_base(app))
}

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

/// URI -> a strictly relative asset path. URI paths are decoded before the
/// component checks so encoded traversal cannot become dangerous later.
fn rel_path(uri: &str) -> Option<String> {
    let after_scheme = uri.splitn(2, "://").nth(1).unwrap_or(uri);
    let path = after_scheme.splitn(2, '/').nth(1).unwrap_or("");
    let path = path.split(['?', '#']).next().unwrap_or("");
    if path.is_empty() {
        return Some("index.html".to_string());
    }
    let decoded = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()?;
    if decoded.starts_with(['/', '\\'])
        || decoded.contains('\\')
        || decoded.chars().any(|c| c.is_control())
    {
        return None;
    }
    let mut parts = Vec::new();
    for part in decoded.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return None;
        }
        parts.push(part);
    }
    Some(parts.join("/"))
}

/// The origin selected for this WebView lifetime. Once an OTA is selected we
/// never fall back per-file to embedded assets: a missing/corrupt OTA file must
/// fail and trigger rollback, not create a mixed-version page.
#[cfg(any(target_os = "android", target_os = "macos"))]
fn selected_ota_version<R: Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.try_state::<BootGuard>().and_then(|guard| {
        guard
            .expected
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    })
}

#[cfg(any(target_os = "android", target_os = "macos"))]
fn selected_boot_nonce<R: Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.try_state::<BootGuard>().and_then(|guard| {
        guard
            .nonce
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    })
}

fn boot_nonce_is_canonical(nonce: &str) -> bool {
    nonce.len() == 36
        && uuid::Uuid::parse_str(nonce)
            .map(|parsed| parsed.hyphenated().to_string() == nonce && parsed.get_version_num() == 4)
            .unwrap_or(false)
}

fn ota_document_nonce(url: &tauri::Url) -> Option<String> {
    let custom_origin = url.port().is_none()
        && ((url.scheme() == "hanniweb" && url.host_str() == Some("localhost"))
            || (url.scheme() == "http" && url.host_str() == Some("hanniweb.localhost")));
    if !custom_origin || url.path() != "/index.html" || url.query().is_some() {
        return None;
    }
    let nonce = url.fragment()?.strip_prefix("hanni-ota-boot=")?;
    if boot_nonce_is_canonical(nonce) {
        Some(nonce.to_string())
    } else {
        None
    }
}

fn select_embedded_origin<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(guard) = app.try_state::<BootGuard>() {
        *guard
            .expected
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        *guard
            .nonce
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        *guard
            .loaded_nonce
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        guard.confirmed.store(true, Ordering::SeqCst);
    }
}

/// Resolve to (bytes, mime, csp) from the single origin selected before
/// navigation. Rejects `..` traversal out of the bundle dir.
#[cfg(any(target_os = "android", target_os = "macos"))]
fn resolve<R: Runtime>(
    app: &tauri::AppHandle<R>,
    rel: &str,
) -> Option<(Vec<u8>, String, Option<String>)> {
    let traversal = rel.split('/').any(|c| c == "..");
    if traversal {
        return None;
    }
    if let Some(expected) = selected_ota_version(app) {
        if version_cmp(&current_bundle_version(app), &expected) != Some(std::cmp::Ordering::Equal) {
            return None;
        }
        let bytes = std::fs::read(current_dir(app).join(rel)).ok()?;
        return Some((bytes, mime_for(rel).to_string(), embedded_csp(app)));
    }
    app.asset_resolver()
        .get(rel.to_string())
        .map(|a| (a.bytes, a.mime_type, a.csp_header))
}

/// CSP from the embedded index.html so OTA-served HTML carries the same policy.
#[cfg(any(target_os = "android", target_os = "macos"))]
fn embedded_csp<R: Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.asset_resolver()
        .get("index.html".to_string())
        .and_then(|a| a.csp_header)
}

#[cfg(any(target_os = "android", target_os = "macos"))]
fn not_found() -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(404)
        .body(b"not found".to_vec())
        .unwrap()
}

/// The version currently being served — the applied OTA bundle's version if one
/// is live, else the native (embedded) version. Drives the protocol ETag so the
/// WebView's cache invalidates exactly when the served content changes.
#[cfg(any(target_os = "android", target_os = "macos"))]
fn serve_version<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    let epoch = read_cache_epoch(app).unwrap_or_else(|_| {
        VOLATILE_CACHE_EPOCH
            .get_or_init(|| uuid::Uuid::new_v4().hyphenated().to_string())
            .clone()
    });
    if let Some(expected) = selected_ota_version(app) {
        if version_cmp(&current_bundle_version(app), &expected) == Some(std::cmp::Ordering::Equal) {
            return format!("hanni-v2:{epoch}:ota:{expected}");
        }
        return format!("hanni-v2:{epoch}:ota-invalid:{expected}");
    }
    format!("hanni-v2:{epoch}:native:{}", app.package_info().version)
}

/// Registers the OTA web-asset protocol on the builder (Android + macOS).
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    use tauri::http::{Request, Response};
    use tauri::UriSchemeContext;
    builder.register_uri_scheme_protocol(
        SCHEME,
        move |ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>| {
            let app = ctx.app_handle();
            let Some(rel) = rel_path(&request.uri().to_string()) else {
                return not_found();
            };
            // Keep the eligibility decision, ETag response, and bytes read in
            // one state snapshot so rollback cannot yield a stale 304.
            let _state_guard = lock_ota_state();
            // Cache + revalidate via an ETag tied to the served version (not no-store).
            // no-store forced the WebView to re-fetch + re-parse all ~120 JS modules
            // on every cold start (multi-second freeze after Android evicts the app
            // from the background). With an ETag the WebView caches the parsed modules
            // and revalidates cheaply: matching If-None-Match → 304 → reuse cache (no
            // re-read/re-parse). An applied OTA bundle bumps version.txt → the ETag
            // changes → the cache busts and the new bundle is fetched once.
            let etag = format!("\"{}\"", serve_version(app));
            if request
                .headers()
                .get("If-None-Match")
                .and_then(|v| v.to_str().ok())
                == Some(etag.as_str())
            {
                return Response::builder()
                    .status(304)
                    .header("ETag", etag)
                    .header("Cache-Control", "no-cache")
                    .body(Vec::new())
                    .unwrap_or_else(|_| not_found());
            }
            match resolve(app, &rel) {
                Some((bytes, mime, csp)) => {
                    let mut b = Response::builder()
                        .status(200)
                        .header("Content-Type", mime)
                        .header("Cross-Origin-Resource-Policy", "same-origin")
                        .header("X-Content-Type-Options", "nosniff")
                        .header("Cache-Control", "no-cache")
                        .header("ETag", etag);
                    if let Some(csp) = csp {
                        b = b.header("Content-Security-Policy", csp);
                    }
                    b.body(bytes).unwrap_or_else(|_| not_found())
                }
                None => not_found(),
            }
        },
    )
}

// ───────────────────────── OTA update (all platforms) ──────────────────────

fn version_parts(value: &str) -> Option<Vec<u64>> {
    let value = value.trim().trim_start_matches('v');
    if value.is_empty() || value.len() > 64 {
        return None;
    }
    let parts: Vec<&str> = value.split('.').collect();
    if parts.is_empty() || parts.len() > 8 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    parts
        .into_iter()
        .map(|p| {
            if p.chars().all(|c| c.is_ascii_digit()) {
                p.parse::<u64>().ok()
            } else {
                None
            }
        })
        .collect()
}

fn version_cmp(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let (a, b) = (version_parts(a)?, version_parts(b)?);
    for i in 0..a.len().max(b.len()) {
        let order = a
            .get(i)
            .copied()
            .unwrap_or(0)
            .cmp(&b.get(i).copied().unwrap_or(0));
        if order != std::cmp::Ordering::Equal {
            return Some(order);
        }
    }
    Some(std::cmp::Ordering::Equal)
}

fn version_gte(a: &str, b: &str) -> bool {
    matches!(
        version_cmp(a, b),
        Some(std::cmp::Ordering::Equal | std::cmp::Ordering::Greater)
    )
}

fn version_gt(a: &str, b: &str) -> bool {
    version_cmp(a, b) == Some(std::cmp::Ordering::Greater)
}

fn required_native_version(web_version: &str) -> Option<String> {
    let parts = version_parts(web_version)?;
    if parts.len() != 3 && !(parts.len() == 4 && parts[3] > 0) {
        return None;
    }
    Some(
        parts[..3]
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn web_version_compatible(web_version: &str, native_version: &str) -> bool {
    required_native_version(web_version)
        .is_some_and(|required| version_gte(native_version, &required))
}

fn update_is_admissible(
    candidate: &str,
    native: &str,
    watermark: &str,
    applied: &str,
    rejected: bool,
) -> bool {
    if rejected
        || normalized_version(candidate).as_deref() != Some(candidate)
        || !web_version_compatible(candidate, native)
    {
        return false;
    }
    let mut floor = native;
    if version_gt(watermark, floor) {
        floor = watermark;
    }
    if version_gt(applied, floor) {
        floor = applied;
    }
    version_gt(candidate, floor)
        || (version_cmp(candidate, watermark) == Some(std::cmp::Ordering::Equal)
            && version_gt(watermark, native)
            && (applied.is_empty() || version_gt(watermark, applied)))
}

#[derive(serde::Serialize)]
pub struct WebUpdate {
    pub available: bool,
    pub web_version: String,
    #[serde(skip_serializing)]
    pub url: String,
    #[serde(skip_serializing)]
    pub sha256: String,
    #[serde(skip_serializing)]
    pub asset_size: u64,
}

const RELEASES_API: &str = "https://api.github.com/repos/sultanjakhan/hanni/releases/latest";
const UPDATER_PUBLIC_KEY: &str = include_str!("../updater.pub");
const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const MAX_SIGNATURE_BYTES: usize = 8 * 1024;
const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WebManifest {
    schema: String,
    repository: String,
    channel: String,
    release_tag: String,
    web_version: String,
    min_native_version: String,
    sequence: u64,
    source_commit: String,
    manifest_asset: String,
    asset: String,
    asset_size: u64,
    asset_sha256: String,
}

async fn fetch_limited(
    client: &reqwest::Client,
    url: &str,
    label: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;
    let response = client
        .get(url)
        .header("User-Agent", "Hanni-Web-OTA")
        .send()
        .await
        .map_err(|e| format!("{label}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("{label} status: {e}"))?;
    if response
        .content_length()
        .is_some_and(|n| n > max_bytes as u64)
    {
        return Err(format!("{label} is too large"));
    }
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("{label}: {e}"))?;
        if out.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("{label} is too large"));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

fn verify_manifest_signature(
    manifest_name: &str,
    manifest: &[u8],
    signature_asset: &[u8],
) -> Result<(), String> {
    use base64::Engine;
    let encoded = std::str::from_utf8(signature_asset)
        .map_err(|_| "manifest signature is not UTF-8")?
        .trim();
    let signature_box = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "manifest signature outer base64 is invalid")?;
    let signature_text =
        std::str::from_utf8(&signature_box).map_err(|_| "manifest signature box is not UTF-8")?;
    if signature_text.lines().count() != 4 {
        return Err("unexpected minisign signature format".into());
    }
    let public_key = minisign_verify::PublicKey::decode(UPDATER_PUBLIC_KEY)
        .map_err(|e| format!("OTA public key: {e}"))?;
    let signature = minisign_verify::Signature::decode(signature_text)
        .map_err(|e| format!("manifest signature: {e}"))?;
    public_key
        .verify(manifest, &signature, false)
        .map_err(|e| format!("manifest signature verification failed: {e}"))?;
    let expected_suffix = format!("\tfile:{manifest_name}");
    let trusted = signature.trusted_comment();
    if !trusted.starts_with("timestamp:") || !trusted.ends_with(&expected_suffix) {
        return Err("manifest signature is bound to another filename".into());
    }
    Ok(())
}

fn manifest_version_from_name(name: &str) -> Option<&str> {
    name.strip_prefix("web-manifest-")?
        .strip_suffix(".json")
        .filter(|v| version_parts(v).is_some())
}

async fn fetch_verified_web_update<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<WebUpdate, String> {
    let none = || WebUpdate {
        available: false,
        web_version: String::new(),
        url: String::new(),
        sha256: String::new(),
        asset_size: 0,
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let rel: serde_json::Value = client
        .get(RELEASES_API)
        .header("User-Agent", "Hanni-Web-OTA")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("release fetch: {e}"))?
        .error_for_status()
        .map_err(|e| format!("release status: {e}"))?
        .json()
        .await
        .map_err(|e| format!("release JSON: {e}"))?;
    let release_tag = rel.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
    let assets = rel
        .get("assets")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    let has_asset = |name: &str| {
        assets
            .iter()
            .any(|asset| asset.get("name").and_then(|n| n.as_str()) == Some(name))
    };
    let asset_url = |tag: &str, name: &str| {
        format!("https://github.com/sultanjakhan/hanni/releases/download/{tag}/{name}")
    };

    // Versioned names are intentional. Old native clients only recognize the
    // legacy `web-manifest.json`, which was unsigned; publishing that name again
    // would let those clients install the new frontend without verification.
    let mut candidates: Vec<(String, String)> = assets
        .iter()
        .filter_map(|asset| {
            let name = asset.get("name")?.as_str()?;
            Some((
                name.to_string(),
                manifest_version_from_name(name)?.to_string(),
            ))
        })
        .collect();
    candidates.sort_by(|a, b| version_cmp(&b.1, &a.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(32);

    let native = app.package_info().version.to_string();
    let watermark = read_watermark(app)?;
    let applied = highest_installed_version(app);

    for (manifest_name, filename_version) in candidates {
        let signature_name = format!("{manifest_name}.sig");
        if !has_asset(&signature_name) {
            continue;
        }
        let manifest_url = asset_url(release_tag, &manifest_name);
        let signature_url = asset_url(release_tag, &signature_name);
        let manifest_bytes =
            fetch_limited(&client, &manifest_url, "manifest", MAX_MANIFEST_BYTES).await?;
        let signature_bytes = fetch_limited(
            &client,
            &signature_url,
            "manifest signature",
            MAX_SIGNATURE_BYTES,
        )
        .await?;
        if verify_manifest_signature(&manifest_name, &manifest_bytes, &signature_bytes).is_err() {
            continue;
        }
        let manifest: WebManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| format!("signed manifest JSON: {e}"))?;
        let Some(web_parts) = version_parts(&manifest.web_version) else {
            continue;
        };
        let Some(native_parts) = version_parts(&manifest.min_native_version) else {
            continue;
        };
        if native_parts.len() != 3 {
            continue;
        }
        let expected_sequence = if web_parts.len() == 3 && web_parts == native_parts {
            0
        } else if web_parts.len() == 4 && web_parts[..3] == native_parts[..] && web_parts[3] > 0 {
            web_parts[3]
        } else {
            continue;
        };
        let commit_ok = manifest.source_commit.len() == 40
            && manifest
                .source_commit
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        let commit_short = manifest.source_commit.get(..12).unwrap_or("");
        let expected_asset = format!("web-{}-{}.tar.gz", manifest.web_version, commit_short);
        if manifest.schema != "hanni.web-ota.v1"
            || manifest.repository != "sultanjakhan/hanni"
            || manifest.channel != "stable"
            || manifest.release_tag != release_tag
            || manifest.release_tag != format!("v{}", manifest.min_native_version)
            || normalized_version(&manifest.web_version).as_deref()
                != Some(manifest.web_version.as_str())
            || normalized_version(&manifest.min_native_version).as_deref()
                != Some(manifest.min_native_version.as_str())
            || manifest.web_version != filename_version
            || manifest.sequence != expected_sequence
            || !commit_ok
            || manifest.manifest_asset != manifest_name
            || manifest.asset != expected_asset
            || manifest.asset_size == 0
            || manifest.asset_size > MAX_BUNDLE_BYTES as u64
            || manifest.asset_sha256.len() != 64
            || !manifest
                .asset_sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            || !has_asset(&manifest.asset)
        {
            continue;
        }
        let url = asset_url(&manifest.release_tag, &manifest.asset);
        let compatible = version_gte(&native, &manifest.min_native_version);
        let admissible = update_is_admissible(
            &manifest.web_version,
            &native,
            &watermark,
            &applied,
            WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst)
                || is_rejected(app, &manifest.web_version),
        );
        return Ok(WebUpdate {
            available: compatible && admissible,
            web_version: manifest.web_version,
            url,
            sha256: manifest.asset_sha256,
            asset_size: manifest.asset_size,
        });
    }
    Ok(none())
}

/// Applied web-bundle version + the native (APK) version.
#[tauri::command]
pub async fn web_ota_status<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<serde_json::Value, String> {
    let applied = current_bundle_version(&app);
    let watermark = read_watermark(&app)?;
    Ok(serde_json::json!({
        "applied": applied,
        "native": app.package_info().version.to_string(),
        "watermark": watermark,
    }))
}

/// Check the latest GitHub release for a newer, compatible signed web bundle.
#[tauri::command]
pub async fn web_ota_check<R: Runtime>(app: tauri::AppHandle<R>) -> Result<WebUpdate, String> {
    fetch_verified_web_update(&app).await
}

/// Download → verify sha256 → extract → atomic swap into web/current.
/// On any failure the existing bundle (or embedded fallback) stays intact.
#[tauri::command]
pub async fn web_ota_apply<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let _guard = WEB_OTA_APPLY_LOCK.lock().await;
    if !acquire_ota_process_lease(&app) {
        return Err("another process owns web OTA state".into());
    }
    // Never trust URL/version/hash supplied by the WebView. Re-fetch and verify
    // the signed manifest inside the privileged Rust boundary immediately
    // before downloading the bundle.
    let update = fetch_verified_web_update(&app).await?;
    if !update.available {
        return Err("no newer compatible signed web update".into());
    }
    let WebUpdate {
        url,
        web_version,
        sha256,
        asset_size,
        ..
    } = update;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = fetch_limited(&client, &url, "bundle download", MAX_BUNDLE_BYTES).await?;
    if bytes.len() as u64 != asset_size {
        return Err(format!(
            "bundle size mismatch: got {}, want {asset_size}",
            bytes.len()
        ));
    }

    // Verify integrity before touching disk.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(&sha256) {
        return Err(format!("sha256 mismatch: got {got}, want {sha256}"));
    }

    let staging = staging_dir(&app);
    match std::fs::remove_dir_all(&staging) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("clear staging: {e}")),
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("mkdir staging: {e}"))?;

    // Extract tar.gz (must be flat: index.html at root, no top-level dir).
    // Reject symlinks/hardlinks and any entry escaping staging — defense in depth
    // on top of sha256 so a tampered bundle can't drop a link out of the dir.
    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);
    let mut extracted_bytes: u64 = 0;
    let mut entry_count: usize = 0;
    for entry in archive
        .entries()
        .map_err(|e| format!("read archive: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("entry: {e}"))?;
        entry_count += 1;
        extracted_bytes = extracted_bytes.saturating_add(entry.header().size().unwrap_or(u64::MAX));
        if entry_count > 4096 || extracted_bytes > 128 * 1024 * 1024 {
            let _ = std::fs::remove_dir_all(&staging);
            return Err("bundle expands beyond safety limits".into());
        }
        match entry.header().entry_type() {
            tar::EntryType::Regular | tar::EntryType::Directory => {
                if !entry
                    .unpack_in(&staging)
                    .map_err(|e| format!("extract: {e}"))?
                {
                    let _ = std::fs::remove_dir_all(&staging);
                    return Err("bundle entry escapes staging dir".into());
                }
            }
            _ => continue, // skip symlinks, hardlinks, devices, fifos
        }
    }

    // Validate the bundle has an entry point before swapping it in.
    if !staging.join("index.html").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("bundle missing index.html".into());
    }
    sync_tree(&staging)?;
    write_new_synced(
        &staging.join(BUNDLE_VERSION_MARKER),
        web_version.as_bytes(),
        "bundle version marker",
    )?;
    // Trial: the bundle serves on next launch but must prove it boots (the
    // frontend calls web_ota_boot_ok) or verify_trial_on_boot reverts it.
    write_new_synced(
        &staging.join(TRIAL_MARKER),
        web_version.as_bytes(),
        "trial marker",
    )?;
    // READY is always last: a crash before this point cannot expose a partial
    // extraction as a complete bundle.
    write_new_synced(&staging.join(READY_MARKER), b"", "ready marker")?;

    // Re-check the persistent high-water mark after download/extract. The apply
    // mutex makes this an effective replay/downgrade gate even if the WebView
    // triggers multiple concurrent invokes.
    let _state_guard = lock_ota_state();
    let native = app.package_info().version.to_string();
    let watermark = read_watermark(&app)?;
    let applied = highest_installed_version(&app);
    if !update_is_admissible(
        &web_version,
        &native,
        &watermark,
        &applied,
        WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst) || is_rejected(&app, &web_version),
    ) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("web update is rejected or below the OTA watermark".into());
    }

    // Persist the rollback floor before exposing new bytes. Append-only marker
    // files cannot be truncated by a later in-place overwrite.
    advance_watermark(&app, &web_version)?;

    // Publish into `next`, never into the directory backing the live page. The
    // next startup promotes it before navigation, so one WebView lifetime can
    // never mix modules from two frontend versions.
    let next = next_dir(&app);
    detach_next_bundle(&app)?;
    std::fs::rename(&staging, &next).map_err(|e| format!("publish next bundle: {e}"))?;
    sync_directory(&web_base(&app))?;
    Ok(())
}

/// Promote a verified bundle only during startup, before the WebView navigates
/// to the custom scheme. The live `current` tree then stays immutable for the
/// entire page lifetime.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn promote_next_bundle<R: Runtime>(app: &tauri::AppHandle<R>) {
    let _state_guard = lock_ota_state();
    if !owns_ota_process_lease() || WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst) {
        return;
    }
    let next = next_dir(app);
    if !next.exists() {
        return;
    }
    let candidate = next_bundle_version(app);
    let native = app.package_info().version.to_string();
    let watermark = match read_watermark(app) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("[hanni] web_assets: cannot validate pending OTA watermark: {e}");
            return;
        }
    };
    let current = bundle_version(app);
    let trial = marker_version_at(&next, TRIAL_MARKER);
    let admissible = !candidate.is_empty()
        && next.join(TRIAL_MARKER).is_file()
        && !next.join(PENDING_MARKER).exists()
        && version_cmp(&trial, &candidate) == Some(std::cmp::Ordering::Equal)
        && !is_rejected(app, &candidate)
        && web_version_compatible(&candidate, &native)
        && version_gt(&candidate, &native)
        && version_cmp(&candidate, &watermark) == Some(std::cmp::Ordering::Equal)
        && (current.is_empty() || version_gt(&candidate, &current));
    if !admissible {
        if let Err(e) = detach_next_bundle(app) {
            eprintln!("[hanni] web_assets: cannot discard inadmissible pending bundle: {e}");
        }
        return;
    }
    if let Err(e) = detach_current_bundle(app) {
        eprintln!("[hanni] web_assets: cannot detach current bundle before promotion: {e}");
        return;
    }
    if let Err(e) = std::fs::rename(&next, current_dir(app)) {
        eprintln!("[hanni] web_assets: cannot promote pending bundle: {e}");
        return;
    }
    if let Err(e) = sync_directory(&web_base(app)) {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        eprintln!("[hanni] web_assets: cannot sync promoted bundle: {e}");
    }
}

/// Capture the exact OTA version selected before navigating the WebView. A
/// later frontend confirmation may only commit this version, never a bundle
/// swapped in concurrently after the page started loading.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn begin_boot_session<R: Runtime>(app: &tauri::AppHandle<R>) {
    let _state_guard = lock_ota_state();
    if !owns_ota_process_lease() {
        return;
    }
    let expected = match current_bundle_version(app) {
        version if version.is_empty() => None,
        version => Some(version),
    };
    let nonce = expected
        .as_ref()
        .map(|_| uuid::Uuid::new_v4().hyphenated().to_string());
    if let Some(guard) = app.try_state::<BootGuard>() {
        guard.confirmed.store(false, Ordering::SeqCst);
        *guard
            .expected
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = expected;
        *guard
            .nonce
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = nonce;
        *guard
            .loaded_nonce
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

/// Bind the boot nonce to a completed top-level navigation. The nonce lives in
/// the URL fragment, which is not sent to the custom-protocol responder and
/// therefore cannot be recovered by cross-origin fetching `index.html`.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn note_page_load<R: Runtime>(
    webview: &tauri::Webview<R>,
    payload: &tauri::webview::PageLoadPayload<'_>,
) {
    if webview.label() != "main"
        || payload.event() != tauri::webview::PageLoadEvent::Finished
        || !owns_ota_process_lease()
    {
        return;
    }
    let Some(nonce) = ota_document_nonce(payload.url()) else {
        return;
    };
    let app = webview.app_handle();
    let _state_guard = lock_ota_state();
    let Some(guard) = app.try_state::<BootGuard>() else {
        return;
    };
    let expected_nonce = guard
        .nonce
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let expected_version = guard
        .expected
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if expected_nonce.as_deref() == Some(nonce.as_str())
        && expected_version
            .as_deref()
            .map(|expected| {
                version_cmp(&current_bundle_version(app), expected)
                    == Some(std::cmp::Ordering::Equal)
            })
            .unwrap_or(false)
    {
        *guard
            .loaded_nonce
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(nonce);
    }
}

/// Trial-boot safety net — call once at startup BEFORE navigating to the OTA
/// bundle. If the current bundle is a trial that already had its one boot
/// attempt (the pending marker survived), it white-screened last launch → drop
/// it so this launch falls back to embedded assets. An append-only rejected
/// marker prevents re-downloading the same bad bundle.
#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn reject_failed_trial_on_startup<R: Runtime>(app: &tauri::AppHandle<R>) {
    if !owns_ota_process_lease() || WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst) {
        return;
    }
    let _state_guard = lock_ota_state();
    let current = current_dir(app);
    if !current.join(TRIAL_MARKER).exists() || !current.join(PENDING_MARKER).exists() {
        return;
    }
    let version = current_rejection_version(app);
    if version.is_empty() {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        eprintln!(
            "[hanni] web_assets: failed trial has no trustworthy version; keeping bytes disabled"
        );
        return;
    }
    if !web_version_compatible(&version, &app.package_info().version.to_string()) {
        return;
    }
    let rejected = match reject_version(app, &version) {
        Ok(()) => true,
        Err(e) => {
            WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
            eprintln!("[hanni] web_assets: could not persist rejected trial: {e}");
            false
        }
    };
    if rejected {
        if let Err(e) = detach_current_bundle(app) {
            eprintln!("[hanni] web_assets: failed to detach rejected trial: {e}");
        } else {
            let _ = std::fs::remove_file(version_file(app));
        }
    }
}

#[cfg(any(target_os = "android", target_os = "macos"))]
pub fn prepare_current_trial_for_boot<R: Runtime>(app: &tauri::AppHandle<R>) {
    if !owns_ota_process_lease() || WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst) {
        return;
    }
    let _state_guard = lock_ota_state();
    let current = current_dir(app);
    if !current.join(TRIAL_MARKER).exists() || current.join(PENDING_MARKER).exists() {
        return;
    }
    let version = bundle_version(app);
    let trial_version = marker_version_at(&current, TRIAL_MARKER);
    let native = app.package_info().version.to_string();
    let watermark = match read_watermark(app) {
        Ok(value) => value,
        Err(e) => {
            WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
            eprintln!("[hanni] web_assets: cannot validate trial watermark: {e}");
            return;
        }
    };
    if version.is_empty()
        || version_cmp(&trial_version, &version) != Some(std::cmp::Ordering::Equal)
        || !web_version_compatible(&version, &native)
        || !version_gte(&version, &native)
        || !version_gte(&watermark, &version)
        || is_rejected(app, &version)
    {
        return;
    }
    if let Err(e) = write_new_synced(&current.join(PENDING_MARKER), b"", "trial pending marker") {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        let version = current_rejection_version(app);
        let rejected = if version.is_empty() {
            false
        } else {
            match reject_version(app, &version) {
                Ok(()) => true,
                Err(reject_error) => {
                    eprintln!("[hanni] web_assets: could not reject trial after marker failure: {reject_error}");
                    false
                }
            }
        };
        if rejected {
            if let Err(detach_error) = detach_current_bundle(app) {
                eprintln!("[hanni] web_assets: could not detach disabled trial: {detach_error}");
            }
        }
        eprintln!("[hanni] web_assets: could not persist trial boot state; bundle disabled: {e}");
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
    if !owns_ota_process_lease() || WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst) {
        return;
    }
    let _state_guard = lock_ota_state();
    let native = app.package_info().version.to_string();
    let applied = bundle_version(app);
    let has_bundle = current_dir(app).join(READY_MARKER).exists();
    if let Err(e) = advance_watermark(app, &native) {
        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
        eprintln!("[hanni] web_assets: could not advance native OTA watermark: {e}");
        return;
    }
    if WEB_OTA_SERVING_DISABLED.load(Ordering::SeqCst) {
        return;
    }
    // A genuine web update (>= native) sitting on top of this shell → keep it.
    if has_bundle
        && version_gte(&applied, &native)
        && web_version_compatible(&applied, &native)
        && !is_rejected(app, &applied)
    {
        if let Err(e) = advance_watermark(app, &applied) {
            WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
            eprintln!("[hanni] web_assets: could not repair applied OTA watermark: {e}");
            return;
        }
        return;
    }
    if has_bundle {
        if let Err(e) = detach_current_bundle(app) {
            eprintln!("[hanni] web_assets: could not detach stale or rejected OTA bundle: {e}");
        }
    }
    let _ = std::fs::remove_file(version_file(app));
}

/// Called by the frontend once it has loaded successfully. Confirms a trial
/// bundle by clearing its markers so it's kept permanently. No-op otherwise.
#[tauri::command]
pub fn web_ota_boot_ok<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    nonce: String,
) -> Result<(), String> {
    // Cancel the boot watchdog: the frontend confirmed a real paint, so the
    // served bundle is good — don't revert it.
    let _state_guard = lock_ota_state();
    if !owns_ota_process_lease() {
        return Err("this process does not own web OTA state".into());
    }
    if window.label() != "main" {
        return Err("web boot confirmation came from another window".into());
    }
    let url = window
        .url()
        .map_err(|e| format!("cannot verify web boot origin: {e}"))?;
    if ota_document_nonce(&url).as_deref() != Some(nonce.as_str()) {
        return Err("web boot confirmation came from another document".into());
    }
    let app = window.app_handle();
    let Some(guard) = app.try_state::<BootGuard>() else {
        return Err("web boot guard is unavailable".into());
    };
    let expected = guard
        .expected
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let Some(expected) = expected else {
        return Err("no OTA bundle is awaiting boot confirmation".into());
    };
    if !boot_nonce_is_canonical(&nonce) {
        return Err("invalid web boot confirmation".into());
    }
    let expected_nonce = guard
        .nonce
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .ok_or_else(|| "web boot nonce is unavailable".to_string())?;
    if nonce != expected_nonce {
        return Err("web boot confirmation does not match this document".into());
    }
    let loaded_nonce = guard
        .loaded_nonce
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if loaded_nonce.as_deref() != Some(nonce.as_str()) {
        return Err("OTA document has not finished loading".into());
    }
    let current_version = current_bundle_version(app);
    if version_cmp(&current_version, &expected) != Some(std::cmp::Ordering::Equal) {
        return Err("web boot confirmation does not match the served OTA version".into());
    }
    let current = current_dir(app);
    // TRIAL is the commit marker. Remove and sync it first; once this succeeds
    // the bundle is durably confirmed and a stale PENDING marker is harmless.
    match std::fs::remove_file(current.join(TRIAL_MARKER)) {
        Ok(()) => sync_directory(&current)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("confirm OTA trial: {e}")),
    }
    guard.confirmed.store(true, Ordering::SeqCst);
    match std::fs::remove_file(current.join(PENDING_MARKER)) {
        Ok(()) => {
            if let Err(e) = sync_directory(&current) {
                eprintln!(
                    "[hanni] web_assets: confirmed OTA but could not sync pending cleanup: {e}"
                );
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!("[hanni] web_assets: confirmed OTA but could not clear pending marker: {e}");
        }
    }
    Ok(())
}

/// Arm a one-shot watchdog after navigating to the OTA bundle. If the frontend
/// hasn't confirmed a real paint (`web_ota_boot_ok` → `BootGuard`) within
/// `WATCHDOG_SECS` while an OTA bundle is actually live, the bundle white-screened
/// → delete it and reload so embedded assets serve (same-launch recovery, no
/// reinstall/wipe). No-op when embedded assets already serve (nothing to revert),
/// so it can never loop. The trial marker (next-launch revert) stays as a backstop.
#[cfg(any(target_os = "android", target_os = "macos"))]
#[allow(dead_code)] // armed on Android only; compiled on macOS so cargo check covers it
pub fn arm_boot_watchdog<R: Runtime>(app: &tauri::AppHandle<R>) {
    if !owns_ota_process_lease() {
        return;
    }
    let expected = app.try_state::<BootGuard>().and_then(|guard| {
        guard
            .expected
            .lock()
            .ok()
            .and_then(|version| version.clone())
    });
    let Some(expected) = expected else { return };
    if !current_dir(app).join(READY_MARKER).exists() {
        return; // embedded assets are the floor — nothing to fall back to
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(WATCHDOG_SECS));
        let alive = handle
            .try_state::<BootGuard>()
            .map(|guard| guard.confirmed.load(Ordering::SeqCst))
            .unwrap_or(true);
        if alive {
            return;
        }
        let _state_guard = lock_ota_state();
        let alive = handle
            .try_state::<BootGuard>()
            .map(|guard| guard.confirmed.load(Ordering::SeqCst))
            .unwrap_or(true);
        let current = current_bundle_version(&handle);
        if alive || version_cmp(&current, &expected) != Some(std::cmp::Ordering::Equal) {
            return;
        }
        // The recovery reload starts a new embedded page lifetime. Switch the
        // in-memory selector before any fallible durable cleanup so every
        // failure mode still fails closed to embedded bytes rather than 404.
        select_embedded_origin(&handle);
        let rejected = match reject_version(&handle, &expected) {
            Ok(()) => true,
            Err(e) => {
                WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
                eprintln!("[hanni] web_assets: could not persist rejected OTA version: {e}");
                false
            }
        };
        if rejected {
            if let Err(e) = detach_current_bundle(&handle) {
                eprintln!("[hanni] web_assets: could not detach failed OTA bundle: {e}");
            } else {
                let _ = std::fs::remove_file(version_file(&handle));
            }
        }
        eprintln!(
            "[hanni] web_assets: boot watchdog — OTA bundle never painted, reverted to embedded"
        );
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if let Some(win) = h.get_webview_window("main") {
                let _ = win.eval("window.location.reload()");
            }
        });
    });
}

/// Emergency reset: drop any applied OTA bundle so the next load serves the
/// embedded assets. Manual recovery path for a device stuck on a bad bundle.
#[tauri::command]
pub async fn web_reset_bundle<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    // Serialize with download/extract/publish so a reset cannot be undone by an
    // update that was already in flight but not yet visible in `next`.
    let _apply_guard = WEB_OTA_APPLY_LOCK.lock().await;
    if !acquire_ota_process_lease(&app) {
        return Err("another process owns web OTA state".into());
    }
    let outcome = {
        let _state_guard = lock_ota_state();
        select_embedded_origin(&app);
        (|| -> Result<(), String> {
            let current = current_dir(&app);
            if current.join(READY_MARKER).exists() || current.join(TRIAL_MARKER).exists() {
                let version = current_rejection_version(&app);
                if version.is_empty() {
                    WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
                    return Err(
                        "cannot durably reject current bundle without a trustworthy version".into(),
                    );
                }
                if let Err(e) = reject_version(&app, &version) {
                    WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
                    return Err(e);
                }
            }
            if next_dir(&app).exists() {
                let version = next_rejection_version(&app);
                if !version.is_empty() {
                    if let Err(e) = reject_version(&app, &version) {
                        WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
                        return Err(e);
                    }
                }
            }
            detach_current_bundle(&app)?;
            detach_next_bundle(&app)?;
            let _ = std::fs::remove_file(version_file(&app));
            // Deliberately keep append-only watermark/rejection markers so
            // emergency rollback cannot reopen an older or bad bundle.
            Ok(())
        })()
    };
    // The selector changed page origin; reload on both success and failure so
    // the old OTA page cannot fetch a mixed set of embedded modules.
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = handle.get_webview_window("main") {
            if let Err(e) = win.eval("window.location.reload()") {
                WEB_OTA_SERVING_DISABLED.store(true, Ordering::SeqCst);
                eprintln!("[hanni] web_assets: reset reload failed: {e}");
            }
        }
    });
    outcome
}

#[cfg(test)]
mod web_asset_security_tests {
    use super::*;

    #[test]
    fn ota_uri_paths_stay_relative() {
        assert_eq!(rel_path("hanniweb://localhost/"), Some("index.html".into()));
        assert_eq!(
            rel_path("http://hanniweb.localhost/js/app.js?x=1"),
            Some("js/app.js".into())
        );
        for attack in [
            "hanniweb://localhost//etc/passwd",
            "hanniweb://localhost/../secret",
            "hanniweb://localhost/%2e%2e/secret",
            "hanniweb://localhost/js\\..\\secret",
            "hanniweb://localhost/C:/Windows/win.ini",
            "hanniweb://localhost/js//app.js",
        ] {
            assert_eq!(rel_path(attack), None, "{attack}");
        }
    }

    #[test]
    fn boot_nonce_is_bound_to_exact_ota_document_url() {
        let nonce = "123e4567-e89b-42d3-a456-426614174000";
        let android = tauri::Url::parse(&format!(
            "http://hanniweb.localhost/index.html#hanni-ota-boot={nonce}"
        ))
        .unwrap();
        let mac = tauri::Url::parse(&format!(
            "hanniweb://localhost/index.html#hanni-ota-boot={nonce}"
        ))
        .unwrap();
        assert_eq!(ota_document_nonce(&android).as_deref(), Some(nonce));
        assert_eq!(ota_document_nonce(&mac).as_deref(), Some(nonce));
        for rejected in [
            format!("http://evil.localhost/index.html#hanni-ota-boot={nonce}"),
            format!("http://hanniweb.localhost/other.html#hanni-ota-boot={nonce}"),
            format!("http://hanniweb.localhost/index.html?x=1#hanni-ota-boot={nonce}"),
            format!("http://hanniweb.localhost/index.html#x=1&hanni-ota-boot={nonce}"),
            "http://hanniweb.localhost/index.html#hanni-ota-boot=123e4567-e89b-12d3-a456-426614174000".into(),
        ] {
            assert_eq!(ota_document_nonce(&tauri::Url::parse(&rejected).unwrap()), None);
        }
    }

    #[test]
    fn watermark_versions_are_canonicalized() {
        assert_eq!(normalized_version("v1.2.03"), Some("1.2.3".into()));
        assert_eq!(normalized_version("1.2.x"), None);
        assert!(version_gt("1.2.4", "1.2.3"));
    }

    #[test]
    fn accepted_update_can_retry_but_rejected_or_applied_one_cannot() {
        assert!(update_is_admissible(
            "1.2.3.1", "1.2.3", "1.2.3.1", "1.2.3", false
        ));
        assert!(!update_is_admissible(
            "1.2.3.1", "1.2.3", "1.2.3.1", "1.2.3.1", false
        ));
        assert!(!update_is_admissible(
            "01.2.3.1", "1.2.3", "1.2.3.1", "1.2.3", false
        ));
        assert!(!update_is_admissible(
            "1.2.3.1", "1.2.3", "1.2.3.1", "1.2.3", true
        ));
        assert!(!update_is_admissible(
            "1.2.3.2", "1.2.3", "1.2.3.1", "", true
        ));
        assert!(!update_is_admissible(
            "1.2.3", "1.2.3", "1.2.3.1", "", false
        ));
        assert!(!update_is_admissible(
            "1.2.4.1", "1.2.5", "1.2.4.1", "", false
        ));
        assert!(!update_is_admissible(
            "1.2.3.2", "1.2.3", "1.2.3.1", "1.2.3.3", false
        ));
        assert!(update_is_admissible(
            "1.2.3.2", "1.2.3", "1.2.3.1", "1.2.3.1", false
        ));
        assert!(!update_is_admissible(
            "1.2.4.1", "1.2.3", "1.2.3", "", false
        ));
        assert!(web_version_compatible("1.2.3.9", "1.2.3"));
        assert!(web_version_compatible("1.2.3.9", "1.2.4"));
        assert!(!web_version_compatible("1.2.4.1", "1.2.3"));
    }
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

fn origin_stage_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_root(app).join("origin_stage")
}
fn ls_dump_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_root(app).join("ls_dump.json")
}
fn origin_pending_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_root(app).join("origin_pending")
}
/// Native version recorded when the switch was disabled — used to auto-retry
/// once a newer shell is installed.
fn origin_native_file<R: Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    web_root(app).join("origin_native")
}

fn read_origin_stage<R: Runtime>(app: &tauri::AppHandle<R>) -> String {
    std::fs::read_to_string(origin_stage_file(app))
        .unwrap_or_default()
        .trim()
        .to_string()
}
fn write_origin_stage<R: Runtime>(app: &tauri::AppHandle<R>, stage: &str) {
    let _ = std::fs::create_dir_all(web_root(app));
    let _ = std::fs::write(origin_stage_file(app), stage.as_bytes());
}
/// Consecutive unconfirmed-boot strikes (absent file = 0). web_origin_ok clears
/// it each successful launch, so only repeated failures accumulate.
fn read_pending_strikes<R: Runtime>(app: &tauri::AppHandle<R>) -> u32 {
    std::fs::read_to_string(origin_pending_file(app))
        .ok()
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
        let at = std::fs::read_to_string(origin_native_file(app))
            .unwrap_or_default()
            .trim()
            .to_string();
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
                let _ = std::fs::create_dir_all(web_root(app));
                let _ = std::fs::write(
                    origin_pending_file(app),
                    (strikes + 1).to_string().as_bytes(),
                );
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
    let _ = std::fs::create_dir_all(web_root(&app));
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
