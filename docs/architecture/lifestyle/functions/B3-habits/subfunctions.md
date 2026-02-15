# B3 — CRUD habits: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: CRUD habits |
| Файл(ы) | lib.rs:L4613-4671 |
| LOC | 59 |
| Подфункций | 3 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.1 | Создание привычки | L4613-L4622 | 10 | `(name, icon, frequency, db)` → `Result<i64>` | DB INSERT habits | persist | Trivial |
| B3.2 | Toggle отметки привычки | L4624-L4645 | 22 | `(habit_id, date?, db)` → `Result<()>` | DB SELECT+DELETE/INSERT habit_checks | persist | Low |
| B3.3 | Список привычек на сегодня с streak | L4647-L4671 | 25 | `(db)` → `Result<Vec<Value>>` | DB SELECT habits + habit_checks (subquery) | query | Low |

## Data Flow

```
[create_habit]
  name, icon, frequency → INSERT habits → rowid

[check_habit]
  habit_id, date?
      │
      ▼
  SELECT habit_checks (существует?)
      ├── да → DELETE habit_checks (toggle off)
      └── нет → INSERT habit_checks (toggle on)

[get_habits_today]
  today = now()
      │
      ▼
  SELECT habits + subquery checked + subquery streak_approx
      │
      ▼
  Vec<{id, name, icon, frequency, completed, streak}>
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B3.3 | Список привычек | streak_approx считает просто COUNT за 30 дней, а не реальный streak (последовательные дни) | Реализовать точный подсчёт streak через рекурсивный CTE или в коде | Low |
