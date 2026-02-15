# F2 — Media List by Type: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: Media list by type |
| Файл(ы) | main.js:L4635-4680 |
| LOC | 46 |
| Подфункций | 4 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Загрузка медиа-элементов | L4635-4639 | 5 | `mediaType, mediaStatusFilter` → `invoke('get_media_items')` → `items[]` | bridge | Trivial |
| F2.2 | Рендер фильтров статуса и колонок | L4641-4658 | 18 | `mediaType, hasEp` → `filterBar HTML, fixedColumns[]` (title, status, rating, progress?, year) | render + transform | Low |
| F2.3 | Рендер DatabaseView | L4660-4673 | 14 | `items[], fixedColumns, mediaType` → `renderDatabaseView()` с onAdd/onRowClick/reloadFn | render | Trivial |
| F2.4 | Привязка обработчиков фильтров | L4675-4677 | 3 | `.dev-filters .pill` click → `mediaStatusFilter = filter` → `loadMediaList()` | interaction | Trivial |

## Проблемные зоны

Нет критичных проблем.
