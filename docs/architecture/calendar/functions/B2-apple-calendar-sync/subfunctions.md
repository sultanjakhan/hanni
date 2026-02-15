# B2 — Синхронизация Apple Calendar: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Синхронизация Apple Calendar |
| Файл(ы) | lib.rs:L3872-4031 |
| LOC | 160 |
| Подфункций | 7 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Вычисление границ месяца | L3875-3883 | 9 | `month, year` → `prefix, last_day` | — | transform | Trivial |
| B2.2 | Генерация AppleScript | L3885-3928 | 44 | `year, month, last_day` → `script: String` | — | transform | Medium |
| B2.3 | Проверка доступа к Calendar.app | L3930-3937 | 8 | — → `bool` (доступ есть/нет) | — | validation | Trivial |
| B2.4 | Выполнение AppleScript | L3939-3948 | 10 | `script` → `output: String` | Запуск osascript (shell) | external | Low |
| B2.5 | Очистка старых событий | L3949-3956 | 8 | `prefix` → — | DELETE FROM events (DB) | persist | Trivial |
| B2.6 | Парсинг и вставка событий | L3958-3976 | 19 | `output lines` → `count: i32` | INSERT INTO events (DB) | parse + persist | Medium |
| B2.7 | Возврат результата | L3978-3978 | 1 | `count` → `JSON { synced, source }` | — | transform | Trivial |

### Вспомогательная функция: parse_ics_datetime (L3982-4031)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.8 | Парсинг all-day дат | L3986-3991 | 6 | `line: &str` → `Option<(NaiveDate, None, true)>` | — | parse | Trivial |
| B2.9 | Парсинг datetime компонентов | L3993-4005 | 13 | `line: &str` → `NaiveDateTime` | — | parse | Trivial |
| B2.10 | Конвертация часовых поясов | L4007-4028 | 22 | `line, naive_dt` → `local_dt: NaiveDateTime` | — | transform | Low |
| B2.11 | Формирование результата | L4030-4031 | 2 | `local_dt` → `(NaiveDate, Option<NaiveTime>, false)` | — | transform | Trivial |

## Data Flow

```
[Input: month, year, db]
    │
    ▼
┌─ B2.1 Вычисление границ ─┐
│  prefix = "YYYY-MM"        │
│  last_day = 28/29/30/31    │
└──────┬────────────────────┘
       │ prefix, last_day
       ▼
┌─ B2.2 Генерация AppleScript ─┐
│  Шаблон скрипта с датами       │
│  startD/endD + repeat по       │
│  календарям и событиям         │
└──────┬────────────────────────┘
       │ script
       ▼
┌─ B2.3 Проверка доступа ─┐
│  check_calendar_access()  │
│  Нет → return error JSON  │
└──────┬───────────────────┘
       │ ok
       ▼
┌─ B2.4 Выполнение AppleScript ─┐
│  run_osascript(&script)         │ ──→ [System: osascript]
│  Ошибка → return error JSON    │
└──────┬─────────────────────────┘
       │ output (text)
       ▼
┌─ B2.5 Очистка старых событий ─┐
│  DELETE FROM events              │ ──→ [DB: events]
│  WHERE source='apple' AND        │
│  date LIKE prefix%               │
└──────┬───────────────────────────┘
       │
       ▼
┌─ B2.6 Парсинг + вставка ─┐
│  for line in output.lines()│
│  split("||") → 7 полей    │
│  INSERT INTO events        │ ──→ [DB: events]
│  count += 1                │
└──────┬────────────────────┘
       │ count
       ▼
┌─ B2.7 Возврат результата ─┐
│  json!({ synced, source }) │
└────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.2 | Генерация AppleScript | 44 строки AppleScript в строковом литерале Rust — трудно тестировать и отлаживать | Вынести AppleScript в отдельный файл-ресурс | Low |
| B2.6 | Парсинг и вставка | Парсинг и запись в БД совмещены в одном цикле — нарушение SRP | Разделить на парсинг строк → Vec<Event> и batch insert | Low |
