use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MLX_URL: &str = "http://127.0.0.1:8234/v1/chat/completions";
const MODEL: &str = "mlx-community/Qwen3-30B-A3B-4bit";

const SYSTEM_PROMPT: &str = r#"You are Hanni, a helpful AI assistant running locally on Mac. Answer concisely. Use the user's language.
You can track life data via ```action``` JSON blocks with types: add_purchase(amount,category,description), add_time(activity,duration,category,productive), add_goal(title,category), add_note(title,content), get_stats.
macOS integrations: get_activity (app usage today), get_calendar (upcoming events), get_music (now playing), get_browser (current tab).
Example: ```action
{"type": "add_purchase", "amount": 5000, "category": "food", "description": "обед"}
```
Example: ```action
{"type": "get_calendar"}
```
Confirm actions. If a file is attached, analyze it."#;

fn data_file_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join("Documents/life-tracker/data.json")
}

// ── Life Tracker data types ──

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TrackerData {
    purchases: Vec<serde_json::Value>,
    #[serde(rename = "timeEntries")]
    time_entries: Vec<serde_json::Value>,
    goals: Vec<serde_json::Value>,
    notes: Vec<serde_json::Value>,
    #[serde(default)]
    settings: serde_json::Value,
}

// ── Chat types ──

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    stream: bool,
    temperature: f32,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Option<Delta>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Clone, Serialize)]
struct TokenPayload {
    token: String,
}

#[derive(Clone, Serialize)]
struct ActionPayload {
    action_type: String,
    result: String,
    success: bool,
}

struct HttpClient(reqwest::Client);

// ── macOS Activity tracking ──

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ActivityEntry {
    timestamp: String,
    app: String,
    idle_seconds: u64,
    is_afk: bool,
    category: String,
}

#[derive(Default, Clone, Debug)]
struct ActivityState {
    log: Vec<ActivityEntry>,
    current_app: String,
    is_afk: bool,
}

struct MacState(Arc<Mutex<ActivityState>>);

fn run_osascript(script: &str) -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| format!("osascript error: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn get_idle_seconds() -> u64 {
    let output = std::process::Command::new("ioreg")
        .args(["-c", "IOHIDSystem", "-d", "4"])
        .output()
        .ok();
    if let Some(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("HIDIdleTime") {
                if let Some(val) = line.split('=').last() {
                    let val = val.trim().trim_end_matches(|c: char| !c.is_ascii_digit());
                    if let Ok(ns) = val.parse::<u64>() {
                        return ns / 1_000_000_000;
                    }
                }
            }
        }
    }
    0
}

fn classify_app(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let productive = [
        "code", "cursor", "terminal", "iterm", "xcode", "intellij", "webstorm",
        "sublime", "vim", "neovim", "warp", "alacritty", "kitty", "notion",
        "obsidian", "figma", "linear", "github", "postman",
    ];
    let distraction = [
        "telegram", "discord", "slack", "whatsapp", "instagram", "twitter",
        "tiktok", "youtube", "reddit", "netflix", "twitch", "facebook",
    ];
    if productive.iter().any(|p| lower.contains(p)) {
        "productive"
    } else if distraction.iter().any(|d| lower.contains(d)) {
        "distraction"
    } else {
        "neutral"
    }
}

// ── Chat command ──

#[tauri::command]
async fn chat(app: AppHandle, messages: Vec<(String, String)>) -> Result<String, String> {
    let client = &app.state::<HttpClient>().0;

    let mut chat_messages = vec![ChatMessage {
        role: "system".into(),
        content: SYSTEM_PROMPT.into(),
    }];

    let msg_count = messages.len();
    for (i, (role, content)) in messages.iter().enumerate() {
        let mut c = content.clone();
        // Append /no_think to the last user message for reliable thinking suppression
        if i == msg_count - 1 && role == "user" {
            c.push_str(" /no_think");
        }
        chat_messages.push(ChatMessage {
            role: role.clone(),
            content: c,
        });
    }

    let request = ChatRequest {
        model: MODEL.into(),
        messages: chat_messages,
        max_tokens: 1024,
        stream: true,
        temperature: 0.7,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("MLX connection error: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut full_reply = String::new();
    let mut in_think = false;
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        for line in buffer.split('\n').collect::<Vec<_>>() {
            let line = line.trim();
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data == "[DONE]" {
                let _ = app.emit("chat-done", ());
                continue;
            }

            if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                if let Some(choice) = chunk.choices.first() {
                    if let Some(delta) = &choice.delta {
                        if let Some(token) = &delta.content {
                            if token.contains("<think>") {
                                in_think = true;
                                continue;
                            }
                            if token.contains("</think>") {
                                in_think = false;
                                continue;
                            }
                            if in_think {
                                continue;
                            }
                            full_reply.push_str(token);
                            let _ = app.emit("chat-token", TokenPayload {
                                token: token.clone(),
                            });
                        }
                    }
                }
            }
        }
        if let Some(pos) = buffer.rfind('\n') {
            buffer = buffer[pos + 1..].to_string();
        }
    }

    Ok(full_reply)
}

// ── File commands ──

#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Cannot access {}: {}", path, e))?;

    // Limit to 500KB for text files
    if metadata.len() > 512_000 {
        return Err(format!("File too large: {} bytes (max 500KB)", metadata.len()));
    }

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Cannot read {}: {}", path, e))
}

#[tauri::command]
async fn list_dir(path: String) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| format!("Cannot read dir {}: {}", path, e))?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        if let Some(name) = entry.file_name().to_str() {
            entries.push(name.to_string());
        }
    }
    Ok(entries)
}

// ── Life Tracker commands ──

fn load_tracker_data() -> Result<TrackerData, String> {
    let path = data_file_path();
    if !path.exists() {
        return Err("Life Tracker data file not found".into());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read tracker data: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Cannot parse tracker data: {}", e))
}

fn save_tracker_data(data: &TrackerData) -> Result<(), String> {
    let path = data_file_path();
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Cannot write: {}", e))
}

#[tauri::command]
async fn tracker_add_purchase(amount: f64, category: String, description: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("p_{}", now.timestamp_millis()),
        "date": now.format("%Y-%m-%d").to_string(),
        "amount": amount,
        "currency": "KZT",
        "category": category,
        "description": description,
        "tags": [],
        "source": "hanni"
    });
    data.purchases.push(entry.clone());
    save_tracker_data(&data)?;
    Ok(format!("Added purchase: {} KZT — {}", amount, description))
}

#[tauri::command]
async fn tracker_add_time(activity: String, duration: u32, category: String, productive: bool) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("t_{}", now.timestamp_millis()),
        "date": now.format("%Y-%m-%d").to_string(),
        "duration": duration,
        "activity": activity,
        "category": category,
        "productive": productive,
        "notes": "",
        "source": "hanni"
    });
    data.time_entries.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added time: {} min — {}", duration, activity))
}

#[tauri::command]
async fn tracker_add_goal(title: String, category: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("g_{}", now.timestamp_millis()),
        "title": title,
        "description": "",
        "category": category,
        "progress": 0,
        "milestones": [],
        "status": "active",
        "createdAt": now.to_rfc3339()
    });
    data.goals.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added goal: {}", title))
}

#[tauri::command]
async fn tracker_add_note(title: String, content: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("n_{}", now.timestamp_millis()),
        "title": title,
        "content": content,
        "tags": [],
        "pinned": false,
        "archived": false,
        "createdAt": now.to_rfc3339(),
        "updatedAt": now.to_rfc3339()
    });
    data.notes.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added note: {}", title))
}

#[tauri::command]
async fn tracker_get_stats() -> Result<String, String> {
    let data = load_tracker_data()?;
    let today = chrono::Local::now().format("%Y-%m").to_string();

    let month_purchases: f64 = data.purchases.iter()
        .filter(|p| p["date"].as_str().unwrap_or("").starts_with(&today))
        .map(|p| p["amount"].as_f64().unwrap_or(0.0))
        .sum();

    let month_time: u64 = data.time_entries.iter()
        .filter(|t| t["date"].as_str().unwrap_or("").starts_with(&today))
        .map(|t| t["duration"].as_u64().unwrap_or(0))
        .sum();

    let active_goals = data.goals.iter()
        .filter(|g| g["status"].as_str().unwrap_or("") == "active")
        .count();

    let total_notes = data.notes.len();

    Ok(format!(
        "📊 Статистика за {}:\n• Расходы: {:.0} KZT ({} записей)\n• Время: {} мин ({} записей)\n• Активных целей: {}\n• Заметок: {}",
        today, month_purchases, data.purchases.len(),
        month_time, data.time_entries.len(),
        active_goals, total_notes
    ))
}

#[tauri::command]
async fn tracker_get_recent(entry_type: String, limit: usize) -> Result<String, String> {
    let data = load_tracker_data()?;
    let entries: Vec<&serde_json::Value> = match entry_type.as_str() {
        "purchases" => data.purchases.iter().rev().take(limit).collect(),
        "time" => data.time_entries.iter().rev().take(limit).collect(),
        "goals" => data.goals.iter().rev().take(limit).collect(),
        "notes" => data.notes.iter().rev().take(limit).collect(),
        _ => return Err(format!("Unknown type: {}", entry_type)),
    };
    serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Serialize error: {}", e))
}

// ── macOS commands ──

#[tauri::command]
async fn get_activity_summary(app: AppHandle) -> Result<String, String> {
    let state = app.state::<MacState>();
    let activity = state.0.lock().map_err(|e| e.to_string())?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today_entries: Vec<&ActivityEntry> = activity.log.iter()
        .filter(|e| e.timestamp.starts_with(&today))
        .collect();

    if today_entries.is_empty() {
        return Ok("No activity recorded today yet.".into());
    }

    let mut productive_min: u64 = 0;
    let mut distraction_min: u64 = 0;
    let mut neutral_min: u64 = 0;
    let mut afk_min: u64 = 0;
    let mut app_minutes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for entry in &today_entries {
        if entry.is_afk {
            afk_min += 1; // each entry ~30s, but we report as half-minutes
        } else {
            match entry.category.as_str() {
                "productive" => productive_min += 1,
                "distraction" => distraction_min += 1,
                _ => neutral_min += 1,
            }
            *app_minutes.entry(entry.app.clone()).or_insert(0) += 1;
        }
    }

    // Convert from 30s intervals to minutes
    let to_min = |count: u64| count / 2;

    let mut top_apps: Vec<(String, u64)> = app_minutes.into_iter().collect();
    top_apps.sort_by(|a, b| b.1.cmp(&a.1));
    top_apps.truncate(5);

    let top_str: Vec<String> = top_apps.iter()
        .map(|(name, count)| format!("  {} — {} min", name, to_min(*count)))
        .collect();

    Ok(format!(
        "Activity today ({} snapshots):\n\
         Productive: {} min\nDistraction: {} min\nNeutral: {} min\nAFK: {} min\n\n\
         Top apps:\n{}",
        today_entries.len(),
        to_min(productive_min), to_min(distraction_min),
        to_min(neutral_min), to_min(afk_min),
        top_str.join("\n")
    ))
}

#[tauri::command]
async fn get_current_activity(app: AppHandle) -> Result<String, String> {
    let state = app.state::<MacState>();
    let activity = state.0.lock().map_err(|e| e.to_string())?;
    if activity.current_app.is_empty() {
        return Ok("No activity data yet. Tracking starts in ~30 seconds.".into());
    }
    Ok(format!(
        "Current app: {}\nAFK: {}",
        activity.current_app,
        if activity.is_afk { "yes" } else { "no" }
    ))
}

#[tauri::command]
async fn get_calendar_events() -> Result<String, String> {
    let script = r#"
        set output to ""
        set today to current date
        set endDate to today + (2 * days)
        tell application "Calendar"
            repeat with cal in calendars
                set evts to (every event of cal whose start date >= today and start date <= endDate)
                repeat with evt in evts
                    set evtStart to start date of evt
                    set evtName to summary of evt
                    set output to output & (evtStart as string) & " | " & evtName & linefeed
                end repeat
            end repeat
        end tell
        if output is "" then
            return "No upcoming events in the next 2 days."
        end if
        return output
    "#;
    run_osascript(script)
}

#[tauri::command]
async fn get_now_playing() -> Result<String, String> {
    // Check Music.app
    let music_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Music\""
    );
    if let Ok(ref val) = music_check {
        if val == "true" {
            let result = run_osascript(
                "tell application \"Music\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Music paused\" end if"
            );
            if let Ok(info) = result {
                return Ok(format!("Apple Music: {}", info));
            }
        }
    }

    // Check Spotify
    let spotify_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Spotify\""
    );
    if let Ok(ref val) = spotify_check {
        if val == "true" {
            let result = run_osascript(
                "tell application \"Spotify\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Spotify paused\" end if"
            );
            if let Ok(info) = result {
                return Ok(format!("Spotify: {}", info));
            }
        }
    }

    Ok("No music app is currently playing.".into())
}

#[tauri::command]
async fn get_browser_tab() -> Result<String, String> {
    let browsers = [
        ("Arc", "tell application \"Arc\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Google Chrome", "tell application \"Google Chrome\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Safari", "tell application \"Safari\" to return URL of front document & \" | \" & name of front document"),
    ];

    for (name, script) in &browsers {
        let check = run_osascript(&format!(
            "tell application \"System Events\" to (name of processes) contains \"{}\"", name
        ));
        if let Ok(ref val) = check {
            if val == "true" {
                if let Ok(info) = run_osascript(script) {
                    return Ok(format!("{}: {}", name, info));
                }
            }
        }
    }

    Ok("No supported browser is currently open.".into())
}

// ── Integrations info ──

#[derive(Serialize)]
struct IntegrationItem {
    name: String,
    status: String,  // "active", "inactive", "blocked"
    detail: String,
}

#[derive(Serialize)]
struct IntegrationsInfo {
    access: Vec<IntegrationItem>,
    tracking: Vec<IntegrationItem>,
    blocked_apps: Vec<IntegrationItem>,
    blocked_sites: Vec<IntegrationItem>,
    blocker_active: bool,
    macos: Vec<IntegrationItem>,
}

#[tauri::command]
async fn get_integrations(app: AppHandle) -> Result<IntegrationsInfo, String> {
    // ── Access ──
    let tracker_path = data_file_path();
    let tracker_exists = tracker_path.exists();
    let access = vec![
        IntegrationItem {
            name: "Life Tracker".into(),
            status: if tracker_exists { "active" } else { "inactive" }.into(),
            detail: if tracker_exists {
                "~/Documents/life-tracker/data.json".into()
            } else {
                "Файл не найден".into()
            },
        },
        IntegrationItem {
            name: "File System".into(),
            status: "active".into(),
            detail: "$HOME/** — чтение файлов".into(),
        },
        IntegrationItem {
            name: "Shell".into(),
            status: "active".into(),
            detail: "Выполнение команд".into(),
        },
    ];

    // ── Tracking ──
    let tracking = if tracker_exists {
        let data = load_tracker_data().unwrap_or(TrackerData {
            purchases: vec![], time_entries: vec![], goals: vec![], notes: vec![],
            settings: serde_json::Value::Null,
        });
        vec![
            IntegrationItem {
                name: "Расходы".into(),
                status: "active".into(),
                detail: format!("{} записей", data.purchases.len()),
            },
            IntegrationItem {
                name: "Время".into(),
                status: "active".into(),
                detail: format!("{} записей", data.time_entries.len()),
            },
            IntegrationItem {
                name: "Цели".into(),
                status: "active".into(),
                detail: format!("{} целей", data.goals.len()),
            },
            IntegrationItem {
                name: "Заметки".into(),
                status: "active".into(),
                detail: format!("{} заметок", data.notes.len()),
            },
        ]
    } else {
        vec![IntegrationItem {
            name: "Life Tracker".into(),
            status: "inactive".into(),
            detail: "Не подключен".into(),
        }]
    };

    // ── Blocker config ──
    let blocker_config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    let default_apps = vec!["Telegram", "Discord", "Slack", "Safari"];
    let default_sites = vec![
        "youtube.com", "twitter.com", "x.com", "instagram.com",
        "facebook.com", "tiktok.com", "reddit.com", "vk.com", "netflix.com",
    ];

    let (apps, sites) = if blocker_config_path.exists() {
        let content = std::fs::read_to_string(&blocker_config_path).unwrap_or_default();
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
            let apps: Vec<String> = cfg["apps"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_apps.iter().map(|s| s.to_string()).collect());
            let sites: Vec<String> = cfg["sites"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_sites.iter().map(|s| s.to_string()).collect());
            (apps, sites)
        } else {
            (default_apps.iter().map(|s| s.to_string()).collect(),
             default_sites.iter().map(|s| s.to_string()).collect())
        }
    } else {
        (default_apps.iter().map(|s| s.to_string()).collect(),
         default_sites.iter().map(|s| s.to_string()).collect())
    };

    // Check if blocking is active via /etc/hosts
    let blocker_active = std::fs::read_to_string("/etc/hosts")
        .map(|c| c.contains("# === HANNI FOCUS BLOCKER ==="))
        .unwrap_or(false);

    let blocked_apps = apps.iter().map(|a| IntegrationItem {
        name: a.clone(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: format!("/Applications/{}.app", a),
    }).collect();

    // Deduplicate sites (remove www. variants for display)
    let unique_sites: Vec<&String> = sites.iter()
        .filter(|s| !s.starts_with("www."))
        .collect();

    let blocked_sites = unique_sites.iter().map(|s| IntegrationItem {
        name: s.to_string(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: if blocker_active { "Заблокирован" } else { "Не заблокирован" }.into(),
    }).collect();

    // ── macOS integrations ──
    let mac_state = app.state::<MacState>();
    let activity = mac_state.0.lock().map_err(|e| e.to_string())?;

    let activity_detail = if activity.current_app.is_empty() {
        "Сбор данных...".to_string()
    } else {
        format!("{}{}", activity.current_app, if activity.is_afk { " (AFK)" } else { "" })
    };

    let macos = vec![
        IntegrationItem {
            name: "Активность".into(),
            status: if activity.current_app.is_empty() { "inactive" } else { "active" }.into(),
            detail: activity_detail,
        },
        IntegrationItem {
            name: "Календарь".into(),
            status: "active".into(),
            detail: "Calendar.app".into(),
        },
        IntegrationItem {
            name: "Музыка".into(),
            status: "active".into(),
            detail: "Music / Spotify".into(),
        },
        IntegrationItem {
            name: "Браузер".into(),
            status: "active".into(),
            detail: "Safari / Chrome / Arc".into(),
        },
    ];

    drop(activity); // release lock

    Ok(IntegrationsInfo {
        access,
        tracking,
        blocked_apps,
        blocked_sites,
        blocker_active,
        macos,
    })
}

// ── Model info ──

#[derive(Serialize)]
struct ModelInfo {
    model_name: String,
    server_url: String,
    server_online: bool,
}

#[tauri::command]
async fn get_model_info() -> Result<ModelInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    let online = client
        .get("http://127.0.0.1:8234/v1/models")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    Ok(ModelInfo {
        model_name: MODEL.to_string(),
        server_url: MLX_URL.to_string(),
        server_online: online,
    })
}

// ── App setup ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mac_state = MacState(Arc::new(Mutex::new(ActivityState::default())));
    let activity_arc = mac_state.0.clone();

    tauri::Builder::default()
        .manage(HttpClient(reqwest::Client::new()))
        .manage(mac_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            chat,
            read_file,
            list_dir,
            tracker_add_purchase,
            tracker_add_time,
            tracker_add_goal,
            tracker_add_note,
            tracker_get_stats,
            tracker_get_recent,
            get_integrations,
            get_model_info,
            get_activity_summary,
            get_current_activity,
            get_calendar_events,
            get_now_playing,
            get_browser_tab,
        ])
        .setup(move |app| {
            // Auto-updater
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match handle.updater().expect("updater not configured").check().await {
                    Ok(Some(update)) => {
                        let version = update.version.clone();
                        let _ = handle.emit("update-available", &version);
                        if let Ok(()) = update.download_and_install(|_, _| {}, || {}).await {
                            handle.restart();
                        }
                    }
                    _ => {}
                }
            });

            // Background activity tracker
            let tracker_handle = app.handle().clone();
            let tracker_state = activity_arc;
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;

                    let app_name = run_osascript(
                        "tell application \"System Events\" to get name of first application process whose frontmost is true"
                    ).unwrap_or_default();

                    let idle = get_idle_seconds();
                    let is_afk = idle > 300;
                    let category = classify_app(&app_name).to_string();
                    let timestamp = chrono::Local::now().to_rfc3339();

                    let entry = ActivityEntry {
                        timestamp,
                        app: app_name.clone(),
                        idle_seconds: idle,
                        is_afk,
                        category,
                    };

                    if let Ok(mut state) = tracker_state.lock() {
                        state.current_app = app_name;
                        state.is_afk = is_afk;
                        state.log.push(entry);
                        // Keep max 2880 entries (~24h at 30s intervals)
                        if state.log.len() > 2880 {
                            state.log.remove(0);
                        }
                    }

                    let _ = tracker_handle.emit("activity-update", ());
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Hanni");
}
