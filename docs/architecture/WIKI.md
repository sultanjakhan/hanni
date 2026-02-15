# Hanni — Architecture Wiki

> Навигационная карта всей архитектурной документации.
> 3 уровня декомпозиции: **Проект → Модули → Функции → Подфункции**
> Версия: v0.13.9 | 2026-02-15

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
Проект Hanni
├── 15 модулей
├── 149 бизнес-функций (91 Medium/Complex + 58 Simple)
├── 561 подфункций (декомпозированы из 91 функций)
├── ~18,840 LOC исходного кода
├── ~78 проблемных зон обнаружено
└── 127 файлов документации
```

---

## Карта модулей

### Ядро (Core Layer)

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 1 | **core** | ~1,800 | 5 | 31 | [wiki](core/core.md) | [functions](core/functions.md) | [B1](core/functions/B1-system-prompt/subfunctions.md) / [B2](core/functions/B2-init-db/subfunctions.md) / [B5](core/functions/B5-run/subfunctions.md) |
| 6 | **system** | ~540 | 8 | 28 | [wiki](system/system.md) | [functions](system/functions.md) | [B4](system/functions/B4-web-search/subfunctions.md) / [B5](system/functions/B5-api-server/subfunctions.md) / [B6](system/functions/B6-mlx-server/subfunctions.md) / [B7](system/functions/B7-macos-integration/subfunctions.md) |

### AI & Chat

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 2 | **chat** | ~1,340 | 10 | 56 | [wiki](chat/chat.md) | [functions](chat/functions.md) | [B2](chat/functions/B2-streaming-chat/subfunctions.md) / [B3](chat/functions/B3-conversation-crud/subfunctions.md) / [B4](chat/functions/B4-fact-extraction/subfunctions.md) / [F1](chat/functions/F1-conversation-list/subfunctions.md) / [F2](chat/functions/F2-load-conversation/subfunctions.md) / [F3](chat/functions/F3-execute-action/subfunctions.md) / [F4](chat/functions/F4-send-message/subfunctions.md) / [F5](chat/functions/F5-stream-chat/subfunctions.md) / [F6](chat/functions/F6-add-message-ui/subfunctions.md) |
| 3 | **memory** | ~620 | 8 | 17 | [wiki](memory/memory.md) | [functions](memory/functions.md) | [B2](memory/functions/B2-memory-context/subfunctions.md) / [B4](memory/functions/B4-memory-search/subfunctions.md) / [F1](memory/functions/F1-memory-tab/subfunctions.md) / [F2](memory/functions/F2-memory-list/subfunctions.md) |
| 5 | **proactive** | ~690 | 10 | 29 | [wiki](proactive/proactive.md) | [functions](proactive/functions.md) | [B1](proactive/functions/B1-proactive-types/subfunctions.md) / [B3](proactive/functions/B3-proactive-prompt/subfunctions.md) / [B4](proactive/functions/B4-os-context/subfunctions.md) / [B5](proactive/functions/B5-morning-digest/subfunctions.md) / [B8](proactive/functions/B8-proactive-generation/subfunctions.md) |

### Voice & Focus

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 4 | **voice** | ~1,650 | 11 | 73 | [wiki](voice/voice.md) | [functions](voice/functions.md) | [B1](voice/functions/B1-whisper-model/subfunctions.md) / [B2](voice/functions/B2-recording/subfunctions.md) / [B3](voice/functions/B3-audio-capture/subfunctions.md) / [B4](voice/functions/B4-call-mode-control/subfunctions.md) / [B5](voice/functions/B5-call-audio-loop/subfunctions.md) / [B6](voice/functions/B6-tts-prepare/subfunctions.md) / [B7](voice/functions/B7-tts-synthesis/subfunctions.md) / [B8](voice/functions/B8-tts-commands/subfunctions.md) / [F1](voice/functions/F1-voice-button/subfunctions.md) / [F2](voice/functions/F2-call-mode-ui/subfunctions.md) / [I1](voice/functions/I1-remote-tts/subfunctions.md) |
| 7 | **focus** | ~350 | 7 | 13 | [wiki](focus/focus.md) | [functions](focus/functions.md) | [B1](focus/functions/B1-start-focus/subfunctions.md) / [F1](focus/functions/F1-focus-ui/subfunctions.md) |

### Data & Content

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 8 | **calendar** | ~1,030 | 10 | 60 | [wiki](calendar/calendar.md) | [functions](calendar/functions.md) | [B2](calendar/functions/B2-apple-calendar-sync/subfunctions.md) / [B3](calendar/functions/B3-rrule-parser/subfunctions.md) / [B4](calendar/functions/B4-google-calendar-sync/subfunctions.md) / [F1](calendar/functions/F1-month-calendar/subfunctions.md) / [F2](calendar/functions/F2-week-calendar/subfunctions.md) / [F3](calendar/functions/F3-day-calendar/subfunctions.md) / [F4](calendar/functions/F4-event-list/subfunctions.md) / [F5](calendar/functions/F5-calendar-integrations/subfunctions.md) / [F6](calendar/functions/F6-add-event-modal/subfunctions.md) |
| 9 | **tracker** | ~600 | 6 | 31 | [wiki](tracker/tracker.md) | [functions](tracker/functions.md) | [B1](tracker/functions/B1-tracker-crud/subfunctions.md) / [B2](tracker/functions/B2-screen-time/subfunctions.md) / [B3](tracker/functions/B3-macos-context/subfunctions.md) / [B4](tracker/functions/B4-activities/subfunctions.md) / [B5](tracker/functions/B5-dashboard-data/subfunctions.md) / [F1](tracker/functions/F1-dashboard-ui/subfunctions.md) |
| 10 | **content** | ~520 | 6 | 31 | [wiki](content/content.md) | [functions](content/functions.md) | [B1](content/functions/B1-notes-crud/subfunctions.md) / [B2](content/functions/B2-projects-tasks/subfunctions.md) / [F1](content/functions/F1-notes-ui/subfunctions.md) / [F2](content/functions/F2-projects-ui/subfunctions.md) / [F3](content/functions/F3-learning-ui/subfunctions.md) |
| 11 | **media** | ~460 | 7 | 22 | [wiki](media/media.md) | [functions](media/functions.md) | [B2](media/functions/B2-media-crud/subfunctions.md) / [B3](media/functions/B3-user-lists/subfunctions.md) / [F2](media/functions/F2-media-list/subfunctions.md) / [F3](media/functions/F3-media-modals/subfunctions.md) |

### Lifestyle

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 12 | **lifestyle** | ~2,080 | 25 | 53 | [wiki](lifestyle/lifestyle.md) | [functions](lifestyle/functions.md) | [B3](lifestyle/functions/B3-habits/subfunctions.md) / [B6](lifestyle/functions/B6-products/subfunctions.md) / [B7](lifestyle/functions/B7-transactions/subfunctions.md) / [B12](lifestyle/functions/B12-journal/subfunctions.md) / [B17](lifestyle/functions/B17-contacts/subfunctions.md) / [F1](lifestyle/functions/F1-home/subfunctions.md) / [F2](lifestyle/functions/F2-mindset/subfunctions.md) / [F3](lifestyle/functions/F3-food/subfunctions.md) / [F4](lifestyle/functions/F4-money/subfunctions.md) / [F5](lifestyle/functions/F5-contacts/subfunctions.md) / [F6](lifestyle/functions/F6-sports/subfunctions.md) / [F7](lifestyle/functions/F7-health/subfunctions.md) |

### Framework & UI

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 13 | **page_framework** | ~1,610 | 14 | 77 | [wiki](page_framework/page_framework.md) | [functions](page_framework/functions.md) | [B1](page_framework/functions/B1-training-stats/subfunctions.md) / [B6](page_framework/functions/B6-properties/subfunctions.md) / [B7](page_framework/functions/B7-view-configs/subfunctions.md) / [B8](page_framework/functions/B8-integrations/subfunctions.md) / [F1](page_framework/functions/F1-goals-widget/subfunctions.md) / [F2](page_framework/functions/F2-chat-settings/subfunctions.md) / [F3](page_framework/functions/F3-settings-ui/subfunctions.md) / [F4](page_framework/functions/F4-database-view/subfunctions.md) / [F5](page_framework/functions/F5-property-ui/subfunctions.md) / [F6](page_framework/functions/F6-filters/subfunctions.md) |
| 14 | **ui** | ~4,370 | 17 | 11 | [wiki](ui/ui.md) | [functions](ui/functions.md) | [F4](ui/functions/F4-tab-bar/subfunctions.md) / [F5](ui/functions/F5-sub-sidebar/subfunctions.md) / [F7](ui/functions/F7-subtab-loading/subfunctions.md) |

### ML & DevOps

| # | Модуль | LOC | Функций | Подфункций | Wiki | Функции | Подфункции |
|---|--------|-----|---------|------------|------|---------|------------|
| 15 | **ml_devops** | ~1,180 | 5 | 29 | [wiki](ml_devops/ml_devops.md) | [functions](ml_devops/functions.md) | [B1](ml_devops/functions/B1-training-data/subfunctions.md) / [B2](ml_devops/functions/B2-claude-distill/subfunctions.md) / [B3](ml_devops/functions/B3-nightly-train/subfunctions.md) / [B5](ml_devops/functions/B5-release-pipeline/subfunctions.md) |

---

## Граф зависимостей модулей

```
                    ┌──────────┐
                    │   core   │
                    │ (DB,run) │
                    └────┬─────┘
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
      ┌─────────┐  ┌──────────┐  ┌──────────┐
      │  system  │  │   chat   │  │    ui    │
      │(OS,MLX) │  │(LLM,SSE)│  │(tabs,CSS)│
      └────┬────┘  └────┬─────┘  └────┬─────┘
           │             │             │
     ┌─────┤       ┌─────┼─────┐       │
     ▼     ▼       ▼     ▼     ▼       ▼
  ┌──────┐ │  ┌────────┐ │ ┌────────┐ ┌──────────────┐
  │focus │ │  │ memory │ │ │proactiv│ │page_framework│
  └──────┘ │  └────────┘ │ └────────┘ │(views,props) │
           │             │            └──────┬───────┘
           ▼             ▼                   │
      ┌─────────┐  ┌──────────┐             │
      │ tracker │  │  voice   │             ▼
      │(screen) │  │(STT,TTS) │     ┌───────────────┐
      └─────────┘  └──────────┘     │ content/media │
                                    │  /lifestyle   │
                                    │  /calendar    │
                                    └───────────────┘
```

**Направление стрелок:** модуль зависит от →

- **core** — ни от кого, все зависят от core (DB, constants)
- **chat** — от core, memory (контекст), system (shell actions)
- **voice** — от chat (транскрипт → чат), system (TTS процессы)
- **proactive** — от chat (LLM), memory (контекст), tracker (screen time)
- **page_framework** — от core (DB), используется lifestyle/media/content
- **ui** — от core, page_framework (tab система)

---

## Топ-10 проблемных зон

| # | Модуль | Функция | Подфункция | Проблема | Приоритет |
|---|--------|---------|------------|---------|-----------|
| 1 | core | B5 run() | B5.13 | 217 LOC проактивный loop в одном async блоке | High |
| 2 | core | B5 run() | — | 811 LOC — самая большая функция проекта | High |
| 3 | core | B2 init_db() | — | 534 LOC в одном execute_batch, нет миграций | Medium |
| 4 | chat | F3 executeAction() | — | 35+ action types в монолитном switch | Medium |
| 5 | voice | B5/B3 | B5.1+B5.2 | Дублирование аудио инициализации между capture и call loop | High |
| 6 | voice | B7 TTS | — | Async/sync варианты TTS — почти идентичный код | Medium |
| 7 | chat | B2 streaming | B2.1 | Busy-wait spin-lock вместо tokio Semaphore | Medium |
| 8 | chat | B4 facts | B4.5 | Хрупкий JSON парсинг через find('{') / rfind('}') | Medium |
| 9 | page_framework | F4 dbview | F4.10 | startInlineEdit 98 LOC, 6 типов редакторов в одной fn | Medium |
| 10 | ml_devops | B2+B3 | — | Python inline в bash heredoc — антипаттерн | Medium |

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
    │       ├── B1-system-prompt/subfunctions.md     # Подфункции
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
1. Открой [MODULE_MAP.md](../MODULE_MAP.md) → найди файл/секцию → узнай модуль
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
2. Все модули Phase 1-3: ✅ done
3. Контрольная сумма LOC: ~18,840 mapped = ~18,600 actual ✅
