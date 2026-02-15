# F2 — Недельный календарь: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: Недельный календарь |
| Файл(ы) | main.js:L3457-3537 |
| LOC | 81 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Вычисление начала недели и группировка | L3457-3471 | 15 | `events[], calWeekOffset` → `weekStart, eventsByDate{}, todayStr` | transform | Low |
| F2.2 | Генерация заголовков дней недели | L3474-3486 | 13 | `weekStart, todayStr` → `daysHeader HTML, dayDates[]` | render | Trivial |
| F2.3 | Рендер сетки часов/событий | L3488-3501 | 14 | `hours[], dayDates, eventsByDate` → `gridHtml` (события распределены по часам) | render | Trivial |
| F2.4 | Метки периода недели | L3503-3506 | 4 | `weekStart` → `startLabel, endLabel` (формат "DD мес") | transform | Trivial |
| F2.5 | Сборка HTML и вставка в DOM | L3508-3520 | 13 | `daysHeader, gridHtml, startLabel, endLabel` → `el.innerHTML` | render | Trivial |
| F2.6 | Привязка обработчиков | L3522-3536 | 15 | DOM ready → навигация (prev/next/today), добавление события, клик по ячейке → showAddEventModal с предзаполненным временем | interaction | Low |

## Data Flow

```
[Input: el (DOM), events[]]
    │
    ▼
┌─ F2.1 Вычисление недели ──────┐
│  weekStart (Monday + offset)    │
│  eventsByDate{}, todayStr       │
└──────┬─────────────────────────┘
       │
       ├─────────────────────────────────┐
       ▼                                 ▼
┌─ F2.2 Заголовки ──────┐  ┌─ F2.3 Сетка часов ─────┐
│  dayDates[7]            │  │  hours[7:00-22:00]      │
│  daysHeader HTML        │  │  events по часам        │
└──────┬─────────────────┘  │  gridHtml               │
       │                     └──────┬────────────────────┘
       │                            │
       ▼                            ▼
┌─ F2.4 Метки ─┐   ┌─ F2.5 Сборка HTML ───────┐
│  startLabel   │──→│  nav + grid + header      │
│  endLabel     │   │  → el.innerHTML           │ ──→ [DOM]
└──────────────┘   └──────┬────────────────────┘
                          │
                          ▼
                   ┌─ F2.6 Обработчики ─────────────┐
                   │  #week-prev/next → calWeekOffset│
                   │  #week-today → reset             │
                   │  .week-cell click → modal+time   │
                   └─────────────────────────────────┘
```

## Проблемные зоны

Нет критичных проблем.
