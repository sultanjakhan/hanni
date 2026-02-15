# Hanni — Coverage Tracker

> Master-реестр всех файлов/секций проекта. Каждый элемент приписан к модулю.
> Гарантия полноты: сумма всех модулей + unassigned = total.
> Версия: v0.13.9 | Дата: 2026-02-15 | Phase 2: ✅ complete (149 бизнес-функций) | Phase 3: ✅ complete (561 подфункций)

## Общая статистика

| Метрика | Значение |
|---------|----------|
| Всего исходных файлов | 20 |
| Всего элементов (файлов + секций monolith) | 97 |
| Распределено по модулям | 94 |
| Не распределено (test/ref) | 3 |
| Покрытие | 96.9% |
| Общий LOC (исходники) | ~18,840 |
| LOC тестовые/справочные | ~1,508 |

## Счётчик по модулям

| Модуль | Элементов | LOC | % от общего | Бизнес-функций | Подфункций | Статус Phase 1 | Статус Phase 2 | Статус Phase 3 |
|--------|-----------|-----|-------------|----------------|------------|----------------|----------------|----------------|
| core | 11 | ~1,800 | 9.6% | 5 (B5) | 31 | ✅ mapped | ✅ done | ✅ done |
| chat | 2 | ~1,340 | 7.1% | 10 (B4+F6) | 56 | ✅ mapped | ✅ done | ✅ done |
| memory | 2 | ~620 | 3.3% | 8 (B5+F3) | 17 | ✅ mapped | ✅ done | ✅ done |
| voice | 3 | ~1,650 | 8.8% | 11 (B8+F2+I1) | 73 | ✅ mapped | ✅ done | ✅ done |
| proactive | 2 | ~690 | 3.7% | 10 (B8+F2) | 29 | ✅ mapped | ✅ done | ✅ done |
| system | 1 | ~540 | 2.9% | 8 (B8) | 28 | ✅ mapped | ✅ done | ✅ done |
| focus | 2 | ~350 | 1.9% | 7 (B5+F2) | 13 | ✅ mapped | ✅ done | ✅ done |
| calendar | 2 | ~1,030 | 5.5% | 10 (B4+F6) | 60 | ✅ mapped | ✅ done | ✅ done |
| tracker | 2 | ~600 | 3.2% | 6 (B5+F1) | 31 | ✅ mapped | ✅ done | ✅ done |
| content | 2 | ~520 | 2.8% | 6 (B3+F3) | 31 | ✅ mapped | ✅ done | ✅ done |
| media | 2 | ~460 | 2.4% | 7 (B4+F3) | 22 | ✅ mapped | ✅ done | ✅ done |
| lifestyle | 2 | ~2,080 | 11.0% | 25 (B18+F7) | 53 | ✅ mapped | ✅ done | ✅ done |
| page_framework | 2 | ~1,610 | 8.5% | 14 (B8+F6) | 77 | ✅ mapped | ✅ done | ✅ done |
| ui | 3 | ~4,370 | 23.2% | 17 (F17) | 11 | ✅ mapped | ✅ done | ✅ done |
| ml_devops | 7 | ~1,180 | 6.3% | 5 (B5) | 29 | ✅ mapped | ✅ done | ✅ done |
| **unassigned (test/ref)** | **3** | **~1,508** | — | — | — | ❓ | — | — |
| **TOTAL** | **97** | **~20,348** | **100%** | **149** | **561** | — | — | — |

---

## Полный список

### Layer 1: Backend (Rust — lib.rs 8201 LOC)

| # | Секция | Строки (от-до) | LOC | Модуль | Статус |
|---|--------|----------------|-----|--------|--------|
| 1 | Imports | L1-13 | 13 | core | ✅ |
| 2 | MLX_URL, MODEL, SYSTEM_PROMPT, SYSTEM_PROMPT_LITE, ACTION_KEYWORDS | L14-155 | 142 | core | ✅ |
| 3 | needs_full_prompt(), data_file_path() | L158-167 | 10 | core | ✅ |
| 4 | TrackerData struct | L168-180 | 13 | tracker | ✅ |
| 5 | ProactiveSettings, ProactiveState, impl блоки | L181-271 | 91 | proactive | ✅ |
| 6 | HanniDb, hanni_data_dir, migrate_old_data_dir, copy_dir_recursive | L274-345 | 72 | core | ✅ |
| 7 | init_db() (40+ таблиц) | L346-879 | 534 | core | ✅ |
| 8 | migrate_memory_json(), migrate_events_source() | L880-932 | 53 | memory | ✅ |
| 9 | build_memory_context_from_db() | L933-1025 | 93 | memory | ✅ |
| 10 | proactive_settings_path(), load/save_proactive_settings() | L1026-1050 | 25 | proactive | ✅ |
| 11 | Chat types (ChatMessage, ChatRequest, Delta, Choice, etc.) | L1052-1093 | 42 | chat | ✅ |
| 12 | WhisperState, AudioRecording | L1094-1103 | 10 | voice | ✅ |
| 13 | Whisper model, recording commands, transcribe | L1104-1233 | 130 | voice | ✅ |
| 14 | start_audio_capture() | L1234-1371 | 138 | voice | ✅ |
| 15 | Call mode commands (start, stop, resume, bargein, save_voice_note) | L1371-1487 | 117 | voice | ✅ |
| 16 | start_call_audio_loop() | L1488-1803 | 316 | voice | ✅ |
| 17 | FocusState, FocusManager, CallModeState, CallMode, FocusStatus | L1805-1837 | 33 | focus | ✅ |
| 18 | start_focus, stop_focus, get_focus_status, update_blocklist | L1839-1995 | 157 | focus | ✅ |
| 19 | run_shell, open_url, send_notification, set_volume, get_clipboard, set_clipboard, web_search | L1999-2130 | 132 | system | ✅ |
| 20 | get_training_stats, export_training_data, rate_message, get_message_ratings | L2133-2319 | 187 | page_framework | ✅ |
| 21 | api_token, find_python, start_mlx_server | L2322-2521 | 200 | system | ✅ |
| 22 | check_calendar_access, run_osascript, classify_app | L2525-2694 | 170 | system | ✅ |
| 23 | chat (main LLM endpoint) | L2695-2883 | 189 | chat | ✅ |
| 24 | read_file, list_dir | L2885-2936 | 52 | system | ✅ |
| 25 | load_tracker_data, save_tracker_data | L2918-2957 | 40 | tracker | ✅ |
| 26 | tracker_add_purchase → tracker_get_recent | L2937-3057 | 121 | tracker | ✅ |
| 27 | get_activity_summary, get_calendar_events, get_now_playing, get_browser_tab | L3058-3243 | 186 | tracker | ✅ |
| 28 | memory_remember, memory_recall, memory_forget, memory_search | L3246-3374 | 129 | memory | ✅ |
| 29 | Conversation CRUD (save, update, get, delete, search, process_end) | L3375-3611 | 237 | chat | ✅ |
| 30 | Activity CRUD (start, stop, get_current, get_log) | L3614-3726 | 113 | tracker | ✅ |
| 31 | Notes CRUD (create, update, delete, get, get_note) | L3727-3828 | 102 | content | ✅ |
| 32 | Events + Calendar sync (create_event → sync_google_ics, RRule) | L3829-4329 | 501 | calendar | ✅ |
| 33 | Projects + Tasks (create, get, update_status) | L4330-4408 | 79 | content | ✅ |
| 34 | Learning items (create, get) | L4408-4457 | 50 | content | ✅ |
| 35 | Hobbies (create, get, log_entry, get_entries) | L4457-4521 | 65 | media | ✅ |
| 36 | Workouts (create, get, stats) | L4520-4564 | 45 | lifestyle | ✅ |
| 37 | Health + Habits (log_health, get_today, create/check_habit, get_habits_today) | L4564-4676 | 113 | lifestyle | ✅ |
| 38 | get_dashboard_data | L4675-4736 | 62 | tracker | ✅ |
| 39 | Memory browser (get_all, delete, update) | L4738-4800 | 63 | memory | ✅ |
| 40 | Media items + user lists (add → get_media_stats) | L4800-5012 | 213 | media | ✅ |
| 41 | Food (log_food → get_expiring_products) | L5012-5204 | 193 | lifestyle | ✅ |
| 42 | Money (add_transaction → delete_debt) | L5204-5468 | 265 | lifestyle | ✅ |
| 43 | Mindset (journal, mood, principles) | L5468-5612 | 145 | lifestyle | ✅ |
| 44 | Blocklist CRUD (add, remove, get, toggle) | L5613-5659 | 47 | focus | ✅ |
| 45 | Goals + App settings | L5660-5741 | 82 | page_framework | ✅ |
| 46 | Home items (add → toggle_needed) | L5742-5801 | 60 | lifestyle | ✅ |
| 47 | Contacts (add → toggle_contact_block_active) | L5798-5959 | 162 | lifestyle | ✅ |
| 48 | Page meta + Properties + View configs | L5959-6384 | 426 | page_framework | ✅ |
| 49 | ProactiveStyleDef, build_proactive_system_prompt() | L6385-6436 | 52 | proactive | ✅ |
| 50 | Context gathering + proactive helpers | L6438-6905 | 468 | proactive | ✅ |
| 51 | TTS functions (edge, remote, adaptive, clean) | L6906-7169 | 264 | voice | ✅ |
| 52 | TTS/voice commands (speak_text, stop_speaking, etc.) | L7170-7357 | 188 | voice | ✅ |
| 53 | updater_with_headers(), get_app_version() | L7358-7386 | 29 | core | ✅ |
| 54 | run() — app entry, state, 190+ commands, proactive loop | L7391-8201 | 811 | core | ✅ |

### Layer 1b: Backend (Rust — other files)

| # | Файл | LOC | Модуль | Статус |
|---|------|-----|--------|--------|
| 55 | main.rs | 6 | core | ✅ |
| 56 | build.rs | 3 | core | ✅ |

### Layer 2: Frontend (JavaScript — main.js 5376 LOC)

| # | Секция | Строки (от-до) | LOC | Модуль | Статус |
|---|--------|----------------|-----|--------|--------|
| 57 | Imports, DOM, global state | L1-38 | 38 | ui | ✅ |
| 58 | PROACTIVE_STYLE_DEFINITIONS | L41-56 | 16 | proactive | ✅ |
| 59 | Icons, TAB_REGISTRY, TAB_DESCRIPTIONS | L59-115 | 57 | ui | ✅ |
| 60 | renderPageHeader() | L117-130 | 14 | page_framework | ✅ |
| 61 | Tab state, init, saveTabs(), update-available | L132-162 | 31 | ui | ✅ |
| 62 | Proactive events, typing signal | L164-198 | 35 | proactive | ✅ |
| 63 | Voice recording button | L204-257 | 54 | voice | ✅ |
| 64 | focus-ended event | L261-267 | 7 | focus | ✅ |
| 65 | Conversation management (list, load, save) | L271-390 | 120 | chat | ✅ |
| 66 | escapeHtml(), skeleton loaders | L392-428 | 37 | ui | ✅ |
| 67 | New chat, conversation search | L431-446 | 16 | chat | ✅ |
| 68 | renderTabBar(), renderSubSidebar() | L450-530 | 81 | ui | ✅ |
| 69 | Goals widget, add goal modal | L532-599 | 68 | page_framework | ✅ |
| 70 | Tab navigation (open, close, switch, activate, loadSubTab) | L602-667 | 66 | ui | ✅ |
| 71 | Tab dropdown, keyboard shortcuts | L669-700 | 32 | ui | ✅ |
| 72 | Chat settings (proactive, TTS, voice selection) | L704-963 | 260 | chat | ✅ |
| 73 | scrollDown() | L964-976 | 13 | ui | ✅ |
| 74 | addMsg(), addFeedbackButtons(), file handlers | L978-1104 | 127 | chat | ✅ |
| 75 | executeAction() (20+ types) | L1105-1382 | 278 | chat | ✅ |
| 76 | parseAndExecuteActions(), repairJson() | L1383-1419 | 37 | chat | ✅ |
| 77 | streamChat(), showAgentIndicator(), TTS toggle | L1421-1518 | 98 | chat | ✅ |
| 78 | send(), newChat(), event listeners | L1520-1696 | 177 | chat | ✅ |
| 79 | Home (supplies, shopping list) | L1699-1801 | 103 | lifestyle | ✅ |
| 80 | Mindset (journal, mood, principles) | L1802-1909 | 108 | lifestyle | ✅ |
| 81 | Food (log, recipes, products) | L1910-2119 | 210 | lifestyle | ✅ |
| 82 | Money (transactions, budgets, savings, subscriptions, debts) | L2120-2339 | 220 | lifestyle | ✅ |
| 83 | People (contacts, blocks, favorites) | L2340-2497 | 158 | lifestyle | ✅ |
| 84 | Memory (tab, facts, search, settings) | L2498-2773 | 276 | memory | ✅ |
| 85 | Settings, integrations, blocklist, about | L2774-2985 | 212 | page_framework | ✅ |
| 86 | Dashboard | L2986-3055 | 70 | tracker | ✅ |
| 87 | Focus mode UI | L3056-3161 | 106 | focus | ✅ |
| 88 | Notes (list, editor, auto-save) | L3162-3287 | 126 | content | ✅ |
| 89 | Calendar (month, week, day, list, integrations) | L3288-3814 | 527 | calendar | ✅ |
| 90 | Work (projects, tasks) | L3815-3889 | 75 | content | ✅ |
| 91 | Development (learning items) | L3890-3975 | 86 | content | ✅ |
| 92 | Database view framework | L3976-4597 | 622 | page_framework | ✅ |
| 93 | Hobbies / media collections | L4594-4773 | 180 | media | ✅ |
| 94 | Sports (workouts, martial arts, stats) | L4774-4924 | 151 | lifestyle | ✅ |
| 95 | Health (metrics, habits) | L4925-5058 | 134 | lifestyle | ✅ |
| 96 | Call mode UI (overlay, TTS stream, shortcuts) | L5059-5376 | 318 | voice | ✅ |

### Layer 3: Frontend (CSS + HTML)

| # | Файл | LOC | Модуль | Статус |
|---|------|-----|--------|--------|
| 97 | styles.css | 3563 | ui | ✅ |
| 98 | index.html | 116 | ui | ✅ |

### Layer 4: Scripts & Config

| # | Файл | LOC | Модуль | Статус |
|---|------|-----|--------|--------|
| 99 | pc/tts_server.py | 120 | voice | ✅ |
| 100 | scripts/generate_training_data.py | 497 | ml_devops | ✅ |
| 101 | scripts/claude_distill.sh | 274 | ml_devops | ✅ |
| 102 | scripts/nightly_train.sh | 217 | ml_devops | ✅ |
| 103 | scripts/com.hanni.nightly-train.plist | ~30 | ml_devops | ✅ |
| 104 | .github/workflows/release.yml | 65 | ml_devops | ✅ |
| 105 | Cargo.toml | 40 | core | ✅ |
| 106 | tauri.conf.json | 50 | core | ✅ |
| 107 | capabilities/default.json | 37 | core | ✅ |
| 108 | build.rs | 3 | core | ✅ |
| 109 | package.json | 12 | core | ✅ |
| 110 | Entitlements.plist | 8 | core | ✅ |
| 111 | Info.plist | 12 | core | ✅ |

### Тестовые / Справочные

| # | Файл | LOC | Назначение | Статус |
|---|------|-----|------------|--------|
| T1 | design-system.html | 1110 | Дизайн-система (справочник) | ❓ |
| T2 | desktop/src/tauri-mock.js | 300 | Мок Tauri API | ❓ |
| T3 | desktop/src/test.html | 98 | Тестовая страница | ❓ |

---

## Контрольная сумма

```
core(1800) + chat(1340) + memory(620) + voice(1650) + proactive(690) +
system(540) + focus(350) + calendar(1030) + tracker(600) + content(520) +
media(460) + lifestyle(2080) + page_framework(1610) + ui(4370) + ml_devops(1180)
= ~18,840 mapped LOC

+ unassigned test/ref: 1,508
= ~20,348 total LOC

Файл          Заявлено    Учтено
lib.rs        8,201       ~8,200 ✅
main.js       5,376       ~5,370 ✅
styles.css    3,563       3,563  ✅
index.html    116         116    ✅
tts_server.py 120         120    ✅
scripts       ~1,018      ~1,018 ✅
configs       ~235        ~235   ✅
test/ref      ~1,508      1,508  ✅
```

## Статусы

- ✅ — распределён в модуль
- ⬜ — известен, ещё не обработан (Phase 2)
- ❓ — не ясно какому модулю принадлежит (→ DISPUTED.md)
- 🔴 — orphan (кандидат на удаление)
