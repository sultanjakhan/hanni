// voice/call_mode.rs — Call mode: VAD, barge-in, voice notes
use crate::types::*;
use tauri::{AppHandle, Emitter};
use std::sync::Arc;

use super::recording::{init_audio_device, downmix_resample_into, transcribe_samples};

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

// ── Call audio loop ──

fn start_call_audio_loop(call_state: Arc<CallMode>, app: AppHandle) {
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
