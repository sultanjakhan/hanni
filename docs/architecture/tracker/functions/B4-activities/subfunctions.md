# B4 — Activities (start/stop): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Activities (start/stop) |
| Файл(ы) | lib.rs:L3614-3726 |
| LOC | 113 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B4.1 | start_activity — создание записи + опциональный фокус | L3614-3640 | 27 | (title, category, focus_mode, duration?, apps?, sites?, db, focus) → Result<i64> | DB INSERT activities, опционально start_focus() | persist | Low |
| B4.2 | stop_activity — поиск и завершение текущей | L3642-3676 | 35 | (db, focus) → Result<String> | DB SELECT + UPDATE activities (ended_at, duration), stop_focus() | persist | Medium |
| B4.3 | get_current_activity — текущая активность | L3678-3698 | 21 | db → Result<Value> | DB SELECT (WHERE ended_at IS NULL) | query | Low |
| B4.4 | get_activity_log — лог за дату | L3700-3723 | 24 | (date?, db) → Result<Vec<Value>> | DB SELECT с LIKE по дате | query | Low |
| B4.5 | Вычисление elapsed time (shared logic) | L3688-3693, L3657-3658 | 6 | started_at RFC3339 → elapsed String | — | transform | Trivial |

## Data Flow

```
[Tauri invoke: start_activity]
    │
    ▼
┌─ B4.1 start_activity ───────────────────────────┐
│  conn = db.conn()                                 │
│  INSERT INTO activities (title, category,          │
│    started_at, focus_mode)                         │ ──→ [DB: activities]
│  id = last_insert_rowid()                          │
│  if focus_mode → start_focus(dur, apps, sites)     │ ──→ [FocusManager: блокировка]
│  return id                                         │
└──────────────────────────────────────────────────┘

[Tauri invoke: stop_activity]
    │
    ▼
┌─ B4.2 stop_activity ────────────────────────────┐
│  SELECT id, started_at FROM activities            │
│    WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1│ ──→ [DB: activities]
│  if found:                                         │
│    parse started_at → compute duration_minutes     │
│    UPDATE activities SET ended_at, duration         │ ──→ [DB: activities]
│    stop_focus()                                    │ ──→ [FocusManager]
│  else: "No active activity"                        │
└──────────────────────────────────────────────────┘

[Tauri invoke: get_current_activity]
    │
    ▼
┌─ B4.3 get_current_activity ─────────────────────┐
│  SELECT id, title, category, started_at           │
│    WHERE ended_at IS NULL LIMIT 1                  │ ──→ [DB: activities]
│  B4.5: parse started_at → elapsed "Xч Yм"         │
│  return JSON {id, title, category, elapsed}        │
└──────────────────────────────────────────────────┘

[Tauri invoke: get_activity_log]
    │
    ▼
┌─ B4.4 get_activity_log ────────────────────────┐
│  date = date.unwrap_or(today)                    │
│  SELECT id, title, category, started_at,          │
│    ended_at, duration_minutes                      │
│  WHERE started_at LIKE "date%"                     │ ──→ [DB: activities]
│  B4.5: format time (HH:MM) + duration (Xч Yм)     │
│  return Vec<JSON>                                  │
└──────────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B4.2 | stop_activity | Если parse RFC3339 fails — duration не записывается (silent fallback) | Логировать предупреждение при parse failure | Low |
| B4.4 | get_activity_log | LIKE по строке даты — не использует индекс по date column | Добавить отдельную date колонку или index | Low |
