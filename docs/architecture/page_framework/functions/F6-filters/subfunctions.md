# F6 — Filters and sorting: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F6: Filters and sorting |
| Файл(ы) | main.js:L4429-4597 |
| LOC | 169 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F6.1 | Сортировка database view | L4429-4444 | 16 | sortDatabaseView(sortKey, dir) → sort records by key (prop or fixed) → re-render | process | Low |
| F6.2 | Рендер filter bar (chips) | L4451-4481 | 31 | renderFilterBar(el, tabId, customProps, onApply) → DOM: filter chips + "Add Filter" button + remove handlers | render | Low |
| F6.3 | Модал построения фильтра | L4483-4551 | 69 | showFilterBuilderModal() → modal: property select, condition select, value input → dbvFilters.push + save + onApply | interaction | Medium |
| F6.4 | Динамическое обновление value input | L4513-4536 | 24 | change property/condition → hide value (empty/not_empty) или switch input to select (select/multi_select type) | render | Low |
| F6.5 | Применение фильтров к записям | L4553-4569 | 17 | applyFilters(records, valuesMap, filters, idField) → filteredRecords (eq, neq, contains, empty, not_empty) | process | Low |
| F6.6 | Сохранение фильтров в view config | L4571-4582 | 12 | saveFiltersToViewConfig(tabId) → get/create view_config → update filter_json | persist | Trivial |
| F6.7 | Загрузка фильтров из view config | L4584-4591 | 8 | loadFiltersFromViewConfig(tabId) → invoke('get_view_configs') → dbvFilters[tabId] | query | Trivial |

## Data Flow

```
[Input: user click "Add Filter"]
    │
    ▼
┌─ F6.3 Filter Builder Modal ─────┐
│  select property, condition, value │
│  click Apply →                     │
│    dbvFilters[tabId].push(filter)  │
│    → F6.6 saveFiltersToViewConfig  │ ──→ [Backend: view_configs]
│    → onApply() → re-render view    │
└──────────────────────────────────────┘

[Input: renderDatabaseView загружает фильтры]
    │
    ▼
┌─ F6.7 loadFiltersFromViewConfig ─┐
│  invoke('get_view_configs')        │ ──→ [Backend: view_configs]
│  parse filter_json →               │
│    dbvFilters[tabId] = [...]       │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F6.5 applyFilters ─────────────┐
│  records.filter(r =>               │
│    filters.every(f =>              │
│      match condition: eq/neq/      │
│        contains/empty/not_empty))  │
│  → filteredRecords                 │
└──────────────────────────────────────┘

[Input: click sortable header / context menu sort]
    │
    ▼
┌─ F6.1 sortDatabaseView ─────────┐
│  [...records].sort((a,b) =>        │
│    prop_ prefix → valuesMap lookup │
│    else → direct field access      │
│    number → arithmetic compare     │
│    string → localeCompare          │
│  → renderDatabaseView(sorted)      │
└──────────────────────────────────────┘

[Input: filter bar rendered]
    │
    ▼
┌─ F6.2 Filter chips ─────────────┐
│  filters.map → chip with label     │
│  click remove → splice(idx) →      │
│    F6.6 save → onApply()           │
└──────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F6.3 | Модал фильтра | 69 LOC — смешивает рендер модала, обработчики, динамическое обновление value input | Вынести F6.4 (updateValueInput) как отдельную вызываемую функцию вне модала | Low |
| F6.6 | Сохранение фильтров | Пустой catch блок — ошибки сохранения молча игнорируются | Добавить логирование или user notification | Low |
| F6.1 | Сортировка | Re-render всей таблицы при каждой сортировке — может быть медленным при большом количестве записей | Сортировать DOM-элементы без полного re-render | Low |
