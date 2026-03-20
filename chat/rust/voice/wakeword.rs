// voice/wakeword.rs — Wake word detection via voice server
use crate::types::*;

#[tauri::command]
pub async fn start_wakeword(keyword: Option<String>) -> Result<String, String> {
    let kw = keyword.unwrap_or_else(|| "ханни".into());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP error: {}", e))?;
    let resp = client
        .post(format!("{}/wakeword/start", VOICE_SERVER_URL))
        .json(&serde_json::json!({"keyword": kw}))
        .send()
        .await
        .map_err(|e| format!("Voice server error: {}", e))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;
    Ok(body.to_string())
}

#[tauri::command]
pub async fn stop_wakeword() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP error: {}", e))?;
    let _ = client
        .post(format!("{}/wakeword/stop", VOICE_SERVER_URL))
        .send()
        .await
        .map_err(|e| format!("Voice server error: {}", e))?;
    Ok("stopped".into())
}
