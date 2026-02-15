# B3 — User Lists: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: User lists |
| Файл(ы) | lib.rs:L4911-4983 |
| LOC | 73 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

### create_user_list (L4911-4920)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.1 | Создание пользовательского списка | L4911-4920 | 10 | `name, description?, color?` → `id: i64` | INSERT INTO user_lists (DB) | persist | Trivial |

### get_user_lists (L4922-4940)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.2 | Получение списков с подсчётом элементов | L4922-4940 | 19 | — → `Vec<Value>` (id, name, description, color, item_count) | SELECT FROM user_lists + subquery list_items (DB read) | query | Low |

### add_to_list (L4942-4951)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.3 | Добавление элемента в список | L4942-4951 | 10 | `list_id, media_item_id` → `()` | INSERT OR IGNORE INTO list_items (DB) | persist | Trivial |

### remove_from_list (L4953-4961)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.4 | Удаление элемента из списка | L4953-4961 | 9 | `list_id, media_item_id` → `()` | DELETE FROM list_items (DB) | persist | Trivial |

### get_list_items (L4963-4982)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.5 | Получение элементов списка с JOIN | L4963-4982 | 20 | `list_id` → `Vec<Value>` (id, media_type, title, status, rating, cover_url) | SELECT FROM list_items JOIN media_items (DB read) | query | Low |

## Проблемные зоны

Нет критичных проблем.
