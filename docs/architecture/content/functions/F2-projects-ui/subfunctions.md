# F2 — Projects and Tasks UI: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: Projects and tasks (UI) |
| Файл(ы) | main.js:L3815-3889 |
| LOC | 75 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

### loadWork (L3815-3826)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Загрузка проектов | L3815-3826 | 12 | — → `invoke('get_projects')` → renderWork(pc, projects) | bridge | Trivial |

### renderWork (L3828-3887)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.2 | Загрузка задач и рендер layout | L3828-3846 | 19 | `projects[]` → `invoke('get_tasks', {projectId})` → `el.innerHTML` (two-column layout: projects + tasks) | bridge + render | Low |
| F2.3 | Рендер списка проектов | L3848-3858 | 11 | `projects[]` → DOM items в `#work-projects-list` (name, dot color, task count, click → switch project) | render | Trivial |
| F2.4 | Рендер списка задач | L3860-3876 | 17 | `tasks[]` → DOM items в `#work-tasks-list` (checkbox, title, priority badge, click → toggle status) | render + interaction | Low |
| F2.5 | Обработчик создания проекта | L3878-3881 | 4 | `#new-project-btn` click → `prompt()` → `invoke('create_project')` → `loadWork()` | interaction + bridge | Trivial |
| F2.6 | Обработчик создания задачи | L3883-3886 | 4 | `#new-task-btn` click → `prompt()` → `invoke('create_task')` → `loadWork()` | interaction + bridge | Trivial |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F2.5-F2.6 | Создание проекта/задачи | Используется `prompt()` — примитивный UX, нет возможности ввести описание, цвет, приоритет | Заменить на модальное окно с полной формой | Low |
