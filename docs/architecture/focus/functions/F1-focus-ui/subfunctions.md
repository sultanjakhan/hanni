# F1 — UI режима фокуса: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: UI режима фокуса |
| Файл(ы) | main.js:L3056-3161 |
| LOC | 106 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger -> Effect | Тип | Сложность |
|---|-----------|--------|-----|-------------------|-----|-----------|
| F1.1 | Загрузка данных (current activity + log) | L3056-3063 | 8 | tab switch -> invoke get_current_activity, get_activity_log | bridge | Trivial |
| F1.2 | Рендер текущей активности (или формы запуска) | L3064-3095 | 32 | current? -> HTML: таймер или форма ввода | render | Medium |
| F1.3 | Рендер лога за сегодня | L3097-3106 | 10 | log[] -> HTML: список записей | render | Trivial |
| F1.4 | Сборка страницы и вставка в DOM | L3108 | 1 | html parts -> el.innerHTML | render | Trivial |
| F1.5 | Привязка событий: выбор категории | L3110-3120 | 11 | click preset btn -> set selectedCategory, autofill title | interaction | Trivial |
| F1.6 | Привязка событий: start/stop activity | L3122-3140 | 19 | click start/stop -> invoke start_activity / stop_activity | interaction | Low |
| F1.7 | Живой таймер (setInterval) | L3142-3155 | 14 | current.started_at -> обновление DOM каждую секунду | render | Trivial |

## Data Flow

```
[Trigger: loadFocus() вызывается при переключении на вкладку Focus]
    │
    ▼
┌─ F1.1 Load Data ──────────────────────┐
│  invoke('get_current_activity')        │ ──→ [Backend: Tauri]
│  invoke('get_activity_log', {date:null})│ ──→ [Backend: Tauri]
│  skeleton page while loading           │
└──────┬────────────────────────────────┘
       │ current, log
       ▼
┌─ F1.2 Render Current ─────────────────┐
│  if current:                           │
│    → таймер + кнопка "Завершить"       │
│  else:                                 │
│    → input + пресеты категорий         │
│    → чекбокс "Блокировать отвлечения"  │
│    → кнопка "Начать"                   │
└──────┬────────────────────────────────┘
       │
       ▼
┌─ F1.3 Render Log ──────────────┐
│  log.map(item => HTML)          │
│  time | title | category | dur  │
└──────┬─────────────────────────┘
       │
       ▼
┌─ F1.4 Insert DOM ──────────────┐
│  el.innerHTML = header + content│
└──────┬─────────────────────────┘
       │
       ▼
┌─ F1.5-F1.6 Bind Events ───────────────────────┐
│  preset click → select category, fill title     │
│  start click  → invoke('start_activity', ...)   │ ──→ [Backend]
│  stop click   → invoke('stop_activity')         │ ──→ [Backend]
│  reload page after start/stop                    │
└──────┬─────────────────────────────────────────┘
       │
       ▼
┌─ F1.7 Live Timer ──────────────────────────┐
│  setInterval(1000):                         │
│    elapsed = now - started_at               │
│    format h:mm:ss                           │
│    timerEl.textContent = formatted           │
│    clear if tab changed                      │
└─────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F1.2 | Рендер формы | 32 LOC строковых шаблонов — хрупко для поддержки | Вынести шаблоны в функции-компоненты | Low |
| F1.7 | Таймер | Глобальная переменная focusTimerInterval — не cleanup при ошибках | Привязать lifecycle к вкладке | Low |
