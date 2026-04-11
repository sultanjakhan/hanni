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
