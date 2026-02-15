# B4 — Синхронизация Google Calendar ICS: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Синхронизация Google Calendar (ICS) |
| Файл(ы) | lib.rs:L4221-4329 |
| LOC | 109 |
| Подфункций | 7 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B4.1 | Валидация URL | L4222-4223 | 2 | `url: String` → ok / error | — | validation | Trivial |
| B4.2 | HTTP-запрос ICS файла | L4225-4233 | 9 | `url` → `body: String` | HTTP GET (сеть) | external | Low |
| B4.3 | Очистка старых google-событий | L4235-4243 | 9 | `prefix, db` → — | DELETE FROM events (DB) | persist | Trivial |
| B4.4 | Парсинг VEVENT блоков | L4247-4272 | 26 | `block: &str` → `summary, dtstart, dtend, uid, desc` | — | parse | Low |
| B4.5 | Вычисление длительности | L4276-4285 | 10 | `start_time, end_time, start_date, end_date, is_allday` → `dur: i64` | — | transform | Trivial |
| B4.6 | Расширение RRULE и сбор дат | L4287-4307 | 21 | `start_date, block, prefix, year, month` → `dates_to_insert: Vec<NaiveDate>` | — | process | Low |
| B4.7 | Вставка вхождений в БД | L4309-4325 | 17 | `dates_to_insert, summary, desc, time_str, dur, uid, now` → `count += N` | INSERT INTO events (DB) | persist | Low |

## Data Flow

```
[Input: url, month, year, db]
    │
    ▼
┌─ B4.1 Валидация URL ─┐
│  url.is_empty() → Err  │
└──────┬────────────────┘
       │
       ▼
┌─ B4.2 HTTP GET ICS ──────┐
│  reqwest::Client::get(url) │ ──→ [External: Google ICS endpoint]
│  timeout 15s                │
│  .text() → body             │
└──────┬─────────────────────┘
       │ body
       ▼
┌─ B4.3 Очистка старых событий ─┐
│  DELETE FROM events             │ ──→ [DB: events]
│  WHERE source='google'          │
│  AND date LIKE prefix%          │
└──────┬─────────────────────────┘
       │
       ▼
┌─ for block in body.split("BEGIN:VEVENT") ─┐
│                                            │
│  ┌─ B4.4 Парсинг VEVENT ─────────┐       │
│  │  get_field("SUMMARY:")          │       │
│  │  DTSTART/DTEND lines            │       │
│  │  parse_ics_datetime()           │       │
│  └──────┬─────────────────────────┘       │
│         │                                  │
│  ┌─ B4.5 Вычисление длительности ─┐      │
│  │  start_mins, end_mins, day_diff  │      │
│  └──────┬──────────────────────────┘      │
│         │                                  │
│  ┌─ B4.6 Расширение RRULE ───────┐       │
│  │  parse_rrule(block)             │       │
│  │  parse_exdates(block)           │       │
│  │  expand_rrule() → dates         │       │
│  │  dedup                          │       │
│  └──────┬─────────────────────────┘       │
│         │                                  │
│  ┌─ B4.7 Вставка в БД ──────────┐        │
│  │  INSERT INTO events            │ ──→ [DB: events]
│  │  count += 1 per occurrence     │        │
│  └──────┬────────────────────────┘        │
│         │                                  │
└─────────┴──────────────────────────────────┘
       │ count
       ▼
[Output: json!({ synced: count, source: "google" })]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B4.4 | Парсинг VEVENT | Замыкание `get_field` внутри цикла — нет переиспользования | Вынести в отдельную утилитную функцию `get_ics_field(block, field)` | Low |
| B4.7 | Вставка в БД | Каждое вхождение — отдельный INSERT (нет batch/transaction) | Обернуть в транзакцию для производительности | Low |
