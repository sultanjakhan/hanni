# Hanni — Module Map

> Quick lookup: файл/секция → модуль.
> Авто-генерация Phase 1. Обновляется при Phase 2.
> Версия: v0.13.9 | Дата: 2026-02-15

## Обзор проекта

| Параметр | Значение |
|----------|----------|
| Язык(и) | Rust, JavaScript, CSS, Python, Bash |
| Фреймворк(и) | Tauri 2, vanilla JS |
| Архитектура | Hybrid monolithic (2 монолитных файла + сателлиты) |
| Файл-тип | Monolithic (lib.rs 8201 LOC, main.js 5376 LOC) + multi-file (scripts, config) |
| Entry points | `lib.rs:run()` (backend), `index.html` → `main.js` (frontend) |
| Конфиг-файлы | Cargo.toml, tauri.conf.json, package.json, capabilities/default.json |

## Модули

| # | Модуль | Описание | Элементов | LOC |
|---|--------|----------|-----------|-----|
| 1 | core | Инициализация приложения, схема БД, константы, типы, миграции, entry point | 11 | ~1800 |
| 2 | chat | Протокол чата, LLM streaming, управление диалогами, выполнение действий | 2 | ~1340 |
| 3 | memory | Факты памяти, FTS5 поиск, построение контекста, браузер памяти | 2 | ~620 |
| 4 | voice | Whisper STT, захват аудио, запись, call mode, VAD, TTS | 3 | ~1650 |
| 5 | proactive | Проактивные сообщения, сбор контекста, стили, engagement | 2 | ~690 |
| 6 | system | Shell-команды, URL, clipboard, уведомления, web search, файлы, MLX, API | 1 | ~540 |
| 7 | focus | Режим фокуса, блок-лист (persistent + /etc/hosts) | 2 | ~350 |
| 8 | calendar | События CRUD, синхронизация Apple/Google Calendar, RRULE | 2 | ~1030 |
| 9 | tracker | Life tracker (JSON), активности, screen time, dashboard | 2 | ~600 |
| 10 | content | Заметки, проекты, задачи, обучение | 2 | ~520 |
| 11 | media | Хобби, медиа-коллекции (9 типов), пользовательские списки | 2 | ~460 |
| 12 | lifestyle | Еда, деньги, mindset, здоровье/фитнес, контакты, дом | 2 | ~2080 |
| 13 | page_framework | Мета страниц, свойства, database views, цели, настройки, интеграции | 2 | ~1610 |
| 14 | ui | Система вкладок, навигация, стилизация, HTML структура | 3 | ~4370 |
| 15 | ml_devops | Скрипты обучения, CI/CD, конфиги сборки, release pipeline | 7 | ~1180 |
| **TOTAL** | | | **44** | **~18840** |

---

## Маппинг по путям

### Backend (Rust)

| Файл | Секция / Функции | Строки (от-до) | LOC | Модуль |
|------|-----------------|----------------|-----|--------|
| lib.rs | Imports | L1-13 | 13 | core |
| lib.rs | MLX_URL, MODEL, SYSTEM_PROMPT, SYSTEM_PROMPT_LITE, ACTION_KEYWORDS | L14-155 | 142 | core |
| lib.rs | needs_full_prompt(), data_file_path() | L158-167 | 10 | core |
| lib.rs | TrackerData struct | L168-180 | 13 | tracker |
| lib.rs | ProactiveSettings, ProactiveState, impl блоки | L181-271 | 91 | proactive |
| lib.rs | HanniDb, hanni_data_dir(), hanni_db_path(), migrate_old_data_dir(), copy_dir_recursive() | L274-345 | 72 | core |
| lib.rs | init_db() — схема 40+ таблиц | L346-879 | 534 | core |
| lib.rs | migrate_memory_json(), migrate_events_source() | L880-932 | 53 | memory |
| lib.rs | build_memory_context_from_db() | L933-1025 | 93 | memory |
| lib.rs | proactive_settings_path(), load_proactive_settings(), save_proactive_settings() | L1026-1050 | 25 | proactive |
| lib.rs | ChatMessage, ChatRequest, Delta, Choice, StreamChunk, TokenPayload, HttpClient, LlmBusy, MlxProcess | L1052-1093 | 42 | chat |
| lib.rs | WhisperState, AudioRecording | L1094-1103 | 10 | voice |
| lib.rs | whisper_model_path(), whisper_turbo_path(), download_whisper_model, start_recording, stop_recording, check_whisper_model, transcribe_samples() | L1104-1233 | 130 | voice |
| lib.rs | start_audio_capture() | L1234-1371 | 138 | voice |
| lib.rs | start_call_mode, stop_call_mode, call_mode_resume_listening, call_mode_set_speaking, call_mode_check_bargein, save_voice_note | L1371-1487 | 117 | voice |
| lib.rs | start_call_audio_loop() | L1488-1803 | 316 | voice |
| lib.rs | FocusState, FocusManager, CallModeState, CallMode, FocusStatus | L1805-1837 | 33 | focus |
| lib.rs | start_focus, stop_focus, get_focus_status, update_blocklist | L1839-1995 | 157 | focus |
| lib.rs | run_shell, open_url, send_notification, set_volume, get_clipboard, set_clipboard, web_search | L1999-2130 | 132 | system |
| lib.rs | get_training_stats, export_training_data, rate_message, get_message_ratings | L2133-2319 | 187 | page_framework |
| lib.rs | api_token_path(), get_or_create_api_token(), find_python(), start_mlx_server() | L2322-2521 | 200 | system |
| lib.rs | check_calendar_access(), run_osascript(), classify_app() | L2525-2918 | 394 | system |
| lib.rs | chat (main LLM endpoint) | L2695-2883 | 189 | chat |
| lib.rs | read_file, list_dir | L2885-2936 | 52 | system |
| lib.rs | tracker_add_purchase, tracker_add_time, tracker_add_goal, tracker_add_note, tracker_get_stats, tracker_get_recent | L2937-3057 | 121 | tracker |
| lib.rs | get_activity_summary, get_calendar_events, get_now_playing, get_browser_tab | L3058-3243 | 186 | tracker |
| lib.rs | memory_remember, memory_recall, memory_forget, memory_search | L3246-3374 | 129 | memory |
| lib.rs | save_conversation, update_conversation, get_conversations, get_conversation, delete_conversation, search_conversations, process_conversation_end | L3375-3611 | 237 | chat |
| lib.rs | start_activity, stop_activity, get_current_activity, get_activity_log | L3614-3726 | 113 | tracker |
| lib.rs | create_note, update_note, delete_note, get_notes, get_note | L3727-3828 | 102 | content |
| lib.rs | create_event, get_events, delete_event, sync_apple_calendar, sync_google_ics | L3829-4329 | 501 | calendar |
| lib.rs | create_project, get_projects, create_task, get_tasks, update_task_status | L4330-4408 | 79 | content |
| lib.rs | create_learning_item, get_learning_items | L4408-4457 | 50 | content |
| lib.rs | create_hobby, get_hobbies, log_hobby_entry, get_hobby_entries | L4457-4521 | 65 | media |
| lib.rs | create_workout, get_workouts, get_workout_stats | L4520-4564 | 45 | lifestyle |
| lib.rs | log_health, get_health_today, create_habit, check_habit, get_habits_today | L4564-4676 | 113 | lifestyle |
| lib.rs | get_dashboard_data | L4675-4736 | 62 | tracker |
| lib.rs | get_all_memories, delete_memory, update_memory | L4738-4800 | 63 | memory |
| lib.rs | add_media_item → get_media_stats (media items, user lists) | L4800-5012 | 213 | media |
| lib.rs | log_food → get_expiring_products (food log, recipes, products) | L5012-5204 | 193 | lifestyle |
| lib.rs | add_transaction → delete_debt (transactions, budgets, savings, subscriptions, debts) | L5204-5468 | 265 | lifestyle |
| lib.rs | save_journal_entry → get_mindset_check (journal, mood, principles) | L5468-5612 | 145 | lifestyle |
| lib.rs | add_to_blocklist → toggle_blocklist_item | L5613-5659 | 47 | focus |
| lib.rs | create_goal, get_goals, update_goal, delete_goal, set_app_setting, get_app_setting | L5660-5741 | 82 | page_framework |
| lib.rs | add_home_item → toggle_home_item_needed | L5742-5801 | 60 | lifestyle |
| lib.rs | add_contact → toggle_contact_block_active (contacts, contact blocks) | L5798-5959 | 162 | lifestyle |
| lib.rs | get_page_meta → update_view_config (page meta, properties, views) | L5959-6384 | 426 | page_framework |
| lib.rs | ProactiveStyleDef, build_proactive_system_prompt() | L6385-6436 | 52 | proactive |
| lib.rs | get_frontmost_app → get_user_name_from_memory (context gathering, proactive helpers) | L6438-6905 | 468 | proactive |
| lib.rs | adaptive_tts_rate, speak_edge_tts, speak_remote_tts, clean_text_for_tts, speak_tts, speak_tts_sync, speak_edge_tts_sync, speak_remote_tts_sync | L6906-7169 | 264 | voice |
| lib.rs | TTS/voice commands (speak_text, stop_speaking, etc.) | L7170-7357 | 188 | voice |
| lib.rs | updater_with_headers(), get_app_version() | L7358-7386 | 29 | core |
| lib.rs | run() — app entry, state setup, 190+ command registration, proactive loop | L7391-8201 | 811 | core |
| main.rs | fn main() → tauri entry | L1-6 | 6 | core |

### Frontend (JavaScript)

| Файл | Секция / Функции | Строки (от-до) | LOC | Модуль |
|------|-----------------|----------------|-----|--------|
| main.js | Tauri imports, DOM elements, global state | L1-38 | 38 | ui |
| main.js | PROACTIVE_STYLE_DEFINITIONS | L41-56 | 16 | proactive |
| main.js | SVG icons, TAB_ICONS, TAB_REGISTRY, TAB_DESCRIPTIONS | L59-115 | 57 | ui |
| main.js | renderPageHeader() | L117-130 | 14 | page_framework |
| main.js | openTabs, activeTab, activeSubTab, tab init, saveTabs(), update-available | L132-162 | 31 | ui |
| main.js | proactive-message event, typing signal | L164-198 | 35 | proactive |
| main.js | Voice recording button | L204-257 | 54 | voice |
| main.js | focus-ended event | L261-267 | 7 | focus |
| main.js | loadConversationsList(), loadConversation(), autoSaveConversation() | L271-390 | 120 | chat |
| main.js | escapeHtml(), skeleton loaders | L392-428 | 37 | ui |
| main.js | New chat button, conversation search | L431-446 | 16 | chat |
| main.js | renderTabBar(), renderSubSidebar() | L450-530 | 81 | ui |
| main.js | loadGoalsWidget(), showAddGoalModal() | L532-599 | 68 | page_framework |
| main.js | openTab(), closeTab(), switchTab(), activateView(), loadSubTabContent() | L602-667 | 66 | ui |
| main.js | Tab dropdown, close dropdown, keyboard shortcuts (Cmd+W/T/1-9) | L669-700 | 32 | ui |
| main.js | showChatSettingsMode(), hideChatSettingsMode(), loadChatSettings() | L704-963 | 260 | chat |
| main.js | scrollDown() | L964-976 | 13 | ui |
| main.js | addMsg() | L978-1002 | 25 | chat |
| main.js | addFeedbackButtons(), attachment/drag-drop handlers | L1004-1104 | 101 | chat |
| main.js | executeAction() — 20+ action types | L1105-1382 | 278 | chat |
| main.js | parseAndExecuteActions(), repairJson() | L1383-1419 | 37 | chat |
| main.js | streamChat(), showAgentIndicator(), toggleTTS(), stopAllTTS() | L1421-1518 | 98 | chat |
| main.js | send(), newChat(), event listeners | L1520-1696 | 177 | chat |
| main.js | loadHome(), loadSupplies(), loadShoppingList() | L1699-1801 | 103 | lifestyle |
| main.js | loadMindset(), loadJournal(), loadMoodLog(), loadPrinciples() | L1802-1909 | 108 | lifestyle |
| main.js | loadFood(), loadFoodLog(), showAddFoodModal(), loadRecipes(), showAddRecipeModal(), loadProducts(), showAddProductModal() | L1910-2119 | 210 | lifestyle |
| main.js | loadMoney(), loadTransactions(), loadBudgets(), loadSavings(), loadSubscriptions(), loadDebts() | L2120-2339 | 220 | lifestyle |
| main.js | loadPeople(), contact actions, showAddContactModal() | L2340-2497 | 158 | lifestyle |
| main.js | loadMemoryTab(), loadAllFacts(), MEMORY_CATEGORIES, renderMemoryList(), loadMemoryInSettings(), loadMemorySearch(), panelItem() | L2498-2773 | 276 | memory |
| main.js | loadIntegrations(), loadSettings(), loadBlocklist(), loadAbout(), showStub() | L2774-2985 | 212 | page_framework |
| main.js | loadDashboard() | L2986-3055 | 70 | tracker |
| main.js | loadFocus() | L3056-3161 | 106 | focus |
| main.js | loadNotes(), renderNotesList(), openNote() | L3162-3287 | 126 | content |
| main.js | loadCalendar(), renderCalendar(), renderWeekCalendar(), showAddEventModal(), renderDayCalendar(), renderCalendarList(), renderCalendarIntegrations() | L3288-3814 | 527 | calendar |
| main.js | loadWork(), renderWork() | L3815-3889 | 75 | content |
| main.js | loadDevelopment(), renderDevelopment(), showAddLearningModal() | L3890-3975 | 86 | content |
| main.js | renderDatabaseView(), formatPropValue(), startInlineEdit(), PROPERTY_TYPE_DEFS, showAddPropertyModal(), showColumnMenu(), sortDatabaseView(), renderFilterBar(), showFilterBuilderModal(), applyFilters(), saveFiltersToViewConfig(), loadFiltersFromViewConfig() | L3976-4597 | 622 | page_framework |
| main.js | MEDIA_TYPES, loadHobbies(), loadHobbiesOverview(), loadMediaList(), showAddMediaModal(), showMediaDetail() | L4594-4773 | 180 | media |
| main.js | loadSports(), loadMartialArts(), loadSportsStats(), renderSports(), showAddWorkoutModal() | L4774-4924 | 151 | lifestyle |
| main.js | loadHealth(), renderHealth() | L4925-5058 | 134 | lifestyle |
| main.js | Call mode state, toggleCallMode(), startCallMode(), endCallMode(), splitIntoSentences(), speakAndListen(), shortcuts | L5059-5376 | 318 | voice |

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
Фундамент приложения: константы (MLX_URL, MODEL, SYSTEM_PROMPT), типы данных, инициализация SQLite схемы (40+ таблиц), миграция старых данных, функция `run()` (регистрация 190+ команд, setup state, proactive loop). Включает все конфиг-файлы сборки.

### 2. chat
Протокол общения с LLM: SSE-стриминг токенов, типы ChatMessage/ChatRequest, основная команда `chat`, CRUD диалогов (save/load/delete/search), парсинг и выполнение `\`\`\`action` блоков (20+ типов действий), feedback-рейтинги.

### 3. memory
Система памяти ИИ: CRUD фактов в SQLite с FTS5, построение контекстного окна (до 80 фактов, 800 токенов), миграция из JSON, браузер памяти с фильтрами по категориям и поиском.

### 4. voice
Голосовой ввод/вывод: Whisper STT (авто-загрузка моделей, транскрипция), захват аудио через cpal, Call Mode (VAD детекция, подсчёт фреймов речи/тишины, barge-in), TTS (edge-tts + remote Qwen3-TTS), overlay звонка. Включает `pc/tts_server.py` (Flask-сервер на PC).

### 5. proactive
Система проактивных сообщений: настройки (интервал, тихие часы, голос), 13+ стилей (digest, mood_check, nudge, curiosity, humor, care, etc.), сбор OS-контекста (активное приложение, URL, музыка, события, screen time), engagement tracking.

### 6. system
Системные команды ОС: выполнение shell, открытие URL, уведомления macOS, громкость, clipboard (pbcopy/pbpaste), DuckDuckGo web search, чтение/листинг файлов, API токен, запуск MLX сервера, osascript, классификация приложений.

### 7. focus
Режим фокуса (Pomodoro): таймер с блокировкой приложений/сайтов через /etc/hosts (sudo), persistent блок-лист в БД (с расписанием), UI с обратным отсчётом.

### 8. calendar
Календарь: CRUD событий, синхронизация Apple Calendar (EventKit), парсинг Google ICS, полная RRULE-экспансия (daily/weekly/monthly/yearly, BYDAY, BYMONTHDAY, COUNT, UNTIL). UI: month/week/day/list виды.

### 9. tracker
Life tracker: JSON-персистентность покупок/времени/целей/заметок, трекинг активностей (start/stop/duration), сбор screen time, summary по приложениям/сайтам, агрегированный dashboard.

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
Фреймворк для кастомизации страниц: метаданные вкладок (emoji/title/description), определения свойств (пользовательские колонки), значения свойств, конфиги видов (фильтры/сортировки/колонки), цели (per-tab), настройки приложения, интеграции, экспорт данных, generic database view (сортировка, фильтрация, inline edit).

### 14. ui
Визуальный слой: Notion Dark тема (CSS variables), система вкладок (открытие/закрытие, Cmd+W/T/1-9, localStorage), суб-сайдбар, skeleton loaders, 3563 строки CSS (все компоненты от чата до call mode), HTML-структура. Включает рендеринг tab bar и общую навигацию.

### 15. ml_devops
MLOps и DevOps: генерация 77+ обучающих примеров (all action types), Claude-дистилляция (извлечение промптов → Claude → идеальные ответы), ночной LoRA fine-tuning (3 AM, feedback-based, adapter backup), CI/CD pipeline (GitHub Actions → macOS build → release → gist update).
