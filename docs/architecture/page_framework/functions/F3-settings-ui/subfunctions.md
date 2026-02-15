# F3 — Integrations and settings: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F3: Integrations and settings |
| Файл(ы) | main.js:L2774-2985 |
| LOC | 212 |
| Подфункций | 9 |
| Сложность (max) | High |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.1 | Загрузка данных интеграций | L2774-2779 | 6 | loadIntegrations() → invoke('get_integrations') | bridge | Trivial |
| F3.2 | Формирование HTML карточек | L2781-2804 | 24 | info → accessItems, trackingItems, appsItems, sitesItems, macosItems, legend, blockerBadge | transform | Low |
| F3.3 | Загрузка настроек календаря | L2806-2807 | 2 | — → invoke('get_app_setting') для apple_calendar_enabled, google_ics_url | bridge | Trivial |
| F3.4 | Рендер интеграционной сетки | L2809-2850 | 42 | HTML-части → DOM: 6 integration-card в .integrations-grid | render | Medium |
| F3.5 | Обработчики календарных интеграций | L2852-2871 | 20 | change #int-apple-cal → save setting; click #int-cal-save → save ICS URL | interaction | Low |
| F3.6 | Роутер настроек | L2879-2888 | 10 | loadSettings(subTab) → dispatch: Memory/Blocklist/Integrations/About/default | control | Trivial |
| F3.7 | Рендер Blocklist | L2890-2928 | 39 | loadBlocklist(el) → invoke('get_blocklist') → рендер sites/apps с toggle/delete/add | render | Medium |
| F3.8 | Рендер About | L2930-2977 | 48 | loadAbout(el) → invoke('get_model_info') + invoke('get_training_stats') → info page | render | Medium |
| F3.9 | Stub-функция | L2979-2983 | 5 | showStub(containerId, icon, label) → placeholder для незагруженных табов | render | Trivial |

## Data Flow

```
[Input: loadSettings(subTab)]
    │
    ▼
┌─ F3.6 Роутер ────────────────┐
│  switch subTab:                  │
│    'Memory'       → loadMemory   │
│    'Blocklist'    → F3.7         │
│    'Integrations' → F3.1         │
│    'About'        → F3.8         │
│    default        → loadMemory   │
└──────┬───────────────────────────┘
       │
  ┌────┼────────────────┐
  ▼    ▼                ▼
┌─ F3.1+F3.2+F3.4 ─┐  ┌─ F3.7 Blocklist ──┐  ┌─ F3.8 About ──────┐
│ invoke('get_      │  │ invoke('get_       │  │ invoke('get_model_ │
│   integrations')  │  │   blocklist')      │  │   info')           │
│ → panelItem()     │  │ → render sites/    │  │ invoke('get_       │
│   per category    │  │   apps lists       │  │   training_stats') │
│ invoke('get_app_  │  │ toggle/delete/     │  │ → version, model,  │
│   setting') x2    │  │   add handlers     │  │   data, API status │
│ → integration-    │  └────────────────────┘  └────────────────────┘
│   grid (6 cards)  │
│ F3.5: calendar    │
│   handlers        │
└───────────────────┘
    │                       │                        │
    ▼                       ▼                        ▼
[DOM: #settings-content]  [DOM: #settings-content]  [DOM: #settings-content]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F3.4 | Рендер интеграционной сетки | 42 LOC — огромная template literal | Вынести каждую карточку в helper | Low |
| F3.8 | About | 48 LOC — смешивает рендер и обработчики (check update, export, API status) | Разделить рендер и binding handlers | Low |
| F3.7 | Blocklist | prompt() для ввода типа и значения — плохой UX | Заменить на модальное окно | Medium |
