# F4 — Список событий: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F4: Список событий |
| Файл(ы) | main.js:L3661-3733 |
| LOC | 73 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F4.1 | Загрузка событий | L3661-3663 | 3 | `calendarMonth, calendarYear` → `events[]` | bridge | Trivial |
| F4.2 | Генерация строк таблицы | L3668-3688 | 21 | `events[]` → `rowsHtml` (вычисление endTime, sourceLabel, sourceColor) | render | Low |
| F4.3 | Сборка HTML таблицы | L3690-3712 | 23 | `rowsHtml, monthNames, events.length` → `el.innerHTML` (nav + table с заголовками) | render | Low |
| F4.4 | Обработчики навигации | L3714-3724 | 11 | DOM ready → prev/next month, add event button | interaction | Trivial |
| F4.5 | Обработчик клика по строке | L3725-3730 | 6 | `.cal-list-row` click → переход на дневной вид для выбранного события | interaction | Trivial |

## Проблемные зоны

Нет критичных проблем.
