# F4 — Tab bar rendering: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F4: Tab bar rendering |
| Файл(ы) | main.js:L450-481 |
| LOC | 32 |
| Подфункций | 3 |
| Сложность (max) | Low |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F4.1 | Очистка и итерация по табам | L450-473 | 24 | openTabs[] → для каждого: createElement .tab-item + icon/label/close button + click handlers → tabList DOM | render | Low |
| F4.2 | Привязка событий (switch/close) | L462-471 | 10 | click tab-item → switchTab(tabId); click .tab-item-close → closeTab(tabId) | interaction | Trivial |
| F4.3 | Маркировка before-active | L475-480 | 6 | items[] + activeTab → CSS-класс .before-active на предыдущем табе (для скрытия divider) | render | Trivial |

## Проблемные зоны

Нет критичных проблем. Функция компактная и хорошо структурированная.
