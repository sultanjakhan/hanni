# B7 — TTS синтез (edge-tts + remote): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B7: TTS синтез (edge-tts + remote) |
| Файл(ы) | lib.rs:L6920-7169 |
| LOC | 250 |
| Подфункций | 9 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B7.1 | speak_edge_tts (async, spawn thread) | L6920-6956 | 37 | (text, voice) → — | Spawn thread, shell: edge-tts → tmp mp3, afplay, удаление tmp | external | Medium |
| B7.2 | speak_remote_tts (async, spawn thread) | L6958-6984 | 27 | (text, voice, server_url) → — | Spawn thread, HTTP POST → tmp mp3, afplay, удаление tmp | external | Medium |
| B7.3 | speak_tts — роутер (async) | L7057-7081 | 25 | (text, voice) → — | Clean text, DB read (tts_server_url), dispatch to edge/remote/say | control | Low |
| B7.4 | speak_tts_sync — роутер (blocking) | L7084-7108 | 25 | (text, voice) → — | Clean text, DB read (tts_server_url), dispatch to sync variants | control | Low |
| B7.5 | speak_edge_tts_sync (blocking) | L7110-7138 | 29 | (text, voice) → — | Shell: edge-tts → tmp mp3, afplay (blocking), удаление tmp | external | Medium |
| B7.6 | speak_remote_tts_sync (blocking) | L7141-7162 | 22 | (text, voice, server_url) → — | HTTP POST → tmp mp3, afplay (blocking), удаление tmp | external | Medium |
| B7.7 | speak_text_blocking (Tauri cmd) | L7164-7171 | 8 | (text, voice?) → Result<()> | spawn_blocking → speak_tts_sync | bridge | Trivial |
| B7.8 | speak_sentence_blocking (Tauri cmd) | L7173-7183 | 11 | (sentence, voice?) → Result<()> | clean_text_for_tts + spawn_blocking → speak_tts_sync | bridge | Trivial |
| B7.9 | edge-tts binary discovery (внутри B7.1/B7.5) | встроено в B7.1, B7.5 | ~10 | — → edge-tts path | Перебор путей: edge-tts, /opt/homebrew, python3 -m | validation | Trivial |

## Data Flow

```
[Input: text to speak, voice name]
    │
    ├──────────────────────────┐
    │ (async path)             │ (sync/blocking path)
    ▼                          ▼
┌─ B7.3 speak_tts ──┐   ┌─ B7.4 speak_tts_sync ──┐
│  clean_text()      │   │  clean_text()            │
│  DB: tts_server_url│   │  DB: tts_server_url      │ ──→ [DB: app_settings]
└──────┬─────────────┘   └──────┬───────────────────┘
       │                        │
       ├─ remote? ──┐           ├─ remote? ──┐
       │            ▼           │            ▼
       │  ┌─ B7.2 remote ─┐    │  ┌─ B7.6 remote_sync ──┐
       │  │  POST /tts      │   │  │  POST /tts (blocking) │ ──→ [External: PC TTS]
       │  │  → tmp mp3      │   │  │  → tmp mp3            │
       │  │  → afplay       │   │  │  → afplay (wait)      │ ──→ [System: afplay]
       │  └────────────────┘    │  └────────────────────────┘
       │                        │
       ├─ Neural? ──┐           ├─ Neural? ──┐
       │            ▼           │            ▼
       │  ┌─ B7.1 edge ───┐    │  ┌─ B7.5 edge_sync ──────┐
       │  │  B7.9 discover  │   │  │  B7.9 discover          │
       │  │  edge-tts CLI   │   │  │  edge-tts CLI (blocking) │ ──→ [System: edge-tts]
       │  │  → tmp mp3      │   │  │  → tmp mp3              │
       │  │  → afplay       │   │  │  → afplay (wait)        │ ──→ [System: afplay]
       │  └────────────────┘    │  └──────────────────────────┘
       │                        │
       └─ macOS say ──┐         └─ macOS say ──┐
                      ▼                        ▼
              ┌─ say -v ─┐            ┌─ say -v (wait) ─┐
              │  .spawn() │            │  .status()       │ ──→ [System: say]
              └──────────┘            └──────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B7.1/B7.5 | edge-tts async/sync | Дублирование: одинаковая логика (discover → run → afplay → cleanup) в двух вариантах | Вынести общую fn с параметром blocking: bool | Medium |
| B7.2/B7.6 | remote TTS async/sync | Аналогичное дублирование (POST → save → afplay → cleanup) | Объединить в одну fn с параметром blocking | Medium |
| B7.3/B7.4 | Роутеры async/sync | Дублирование логики выбора (DB check → remote/edge/say) | Вынести routing logic в shared fn | Medium |
| B7.9 | Binary discovery | Дублируется в B7.1 и B7.5 (и в B8 get_tts_voices) | Вынести find_edge_tts() → Option<String> | Low |
