# F3 — Выполнение действий LLM: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F3: Выполнение действий LLM |
| Файл(ы) | main.js:L1105-1382 |
| LOC | 278 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.1 | Парсинг JSON и нормализация полей | L1105-1119 | 15 | actionJson string → action object + actionType | parse | Low |
| F3.2 | Трекер-действия (purchase/time/goal/note/stats) | L1121-1161 | 41 | actionType, action → result | bridge | Medium |
| F3.3 | Системные интеграции (activity/calendar/music/browser) | L1162-1170 | 9 | actionType → result | bridge | Trivial |
| F3.4 | Память (remember/recall/forget/search) | L1171-1195 | 25 | actionType, action → result | bridge | Low |
| F3.5 | Системные действия (shell/url/notification/clipboard/search) | L1196-1232 | 37 | actionType, action → result | bridge | Medium |
| F3.6 | Контент-действия (media/food/money/mindset) | L1233-1284 | 52 | actionType, action → result | bridge | Medium |
| F3.7 | Планирование (events/activities/tasks/home/health/workout/goals) | L1285-1371 | 87 | actionType, action → result | bridge | High |
| F3.8 | Обработка неизвестных + возврат результата | L1372-1382 | 11 | result → {success, result} | error | Trivial |

## Data Flow

```
[Input: actionJson string]
    │
    ▼
┌─ F3.1 Parse + Normalize ───┐
│  JSON.parse(actionJson)      │
│  normalize meal_type         │
│  extract actionType          │
│  redirect log_health→log_mood│
└──────┬──────────────────────┘
       │ action, actionType
       ▼
┌─ F3.2-F3.7 Switch Dispatch ┐
│  switch (actionType) {       │
│    case 'add_purchase':      │──→ [Tauri: tracker_add_purchase]
│    case 'remember':          │──→ [Tauri: memory_remember]
│    case 'run_shell':         │──→ [Tauri: run_shell]
│    case 'log_food':          │──→ [Tauri: log_food]
│    case 'create_event':      │──→ [Tauri: create_event]
│    ... (35+ action types)    │
│  }                           │
└──────┬──────────────────────┘
       │ result
       ▼
┌─ F3.8 Return ──────────────┐
│  {success: true, result}     │
│  catch → {success: false}    │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F3.7 | Планирование | 87 LOC, 12+ case-веток в одном блоке — самый большой участок switch | Вынести группы действий в отдельные функции (dispatchTracker, dispatchSystem и т.д.) | Medium |
| F3.2-F3.7 | Switch в целом | 35+ action types в монолитном switch — сложно поддерживать | Рефакторинг в action registry (Map<string, handler>) | Medium |
| F3.1 | Нормализация | Нормализация только для meal_type — другие поля могут тоже содержать русские значения | Расширить нормализацию на другие поля | Low |
