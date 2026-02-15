# B12 — Journal (diary): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B12: Journal (diary) |
| Файл(ы) | lib.rs:L5468-5522 |
| LOC | 55 |
| Подфункций | 3 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B12.1 | Сохранение записи дневника (upsert) | L5468-L5485 | 18 | `(date?, mood, energy, stress, gratitude?, reflection?, wins?, struggles?, db)` → `Result<i64>` | DB INSERT ... ON CONFLICT UPDATE journal_entries | persist | Low |
| B12.2 | Получение записей за период | L5487-L5505 | 19 | `(period?, db)` → `Result<Vec<Value>>` | DB SELECT journal_entries WHERE date >= since | query | Low |
| B12.3 | Получение записи за конкретную дату | L5507-L5522 | 16 | `(date, db)` → `Result<Value>` | DB SELECT journal_entries WHERE date=date | query | Low |

## Data Flow

```
[save_journal_entry]
  date?, mood, energy, stress, gratitude?, ...
      │
      ▼
  date = date ∪ today
      │
      ▼
  INSERT ... ON CONFLICT(date) DO UPDATE
  (upsert: одна запись на дату)
      │
      ▼
  rowid

[get_journal_entries]
  period? (default=30)
      │
      ▼
  since = now - period days
      │
      ▼
  SELECT WHERE date >= since ORDER BY date DESC
      │
      ▼
  Vec<{id, date, mood, energy, stress, gratitude, reflection, wins, struggles}>

[get_journal_entry]
  date → SELECT WHERE date=date → single Value
```

## Проблемные зоны

Нет критичных проблем. Паттерн UPSERT (ON CONFLICT) — хорошее решение для единственной записи в день.
