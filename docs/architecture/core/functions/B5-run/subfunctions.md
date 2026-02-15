# B5 — Запуск приложения run(): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: Запуск приложения (entry point) |
| Файл(ы) | lib.rs:L7391-8201 |
| LOC | 811 |
| Подфункций | 14 |
| Сложность (max) | High |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B5.1 | Инициализация proactive state | L7391-7393 | 3 | -- -> ProactiveState | File read (settings) | control | Trivial |
| B5.2 | Миграция данных и инициализация БД | L7395-7416 | 22 | -- -> HanniDb | DB: open, init, migrate; File: create_dir_all | persist | Low |
| B5.3 | Запуск MLX сервера | L7418-7421 | 4 | -- -> Arc<MlxProcess> | Process: spawn mlx_lm | external | Trivial |
| B5.4 | Инициализация состояния audio/focus/call | L7423-7449 | 27 | -- -> AudioRecording, FocusManager, CallMode | -- | control | Low |
| B5.5 | Tauri Builder: manage + plugins | L7451-7464 | 14 | states -> Builder | -- | control | Trivial |
| B5.6 | Регистрация команд (invoke_handler) | L7465-7683 | 219 | -- -> Builder | -- | control | Medium |
| B5.7 | Setup: автообновление | L7684-7702 | 19 | AppHandle -> () | Network: check update, download; System: restart | external | Low |
| B5.8 | Setup: сохранение system_prompt + API сервер | L7704-7712 | 9 | AppHandle -> () | File: write system_prompt.txt; Network: spawn API server | persist | Trivial |
| B5.9 | Setup: глобальный шорткат Cmd+Shift+H | L7714-7723 | 10 | AppHandle -> () | System: register global shortcut | control | Trivial |
| B5.10 | Setup: мониторинг фокус-режима | L7725-7772 | 48 | AppHandle, FocusState -> () | Shell: osascript /etc/hosts; Event: focus-ended | control | Medium |
| B5.11 | Setup: сбор activity snapshots (10 мин) | L7774-7852 | 79 | AppHandle -> () | DB: INSERT activity_snapshots, DELETE old; Shell: get OS data; State: pending_triggers | process | High |
| B5.12 | Setup: фоновое обучение (30 мин) | L7854-7966 | 113 | AppHandle -> () | DB: read snapshots, INSERT facts; Network: LLM call | external | High |
| B5.13 | Setup: проактивные сообщения (5 сек poll) | L7968-8184 | 217 | AppHandle, ProactiveState -> () | DB: read/write proactive_history; Network: LLM call; Event: proactive-message; TTS: speak | external | High |
| B5.14 | Build + Run + Exit cleanup | L8186-8201 | 16 | Builder -> () | Process: kill MLX on exit | control | Trivial |

## Data Flow

```
[Entry: run()]
    │
    ▼
┌─ B5.1 Proactive State ─┐
│  load_proactive_settings │ ←── [File: settings]
└──────┬──────────────────┘
       │
       ▼
┌─ B5.2 DB Init ──────────┐
│  migrate_old_data_dir()   │ ←── [File: ~/Documents/Hanni/]
│  Connection::open()       │ ──→ [DB: hanni.db]
│  init_db()                │
│  migrate_memory_json()    │
│  load calendar toggle     │
└──────┬───────────────────┘
       │
       ▼
┌─ B5.3 MLX Server ──┐
│  start_mlx_server()  │ ──→ [Process: python3 -m mlx_lm server]
└──────┬──────────────┘
       │
       ▼
┌─ B5.4 State Init ────────────┐
│  AudioRecording (WhisperState) │
│  FocusManager (FocusState)     │
│  CallMode (CallModeState)      │
└──────┬───────────────────────┘
       │
       ▼
┌─ B5.5-B5.6 Tauri Builder ───┐
│  .manage(HttpClient, LlmBusy │
│  ProactiveState, HanniDb,     │
│  AudioRecording, FocusManager,│
│  CallMode)                    │
│  .plugin(opener, fs, shell,   │
│  updater, process, shortcut)  │
│  .invoke_handler(150+ cmds)   │
└──────┬───────────────────────┘
       │
       ▼
┌─ B5.7-B5.13 Setup ──────────────────────────────────────┐
│                                                          │
│  B5.7 [Auto-updater]                                     │
│    check() -> download_and_install() -> restart()        │
│                                                          │
│  B5.8 [System Prompt + API]                              │
│    write SYSTEM_PROMPT ──→ [File]                        │
│    spawn_api_server()  ──→ [Network: :8235]              │
│                                                          │
│  B5.9 [Global Shortcut]                                  │
│    Cmd+Shift+H -> emit("global-toggle-call")             │
│                                                          │
│  B5.10 [Focus Monitor] — loop 2s                         │
│    check timer -> auto-stop -> clean /etc/hosts          │
│    kill blocked apps                                     │
│                                                          │
│  B5.11 [Activity Snapshots] — loop 10min                 │
│    get_frontmost_app() + browser + music                 │
│    INSERT activity_snapshots ──→ [DB]                    │
│    check triggers (distraction, events)                  │
│                                                          │
│  B5.12 [Background Learning] — loop 30min                │
│    read snapshots ←── [DB]                               │
│    LLM analyze ──→ [MLX Server]                          │
│    INSERT facts (observations) ──→ [DB]                  │
│                                                          │
│  B5.13 [Proactive Messages] — poll 5s                    │
│    adaptive timing (score-based)                         │
│    gather_context() + build_memory_context()             │
│    proactive_llm_call() ──→ [MLX Server]                 │
│    emit("proactive-message") + TTS                       │
│    INSERT proactive_history ──→ [DB]                     │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌─ B5.14 Build & Run ─┐
│  .build()             │
│  .run(exit handler)   │ ──→ kill MLX process on Exit
└──────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B5.13 | Проактивные сообщения | 217 LOC в одном async блоке — scoring, timing, LLM call, DB, TTS | Вынести scoring, gather, LLM call в отдельные функции | High |
| B5.12 | Фоновое обучение | 113 LOC — чтение снимков, LLM вызов, парсинг, запись | Вынести в отдельный модуль background_learning | Medium |
| B5.11 | Activity snapshots | 79 LOC — сбор OS данных, запись, проверка триггеров | Разделить сбор и анализ триггеров | Medium |
| B5.6 | Регистрация команд | 219 LOC — список из 150+ команд | Группировать по модулям через отдельные массивы | Low |
| B5.* | Вся функция | 811 LOC — самая большая функция в проекте | Декомпозировать setup в отдельные функции | High |
