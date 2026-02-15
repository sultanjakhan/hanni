# B6 — CRUD properties (definitions+values): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B6: CRUD properties (definitions+values) |
| Файл(ы) | lib.rs:L5994-6105 |
| LOC | 112 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B6.1 | Получение определений свойств | L5994-6014 | 21 | tab_id → Vec<JSON> (id, name, type, position, color, options, default_value, visible) | DB read: property_definitions | query | Low |
| B6.2 | Создание определения свойства | L6016-6030 | 15 | tab_id, name, prop_type, position?, color?, options?, default_value? → i64 (new id) | DB write: property_definitions INSERT | persist | Low |
| B6.3 | Обновление определения свойства | L6032-6042 | 11 | id, name?, prop_type?, position?, color?, options?, visible? → () | DB write: property_definitions UPDATE (до 6 отдельных) | persist | Trivial |
| B6.4 | Удаление определения свойства | L6044-6051 | 8 | id → () | DB write: DELETE property_values + property_definitions | persist | Trivial |
| B6.5 | Получение значений свойств | L6053-6081 | 29 | record_table, record_ids[] → Vec<JSON> (id, record_id, property_id, value, prop_name, prop_type) | DB read: property_values JOIN property_definitions | query | Low |
| B6.6 | Установка значения свойства | L6083-6093 | 11 | record_id, record_table, property_id, value? → () | DB write: property_values UPSERT | persist | Trivial |
| B6.7 | Удаление значения свойства | L6095-6103 | 9 | record_id, record_table, property_id → () | DB write: DELETE property_values | persist | Trivial |

## Data Flow

```
[Frontend] ──invoke──→ B6.1 get_property_definitions(tab_id) ──SELECT──→ [DB: property_definitions]
[Frontend] ──invoke──→ B6.2 create_property_definition(...) ──INSERT──→ [DB: property_definitions]
[Frontend] ──invoke──→ B6.3 update_property_definition(...) ──UPDATE──→ [DB: property_definitions]
[Frontend] ──invoke──→ B6.4 delete_property_definition(id)  ──DELETE──→ [DB: property_values + property_definitions]
[Frontend] ──invoke──→ B6.5 get_property_values(table, ids)  ──SELECT JOIN──→ [DB: property_values ⟕ property_definitions]
[Frontend] ──invoke──→ B6.6 set_property_value(...)          ──UPSERT──→ [DB: property_values]
[Frontend] ──invoke──→ B6.7 delete_property_value(...)       ──DELETE──→ [DB: property_values]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B6.3 | Обновление определения | До 6 отдельных UPDATE-запросов вместо одного составного | Собрать SET-клаузы динамически для одного запроса | Low |
| B6.5 | Получение значений | Динамический SQL с IN-плейсхолдерами — уязвим при больших списках record_ids | Ограничить максимальный размер или использовать batch | Low |
