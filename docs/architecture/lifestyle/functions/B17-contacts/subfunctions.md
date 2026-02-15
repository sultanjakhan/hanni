# B17 — CRUD contacts: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B17: CRUD contacts |
| Файл(ы) | lib.rs:L5800-5902 |
| LOC | 103 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B17.1 | Создание контакта | L5800-L5818 | 19 | `(name, phone?, email?, category?, relationship?, notes?, blocked?, block_reason?, db)` → `Result<i64>` | DB INSERT contacts | persist | Low |
| B17.2 | Получение контактов с динамическими фильтрами | L5820-L5854 | 35 | `(category?, blocked?, db)` → `Result<Value>` | DB SELECT contacts (dynamic SQL) | query | Medium |
| B17.3 | Обновление контакта (поля по отдельности) | L5856-L5881 | 26 | `(id, name?, phone?, email?, category?, relationship?, notes?, blocked?, block_reason?, favorite?, db)` → `Result<String>` | DB UPDATE contacts (до 9 отдельных UPDATE) | persist | Low |
| B17.4 | Удаление контакта | L5883-L5888 | 6 | `(id, db)` → `Result<String>` | DB DELETE contacts | persist | Trivial |
| B17.5 | Toggle блокировки и избранного | L5890-L5902 | 13 | `(id, db)` → `Result<String>` | DB UPDATE contacts (CASE WHEN toggle) | persist | Trivial |

## Data Flow

```
[add_contact]
  name, phone?, ... → defaults → INSERT contacts → rowid

[get_contacts]
  category?, blocked?
      │
      ▼
  Динамическое построение SQL:
    base: "SELECT ... FROM contacts WHERE 1=1"
      ├── +category → AND category=?N
      ├── +blocked → AND blocked=?N
      └── ORDER BY favorite DESC, name ASC
      │
      ▼
  query_map → Vec<{id, name, phone, email, ..., favorite, blocked, ...}>

[update_contact]
  id, name?, phone?, ... (9 Optional полей)
      │
      ▼
  for each Some(field):
    UPDATE contacts SET field=val, updated_at=now WHERE id=id
  (до 9 отдельных UPDATE-запросов)

[toggle_contact_blocked / toggle_contact_favorite]
  id → UPDATE SET field = CASE WHEN 1 THEN 0 ELSE 1 END
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B17.3 | Обновление контакта | До 9 отдельных UPDATE-запросов вместо одного — N roundtrips к БД | Собрать SET-клаузу динамически и выполнить один UPDATE | Medium |
| B17.2 | Получение контактов | `Box<dyn ToSql>` — хорошо, но маппинг 12 полей в JSON вручную — хрупко | Рассмотреть макрос или derive для сериализации | Low |
