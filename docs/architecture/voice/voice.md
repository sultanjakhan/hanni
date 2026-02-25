# Voice Module

Whisper STT, захват аудио, запись, Call Mode, VAD, TTS (Silero), Voice cloning.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~1650 |
| Backend functions | 8 |
| Frontend functions | 2 |
| Integration functions | 1 |
| Complexity | Simple: 0, Medium: 7, Complex: 4 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L1094-1803 | Whisper, запись, CPAL, Call Mode, VAD |
| Backend | `lib.rs` | L9549-9625 | TTS: speak_text, speak_blocking, speak_sentence |
| Frontend | `main.js` | L204-257 | Кнопка записи голоса |
| Frontend | `main.js` | L5900-6610 | UI режима звонка, wake word, call mode |
| External | `voice_server.py` | L1-700+ | Voice server (VAD, Whisper, Silero TTS, embeddings, rerank) |
| External | `pc/tts_server.py` | L1-120 | Remote TTS сервер (PC, Qwen3-TTS) |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core, chat (for call mode LLM) |
| **Used by** | chat (TTS playback), proactive (voice messages) |

## Key Improvements (v0.18.6)

### Stability Fixes
- **speak_text() await** — was fire-and-forget (spawn without await), now properly awaits TTS completion
- **speak_sentence_blocking truncation** — long sentences now truncated to MAX_TTS_TEXT_LEN before TTS
- **wakeword/start JSON validation** — added try-except for json.loads (was unprotected)
- **Mic disconnect detection** — emits `call-error` event to UI after 5s of no audio data

### Architecture
- Voice server at :8237 handles: VAD (Silero ONNX), STT (MLX Whisper), TTS (Silero v5), embeddings (fastembed), rerank (flashrank)
- Call mode: SSE stream `/listen` with continuous VAD + transcription loop
- Wake word: Whisper-based keyword detection via `/wakeword/start`
- TTS pipeline: text → clean → truncate → Silero → WAV → afplay

## Improvements Status

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| V1 | Multi-language STT (auto-detect / настройки) | S | ✅ |
| V2 | Wake word (openWakeWord / Porcupine) | L | ✅ |
| V3 | Sentence-level streaming TTS | M | ✅ |
| V4 | Voice activity indicator (waveform) | S | ⬜ |
| V5 | Adaptive speech rate | S | ⬜ |
| V6 | TTS completion event (убрать setTimeout хак) | M | ✅ |
| V7 | Noise cancellation (rnnoise) | M | ⬜ |
| V8 | Voice cloning (Qwen3-TTS) | L | ✅ |
