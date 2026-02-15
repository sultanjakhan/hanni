# B8 — TTS Tauri-команды: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B8: TTS Tauri-команды |
| Файл(ы) | lib.rs:L7170-7357 |
| LOC | 188 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B8.1 | speak_text — основная TTS команда | L7185-7212 | 28 | (text, voice?, db) → Result<()> | Clean text, DB read tts_server_url, dispatch to edge/remote/say (non-blocking) | bridge | Low |
| B8.2 | stop_speaking — остановка TTS | L7214-7219 | 6 | — → Result<()> | killall say, killall afplay | control | Trivial |
| B8.3 | get_tts_voices — macOS voices discovery | L7221-7237 | 17 | — → Vec<Voice> (macOS) | Shell: say -v ? | query | Low |
| B8.4 | get_tts_voices — edge-tts voices discovery | L7238-7266 | 29 | — → Vec<Voice> (edge-tts) | Shell: edge-tts --list-voices (множественные пути) | query | Medium |
| B8.5 | get_tts_voices — парсинг edge-tts output | L7267-7283 | 17 | stdout → Vec<serde_json::Value> | — | parse | Low |
| B8.6 | get_tts_voices — финализация | L7284-7285 | 2 | Vec<Voice> → Result<Value> | — | transform | Trivial |

## Data Flow

```
[Tauri invoke: speak_text]
    │
    ▼
┌─ B8.1 speak_text ──────────────────────┐
│  voice = voice.unwrap_or(Svetlana)      │
│  clean = clean_text_for_tts(text)       │ ──→ [B6: clean_text_for_tts]
│  DB: SELECT tts_server_url              │ ──→ [DB: app_settings]
│  if remote → speak_remote_tts()         │ ──→ [B7.2]
│  elif Neural → speak_edge_tts()         │ ──→ [B7.1]
│  else → say -v (spawn)                  │ ──→ [System: say]
└─────────────────────────────────────────┘

[Tauri invoke: stop_speaking]
    │
    ▼
┌─ B8.2 stop_speaking ──────────────────┐
│  killall say                           │ ──→ [System: kill process]
│  killall afplay                        │ ──→ [System: kill process]
└────────────────────────────────────────┘

[Tauri invoke: get_tts_voices]
    │
    ▼
┌─ B8.3 macOS voices ──────────────────┐
│  say -v ?                             │ ──→ [System: say]
│  Parse: name, ru_RU/kk_KZ filter     │
│  → voices.push(macos engine)          │
└──────┬────────────────────────────────┘
       │
       ▼
┌─ B8.4 edge-tts discovery ────────────┐
│  Try: edge-tts, /opt/homebrew/...,    │ ──→ [System: edge-tts --list-voices]
│        /usr/local/...                 │
│  Fallback: python3 -m edge_tts        │ ──→ [System: python3]
└──────┬────────────────────────────────┘
       │ stdout
       ▼
┌─ B8.5 Парсинг edge-tts output ──────┐
│  Split lines → name, gender, lang     │
│  Filter: contains("Neural")           │
│  → voices.push(edge-tts engine)       │
└──────┬────────────────────────────────┘
       │
       ▼
┌─ B8.6 Финализация ──────────────────┐
│  Ok(json!(voices))                    │
└──────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B8.4 | edge-tts discovery | Дублирование binary search логики из B7.9 | Использовать общий find_edge_tts() helper | Low |
| B8.1 | speak_text | Дублирует routing логику из B7.3 speak_tts | Делегировать в speak_tts напрямую | Low |
