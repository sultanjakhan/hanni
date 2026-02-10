use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use chrono::Timelike;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::io::Write;

const MLX_URL: &str = "http://127.0.0.1:8234/v1/chat/completions";
const MODEL: &str = "mlx-community/Qwen3-30B-A3B-4bit";

const SYSTEM_PROMPT: &str = r#"You are Hanni, a helpful AI assistant running locally on Mac. Answer concisely. Use the user's language.

You can execute actions using ```action JSON blocks:
Life tracking: add_purchase(amount,category,description), add_time(activity,duration,category,productive), add_goal(title,category), add_note(title,content), get_stats.
macOS: get_activity (app usage today), get_calendar (upcoming events), get_music (now playing), get_browser (current tab).
Memory: remember(category,key,value), recall(category), forget(category,key), search_memory(query,limit?). Categories: user, preferences, world, tasks, people, habits.
Focus: start_focus(duration,apps?,sites?), stop_focus. Block distracting apps and sites for focused work.
System: run_shell(command), open_url(url), send_notification(title,body), set_volume(level), get_clipboard, set_clipboard(text).
Voice: (automatic — user speaks, text appears).

IMPORTANT:
- You can chain actions. After each action, you receive results as [Action result: ...].
- Call one action at a time. After getting the result, call another or give your final answer.
- First gather data via actions, then answer the user WITHOUT action blocks.
- Do NOT repeat raw results. Analyze and summarize naturally.
- If a file is attached, analyze it.
- Proactively remember important facts the user shares (name, preferences, habits, language, people they mention). You always have your memories in context — no need to recall before answering.
- Use search_memory(query) to find specific memories when the user asks about something that might be stored."#;

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

// ── Proactive messaging types ──

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProactiveSettings {
    enabled: bool,
    voice_enabled: bool,
    voice_name: String,
    interval_minutes: u64,
    quiet_hours_start: u32,
    quiet_hours_end: u32,
}

impl Default for ProactiveSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            voice_enabled: false,
            voice_name: "Milena".into(),
            interval_minutes: 10,
            quiet_hours_start: 23,
            quiet_hours_end: 8,
        }
    }
}

struct ProactiveState {
    settings: ProactiveSettings,
    last_message_time: Option<chrono::DateTime<chrono::Local>>,
    last_message_text: String,
    consecutive_skips: u32,
    user_is_typing: bool,
}

impl ProactiveState {
    fn new(settings: ProactiveSettings) -> Self {
        Self {
            settings,
            last_message_time: None,
            last_message_text: String::new(),
            consecutive_skips: 0,
            user_is_typing: false,
        }
    }
}

// ── SQLite Memory system ──

struct HanniDb(std::sync::Mutex<rusqlite::Connection>);

fn hanni_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Documents/Hanni/hanni.db")
}

fn init_db(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS facts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            source TEXT DEFAULT 'user',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(category, key)
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
            category, key, value,
            content='facts', content_rowid='id'
        );

        -- Triggers to keep FTS in sync
        CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
            INSERT INTO facts_fts(rowid, category, key, value) VALUES (new.id, new.category, new.key, new.value);
        END;
        CREATE TRIGGER IF NOT EXISTS facts_ad AFTER DELETE ON facts BEGIN
            INSERT INTO facts_fts(facts_fts, rowid, category, key, value) VALUES('delete', old.id, old.category, old.key, old.value);
        END;
        CREATE TRIGGER IF NOT EXISTS facts_au AFTER UPDATE ON facts BEGIN
            INSERT INTO facts_fts(facts_fts, rowid, category, key, value) VALUES('delete', old.id, old.category, old.key, old.value);
            INSERT INTO facts_fts(rowid, category, key, value) VALUES (new.id, new.category, new.key, new.value);
        END;

        CREATE TABLE IF NOT EXISTS conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            summary TEXT,
            message_count INTEGER DEFAULT 0,
            messages TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts USING fts5(
            summary, messages,
            content='conversations', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS conv_ai AFTER INSERT ON conversations BEGIN
            INSERT INTO conversations_fts(rowid, summary, messages) VALUES (new.id, COALESCE(new.summary, ''), new.messages);
        END;
        CREATE TRIGGER IF NOT EXISTS conv_ad AFTER DELETE ON conversations BEGIN
            INSERT INTO conversations_fts(conversations_fts, rowid, summary, messages) VALUES('delete', old.id, COALESCE(old.summary, ''), old.messages);
        END;
        CREATE TRIGGER IF NOT EXISTS conv_au AFTER UPDATE ON conversations BEGIN
            INSERT INTO conversations_fts(conversations_fts, rowid, summary, messages) VALUES('delete', old.id, COALESCE(old.summary, ''), old.messages);
            INSERT INTO conversations_fts(rowid, summary, messages) VALUES (new.id, COALESCE(new.summary, ''), new.messages);
        END;

        -- v0.7.0: Activities (Focus)
        CREATE TABLE IF NOT EXISTS activities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'other',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_minutes INTEGER,
            focus_mode INTEGER DEFAULT 0,
            blocked_apps TEXT,
            blocked_sites TEXT,
            notes TEXT,
            created_at TEXT NOT NULL
        );

        -- v0.7.0: Notes
        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            pinned INTEGER DEFAULT 0,
            archived INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title, content, tags,
            content='notes', content_rowid='id'
        );
        CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, tags) VALUES('delete', old.id, old.title, old.content, old.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, tags) VALUES('delete', old.id, old.title, old.content, old.tags);
            INSERT INTO notes_fts(rowid, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
        END;

        -- v0.7.0: Events (Calendar)
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            date TEXT NOT NULL,
            time TEXT NOT NULL DEFAULT '',
            duration_minutes INTEGER DEFAULT 60,
            category TEXT NOT NULL DEFAULT 'general',
            color TEXT NOT NULL DEFAULT '#818cf8',
            completed INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );

        -- v0.7.0: Projects & Tasks (Work)
        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            color TEXT NOT NULL DEFAULT '#818cf8',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'todo',
            priority TEXT NOT NULL DEFAULT 'normal',
            due_date TEXT,
            completed_at TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );

        -- v0.7.0: Learning Items (Development)
        CREATE TABLE IF NOT EXISTS learning_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL DEFAULT 'course',
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            url TEXT NOT NULL DEFAULT '',
            progress INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'planned',
            category TEXT NOT NULL DEFAULT 'general',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- v0.7.0: Hobbies
        CREATE TABLE IF NOT EXISTS hobbies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'general',
            icon TEXT NOT NULL DEFAULT '',
            color TEXT NOT NULL DEFAULT '#818cf8',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS hobby_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hobby_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            FOREIGN KEY (hobby_id) REFERENCES hobbies(id)
        );

        -- v0.7.0: Workouts & Exercises (Sports)
        CREATE TABLE IF NOT EXISTS workouts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL DEFAULT 'other',
            title TEXT NOT NULL DEFAULT '',
            date TEXT NOT NULL,
            duration_minutes INTEGER DEFAULT 0,
            calories INTEGER,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS exercises (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            workout_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            sets INTEGER,
            reps INTEGER,
            weight_kg REAL,
            duration_seconds INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY (workout_id) REFERENCES workouts(id)
        );

        -- v0.7.0: Health Log & Habits
        CREATE TABLE IF NOT EXISTS health_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            value REAL NOT NULL,
            unit TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS habits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            icon TEXT NOT NULL DEFAULT '',
            frequency TEXT NOT NULL DEFAULT 'daily',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS habit_checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            habit_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            completed INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            UNIQUE(habit_id, date),
            FOREIGN KEY (habit_id) REFERENCES habits(id)
        );"
    ).map_err(|e| format!("DB init error: {}", e))
}

fn migrate_memory_json(conn: &rusqlite::Connection) {
    let json_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Documents/Hanni/memory.json");
    if !json_path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(&json_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    #[derive(Deserialize)]
    struct OldEntry {
        value: String,
        #[allow(dead_code)]
        category: String,
        #[allow(dead_code)]
        timestamp: String,
    }
    #[derive(Deserialize)]
    struct OldMemory {
        facts: HashMap<String, HashMap<String, OldEntry>>,
    }

    let old: OldMemory = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return,
    };

    let now = chrono::Local::now().to_rfc3339();
    for (category, entries) in &old.facts {
        for (key, entry) in entries {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO facts (category, key, value, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'migrated', ?4, ?4)",
                rusqlite::params![category, key, entry.value, now],
            );
        }
    }

    // Rename old file to .bak
    let bak_path = json_path.with_extension("json.bak");
    let _ = std::fs::rename(&json_path, &bak_path);
}

fn build_memory_context_from_db(conn: &rusqlite::Connection, user_msg: &str, limit: usize) -> String {
    let mut lines = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 1. Always include core user/preferences facts (top 20)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, category, key, value FROM facts
         WHERE category IN ('user', 'preferences')
         ORDER BY updated_at DESC LIMIT 20"
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                seen_ids.insert(row.0);
                lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
            }
        }
    }

    // 2. FTS5 search matching user's latest message (top 20 more)
    let remaining = limit.saturating_sub(lines.len());
    if remaining > 0 && !user_msg.is_empty() {
        // Build FTS query: split words, join with OR
        let words: Vec<&str> = user_msg.split_whitespace()
            .filter(|w| w.len() > 2)
            .take(10)
            .collect();
        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.id, f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1
                 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, remaining as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        if seen_ids.insert(row.0) {
                            lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                        }
                    }
                }
            }
        }
    }

    // 3. Fill remaining with most recent facts
    let remaining = limit.saturating_sub(lines.len());
    if remaining > 0 {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, category, key, value FROM facts
             ORDER BY updated_at DESC LIMIT ?1"
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![remaining as i64 + seen_ids.len() as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    if lines.len() >= limit {
                        break;
                    }
                    if seen_ids.insert(row.0) {
                        lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                    }
                }
            }
        }
    }

    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n")
    }
}

fn proactive_settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Documents/Hanni/proactive_settings.json")
}

fn load_proactive_settings() -> ProactiveSettings {
    let path = proactive_settings_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        ProactiveSettings::default()
    }
}

fn save_proactive_settings(settings: &ProactiveSettings) -> Result<(), String> {
    let path = proactive_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Cannot write: {}", e))
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

struct HttpClient(reqwest::Client);
struct LlmBusy(AtomicBool);

struct MlxProcess(std::sync::Mutex<Option<Child>>);

// ── Whisper / Voice state ──

struct WhisperState {
    recording: bool,
    audio_buffer: Vec<f32>,
    capture_running: bool,
}

struct AudioRecording(std::sync::Mutex<WhisperState>);

fn whisper_model_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Documents/Hanni/models/ggml-medium.bin")
}

#[tauri::command]
async fn download_whisper_model(app: AppHandle) -> Result<String, String> {
    let model_path = whisper_model_path();
    if model_path.exists() {
        return Ok("Model already downloaded".into());
    }

    if let Some(parent) = model_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
    }

    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin";
    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| format!("Download error: {}", e))?;

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    let tmp_path = model_path.with_extension("bin.tmp");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| format!("File error: {}", e))?;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        file.write_all(&bytes).map_err(|e| format!("Write error: {}", e))?;
        downloaded += bytes.len() as u64;
        if total > 0 {
            let pct = (downloaded as f64 / total as f64 * 100.0) as u32;
            let _ = app.emit("whisper-download-progress", pct);
        }
    }

    std::fs::rename(&tmp_path, &model_path).map_err(|e| format!("Rename error: {}", e))?;
    Ok("Model downloaded successfully".into())
}

#[tauri::command]
fn start_recording(state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    let needs_capture = {
        let mut ws = state.0.lock().map_err(|e| format!("Lock error: {}", e))?;
        if ws.recording {
            return Err("Already recording".into());
        }
        ws.recording = true;
        ws.audio_buffer.clear();
        let needs = !ws.capture_running;
        if needs { ws.capture_running = true; }
        needs
    };
    if needs_capture {
        start_audio_capture(state.inner().clone());
    }
    Ok("Recording started".into())
}

#[tauri::command]
fn stop_recording(state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    let mut ws = state.0.lock().map_err(|e| format!("Lock error: {}", e))?;
    ws.recording = false;

    if ws.audio_buffer.is_empty() {
        return Err("No audio recorded".into());
    }

    let model_path = whisper_model_path();
    if !model_path.exists() {
        return Err("Whisper model not downloaded. Please download it first.".into());
    }

    let samples = ws.audio_buffer.clone();
    ws.audio_buffer.clear();

    // Run whisper transcription
    let ctx = whisper_rs::WhisperContext::new_with_params(
        model_path.to_str().unwrap_or(""),
        whisper_rs::WhisperContextParameters::default(),
    ).map_err(|e| format!("Whisper init error: {}", e))?;

    let mut state = ctx.create_state().map_err(|e| format!("Whisper state error: {}", e))?;

    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(None); // Auto-detect
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state.full(params, &samples).map_err(|e| format!("Transcription error: {}", e))?;

    let num_segments = state.full_n_segments().map_err(|e| format!("Segment error: {}", e))?;
    let mut text = String::new();
    for i in 0..num_segments {
        if let Ok(segment) = state.full_get_segment_text(i) {
            text.push_str(&segment);
        }
    }

    Ok(text.trim().to_string())
}

#[tauri::command]
fn check_whisper_model() -> Result<bool, String> {
    Ok(whisper_model_path().exists())
}

// ── Audio capture via cpal ──

fn start_audio_capture(recording_state: Arc<AudioRecording>) {
    std::thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => return,
        };

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(16000),
            buffer_size: cpal::BufferSize::Default,
        };

        let state_clone = recording_state.clone();
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut ws) = state_clone.0.lock() {
                    if ws.recording {
                        ws.audio_buffer.extend_from_slice(data);
                    }
                }
            },
            |err| {
                eprintln!("Audio capture error: {}", err);
            },
            None,
        );

        if let Ok(stream) = stream {
            let _ = stream.play();
            // Keep stream alive while recording, exit when done
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Ok(ws) = recording_state.0.lock() {
                    if !ws.recording {
                        break;
                    }
                }
            }
            // Mark capture as stopped so it can be restarted
            if let Ok(mut ws) = recording_state.0.lock() {
                ws.capture_running = false;
            }
        }
    });
}

// ── Focus Mode state ──

struct FocusState {
    active: bool,
    end_time: Option<chrono::DateTime<chrono::Local>>,
    blocked_apps: Vec<String>,
    blocked_sites: Vec<String>,
    monitor_running: Arc<AtomicBool>,
}

struct FocusManager(std::sync::Mutex<FocusState>);

#[derive(Serialize, Clone)]
struct FocusStatus {
    active: bool,
    remaining_seconds: u64,
    blocked_apps: Vec<String>,
    blocked_sites: Vec<String>,
}

#[tauri::command]
fn start_focus(
    duration_minutes: u64,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let mut state = focus.0.lock().map_err(|e| format!("Lock error: {}", e))?;

    if state.active {
        return Err("Focus mode is already active".into());
    }

    // Load default config if not provided
    let blocker_config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

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

    // Build hosts entries
    let mut hosts_entries = String::new();
    for site in &block_sites {
        hosts_entries.push_str(&format!("127.0.0.1 {}\n127.0.0.1 www.{}\n", site, site));
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

    // Quit blocked apps
    for app_name in &block_apps {
        let _ = run_osascript(&format!(
            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
            app_name, app_name
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
fn stop_focus(focus: tauri::State<'_, FocusManager>) -> Result<String, String> {
    let mut state = focus.0.lock().map_err(|e| format!("Lock error: {}", e))?;

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
fn get_focus_status(focus: tauri::State<'_, FocusManager>) -> Result<FocusStatus, String> {
    let state = focus.0.lock().map_err(|e| format!("Lock error: {}", e))?;
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
fn update_blocklist(apps: Option<Vec<String>>, sites: Option<Vec<String>>) -> Result<String, String> {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

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

// ── Phase 5: macOS Actions ──

#[tauri::command]
async fn run_shell(command: String) -> Result<String, String> {
    // Safety: limit command length and block dangerous patterns
    if command.len() > 1000 {
        return Err("Command too long (max 1000 chars)".into());
    }
    let dangerous = ["rm -rf /", "mkfs", "dd if=", "> /dev/", ":(){ :|:& };:"];
    for d in &dangerous {
        if command.contains(d) {
            return Err(format!("Blocked dangerous command pattern: {}", d));
        }
    }

    let output = std::process::Command::new("sh")
        .args(["-c", &command])
        .output()
        .map_err(|e| format!("Shell error: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        let result = stdout.trim().to_string();
        if result.len() > 5000 {
            Ok(format!("{}...\n[truncated, {} bytes total]", &result[..5000], result.len()))
        } else {
            Ok(result)
        }
    } else {
        Err(format!("Command failed: {}", stderr.trim()))
    }
}

#[tauri::command]
async fn open_url(url: String) -> Result<String, String> {
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Open error: {}", e))?;
    Ok(format!("Opened {}", url))
}

#[tauri::command]
async fn send_notification(title: String, body: String) -> Result<String, String> {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        body.replace("\"", "\\\""),
        title.replace("\"", "\\\"")
    );
    run_osascript(&script)?;
    Ok("Notification sent".into())
}

#[tauri::command]
async fn set_volume(level: u32) -> Result<String, String> {
    let clamped = level.min(100);
    run_osascript(&format!("set volume output volume {}", clamped))?;
    Ok(format!("Volume set to {}%", clamped))
}

#[tauri::command]
async fn get_clipboard() -> Result<String, String> {
    let output = std::process::Command::new("pbpaste")
        .output()
        .map_err(|e| format!("Clipboard error: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
async fn set_clipboard(text: String) -> Result<String, String> {
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Clipboard error: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("Write error: {}", e))?;
    }
    child.wait().map_err(|e| format!("Wait error: {}", e))?;
    Ok("Copied to clipboard".into())
}

// ── Phase 3: Training Data Export ──

#[tauri::command]
fn get_training_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;

    let conv_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations WHERE message_count >= 4",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let total_messages: i64 = conn.query_row(
        "SELECT COALESCE(SUM(message_count), 0) FROM conversations WHERE message_count >= 4",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let date_range: (String, String) = conn.query_row(
        "SELECT COALESCE(MIN(started_at), ''), COALESCE(MAX(started_at), '') FROM conversations WHERE message_count >= 4",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).unwrap_or(("".into(), "".into()));

    Ok(serde_json::json!({
        "conversations": conv_count,
        "total_messages": total_messages,
        "earliest": date_range.0,
        "latest": date_range.1,
    }))
}

#[tauri::command]
fn export_training_data(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;

    let mut stmt = conn.prepare(
        "SELECT messages, summary FROM conversations WHERE message_count >= 4 ORDER BY started_at"
    ).map_err(|e| format!("DB error: {}", e))?;

    let rows: Vec<(String, Option<String>)> = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let mut training_examples: Vec<serde_json::Value> = Vec::new();

    for (messages_json, _summary) in &rows {
        let messages: Vec<(String, String)> = match serde_json::from_str(messages_json) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Filter: skip if fewer than 2 real messages
        let real_msgs: Vec<&(String, String)> = messages.iter()
            .filter(|(role, content)| {
                (role == "user" || role == "assistant")
                && !content.starts_with("[Action result:")
                && !content.contains("```action")
            })
            .collect();

        if real_msgs.len() < 2 {
            continue;
        }

        let mut chat_msgs = vec![serde_json::json!({
            "role": "system",
            "content": SYSTEM_PROMPT
        })];

        for (role, content) in &messages {
            if role == "user" || role == "assistant" {
                // Strip /no_think suffix from user messages
                let clean = content.trim_end_matches(" /no_think").to_string();
                chat_msgs.push(serde_json::json!({
                    "role": role,
                    "content": clean,
                }));
            }
        }

        training_examples.push(serde_json::json!({
            "messages": chat_msgs
        }));
    }

    if training_examples.is_empty() {
        return Err("No conversations suitable for training".into());
    }

    // 80/20 split
    let split_idx = (training_examples.len() as f64 * 0.8).ceil() as usize;
    let (train, valid) = training_examples.split_at(split_idx);

    // Write files
    let output_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Documents/Hanni/training");
    std::fs::create_dir_all(&output_dir).map_err(|e| format!("Dir error: {}", e))?;

    let train_path = output_dir.join("train.jsonl");
    let valid_path = output_dir.join("valid.jsonl");

    let mut train_file = std::fs::File::create(&train_path).map_err(|e| format!("File error: {}", e))?;
    for example in train {
        writeln!(train_file, "{}", serde_json::to_string(example).unwrap_or_default())
            .map_err(|e| format!("Write error: {}", e))?;
    }

    let mut valid_file = std::fs::File::create(&valid_path).map_err(|e| format!("File error: {}", e))?;
    for example in valid {
        writeln!(valid_file, "{}", serde_json::to_string(example).unwrap_or_default())
            .map_err(|e| format!("Write error: {}", e))?;
    }

    Ok(serde_json::json!({
        "train_path": train_path.to_string_lossy(),
        "valid_path": valid_path.to_string_lossy(),
        "train_count": train.len(),
        "valid_count": valid.len(),
        "total": training_examples.len(),
    }))
}

// ── Phase 4: HTTP API ──

fn api_token_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Documents/Hanni/api_token.txt")
}

fn get_or_create_api_token() -> String {
    let path = api_token_path();
    if path.exists() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &token);
    token
}

async fn spawn_api_server(app_handle: AppHandle) {
    use axum::{Router, routing::{get, post}, extract::{State as AxumState, Query}, Json, http::{StatusCode, HeaderMap}};

    let api_token = get_or_create_api_token();

    #[derive(Clone)]
    struct ApiState {
        app: AppHandle,
        token: String,
    }

    let state = ApiState {
        app: app_handle,
        token: api_token,
    };

    fn check_auth(headers: &HeaderMap, token: &str) -> Result<(), (StatusCode, String)> {
        let auth = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let provided = auth.strip_prefix("Bearer ").unwrap_or(auth);
        if provided == token {
            Ok(())
        } else {
            Err((StatusCode::UNAUTHORIZED, "Invalid token".into()))
        }
    }

    #[derive(Deserialize)]
    struct ChatReq {
        message: String,
        history: Option<Vec<(String, String)>>,
    }

    #[derive(Deserialize)]
    struct SearchQuery {
        q: String,
        limit: Option<usize>,
    }

    #[derive(Deserialize)]
    struct RememberReq {
        category: String,
        key: String,
        value: String,
    }

    async fn api_status(
        AxumState(state): AxumState<ApiState>,
    ) -> Json<serde_json::Value> {
        // No auth required for status — allows frontend health check
        let busy = state.app.state::<LlmBusy>().0.load(Ordering::Relaxed);
        let focus_active = state.app.state::<FocusManager>().0.lock().map(|s| s.active).unwrap_or(false);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let model_online = client
            .get("http://127.0.0.1:8234/v1/models")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        Json(serde_json::json!({
            "status": "ok",
            "model_online": model_online,
            "llm_busy": busy,
            "focus_active": focus_active,
        }))
    }

    async fn api_chat(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<ChatReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let mut messages = req.history.unwrap_or_default();
        messages.push(("user".into(), req.message));

        match chat_inner(&state.app, messages).await {
            Ok(reply) => Ok(Json(serde_json::json!({ "reply": reply }))),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
        }
    }

    async fn api_memory_search(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Query(params): Query<SearchQuery>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.0.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
        let max = params.limit.unwrap_or(20) as i64;

        let words: Vec<&str> = params.q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
        let mut results = Vec::new();

        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        if results.is_empty() {
            let like_pattern = format!("%{}%", params.q);
            if let Ok(mut stmt) = conn.prepare(
                "SELECT category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        Ok(Json(serde_json::json!({ "results": results })))
    }

    async fn api_memory_add(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<RememberReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.0.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO facts (category, key, value, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'api', ?4, ?4)
             ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
            rusqlite::params![req.category, req.key, req.value, now],
        ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

        Ok(Json(serde_json::json!({ "status": "ok" })))
    }

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/chat", post(api_chat))
        .route("/api/memory/search", get(api_memory_search))
        .route("/api/memory", post(api_memory_add))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8235").await;
    match listener {
        Ok(listener) => {
            let _ = axum::serve(listener, app).await;
        }
        Err(e) => {
            eprintln!("Failed to start API server: {}", e);
        }
    }
}

fn find_python() -> Option<String> {
    // Try common locations for python3 with mlx_lm
    let candidates = [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/usr/bin/python3",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn start_mlx_server() -> Option<Child> {
    let python = find_python()?;

    // Check if server is already running
    let check = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .ok()?;
    if check.get("http://127.0.0.1:8234/v1/models").send().map(|r| r.status().is_success()).unwrap_or(false) {
        return None; // Already running
    }

    let child = Command::new(&python)
        .args(["-m", "mlx_lm.server", "--model", MODEL, "--port", "8234"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    Some(child)
}

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
    let busy = &app.state::<LlmBusy>().0;
    busy.store(true, Ordering::Relaxed);
    let result = chat_inner(&app, messages).await;
    busy.store(false, Ordering::Relaxed);
    result
}

async fn chat_inner(app: &AppHandle, messages: Vec<(String, String)>) -> Result<String, String> {
    let client = &app.state::<HttpClient>().0;

    let mut chat_messages = vec![ChatMessage {
        role: "system".into(),
        content: SYSTEM_PROMPT.into(),
    }];

    // Inject memory context from SQLite
    {
        let last_user_msg = messages.iter().rev()
            .find(|(role, _)| role == "user")
            .map(|(_, c)| c.as_str())
            .unwrap_or("");
        let ctx = {
            let db = app.state::<HanniDb>();
            let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
            build_memory_context_from_db(&conn, last_user_msg, 50)
        };
        if !ctx.is_empty() {
            chat_messages.push(ChatMessage {
                role: "system".into(),
                content: format!("[Your memories]\n{}", ctx),
            });
        }
    }

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
async fn get_activity_summary() -> Result<String, String> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/Knowledge/knowledgeC.db");

    if !db_path.exists() {
        return Err(
            "Screen Time data unavailable. Grant Full Disk Access: \
             System Settings → Privacy & Security → Full Disk Access → add Hanni"
                .into(),
        );
    }

    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if e.to_string().contains("unable to open") || e.to_string().contains("authorization denied") {
            "Screen Time data unavailable. Grant Full Disk Access: \
             System Settings → Privacy & Security → Full Disk Access → add Hanni"
                .to_string()
        } else {
            format!("Cannot open knowledgeC.db: {}", e)
        }
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT
                ZSOURCE.ZNAME as app_name,
                ZSOURCE.ZBUNDLEID as bundle_id,
                ROUND(SUM(CAST((ZOBJECT.ZENDDATE - ZOBJECT.ZSTARTDATE) AS REAL)) / 60, 1) as minutes
            FROM ZOBJECT
            JOIN ZSOURCE ON ZOBJECT.ZSOURCE = ZSOURCE.Z_PK
            WHERE
                DATE(datetime(ZOBJECT.ZSTARTDATE + 978307200, 'unixepoch', 'localtime')) = DATE('now')
                AND ZOBJECT.ZSTREAMNAME = '/app/inFocus'
                AND ZOBJECT.ZENDDATE > ZOBJECT.ZSTARTDATE
            GROUP BY ZSOURCE.ZBUNDLEID
            ORDER BY minutes DESC",
        )
        .map_err(|e| format!("SQL error: {}", e))?;

    struct AppRow {
        app_name: String,
        minutes: f64,
        category: String,
    }

    let rows: Vec<AppRow> = stmt
        .query_map([], |row| {
            let app_name: String = row.get::<_, Option<String>>(0)?.unwrap_or_default();
            let minutes: f64 = row.get(2)?;
            Ok((app_name, minutes))
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .map(|(app_name, minutes)| {
            let category = classify_app(&app_name).to_string();
            AppRow { app_name, minutes, category }
        })
        .collect();

    if rows.is_empty() {
        return Ok("No Screen Time data for today yet.".into());
    }

    let mut productive: f64 = 0.0;
    let mut distraction: f64 = 0.0;
    let mut neutral: f64 = 0.0;

    for r in &rows {
        match r.category.as_str() {
            "productive" => productive += r.minutes,
            "distraction" => distraction += r.minutes,
            _ => neutral += r.minutes,
        }
    }

    let top_apps: Vec<String> = rows
        .iter()
        .take(5)
        .map(|r| format!("  {} — {:.0} min ({})", r.app_name, r.minutes, r.category))
        .collect();

    Ok(format!(
        "Activity today (Screen Time):\n\
         Productive: {:.0} min | Distraction: {:.0} min | Neutral: {:.0} min\n\n\
         Top apps:\n{}",
        productive, distraction, neutral,
        top_apps.join("\n")
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

// ── Memory commands (SQLite) ──

#[tauri::command]
fn memory_remember(
    category: String,
    key: String,
    value: String,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO facts (category, key, value, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'user', ?4, ?4)
         ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
        rusqlite::params![category, key, value, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(format!("Remembered {}/{}={}", category, key, value))
}

#[tauri::command]
fn memory_recall(
    category: String,
    key: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    match key {
        Some(k) => {
            let result: Result<String, _> = conn.query_row(
                "SELECT value FROM facts WHERE category=?1 AND key=?2",
                rusqlite::params![category, k],
                |row| row.get(0),
            );
            match result {
                Ok(val) => Ok(format!("{}={}", k, val)),
                Err(_) => Ok(format!("No memory for {}/{}", category, k)),
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT key, value FROM facts WHERE category=?1 ORDER BY updated_at DESC"
            ).map_err(|e| format!("DB error: {}", e))?;
            let pairs: Vec<String> = stmt.query_map(rusqlite::params![category], |row| {
                Ok(format!("{}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("DB error: {}", e))?
            .flatten()
            .collect();
            if pairs.is_empty() {
                Ok(format!("No memories in category '{}'", category))
            } else {
                Ok(pairs.join(", "))
            }
        }
    }
}

#[tauri::command]
fn memory_forget(
    category: String,
    key: String,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let deleted = conn.execute(
        "DELETE FROM facts WHERE category=?1 AND key=?2",
        rusqlite::params![category, key],
    ).map_err(|e| format!("DB error: {}", e))?;
    if deleted > 0 {
        Ok(format!("Forgot {}/{}", category, key))
    } else {
        Ok(format!("No memory for {}/{}", category, key))
    }
}

#[tauri::command]
fn memory_search(
    query: String,
    limit: Option<usize>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let max = limit.unwrap_or(20) as i64;

    // Try FTS5 MATCH first
    let words: Vec<&str> = query.split_whitespace()
        .filter(|w| w.len() > 1)
        .take(10)
        .collect();
    if !words.is_empty() {
        let fts_query = words.join(" OR ");
        if let Ok(mut stmt) = conn.prepare(
            "SELECT f.category, f.key, f.value FROM facts_fts fts
             JOIN facts f ON f.id = fts.rowid
             WHERE facts_fts MATCH ?1
             ORDER BY rank LIMIT ?2"
        ) {
            let results: Vec<String> = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                Ok(format!("[{}] {}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| format!("DB error: {}", e))?
            .flatten()
            .collect();
            if !results.is_empty() {
                return Ok(results.join("\n"));
            }
        }
    }

    // Fallback: LIKE search
    let like_pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT category, key, value FROM facts
         WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1
         ORDER BY updated_at DESC LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let results: Vec<String> = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
        Ok(format!("[{}] {}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })
    .map_err(|e| format!("DB error: {}", e))?
    .flatten()
    .collect();

    if results.is_empty() {
        Ok("No memories found.".into())
    } else {
        Ok(results.join("\n"))
    }
}

#[tauri::command]
fn save_conversation(
    messages: Vec<(String, String)>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let msg_count = messages.len() as i64;
    conn.execute(
        "INSERT INTO conversations (started_at, message_count, messages) VALUES (?1, ?2, ?3)",
        rusqlite::params![now, msg_count, messages_json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_conversation(
    id: i64,
    messages: Vec<(String, String)>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let msg_count = messages.len() as i64;
    conn.execute(
        "UPDATE conversations SET messages=?1, message_count=?2, ended_at=?3 WHERE id=?4",
        rusqlite::params![messages_json, msg_count, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_conversations(
    limit: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let max = limit.unwrap_or(30);
    let mut stmt = conn.prepare(
        "SELECT id, started_at, summary, message_count FROM conversations
         ORDER BY started_at DESC LIMIT ?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![max], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "summary": row.get::<_, Option<String>>(2)?,
            "message_count": row.get::<_, i64>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
fn get_conversation(
    id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let (messages_json, summary, started_at): (String, Option<String>, String) = conn.query_row(
        "SELECT messages, summary, started_at FROM conversations WHERE id=?1",
        rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    let messages: Vec<(String, String)> = serde_json::from_str(&messages_json)
        .map_err(|e| format!("Parse error: {}", e))?;
    Ok(serde_json::json!({
        "id": id,
        "started_at": started_at,
        "summary": summary,
        "messages": messages,
    }))
}

#[tauri::command]
fn delete_conversation(
    id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    conn.execute("DELETE FROM conversations WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn search_conversations(
    query: String,
    limit: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let max = limit.unwrap_or(20);
    let words: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
    if words.is_empty() {
        return Ok(vec![]);
    }
    let fts_query = words.join(" OR ");
    let mut stmt = conn.prepare(
        "SELECT c.id, c.started_at, c.summary, c.message_count
         FROM conversations_fts fts
         JOIN conversations c ON c.id = fts.rowid
         WHERE conversations_fts MATCH ?1
         ORDER BY rank LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![fts_query, max], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "summary": row.get::<_, Option<String>>(2)?,
            "message_count": row.get::<_, i64>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
async fn process_conversation_end(
    messages: Vec<(String, String)>,
    conversation_id: i64,
    app: AppHandle,
) -> Result<(), String> {
    let client = &app.state::<HttpClient>().0;

    // Build a compact version of the conversation for the LLM
    let conv_text: String = messages.iter()
        .filter(|(role, _)| role == "user" || role == "assistant")
        .map(|(role, content)| format!("{}: {}", role, content))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Analyze this conversation and extract key facts about the user.\n\
        Return a JSON object with exactly this format (no other text):\n\
        {{\"summary\": \"1-2 sentence summary\", \"facts\": [{{\"category\": \"...\", \"key\": \"...\", \"value\": \"...\"}}]}}\n\
        Categories: user, preferences, world, tasks, people, habits.\n\
        Only extract genuinely new/important facts. If none, return empty facts array.\n\n\
        Conversation:\n{}\n/no_think", conv_text
    );

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage { role: "system".into(), content: "You extract structured data from conversations. Return only valid JSON.".into() },
            ChatMessage { role: "user".into(), content: prompt },
        ],
        max_tokens: 512,
        stream: false,
        temperature: 0.3,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    let parsed: NonStreamResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let raw = parsed.choices.first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think> tags
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    // Try to parse JSON from the response (it might be wrapped in ```json blocks)
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            &text
        }
    } else {
        &text
    };

    #[derive(Deserialize)]
    struct ExtractionResult {
        summary: Option<String>,
        #[serde(default)]
        facts: Vec<ExtractedFact>,
    }
    #[derive(Deserialize)]
    struct ExtractedFact {
        category: String,
        key: String,
        value: String,
    }

    if let Ok(result) = serde_json::from_str::<ExtractionResult>(json_str) {
        let db = app.state::<HanniDb>();
        let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
        let now = chrono::Local::now().to_rfc3339();

        // Update conversation summary
        if let Some(summary) = &result.summary {
            let _ = conn.execute(
                "UPDATE conversations SET summary=?1, ended_at=?2 WHERE id=?3",
                rusqlite::params![summary, now, conversation_id],
            );
        }

        // Insert extracted facts
        for fact in &result.facts {
            let _ = conn.execute(
                "INSERT INTO facts (category, key, value, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'auto', ?4, ?4)
                 ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
                rusqlite::params![fact.category, fact.key, fact.value, now],
            );
        }
    }

    Ok(())
}

// ── v0.7.0: Activities (Focus) commands ──

#[tauri::command]
fn start_activity(
    title: String,
    category: String,
    focus_mode: bool,
    duration: Option<u64>,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    db: tauri::State<'_, HanniDb>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO activities (title, category, started_at, focus_mode, created_at) VALUES (?1, ?2, ?3, ?4, ?3)",
        rusqlite::params![title, category, now, focus_mode as i32],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id = conn.last_insert_rowid();

    // Optionally start focus blocking
    if focus_mode {
        drop(conn);
        let dur = duration.unwrap_or(120);
        let _ = start_focus(dur, apps, sites, focus);
    }
    Ok(id)
}

#[tauri::command]
fn stop_activity(
    db: tauri::State<'_, HanniDb>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    // Find current (unfinished) activity
    let result: Result<(i64, String), _> = conn.query_row(
        "SELECT id, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok((id, started_at)) => {
            if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let duration = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                conn.execute(
                    "UPDATE activities SET ended_at=?1, duration_minutes=?2 WHERE id=?3",
                    rusqlite::params![now, duration, id],
                ).map_err(|e| format!("DB error: {}", e))?;
            } else {
                conn.execute(
                    "UPDATE activities SET ended_at=?1 WHERE id=?2",
                    rusqlite::params![now, id],
                ).map_err(|e| format!("DB error: {}", e))?;
            }
            // Stop focus if active
            drop(conn);
            let _ = stop_focus(focus);
            Ok("Activity stopped".into())
        }
        Err(_) => Ok("No active activity".into()),
    }
}

#[tauri::command]
fn get_current_activity(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let result: Result<(i64, String, String, String), _> = conn.query_row(
        "SELECT id, title, category, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );
    match result {
        Ok((id, title, category, started_at)) => {
            let elapsed = if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let mins = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                let h = mins / 60;
                let m = mins % 60;
                if h > 0 { format!("{}ч {}м", h, m) } else { format!("{}м", m) }
            } else { String::new() };
            Ok(serde_json::json!({ "id": id, "title": title, "category": category, "started_at": started_at, "elapsed": elapsed }))
        }
        Err(_) => Err("No active activity".into()),
    }
}

#[tauri::command]
fn get_activity_log(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn.prepare(
        "SELECT id, title, category, started_at, ended_at, duration_minutes FROM activities
         WHERE started_at LIKE ?1 ORDER BY started_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", target_date);
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| {
        let started: String = row.get(3)?;
        let time = if started.len() >= 16 { started[11..16].to_string() } else { String::new() };
        let dur_min: Option<i64> = row.get(5)?;
        let duration = dur_min.map(|m| if m >= 60 { format!("{}ч {}м", m/60, m%60) } else { format!("{}м", m) }).unwrap_or_default();
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "time": time,
            "duration": duration,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Notes commands ──

#[tauri::command]
fn create_note(title: String, content: String, tags: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO notes (title, content, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![title, content, tags, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_note(
    id: i64, title: String, content: String, tags: String,
    pinned: Option<bool>, archived: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    // Get current values for pinned/archived if not provided
    let (cur_pinned, cur_archived): (i32, i32) = conn.query_row(
        "SELECT pinned, archived FROM notes WHERE id=?1", rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0, 0));
    let p = pinned.map(|v| v as i32).unwrap_or(cur_pinned);
    let a = archived.map(|v| v as i32).unwrap_or(cur_archived);
    conn.execute(
        "UPDATE notes SET title=?1, content=?2, tags=?3, pinned=?4, archived=?5, updated_at=?6 WHERE id=?7",
        rusqlite::params![title, content, tags, p, a, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn delete_note(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    conn.execute("DELETE FROM notes WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_notes(_filter: Option<String>, search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let rows = if let Some(q) = search {
        if q.trim().is_empty() { get_notes_all(&conn)? }
        else {
            let words: Vec<&str> = q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
            if words.is_empty() { get_notes_all(&conn)? }
            else {
                let fts_query = words.join(" OR ");
                let mut stmt = conn.prepare(
                    "SELECT n.id, n.title, n.content, n.tags, n.pinned, n.archived, n.created_at, n.updated_at
                     FROM notes_fts fts JOIN notes n ON n.id = fts.rowid
                     WHERE notes_fts MATCH ?1 ORDER BY rank LIMIT 50"
                ).map_err(|e| format!("DB error: {}", e))?;
                let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![fts_query], |row| note_from_row(row))
                    .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
                result
            }
        }
    } else {
        get_notes_all(&conn)?
    };
    Ok(rows)
}

fn get_notes_all(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at FROM notes
         WHERE archived=0 ORDER BY pinned DESC, updated_at DESC LIMIT 100"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| note_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

fn note_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "title": row.get::<_, String>(1)?,
        "content": row.get::<_, String>(2)?,
        "tags": row.get::<_, String>(3)?,
        "pinned": row.get::<_, i32>(4)? != 0,
        "archived": row.get::<_, i32>(5)? != 0,
        "created_at": row.get::<_, String>(6)?,
        "updated_at": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
fn get_note(id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    conn.query_row(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at FROM notes WHERE id=?1",
        rusqlite::params![id],
        |row| note_from_row(row),
    ).map_err(|e| format!("Not found: {}", e))
}

// ── v0.7.0: Events (Calendar) commands ──

#[tauri::command]
fn create_event(title: String, description: String, date: String, time: String, duration_minutes: i64, category: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO events (title, description, date, time, duration_minutes, category, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![title, description, date, time, duration_minutes, category, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_events(month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let prefix = format!("{}-{:02}", year, month);
    let mut stmt = conn.prepare(
        "SELECT id, title, description, date, time, duration_minutes, category, color, completed FROM events WHERE date LIKE ?1 ORDER BY date, time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", prefix);
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "time": row.get::<_, String>(4)?,
            "duration_minutes": row.get::<_, i64>(5)?,
            "category": row.get::<_, String>(6)?,
            "color": row.get::<_, String>(7)?,
            "completed": row.get::<_, i32>(8)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn delete_event(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    conn.execute("DELETE FROM events WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.7.0: Projects & Tasks (Work) commands ──

#[tauri::command]
fn create_project(name: String, description: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (name, description, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![name, description, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_projects(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.description, p.status, p.color, p.created_at,
                (SELECT COUNT(*) FROM tasks WHERE project_id=p.id) as task_count
         FROM projects p WHERE p.status='active' ORDER BY p.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "color": row.get::<_, String>(4)?,
            "task_count": row.get::<_, i64>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn create_task(project_id: i64, title: String, description: String, priority: String, due_date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tasks (project_id, title, description, priority, due_date, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![project_id, title, description, priority, due_date, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_tasks(project_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, description, status, priority, due_date, completed_at FROM tasks
         WHERE project_id=?1 ORDER BY CASE status WHEN 'todo' THEN 0 WHEN 'in_progress' THEN 1 ELSE 2 END, created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![project_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "priority": row.get::<_, String>(4)?,
            "due_date": row.get::<_, Option<String>>(5)?,
            "completed_at": row.get::<_, Option<String>>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_task_status(id: i64, status: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    let completed_at = if status == "done" { Some(now.clone()) } else { None };
    conn.execute(
        "UPDATE tasks SET status=?1, completed_at=?2 WHERE id=?3",
        rusqlite::params![status, completed_at, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.7.0: Learning Items (Development) commands ──

#[tauri::command]
fn create_learning_item(item_type: String, title: String, description: String, url: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO learning_items (type, title, description, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![item_type, title, description, url, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_learning_items(type_filter: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let rows = if let Some(t) = type_filter {
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, url, progress, status, category FROM learning_items WHERE type=?1 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![t], |row| learning_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        result
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, url, progress, status, category FROM learning_items ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result: Vec<serde_json::Value> = stmt.query_map([], |row| learning_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        result
    };
    Ok(rows)
}

fn learning_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "type": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?,
        "description": row.get::<_, String>(3)?,
        "url": row.get::<_, String>(4)?,
        "progress": row.get::<_, i32>(5)?,
        "status": row.get::<_, String>(6)?,
        "category": row.get::<_, String>(7)?,
    }))
}

// ── v0.7.0: Hobbies commands ──

#[tauri::command]
fn create_hobby(name: String, category: String, icon: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO hobbies (name, category, icon, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, category, icon, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_hobbies(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT h.id, h.name, h.category, h.icon, h.color,
                COALESCE((SELECT SUM(duration_minutes) FROM hobby_entries WHERE hobby_id=h.id), 0) / 60.0 as total_hours
         FROM hobbies h ORDER BY h.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "color": row.get::<_, String>(4)?,
            "total_hours": format!("{:.1}", row.get::<_, f64>(5)?),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn log_hobby_entry(hobby_id: i64, duration_minutes: i64, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO hobby_entries (hobby_id, date, duration_minutes, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![hobby_id, date, duration_minutes, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_hobby_entries(hobby_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, date, duration_minutes, notes FROM hobby_entries WHERE hobby_id=?1 ORDER BY date DESC LIMIT 30"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![hobby_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "date": row.get::<_, String>(1)?,
            "duration_minutes": row.get::<_, i64>(2)?,
            "notes": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Workouts (Sports) commands ──

#[tauri::command]
fn create_workout(workout_type: String, title: String, duration_minutes: i64, calories: Option<i64>, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO workouts (type, title, date, duration_minutes, calories, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![workout_type, title, date, duration_minutes, calories, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_workouts(_date_range: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, type, title, date, duration_minutes, calories, notes FROM workouts ORDER BY date DESC, created_at DESC LIMIT 50"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, String>(1)?,
            "title": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "duration_minutes": row.get::<_, i64>(4)?,
            "calories": row.get::<_, Option<i64>>(5)?,
            "notes": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn get_workout_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let (count, total_min, total_cal): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(duration_minutes), 0), COALESCE(SUM(calories), 0) FROM workouts WHERE date >= ?1",
        rusqlite::params![week_ago],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0, 0));
    Ok(serde_json::json!({ "count": count, "total_minutes": total_min, "total_calories": total_cal }))
}

// ── v0.7.0: Health & Habits commands ──

#[tauri::command]
fn log_health(health_type: String, value: f64, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let unit = match health_type.as_str() {
        "sleep" => "hours", "water" => "glasses", "weight" => "kg", "mood" => "1-5", "steps" => "steps",
        _ => "",
    };
    // Upsert: update if same date+type exists
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM health_log WHERE date=?1 AND type=?2 LIMIT 1",
        rusqlite::params![date, health_type],
        |row| row.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE health_log SET value=?1, notes=?2 WHERE id=?3",
            rusqlite::params![value, notes.unwrap_or_default(), id],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO health_log (date, type, value, unit, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![date, health_type, value, unit, notes.unwrap_or_default(), now.to_rfc3339()],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(conn.last_insert_rowid())
    }
}

#[tauri::command]
fn get_health_today(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT type, value FROM health_log WHERE date=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let mut result = serde_json::json!({});
    let rows = stmt.query_map(rusqlite::params![today], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?;
    for row in rows.flatten() {
        result[row.0] = serde_json::json!(row.1);
    }
    Ok(result)
}

#[tauri::command]
fn create_habit(name: String, icon: String, frequency: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO habits (name, icon, frequency, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, icon, frequency, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn check_habit(habit_id: i64, date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let now = chrono::Local::now().to_rfc3339();
    // Toggle: if exists, delete; else insert
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM habit_checks WHERE habit_id=?1 AND date=?2",
        rusqlite::params![habit_id, target_date],
        |row| row.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute("DELETE FROM habit_checks WHERE id=?1", rusqlite::params![id])
            .map_err(|e| format!("DB error: {}", e))?;
    } else {
        conn.execute(
            "INSERT INTO habit_checks (habit_id, date, completed, created_at) VALUES (?1, ?2, 1, ?3)",
            rusqlite::params![habit_id, target_date, now],
        ).map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn get_habits_today(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT h.id, h.name, h.icon, h.frequency,
                (SELECT COUNT(*) FROM habit_checks WHERE habit_id=h.id AND date=?1) as checked,
                (SELECT COUNT(*) FROM habit_checks hc WHERE hc.habit_id=h.id AND hc.date >= date(?1, '-30 days')) as streak_approx
         FROM habits h ORDER BY h.created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![today], |row| {
        // Simple streak calc: count consecutive days backward
        let checked: i64 = row.get(4)?;
        let streak_approx: i64 = row.get(5)?;
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "icon": row.get::<_, String>(2)?,
            "frequency": row.get::<_, String>(3)?,
            "completed": checked > 0,
            "streak": streak_approx,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Dashboard aggregate command ──

#[tauri::command]
fn get_dashboard_data(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today_pattern = format!("{}%", today);

    // Current activity
    let current_activity: Option<serde_json::Value> = conn.query_row(
        "SELECT title, category, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            let started: String = row.get(2)?;
            let elapsed = if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started) {
                let mins = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                format!("{}м", mins)
            } else { String::new() };
            Ok(serde_json::json!({ "title": row.get::<_, String>(0)?, "category": row.get::<_, String>(1)?, "elapsed": elapsed }))
        },
    ).ok();

    // Activities count today
    let activities_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activities WHERE started_at LIKE ?1", rusqlite::params![today_pattern], |row| row.get(0),
    ).unwrap_or(0);

    // Focus minutes today
    let focus_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_minutes), 0) FROM activities WHERE started_at LIKE ?1 AND ended_at IS NOT NULL",
        rusqlite::params![today_pattern], |row| row.get(0),
    ).unwrap_or(0);

    // Notes count
    let notes_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes WHERE archived=0", [], |row| row.get(0)).unwrap_or(0);

    // Events today
    let mut events_stmt = conn.prepare(
        "SELECT title, time FROM events WHERE date=?1 ORDER BY time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let events: Vec<serde_json::Value> = events_stmt.query_map(rusqlite::params![today], |row| {
        Ok(serde_json::json!({ "title": row.get::<_, String>(0)?, "time": row.get::<_, String>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    // Recent notes
    let mut notes_stmt = conn.prepare(
        "SELECT title FROM notes WHERE archived=0 ORDER BY updated_at DESC LIMIT 3"
    ).map_err(|e| format!("DB error: {}", e))?;
    let recent_notes: Vec<serde_json::Value> = notes_stmt.query_map([], |row| {
        Ok(serde_json::json!({ "title": row.get::<_, String>(0)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    Ok(serde_json::json!({
        "current_activity": current_activity,
        "activities_today": activities_today,
        "focus_minutes": focus_minutes,
        "notes_count": notes_count,
        "events_today": events.len(),
        "events": events,
        "recent_notes": recent_notes,
    }))
}

// ── v0.7.0: Memory browser command ──

#[tauri::command]
fn get_all_memories(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    if let Some(q) = search {
        if !q.trim().is_empty() {
            let like = format!("%{}%", q);
            let mut stmt = conn.prepare(
                "SELECT id, category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1 ORDER BY updated_at DESC LIMIT 100"
            ).map_err(|e| format!("DB error: {}", e))?;
            let rows = stmt.query_map(rusqlite::params![like], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "category": row.get::<_, String>(1)?,
                    "key": row.get::<_, String>(2)?,
                    "value": row.get::<_, String>(3)?,
                }))
            }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
            return Ok(rows);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT id, category, key, value FROM facts ORDER BY category, updated_at DESC LIMIT 200"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "category": row.get::<_, String>(1)?,
            "key": row.get::<_, String>(2)?,
            "value": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn delete_memory(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    conn.execute("DELETE FROM facts WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn update_memory(id: i64, value: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("DB lock error: {}", e))?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute("UPDATE facts SET value=?1, updated_at=?2 WHERE id=?3", rusqlite::params![value, now, id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
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
async fn get_integrations() -> Result<IntegrationsInfo, String> {
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
    let macos = vec![
        IntegrationItem {
            name: "Screen Time".into(),
            status: "ready".into(),
            detail: "knowledgeC.db · по запросу".into(),
        },
        IntegrationItem {
            name: "Календарь".into(),
            status: "ready".into(),
            detail: "Calendar.app · по запросу".into(),
        },
        IntegrationItem {
            name: "Музыка".into(),
            status: "ready".into(),
            detail: "Music / Spotify · по запросу".into(),
        },
        IntegrationItem {
            name: "Браузер".into(),
            status: "ready".into(),
            detail: "Safari / Chrome / Arc · по запросу".into(),
        },
    ];

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

// ── Proactive messaging logic ──

const PROACTIVE_SYSTEM_PROMPT: &str = r#"You are Hanni, a warm AI companion living on the user's Mac. You're like a close friend who shares the same space. You see what's on the screen, what music is playing, what's in the calendar.

Your job: write a short message to the user. Be natural, like texting a friend.

WHAT YOU CAN DO:
- Comment on what they're doing, listening to, browsing
- Mention an upcoming calendar event
- React to their screen time (too much distraction? productive streak?)
- Share a thought, observation, or gentle nudge
- Just say hi if it's been a while — you live here, it's natural
- Be playful, curious, warm

RULES:
- 1-2 sentences max
- Default to Russian
- NEVER repeat your last message (it's shown below if exists)
- Be yourself — a companion, not a notification bot
- Only reply [SKIP] if you literally just sent a message and nothing changed

Reply with your message text, or [SKIP]."#;

async fn gather_context() -> String {
    let now = chrono::Local::now();
    let mut ctx = format!("Current time: {}\n", now.format("%H:%M %A, %d %B %Y"));

    if let Ok(activity) = get_activity_summary().await {
        ctx.push_str(&format!("\n--- Screen Time ---\n{}\n", activity));
    }

    if let Ok(calendar) = get_calendar_events().await {
        ctx.push_str(&format!("\n--- Calendar ---\n{}\n", calendar));
    }

    if let Ok(music) = get_now_playing().await {
        ctx.push_str(&format!("\n--- Music ---\n{}\n", music));
    }

    if let Ok(browser) = get_browser_tab().await {
        ctx.push_str(&format!("\n--- Browser ---\n{}\n", browser));
    }

    ctx
}

#[derive(Deserialize)]
struct NonStreamChoice {
    message: NonStreamMessage,
}

#[derive(Deserialize)]
struct NonStreamMessage {
    content: String,
}

#[derive(Deserialize)]
struct NonStreamResponse {
    choices: Vec<NonStreamChoice>,
}

async fn proactive_llm_call(
    client: &reqwest::Client,
    context: &str,
    last_message: &str,
    consecutive_skips: u32,
    memory_context: &str,
) -> Result<Option<String>, String> {
    let mut user_content = String::new();
    if !memory_context.is_empty() {
        user_content.push_str(&format!("[Your memories]\n{}\n\n", memory_context));
    }
    user_content.push_str(&format!("{}\n", context));
    if !last_message.is_empty() {
        user_content.push_str(&format!("Your last proactive message was: \"{}\"\n", last_message));
    }
    user_content.push_str(&format!(
        "You've skipped {} times in a row since your last message.\n/no_think",
        consecutive_skips
    ));

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: PROACTIVE_SYSTEM_PROMPT.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_content,
            },
        ],
        max_tokens: 200,
        stream: false,
        temperature: 0.7,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    let parsed: NonStreamResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let raw = parsed
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think> tags
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    if text.contains("[SKIP]") || text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn speak_tts(text: &str, voice: &str) {
    let clean = text.replace('"', "'");
    let _ = std::process::Command::new("say")
        .args(["-v", voice, "-r", "210", &clean])
        .spawn();
}

#[tauri::command]
async fn speak_text(text: String, voice: Option<String>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "Milena".into());
    let clean = text.replace('"', "'");
    std::process::Command::new("say")
        .args(["-v", &v, "-r", "210", &clean])
        .spawn()
        .map_err(|e| format!("TTS error: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn stop_speaking() -> Result<(), String> {
    let _ = std::process::Command::new("killall").arg("say").output();
    Ok(())
}

// ── Proactive messaging commands ──

#[tauri::command]
async fn get_proactive_settings(state: tauri::State<'_, Arc<Mutex<ProactiveState>>>) -> Result<ProactiveSettings, String> {
    let state = state.lock().await;
    Ok(state.settings.clone())
}

#[tauri::command]
async fn set_proactive_settings(
    settings: ProactiveSettings,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    save_proactive_settings(&settings)?;
    let mut state = state.lock().await;
    state.settings = settings;
    Ok(())
}

#[tauri::command]
async fn set_user_typing(
    typing: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.user_is_typing = typing;
    Ok(())
}

// ── Updater ──

const UPDATER_PAT: &str = env!("UPDATER_GITHUB_TOKEN");

fn updater_with_headers(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    app.updater_builder()
        .header("Authorization", &format!("token {}", UPDATER_PAT))
        .map_err(|e| format!("Header error: {}", e))?
        .header("Accept", "application/octet-stream")
        .map_err(|e| format!("Header error: {}", e))?
        .build()
        .map_err(|e| format!("Updater error: {}", e))
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<String, String> {
    let updater = updater_with_headers(&app)?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let _ = app.emit("update-available", &version);
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| format!("Install error: {}", e))?;
            app.restart();
        }
        Ok(None) => Ok("Вы на последней версии.".into()),
        Err(e) => Err(format!("Не удалось проверить обновления: {}", e)),
    }
}

// ── App setup ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let proactive_settings = load_proactive_settings();
    let proactive_state = Arc::new(Mutex::new(ProactiveState::new(proactive_settings)));

    // Initialize SQLite database
    let db_path = hanni_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = rusqlite::Connection::open(&db_path)
        .expect("Cannot open hanni.db");
    init_db(&conn).expect("Cannot initialize database");
    migrate_memory_json(&conn);
    let hanni_db = HanniDb(std::sync::Mutex::new(conn));

    // Start MLX server if not already running
    let mlx_child = start_mlx_server();
    let mlx_process = Arc::new(MlxProcess(std::sync::Mutex::new(mlx_child)));
    let mlx_cleanup = mlx_process.clone();

    // Audio recording state (capture starts lazily on first recording)
    let audio_state = Arc::new(AudioRecording(std::sync::Mutex::new(WhisperState {
        recording: false,
        audio_buffer: Vec::new(),
        capture_running: false,
    })));

    // Focus mode state
    let focus_monitor_flag = Arc::new(AtomicBool::new(false));
    let focus_manager = FocusManager(std::sync::Mutex::new(FocusState {
        active: false,
        end_time: None,
        blocked_apps: Vec::new(),
        blocked_sites: Vec::new(),
        monitor_running: focus_monitor_flag.clone(),
    }));

    tauri::Builder::default()
        .manage(HttpClient(reqwest::Client::new()))
        .manage(LlmBusy(AtomicBool::new(false)))
        .manage(proactive_state.clone())
        .manage(hanni_db)
        .manage(audio_state)
        .manage(focus_manager)
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
            get_calendar_events,
            get_now_playing,
            get_browser_tab,
            check_update,
            get_proactive_settings,
            set_proactive_settings,
            set_user_typing,
            memory_remember,
            memory_recall,
            memory_forget,
            memory_search,
            save_conversation,
            update_conversation,
            get_conversations,
            get_conversation,
            delete_conversation,
            search_conversations,
            process_conversation_end,
            // Phase 2: TTS
            speak_text,
            stop_speaking,
            // Phase 1: Voice
            download_whisper_model,
            start_recording,
            stop_recording,
            check_whisper_model,
            // Phase 2: Focus
            start_focus,
            stop_focus,
            get_focus_status,
            update_blocklist,
            // Phase 3: Training
            get_training_stats,
            export_training_data,
            // Phase 5: Actions
            run_shell,
            open_url,
            send_notification,
            set_volume,
            get_clipboard,
            set_clipboard,
            // v0.7.0: Activities (Focus)
            start_activity,
            stop_activity,
            get_current_activity,
            get_activity_log,
            // v0.7.0: Notes
            create_note,
            update_note,
            delete_note,
            get_notes,
            get_note,
            // v0.7.0: Events (Calendar)
            create_event,
            get_events,
            delete_event,
            // v0.7.0: Projects & Tasks (Work)
            create_project,
            get_projects,
            create_task,
            get_tasks,
            update_task_status,
            // v0.7.0: Learning Items (Development)
            create_learning_item,
            get_learning_items,
            // v0.7.0: Hobbies
            create_hobby,
            get_hobbies,
            log_hobby_entry,
            get_hobby_entries,
            // v0.7.0: Workouts (Sports)
            create_workout,
            get_workouts,
            get_workout_stats,
            // v0.7.0: Health & Habits
            log_health,
            get_health_today,
            create_habit,
            check_habit,
            get_habits_today,
            // v0.7.0: Dashboard
            get_dashboard_data,
            // v0.7.0: Memory browser
            get_all_memories,
            delete_memory,
            update_memory,
        ])
        .setup(move |app| {
            // Auto-updater
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let updater = match updater_with_headers(&handle) {
                    Ok(u) => u,
                    Err(_) => return,
                };
                match updater.check().await {
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

            // HTTP API server (Phase 4)
            let api_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                spawn_api_server(api_handle).await;
            });

            // Focus mode monitor loop
            let focus_handle = app.handle().clone();
            let focus_flag = focus_monitor_flag.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if !focus_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let focus = focus_handle.state::<FocusManager>();
                    let (active, end_time, apps) = {
                        let state = match focus.0.lock() {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        (state.active, state.end_time, state.blocked_apps.clone())
                    };

                    if !active {
                        continue;
                    }

                    // Check if focus timer expired
                    if let Some(end) = end_time {
                        if chrono::Local::now() >= end {
                            // Auto-stop focus mode
                            let script = "do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges";
                            let _ = run_osascript(script);
                            if let Ok(mut state) = focus.0.lock() {
                                state.active = false;
                                state.end_time = None;
                                state.blocked_apps.clear();
                                state.blocked_sites.clear();
                                state.monitor_running.store(false, Ordering::Relaxed);
                            }
                            let _ = focus_handle.emit("focus-ended", ());
                            continue;
                        }
                    }

                    // Kill blocked apps if they relaunch
                    for app_name in &apps {
                        let _ = run_osascript(&format!(
                            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
                            app_name, app_name
                        ));
                    }
                }
            });

            // Proactive messaging background loop
            let proactive_handle = app.handle().clone();
            let proactive_state_ref = proactive_state.clone();
            tauri::async_runtime::spawn(async move {
                let client = reqwest::Client::new();

                // Initial delay — let the app fully start
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                let mut last_check = std::time::Instant::now();
                let mut first_run = true;

                loop {
                    // Poll every 5 seconds so we react quickly to settings changes
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    let (enabled, interval, quiet_start, quiet_end, is_typing, last_msg, skips, voice_enabled, voice_name) = {
                        let state = proactive_state_ref.lock().await;
                        (
                            state.settings.enabled,
                            state.settings.interval_minutes,
                            state.settings.quiet_hours_start,
                            state.settings.quiet_hours_end,
                            state.user_is_typing,
                            state.last_message_text.clone(),
                            state.consecutive_skips,
                            state.settings.voice_enabled,
                            state.settings.voice_name.clone(),
                        )
                    };

                    if !enabled {
                        first_run = true;
                        continue;
                    }

                    // On first run after enabling, fire immediately; otherwise wait for interval
                    let interval_secs = if skips >= 6 { interval * 2 * 60 } else { interval * 60 };
                    if !first_run && last_check.elapsed().as_secs() < interval_secs {
                        continue;
                    }

                    let hour = chrono::Local::now().hour();
                    let in_quiet = if quiet_start > quiet_end {
                        hour >= quiet_start || hour < quiet_end
                    } else {
                        hour >= quiet_start && hour < quiet_end
                    };

                    let llm_busy = proactive_handle.state::<LlmBusy>().0.load(Ordering::Relaxed);

                    if in_quiet || is_typing || llm_busy {
                        continue;
                    }

                    last_check = std::time::Instant::now();
                    first_run = false;

                    let context = gather_context().await;
                    let mem_ctx = {
                        let db = proactive_handle.state::<HanniDb>();
                        let result = match db.0.lock() {
                            Ok(conn) => build_memory_context_from_db(&conn, "", 30),
                            Err(_) => String::new(),
                        };
                        result
                    };
                    match proactive_llm_call(&client, &context, &last_msg, skips, &mem_ctx).await {
                        Ok(Some(message)) => {
                            let _ = proactive_handle.emit("proactive-message", &message);
                            if voice_enabled {
                                speak_tts(&message, &voice_name);
                            }
                            let mut state = proactive_state_ref.lock().await;
                            state.last_message_time = Some(chrono::Local::now());
                            state.last_message_text = message;
                            state.consecutive_skips = 0;
                        }
                        Ok(None) => {
                            let mut state = proactive_state_ref.lock().await;
                            state.consecutive_skips += 1;
                        }
                        Err(_) => {
                            // LLM server not running — back off
                            last_check = std::time::Instant::now();
                        }
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Hanni")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Kill MLX server process on app exit
                if let Ok(mut child) = mlx_cleanup.0.lock() {
                    if let Some(ref mut proc) = *child {
                        let _ = proc.kill();
                    }
                }
            }
        });
}
