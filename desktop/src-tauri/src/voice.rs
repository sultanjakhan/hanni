// voice.rs — Whisper, recording, call mode, wake word, voice cloning, TTS
use crate::types::*;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::path::PathBuf;
use std::io::Write;

pub fn whisper_model_path() -> PathBuf {
    let turbo = hanni_data_dir().join("models/ggml-large-v3-turbo.bin");
    if turbo.exists() { return turbo; }
    // Fallback to medium if turbo not yet downloaded
    let medium = hanni_data_dir().join("models/ggml-medium.bin");
    if medium.exists() { return medium; }
    // Default to turbo for new downloads
    turbo
}

pub fn whisper_turbo_path() -> PathBuf {
    hanni_data_dir().join("models/ggml-large-v3-turbo.bin")
}

#[tauri::command]
pub async fn download_whisper_model(app: AppHandle) -> Result<String, String> {
    let model_path = whisper_turbo_path();
    if model_path.exists() {
        return Ok("Model already downloaded".into());
    }

    if let Some(parent) = model_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
    }

    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin";
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
pub fn start_recording(state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    let needs_capture = {
        let mut ws = state.0.lock().unwrap_or_else(|e| e.into_inner());
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
pub async fn stop_recording(state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    let samples = {
        let mut ws = state.0.lock().unwrap_or_else(|e| e.into_inner());
        ws.recording = false;
        state.1.notify_all(); // wake capture thread immediately
        if ws.audio_buffer.is_empty() {
            return Err("No audio recorded".into());
        }
        let s = std::mem::take(&mut ws.audio_buffer);
        s
    };

    let model_path = whisper_model_path();
    if !model_path.exists() {
        return Err("Whisper model not downloaded. Please download it first.".into());
    }

    // Run transcription off main thread so UI stays responsive
    tokio::task::spawn_blocking(move || transcribe_samples(&samples))
        .await
        .map_err(|e| format!("Transcription join error: {}", e))?
}

#[tauri::command]
pub fn check_whisper_model() -> Result<bool, String> {
    Ok(whisper_model_path().exists())
}

/// Known Whisper hallucination phrases (from faster-whisper, HuggingFace dataset, Russian gist)
const WHISPER_HALLUCINATIONS: &[&str] = &[
    // Russian hallucinations
    "спасибо за внимание", "спасибо за просмотр", "продолжение следует",
    "субтитры сделал", "субтитры подогнал", "редактор субтитров",
    "подписывайтесь на мой канал", "подписывайтесь на канал",
    "ставьте лайки", "не забудьте подписаться",
    "веселая музыка", "спокойная музыка", "грустная мелодия",
    "динамичная музыка", "торжественная музыка", "тревожная музыка",
    "музыкальная заставка", "аплодисменты", "смех",
    "перестрелка", "гудок поезда", "рёв мотора", "шум двигателя",
    "лай собак", "выстрелы", "стук в дверь",
    // English hallucinations
    "thank you for watching", "thanks for watching", "thank you",
    "please subscribe", "subtitles by the amara",
    "transcription by castingwords", "the end", "bye bye",
    "satsang with mooji", "bbc radio",
];

pub fn is_whisper_hallucination(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() || normalized.len() < 2 { return true; }
    // Exact match only — prevents false positives on legit phrases containing hallucination substrings
    for h in WHISPER_HALLUCINATIONS {
        if normalized == *h { return true; }
    }
    // Detect repetitive text (compression ratio > 4.0 = likely looping hallucination)
    if normalized.len() > 20 {
        let unique_chars: std::collections::HashSet<char> = normalized.chars().collect();
        let ratio = normalized.len() as f32 / unique_chars.len().max(1) as f32;
        if ratio > 4.0 { return true; }
    }
    false
}

pub fn transcribe_samples(samples: &[f32]) -> Result<String, String> {
    // Skip very short audio (< 0.3s at 16kHz = likely noise)
    if samples.len() < 4800 {
        return Ok(String::new());
    }

    let model_path = whisper_model_path();
    if !model_path.exists() {
        return Err("Whisper model not downloaded".into());
    }
    let ctx = whisper_rs::WhisperContext::new_with_params(
        model_path.to_str().unwrap_or(""),
        whisper_rs::WhisperContextParameters::default(),
    ).map_err(|e| format!("Whisper init error: {}", e))?;

    let mut state = ctx.create_state().map_err(|e| format!("Whisper state error: {}", e))?;

    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::BeamSearch { beam_size: 5, patience: 1.0 });
    params.set_language(None); // auto-detect language
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_no_speech_thold(0.6);
    params.set_suppress_blank(true);
    params.set_temperature(0.0);  // deterministic, no random sampling
    params.set_n_threads(8);  // M3 Pro has plenty of cores

    state.full(params, samples).map_err(|e| format!("Transcription error: {}", e))?;

    let num_segments = state.full_n_segments().map_err(|e| format!("Segment error: {}", e))?;
    let mut text = String::new();
    for i in 0..num_segments {
        if let Ok(segment) = state.full_get_segment_text(i) {
            text.push_str(&segment);
        }
    }
    let result = text.trim().to_string();
    // Filter hallucinations
    if is_whisper_hallucination(&result) {
        return Ok(String::new());
    }
    Ok(result)
}

// ── Audio capture via cpal ──

/// Initialize audio input device: try 16kHz mono, fallback to device default with resampling
pub fn init_audio_device() -> Result<(cpal::Device, cpal::StreamConfig, f64, usize), String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let device = host.default_input_device()
        .ok_or_else(|| "no input device found".to_string())?;

    let target = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(16000),
        buffer_size: cpal::BufferSize::Default,
    };
    match device.build_input_stream(&target, |_: &[f32], _: &cpal::InputCallbackInfo| {}, |_| {}, None) {
        Ok(_) => Ok((device, target, 1.0, 1)),
        Err(_) => {
            let supported = device.default_input_config()
                .map_err(|e| format!("no supported config: {}", e))?;
            let rate = supported.sample_rate().0;
            let ch = supported.channels();
            eprintln!("Audio: using device config {}Hz {}ch (resampling to 16kHz)", rate, ch);
            let cfg = cpal::StreamConfig {
                channels: ch,
                sample_rate: cpal::SampleRate(rate),
                buffer_size: cpal::BufferSize::Default,
            };
            Ok((device, cfg, rate as f64 / 16000.0, ch as usize))
        }
    }
}

/// Downmix multi-channel audio to mono and resample to 16kHz into target buffer
pub fn downmix_resample_into(data: &[f32], channels: usize, ratio: f64, buf: &mut Vec<f32>) {
    if channels == 1 && ratio == 1.0 {
        buf.extend_from_slice(data);
        return;
    }
    if channels == 1 {
        // Mono, just resample (skip intermediate Vec)
        let mut pos = 0.0_f64;
        while (pos as usize) < data.len() {
            buf.push(data[pos as usize]);
            pos += ratio;
        }
    } else if ratio <= 1.0 {
        // Multi-channel, no resampling needed — downmix directly into buf
        for ch in data.chunks(channels) {
            buf.push(ch.iter().sum::<f32>() / channels as f32);
        }
    } else {
        // Multi-channel + resampling — downmix + resample in one pass
        let mono_len = data.len() / channels;
        let mut pos = 0.0_f64;
        while (pos as usize) < mono_len {
            let i = pos as usize * channels;
            let sample: f32 = data[i..i + channels].iter().sum::<f32>() / channels as f32;
            buf.push(sample);
            pos += ratio;
        }
    }
}

pub fn start_audio_capture(recording_state: Arc<AudioRecording>) {
    std::thread::spawn(move || {
        use cpal::traits::{DeviceTrait, StreamTrait};

        let (device, config, ratio, channels) = match init_audio_device() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Voice: {}", e);
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                ws.capture_running = false;
                ws.recording = false;
                return;
            }
        };

        let state_clone = recording_state.clone();
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut ws = state_clone.0.lock().unwrap_or_else(|e| e.into_inner());
                if ws.recording {
                    downmix_resample_into(data, channels, ratio, &mut ws.audio_buffer);
                }
            },
            |err| eprintln!("Audio capture error: {}", err),
            None,
        );

        match stream {
            Ok(stream) => {
                if let Err(e) = stream.play() {
                    eprintln!("Voice: stream play error: {}", e);
                    {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        ws.capture_running = false;
                        ws.recording = false;
                    }
                    return;
                }
                // Wait for stop signal via condvar instead of polling
                {
                    let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                    while ws.recording {
                        ws = recording_state.1.wait(ws).unwrap_or_else(|e| e.into_inner());
                    }
                }
                {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                    ws.capture_running = false;
                }
            }
            Err(e) => {
                eprintln!("Voice: build stream error: {} — check microphone permissions", e);
                {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                    ws.capture_running = false;
                    ws.recording = false;
                }
            }
        }
    });
}

// ── Call Mode ──

#[tauri::command]
pub fn start_call_mode(
    call_state: tauri::State<'_, Arc<CallMode>>,
    app: AppHandle,
) -> Result<String, String> {
    {
        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if cs.active {
            return Ok("Already in call mode".into());
        }
        cs.active = true;
        cs.phase = "listening".into();
        cs.audio_buffer.clear();
        cs.speech_frames = 0;
        cs.silence_frames = 0;
        cs.barge_in = false;
    }
    let _ = app.emit("call-phase-changed", "listening");
    let call_state_arc = call_state.inner().clone();
    start_call_audio_loop(call_state_arc, app);
    Ok("Call mode started".into())
}

#[tauri::command]
pub fn stop_call_mode(
    call_state: tauri::State<'_, Arc<CallMode>>,
    app: AppHandle,
) -> Result<String, String> {
    let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    cs.active = false;
    cs.phase = "idle".into();
    cs.audio_buffer.clear();
    cs.speech_frames = 0;
    cs.silence_frames = 0;
    cs.barge_in = false;
    let _ = app.emit("call-phase-changed", "idle");
    // Kill any playing TTS
    let _ = std::process::Command::new("killall").arg("afplay").output();
    Ok("Call mode stopped".into())
}

#[tauri::command]
pub fn call_mode_resume_listening(
    call_state: tauri::State<'_, Arc<CallMode>>,
    app: AppHandle,
) -> Result<(), String> {
    let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    if !cs.active { return Ok(()); }
    cs.phase = "listening".into();
    cs.audio_buffer.clear();
    cs.speech_frames = 0;
    cs.silence_frames = 0;
    cs.barge_in = false;
    let _ = app.emit("call-phase-changed", "listening");
    Ok(())
}

#[tauri::command]
pub fn call_mode_set_speaking(
    call_state: tauri::State<'_, Arc<CallMode>>,
) -> Result<(), String> {
    let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    if !cs.active { return Ok(()); }
    cs.phase = "speaking".into();
    cs.speech_frames = 0;
    cs.barge_in = false;
    Ok(())
}

#[tauri::command]
pub fn call_mode_check_bargein(
    call_state: tauri::State<'_, Arc<CallMode>>,
) -> Result<bool, String> {
    let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    Ok(cs.barge_in)
}

#[tauri::command]
pub fn save_voice_note(
    call_state: tauri::State<'_, Arc<CallMode>>,
    title: String,
) -> Result<String, String> {
    let samples = {
        let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if cs.last_recording.is_empty() {
            return Err("No recording available".into());
        }
        cs.last_recording.clone()
    };

    let app_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Hanni")
        .join("voice_notes");
    std::fs::create_dir_all(&app_dir).map_err(|e| format!("Dir error: {}", e))?;

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("{}_{}.wav", ts, title.chars().take(30).collect::<String>());
    let filepath = app_dir.join(&filename);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&filepath, spec)
        .map_err(|e| format!("WAV write error: {}", e))?;
    for &s in &samples {
        let val = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(val).map_err(|e| format!("Sample write error: {}", e))?;
    }
    writer.finalize().map_err(|e| format!("Finalize error: {}", e))?;

    Ok(filepath.to_string_lossy().to_string())
}

// ── v0.18.0 Wave 3: Wake Word (V2) ──

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

// ── v0.18.0 Wave 3: Voice Cloning (V8) ──

#[tauri::command]
pub fn save_voice_sample(
    call_state: tauri::State<'_, Arc<CallMode>>,
    name: String,
) -> Result<String, String> {
    let samples = {
        let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if cs.last_recording.is_empty() {
            return Err("No recording available".into());
        }
        cs.last_recording.clone()
    };
    let safe_name: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .take(50)
        .collect();
    if safe_name.trim().is_empty() {
        return Err("Invalid sample name".into());
    }
    let samples_dir = hanni_data_dir().join("voice_samples");
    std::fs::create_dir_all(&samples_dir).map_err(|e| format!("Dir error: {}", e))?;

    let filepath = samples_dir.join(format!("{}.wav", safe_name.trim()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(&filepath, spec).map_err(|e| format!("WAV error: {}", e))?;
    for &s in &samples {
        let val = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer
            .write_sample(val)
            .map_err(|e| format!("Write error: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Finalize error: {}", e))?;
    Ok(filepath.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn record_voice_sample(name: String, duration_secs: Option<u64>) -> Result<String, String> {
    let dur = duration_secs.unwrap_or(5);
    let safe_name: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .take(50)
        .collect();
    if safe_name.trim().is_empty() {
        return Err("Invalid sample name".into());
    }
    let samples_dir = hanni_data_dir().join("voice_samples");
    std::fs::create_dir_all(&samples_dir).map_err(|e| format!("Dir error: {}", e))?;
    let filepath = samples_dir.join(format!("{}.wav", safe_name.trim()));

    tokio::task::spawn_blocking(move || -> Result<String, String> {
        use cpal::traits::{DeviceTrait, StreamTrait};

        let (device, config, ratio, channels) = init_audio_device()?;
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<f32>::new()));
        let buf_ref = buf.clone();
        let ch = channels;
        let r = ratio;

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut b = buf_ref.lock().unwrap_or_else(|e| e.into_inner());
                    downmix_resample_into(data, ch, r, &mut b);
                },
                |e| eprintln!("Audio error: {}", e),
                None,
            )
            .map_err(|e| format!("Stream error: {}", e))?;

        stream.play().map_err(|e| format!("Play error: {}", e))?;
        std::thread::sleep(std::time::Duration::from_secs(dur));
        drop(stream);

        let samples = buf.lock().unwrap_or_else(|e| e.into_inner());
        if samples.is_empty() {
            return Err("No audio captured".into());
        }

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::create(&filepath, spec).map_err(|e| format!("WAV: {}", e))?;
        for &s in samples.iter() {
            let val = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(val).map_err(|e| format!("Write: {}", e))?;
        }
        writer.finalize().map_err(|e| format!("Finalize: {}", e))?;
        Ok(filepath.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Task: {}", e))?
}

#[tauri::command]
pub fn list_voice_samples() -> Result<Vec<serde_json::Value>, String> {
    let samples_dir = hanni_data_dir().join("voice_samples");
    if !samples_dir.exists() {
        return Ok(vec![]);
    }
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&samples_dir).map_err(|e| format!("Read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Entry: {}", e))?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "wav") {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            items.push(serde_json::json!({"name": name, "path": path.to_string_lossy(), "size": size}));
        }
    }
    Ok(items)
}

#[tauri::command]
pub fn delete_voice_sample(name: String) -> Result<(), String> {
    let safe: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .collect();
    let path = hanni_data_dir()
        .join("voice_samples")
        .join(format!("{}.wav", safe.trim()));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Delete error: {}", e))?;
    }
    Ok(())
}

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

pub fn start_call_audio_loop(call_state: Arc<CallMode>, app: AppHandle) {
    std::thread::spawn(move || {
        use cpal::traits::{StreamTrait, DeviceTrait};

        let (device, config, ratio, channels) = match init_audio_device() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Call mode: {}", e);
                let _ = app.emit("call-phase-changed", "idle");
                let _ = app.emit("call-error", format!("Ошибка микрофона: {}", e));
                return;
            }
        };

        // Shared ring buffer for raw audio chunks (already 16kHz mono after resampling)
        let chunk_buf: Arc<std::sync::Mutex<Vec<f32>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let chunk_buf_writer = chunk_buf.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = chunk_buf_writer.lock() {
                    downmix_resample_into(data, channels, ratio, &mut buf);
                }
            },
            |err| eprintln!("Call audio error: {}", err),
            None,
        );

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Call mode stream error: {} — check microphone permissions", e);
                let _ = app.emit("call-phase-changed", "idle");
                let _ = app.emit("call-error", format!("Нет доступа к микрофону: {}", e));
                return;
            }
        };
        if let Err(e) = stream.play() {
            eprintln!("Call: stream play error: {}", e);
            let _ = app.emit("call-phase-changed", "idle");
            let _ = app.emit("call-error", format!("Не удалось запустить аудио: {}", e));
            return;
        }

        // Initialize VAD (try Silero, fallback to energy-based)
        let mut vad_opt: Option<voice_activity_detector::VoiceActivityDetector> = None;
        match voice_activity_detector::VoiceActivityDetector::builder()
            .sample_rate(16000)
            .chunk_size(512usize)
            .build() {
            Ok(v) => {
                vad_opt = Some(v);
            }
            Err(e) => {
                eprintln!("VAD init error: {} — using energy-based detection", e);
            }
        };

        // Process loop
        let mut process_buf: Vec<f32> = Vec::new();
        // Adaptive noise floor tracking
        let mut noise_floor: f32 = 0.003;
        let noise_alpha: f32 = 0.01; // Slow adaptation
        let mut last_audio_time = std::time::Instant::now();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(16));

            // Check if call mode still active
            {
                let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                if !cs.active { break; }
            }

            // Drain audio from ring buffer (with high-water mark to prevent unbounded growth)
            {
                let mut buf = match chunk_buf.lock() {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                if buf.len() > 32000 {
                    // ~2s at 16kHz — too far behind, drop oldest half to recover
                    let half = buf.len() / 2;
                    eprintln!("Call: audio buffer overrun ({} samples), dropping {}", buf.len(), half);
                    buf.drain(..half);
                }
                if !buf.is_empty() {
                    last_audio_time = std::time::Instant::now();
                    process_buf.extend(buf.drain(..));
                }
            }

            // Detect mic disconnect: no audio data for 5 seconds
            if last_audio_time.elapsed() > std::time::Duration::from_secs(5) {
                eprintln!("Call mode: no audio for 5s — mic likely disconnected");
                let _ = app.emit("call-error", "Микрофон отключён");
                let _ = app.emit("call-phase-changed", "idle");
                let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                cs.active = false;
                break;
            }

            // Process in 512-sample chunks
            while process_buf.len() >= 512 {
                let chunk: Vec<f32> = process_buf.drain(..512).collect();

                // Compute RMS energy
                let energy: f32 = chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32;
                let rms = energy.sqrt();

                // Read phase once per chunk (minimize lock scope)
                let current_phase = call_state.0.lock().unwrap_or_else(|e| e.into_inner()).phase.clone();

                // Adaptive noise floor: update during silence
                if current_phase == "listening" && rms < noise_floor * 3.0 {
                    noise_floor = noise_floor * (1.0 - noise_alpha) + rms * noise_alpha;
                    noise_floor = noise_floor.max(0.001); // Minimum floor
                }
                let noise_gate = (noise_floor * 2.0).max(0.003); // Gate at 2x noise floor

                // Emit audio level for waveform visualization (throttled: every 3rd chunk)
                {
                    static LEVEL_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    let count = LEVEL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count % 3 == 0 {
                        let level = ((rms / 0.15).min(1.0) * 100.0) as u32;
                        let _ = app.emit("call-audio-level", level);
                    }
                }

                if rms < noise_gate {
                    // Below noise floor — treat as definite silence
                    match current_phase.as_str() {
                        "listening" => {
                            let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                            cs.speech_frames = 0;
                            cs.audio_buffer.clear();
                        }
                        "recording" => {
                            // Count silence + check threshold in a single lock
                            let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                            cs.silence_frames += 1;
                            if cs.silence_frames >= 15 {
                                cs.phase = "processing".into();
                                cs.transcription_gen += 1;
                                let gen = cs.transcription_gen;
                                let samples = std::mem::take(&mut cs.audio_buffer);
                                cs.speech_frames = 0;
                                cs.silence_frames = 0;
                                drop(cs);
                                let _ = app.emit("call-phase-changed", "processing");
                                let call_state2 = call_state.clone();
                                let app2 = app.clone();
                                std::thread::spawn(move || {
                                    match transcribe_samples(&samples) {
                                        Ok(text) => {
                                            let trimmed = text.trim().to_string();
                                            let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                            if cs2.transcription_gen != gen { return; } // stale
                                            if !trimmed.is_empty() {
                                                cs2.last_recording = samples;
                                                drop(cs2);
                                                let _ = app2.emit("call-transcript", trimmed);
                                            } else {
                                                cs2.phase = "listening".into();
                                                drop(cs2);
                                                let _ = app2.emit("call-not-heard", "empty");
                                                let _ = app2.emit("call-phase-changed", "listening");
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Call transcription error: {}", e);
                                            let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                            if cs2.transcription_gen != gen { return; }
                                            cs2.phase = "listening".into();
                                            drop(cs2);
                                            let _ = app2.emit("call-not-heard", format!("error: {}", e));
                                            let _ = app2.emit("call-phase-changed", "listening");
                                        }
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                let prob = if let Some(ref mut vad) = vad_opt {
                    vad.predict(chunk.iter().copied())
                } else {
                    (rms * 50.0).min(1.0)
                };

                match current_phase.as_str() {
                    "listening" => {
                        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        if prob > 0.5 {
                            cs.speech_frames += 1;
                            cs.audio_buffer.extend_from_slice(&chunk);
                            if cs.speech_frames >= 5 {
                                // Confirmed speech — transition to recording
                                cs.phase = "recording".into();
                                cs.silence_frames = 0;
                                let _ = app.emit("call-phase-changed", "recording");
                            }
                        } else {
                            cs.speech_frames = 0;
                            cs.audio_buffer.clear();
                        }
                    }
                    "recording" => {
                        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        cs.audio_buffer.extend_from_slice(&chunk);

                        if prob < 0.5 {
                            cs.silence_frames += 1;
                            if cs.silence_frames >= 15 {
                                // ~640ms silence — done recording (faster turn-taking)
                                cs.phase = "processing".into();
                                cs.transcription_gen += 1;
                                let gen = cs.transcription_gen;
                                let samples = std::mem::take(&mut cs.audio_buffer);
                                cs.speech_frames = 0;
                                cs.silence_frames = 0;
                                let _ = app.emit("call-phase-changed", "processing");
                                drop(cs);

                                // Transcribe on a separate thread to avoid blocking audio loop
                                let call_state2 = call_state.clone();
                                let app2 = app.clone();
                                std::thread::spawn(move || {
                                    match transcribe_samples(&samples) {
                                        Ok(text) => {
                                            let trimmed = text.trim().to_string();
                                            let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                            if cs2.transcription_gen != gen { return; } // stale
                                            if !trimmed.is_empty() {
                                                cs2.last_recording = samples;
                                                drop(cs2);
                                                let _ = app2.emit("call-transcript", trimmed);
                                            } else {
                                                cs2.phase = "listening".into();
                                                drop(cs2);
                                                let _ = app2.emit("call-phase-changed", "listening");
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Call transcription error: {}", e);
                                            let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                            if cs2.transcription_gen != gen { return; }
                                            cs2.phase = "listening".into();
                                            drop(cs2);
                                            let _ = app2.emit("call-phase-changed", "listening");
                                        }
                                    }
                                });
                            }
                        } else {
                            cs.silence_frames = 0;
                        }
                    }
                    "speaking" => {
                        // Barge-in detection — must be loud enough to not be speaker echo
                        // Speaker echo typically has RMS 0.01-0.04 (from built-in speakers)
                        // Direct speech into mic is typically 0.06+
                        // Higher threshold prevents false barge-in from TTS audio leaking into mic
                        let barge_rms_thresh = (noise_floor * 15.0).max(0.06);
                        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        if prob > 0.85 && rms > barge_rms_thresh {
                            cs.speech_frames += 1;
                            if cs.speech_frames >= 8 {
                                // 8 frames * 32ms = ~256ms of loud confirmed speech
                                cs.barge_in = true;
                                let _ = app.emit("call-barge-in", true);
                            }
                        } else {
                            // Reset only if clearly not speech; don't reset on borderline
                            if prob < 0.3 {
                                cs.speech_frames = 0;
                            }
                        }
                    }
                    _ => {} // processing, idle — no-op
                }
            }
        }

        drop(stream);
    });
}


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
            (0x2600..=0x26FF).contains(&cp) ||   // Misc symbols (☀☁☂ etc)
            (0x2700..=0x27BF).contains(&cp) ||   // Dingbats (✂✈✉ etc)
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

#[tauri::command]
pub async fn speak_text_blocking(text: String, voice: Option<String>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "xenia".into());
    // V3: Split into sentences and speak sequentially for faster first-word latency
    tokio::task::spawn_blocking(move || {
        let clean = clean_text_for_tts(&text);
        if clean.is_empty() { return; }
        let sentences: Vec<&str> = clean.split_inclusive(|c: char| c == '.' || c == '!' || c == '?' || c == '。')
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
