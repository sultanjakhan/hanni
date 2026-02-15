# F1 — Dashboard loading: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Dashboard loading |
| Файл(ы) | main.js:L2986-3055 |
| LOC | 70 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Skeleton загрузки | L2986-2989 | 4 | tab switch → el.innerHTML = skeletonPage() | render | Trivial |
| F1.2 | Запрос данных и формирование greeting | L2990-2994 | 5 | — → invoke get_dashboard_data, dateStr, greeting | bridge | Trivial |
| F1.3 | Рендеринг focus banner + stats | L2996-3014 | 19 | data → HTML (focus banner + 4 stat cards) | render | Low |
| F1.4 | Рендеринг events + notes + quick actions | L3015-3031 | 17 | data → HTML (events list, notes list, action buttons) | render | Low |
| F1.5 | Fallback при ошибке (default dashboard) | L3032-3053 | 22 | catch(e) → HTML (static dashboard без данных) | render | Low |

## Data Flow

```
[Tab switch → loadDashboard()]
    │
    ▼
┌─ F1.1 Skeleton ──────────────────────────────┐
│  el = getElementById('dashboard-content')      │
│  el.innerHTML = skeletonPage()                 │ ──→ [DOM: loading skeleton]
└──────┬────────────────────────────────────────┘
       │
       ▼
┌─ F1.2 Запрос данных ────────────────────────┐
│  data = await invoke('get_dashboard_data')    │ ──→ [B5: get_dashboard_data]
│  dateStr = toLocaleDateString('ru-RU')        │
│  greeting = time-based (утро/день/вечер)      │
└──────┬────────────────────────────────────────┘
       │ data
       ▼
┌─ F1.3 Focus banner + stats ─────────────────┐
│  if data.current_activity:                    │
│    render focus-banner (title, elapsed)        │ ──→ [DOM]
│  Render 4 stat cards:                          │
│    activities_today, focus_minutes,            │
│    notes_count, events_today                   │ ──→ [DOM]
└──────┬────────────────────────────────────────┘
       │
       ▼
┌─ F1.4 Events + notes + actions ─────────────┐
│  if events.length > 0:                        │
│    render event list (time + title)            │ ──→ [DOM]
│  if recent_notes.length > 0:                  │
│    render notes list (title)                   │ ──→ [DOM]
│  Render quick action buttons                   │
│    (notes, focus, health)                      │ ──→ [DOM: onclick=switchTab]
└──────────────────────────────────────────────┘

[Ошибка invoke]
    │
    ▼
┌─ F1.5 Fallback ────────────────────────────┐
│  Render static dashboard (0 values)          │
│  Same layout, default greeting               │ ──→ [DOM]
└──────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F1.5 | Fallback | Дублирует 90% HTML из F1.3-F1.4 | Вынести renderDashboard(data) с default values | Low |
