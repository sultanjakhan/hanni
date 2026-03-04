// types.rs — All struct/type definitions, static atomics, constants, small helpers
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ── Constants ──

pub const MLX_URL: &str = "http://127.0.0.1:8234/v1/chat/completions";
pub const MODEL: &str = "NexVeridian/Qwen3.5-35B-A3B-4bit";
pub const VOICE_SERVER_URL: &str = "http://127.0.0.1:8237";

// OpenClaw Gateway
pub const OPENCLAW_URL: &str = "http://127.0.0.1:18789/v1/chat/completions";
pub const OPENCLAW_TOKEN: &str = "b948b1e8eebab8c447035ad7b0c0c61e6242861f90f32e5e";

// ── Static atomics ──

pub static CALENDAR_ACCESS_DENIED: AtomicBool = AtomicBool::new(false);
pub static CALENDAR_ACCESS_CHECKED: AtomicBool = AtomicBool::new(false);
pub static APPLE_CALENDAR_DISABLED: AtomicBool = AtomicBool::new(false);
pub static IS_SPEAKING: AtomicBool = AtomicBool::new(false);

// ── Speaking guard (RAII) ──

pub struct SpeakingGuard;
impl Drop for SpeakingGuard {
    fn drop(&mut self) {
        IS_SPEAKING.store(false, Ordering::Relaxed);
    }
}

// ── Life Tracker data types ──

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackerData {
    pub purchases: Vec<serde_json::Value>,
    #[serde(rename = "timeEntries")]
    pub time_entries: Vec<serde_json::Value>,
    pub goals: Vec<serde_json::Value>,
    pub notes: Vec<serde_json::Value>,
    #[serde(default)]
    pub settings: serde_json::Value,
}

// ── Proactive messaging types ──

fn default_daily_limit() -> u32 {
    20
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProactiveSettings {
    pub enabled: bool,
    pub voice_enabled: bool,
    pub voice_name: String,
    pub interval_minutes: u64,
    pub quiet_hours_start: u32,
    pub quiet_hours_end: u32,
    #[serde(default)]
    pub quiet_start_time: String,
    #[serde(default)]
    pub quiet_end_time: String,
    #[serde(default)]
    pub enabled_styles: Vec<String>,
    #[serde(default = "default_daily_limit")]
    pub daily_limit: u32,
}

impl ProactiveSettings {
    pub fn quiet_start_minutes(&self) -> u32 {
        parse_time_to_minutes(&self.quiet_start_time).unwrap_or(self.quiet_hours_start * 60)
    }

    pub fn quiet_end_minutes(&self) -> u32 {
        parse_time_to_minutes(&self.quiet_end_time).unwrap_or(self.quiet_hours_end * 60)
    }
}

pub fn parse_time_to_minutes(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let h = parts[0].parse::<u32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        Some(h * 60 + m)
    } else {
        None
    }
}

impl Default for ProactiveSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            voice_enabled: false,
            voice_name: "xenia".into(),
            interval_minutes: 10,
            quiet_hours_start: 23,
            quiet_hours_end: 8,
            quiet_start_time: String::new(),
            quiet_end_time: String::new(),
            enabled_styles: Vec::new(),
            daily_limit: 20,
        }
    }
}

pub struct ProactiveState {
    pub settings: ProactiveSettings,
    pub last_message_time: Option<chrono::DateTime<chrono::Local>>,
    pub last_message_text: String,
    pub consecutive_skips: u32,
    pub user_is_typing: bool,
    pub is_recording: bool,
    pub auto_quiet: bool,
    pub recent_messages: Vec<(String, chrono::DateTime<chrono::Local>)>,
    pub last_context_snapshot: String,
    pub last_proactive_id: Option<i64>,
    pub engagement_rate: f64,
    pub last_user_chat_time: Option<chrono::DateTime<chrono::Local>>,
    pub pending_triggers: Vec<(String, std::time::Instant)>,
}

impl ProactiveState {
    pub fn new(settings: ProactiveSettings) -> Self {
        Self {
            settings,
            last_message_time: None,
            last_message_text: String::new(),
            consecutive_skips: 0,
            user_is_typing: false,
            is_recording: false,
            auto_quiet: false,
            recent_messages: Vec::new(),
            last_context_snapshot: String::new(),
            last_proactive_id: None,
            engagement_rate: 0.5,
            last_user_chat_time: None,
            pending_triggers: Vec::new(),
        }
    }
}

// ── SQLite wrapper ──

pub struct HanniDb(pub std::sync::Mutex<rusqlite::Connection>);

impl HanniDb {
    pub fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// ~/Library/Application Support/Hanni/
pub fn hanni_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join("Library/Application Support")
        })
        .join("Hanni")
}

pub fn hanni_db_path() -> PathBuf {
    hanni_data_dir().join("hanni.db")
}

pub fn data_file_path() -> PathBuf {
    hanni_data_dir().join("life-tracker-data.json")
}

// ── Chat types ──

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallResult>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn text(role: &str, content: &str) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    #[allow(dead_code)]
    pub fn tool_result(tool_call_id: &str, name: &str, content: &str) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
        }
    }
}

#[derive(Serialize)]
pub struct ChatTemplateKwargs {
    pub enable_thinking: bool,
}

#[derive(Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub stream: bool,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f32>,
    pub chat_template_kwargs: ChatTemplateKwargs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize, Debug)]
pub struct Delta {
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub delta: Option<Delta>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct StreamChunk {
    pub choices: Vec<Choice>,
}

// ── Tool calling types ──

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCallDelta {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: Option<ToolCallFunction>,
    #[serde(rename = "type", default)]
    #[allow(dead_code)]
    pub call_type: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCallFunction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolCallResult {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallResultFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolCallResultFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ChatResult {
    pub text: String,
    pub tool_calls: Vec<ToolCallResult>,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct TokenPayload {
    pub token: String,
}

pub struct HttpClient(pub reqwest::Client);
pub struct LlmBusy(pub tokio::sync::Semaphore);

pub struct MlxProcess(pub std::sync::Mutex<Option<Child>>);
pub struct OpenClawProcess(pub std::sync::Mutex<Option<Child>>);

// ── Whisper / Voice state ──

pub struct WhisperState {
    pub recording: bool,
    pub audio_buffer: Vec<f32>,
    pub capture_running: bool,
}

pub struct AudioRecording(pub std::sync::Mutex<WhisperState>, pub std::sync::Condvar);

// ── Focus Mode state ──

pub struct FocusState {
    pub active: bool,
    pub end_time: Option<chrono::DateTime<chrono::Local>>,
    pub blocked_apps: Vec<String>,
    pub blocked_sites: Vec<String>,
    pub monitor_running: Arc<AtomicBool>,
}

pub struct FocusManager(pub std::sync::Mutex<FocusState>);

// ── Call Mode state ──

pub struct CallModeState {
    pub active: bool,
    pub phase: String,
    pub audio_buffer: Vec<f32>,
    pub speech_frames: u32,
    pub silence_frames: u32,
    pub barge_in: bool,
    pub last_recording: Vec<f32>,
    pub transcription_gen: u64,
}

pub struct CallMode(pub std::sync::Mutex<CallModeState>);

#[derive(Serialize, Clone)]
pub struct FocusStatus {
    pub active: bool,
    pub remaining_seconds: u64,
    pub blocked_apps: Vec<String>,
    pub blocked_sites: Vec<String>,
}

// ── Non-streaming response types (proactive) ──

#[derive(Deserialize)]
pub struct NonStreamChoice {
    pub message: NonStreamMessage,
}

#[derive(Deserialize)]
pub struct NonStreamMessage {
    pub content: String,
}

#[derive(Deserialize)]
pub struct NonStreamResponse {
    pub choices: Vec<NonStreamChoice>,
}

// ── Proactive style definitions ──

pub struct ProactiveStyleDef {
    pub id: &'static str,
    pub description: &'static str,
}

pub const ALL_PROACTIVE_STYLES: &[ProactiveStyleDef] = &[
    ProactiveStyleDef { id: "observation", description: "Наблюдение: комментарий к текущему приложению/музыке/браузеру" },
    ProactiveStyleDef { id: "calendar", description: "Календарь: напоминание о предстоящем событии" },
    ProactiveStyleDef { id: "nudge", description: "Подсказка: мягкое напоминание о продуктивности/здоровье" },
    ProactiveStyleDef { id: "curiosity", description: "Любопытство: вопрос о дне/проекте/настроении" },
    ProactiveStyleDef { id: "humor", description: "Юмор: лёгкая шутка привязанная к текущему контексту" },
    ProactiveStyleDef { id: "care", description: "Забота: проверить настроение, предложить перерыв" },
    ProactiveStyleDef { id: "memory", description: "Память: упомянуть факт из памяти, если он релевантен текущей ситуации" },
    ProactiveStyleDef { id: "food", description: "Еда: предупредить об истекающих продуктах" },
    ProactiveStyleDef { id: "goals", description: "Цели: прогресс или дедлайны" },
    ProactiveStyleDef { id: "journal", description: "Журнал: напомнить написать вечернюю рефлексию" },
    ProactiveStyleDef { id: "digest", description: "Дайджест: ТОЛЬКО утром (8-10) — краткий план дня" },
    ProactiveStyleDef { id: "accountability", description: "Ответственность: если залип в YouTube/Reddit/TikTok 30+ мин — мягко указать" },
    ProactiveStyleDef { id: "schedule", description: "Расписание: событие через 30 мин — напомнить подготовиться" },
    ProactiveStyleDef { id: "continuity", description: "Продолжение: развить тему из недавнего разговора с новой стороны" },
];

// ── Integrations types ──

#[derive(Serialize)]
pub struct IntegrationItem {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct IntegrationsInfo {
    pub access: Vec<IntegrationItem>,
    pub tracking: Vec<IntegrationItem>,
    pub blocked_apps: Vec<IntegrationItem>,
    pub blocked_sites: Vec<IntegrationItem>,
    pub blocker_active: bool,
    pub macos: Vec<IntegrationItem>,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub model_name: String,
    pub server_url: String,
    pub server_online: bool,
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub mlx_online: bool,
    pub mlx_model: String,
    pub voice_server_online: bool,
    pub db_ok: bool,
    pub db_tables: usize,
    pub db_facts: usize,
    pub db_conversations: usize,
    pub db_size_mb: f64,
}
