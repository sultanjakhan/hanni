# F1 — Home supplies + shopping list: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Home supplies + shopping list |
| Файл(ы) | main.js:L1699-1799 |
| LOC | 101 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Роутинг Home по sub-tab | L1699-L1706 | 8 | `loadHome(subTab)` → рендер page-header, делегирование loadSupplies / loadShoppingList | control | Trivial |
| F1.2 | Рендер списка запасов | L1708-L1723 | 16 | `loadSupplies(el)` → invoke get_home_items → HTML-список с категориями, кнопками Need/Delete | render | Low |
| F1.3 | Обработчики toggle/delete запасов | L1724-L1735 | 12 | Клик на `[data-need]` / `[data-hdel]` → invoke toggle_home_item_needed / delete_home_item → перезагрузка | interaction | Trivial |
| F1.4 | Модальное окно добавления запаса | L1736-L1776 | 41 | Клик на "Add Item" → модалка с полями (name, category, qty, unit, location) → invoke add_home_item → перезагрузка | interaction | Medium |
| F1.5 | Рендер Shopping List | L1780-L1799 | 20 | `loadShoppingList(el)` → invoke get_home_items(neededOnly=true) → список с toggle "bought" | render | Low |

## Data Flow

```
[loadHome(subTab)]
      │
      ├── subTab = 'Shopping List' → loadShoppingList(pc)
      └── else → loadSupplies(pc)

[loadSupplies]
      │
      ▼
  invoke('get_home_items') ──→ [Backend: B16]
      │
      ▼
  Рендер HTML: items list + category badges
      │
      ├── [data-need] click → invoke('toggle_home_item_needed') → loadSupplies()
      ├── [data-hdel] click → invoke('delete_home_item') → loadSupplies()
      └── "Add Item" click → модалка → invoke('add_home_item') → loadSupplies()

[loadShoppingList]
      │
      ▼
  invoke('get_home_items', neededOnly=true)
      │
      ▼
  Рендер: items + checkbox "bought"
      │
      └── [data-bought] click → invoke('toggle_home_item_needed') → loadShoppingList()
```

## Проблемные зоны

Нет критичных проблем. Код линейный, модалка хорошо изолирована.
