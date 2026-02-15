# B7 — CRUD transactions: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B7: CRUD transactions |
| Файл(ы) | lib.rs:L5205-5279 |
| LOC | 75 |
| Подфункций | 4 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B7.1 | Создание транзакции | L5205-L5222 | 18 | `(date?, transaction_type, amount, currency?, category, description?, recurring?, recurring_period?, db)` → `Result<i64>` | DB INSERT transactions | persist | Low |
| B7.2 | Получение транзакций с фильтрами | L5224-L5242 | 19 | `(month?, transaction_type?, db)` → `Result<Vec<Value>>` | DB SELECT transactions (LIKE по месяцу + опц. фильтр типа) | query | Low |
| B7.3 | Маппинг строки транзакции в JSON + удаление | L5244-L5258 | 15 | `Row` → `Value` / `(id, db)` → `Result<()>` | DB DELETE transactions | transform + persist | Trivial |
| B7.4 | Статистика транзакций за месяц | L5260-L5279 | 20 | `(month?, db)` → `Result<Value>` | DB SELECT SUM (expense/income) + GROUP BY category | query | Low |

## Data Flow

```
[add_transaction]
  date?, type, amount, currency?, category, ... → defaults → INSERT transactions → rowid

[get_transactions]
  month?, transaction_type?
      │
      ▼
  pattern = "YYYY-MM%"
      ├── Some(type) → SELECT WHERE date LIKE AND type=type
      └── None → SELECT WHERE date LIKE
      │
      ▼
  tx_from_row() × N → Vec<Value>

[get_transaction_stats]
  month?
      │
      ▼
  pattern = "YYYY-MM%"
      │
      ├── SUM(CASE expense) → total_expense
      ├── SUM(CASE income) → total_income
      │
      ▼
  SELECT category, SUM(amount) GROUP BY category (расходы)
      │
      ▼
  {total_expense, total_income, balance, by_category[]}
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B7.2 | Получение транзакций | Дублирование кода: две ветки if/else отличаются только наличием фильтра `type` — можно объединить через динамическое построение SQL | Рефакторинг: динамический SQL builder (аналогично get_contacts) | Low |
