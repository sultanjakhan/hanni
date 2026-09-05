// bg_sync.rs — Tauri bridge to the Kotlin BackgroundSyncPlugin.
// Schedules / cancels HanniHealthWorker (WorkManager periodic), so Hanni
// keeps pulling from Health Connect + pushing to Mac even when the app
// process isn't running.

use serde::Serialize;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[cfg(target_os = "android")]
use tauri::{plugin::PluginHandle, Manager};

#[cfg(target_os = "android")]
pub struct BackgroundSyncHandle<R: Runtime>(pub PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("background-sync")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api.register_android_plugin(
                    "com.sultanjakhan.hanni", "BackgroundSyncPlugin"
                )?;
                app.manage(BackgroundSyncHandle(handle));
            }
            #[cfg(not(target_os = "android"))]
            { let _ = app; }
            Ok(())
        })
        .build()
}

#[derive(Serialize)]
struct ScheduleArgs {
    #[serde(rename = "intervalMinutes")]
    interval_minutes: u64,
}

#[derive(Serialize)]
struct RelayConfigArgs { config: String }

/// Explicit local pairing metadata. No credentials, provider access or health rows.
#[tauri::command]
pub async fn cloud_relay_pairing_source<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value,String> {
    #[cfg(target_os = "android")]
    {
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<BackgroundSyncHandle<R>>().0
                .run_mobile_plugin::<serde_json::Value>("getRelayPairingSource", &())
                .map_err(|_| "relay_pairing_source_unavailable".to_string())
        }).await.map_err(|_| "relay_pairing_source_unavailable".to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(serde_json::json!({"supported":false}))
    }
}

#[tauri::command]
pub async fn cloud_relay_set_config<R: Runtime>(app: tauri::AppHandle<R>, config: String) -> Result<serde_json::Value,String> {
    #[cfg(target_os = "android")]
    {
        app.state::<BackgroundSyncHandle<R>>().0.run_mobile_plugin("setRelayConfig",RelayConfigArgs { config })
            .map_err(|_| "relay_configuration_failed".into())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _=app;
        tauri::async_runtime::spawn_blocking(move || crate::cloud_relay_runtime::cloud_relay_set_config(config))
            .await.map_err(|_| "relay_configuration_failed")?
    }
}

#[tauri::command]
pub async fn cloud_relay_status<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value,String> {
    #[cfg(target_os = "android")]
    {
        let mut status: serde_json::Value=app.state::<BackgroundSyncHandle<R>>().0.run_mobile_plugin("relaySyncStatus",())
            .map_err(|_| "relay_status_unavailable")?;
        let delivery=tauri::async_runtime::spawn_blocking(move || {
            let db=app.state::<crate::types::HanniDb>();
            let conn = db.conn();
            crate::cloud_relay::database_status(&conn)
        }).await.map_err(|_| "relay_status_unavailable")??;
        status.as_object_mut().ok_or("relay_status_unavailable")?
            .extend(delivery.as_object().ok_or("relay_status_unavailable")?.clone());
        Ok(status)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _=app;
        tauri::async_runtime::spawn_blocking(crate::cloud_relay_runtime::cloud_relay_status)
            .await.map_err(|_| "relay_status_unavailable")?
    }
}

#[tauri::command]
pub async fn cloud_relay_sync_now<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value,String> {
    #[cfg(target_os = "android")]
    {
        app.state::<BackgroundSyncHandle<R>>().0.run_mobile_plugin("runRelaySyncOnce",())
            .map_err(|_| "relay_enqueue_failed".into())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _=app;
        crate::cloud_relay_runtime::request_sync();
        Ok(serde_json::json!({"enqueued":true}))
    }
}

/// Schedule the periodic Health Connect sync. The minimum on Android is
/// 15 min — anything lower is silently bumped up by WorkManager. Returns
/// the actual interval the OS will honour.
#[tauri::command]
pub async fn bg_sync_enable<R: Runtime>(
    app: tauri::AppHandle<R>,
    interval_minutes: Option<u64>,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let h = app.state::<BackgroundSyncHandle<R>>();
        let args = ScheduleArgs { interval_minutes: interval_minutes.unwrap_or(15).max(15) };
        h.0.run_mobile_plugin::<serde_json::Value>("scheduleBackgroundSync", &args)
            .map_err(|e| format!("{e}"))
    }
    #[cfg(not(target_os = "android"))]
    { let _ = (app, interval_minutes); Ok(serde_json::json!({"scheduled": false, "reason": "android-only"})) }
}

#[tauri::command]
pub async fn bg_sync_disable<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let h = app.state::<BackgroundSyncHandle<R>>();
        h.0.run_mobile_plugin::<serde_json::Value>("cancelBackgroundSync", &())
            .map_err(|e| format!("{e}"))
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Ok(serde_json::json!({"cancelled": false, "reason": "android-only"})) }
}

/// Run the background worker once immediately (diagnostic / manual sync).
#[tauri::command]
pub async fn bg_sync_run_once<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let h = app.state::<BackgroundSyncHandle<R>>();
        h.0.run_mobile_plugin::<serde_json::Value>("runBackgroundSyncOnce", &())
            .map_err(|e| format!("{e}"))
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Ok(serde_json::json!({"enqueued": false, "reason": "android-only"})) }
}

#[tauri::command]
pub async fn bg_sync_status<R: Runtime>(app: tauri::AppHandle<R>) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let h = app.state::<BackgroundSyncHandle<R>>();
        h.0.run_mobile_plugin::<serde_json::Value>("backgroundSyncStatus", &())
            .map_err(|e| format!("{e}"))
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Ok(serde_json::json!({"active": false})) }
}
