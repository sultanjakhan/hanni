# B2 — CRUD Media Items: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: CRUD media items |
| Файл(ы) | lib.rs:L4800-4910 |
| LOC | 111 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

### add_media_item (L4801-4822)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Добавление медиа-элемента | L4801-4822 | 22 | `media_type, title, original_title?, year?, description?, cover_url?, status?, rating?, progress?, total_episodes?, notes?` → `id: i64` | INSERT INTO media_items (DB) | persist | Low |

### update_media_item (L4824-4844)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.2 | Чтение текущих значений | L4832-4835 | 4 | `id` → `cur_status, cur_rating, cur_progress, cur_notes, cur_title` | SELECT FROM media_items (DB read) | query | Trivial |
| B2.3 | Обновление медиа-элемента | L4836-4843 | 8 | `id, status?, rating?, progress?, notes?, title?` → `()` (если status="completed" → completed_at = now) | UPDATE media_items (DB) | persist | Trivial |

### delete_media_item (L4846-4853)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.4 | Удаление медиа-элемента | L4846-4853 | 8 | `id` → `()` | DELETE FROM list_items + DELETE FROM media_items (DB) | persist | Trivial |

### get_media_items (L4855-4876)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.5 | Получение медиа-элементов с фильтрацией | L4855-4876 | 22 | `media_type, status?, show_hidden?` → `Vec<Value>` | SELECT FROM media_items (DB read) | query | Low |

### media_from_row (L4878-4893)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.6 | Маппинг строки БД в JSON | L4878-4893 | 16 | `rusqlite::Row` → `serde_json::Value` | — | transform | Low |

### hide_media_item / unhide_media_item (L4895-4909)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.7 | Скрытие/показ медиа-элемента | L4895-4909 | 15 | `id` → `()` | UPDATE media_items SET hidden (DB) | persist | Trivial |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.5 | Получение медиа-элементов | Дублирование SQL-запроса (два почти идентичных prepared statement в ветках if/else) | Динамическая сборка WHERE-условия | Low |
