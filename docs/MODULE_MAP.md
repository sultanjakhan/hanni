# Hanni — Module Map

> Quick lookup: файл/секция → модуль.
> Авто-генерация Phase 1. Обновляется при Phase 2.
> Версия: v0.18.0-dev | Дата: 2026-02-23

## Обзор проекта

| Параметр | Значение |
|----------|----------|
| Язык(и) | Rust, JavaScript, CSS, Python, Bash |
| Фреймворк(и) | Tauri 2, vanilla JS |
| Архитектура | Hybrid monolithic (2 монолитных файла + сателлиты) |
| Файл-тип | Monolithic (lib.rs 9734 LOC, main.js 6070 LOC) + multi-file (scripts, config) |
| Entry points | `lib.rs:run()` (backend), `index.html` → `main.js` (frontend) |
| Конфиг-файлы | Cargo.toml, tauri.conf.json, package.json, capabilities/default.json |

## Модули

| # | Модуль | Описание | Элементов | LOC |
|---|--------|----------|-----------|-----|
| 1 | core | Инициализация приложения, схема БД (43 таблицы + 16 индексов), константы, типы, миграции, entry point | 11 | ~2100 |
| 2 | chat | Протокол чата, LLM streaming, управление диалогами, выполнение действий, self-critique, fact extraction, insights | 2 | ~1500 |
| 3 | memory | Факты памяти, FTS5 поиск, семантический поиск (vec0 384d), reranking (FlashRank), Mem0 dedup, построение контекста, браузер памяти | 2 | ~750 |
| 4 | voice | Whisper STT (voice_server.py), захват аудио (CPAL), call mode (VAD + barge-in), TTS (Silero + edge-tts + remote Qwen3-TTS) | 4 | ~2200 |
| 5 | proactive | Проактивные сообщения, 14 стилей, сбор OS-контекста, scoring, engagement tracking, morning digest | 2 | ~750 |
| 6 | system | Shell-команды, URL, clipboard, уведомления, web search, файлы, MLX сервер, API (Axum :8235), macOS интеграция | 1 | ~600 |
| 7 | focus | Режим фокуса (Pomodoro), блок-лист (persistent + /etc/hosts), input sanitization | 2 | ~370 |
| 8 | calendar | События CRUD, синхронизация Apple/Google Calendar, полная RRULE-экспансия | 2 | ~1030 |
| 9 | tracker | Life tracker, активности (start/stop), screen time, snapshot capture, dashboard | 2 | ~650 |
| 10 | content | Заметки (FTS5), проекты с Kanban-задачами, ресурсы обучения | 2 | ~520 |
| 11 | media | Хобби, медиа-коллекции (9 типов), пользовательские списки, статистика | 2 | ~460 |
| 12 | lifestyle | Еда, деньги, mindset, здоровье/фитнес, контакты, дом (6 суб-доменов) | 2 | ~2200 |
| 13 | page_framework | Мета страниц, свойства, database views, цели, настройки, фидбек, интеграции | 2 | ~1700 |
| 14 | ui | Система вкладок (Cmd+W/T/1-9), навигация, Notion Dark тема, HTML структура | 3 | ~4500 |
| 15 | ml_devops | Training data, Claude distillation, LoRA fine-tuning, CI/CD pipeline, finetune.py | 8 | ~1350 |
| **TOTAL** | | | **46** | **~20168** |

---

## Маппинг по путям

### Backend (Rust)

| Файл | Секция / Функции | Строки (от-до) | LOC | Модуль |
|------|-----------------|----------------|-----|--------|
| lib.rs | Imports | L1-17 | 17 | core |
| lib.rs | MLX_URL, MODEL, SYSTEM_PROMPT, SYSTEM_PROMPT_LITE | L18-96 | 79 | core |
| lib.rs | build_tool_definitions() — 30+ tool defs для LLM | L99-570 | 472 | chat |
| lib.rs | ProactiveSettings, ProactiveState, impl блоки | L574-670 | 97 | proactive |
| lib.rs | HanniDb, hanni_data_dir(), hanni_db_path(), migrate_old_data_dir() | L675-735 | 61 | core |
| lib.rs | init_db() — 43 таблицы + 16 индексов (v0.18.0) | L736-1299 | 564 | core |
| lib.rs | migrate_memory_json(), migrate_events_source() | L1300-1355 | 56 | memory |
| lib.rs | embed_texts() — HTTP call к voice_server /embed | L1358-1413 | 56 | memory |
| lib.rs | build_memory_context_from_db() — FTS5 + semantic | L1416-1528 | 113 | memory |
| lib.rs | gather_memory_candidates() — 4-tier memory pipeline | L1531-1607 | 77 | memory |
| lib.rs | rerank_facts() — FlashRank через voice_server /rerank | L1610-1680 | 71 | memory |
| lib.rs | ChatMessage, ChatRequest, Delta, StreamChunk, HttpClient, LlmBusy, MlxProcess | L1683-1807 | 125 | chat |
| lib.rs | WhisperState, AudioRecording | L1810-1830 | 21 | voice |
| lib.rs | download_whisper_model, start/stop_recording, check_whisper_model, transcribe | L1833-2058 | 226 | voice |
| lib.rs | start_audio_capture() — CPAL stream setup | L2061-2124 | 64 | voice |
| lib.rs | start/stop_call_mode, resume_listening, set_speaking, check_bargein | L2127-2240 | 114 | voice |
| lib.rs | start_call_audio_loop() — VAD, silence detection, auto-transcribe | L2243-2535 | 293 | voice |
| lib.rs | FocusState, FocusManager, CallModeState, CallMode | L2538-2570 | 33 | focus |
| lib.rs | start_focus, stop_focus, get_focus_status, update_blocklist | L2572-2740 | 169 | focus |
| lib.rs | run_shell (whitelist), open_url, send_notification, set_volume, clipboard, web_search | L2743-2895 | 153 | system |
| lib.rs | get_training_stats, export_training_data, rate_message, get_message_ratings | L2898-3057 | 160 | page_framework |
| lib.rs | get_adapter_status, run_finetune — LoRA fine-tuning из UI | L3060-3120 | 61 | ml_devops |
| lib.rs | ChatModeConfig (FULL/LITE/CALL), select_relevant_tools(), needs_full_prompt() | L3120-3300 | 181 | chat |
| lib.rs | API server (Axum, :8235) — token auth, memory/chat endpoints | L3305-3510 | 206 | system |
| lib.rs | check_calendar_access(), run_osascript(), classify_app() | L3514-3600 | 87 | system |
| lib.rs | chat() — main LLM endpoint, semaphore, SSE streaming, tool calls | L3603-4030 | 428 | chat |
| lib.rs | quality_check_response() — self-critique pipeline | L4033-4085 | 53 | chat |
| lib.rs | read_file, list_dir | L4087-4137 | 51 | system |
| lib.rs | tracker_add_purchase → tracker_get_recent | L4139-4270 | 132 | tracker |
| lib.rs | get_activity_summary, get_calendar_events, get_now_playing, get_browser_tab | L4273-4465 | 193 | tracker |
| lib.rs | memory_remember, memory_recall, memory_forget, memory_search | L4468-4600 | 133 | memory |
| lib.rs | save/update/get/delete/search_conversation | L4603-4703 | 101 | chat |
| lib.rs | process_conversation_end() — fact extraction + insights + Mem0 dedup | L4706-4930 | 225 | chat |
| lib.rs | start/stop_activity, get_current_activity, get_activity_log | L5168-5278 | 111 | tracker |
| lib.rs | create_note → get_note (FTS5 search) | L5281-5380 | 100 | content |
| lib.rs | create_event, get_events, delete_event, sync_apple/google_calendar | L5383-5883 | 501 | calendar |
| lib.rs | create_project, get_projects, create/get_tasks, update_task_status | L5886-5963 | 78 | content |
| lib.rs | create_learning_item, get_learning_items | L5964-6010 | 47 | content |
| lib.rs | create_hobby → get_hobby_entries | L6011-6073 | 63 | media |
| lib.rs | create_workout, get_workouts, get_workout_stats | L6074-6118 | 45 | lifestyle |
| lib.rs | log_health → get_habits_today | L6120-6228 | 109 | lifestyle |
| lib.rs | get_dashboard_data | L6229-6290 | 62 | tracker |
| lib.rs | get_all_memories, delete_memory, update_memory | L6292-6354 | 63 | memory |
| lib.rs | add_media_item → get_media_stats (media, user lists) | L6356-6567 | 212 | media |
| lib.rs | log_food → get_expiring_products (food, recipes, products) | L6569-6758 | 190 | lifestyle |
| lib.rs | add_transaction → delete_debt (money, budgets, savings, subscriptions, debts) | L6760-7020 | 261 | lifestyle |
| lib.rs | save_journal_entry → get_mindset_check (journal, mood, principles) | L7022-7168 | 147 | lifestyle |
| lib.rs | add_to_blocklist → toggle_blocklist_item | L7170-7212 | 43 | focus |
| lib.rs | create_goal → get_app_setting | L7214-7294 | 81 | page_framework |
| lib.rs | add_home_item → toggle_home_item_needed | L7296-7353 | 58 | lifestyle |
| lib.rs | add_contact → toggle_contact_block_active | L7355-7510 | 156 | lifestyle |
| lib.rs | get_page_meta → update_view_config (metadata, properties, views) | L7513-7936 | 424 | page_framework |
| lib.rs | ProactiveStyleDef, build_proactive_system_prompt() | L7939-7990 | 52 | proactive |
| lib.rs | get_frontmost_app → get_user_name_from_memory (context gathering) | L7992-8460 | 469 | proactive |
| lib.rs | adaptive_tts_rate → speak_tts_sync (TTS pipeline) | L8463-8648 | 186 | voice |
| lib.rs | speak_text, speak_text_blocking, speak_sentence_blocking, stop_speaking, get_tts_voices | L8650-8783 | 134 | voice |
| lib.rs | updater_with_headers(), get_app_version() | L8786-8816 | 31 | core |
| lib.rs | run() — app entry, state setup, 192 команд, proactive loop | L8819-9734 | 916 | core |
| main.rs | fn main() → tauri entry | L1-6 | 6 | core |

### Voice Server (Python)

| Файл | Секция / Функции | Строки (от-до) | LOC | Модуль |
|------|-----------------|----------------|-----|--------|
| voice_server.py | Config, state, VAD init, Whisper loading | L1-120 | 120 | voice |
| voice_server.py | record_and_transcribe(), CPAL recording | L121-300 | 180 | voice |
| voice_server.py | HTTP endpoints (/transcribe, /stop, /health, /tts) | L301-490 | 190 | voice |
| voice_server.py | /embed endpoint (384d embeddings) | L490-530 | 41 | memory |
| voice_server.py | /rerank endpoint (FlashRank) | L530-611 | 82 | memory |

### Frontend (JavaScript)

| Файл | Секция / Функции | Строки (от-до) | LOC | Модуль |
|------|-----------------|----------------|-----|--------|
| main.js | Tauri imports, DOM elements, global state | L1-38 | 38 | ui |
| main.js | PROACTIVE_STYLE_DEFINITIONS | L41-56 | 16 | proactive |
| main.js | SVG icons, TAB_ICONS, TAB_REGISTRY, TAB_DESCRIPTIONS | L59-115 | 57 | ui |
| main.js | renderPageHeader() | L117-130 | 14 | page_framework |
| main.js | openTabs, activeTab, activeSubTab, tab init, saveTabs(), update-available | L132-162 | 31 | ui |
| main.js | proactive-message event, typing signal | L164-198 | 35 | proactive |
| main.js | Voice recording button, push-to-talk, cancel | L204-360 | 157 | voice |
| main.js | loadConversationsList(), loadConversation(), autoSaveConversation() | L368-500 | 133 | chat |
| main.js | escapeHtml(), skeleton loaders | L502-540 | 39 | ui |
| main.js | New chat button, conversation search | L542-560 | 19 | chat |
| main.js | renderTabBar(), renderSubSidebar() | L562-650 | 89 | ui |
| main.js | loadGoalsWidget(), showAddGoalModal() | L652-720 | 69 | page_framework |
| main.js | openTab(), closeTab(), switchTab(), activateView(), loadSubTabContent() | L722-790 | 69 | ui |
| main.js | Tab dropdown, close dropdown, keyboard shortcuts (Cmd+W/T/1-9) | L792-830 | 39 | ui |
| main.js | showChatSettingsMode(), hideChatSettingsMode(), loadChatSettings() (4 tabs: General/Memory/Personality/Training) | L832-975 | 144 | chat |
| main.js | scrollDown() | L977-990 | 14 | ui |
| main.js | addMsg(), addFeedbackButtons() | L992-1100 | 109 | chat |
| main.js | attachment/drag-drop handlers | L1102-1180 | 79 | chat |
| main.js | executeAction() — 30+ action types | L1399-1695 | 297 | chat |
| main.js | parseAndExecuteActions(), repairJson() | L1697-1712 | 16 | chat |
| main.js | streamChat() — SSE token listener | L1715-1775 | 61 | chat |
| main.js | toggleTTS(), stopAllTTS() | L1777-1830 | 54 | voice |
| main.js | send() — agent loop (MAX_ITERATIONS=5), streaming, tool execution | L1835-2040 | 206 | chat |
| main.js | loadHome(), loadSupplies(), loadShoppingList() | L2043-2145 | 103 | lifestyle |
| main.js | loadMindset(), loadJournal(), loadMoodLog(), loadPrinciples() | L2146-2253 | 108 | lifestyle |
| main.js | loadFood() → showAddProductModal() (food log, recipes, products) | L2254-2463 | 210 | lifestyle |
| main.js | loadMoney() → loadDebts() (transactions, budgets, savings, subscriptions, debts) | L2464-2683 | 220 | lifestyle |
| main.js | loadPeople(), contact actions, showAddContactModal() | L2684-2841 | 158 | lifestyle |
| main.js | loadMemoryTab(), loadAllFacts(), renderMemoryList(), loadMemorySearch() | L2842-3117 | 276 | memory |
| main.js | loadIntegrations(), loadSettings(), loadBlocklist(), loadAbout() | L3118-3360 | 243 | page_framework |
| main.js | loadDashboard() | L3361-3430 | 70 | tracker |
| main.js | loadFocus() | L3431-3536 | 106 | focus |
| main.js | loadNotes(), renderNotesList(), openNote() | L3537-3662 | 126 | content |
| main.js | loadCalendar() → renderCalendarIntegrations() (month/week/day/list views) | L3663-4189 | 527 | calendar |
| main.js | loadWork(), renderWork() | L4190-4264 | 75 | content |
| main.js | loadDevelopment(), renderDevelopment(), showAddLearningModal() | L4265-4350 | 86 | content |
| main.js | renderDatabaseView() → loadFiltersFromViewConfig() (Notion-like views) | L4351-4968 | 618 | page_framework |
| main.js | MEDIA_TYPES, loadHobbies() → showMediaDetail() (9 media types) | L4969-5148 | 180 | media |
| main.js | loadSports() → showAddWorkoutModal() | L5149-5299 | 151 | lifestyle |
| main.js | loadHealth(), renderHealth() | L5300-5455 | 156 | lifestyle |
| main.js | Call Mode: toggleCallMode(), startCallMode(), endCallMode(), speakAndListen() | L5457-5770 | 314 | voice |
| main.js | Keyboard shortcuts, DOMContentLoaded setup | L5772-6070 | 299 | ui |

### Frontend (CSS + HTML)

| Файл | Секция | Строки (от-до) | LOC | Модуль |
|------|--------|----------------|-----|--------|
| styles.css | CSS variables, theme (`:root`) | L7-65 | 59 | ui |
| styles.css | Scrollbar, top tab bar | L79-223 | 145 | ui |
| styles.css | App layout (body, sub-sidebar, content) | L265-404 | 140 | ui |
| styles.css | Chat interface (container, messages, scroll) | L416-633 | 218 | ui |
| styles.css | Input controls (send, TTS, file, recording) | L644-743 | 100 | ui |
| styles.css | General components (modals, cards, buttons, pills) | L744-1153 | 410 | ui |
| styles.css | Chat settings mode | L1154-1172 | 19 | ui |
| styles.css | Tab-specific styles (focus → people) | L1173-2638 | 1466 | ui |
| styles.css | Modal & data views (database, inline edit, properties) | L2638-3145 | 508 | ui |
| styles.css | Call mode interface | L3145-3232 | 88 | ui |
| styles.css | Advanced controls (proactive, sub-tabs) | L3315-3563 | 249 | ui |
| index.html | HTML structure, head, body, all containers | L1-116 | 116 | ui |

### Scripts & DevOps

| Файл | Строки | Модуль |
|------|--------|--------|
| pc/tts_server.py | 120 | voice |
| desktop/finetune.py | 95 | ml_devops |
| scripts/generate_training_data.py | 497 | ml_devops |
| scripts/claude_distill.sh | 274 | ml_devops |
| scripts/nightly_train.sh | 217 | ml_devops |
| scripts/com.hanni.nightly-train.plist | ~30 | ml_devops |
| .github/workflows/release.yml | 65 | ml_devops |

### Config & Build

| Файл | Строки | Модуль |
|------|--------|--------|
| desktop/src-tauri/Cargo.toml | 40 | core |
| desktop/src-tauri/tauri.conf.json | 50 | core |
| desktop/src-tauri/capabilities/default.json | 37 | core |
| desktop/src-tauri/build.rs | 3 | core |
| desktop/src-tauri/main.rs | 6 | core |
| desktop/package.json | 12 | core |
| desktop/src-tauri/Entitlements.plist | 8 | core |
| desktop/src-tauri/Info.plist | 12 | core |

### Тестовые / справочные (не в модулях)

| Файл | Строки | Назначение |
|------|--------|------------|
| design-system.html | 1110 | Справочник дизайн-системы |
| desktop/src/tauri-mock.js | 300 | Мок Tauri API для тестирования |
| desktop/src/test.html | 98 | Тестовая HTML-страница |

---

## Описание модулей

### 1. core
Фундамент приложения: константы (MLX_URL, MODEL, SYSTEM_PROMPT), типы данных, инициализация SQLite схемы (43 таблицы + 16 индексов), миграция старых данных, функция `run()` (регистрация 192 команд, setup state, proactive loop). Включает все конфиг-файлы сборки. v0.18.0: добавлены SQL индексы для производительности.

### 2. chat
Протокол общения с LLM: SSE-стриминг токенов, типы ChatMessage/ChatRequest, основная команда `chat` с LLM семафором и tool calling, CRUD диалогов, парсинг и выполнение `\`\`\`action` блоков (30+ типов действий), agent loop до 5 итераций, feedback-рейтинги. v0.18.0: self-critique (quality_check_response), conversation insights extraction, build_tool_definitions (30+ tool defs), adaptive ChatModeConfig (FULL/LITE/CALL).

### 3. memory
Система памяти ИИ: CRUD фактов в SQLite с FTS5 + семантический поиск (vec0 384d embeddings), 4-уровневый pipeline сбора кандидатов (recent → vec_search → FTS5 → global), reranking через FlashRank, Mem0-style дедупликация, построение контекстного окна (до 80 фактов). v0.17.0: semantic retrieval + dedup pipeline.

### 4. voice
Голосовой ввод/вывод: Whisper STT через voice_server.py (Python, auto-загрузка large-v3-turbo), захват аудио через CPAL, Call Mode (Silero VAD v5, подсчёт фреймов речи/тишины, barge-in), TTS (Silero local + edge-tts + remote Qwen3-TTS на PC). voice_server.py (611 LOC) также обслуживает /embed и /rerank эндпоинты.

### 5. proactive
Система проактивных сообщений: настройки (интервал, тихие часы, голос), 14 стилей (digest, mood_check, nudge, curiosity, humor, care, etc.), scoring-система с 10+ факторами, сбор OS-контекста (активное приложение, URL, музыка, события, screen time), engagement tracking, morning digest.

### 6. system
Системные команды ОС: выполнение shell (whitelist), открытие URL, уведомления macOS, громкость, clipboard, DuckDuckGo web search, чтение/листинг файлов, REST API сервер (Axum, :8235, token auth), запуск MLX сервера (LaunchAgent + fallback), osascript, классификация приложений.

### 7. focus
Режим фокуса (Pomodoro): таймер с блокировкой приложений/сайтов через /etc/hosts (sudo), persistent блок-лист в БД (с расписанием), UI с обратным отсчётом. v0.18.0: санитизация имён приложений/сайтов (AppleScript injection fix).

### 8. calendar
Календарь: CRUD событий, синхронизация Apple Calendar (EventKit), парсинг Google ICS, полная RRULE-экспансия (daily/weekly/monthly/yearly, BYDAY, BYMONTHDAY, COUNT, UNTIL). UI: month/week/day/list виды.

### 9. tracker
Life tracker: покупки/время/цели/заметки, трекинг активностей (start/stop/duration), сбор screen time через activity_snapshots, summary по приложениям/сайтам, агрегированный dashboard.

### 10. content
Контентные модули: заметки (CRUD, автосохранение, закрепление/архив, FTS5), проекты с задачами (Kanban-статусы), ресурсы обучения (курсы/книги/статьи). UI: редактор заметок, списки проектов, фильтры по типу.

### 11. media
Медиа-коллекции: 9 типов (Music/Anime/Manga/Movies/Series/Cartoons/Games/Books/Podcasts), статусы (Planned→Completed→Dropped), рейтинги, прогресс, пользовательские списки, статистика. UI: overview + type tabs.

### 12. lifestyle
Объединённые модули жизни (6 суб-доменов):
- **Food**: лог питания (калории/макросы), рецепты, продукты с отслеживанием срока годности
- **Money**: расходы/доходы, бюджеты, цели сбережений, подписки, долги
- **Mindset**: дневник (mood/energy/stress/gratitude/wins), лог настроения, жизненные принципы
- **Health/Fitness**: здоровье (сон/вода/шаги/вес), привычки (ежедневные чек-листы), тренировки
- **Contacts**: люди (телефон/email/категория/день рождения), избранные/заблокированные, персональные блок-листы
- **Home**: инвентарь расходников, список покупок

### 13. page_framework
Фреймворк для кастомизации страниц: метаданные вкладок, определения свойств (пользовательские колонки), значения свойств, конфиги видов (фильтры/сортировки/колонки), цели (per-tab), настройки приложения (4 таба: General/Memory/Personality/Training), интеграции, training data export + feedback ratings, generic database view (Notion-like сортировка, фильтрация, inline edit).

### 14. ui
Визуальный слой: Notion Dark тема (CSS variables), система вкладок (открытие/закрытие, Cmd+W/T/1-9, localStorage), суб-сайдбар, skeleton loaders, 3753 строки CSS (все компоненты от чата до call mode), HTML-структура. Включает рендеринг tab bar, навигацию и keyboard shortcuts.

### 15. ml_devops
MLOps и DevOps: генерация 77+ обучающих примеров, Claude-дистилляция, ночной LoRA fine-tuning (3 AM, feedback-based, adapter backup), QLoRA скрипт (finetune.py), UI для запуска fine-tuning (get_adapter_status + run_finetune), CI/CD pipeline (GitHub Actions → macOS build → release → gist update).
