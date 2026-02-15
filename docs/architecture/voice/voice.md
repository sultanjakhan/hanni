# Voice Module

Whisper STT, захват аудио, запись, Call Mode, VAD, TTS.

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
| Backend | `lib.rs` | L6906-7357 | TTS: edge-tts, remote, очистка текста |
| Frontend | `main.js` | L204-257 | Кнопка записи голоса |
| Frontend | `main.js` | L5059-5376 | UI режима звонка |
| External | `pc/tts_server.py` | L1-120 | Remote TTS сервер (PC, Qwen3-TTS) |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core, chat (for call mode LLM) |
| **Used by** | chat (TTS playback), proactive (voice messages) |
