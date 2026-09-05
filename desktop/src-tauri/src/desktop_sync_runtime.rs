//! Separate no-WebView sync runtime in the same executable.
//! Mode changes relaunch only after Exit, so interactive background tasks do not
//! survive a return to SyncOnly. Native handoff tests are required before release.

use std::{ffi::OsString, sync::Mutex};
use tauri::{AppHandle, Manager, RunEvent, Wry};
use tauri_plugin_autostart::ManagerExt;

const SYNC_ONLY: &str = "--sync-only";
const ENABLE_AUTOSTART: &str = "--enable-sync-autostart";
const DISABLE_AUTOSTART: &str = "--disable-sync-autostart";
const MODE_EXIT_CODE: i32 = 73;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Interactive,
    SyncOnly,
}

struct Lifecycle {
    mode: Mode,
    next: Mutex<Option<Mode>>,
}

pub(crate) fn sync_only_requested() -> bool {
    std::env::args_os().any(|arg| arg == SYNC_ONLY)
}

pub(crate) fn configure(builder: tauri::Builder<Wry>, mode: Mode) -> tauri::Builder<Wry> {
    // Isolated dev must neither claim the production singleton nor alter login.
    if crate::types::is_isolated_dev() {
        return builder;
    }
    builder
        .manage(Lifecycle {
            mode,
            next: Mutex::new(None),
        })
        // Register before application plugins/setup: the secondary exits before
        // startup jobs are spawned. Both modes use the same bundle identifier.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if apply_startup_options(app, &args).is_err() {
                eprintln!("[sync-runtime] autostart configuration failed");
                return;
            }
            if !args.iter().any(|arg| arg == SYNC_ONLY) {
                open_interactive(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![SYNC_ONLY]),
        ))
}

pub(crate) fn setup(app: &AppHandle) -> Result<(), String> {
    if crate::types::is_isolated_dev() {
        return Ok(());
    }
    // The UI singleton plugin may fail open during simultaneous startup.
    // This OS lease is independent of Tauri cleanup and held until process exit.
    let data_dir = crate::types::hanni_data_dir();
    crate::secure_fs::ensure_private_dir(&data_dir)
        .map_err(|_| "hanni_process_lease_unavailable")?;
    crate::desktop_process_lease::acquire_for_process(&data_dir)?;
    apply_startup_options(app, &std::env::args().collect::<Vec<_>>())
}

fn apply_startup_options(app: &AppHandle, args: &[String]) -> Result<(), String> {
    let enable = args.iter().any(|arg| arg == ENABLE_AUTOSTART);
    let disable = args.iter().any(|arg| arg == DISABLE_AUTOSTART);
    if enable && disable {
        return Err("Conflicting autostart options".into());
    }
    if enable || disable {
        set_autostart(app, enable)?;
    }
    Ok(())
}

fn set_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    // Debug and isolated builds must never become the production login target.
    // The caller must provision from the installed release executable.
    if cfg!(debug_assertions) || crate::types::is_isolated_dev() {
        return Err("Autostart requires an installed release build".into());
    }
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|_| "Autostart configuration failed".to_string())
}

fn request_mode(app: &AppHandle, mode: Mode) {
    let Some(lifecycle) = app.try_state::<Lifecycle>() else {
        return;
    };
    let mut next = lifecycle
        .next
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if next.is_some() {
        return;
    }
    *next = Some(mode);
    drop(next);
    // Request Exit rather than calling process::restart in a menu callback:
    // single-instance's Exit hook must release its IPC endpoint first.
    app.exit(MODE_EXIT_CODE);
}

fn open_interactive(app: &AppHandle) {
    let Some(lifecycle) = app.try_state::<Lifecycle>() else {
        return;
    };
    if lifecycle.mode == Mode::SyncOnly {
        request_mode(app, Mode::Interactive);
    } else if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn restart_environment(app: &AppHandle, mode: Mode) -> tauri::Env {
    let mut env = app.env();
    // Do not replay provisioning flags, URL input or arbitrary launch arguments.
    env.args_os.truncate(1);
    if mode == Mode::SyncOnly {
        env.args_os.push(OsString::from(SYNC_ONLY));
    }
    env
}

pub(crate) fn on_event(app: &AppHandle, event: &RunEvent) {
    let Some(lifecycle) = app.try_state::<Lifecycle>() else {
        return;
    };
    match event {
        RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } if label == "main" && lifecycle.mode == Mode::Interactive => {
            if app.autolaunch().is_enabled().unwrap_or(false) {
                api.prevent_close();
                request_mode(app, Mode::SyncOnly);
            }
        }
        RunEvent::ExitRequested {
            code: None, api, ..
        } if lifecycle.mode == Mode::SyncOnly => {
            // No windows is intentional. Explicit Quit has a numeric exit code.
            api.prevent_exit();
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => open_interactive(app),
        RunEvent::Exit => {
            let mode = lifecycle
                .next
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
            if let Some(mode) = mode {
                let env = restart_environment(app, mode);
                // Tauri delivers plugin Exit hooks before this callback. The
                // singleton is already released; do not destroy it twice.
                app.cleanup_before_exit();
                tauri::process::restart(&env);
            }
        }
        _ => {}
    }
}

pub(crate) fn run_sync_only() -> tauri::Result<()> {
    if crate::types::is_isolated_dev() {
        return Err(tauri::Error::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Isolated dev cannot start production sync",
        )));
    }
    let mut context = tauri::generate_context!();
    // Clearing windows BEFORE build is essential: hidden main.html would still
    // initialize JS voice polling, wakeword and other interactive code.
    context.config_mut().app.windows.clear();
    configure(tauri::Builder::default(), Mode::SyncOnly)
        .setup(|app| {
            setup(app.handle()).map_err(std::io::Error::other)?;
            // This path never enters the interactive run() initialization below.
            // A login/close handoff must not run interactive migrations, seeds,
            // deduplication or calendar cleanup against the user's database.
            // An uninitialized/older schema requires opening the normal app.
            let path = crate::types::hanni_data_dir().join("hanni.db");
            let path = path
                .to_str()
                .ok_or_else(|| std::io::Error::other("relay_invalid_path"))?;
            let writer = crate::cloud_relay::open_existing(path).map_err(std::io::Error::other)?;
            let reader = crate::cloud_relay::open_existing(path).map_err(std::io::Error::other)?;
            reader
                .pragma_update(None, "query_only", "ON")
                .map_err(std::io::Error::other)?;
            app.manage(writer.into_hanni_db(reader));
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let open = tauri::menu::MenuItem::with_id(
                app,
                "sync.open",
                "Открыть Hanni",
                true,
                None::<&str>,
            )?;
            let quit = tauri::menu::MenuItem::with_id(
                app,
                "sync.quit",
                "Завершить Hanni",
                true,
                None::<&str>,
            )?;
            let menu = tauri::menu::Menu::with_items(app, &[&open, &quit])?;
            let mut tray = tauri::tray::TrayIconBuilder::with_id("hanni-sync")
                .tooltip("Hanni — фоновая синхронизация")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "sync.open" => open_interactive(app),
                    "sync.quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            #[cfg(target_os = "macos")]
            {
                tray = tray.title("Hanni");
            }
            let _ = tray.build(app)?;

            // Honor configured transports; no endpoint or credentials are
            // provisioned here and no transport is implicitly enabled.
            crate::cloud_relay::start_background_sync(app.handle());
            crate::sync_owner_auto::start_auto_sync_loop(app.handle().clone());
            let lan = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                crate::lan_sync::spawn_lan_sync_server(lan).await;
            });
            crate::lan_sync::start_lan_sync_loop(app.handle().clone());
            Ok(())
        })
        .build(context)?
        .run(|app, event| on_event(app, &event));
    Ok(())
}
