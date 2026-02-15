# F3 — Learning courses/skills: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F3: Learning (courses/skills) |
| Файл(ы) | main.js:L3890-3975 |
| LOC | 86 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

### loadDevelopment (L3890-3901)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.1 | Загрузка данных обучения | L3890-3901 | 12 | — → `invoke('get_learning_items', { typeFilter })` → renderDevelopment(pc, items) | bridge | Trivial |

### renderDevelopment (L3903-3936)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.2 | Определение фильтров и колонок | L3903-3918 | 16 | — → `filters[], filterLabels, statusLabels, statusColors, fixedColumns[]` | transform | Low |
| F3.3 | Рендер фильтров и DatabaseView | L3920-3931 | 12 | `items[], fixedColumns` → `filterBar HTML` + `renderDatabaseView()` | render | Trivial |
| F3.4 | Привязка обработчиков фильтров | L3933-3936 | 4 | `.dev-filters .pill` click → `devFilter = filter` → `loadDevelopment()` | interaction | Trivial |

### showAddLearningModal (L3938-3972)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.5 | Модальное окно добавления | L3938-3972 | 35 | — → DOM overlay (тип, название, описание, URL) → `invoke('create_learning_item')` → `loadDevelopment()` | render + interaction + bridge | Medium |

## Проблемные зоны

Нет критичных проблем.
