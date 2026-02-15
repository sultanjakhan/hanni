# F5 — Property management UI: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F5: Property management UI |
| Файл(ы) | main.js:L4213-4428 |
| LOC | 216 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F5.1 | Инициализация модала добавления свойства | L4213-4219 | 7 | showAddPropertyModal(tabId, reloadFn) → state: selectedType='text', optionsList=[], overlay | control | Trivial |
| F5.2 | Рендер содержимого модала | L4220-4255 | 36 | selectedType + optionsList → DOM: name input, type grid, options section (если select/multi_select) | render | Medium |
| F5.3 | Привязка выбора типа | L4257-4266 | 10 | click .prop-type-card → selectedType = card.dataset.type → re-render modal | interaction | Trivial |
| F5.4 | Управление списком опций (add/remove) | L4268-4293 | 26 | click remove tag / click add btn / Enter in input → optionsList.splice/push → re-render | interaction | Low |
| F5.5 | Сохранение нового свойства | L4298-4311 | 14 | click #prop-save → invoke('create_property_definition') → overlay.remove() + reloadFn() | bridge | Trivial |
| F5.6 | Монтирование модала + overlay close + focus | L4313-4316 | 4 | renderModal() → document.body.appendChild → click outside → remove; auto-focus name | interaction | Trivial |
| F5.7 | Инициализация контекстного меню колонки | L4319-4364 | 46 | showColumnMenu(propDef, rect, ...) → DOM: col-context-menu с rename, sort, hide, delete | render | Medium |
| F5.8 | Обработчики контекстного меню | L4366-4427 | 62 | rename (Enter/blur) → invoke('update_property_definition'); sort → sortDatabaseView; hide → update visible; delete → invoke('delete_property_definition') + confirm | interaction | High |

## Data Flow

```
[Input: showAddPropertyModal(tabId, reloadFn)]
    │
    ▼
┌─ F5.1 Инициализация ────────────┐
│  selectedType = 'text'             │
│  optionsList = []                  │
│  overlay = createElement('div')    │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F5.2 renderModal() ────────────┐
│  type grid из PROPERTY_TYPE_DEFS   │
│  options section (если select)     │
│  name input + save/cancel          │ ──→ [DOM: .modal-overlay]
└──────┬──────────────────────────────┘
       │
  ┌────┴────┐
  ▼         ▼
┌─ F5.3 ─┐ ┌─ F5.4 ─────────────┐
│ Type    │ │ Options management   │
│ select  │ │ add/remove → re-     │
│ → re-   │ │ render modal         │
│ render  │ └──────────────────────┘
└─────────┘
       │
       ▼
┌─ F5.5 Save ─────────────────────┐
│  invoke('create_property_         │ ──→ [Backend: property_definitions]
│    definition', { tabId, name,    │
│    propType, options })            │
│  → overlay.remove() + reloadFn()  │
└──────────────────────────────────────┘

[Input: showColumnMenu(propDef, anchorRect, ...)]
    │
    ▼
┌─ F5.7 Рендер меню ──────────────┐
│  rename input + sort/hide/delete   │
│  items → position near anchorRect  │ ──→ [DOM: .col-context-menu]
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F5.8 Обработчики ──────────────┐
│  rename → invoke('update_property_ │ ──→ [Backend: property_definitions]
│    definition', { name })          │
│  sort-asc/desc → sortDatabaseView()│
│  hide → invoke('update_property_   │ ──→ [Backend: property_definitions]
│    definition', { visible:false }) │
│  delete → confirm() → invoke(      │ ──→ [Backend: property_definitions
│    'delete_property_definition')   │      + property_values]
│  outside click → doRename() +      │
│    menu.remove()                   │
└──────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F5.8 | Обработчики контекстного меню | 62 LOC — switch с 4 action-ветками + rename + outside click | Вынести каждый action в отдельную функцию | Medium |
| F5.2 | renderModal | Re-render всего модала при каждом изменении типа — теряет фокус, требует ручное восстановление value | Обновлять только секцию опций, не весь модал | Low |
| F5.7+F5.8 | showColumnMenu | Принимает 13 параметров — сигнал о необходимости рефакторинга | Передавать объект контекста вместо 13 отдельных параметров | Medium |
