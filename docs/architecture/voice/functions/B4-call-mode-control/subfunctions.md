# B4 — Call Mode управление: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Call Mode управление |
| Файл(ы) | lib.rs:L1371-1487 |
| LOC | 117 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B4.1 | Запуск call mode (start_call_mode) | L1371-1392 | 22 | (CallMode state, AppHandle) → Result<String> | Мутация state (active=true, phase=listening, clear buffers), emit event, запуск audio loop | control | Low |
| B4.2 | Остановка call mode (stop_call_mode) | L1394-1410 | 17 | (CallMode state, AppHandle) → Result<String> | Мутация state (active=false, phase=idle, clear), emit event, killall afplay | control | Low |
| B4.3 | Возобновление прослушивания (resume_listening) | L1412-1426 | 15 | (CallMode state, AppHandle) → Result<()> | Мутация state (phase=listening, clear buffers), emit event | control | Trivial |
| B4.4 | Установка фазы speaking (set_speaking) | L1428-1438 | 11 | CallMode state → Result<()> | Мутация state (phase=speaking, reset speech_frames, barge_in=false) | control | Trivial |
| B4.5 | Проверка barge-in (check_bargein) | L1440-1446 | 7 | CallMode state → Result<bool> | — | query | Trivial |
| B4.6 | Сохранение голосовой заметки (save_voice_note) | L1448-1486 | 39 | (CallMode state, title) → Result<String> | Чтение last_recording, создание WAV файла | persist | Medium |

## Data Flow

```
[Frontend: startCallMode()]
    │
    ▼
┌─ B4.1 start_call_mode ─────────────────┐
│  Lock state → check !active             │
│  Set active=true, phase="listening"     │
│  Clear buffers, reset counters          │
│  emit("call-phase-changed","listening") │ ──→ [Event → Frontend]
│  start_call_audio_loop(state, app)      │ ──→ [B5: аудио-луп]
└─────────────────────────────────────────┘

[Frontend: endCallMode()]
    │
    ▼
┌─ B4.2 stop_call_mode ──────────────────┐
│  Lock state → active=false, phase=idle  │
│  Clear all buffers/counters             │
│  emit("call-phase-changed","idle")      │ ──→ [Event → Frontend]
│  killall afplay                         │ ──→ [System: kill TTS]
└─────────────────────────────────────────┘

[B5 audio loop / Frontend]
    │
    ▼
┌─ B4.3/B4.4 resume/set_speaking ────────┐
│  Переключение фазы state machine       │
│  emit event при resume                  │ ──→ [Event → Frontend]
└─────────────────────────────────────────┘

[Frontend: barge-in polling]
    │
    ▼
┌─ B4.5 check_bargein ──────────────────┐
│  Read state.barge_in → return bool     │
└────────────────────────────────────────┘

[Frontend: save_voice_note]
    │
    ▼
┌─ B4.6 save_voice_note ────────────────┐
│  Read last_recording from state        │
│  Create dir voice_notes/               │ ──→ [FS: ~/Library/App Support/Hanni/voice_notes/]
│  Write WAV (hound::WavWriter)          │ ──→ [FS: .wav file]
└────────────────────────────────────────┘
```

## Проблемные зоны

Нет критичных проблем. Каждая команда — компактный state-переход.
