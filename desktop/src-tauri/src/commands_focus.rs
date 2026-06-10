// commands_focus.rs — Focus mode — start/stop/status + blocklist toggle
use crate::types::*;
use crate::macos::run_osascript;
use std::sync::atomic::Ordering;

// ── Focus Commands ──

#[tauri::command]
pub fn start_focus(
    duration_minutes: u64,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());

    if state.active {
        return Err("Focus mode is already active".into());
    }

    // Load default config if not provided
    let blocker_config_path = hanni_data_dir().join("blocker_config.json");

    let default_apps = vec!["Telegram".to_string(), "Discord".to_string(), "Slack".to_string()];
    let default_sites = vec![
        "youtube.com".to_string(), "twitter.com".to_string(), "x.com".to_string(),
        "instagram.com".to_string(), "facebook.com".to_string(), "tiktok.com".to_string(),
        "reddit.com".to_string(), "vk.com".to_string(), "netflix.com".to_string(),
    ];

    let block_apps = apps.unwrap_or_else(|| {
        if blocker_config_path.exists() {
            std::fs::read_to_string(&blocker_config_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|cfg| cfg["apps"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }))
                .unwrap_or_else(|| default_apps.clone())
        } else {
            default_apps.clone()
        }
    });

    let block_sites = sites.unwrap_or_else(|| {
        if blocker_config_path.exists() {
            std::fs::read_to_string(&blocker_config_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|cfg| cfg["sites"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }))
                .unwrap_or_else(|| default_sites.clone())
        } else {
            default_sites.clone()
        }
    });

    // Sanitize site names — only allow valid hostname chars
    let safe_site = |s: &str| -> String {
        s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect()
    };
    // Build hosts entries
    let mut hosts_entries = String::new();
    for site in &block_sites {
        let s = safe_site(site);
        if s.is_empty() { continue; }
        hosts_entries.push_str(&format!("127.0.0.1 {}\n127.0.0.1 www.{}\n", s, s));
    }

    // Write to /etc/hosts using osascript for sudo
    let hosts_block = format!(
        "# === HANNI FOCUS BLOCKER ===\n{}# === END HANNI FOCUS BLOCKER ===",
        hosts_entries
    );

    let script = format!(
        "do shell script \"printf '\\n{}' >> /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges",
        hosts_block.replace("'", "'\\''").replace("\n", "\\n")
    );
    run_osascript(&script).map_err(|e| format!("Failed to set focus mode (admin needed): {}", e))?;

    // Quit blocked apps — sanitize names to prevent AppleScript injection
    let safe_app = |s: &str| -> String {
        s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '.').collect()
    };
    for app_name in &block_apps {
        let name = safe_app(app_name);
        if name.is_empty() { continue; }
        let _ = run_osascript(&format!(
            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
            name, name
        ));
    }

    let end_time = chrono::Local::now() + chrono::Duration::minutes(duration_minutes as i64);
    state.active = true;
    state.end_time = Some(end_time);
    state.blocked_apps = block_apps;
    state.blocked_sites = block_sites;
    state.monitor_running.store(true, Ordering::Relaxed);

    Ok(format!("Focus mode started for {} minutes", duration_minutes))
}

#[tauri::command]
pub fn stop_focus(focus: tauri::State<'_, FocusManager>) -> Result<String, String> {
    let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());

    if !state.active {
        return Ok("Focus mode is not active".into());
    }

    // Remove HANNI FOCUS BLOCKER section from /etc/hosts
    let script = "do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges";
    let _ = run_osascript(script);

    state.active = false;
    state.end_time = None;
    state.blocked_apps.clear();
    state.blocked_sites.clear();
    state.monitor_running.store(false, Ordering::Relaxed);

    Ok("Focus mode stopped".into())
}

#[tauri::command]
pub fn get_focus_status(focus: tauri::State<'_, FocusManager>) -> Result<FocusStatus, String> {
    let state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
    let remaining = if let Some(end) = state.end_time {
        let diff = end - chrono::Local::now();
        if diff.num_seconds() > 0 { diff.num_seconds() as u64 } else { 0 }
    } else {
        0
    };
    Ok(FocusStatus {
        active: state.active,
        remaining_seconds: remaining,
        blocked_apps: state.blocked_apps.clone(),
        blocked_sites: state.blocked_sites.clone(),
    })
}

#[tauri::command]
pub fn update_blocklist(apps: Option<Vec<String>>, sites: Option<Vec<String>>) -> Result<String, String> {
    let config_path = hanni_data_dir().join("blocker_config.json");

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Dir error: {}", e))?;
    }

    let mut config: serde_json::Value = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(a) = apps {
        config["apps"] = serde_json::json!(a);
    }
    if let Some(s) = sites {
        config["sites"] = serde_json::json!(s);
    }

    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {}", e))?;
    Ok("Blocklist updated".into())
}
