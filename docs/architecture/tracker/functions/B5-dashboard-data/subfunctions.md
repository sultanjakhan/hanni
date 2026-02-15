# B5 — Dashboard aggregation: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: Dashboard aggregation |
| Файл(ы) | lib.rs:L4675-4736 |
| LOC | 62 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B5.1 | Получение текущей активности | L4675-4694 | 20 | db conn → Option<Value> (title, category, elapsed) | DB SELECT activities WHERE ended_at IS NULL | query | Low |
| B5.2 | Подсчёт активностей и фокус-минут за сегодня | L4695-4704 | 10 | (conn, today_pattern) → (activities_today, focus_minutes) | DB SELECT COUNT + SUM | query | Trivial |
| B5.3 | Подсчёт заметок | L4706-4707 | 2 | conn → notes_count | DB SELECT COUNT notes | query | Trivial |
| B5.4 | Получение событий и последних заметок | L4709-4723 | 15 | (conn, today) → (events, recent_notes) | DB SELECT events + notes (2 запроса) | query | Low |
| B5.5 | Сборка JSON ответа | L4725-4734 | 10 | все агрегации → serde_json::Value | — | transform | Trivial |

## Data Flow

```
[Tauri invoke: get_dashboard_data]
    │
    ▼
┌─ B5.1 Текущая активность ──────────────┐
│  SELECT title, category, started_at     │
│  WHERE ended_at IS NULL LIMIT 1          │ ──→ [DB: activities]
│  Parse started_at → elapsed "Xм"         │
└──────┬──────────────────────────────────┘
       │
       ▼
┌─ B5.2 Счётчики за сегодня ─────────────┐
│  COUNT(*) activities today               │ ──→ [DB: activities]
│  SUM(duration_minutes) today             │ ──→ [DB: activities]
└──────┬──────────────────────────────────┘
       │
       ▼
┌─ B5.3 Заметки ─────────────────────────┐
│  COUNT(*) notes WHERE !archived          │ ──→ [DB: notes]
└──────┬──────────────────────────────────┘
       │
       ▼
┌─ B5.4 События + последние заметки ─────┐
│  SELECT events WHERE date=today          │ ──→ [DB: events]
│  SELECT notes ORDER BY updated_at        │
│    DESC LIMIT 3                          │ ──→ [DB: notes]
└──────┬──────────────────────────────────┘
       │
       ▼
┌─ B5.5 Сборка JSON ────────────────────┐
│  json!({                                │
│    current_activity, activities_today,   │
│    focus_minutes, notes_count,           │
│    events_today, events, recent_notes    │
│  })                                     │
└─────────────────────────────────────────┘
```

## Проблемные зоны

Нет критичных проблем. Функция компактная, 5 простых DB-запросов + JSON сборка.
