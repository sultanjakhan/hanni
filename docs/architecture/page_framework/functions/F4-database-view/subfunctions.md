# F4 — Database view framework: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F4: Database view framework |
| Файл(ы) | main.js:L3976-4202 |
| LOC | 227 |
| Подфункций | 10 |
| Сложность (max) | High |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F4.1 | Загрузка property definitions и values | L3976-3995 | 20 | renderDatabaseView() → invoke('get_property_definitions') + invoke('get_property_values') | bridge | Low |
| F4.2 | Построение карты значений | L3990-3995 | 6 | allValues[] → valuesMap: { recordId: { propertyId: value } } | transform | Trivial |
| F4.3 | Применение фильтров | L3997-4001 | 5 | records + valuesMap + dbvFilters → filteredRecords | process | Trivial |
| F4.4 | Рендер заголовка таблицы (header + thead) | L4003-4041 | 39 | fixedColumns + visibleProps → DOM: headerHtml + table с thead | render | Medium |
| F4.5 | Рендер строк таблицы (tbody) | L4015-4041 | 27 | filteredRecords + fixedColumns + visibleProps + valuesMap → tbody HTML | render | Low |
| F4.6 | Рендер filter bar | L4043-4046 | 4 | customProps.length > 0 → renderFilterBar() | render | Trivial |
| F4.7 | Привязка обработчиков строк | L4048-4058 | 11 | onRowClick → click handler на каждой .data-table-row | interaction | Trivial |
| F4.8 | Привязка inline-редактирования | L4061-4067 | 7 | cell-editable click → startInlineEdit() | interaction | Trivial |
| F4.9 | Привязка заголовков колонок (add prop, context menu, sort) | L4069-4102 | 34 | click #dbv-add-prop-col / .prop-header / .sortable-header → showAddPropertyModal / showColumnMenu / sortDatabaseView | interaction | Low |
| F4.10 | Утилиты: formatPropValue + startInlineEdit | L4105-4202 | 98 | propType + value → formatted HTML; cell click → inline editor (select/multi_select/checkbox/date/number/text) → invoke('set_property_value') | render + interaction | High |

## Data Flow

```
[Input: el, tabId, recordTable, records[], options]
    │
    ▼
┌─ F4.1 Загрузка метаданных ──────┐
│  invoke('get_property_definitions') │ ──→ [Backend: property_definitions]
│  invoke('get_property_values')      │ ──→ [Backend: property_values]
└──────┬──────────────────────────────┘
       │ customProps, allValues
       ▼
┌─ F4.2 Карта значений ───────────┐
│  allValues → valuesMap             │
│  { recordId: { propId: value } }   │
└──────┬──────────────────────────────┘
       │ valuesMap
       ▼
┌─ F4.3 Фильтрация ───────────────┐
│  loadFiltersFromViewConfig(tabId)  │ ──→ [Backend: view_configs]
│  applyFilters(records, valuesMap,  │
│    filters, idField)               │
└──────┬──────────────────────────────┘
       │ filteredRecords
       ▼
┌─ F4.4 + F4.5 Рендер таблицы ────┐
│  headerHtml (add button)           │
│  <thead>: fixed cols + custom cols │
│    + add-prop-col                  │
│  <tbody>: fixedColumns.render() +  │
│    formatPropValue() per cell      │ ──→ [DOM: el.innerHTML]
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F4.6 Filter bar ───────────────┐
│  renderFilterBar(el, tabId,        │ ──→ [DOM: prepend .filter-bar]
│    customProps, reloadFn)          │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F4.7-F4.9 Event binding ───────┐
│  row click → onRowClick(record)    │
│  cell click → startInlineEdit()    │
│  + col → showAddPropertyModal()    │
│  prop header → showColumnMenu()    │
│  fixed header → sortDatabaseView() │
└──────────────────────────────────────┘

┌─ F4.10 startInlineEdit ─────────┐
│  cell click → switch propType:     │
│    select → <select> editor        │
│    multi_select → checkboxes       │
│    checkbox → toggle immediately   │ ──→ invoke('set_property_value')
│    date/number/text → <input>      │
│  blur/Enter → save + reloadFn()    │ ──→ [Backend: property_values]
└──────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F4.10 | startInlineEdit | 98 LOC — слишком крупная, смешивает 6 типов редакторов | Вынести каждый тип в отдельный helper: createSelectEditor(), createMultiSelectEditor(), etc. | Medium |
| F4.4+F4.5 | Рендер таблицы | HTML строится через конкатенацию строк — хрупко при изменениях | Рассмотреть шаблонный подход или DOM API | Low |
| F4.10 | formatPropValue | Отсутствие экранирования для некоторых типов (url — XSS через escapeHtml, но ссылка может быть javascript:) | Валидировать URL-протокол | Low |
