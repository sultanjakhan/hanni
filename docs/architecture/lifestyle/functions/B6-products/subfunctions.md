# B6 — CRUD products (expiry): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B6: CRUD products (expiry) |
| Файл(ы) | lib.rs:L5123-5201 |
| LOC | 79 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B6.1 | Добавление продукта | L5123-L5139 | 17 | `(name, category?, quantity?, unit?, expiry_date?, location?, notes?, db)` → `Result<i64>` | DB INSERT products | persist | Low |
| B6.2 | Получение списка продуктов | L5141-L5157 | 17 | `(location?, db)` → `Result<Vec<Value>>` | DB SELECT products (с фильтром или без) | query | Low |
| B6.3 | Маппинг строки продукта в JSON | L5159-L5166 | 8 | `Row` → `Result<Value>` | — | transform | Trivial |
| B6.4 | Обновление продукта | L5168-L5180 | 13 | `(id, quantity?, expiry_date?, notes?, db)` → `Result<()>` | DB SELECT (текущие данные) + UPDATE products | persist | Trivial |
| B6.5 | Получение истекающих продуктов | L5189-L5201 | 13 | `(days?, db)` → `Result<Vec<Value>>` | DB SELECT products WHERE expiry_date <= deadline | query | Trivial |

_Примечание: `delete_product` (L5182-L5187, 6 LOC) — тривиальная однострочная DELETE-обёртка, включена в общий диапазон._

## Подфункция B6.4 уточнение

`update_product` сначала читает текущие значения (SELECT), затем обновляет только переданные поля — паттерн partial update с fallback на текущие значения.

## Data Flow

```
[add_product]
  name, category?, ... → defaults → INSERT products → rowid

[get_products]
  location?
      ├── Some(loc) → SELECT WHERE location=loc
      └── None → SELECT ALL
      │
      ▼
  product_from_row() × N → Vec<Value>

[update_product]
  id, quantity?, expiry_date?, notes?
      │
      ▼
  SELECT текущие (cur_qty, cur_exp, cur_notes)
      │
      ▼
  UPDATE SET (new ∪ cur) WHERE id

[get_expiring_products]
  days? (default=3) → deadline = now + days
      │
      ▼
  SELECT WHERE expiry_date <= deadline ORDER BY expiry_date
```

## Проблемные зоны

Нет критичных проблем. Код чистый и линейный. Хелпер `product_from_row` переиспользуется в B6.2 и B6.5.
