# B1 — CRUD Notes: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: CRUD notes |
| Файл(ы) | lib.rs:L3727-3828 |
| LOC | 102 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

### create_note (L3727-3736)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.1 | Создание заметки | L3727-3736 | 10 | `title, content, tags` → `id: i64` | INSERT INTO notes (DB) | persist | Trivial |

### update_note (L3738-3758)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.2 | Чтение текущих значений pinned/archived | L3744-3750 | 7 | `id` → `cur_pinned, cur_archived` | SELECT FROM notes (DB read) | query | Trivial |
| B1.3 | Обновление заметки | L3751-3758 | 8 | `id, title, content, tags, pinned?, archived?` → `()` | UPDATE notes (DB) | persist | Trivial |

### delete_note (L3760-3766)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.4 | Удаление заметки | L3760-3766 | 7 | `id` → `()` | DELETE FROM notes (DB) | persist | Trivial |

### get_notes (L3768-3792)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.5 | Полнотекстовый поиск заметок | L3768-3792 | 25 | `filter?, search?` → `Vec<Value>` | SELECT FROM notes_fts JOIN notes (DB read) | query | Low |

### get_notes_all + note_from_row (L3794-3815)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.6 | Получение всех заметок (без фильтра) | L3794-3802 | 9 | `conn` → `Vec<Value>` | SELECT FROM notes (DB read) | query | Trivial |
| B1.7 | Маппинг строки БД в JSON | L3804-3815 | 12 | `rusqlite::Row` → `serde_json::Value` | — | transform | Trivial |

### get_note (L3817-3825)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.8 | Получение одной заметки по ID | L3817-3825 | 9 | `id` → `Value` | SELECT FROM notes (DB read) | query | Trivial |

## Проблемные зоны

Нет критичных проблем.
