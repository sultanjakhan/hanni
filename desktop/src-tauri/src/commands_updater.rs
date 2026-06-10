// commands_updater.rs — App updater — check/run/restart + startup auto-check
use crate::types::*;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;
use std::process::Command;
use std::path::PathBuf;

// ── Updater ──

pub fn updater_with_headers(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    // Public repo — no auth headers needed. Direct download URLs work without them.
    app.updater_builder()
        .build()
        .map_err(|e| format!("Updater error: {}", e))
}

fn updater_log(msg: &str) {
    use std::io::Write;
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{}] {}\n", ts, msg);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(hanni_data_dir().join("updater.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

// Walk up from the current executable to find the enclosing `.app` bundle.
// Returns None when not running from a bundle (e.g. `cargo run`).
fn current_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut cur = exe.parent()?;
    loop {
        if cur.extension().and_then(|s| s.to_str()) == Some("app") {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

// Strip quarantine xattr from the bundle so Gatekeeper doesn't block the
// freshly-replaced ad-hoc-signed app (we have no Apple Developer ID).
fn clear_quarantine(bundle: &std::path::Path) {
    let status = Command::new("xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(bundle)
        .status();
    updater_log(&format!("xattr -dr com.apple.quarantine {} -> {:?}", bundle.display(), status));
}

async fn run_update(
    app: &AppHandle,
    update: tauri_plugin_updater::Update,
) -> Result<String, String> {
    use std::sync::{Arc, Mutex};

    let version = update.version.clone();
    let _ = app.emit("update-available", &version);
    updater_log(&format!("available: v{}", version));

    let downloaded = Arc::new(Mutex::new(0u64));
    let total = Arc::new(Mutex::new(0u64));
    let last_percent = Arc::new(Mutex::new(-1i64));

    let app_chunk = app.clone();
    let dl_r = downloaded.clone();
    let tot_r = total.clone();
    let pct_r = last_percent.clone();

    let app_finish = app.clone();

    let res = update
        .download_and_install(
            move |chunk_len, content_len| {
                let mut d = dl_r.lock().unwrap();
                *d += chunk_len as u64;
                if let Some(t) = content_len {
                    *tot_r.lock().unwrap() = t;
                }
                let t = *tot_r.lock().unwrap();
                let pct = if t > 0 { ((*d * 100) / t) as i64 } else { 0 };
                let mut lp = pct_r.lock().unwrap();
                if pct != *lp {
                    *lp = pct;
                    let _ = app_chunk.emit(
                        "update-progress",
                        serde_json::json!({
                            "downloaded": *d,
                            "total": t,
                            "percent": pct,
                        }),
                    );
                }
            },
            move || {
                updater_log("download finished, installing");
                let _ = app_finish.emit("update-installing", ());
            },
        )
        .await;

    match res {
        Ok(()) => {
            updater_log(&format!("install ok: v{}", version));
            if let Some(bundle) = current_app_bundle() {
                clear_quarantine(&bundle);
            }
            let _ = app.emit("update-ready", &version);
            Ok(format!("Готово — перезапусти Hanni для v{}.", version))
        }
        Err(e) => {
            let msg = format!("Ошибка установки: {}", e);
            updater_log(&msg);
            let _ = app.emit("update-error", &msg);
            Err(msg)
        }
    }
}

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn is_debug_build() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<String, String> {
    let updater = updater_with_headers(&app)?;
    updater_log("check_update: started");
    match updater.check().await {
        Ok(Some(update)) => run_update(&app, update).await,
        Ok(None) => {
            updater_log("check_update: up to date");
            Ok("Вы на последней версии.".into())
        }
        Err(e) => {
            let msg = format!("Не удалось проверить обновления: {}", e);
            updater_log(&msg);
            Err(msg)
        }
    }
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    updater_log("restart_app: triggered by user");
    app.restart();
}

// Called by lib.rs setup — background auto-check at startup.
#[cfg(not(target_os = "android"))]
pub async fn auto_check_on_startup(app: AppHandle) {
    let updater = match updater_with_headers(&app) {
        Ok(u) => u,
        Err(e) => {
            updater_log(&format!("auto: builder error: {}", e));
            return;
        }
    };
    updater_log("auto: startup check");
    match updater.check().await {
        Ok(Some(update)) => {
            let _ = run_update(&app, update).await;
        }
        Ok(None) => updater_log("auto: up to date"),
        Err(e) => updater_log(&format!("auto: check error: {}", e)),
    }
}

