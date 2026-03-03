# Hanni — Architecture Wiki

> Навигационная карта всей архитектурной документации.
> 3 уровня декомпозиции: **Проект → Модули → Функции → Подфункции**
> Версия: v0.25.0 | 2026-03-03
> Кодовая база модуляризирована: 13 Rust + 12 JS + 7 CSS модулей.

---

## Быстрая навигация

| Документ | Что содержит | Уровень |
|----------|-------------|---------|
| [MODULE_MAP.md](../MODULE_MAP.md) | Файл → модуль маппинг, описания модулей | Phase 1 |
| [COVERAGE.md](COVERAGE.md) | Master-реестр, счётчики, статусы всех фаз | Phase 1-3 |
| [DISPUTED.md](DISPUTED.md) | Спорные элементы, кандидаты на рефакторинг | Phase 1 |
| Модуль wiki (`[module].md`) | Описание, зависимости, архитектура модуля | Phase 2 |
| Функции (`functions.md`) | Реестр бизнес-функций модуля | Phase 2 |
| Подфункции (`subfunctions.md`) | Атомарные шаги, data flow, проблемные зоны | Phase 3 |

---

## Статистика проекта

```
Проект Hanni v0.25.0
├── 15 логических модулей
├── 149 бизнес-функций (91 Medium/Complex + 58 Simple)
├── 561 подфункций (декомпозированы из 91 функций)
├── ~26,569 LOC исходного кода
│   ├── Rust:  11,831 LOC (14 файлов: 13 модулей + main.rs)
│   ├── JS:     8,975 LOC (12 файлов: 11 модулей + main.js)
│   └── CSS:    5,763 LOC (8 файлов: 7 модулей + barrel)
├── ~78 проблемных зон обнаружено
└── 127+ файлов документации
```

---

## Файловая структура после модуляризации

```
desktop/src-tauri/src/
├── lib.rs              (913)   — orchestrator: mod, run(), generate_handler
├── main.rs             (6)     — tauri entry
├── types.rs            (423)   — все struct/enum/constants
├── prompts.rs          (700)   — system prompts, tool definitions
├── db.rs               (763)   — init_db, migrations
├── chat.rs             (677)   — LLM streaming, quality_check
├── memory.rs           (1153)  — semantic memory, conversations
├── voice.rs            (1232)  — Whisper, CPAL, call mode, TTS
├── proactive.rs        (1035)  — proactive messages, OS context
├── macos.rs            (617)   — shell, macOS integration
├── calendar.rs         (508)   — events, Apple/Google sync, RRULE
├── notes.rs            (357)   — notes/pages CRUD
├── commands_data.rs    (1945)  — activities, media, lifestyle data
└── commands_meta.rs    (1502)  — properties, views, integrations

desktop/src/
├── main.js             (180)   — entry: imports, tabLoaders, init
├── index.html          (151)   — HTML structure
├── styles.css          (8)     — CSS barrel (@import)
├── js/
│   ├── state.js        (301)   — shared state S, constants, DOM refs
│   ├── utils.js        (279)   — markdown, escapeHtml, skeletons
│   ├── tabs.js         (572)   — tab navigation, router, goals
│   ├── conversations.js(164)   — conversation list/load/save
│   ├── chat.js         (1574)  — send, addMsg, streaming, settings
│   ├── actions.js      (469)   — parseAndExecuteActions, 40+ types
│   ├── voice.js        (876)   — recording, call mode, wake word
│   ├── tab-calendar.js (612)   — calendar views
│   ├── tab-focus.js    (562)   — focus timer, widget
│   ├── tab-notes.js    (1529)  — notes, database view, kanban
│   └── tab-data.js     (1857)  — home, mindset, food, money, etc.
└── css/
    ├── base.css        (532)   — variables, reset, components
    ├── layout.css      (455)   — tab bar, sidebar, content area
    ├── chat.css        (1025)  — chat interface
    ├── settings.css    (714)   — settings pages
    ├── tabs-main.css   (1545)  — focus, notes, kanban, db view
    ├── tabs-data.css   (953)   — calendar, dashboard, lifestyle
    └── call.css        (531)   — call mode, focus widget
```

---

## Карта модулей

### Ядро (Core Layer)

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 1 | **core** | types.rs, prompts.rs, db.rs, lib.rs | ~2,800 | 5 | 31 | [wiki](core/core.md) | [functions](core/functions.md) | [B1](core/functions/B1-system-prompt/subfunctions.md) / [B2](core/functions/B2-init-db/subfunctions.md) / [B5](core/functions/B5-run/subfunctions.md) |
| 6 | **system** | macos.rs, commands_meta.rs (partial) | ~620 | 8 | 28 | [wiki](system/system.md) | [functions](system/functions.md) | [B4](system/functions/B4-web-search/subfunctions.md) / [B5](system/functions/B5-api-server/subfunctions.md) / [B6](system/functions/B6-mlx-server/subfunctions.md) / [B7](system/functions/B7-macos-integration/subfunctions.md) |

### AI & Chat

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 2 | **chat** | chat.rs, chat.js, conversations.js, actions.js | ~2,880 | 10 | 56 | [wiki](chat/chat.md) | [functions](chat/functions.md) | [B2](chat/functions/B2-streaming-chat/subfunctions.md) / [B3](chat/functions/B3-conversation-crud/subfunctions.md) / [B4](chat/functions/B4-fact-extraction/subfunctions.md) / [F1](chat/functions/F1-conversation-list/subfunctions.md) / [F2](chat/functions/F2-load-conversation/subfunctions.md) / [F3](chat/functions/F3-execute-action/subfunctions.md) / [F4](chat/functions/F4-send-message/subfunctions.md) / [F5](chat/functions/F5-stream-chat/subfunctions.md) / [F6](chat/functions/F6-add-message-ui/subfunctions.md) |
| 3 | **memory** | memory.rs, tab-data.js (partial) | ~1,150 | 8 | 17 | [wiki](memory/memory.md) | [functions](memory/functions.md) | [B2](memory/functions/B2-memory-context/subfunctions.md) / [B4](memory/functions/B4-memory-search/subfunctions.md) / [F1](memory/functions/F1-memory-tab/subfunctions.md) / [F2](memory/functions/F2-memory-list/subfunctions.md) |
| 5 | **proactive** | proactive.rs | ~1,035 | 10 | 29 | [wiki](proactive/proactive.md) | [functions](proactive/functions.md) | [B1](proactive/functions/B1-proactive-types/subfunctions.md) / [B3](proactive/functions/B3-proactive-prompt/subfunctions.md) / [B4](proactive/functions/B4-os-context/subfunctions.md) / [B5](proactive/functions/B5-morning-digest/subfunctions.md) / [B8](proactive/functions/B8-proactive-generation/subfunctions.md) |

### Voice & Focus

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 4 | **voice** | voice.rs, voice.js | ~2,110 | 11 | 73 | [wiki](voice/voice.md) | [functions](voice/functions.md) | [B1](voice/functions/B1-whisper-model/subfunctions.md) / [B2](voice/functions/B2-recording/subfunctions.md) / [B3](voice/functions/B3-audio-capture/subfunctions.md) / [B4](voice/functions/B4-call-mode-control/subfunctions.md) / [B5](voice/functions/B5-call-audio-loop/subfunctions.md) / [B6](voice/functions/B6-tts-prepare/subfunctions.md) / [B7](voice/functions/B7-tts-synthesis/subfunctions.md) / [B8](voice/functions/B8-tts-commands/subfunctions.md) / [F1](voice/functions/F1-voice-button/subfunctions.md) / [F2](voice/functions/F2-call-mode-ui/subfunctions.md) / [I1](voice/functions/I1-remote-tts/subfunctions.md) |
| 7 | **focus** | commands_data.rs (partial), tab-focus.js | ~560 | 7 | 13 | [wiki](focus/focus.md) | [functions](focus/functions.md) | [B1](focus/functions/B1-start-focus/subfunctions.md) / [F1](focus/functions/F1-focus-ui/subfunctions.md) |

### Data & Content

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 8 | **calendar** | calendar.rs, tab-calendar.js | ~1,120 | 10 | 60 | [wiki](calendar/calendar.md) | [functions](calendar/functions.md) | [B2](calendar/functions/B2-apple-calendar-sync/subfunctions.md) / [B3](calendar/functions/B3-rrule-parser/subfunctions.md) / [B4](calendar/functions/B4-google-calendar-sync/subfunctions.md) / [F1](calendar/functions/F1-month-calendar/subfunctions.md) / [F2](calendar/functions/F2-week-calendar/subfunctions.md) / [F3](calendar/functions/F3-day-calendar/subfunctions.md) / [F4](calendar/functions/F4-event-list/subfunctions.md) / [F5](calendar/functions/F5-calendar-integrations/subfunctions.md) / [F6](calendar/functions/F6-add-event-modal/subfunctions.md) |
| 9 | **tracker** | commands_data.rs (partial), tab-data.js (partial) | ~600 | 6 | 31 | [wiki](tracker/tracker.md) | [functions](tracker/functions.md) | [B1](tracker/functions/B1-tracker-crud/subfunctions.md) / [B2](tracker/functions/B2-screen-time/subfunctions.md) / [B3](tracker/functions/B3-macos-context/subfunctions.md) / [B4](tracker/functions/B4-activities/subfunctions.md) / [B5](tracker/functions/B5-dashboard-data/subfunctions.md) / [F1](tracker/functions/F1-dashboard-ui/subfunctions.md) |
| 10 | **content** | notes.rs, tab-notes.js | ~1,890 | 6 | 31 | [wiki](content/content.md) | [functions](content/functions.md) | [B1](content/functions/B1-notes-crud/subfunctions.md) / [B2](content/functions/B2-projects-tasks/subfunctions.md) / [F1](content/functions/F1-notes-ui/subfunctions.md) / [F2](content/functions/F2-projects-ui/subfunctions.md) / [F3](content/functions/F3-learning-ui/subfunctions.md) |
| 11 | **media** | commands_data.rs (partial), tab-data.js (partial) | ~460 | 7 | 22 | [wiki](media/media.md) | [functions](media/functions.md) | [B2](media/functions/B2-media-crud/subfunctions.md) / [B3](media/functions/B3-user-lists/subfunctions.md) / [F2](media/functions/F2-media-list/subfunctions.md) / [F3](media/functions/F3-media-modals/subfunctions.md) |

### Lifestyle

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 12 | **lifestyle** | commands_data.rs, commands_meta.rs (partial), tab-data.js (partial) | ~3,790 | 25 | 53 | [wiki](lifestyle/lifestyle.md) | [functions](lifestyle/functions.md) | [B3](lifestyle/functions/B3-habits/subfunctions.md) / [B6](lifestyle/functions/B6-products/subfunctions.md) / [B7](lifestyle/functions/B7-transactions/subfunctions.md) / [B12](lifestyle/functions/B12-journal/subfunctions.md) / [B17](lifestyle/functions/B17-contacts/subfunctions.md) / [F1](lifestyle/functions/F1-home/subfunctions.md) / [F2](lifestyle/functions/F2-mindset/subfunctions.md) / [F3](lifestyle/functions/F3-food/subfunctions.md) / [F4](lifestyle/functions/F4-money/subfunctions.md) / [F5](lifestyle/functions/F5-contacts/subfunctions.md) / [F6](lifestyle/functions/F6-sports/subfunctions.md) / [F7](lifestyle/functions/F7-health/subfunctions.md) |

### Framework & UI

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 13 | **page_framework** | commands_meta.rs (partial), tab-notes.js (partial), tabs.js (partial) | ~1,700 | 14 | 77 | [wiki](page_framework/page_framework.md) | [functions](page_framework/functions.md) | [B1](page_framework/functions/B1-training-stats/subfunctions.md) / [B6](page_framework/functions/B6-properties/subfunctions.md) / [B7](page_framework/functions/B7-view-configs/subfunctions.md) / [B8](page_framework/functions/B8-integrations/subfunctions.md) / [F1](page_framework/functions/F1-goals-widget/subfunctions.md) / [F2](page_framework/functions/F2-chat-settings/subfunctions.md) / [F3](page_framework/functions/F3-settings-ui/subfunctions.md) / [F4](page_framework/functions/F4-database-view/subfunctions.md) / [F5](page_framework/functions/F5-property-ui/subfunctions.md) / [F6](page_framework/functions/F6-filters/subfunctions.md) |
| 14 | **ui** | state.js, utils.js, tabs.js, css/*.css, index.html | ~7,300 | 17 | 11 | [wiki](ui/ui.md) | [functions](ui/functions.md) | [F4](ui/functions/F4-tab-bar/subfunctions.md) / [F5](ui/functions/F5-sub-sidebar/subfunctions.md) / [F7](ui/functions/F7-subtab-loading/subfunctions.md) |

### ML & DevOps

| # | Модуль | Файлы | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-------|-----|---------|------------|------|---------|------------|
| 15 | **ml_devops** | scripts/*, finetune.py, hanni-mcp/*, release.yml | ~1,800 | 5 | 29 | [wiki](ml_devops/ml_devops.md) | [functions](ml_devops/functions.md) | [B1](ml_devops/functions/B1-training-data/subfunctions.md) / [B2](ml_devops/functions/B2-claude-distill/subfunctions.md) / [B3](ml_devops/functions/B3-nightly-train/subfunctions.md) / [B5](ml_devops/functions/B5-release-pipeline/subfunctions.md) |

---

## Граф зависимостей модулей

```
                    ┌──────────┐
                    │   core   │
                    │types,db, │
                    │prompts   │
                    └────┬─────┘
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
      ┌─────────┐  ┌──────────┐  ┌──────────┐
      │  system  │  │   chat   │  │    ui    │
      │(macos.rs)│  │(chat.rs) │  │(tabs,css)│
      └────┬────┘  └────┬─────┘  └────┬─────┘
           │             │             │
     ┌─────┤       ┌─────┼─────┐       │
     ▼     ▼       ▼     ▼     ▼       ▼
  ┌──────┐ │  ┌────────┐ │ ┌────────┐ ┌──────────────┐
  │focus │ │  │ memory │ │ │proactiv│ │page_framework│
  └──────┘ │  │(.rs)   │ │ │(.rs)   │ │(views,props) │
           │  └────────┘ │ └────────┘ └──────┬───────┘
           ▼             ▼                   │
      ┌─────────┐  ┌──────────┐             │
      │ tracker │  │  voice   │             ▼
      │         │  │(.rs,.js) │     ┌───────────────┐
      └─────────┘  └──────────┘     │ content/media │
                                    │  /lifestyle   │
                                    │  /calendar    │
                                    └───────────────┘
```

**Направление стрелок:** модуль зависит от →

- **core** — ни от кого, все зависят от core (types.rs, db.rs)
- **chat** — от core, memory (контекст), system (shell actions)
- **voice** — от chat (транскрипт → чат), system (TTS процессы)
- **proactive** — от chat (LLM), memory (контекст), tracker (screen time)
- **page_framework** — от core (DB), используется lifestyle/media/content
- **ui** — от core, page_framework (tab система)

---

## Топ-10 проблемных зон

| # | Модуль | Файл | Проблема | Приоритет |
|---|--------|------|---------|-----------|
| 1 | core | lib.rs:run() | ~200 LOC проактивный loop в одном async блоке | High |
| 2 | core | lib.rs | run() ~900 LOC — самая большая функция (но уже разбита vs old 811) | Medium |
| 3 | core | db.rs:init_db() | ~750 LOC в одном execute_batch, нет миграций | Medium |
| 4 | chat | actions.js | 40+ action types в монолитном switch | Medium |
| 5 | voice | voice.rs | Дублирование аудио инициализации между capture и call loop | High |
| 6 | voice | voice.rs | Async/sync варианты TTS — почти идентичный код | Medium |
| 7 | lifestyle | commands_data.rs | 1,945 LOC — всё ещё крупный файл | Medium |
| 8 | chat | memory.rs | Хрупкий JSON парсинг через find('{') / rfind('}') | Medium |
| 9 | page_framework | tab-notes.js | startInlineEdit 98 LOC, 6 типов редакторов в одной fn | Medium |
| 10 | ml_devops | scripts | Python inline в bash heredoc — антипаттерн | Medium |

---

## Структура файлов документации

```
docs/
├── MODULE_MAP.md                                    # Phase 1: файл → модуль
└── architecture/
    ├── WIKI.md                                      # ← ВЫ ЗДЕСЬ
    ├── COVERAGE.md                                  # Трекер покрытия Phase 1-3
    ├── DISPUTED.md                                  # Спорные элементы
    │
    ├── core/
    │   ├── core.md                                  # Wiki модуля
    │   ├── functions.md                             # Реестр функций
    │   └── functions/
    │       ├── B1-system-prompt/subfunctions.md
    │       ├── B2-init-db/subfunctions.md
    │       └── B5-run/subfunctions.md
    │
    ├── chat/
    │   ├── chat.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B2-streaming-chat/subfunctions.md
    │       ├── B3-conversation-crud/subfunctions.md
    │       ├── B4-fact-extraction/subfunctions.md
    │       ├── F1-conversation-list/subfunctions.md
    │       ├── F2-load-conversation/subfunctions.md
    │       ├── F3-execute-action/subfunctions.md
    │       ├── F4-send-message/subfunctions.md
    │       ├── F5-stream-chat/subfunctions.md
    │       └── F6-add-message-ui/subfunctions.md
    │
    ├── memory/
    │   ├── memory.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B2-memory-context/subfunctions.md
    │       ├── B4-memory-search/subfunctions.md
    │       ├── F1-memory-tab/subfunctions.md
    │       └── F2-memory-list/subfunctions.md
    │
    ├── voice/
    │   ├── voice.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B1-whisper-model/subfunctions.md
    │       ├── B2-recording/subfunctions.md
    │       ├── B3-audio-capture/subfunctions.md
    │       ├── B4-call-mode-control/subfunctions.md
    │       ├── B5-call-audio-loop/subfunctions.md
    │       ├── B6-tts-prepare/subfunctions.md
    │       ├── B7-tts-synthesis/subfunctions.md
    │       ├── B8-tts-commands/subfunctions.md
    │       ├── F1-voice-button/subfunctions.md
    │       ├── F2-call-mode-ui/subfunctions.md
    │       └── I1-remote-tts/subfunctions.md
    │
    ├── proactive/
    │   ├── proactive.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B1-proactive-types/subfunctions.md
    │       ├── B3-proactive-prompt/subfunctions.md
    │       ├── B4-os-context/subfunctions.md
    │       ├── B5-morning-digest/subfunctions.md
    │       └── B8-proactive-generation/subfunctions.md
    │
    ├── system/
    │   ├── system.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B4-web-search/subfunctions.md
    │       ├── B5-api-server/subfunctions.md
    │       ├── B6-mlx-server/subfunctions.md
    │       └── B7-macos-integration/subfunctions.md
    │
    ├── focus/
    │   ├── focus.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B1-start-focus/subfunctions.md
    │       └── F1-focus-ui/subfunctions.md
    │
    ├── calendar/
    │   ├── calendar.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B2-apple-calendar-sync/subfunctions.md
    │       ├── B3-rrule-parser/subfunctions.md
    │       ├── B4-google-calendar-sync/subfunctions.md
    │       ├── F1-month-calendar/subfunctions.md
    │       ├── F2-week-calendar/subfunctions.md
    │       ├── F3-day-calendar/subfunctions.md
    │       ├── F4-event-list/subfunctions.md
    │       ├── F5-calendar-integrations/subfunctions.md
    │       └── F6-add-event-modal/subfunctions.md
    │
    ├── tracker/
    │   ├── tracker.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B1-tracker-crud/subfunctions.md
    │       ├── B2-screen-time/subfunctions.md
    │       ├── B3-macos-context/subfunctions.md
    │       ├── B4-activities/subfunctions.md
    │       ├── B5-dashboard-data/subfunctions.md
    │       └── F1-dashboard-ui/subfunctions.md
    │
    ├── content/
    │   ├── content.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B1-notes-crud/subfunctions.md
    │       ├── B2-projects-tasks/subfunctions.md
    │       ├── F1-notes-ui/subfunctions.md
    │       ├── F2-projects-ui/subfunctions.md
    │       └── F3-learning-ui/subfunctions.md
    │
    ├── media/
    │   ├── media.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B2-media-crud/subfunctions.md
    │       ├── B3-user-lists/subfunctions.md
    │       ├── F2-media-list/subfunctions.md
    │       └── F3-media-modals/subfunctions.md
    │
    ├── lifestyle/
    │   ├── lifestyle.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B3-habits/subfunctions.md
    │       ├── B6-products/subfunctions.md
    │       ├── B7-transactions/subfunctions.md
    │       ├── B12-journal/subfunctions.md
    │       ├── B17-contacts/subfunctions.md
    │       ├── F1-home/subfunctions.md
    │       ├── F2-mindset/subfunctions.md
    │       ├── F3-food/subfunctions.md
    │       ├── F4-money/subfunctions.md
    │       ├── F5-contacts/subfunctions.md
    │       ├── F6-sports/subfunctions.md
    │       └── F7-health/subfunctions.md
    │
    ├── page_framework/
    │   ├── page_framework.md
    │   ├── functions.md
    │   └── functions/
    │       ├── B1-training-stats/subfunctions.md
    │       ├── B6-properties/subfunctions.md
    │       ├── B7-view-configs/subfunctions.md
    │       ├── B8-integrations/subfunctions.md
    │       ├── F1-goals-widget/subfunctions.md
    │       ├── F2-chat-settings/subfunctions.md
    │       ├── F3-settings-ui/subfunctions.md
    │       ├── F4-database-view/subfunctions.md
    │       ├── F5-property-ui/subfunctions.md
    │       └── F6-filters/subfunctions.md
    │
    ├── ui/
    │   ├── ui.md
    │   ├── functions.md
    │   └── functions/
    │       ├── F4-tab-bar/subfunctions.md
    │       ├── F5-sub-sidebar/subfunctions.md
    │       └── F7-subtab-loading/subfunctions.md
    │
    └── ml_devops/
        ├── ml_devops.md
        ├── functions.md
        └── functions/
            ├── B1-training-data/subfunctions.md
            ├── B2-claude-distill/subfunctions.md
            ├── B3-nightly-train/subfunctions.md
            └── B5-release-pipeline/subfunctions.md
```

---

## Как использовать

### Найти, где живёт конкретная функция
1. Открой [MODULE_MAP.md](../MODULE_MAP.md) → найди файл → узнай модуль
2. Перейди в `[module]/functions.md` → найди ID функции
3. Если нужна детальная декомпозиция → `[module]/functions/[ID]-[name]/subfunctions.md`

### Понять, как работает фича
1. Начни с `[module]/[module].md` (wiki) — обзор, зависимости
2. Посмотри `functions.md` — какие бизнес-функции задействованы
3. Для сложных функций — читай `subfunctions.md` с data flow диаграммой

### Найти проблемные места для рефакторинга
1. Открой [DISPUTED.md](DISPUTED.md) → секция "Кандидаты на рефакторинг"
2. Или смотри "Проблемные зоны" в каждом `subfunctions.md`
3. Топ-10 проблем — в этом файле выше

### Проверить покрытие документации
1. Открой [COVERAGE.md](COVERAGE.md) → таблица модулей
2. Все модули Phase 1-3: done
3. Контрольная сумма LOC: ~26,569 mapped
