// types.rs — All struct/type definitions, static atomics, constants, small helpers
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

// ── Constants ──

pub const DEFAULT_LLM_BASE_URL: &str = "http://127.0.0.1:8234";
pub const MODEL: &str = "NexVeridian/Qwen3.5-35B-A3B-4bit";

// ── LLM endpoint override ──
// Runtime-configurable OpenAI-compatible server (app_settings keys
// 'llm_server_url' / 'llm_model'). Loaded into statics at init_database and
// refreshed by set_app_setting, so hot paths never hit the DB.

static LLM_BASE_URL: OnceLock<RwLock<String>> = OnceLock::new();
static LLM_MODEL: OnceLock<RwLock<String>> = OnceLock::new();

fn llm_base_cell() -> &'static RwLock<String> {
    LLM_BASE_URL.get_or_init(|| RwLock::new(DEFAULT_LLM_BASE_URL.to_string()))
}
fn llm_model_cell() -> &'static RwLock<String> {
    LLM_MODEL.get_or_init(|| RwLock::new(MODEL.to_string()))
}

/// Accepts "host:port", "http://host:port" or "http://host:port/" — normalizes
/// to a scheme-prefixed base without a trailing slash. Empty → default.
pub fn set_llm_base_url(url: &str) {
    let v = url.trim().trim_end_matches('/');
    let v = if v.is_empty() {
        DEFAULT_LLM_BASE_URL.to_string()
    } else if v.contains("://") {
        v.to_string()
    } else {
        format!("http://{v}")
    };
    *llm_base_cell().write().unwrap() = v;
}
pub fn set_llm_model(name: &str) {
    let v = name.trim();
    *llm_model_cell().write().unwrap() =
        if v.is_empty() { MODEL.to_string() } else { v.to_string() };
}
pub fn llm_base_url() -> String { llm_base_cell().read().unwrap().clone() }
pub fn llm_chat_url() -> String { format!("{}/v1/chat/completions", llm_base_url()) }
pub fn llm_models_url() -> String { format!("{}/v1/models", llm_base_url()) }
pub fn llm_model() -> String { llm_model_cell().read().unwrap().clone() }
pub const VOICE_SERVER_URL: &str = "http://127.0.0.1:8237";

// OpenClaw Gateway (token lives in app_settings 'openclaw_token', not in source)
pub const OPENCLAW_URL: &str = "http://127.0.0.1:18789/v1/chat/completions";

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
    /// Track which time period already got a voiced message today: "morning"/"day"/"evening"
    pub voiced_periods_today: Vec<String>,
    /// Date of last voiced_periods reset (to reset daily)
    pub voiced_periods_date: String,
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
            voiced_periods_today: Vec::new(),
            voiced_periods_date: String::new(),
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

impl Drop for HanniDb {
    fn drop(&mut self) {
        if let Ok(conn) = self.0.lock() {
            let _ = conn.execute_batch("SELECT crsql_finalize();");
        }
    }
}

/// ~/Library/Application Support/Hanni/
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set the app data directory. Must be called once before any DB access.
/// On Android, call with `app.path().app_data_dir()` from .setup().
pub fn set_data_dir(path: PathBuf) {
    let _ = DATA_DIR.set(path);
}

pub fn hanni_data_dir() -> PathBuf {
    if let Some(dir) = DATA_DIR.get() {
        return dir.clone();
    }
    // Fallback for macOS/desktop — dirs crate works there
    #[cfg(not(target_os = "android"))]
    {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join("Library/Application Support")
            })
            .join("Hanni")
    }
    #[cfg(target_os = "android")]
    {
        panic!("set_data_dir() must be called before hanni_data_dir() on Android");
    }
}

pub fn hanni_db_path() -> PathBuf {
    hanni_data_dir().join("hanni.db")
}

pub fn data_file_path() -> PathBuf {
    hanni_data_dir().join("life-tracker-data.json")
}

/// UUIDv7 — time-ordered 128-bit id, lexicographically sortable by
/// creation time. Used as primary key on sync-eligible tables that used
/// to rely on integer auto-increment: two devices generating rows in
/// parallel never collide (vs INTEGER PRIMARY KEY AUTOINCREMENT which
/// each device walks 1, 2, 3 independently), and the timestamp prefix
/// keeps row indices clustered.
pub fn new_uuid_v7() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Fixed namespace for deterministic routine ids — NEVER change it, or every
/// device would recompute different ids and routines would duplicate.
const ROUTINE_NS: uuid::Uuid = uuid::Uuid::from_u128(0xa1f0e2c4_5b6d_4e7f_8a9b_0c1d2e3f4a5b);

/// Deterministic 53-bit positive integer id derived from a stable `key`. Both
/// devices compute the same value for the same key, so routine rows converge
/// across devices instead of colliding the way INTEGER AUTOINCREMENT ids do
/// (each device walks 1,2,3 independently). Routine ids stay integers (the
/// engine + frontend use them as i64/parseInt), so we hash into an int rather
/// than switching to UUID strings. 53 bits keeps the value exact through JSON /
/// JS Number (precise below 2^53).
pub fn deterministic_id(key: &str) -> i64 {
    let u = uuid::Uuid::new_v5(&ROUTINE_NS, key.as_bytes());
    let b = u.as_bytes();
    let raw = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    (raw & 0x1F_FFFF_FFFF_FFFF) as i64 // mask to 53 bits → always positive, JS-safe
}

/// Deterministic routine_runs.id from its natural key (chain_id, date, slot) —
/// both devices agree on the id, so a pulled run UPSERTs onto the same row
/// instead of violating UNIQUE(chain_id,date,slot).
pub fn routine_run_id(chain_id: i64, date: &str, slot: &str) -> i64 {
    deterministic_id(&format!("run:{}:{}:{}", chain_id, date, slot))
}

/// Deterministic routine_node_status.id from its natural key (run_id, node_id).
pub fn routine_node_status_id(run_id: i64, node_id: i64) -> i64 {
    deterministic_id(&format!("nstat:{}:{}", run_id, node_id))
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

// ── Non-streaming response with tool calls (agent loop) ──

#[derive(Deserialize, Debug)]
pub struct AgentMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<AgentToolCall>>,
}

#[derive(Deserialize, Debug)]
pub struct AgentToolCall {
    pub id: String,
    pub function: AgentToolCallFunction,
}

#[derive(Deserialize, Debug)]
pub struct AgentToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Deserialize, Debug)]
pub struct AgentChoice {
    pub message: AgentMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct AgentResponse {
    pub choices: Vec<AgentChoice>,
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
