# Hanni — Coverage Tracker

> Master-реестр всех файлов проекта. Каждый элемент приписан к модулю.
> Гарантия полноты: сумма всех модулей + unassigned = total.
> Версия: v0.25.0 | Дата: 2026-03-03 | Phase 2: done (149 бизнес-функций) | Phase 3: done (561 подфункций)
> Кодовая база модуляризирована: 13 Rust + 12 JS + 7 CSS модулей.

## Общая статистика

| Метрика | Значение |
|---------|----------|
| Всего исходных файлов | 36 (14 .rs + 12 .js + 8 .css + 1 .html + scripts/config) |
| Распределено по модулям | 36 |
| Не распределено (test/ref) | 3 |
| Покрытие | ~100% |
| Общий LOC (исходники) | ~26,569 |
| LOC тестовые/справочные | ~1,721 |

## Счётчик по модулям

| Модуль | Файлы | LOC | % от общего | Бизнес-функций | Подфункций | Статус |
|--------|-------|-----|-------------|----------------|------------|--------|
| core | types.rs, prompts.rs, db.rs, lib.rs, main.rs, state.js (partial), main.js, configs | ~2,800 | 10.5% | 5 (B5) | 31 | done |
| chat | chat.rs, chat.js, conversations.js, actions.js | ~2,880 | 10.8% | 10 (B4+F6) | 56 | done |
| memory | memory.rs, tab-data.js (partial) | ~1,150 | 4.3% | 8 (B5+F3) | 17 | done |
| voice | voice.rs, voice.js, tts_server.py | ~2,110 | 7.9% | 11 (B8+F2+I1) | 73 | done |
| proactive | proactive.rs | ~1,035 | 3.9% | 10 (B8+F2) | 29 | done |
| system | macos.rs, commands_meta.rs (partial) | ~620 | 2.3% | 8 (B8) | 28 | done |
| focus | commands_data.rs (partial), tab-focus.js | ~560 | 2.1% | 7 (B5+F2) | 13 | done |
| calendar | calendar.rs, tab-calendar.js | ~1,120 | 4.2% | 10 (B4+F6) | 60 | done |
| tracker | commands_data.rs (partial), tab-data.js (partial) | ~600 | 2.3% | 6 (B5+F1) | 31 | done |
| content | notes.rs, tab-notes.js | ~1,890 | 7.1% | 6 (B3+F3) | 31 | done |
| media | commands_data.rs (partial), tab-data.js (partial) | ~460 | 1.7% | 7 (B4+F3) | 22 | done |
| lifestyle | commands_data.rs, commands_meta.rs (partial), tab-data.js (partial) | ~3,790 | 14.3% | 25 (B18+F7) | 53 | done |
| page_framework | commands_meta.rs (partial), tab-notes.js (partial), tabs.js (partial) | ~1,700 | 6.4% | 14 (B8+F6) | 77 | done |
| ui | state.js, utils.js, tabs.js, css/*.css, index.html | ~7,300 | 27.5% | 17 (F17) | 11 | done |
| ml_devops | scripts/*, finetune*, hanni-mcp/*, release.yml | ~1,800 | 6.8% | 5 (B5) | 29 | done |
| **unassigned (test/ref)** | **3 files** | **~1,721** | — | — | — | n/a |
| **TOTAL** | **36+** | **~26,569** | **100%** | **149** | **561** | — |

---

## Полный список

### Layer 1: Backend (Rust — 13 модулей, 11,831 LOC)

| # | Файл | Описание | LOC | Модуль | Статус |
|---|------|----------|-----|--------|--------|
| 1 | types.rs | Все struct/enum, constants, static atomics | 423 | core | done |
| 2 | prompts.rs | System prompts, tool definitions, ChatModeConfig helpers | 700 | core | done |
| 3 | db.rs | init_db (43 таблицы), migrations, data dir helpers | 763 | core | done |
| 4 | chat.rs | chat command, streaming, quality_check, file/tracker commands | 677 | chat | done |
| 5 | memory.rs | Semantic memory, embeddings, reranking, conversation CRUD | 1,153 | memory | done |
| 6 | voice.rs | Whisper STT, CPAL, call mode, TTS, wake word | 1,232 | voice | done |
| 7 | proactive.rs | Proactive messages, OS context, TTS pipeline | 1,035 | proactive | done |
| 8 | macos.rs | Shell, macOS integration, MLX server, idle detection | 617 | system | done |
| 9 | calendar.rs | Events CRUD, Apple/Google sync, RRULE | 508 | calendar | done |
| 10 | notes.rs | Notes CRUD, custom pages CRUD | 357 | content | done |
| 11 | commands_data.rs | Activities, media, sports, health, food, money, mindset, blocklist | 1,945 | lifestyle+tracker+media+focus | done |
| 12 | commands_meta.rs | Properties, views, integrations, contacts, home, goals, updater, API | 1,502 | page_framework+system+lifestyle | done |
| 13 | lib.rs | Orchestrator: mod declarations, run(), generate_handler | 913 | core | done |
| 14 | main.rs | fn main() → tauri entry | 6 | core | done |

### Layer 2: Frontend (JavaScript — 12 модулей, 8,975 LOC)

| # | Файл | Описание | LOC | Модуль | Статус |
|---|------|----------|-----|--------|--------|
| 15 | js/state.js | Shared state S, constants, DOM refs, tabLoaders registry | 301 | core+ui | done |
| 16 | js/utils.js | Markdown, escapeHtml, skeletons, page header | 279 | ui+page_framework | done |
| 17 | js/tabs.js | Tab bar, navigation, router, goals, keyboard shortcuts | 572 | ui | done |
| 18 | js/conversations.js | Conversation list, load, save, search | 164 | chat | done |
| 19 | js/chat.js | Send, addMsg, streaming, settings, proactive listeners | 1,574 | chat | done |
| 20 | js/actions.js | parseAndExecuteActions, 40+ action handlers | 469 | chat | done |
| 21 | js/voice.js | Recording, call mode UI, wake word SSE | 876 | voice | done |
| 22 | js/tab-calendar.js | Calendar views (month/week/day/list/integrations) | 612 | calendar | done |
| 23 | js/tab-focus.js | Focus timer, widget, pomodoro | 562 | focus | done |
| 24 | js/tab-notes.js | Notes views, database view, kanban, property/filter system | 1,529 | content+page_framework | done |
| 25 | js/tab-data.js | Home, mindset, food, money, people, memory, work, dev, hobbies, sports, health | 1,857 | lifestyle+media+tracker+memory | done |
| 26 | main.js | Entry point: imports, tabLoaders registration, init | 180 | core | done |

### Layer 3: Frontend (CSS — 7 модулей + barrel, 5,763 LOC)

| # | Файл | Описание | LOC | Модуль | Статус |
|---|------|----------|-----|--------|--------|
| 27 | styles.css | Barrel: @import statements | 8 | ui | done |
| 28 | css/base.css | Variables, reset, base components | 532 | ui | done |
| 29 | css/layout.css | Tab bar, sidebar, content area layout | 455 | ui | done |
| 30 | css/chat.css | Chat interface styles | 1,025 | ui | done |
| 31 | css/settings.css | Settings pages styles | 714 | ui | done |
| 32 | css/tabs-main.css | Focus, notes, kanban, database view | 1,545 | ui | done |
| 33 | css/tabs-data.css | Calendar, dashboard, lifestyle tabs | 953 | ui | done |
| 34 | css/call.css | Call mode overlay, focus widget | 531 | ui | done |

### Layer 3b: Frontend (HTML)

| # | Файл | LOC | Модуль | Статус |
|---|------|-----|--------|--------|
| 35 | index.html | 151 | ui | done |

### Layer 4: Scripts & Config

| # | Файл | LOC | Модуль | Статус |
|---|------|-----|--------|--------|
| 36 | pc/tts_server.py | 120 | voice | done |
| 37 | scripts/generate_training_data.py | 497 | ml_devops | done |
| 38 | scripts/claude_distill.sh | 274 | ml_devops | done |
| 39 | scripts/nightly_train.sh | 217 | ml_devops | done |
| 40 | scripts/com.hanni.nightly-train.plist | ~30 | ml_devops | done |
| 41 | .github/workflows/release.yml | 65 | ml_devops | done |
| 42 | Cargo.toml | 40 | core | done |
| 43 | tauri.conf.json | 50 | core | done |
| 44 | capabilities/default.json | 37 | core | done |
| 45 | build.rs | 3 | core | done |
| 46 | package.json | 12 | core | done |
| 47 | Entitlements.plist | 8 | core | done |
| 48 | Info.plist | 12 | core | done |
| 49 | desktop/finetune.py | 143 | ml_devops | done |
| 50 | desktop/finetune_data/prepare_data.py | 309 | ml_devops | done |
| 51 | desktop/finetune_data/synthetic_examples.jsonl | 73 | ml_devops | done |
| 52 | desktop/hanni-mcp/server.py | 321 | ml_devops | done |
| 53 | desktop/dev-server.py | 4 | ml_devops | done |

### Тестовые / Справочные

| # | Файл | LOC | Назначение | Статус |
|---|------|-----|------------|--------|
| T1 | design-system.html | 1,110 | Дизайн-система (справочник) | n/a |
| T2 | desktop/src/tauri-mock.js | 479 | Мок Tauri API | n/a |
| T3 | desktop/src/test.html | 132 | Тестовая страница | n/a |

---

## Контрольная сумма

```
v0.25.0 LOC (после модуляризации):

Backend (Rust):     11,831 LOC (14 файлов)
Frontend (JS):       8,975 LOC (12 файлов)
Frontend (CSS):      5,763 LOC (8 файлов)
Frontend (HTML):       151 LOC (1 файл)
Voice Server:          778 LOC (1 файл)
Scripts:            ~1,083 LOC (5 файлов)
Config:               ~212 LOC (7 файлов)
Test/ref:           ~1,721 LOC (3 файла)
─────────────────────────────
Total:             ~30,514 LOC (51 файлов)
Source only:       ~26,569 LOC (36 файлов)

История:
v0.13.9: ~20,348 LOC (2 монолитных файла)
v0.19.0: ~23,420 LOC (2 монолитных файла)
v0.25.0: ~26,569 LOC (36 модулей)
```

## Статусы

- done — распределён в модуль, все фазы завершены
- n/a — тестовый/справочный файл, не в модулях
