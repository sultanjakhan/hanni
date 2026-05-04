// health_connect_plugin.rs — Tauri mobile plugin bridge for Health Connect
// Registers the Kotlin HealthConnectPlugin with Tauri's plugin system.

use tauri::{plugin::{Builder, TauriPlugin}, Runtime};

#[cfg(target_os = "android")]
use tauri::{Manager, plugin::PluginHandle};

#[cfg(target_os = "android")]
pub struct HealthConnectHandle<R: Runtime>(pub PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("health-connect")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(
                    "com.sultanjakhan.hanni", "HealthConnectPlugin"
                )?;
                app.manage(HealthConnectHandle(handle));
            }
            #[cfg(not(target_os = "android"))]
            {
                let _ = (app, api);
            }
            Ok(())
        })
        .build()
}

/// Returns whether all required Health Connect permissions are granted.
/// On non-Android platforms always returns false (no Health Connect available).
#[tauri::command]
pub async fn health_has_permissions<R: Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let handle = app.state::<HealthConnectHandle<R>>();
        match handle.0.run_mobile_plugin::<serde_json::Value>("hasPermissions", &()) {
            Ok(v) => Ok(v.get("granted").and_then(|g| g.as_bool()).unwrap_or(false)),
            Err(e) => Err(format!("{e}")),
        }
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Ok(false) }
}

/// Triggers the Health Connect permission UI on Android. Resolves with the
/// final granted state once the user closes the system dialog.
/// Debug helper — returns the raw JSON response from a Kotlin plugin
/// command, so we can see whether Health Connect is actually returning
/// data or just an empty list.
#[tauri::command]
pub async fn health_debug_read<R: Runtime>(
    app: tauri::AppHandle<R>,
    cmd: String,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let handle = app.state::<HealthConnectHandle<R>>();
        match handle.0.run_mobile_plugin::<serde_json::Value>(&cmd, &()) {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("{e}")),
        }
    }
    #[cfg(not(target_os = "android"))]
    { let _ = (app, cmd); Err("Android-only".into()) }
}

#[tauri::command]
pub async fn health_request_permissions<R: Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let handle = app.state::<HealthConnectHandle<R>>();
        match handle.0.run_mobile_plugin::<serde_json::Value>("requestHealthPermissions", &()) {
            Ok(v) => Ok(v.get("granted").and_then(|g| g.as_bool()).unwrap_or(false)),
            Err(e) => Err(format!("{e}")),
        }
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Err("Health Connect is only available on Android".into()) }
}
