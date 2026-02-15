# Voice — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Управление моделью Whisper | endpoint | download_whisper_model, check_whisper_model | lib.rs:L1104-1197 | 94 | Medium | 6 | ✅ |
| B2 | Запись и транскрипция | endpoint | start/stop_recording, transcribe_samples | lib.rs:L1154-1233 | 80 | Medium | 5 | ✅ |
| B3 | Захват аудио (CPAL) | internal | start_audio_capture() | lib.rs:L1234-1371 | 138 | Complex | 6 | ✅ |
| B4 | Call Mode управление | endpoint | start/stop_call_mode, resume_listening, set_speaking, check_bargein | lib.rs:L1371-1487 | 117 | Medium | 6 | ✅ |
| B5 | Call Mode аудио-луп (VAD) | scheduled | start_call_audio_loop() | lib.rs:L1488-1803 | 316 | Complex | 10 | ✅ |
| B6 | TTS подготовка и очистка текста | internal | adaptive_tts_rate, clean_text_for_tts | lib.rs:L6906-7056 | 151 | Medium | 6 | ✅ |
| B7 | TTS синтез (edge-tts + remote) | internal | speak_edge_tts, speak_remote_tts, speak_tts | lib.rs:L6920-7169 | 250 | Complex | 9 | ✅ |
| B8 | TTS Tauri-команды | endpoint | speak_text, stop_speaking, get_tts_voices | lib.rs:L7170-7357 | 188 | Medium | 6 | ✅ |

## Frontend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| F1 | Кнопка записи голоса | handler | recordBtn listener | main.js:L204-257 | 54 | Medium | 4 | ✅ |
| F2 | UI режима звонка | handler | startCallMode, endCallMode, speakAndListen | main.js:L5059-5376 | 318 | Complex | 10 | ✅ |

## Integration

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| I1 | Remote TTS сервер (PC) | external | pc/tts_server.py | tts_server.py:L1-120 | 120 | Medium | 5 | ✅ |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 11 |
| Simple | 0 |
| Medium | 7 |
| Complex | 4 |
| Total subfunctions | 73 |
| Phase 3 complete | 11/11 |
