// voice/recording.rs — Whisper model, audio capture, recording, voice samples
use crate::types::*;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use std::path::PathBuf;
use std::io::Write;

// ── Whisper model paths ──

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

// ── Whisper hallucination detection ──

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

// ── Voice samples ──

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
