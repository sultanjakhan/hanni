# B2 — Screen Time (knowledgeC.db): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Screen Time (knowledgeC.db) |
| Файл(ы) | lib.rs:L3058-3152 |
| LOC | 95 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Проверка доступа к knowledgeC.db | L3058-3070 | 13 | — → db_path или Error | Чтение FS (exists check) | validation | Trivial |
| B2.2 | Открытие БД (read-only) с обработкой ошибок доступа | L3072-3084 | 13 | db_path → Connection | Открытие SQLite (read-only, no mutex) | external | Low |
| B2.3 | SQL-запрос Screen Time за сегодня | L3086-3121 | 36 | Connection → Vec<AppRow> | SQL SELECT + JOIN + GROUP BY + classify_app() | query | Medium |
| B2.4 | Агрегация по категориям (productive/distraction/neutral) | L3123-3137 | 15 | Vec<AppRow> → (productive, distraction, neutral) | — | process | Trivial |
| B2.5 | Форматирование результата (top 5 apps) | L3139-3152 | 14 | (categories, rows) → String | — | transform | Trivial |

## Data Flow

```
[Tauri invoke: get_activity_summary]
    │
    ▼
┌─ B2.1 Проверка доступа ──────────────────────────┐
│  ~/Library/Application Support/Knowledge/          │
│    knowledgeC.db                                    │ ──→ [FS: macOS Screen Time DB]
│  if !exists → Error "Grant Full Disk Access"       │
└──────┬─────────────────────────────────────────────┘
       │ db_path
       ▼
┌─ B2.2 Открытие БД ───────────────────────────────┐
│  Connection::open_with_flags(                      │
│    SQLITE_OPEN_READ_ONLY | NO_MUTEX)               │ ──→ [DB: knowledgeC.db]
│  Ошибка доступа → user-friendly message            │
└──────┬─────────────────────────────────────────────┘
       │ conn
       ▼
┌─ B2.3 SQL-запрос ────────────────────────────────┐
│  SELECT app_name, bundle_id, SUM(duration)/60     │
│  FROM ZOBJECT JOIN ZSOURCE                         │ ──→ [DB: knowledgeC.db]
│  WHERE DATE(today) AND stream='/app/inFocus'       │
│  GROUP BY bundle_id ORDER BY minutes DESC          │
│  → Vec<AppRow> с classify_app() для каждой строки  │ ──→ [Fn: classify_app]
└──────┬─────────────────────────────────────────────┘
       │ Vec<AppRow>
       ▼
┌─ B2.4 Агрегация ────────────────────────────────┐
│  for r in rows:                                    │
│    match category → productive += / distraction += │
│    / neutral +=                                    │
└──────┬─────────────────────────────────────────────┘
       │ (productive, distraction, neutral)
       ▼
┌─ B2.5 Форматирование ───────────────────────────┐
│  top_apps = rows.take(5).format()                  │
│  return "Productive: X | Distraction: Y |          │
│    Neutral: Z\n\nTop apps:\n..."                   │
└──────────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.2 | Открытие БД | Full Disk Access требуется но ошибка не всегда понятна пользователю | Добавить проверку TCC.db или guided setup | Low |
| B2.3 | SQL-запрос | Использует Core Data timestamp (+ 978307200) — хрупко при изменениях macOS | Документировать зависимость от формата knowledgeC.db | Low |
