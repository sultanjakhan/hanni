//! Desktop scheduling and OS-protected credentials, separate from relay core.
use crate::cloud_relay::{open_existing, run_headless_once, RelayConfig};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest, protocol::WebSocketConfig, Message,
};

static STARTED: AtomicBool = AtomicBool::new(false);
#[derive(Default)]
struct ConfigCache {
    value: Option<Option<RelayConfig>>,
    // A locked/denied Keychain is temporary; never cache it as unconfigured.
    retry_after: Option<Instant>,
}
static CONFIG: OnceLock<Mutex<ConfigCache>> = OnceLock::new();
static WAKE: OnceLock<Arc<tokio::sync::Notify>> = OnceLock::new();
fn wake() -> Arc<tokio::sync::Notify> {
    WAKE.get_or_init(|| Arc::new(tokio::sync::Notify::new()))
        .clone()
}
fn config_cache() -> &'static Mutex<ConfigCache> {
    CONFIG.get_or_init(|| Mutex::new(ConfigCache::default()))
}
pub(crate) fn request_sync() {
    wake().notify_one();
}
fn due(elapsed: Duration, local_pending: bool, inbound_pending: bool, previous_ok: bool) -> bool {
    elapsed >= Duration::from_secs(300)
        || ((local_pending || inbound_pending)
            && elapsed >= Duration::from_secs(if previous_ok { 5 } else { 30 }))
}

#[cfg(target_os = "macos")]
fn noninteractive_keychain_options() -> security_framework::passwords::PasswordOptions {
    use core_foundation::{
        base::TCFType,
        string::{CFString, CFStringRef},
    };
    use security_framework::passwords::PasswordOptions;
    #[link(name = "Security", kind = "framework")]
    extern "C" {
        static kSecUseAuthenticationUI: CFStringRef;
        static kSecUseAuthenticationUIFail: CFStringRef;
    }
    let mut options =
        PasswordOptions::new_generic_password("com.sultanjakhan.hanni.relay", "device-v1");
    // security-framework 3.7 exposes no setter for this per-query policy.
    // Both symbols are process-lifetime CFString constants owned by Security;
    // wrap_under_get_rule retains them for the options dictionary.
    // No process-global Keychain interaction setting is changed.
    #[allow(deprecated)]
    unsafe {
        options.query.push((
            CFString::wrap_under_get_rule(kSecUseAuthenticationUI),
            CFString::wrap_under_get_rule(kSecUseAuthenticationUIFail).into_CFType(),
        ));
    }
    options
}

fn read_config() -> Result<Option<RelayConfig>, String> {
    let mut cache = config_cache().lock().map_err(|_| "relay_config_busy")?;
    if let Some(value) = cache.value.as_ref() {
        return Ok(value.clone());
    }
    if cache
        .retry_after
        .is_some_and(|deadline| Instant::now() < deadline)
    {
        return Err("relay_credentials_unavailable".into());
    }
    #[cfg(windows)]
    let raw = {
        let path = crate::types::hanni_data_dir().join("cloud-relay.credentials");
        if !path.exists() {
            None
        } else {
            Some(
                crate::secret_store::read_file(&path)
                    .map_err(|_| "relay_credentials_unavailable")?,
            )
        }
    };
    #[cfg(target_os = "macos")]
    let raw = {
        use security_framework::passwords::generic_password;
        match generic_password(noninteractive_keychain_options()) {
            Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| "relay_credentials_invalid")?),
            Err(error) if error.code() == -25300 => None, // errSecItemNotFound only
            Err(_) => {
                cache.retry_after = Some(Instant::now() + Duration::from_secs(30));
                return Err("relay_credentials_unavailable".into());
            }
        }
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let raw: Option<String> = None;
    let cfg = raw.as_deref().map(RelayConfig::parse).transpose()?;
    cache.retry_after = None;
    cache.value = Some(cfg.clone());
    Ok(cfg)
}

pub(crate) fn cloud_relay_set_config(config: String) -> Result<Value, String> {
    if crate::types::is_isolated_dev() {
        return Err("relay_disabled_in_isolated_dev".into());
    }
    let cfg = RelayConfig::parse(&config)?;
    if let Some(old) = read_config()? {
        if old.endpoint.trim_end_matches('/') != cfg.endpoint.trim_end_matches('/')
            || old.device_id != cfg.device_id
            || old.key_id != cfg.key_id
            || old.key != cfg.key
            || old.sleep_source_store_id != cfg.sleep_source_store_id
        {
            return Err("relay_pairing_changed".into());
        }
    }
    let encoded = serde_json::to_string(&cfg).map_err(|_| "relay_config_invalid")?;
    #[cfg(windows)]
    crate::secret_store::write_file(
        &crate::types::hanni_data_dir().join("cloud-relay.credentials"),
        &encoded,
    )
    .map_err(|_| "relay_credentials_write_failed")?;
    #[cfg(target_os = "macos")]
    security_framework::passwords::set_generic_password(
        "com.sultanjakhan.hanni.relay",
        "device-v1",
        encoded.as_bytes(),
    )
    .map_err(|_| "relay_credentials_write_failed")?;
    #[cfg(not(any(windows, target_os = "macos")))]
    return Err("relay_platform_unsupported".into());
    *config_cache().lock().map_err(|_| "relay_config_busy")? = ConfigCache {
        value: Some(Some(cfg)),
        retry_after: None,
    };
    wake().notify_one();
    Ok(json!({"configured":true}))
}

pub(crate) fn cloud_relay_status() -> Result<Value, String> {
    if crate::types::is_isolated_dev() {
        return Ok(json!({"configured":false,"isolated":true}));
    }
    let Some(cfg) = read_config()? else {
        return Ok(json!({"configured":false}));
    };
    let path = crate::types::hanni_data_dir().join("hanni.db");
    let conn = open_existing(path.to_str().ok_or("relay_invalid_path")?)?;
    let mut status = crate::cloud_relay::database_status(&conn)?;
    let fields = status.as_object_mut().ok_or("relay_status_unavailable")?;
    fields.insert("configured".into(), json!(true));
    fields.insert("enabled".into(), json!(cfg.enabled));
    Ok(status)
}

async fn stream(cfg: RelayConfig, notify: Arc<tokio::sync::Notify>) {
    let mut delay = 5;
    loop {
        let url = format!("{}/v1/stream", cfg.endpoint.trim_end_matches('/'))
            .replacen("https://", "wss://", 1);
        let Ok(mut request) = url.into_client_request() else {
            return;
        };
        let Ok(header) = format!("Bearer {}", cfg.token).parse() else {
            return;
        };
        request.headers_mut().insert("Authorization", header);
        let mut limits = WebSocketConfig::default();
        limits.max_message_size = Some(4096);
        limits.max_frame_size = Some(4096);
        let connection = tokio::time::timeout(
            Duration::from_secs(20),
            tokio_tungstenite::connect_async_with_config(request, Some(limits), false),
        )
        .await;
        if let Ok(Ok((mut socket, _))) = connection {
            delay = 5;
            notify.notify_one(); // catch up even if the first hint was lost.
            let mut ping = tokio::time::interval(Duration::from_secs(30));
            let mut last_message = Instant::now();
            loop {
                tokio::select! {
                    _ = ping.tick() => {
                        if last_message.elapsed() > Duration::from_secs(90) { break; }
                        if socket.send(Message::Text("ping".into())).await.is_err() { break; }
                    }
                    message = socket.next() => match message {
                        Some(Ok(Message::Text(text))) => {
                            last_message=Instant::now();
                            if text != "pong" { notify.notify_one(); }
                        }
                        Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => last_message=Instant::now(),
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                        _ => {}
                    }
                }
            }
            let _ = socket.close(None).await;
        }
        tokio::time::sleep(Duration::from_secs(delay)).await;
        delay = (delay * 2).min(300);
    }
}

pub(crate) fn start(app: &tauri::AppHandle) {
    if crate::types::is_isolated_dev() || STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let signal = wake();
        let mut socket: Option<tokio::task::JoinHandle<()>> = None;
        let mut identity = String::new();
        let mut last_attempt = Instant::now() - Duration::from_secs(600);
        let mut last_ok = false;
        let mut pending_inbound = false;
        let mut last_projection = Instant::now() - Duration::from_secs(600);
        loop {
            let cache = tauri::async_runtime::spawn_blocking(read_config).await;
            // Local retry deadlines must progress independently of HTTP backoff,
            // disabled transport and the five-minute idle network poll.
            if let Ok(Ok(Some(cfg))) = &cache {
                if last_projection.elapsed() >= Duration::from_secs(30) {
                    let path = crate::types::hanni_data_dir().join("hanni.db").to_string_lossy().to_string();
                    let raw = serde_json::to_string(cfg).expect("relay config serializes");
                    let result = tauri::async_runtime::spawn_blocking(move || crate::cloud_relay::run_headless_projection_once(&path, &raw)).await;
                    last_projection = Instant::now();
                    let changed = matches!(&result, Ok(Ok(raw)) if serde_json::from_str::<Value>(raw).ok().is_some_and(|v|v["records"].as_u64().unwrap_or(0)>0));
                    let _ = app.emit("cloud-relay-updated", json!({"ok":matches!(result, Ok(Ok(_))),"views_changed":changed}));
                }
            }
            let cfg = match cache {
                Ok(Ok(Some(cfg))) if cfg.enabled => Some(cfg),
                _ => None,
            };
            if let Some(cfg) = cfg {
                // This in-memory identity is never logged or persisted.
                let current = format!("{}\n{}\n{}", cfg.endpoint, cfg.device_id, cfg.token);
                if current != identity {
                    if let Some(task) = socket.take() {
                        task.abort();
                    }
                    socket = Some(tokio::spawn(stream(cfg.clone(), signal.clone())));
                    identity = current;
                    last_attempt = Instant::now() - Duration::from_secs(600);
                }
                let path = crate::types::hanni_data_dir()
                    .join("hanni.db")
                    .to_string_lossy()
                    .to_string();
                let check_path = path.clone();
                let pending=tauri::async_runtime::spawn_blocking(move || {
                    let conn=open_existing(&check_path).ok()?;
                    conn.query_row("SELECT (SELECT COUNT(*) FROM cloud_relay_dirty)+(SELECT COUNT(*) FROM cloud_relay_outbox)",[],|r|r.get::<_,i64>(0)).ok()
                }).await.ok().flatten().unwrap_or(1)>0;
                if due(last_attempt.elapsed(), pending, pending_inbound, last_ok) {
                    let raw = serde_json::to_string(&cfg).expect("relay config serializes");
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        run_headless_once(&path, &raw)
                    })
                    .await;
                    last_attempt = Instant::now();
                    last_ok = matches!(&result,Ok(Ok(raw)) if serde_json::from_str::<Value>(raw).ok().is_some_and(|v|v.get("error_code").is_none_or(Value::is_null)));
                    pending_inbound = match &result {
                        Ok(Ok(raw)) => serde_json::from_str::<Value>(raw)
                            .ok()
                            .and_then(|v| v.get("more_pending").and_then(Value::as_bool))
                            .unwrap_or(true),
                        _ => true,
                    };
                    // Event carries no credentials, health rows or timestamps.
                    let changed = matches!(&result, Ok(Ok(raw)) if serde_json::from_str::<Value>(raw).ok().is_some_and(|v|v["projection"]["records"].as_u64().unwrap_or(0)>0));
                    let _ = app.emit("cloud-relay-updated", json!({"ok":last_ok,"views_changed":changed}));
                }
            } else {
                if let Some(task) = socket.take() {
                    task.abort();
                }
                identity.clear();
            }
            tokio::select! {
                _=signal.notified() => { last_attempt=Instant::now()-Duration::from_secs(600); },
                _=tokio::time::sleep(Duration::from_secs(5)) => {},
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inbound_backlog_continues_without_local_outbox_or_another_notification() {
        assert!(due(Duration::from_secs(5), false, true, true));
        assert!(!due(Duration::from_secs(5), false, false, true));
        assert!(!due(Duration::from_secs(5), false, true, false));
        assert!(due(Duration::from_secs(30), false, true, false));
    }
}
