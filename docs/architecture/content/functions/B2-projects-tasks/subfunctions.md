# B2 — CRUD Projects and Tasks: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: CRUD projects and tasks |
| Файл(ы) | lib.rs:L4330-4408 |
| LOC | 79 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

### create_project (L4332-4341)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Создание проекта | L4332-4341 | 10 | `name, description, color` → `id: i64` | INSERT INTO projects (DB) | persist | Trivial |

### get_projects (L4343-4362)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.2 | Получение активных проектов с количеством задач | L4343-4362 | 20 | — → `Vec<Value>` (id, name, description, status, color, task_count) | SELECT FROM projects + subquery tasks (DB read) | query | Low |

### create_task (L4364-4373)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.3 | Создание задачи | L4364-4373 | 10 | `project_id, title, description, priority, due_date?` → `id: i64` | INSERT INTO tasks (DB) | persist | Trivial |

### get_tasks (L4375-4394)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.4 | Получение задач проекта | L4375-4394 | 20 | `project_id` → `Vec<Value>` (id, title, description, status, priority, due_date, completed_at) | SELECT FROM tasks (DB read) | query | Low |

### update_task_status (L4396-4406)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.5 | Обновление статуса задачи | L4396-4406 | 11 | `id, status` → `()` (если "done" → completed_at = now) | UPDATE tasks (DB) | persist | Trivial |

## Проблемные зоны

Нет критичных проблем.
