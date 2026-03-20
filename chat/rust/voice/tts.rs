// voice/tts.rs — Text-to-speech: Silero TTS, voice cloning, macOS say fallback
use crate::types::*;
use std::sync::atomic::Ordering;

// ── Text cleaning ──

pub fn clean_text_for_tts(text: &str) -> String {
    use std::sync::OnceLock;
    static RE_ACTION: OnceLock<regex::Regex> = OnceLock::new();
    static RE_THINK: OnceLock<regex::Regex> = OnceLock::new();
    static RE_URL: OnceLock<regex::Regex> = OnceLock::new();
    static RE_PARENS: OnceLock<regex::Regex> = OnceLock::new();
    static RE_BRACKETS: OnceLock<regex::Regex> = OnceLock::new();
    let re_action = RE_ACTION.get_or_init(|| regex::Regex::new(r"(?s)```action.*?```").unwrap());
    let re_think = RE_THINK.get_or_init(|| regex::Regex::new(r"(?s)<think>.*?</think>").unwrap());
    let re_url = RE_URL.get_or_init(|| regex::Regex::new(r"https?://\S+").unwrap());
    let re_parens = RE_PARENS.get_or_init(|| regex::Regex::new(r"\([^)]*\)").unwrap());
    let re_brackets = RE_BRACKETS.get_or_init(|| regex::Regex::new(r"\[[^\]]*\]").unwrap());

    let mut s = re_action.replace_all(text, "").to_string();
    s = re_think.replace_all(&s, "").to_string();
    s = re_url.replace_all(&s, "").to_string();
    // Remove markdown formatting
    s = s.replace('"', "'");
    s = s.replace("```", "").replace('`', "").replace("**", "").replace('*', "");
    s = s.replace("###", "").replace("##", "").replace('#', "");
    s = re_parens.replace_all(&s, "").to_string();
    s = re_brackets.replace_all(&s, "").to_string();
    // Remove emojis and misc symbols (Unicode ranges)
    s = s.chars().filter(|c| {
        let cp = *c as u32;
        // Keep basic Latin, Cyrillic, common punctuation, digits
        // Filter out emoji/symbol ranges
        !(
            (0x1F600..=0x1F64F).contains(&cp) || // Emoticons
            (0x1F300..=0x1F5FF).contains(&cp) || // Misc Symbols & Pictographs
            (0x1F680..=0x1F6FF).contains(&cp) || // Transport & Map
            (0x1F700..=0x1F77F).contains(&cp) || // Alchemical
            (0x1F780..=0x1F7FF).contains(&cp) || // Geometric Shapes Extended
            (0x1F800..=0x1F8FF).contains(&cp) || // Supplemental Arrows-C
            (0x1F900..=0x1F9FF).contains(&cp) || // Supplemental Symbols & Pictographs
            (0x1FA00..=0x1FA6F).contains(&cp) || // Chess Symbols
            (0x1FA70..=0x1FAFF).contains(&cp) || // Symbols & Pictographs Extended-A
            (0x2600..=0x26FF).contains(&cp) ||   // Misc symbols
            (0x2700..=0x27BF).contains(&cp) ||   // Dingbats
            (0x231A..=0x231B).contains(&cp) ||   // Watch, Hourglass
            (0x23E9..=0x23F3).contains(&cp) ||   // Media control
            (0x23F8..=0x23FA).contains(&cp) ||   // Media control
            (0x25AA..=0x25AB).contains(&cp) ||   // Squares
            (0x25B6..=0x25C0).contains(&cp) ||   // Triangles
            (0x25FB..=0x25FE).contains(&cp) ||   // Squares
            (0x2934..=0x2935).contains(&cp) ||   // Arrows
            (0x2B05..=0x2B07).contains(&cp) ||   // Arrows
            (0x2B1B..=0x2B1C).contains(&cp) ||   // Squares
            (0x3030..=0x3030).contains(&cp) ||   // Wavy dash
            (0x303D..=0x303D).contains(&cp) ||   // Part alternation mark
            (0xFE0F..=0xFE0F).contains(&cp) ||   // Variation selector
            (0x200D..=0x200D).contains(&cp) ||   // Zero-width joiner
            (0x20E3..=0x20E3).contains(&cp) ||   // Combining enclosing keycap
            (0xE0020..=0xE007F).contains(&cp)    // Tags
        )
    }).collect::<String>();
    // Collapse multiple spaces/newlines
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result.trim().to_string()
}

// ── Silero TTS core ──

/// Try local Silero TTS via voice server (core logic with retry)
pub fn speak_silero_core(text: &str, speaker: &str) -> Result<Vec<u8>, String> {
    let url = format!("{}/tts", VOICE_SERVER_URL);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut last_err = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * (1 << attempt)));
            eprintln!("[TTS] Retry #{}", attempt);
        }
        match client.post(&url)
            .json(&serde_json::json!({"text": text, "speaker": speaker}))
            .send()
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.bytes() {
                    Ok(bytes) => return Ok(bytes.to_vec()),
                    Err(e) => last_err = format!("Read bytes: {}", e),
                }
            }
            Ok(resp) => last_err = format!("Server error: {}", resp.status()),
            Err(e) => last_err = format!("Network: {}", e),
        }
    }
    Err(last_err)
}

/// Play WAV bytes via afplay (secure temp file — auto-cleanup, unique name, 0600 perms)
pub fn play_wav_blocking(bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    IS_SPEAKING.store(true, Ordering::Relaxed);
    let _guard = SpeakingGuard; // drop guard — clears flag on any exit
    let mut tmp = tempfile::Builder::new()
        .prefix("hanni_tts_")
        .suffix(".wav")
        .tempfile()
        .map_err(|e| format!("Temp file: {}", e))?;
    tmp.write_all(bytes).map_err(|e| format!("Write temp: {}", e))?;
    let path = tmp.path().to_string_lossy().to_string();
    let _ = std::process::Command::new("afplay").arg(&path).status();
    // tmp auto-deleted on drop
    Ok(())
}

/// Try local Silero TTS via voice server (non-blocking)
pub fn speak_silero_local(text: &str, speaker: &str) {
    let text_owned = text.to_string();
    let speaker_owned = speaker.to_string();
    std::thread::spawn(move || {
        match speak_silero_core(&text_owned, &speaker_owned) {
            Ok(bytes) => { let _ = play_wav_blocking(&bytes); }
            Err(e) => eprintln!("[TTS] Non-blocking failed: {}", e),
        }
    });
}

/// Try local Silero TTS via voice server (blocking)
pub fn speak_silero_local_sync(text: &str, speaker: &str) -> bool {
    match speak_silero_core(text, speaker) {
        Ok(bytes) => play_wav_blocking(&bytes).is_ok(),
        Err(e) => { eprintln!("[TTS] Sync failed: {}", e); false }
    }
}

/// Map voice name to Silero speaker (default: xenia)
pub fn silero_speaker_for(voice: &str) -> &str {
    match voice {
        // English voices — pass through directly
        v if v.starts_with("en_") => v,
        // Russian voices
        v if v.contains("Dmitry") || v.contains("Male") || v.contains("aidar") => "aidar",
        v if v.contains("eugene") => "eugene",
        v if v.contains("baya") => "baya",
        v if v.contains("kseniya") => "kseniya",
        _ => "xenia",
    }
}

pub fn speak_tts(text: &str, voice: &str) {
    let clean = clean_text_for_tts(text);
    if clean.is_empty() { return; }
    // Local Silero TTS via voice server
    speak_silero_local(&clean, silero_speaker_for(voice));
}

const MAX_TTS_TEXT_LEN: usize = 2000;

/// Synchronous TTS — blocks until audio finishes playing
pub fn speak_tts_sync(text: &str, voice: &str) {
    let truncated = if text.len() > MAX_TTS_TEXT_LEN { &text[..text.floor_char_boundary(MAX_TTS_TEXT_LEN)] } else { text };
    let clean = clean_text_for_tts(truncated);
    if clean.is_empty() { return; }
    // Local Silero TTS via voice server (play_wav_blocking sets IS_SPEAKING)
    if speak_silero_local_sync(&clean, silero_speaker_for(voice)) { return; }
    // Fallback to macOS say — also guard with IS_SPEAKING
    eprintln!("[TTS] Silero local failed, falling back to macOS say");
    IS_SPEAKING.store(true, Ordering::Relaxed);
    let _guard = SpeakingGuard;
    let _ = std::process::Command::new("say")
        .args(["-r", "210", &clean])
        .status();
}

// ── Tauri commands ──

#[tauri::command]
pub async fn speak_text_blocking(text: String, voice: Option<String>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "xenia".into());
    // V3: Split into sentences and speak sequentially for faster first-word latency
    tokio::task::spawn_blocking(move || {
        let clean = clean_text_for_tts(&text);
        if clean.is_empty() { return; }
        let sentences: Vec<&str> = clean.split_inclusive(|c: char| c == '.' || c == '!' || c == '?' || c == '\u{3002}')
            .filter(|s| !s.trim().is_empty())
            .collect();
        if sentences.len() <= 1 {
            speak_tts_sync(&text, &v);
        } else {
            for sentence in sentences {
                let trimmed = sentence.trim();
                if !trimmed.is_empty() {
                    speak_tts_sync(trimmed, &v);
                }
            }
        }
    }).await.map_err(|e| format!("TTS join error: {}", e))?;
    Ok(())
}

/// Speak a single sentence synchronously — for streaming TTS in call mode
#[tauri::command]
pub async fn speak_sentence_blocking(sentence: String, voice: Option<String>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "xenia".into());
    // Truncate long sentences to prevent TTS timeout
    let truncated = if sentence.len() > MAX_TTS_TEXT_LEN {
        sentence[..sentence.floor_char_boundary(MAX_TTS_TEXT_LEN)].to_string()
    } else { sentence };
    tokio::task::spawn_blocking(move || {
        speak_tts_sync(&truncated, &v);
    }).await.map_err(|e| format!("TTS join error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn speak_text(text: String, voice: Option<String>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "xenia".into());
    let truncated = if text.len() > MAX_TTS_TEXT_LEN { &text[..text.floor_char_boundary(MAX_TTS_TEXT_LEN)] } else { &text };
    let clean = clean_text_for_tts(truncated);
    if clean.is_empty() { return Ok(()); }
    let speaker = silero_speaker_for(&v).to_string();
    tokio::task::spawn_blocking(move || {
        speak_silero_local(&clean, &speaker);
    }).await.map_err(|e| format!("TTS join error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn stop_speaking() -> Result<(), String> {
    let _ = std::process::Command::new("killall").arg("say").output();
    let _ = std::process::Command::new("killall").arg("afplay").output();
    IS_SPEAKING.store(false, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn get_tts_voices() -> Result<serde_json::Value, String> {
    let mut voices: Vec<serde_json::Value> = Vec::new();
    // Russian voices (Silero v5 — best quality)
    for (name, gender) in &[
        ("xenia", "Female"), ("kseniya", "Female"), ("baya", "Female"),
        ("aidar", "Male"), ("eugene", "Male"),
    ] {
        voices.push(serde_json::json!({
            "name": name, "gender": gender, "lang": "ru-RU", "engine": "silero_v5"
        }));
    }
    // English voices (Silero v3 — local, open-source)
    for (name, gender) in &[
        ("en_0", "Female"), ("en_21", "Female"), ("en_45", "Female"),
        ("en_56", "Female"), ("en_99", "Female"),
        ("en_1", "Male"), ("en_7", "Male"), ("en_30", "Male"),
        ("en_72", "Male"), ("en_100", "Male"),
    ] {
        voices.push(serde_json::json!({
            "name": name, "gender": gender, "lang": "en-US", "engine": "silero_v3"
        }));
    }
    Ok(serde_json::json!(voices))
}

// ── Voice cloning ──

#[tauri::command]
pub async fn speak_clone_blocking(text: String, sample_name: String) -> Result<(), String> {
    let samples_dir = hanni_data_dir().join("voice_samples");
    let sample_path = samples_dir.join(format!("{}.wav", sample_name));
    if !sample_path.exists() {
        return Err(format!("Voice sample '{}' not found", sample_name));
    }
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // Get PC TTS server URL from settings
        let server_url = {
            let data_dir = hanni_data_dir();
            let db_path = data_dir.join("hanni.db");
            if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                conn.query_row(
                    "SELECT value FROM app_settings WHERE key='tts_server_url'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_default()
            } else {
                String::new()
            }
        };
        if server_url.is_empty() {
            return Err("TTS clone server URL not configured".into());
        }

        let clean = clean_text_for_tts(&text);
        if clean.is_empty() {
            return Ok(());
        }

        // Send file path to voice_server — it reads + base64-encodes locally
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("HTTP: {}", e))?;
        let resp = client
            .post(format!("{}/tts/clone", VOICE_SERVER_URL))
            .json(&serde_json::json!({
                "text": clean,
                "server_url": server_url,
                "reference_audio_path": sample_path.to_string_lossy(),
            }))
            .send()
            .map_err(|e| format!("Clone TTS error: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("Clone TTS server error: {}", resp.status()));
        }
        let bytes = resp.bytes().map_err(|e| format!("Read: {}", e))?;
        play_wav_blocking(&bytes)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
