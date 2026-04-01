// voice_stubs.rs — Stub implementations for Android (no whisper/cpal/hound)
use crate::types::*;
use std::sync::Arc;
use tauri::AppHandle;

const NOT_AVAILABLE: &str = "Voice features not available on this platform";

// Internal functions used by other modules
pub fn speak_silero_core(_text: &str, _speaker: &str) -> Result<Vec<u8>, String> {
    Err(NOT_AVAILABLE.into())
}
pub fn speak_tts(_text: &str, _voice: &str) {}
pub fn speak_tts_sync(_text: &str, _voice: &str) {}

// Tauri commands — TTS
#[tauri::command]
pub async fn speak_text(_text: String, _voice: Option<String>) -> Result<(), String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub async fn stop_speaking() -> Result<(), String> { Ok(()) }
#[tauri::command]
pub async fn get_tts_voices() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!([]))
}

// Tauri commands — Whisper
#[tauri::command]
pub async fn download_whisper_model(_app: AppHandle) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub fn start_recording(_state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub async fn stop_recording(
    _state: tauri::State<'_, Arc<AudioRecording>>,
) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub fn check_whisper_model() -> Result<bool, String> { Ok(false) }

// Tauri commands — Call Mode
#[tauri::command]
pub fn start_call_mode(
    _call_state: tauri::State<'_, Arc<CallMode>>,
    _app: AppHandle,
) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub fn stop_call_mode(
    _call_state: tauri::State<'_, Arc<CallMode>>,
    _app: AppHandle,
) -> Result<String, String> {
    Ok("Call mode not available".into())
}
#[tauri::command]
pub fn call_mode_resume_listening(
    _call_state: tauri::State<'_, Arc<CallMode>>,
    _app: AppHandle,
) -> Result<(), String> {
    Ok(())
}
#[tauri::command]
pub fn call_mode_set_speaking(
    _call_state: tauri::State<'_, Arc<CallMode>>,
) -> Result<(), String> {
    Ok(())
}
#[tauri::command]
pub fn call_mode_check_bargein(
    _call_state: tauri::State<'_, Arc<CallMode>>,
) -> Result<bool, String> {
    Ok(false)
}
#[tauri::command]
pub fn save_voice_note(
    _call_state: tauri::State<'_, Arc<CallMode>>,
    _title: String,
) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}

// Tauri commands — Wake Word
#[tauri::command]
pub async fn start_wakeword(_keyword: Option<String>) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub async fn stop_wakeword() -> Result<String, String> {
    Ok("Wake word not available".into())
}

// Tauri commands — Voice Cloning
#[tauri::command]
pub fn save_voice_sample(
    _call_state: tauri::State<'_, Arc<CallMode>>,
    _name: String,
) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub async fn record_voice_sample(
    _name: String,
    _duration_secs: Option<u64>,
) -> Result<String, String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub fn list_voice_samples() -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}
#[tauri::command]
pub fn delete_voice_sample(_name: String) -> Result<(), String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub async fn speak_clone_blocking(
    _text: String,
    _sample_name: String,
) -> Result<(), String> {
    Err(NOT_AVAILABLE.into())
}

// Tauri commands — TTS blocking
#[tauri::command]
pub async fn speak_text_blocking(
    _text: String,
    _voice: Option<String>,
) -> Result<(), String> {
    Err(NOT_AVAILABLE.into())
}
#[tauri::command]
pub async fn speak_sentence_blocking(
    _sentence: String,
    _voice: Option<String>,
) -> Result<(), String> {
    Err(NOT_AVAILABLE.into())
}
