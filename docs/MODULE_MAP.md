# Hanni — Module Map

> Quick lookup: файл/секция → модуль.
> Версия: v0.25.0 | Дата: 2026-03-03
> Архитектура модуляризирована: lib.rs → 13 Rust-модулей, main.js → 12 ES-модулей, styles.css → 7 CSS-модулей.

## Обзор проекта

| Параметр | Значение |
|----------|----------|
| Язык(и) | Rust, JavaScript, CSS, Python, Bash |
| Фреймворк(и) | Tauri 2, vanilla JS (ES modules) |
| Архитектура | Modular (13 Rust modules + 12 JS modules + 7 CSS modules) |
| Entry points | `lib.rs:run()` (backend), `index.html` → `main.js` (frontend) |
| Конфиг-файлы | Cargo.toml, tauri.conf.json, package.json, capabilities/default.json |
| Backend LOC | 11,831 (14 .rs файлов) |
| Frontend JS LOC | 8,975 (12 .js файлов) |
| Frontend CSS LOC | 5,763 (8 .css файлов) |

## Модули

| # | Модуль | Описание | Элементов | LOC |
|---|--------|----------|-----------|-----|
| 1 | core | Типы, промпты, схема БД (43 таблицы), миграции, entry point run(), constants | 16 | ~2,843 |
| 2 | chat | Протокол чата, LLM streaming, quality_check, ChatModeConfig, file commands | 2 | ~2,251 |
| 3 | memory | Семантический поиск (vec0 384d), reranking (FlashRank), Mem0 dedup, memory browser, conversation CRUD | 2 | ~1,153 |
| 4 | voice | Whisper STT, CPAL аудио, call mode (VAD + barge-in), TTS (edge-tts + Qwen3-TTS), wake word | 4 | ~2,108 |
| 5 | proactive | Проактивные сообщения, 14 стилей, OS-контекст, scoring, TTS pipeline | 2 | ~1,035 |
| 6 | system | Shell-команды, macOS интеграция, MLX сервер, clipboard, уведомления, API server | 2 | ~617 |
| 7 | focus | Режим фокуса (Pomodoro), блок-лист (/etc/hosts) | 2 | ~350 |
| 8 | calendar | События CRUD, Apple/Google Calendar sync, RRULE-экспансия | 2 | ~1,120 |
| 9 | tracker | Activities (start/stop), screen time, dashboard, purchases | 2 | ~600 |
| 10 | content | Заметки (FTS5), Kanban-задачи, database view system | 2 | ~1,888 |
| 11 | media | 9 типов медиа, пользовательские списки, статистика | 2 | ~460 |
| 12 | lifestyle | Еда, деньги, mindset, здоровье, контакты, дом (6 суб-доменов) | 2 | ~3,790 |
| 13 | page_framework | Свойства, database views, цели, настройки, фидбек, интеграции | 2 | ~1,700 |
| 14 | ui | Система вкладок, CSS-тема, навигация, HTML-структура, утилиты | 10 | ~7,296 |
| 15 | ml_devops | Training data, Claude distillation, LoRA, CI/CD, MCP server | 11 | ~1,800 |
| **TOTAL** | | | **63** | **~26,569** |

---

## Маппинг по путям

### Backend (Rust) — 13 модулей + lib.rs + main.rs

| Файл | Описание | LOC | Модуль |
|------|----------|-----|--------|
| `types.rs` | Все struct/enum: ChatMessage, ChatRequest, Delta, StreamChunk, ProactiveSettings/State, FocusState, CallModeState, wrapper-ы (HttpClient, LlmBusy, MlxProcess, HanniDb), constants (MLX_URL, MODEL), static atomics | 423 | core |
| `prompts.rs` | SYSTEM_PROMPT, SYSTEM_PROMPT_LITE, ACTION_KEYWORDS, build_tool_definitions (30+ tools), select_relevant_tools, needs_full_prompt | 700 | core |
| `db.rs` | init_db (43 таблицы + индексы), все migrate_* функции, hanni_data_dir, hanni_db_path | 763 | core |
| `chat.rs` | chat command (SSE streaming, tool calling), quality_check, ChatModeConfig (FULL/LITE/CALL), SpeakingGuard, read_file, list_dir, tracker commands, training/ratings | 677 | chat |
| `memory.rs` | embed_texts, store_fact_embedding, search_similar_facts, build_memory_context, gather_memory_candidates, rerank_facts, memory CRUD, conversation CRUD, process_conversation_end | 1,153 | memory |
| `voice.rs` | Whisper (download/init/transcribe), audio capture (CPAL), call mode (start/stop/resume/bargein), start_call_audio_loop (VAD), wake word SSE, voice cloning, save_voice_note | 1,232 | voice |
| `proactive.rs` | proactive_llm_call, gather_context, OS-context helpers (frontmost app, idle, music), TTS core (speak_*), proactive commands, proactive styles | 1,035 | proactive |
| `macos.rs` | run_shell (whitelist), open_url, notifications, volume, open/close app, music_control, clipboard, MLX server (LaunchAgent), idle/lock detection, calendar access guard, get_calendar_events, get_now_playing, get_browser_tab | 617 | system |
| `calendar.rs` | create/get/delete event, sync_apple_calendar, sync_google_ics, ICS parsing, RRule struct + expansion | 508 | calendar |
| `notes.rs` | Notes CRUD, custom pages CRUD, focus overlay toggle | 357 | content |
| `commands_data.rs` | Activities, work/projects, dev, hobbies, media items, sports, health/habits, dashboard, food, money, mindset, blocklist | 1,945 | lifestyle + tracker + media + focus |
| `commands_meta.rs` | Goals/settings, home, people/contacts, properties/views, integrations, model info, health check, reminders, web_search, read_url, updater, API server | 1,502 | page_framework + system |
| `lib.rs` | mod declarations, use re-exports, run() (app setup, state, generate_handler, proactive loop, focus monitor, auto-updater) | 913 | core |
| `main.rs` | fn main() → tauri entry | 6 | core |

### Voice Server (Python)

| Файл | Секция / Функции | LOC | Модуль |
|------|-----------------|-----|--------|
| voice_server.py | Config, state, VAD init, Whisper loading | 120 | voice |
| voice_server.py | record_and_transcribe(), CPAL recording | 180 | voice |
| voice_server.py | HTTP endpoints (/transcribe, /stop, /health, /tts) | 190 | voice |
| voice_server.py | /embed endpoint (384d embeddings) | 41 | memory |
| voice_server.py | /rerank endpoint (FlashRank) | 249 | memory |

### Frontend (JavaScript) — 12 ES-модулей

| Файл | Описание | LOC | Модуль |
|------|----------|-----|--------|
| `js/state.js` | invoke/listen/emit, DOM refs, shared state `S`, constants (TAB_ICONS, TAB_REGISTRY, TAB_DESCRIPTIONS, PROACTIVE_STYLE_DEFINITIONS, MEDIA_TYPES, PROPERTY_TYPE_DEFS), saveTabs, tabLoaders registry | 301 | core + ui |
| `js/utils.js` | renderMarkdown, escapeHtml, confirmModal, skeletonPage/Grid/List, normalizeHistoryMessage, renderPageHeader, setupPageHeaderControls, renderTabSettingsPage | 279 | ui + page_framework |
| `js/tabs.js` | renderTabBar, renderSubSidebar, renderSubTabBar, openTab, closeTab, switchTab, ensureViewDiv, activateView, loadSubTabContent (router), loadGoalsWidget, showAddGoalModal, tab dropdown, keyboard shortcuts, showChatSettingsMode, hideChatSettingsMode, tab drag-drop, context menu | 572 | ui |
| `js/conversations.js` | loadConversationsList, loadConversation, autoSaveConversation, new-chat-btn handler, conv-search | 164 | chat |
| `js/chat.js` | addMsg, scrollDown, send (agent loop), newChat, streamChat, renderChatWelcomeCard, removeChatWelcomeCard, loadChatSettings (4 tabs), addFeedbackButtons, showStub, toggleTTS, stopAllTTS, showAgentIndicator, file attachment, drag-drop, proactive/typing/reminder listeners | 1,574 | chat |
| `js/actions.js` | parseAndExecuteActions, executeAction (40+ action types), repairJson | 469 | chat |
| `js/voice.js` | checkVoiceServer, startRecording, stopRecordingAndSend, cancelRecording, toggleCallMode, startCallMode, endCallMode, handleCallTranscript, speakAndListen, startWakeWordSSE, stopWakeWordSSE, waveform animation, push-and-hold | 876 | voice |
| `js/tab-calendar.js` | loadCalendar, renderCalendar (month), renderWeekCalendar, renderDayCalendar, renderCalendarList, renderCalendarIntegrations, showAddEventModal | 612 | calendar |
| `js/tab-focus.js` | loadFocus, renderFocusCurrent, startFocusTimer, startPomodoro, renderFocusHistory, createFocusWidget, updateFocusWidget, toggleFocusWidgetPopover, updateFocusWidgetVisibility, bindFocusWidgetEvents | 562 | focus |
| `js/tab-notes.js` | loadNotes, notes views (list/kanban/timeline/table/gallery), renderNoteEditor, renderDatabaseView, property system, filter system | 1,529 | content + page_framework |
| `js/tab-data.js` | loadHome, loadMindset, loadFood, loadMoney, loadPeople, loadMemoryTab, loadAbout, loadWork, loadDevelopment, loadHobbies, loadSports, loadHealth, loadCustomPage | 1,857 | lifestyle + media + tracker + memory |
| `main.js` | Entry point: imports, tabLoaders registration (~60 functions), window.switchTab, init IIFE (custom pages, tab bar, focus widget, auto-restore conversation) | 180 | core |

### Frontend (CSS) — 7 модулей + barrel

| Файл | Описание | LOC | Модуль |
|------|----------|-----|--------|
| `styles.css` | Barrel: 7 @import statements | 8 | ui |
| `css/base.css` | :root variables, reset, body, scrollbar, drag region, page-header, data-table, buttons, inputs, badges, card classes, tab stubs | 532 | ui |
| `css/layout.css` | Tab bar (vertical), sub-tab bar, tab dropdown, app-body, sub-sidebar, conversations panel, inline goals, content area, views | 455 | ui |
| `css/chat.css` | Chat container, messages (.msg, .bot, .user), markdown rendering, code blocks, scroll-bottom, input area, action results, proactive, welcome card, training bar, confirm modal | 1,025 | ui |
| `css/settings.css` | Integrations page, macOS card, settings page, about card, toggle switch, proactive controls, chat settings sub-tabs | 714 | ui |
| `css/tabs-main.css` | Focus tab, notes (Notion-like), kanban, timeline, table/gallery view, custom pages, database view, property system | 1,545 | ui |
| `css/tabs-data.css` | Calendar (month/week/day/list), dashboard stats, work, dev, hobbies, sports, health, memory browser, people, modals, skeletons, keyframes | 953 | ui |
| `css/call.css` | Call mode overlay, focus floating widget | 531 | ui |

### Frontend (HTML)

| Файл | LOC | Модуль |
|------|-----|--------|
| index.html | 151 | ui |

### Scripts & DevOps

| Файл | LOC | Модуль |
|------|-----|--------|
| pc/tts_server.py | 120 | voice |
| desktop/finetune.py | 143 | ml_devops |
| desktop/finetune_data/prepare_data.py | 309 | ml_devops |
| desktop/finetune_data/synthetic_examples.jsonl | 73 | ml_devops |
| desktop/hanni-mcp/server.py | 321 | ml_devops |
| desktop/dev-server.py | 4 | ml_devops |
| scripts/generate_training_data.py | 497 | ml_devops |
| scripts/claude_distill.sh | 274 | ml_devops |
| scripts/nightly_train.sh | 217 | ml_devops |
| scripts/com.hanni.nightly-train.plist | ~30 | ml_devops |
| .github/workflows/release.yml | 65 | ml_devops |

### Config & Build

| Файл | LOC | Модуль |
|------|-----|--------|
| desktop/src-tauri/Cargo.toml | 40 | core |
| desktop/src-tauri/tauri.conf.json | 50 | core |
| desktop/src-tauri/capabilities/default.json | 37 | core |
| desktop/src-tauri/build.rs | 3 | core |
| desktop/src-tauri/main.rs | 6 | core |
| desktop/package.json | 12 | core |
| desktop/src-tauri/Entitlements.plist | 8 | core |
| desktop/src-tauri/Info.plist | 12 | core |

### Тестовые / справочные (не в модулях)

| Файл | LOC | Назначение |
|------|-----|------------|
| design-system.html | 1,110 | Справочник дизайн-системы |
| desktop/src/tauri-mock.js | 479 | Мок Tauri API для тестирования |
| desktop/src/test.html | 132 | Тестовая HTML-страница |

---

## Описание модулей

### 1. core
Фундамент: типы данных (`types.rs`), системные промпты и tool definitions (`prompts.rs`), SQLite-схема 43 таблицы (`db.rs`), entry point `run()` с 226 командами (`lib.rs`). Все конфиг-файлы сборки.

### 2. chat
Протокол общения с LLM: SSE-стриминг (`chat.rs`), agent loop до 5 итераций, `\`\`\`action` блоки (`actions.js` — 40+ типов), feedback/training data. Frontend: `chat.js` (send, addMsg, streaming, settings), `conversations.js` (CRUD), `actions.js` (парсинг/выполнение).

### 3. memory
Семантическая память: embeddings 384d + vec0, FTS5, 4-уровневый pipeline кандидатов, reranking FlashRank, Mem0 dedup, conversation processing (`memory.rs`). Frontend: memory browser в `tab-data.js`.

### 4. voice
Whisper STT, CPAL audio capture, call mode (VAD v5, barge-in), TTS (edge-tts + Qwen3-TTS) — всё в `voice.rs`. Frontend: `voice.js` (recording, call mode UI, wake word SSE).

### 5. proactive
Проактивные сообщения: 14 стилей, scoring, OS-контекст (app, URL, music, calendar), morning digest, TTS pipeline — `proactive.rs`. Frontend: listeners в `chat.js`.

### 6. system
macOS интеграция: shell (whitelist), URL, уведомления, clipboard, MLX сервер, idle detection — `macos.rs`. API server, web_search, updater — `commands_meta.rs`.

### 7. focus
Pomodoro с блокировкой через /etc/hosts — `commands_data.rs` (blocklist). Frontend: `tab-focus.js` (timer, widget, history).

### 8. calendar
События CRUD, Apple/Google Calendar sync, RRULE expansion — `calendar.rs`. Frontend: `tab-calendar.js` (month/week/day/list views).

### 9. tracker
Activities (start/stop), screen time, dashboard — `commands_data.rs`. Frontend: dashboard в `tab-data.js`.

### 10. content
Заметки (CRUD, FTS5) — `notes.rs`. Database view system, Kanban, gallery — `tab-notes.js`. Projects/tasks — `commands_data.rs`.

### 11. media
9 типов медиа (Music→Podcasts), user lists, stats — `commands_data.rs`. Frontend: `tab-data.js` (loadHobbies).

### 12. lifestyle
6 суб-доменов: Food, Money, Mindset, Health, Contacts, Home — `commands_data.rs` + `commands_meta.rs`. Frontend: `tab-data.js` (loadFood, loadMoney, loadPeople, loadHealth, loadSports, loadHome, loadMindset).

### 13. page_framework
Properties, views, goals, settings, integrations — `commands_meta.rs`. Frontend: database view в `tab-notes.js`, goals в `tabs.js`, settings в `chat.js`.

### 14. ui
Визуальный слой: 7 CSS-модулей (5,763 LOC), система вкладок (`tabs.js`), state/constants (`state.js`), утилиты (`utils.js`), HTML-структура (`index.html`).

### 15. ml_devops
Training data, Claude distillation, LoRA fine-tuning, MCP server, CI/CD pipeline.

---

## Карта зависимостей (Rust)

```
lib.rs (orchestrator)
├── types.rs      ← все модули
├── prompts.rs    ← chat.rs
├── db.rs         ← lib.rs (init)
├── chat.rs       ← memory.rs, prompts.rs, types.rs
├── memory.rs     ← types.rs
├── voice.rs      ← types.rs, chat (events)
├── proactive.rs  ← types.rs, memory (context)
├── macos.rs      ← types.rs
├── calendar.rs   ← types.rs
├── notes.rs      ← types.rs
├── commands_data.rs  ← types.rs
└── commands_meta.rs  ← types.rs
```

## Карта зависимостей (JS)

```
state.js ← utils.js ← tabs.js ← conversations.js
                               ← chat.js ← actions.js
                                          ← voice.js
                    ← tab-*.js, tab-data.js
main.js ← всё (entry point, tabLoaders registry)
```
