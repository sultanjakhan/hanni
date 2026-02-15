# B1 — Life Tracker CRUD (JSON): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Life Tracker CRUD (JSON) |
| Файл(ы) | lib.rs:L2937-3057 |
| LOC | 121 |
| Подфункций | 6 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.1 | tracker_add_purchase | L2937-2954 | 18 | (amount, category, description) → Result<String> | load_tracker_data + push + save_tracker_data | persist | Low |
| B1.2 | tracker_add_time | L2956-2973 | 18 | (activity, duration, category, productive) → Result<String> | load_tracker_data + push + save_tracker_data | persist | Low |
| B1.3 | tracker_add_goal | L2975-2992 | 18 | (title, category) → Result<String> | load_tracker_data + push + save_tracker_data | persist | Low |
| B1.4 | tracker_add_note | L2994-3011 | 18 | (title, content) → Result<String> | load_tracker_data + push + save_tracker_data | persist | Low |
| B1.5 | tracker_get_stats — агрегация за месяц | L3013-3040 | 28 | — → Result<String> (форматированная статистика) | load_tracker_data (read JSON) | query | Low |
| B1.6 | tracker_get_recent — последние записи | L3042-3054 | 13 | (entry_type, limit) → Result<String> (JSON) | load_tracker_data (read JSON) | query | Trivial |

## Data Flow

```
[Tauri invoke: tracker_add_*]
    │
    ▼
┌─ B1.1-B1.4 (общий паттерн) ─────────────┐
│  load_tracker_data()                       │ ──→ [FS: tracker.json]
│  Создать JSON entry с id, date, fields     │
│  data.[collection].push(entry)             │
│  save_tracker_data(&data)                  │ ──→ [FS: tracker.json]
│  Return форматированную строку             │
└────────────────────────────────────────────┘

[Tauri invoke: tracker_get_stats]
    │
    ▼
┌─ B1.5 Агрегация ────────────────────────┐
│  load_tracker_data()                      │ ──→ [FS: tracker.json]
│  Filter по текущему месяцу                │
│  Sum purchases, time_entries              │
│  Count active goals, notes                │
│  Format emoji string                      │
└──────────────────────────────────────────┘

[Tauri invoke: tracker_get_recent]
    │
    ▼
┌─ B1.6 Последние записи ─────────────────┐
│  load_tracker_data()                      │ ──→ [FS: tracker.json]
│  Match entry_type → .iter().rev().take(N) │
│  serde_json::to_string_pretty()           │
└──────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.1-B1.4 | Все add-функции | Каждый вызов делает load + save всего JSON файла (race condition при параллельных вызовах) | Миграция на SQLite или file lock | Low |
| B1.5 | tracker_get_stats | Фильтрация по month string prefix — хрупко | Использовать chrono для date comparison | Low |
