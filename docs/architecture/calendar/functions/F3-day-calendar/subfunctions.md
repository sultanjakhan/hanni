# F3 — Дневной календарь: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F3: Дневной календарь |
| Файл(ы) | main.js:L3578-3660 |
| LOC | 83 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.1 | Инициализация даты и фильтрация событий | L3578-3588 | 11 | `events[], calDayDate` → `dayEvents[] (sorted), d (Date), dayNames, monthNames` | transform | Trivial |
| F3.2 | Рендер timeline часов с событиями | L3590-3607 | 18 | `hours[6:00-22:00], dayEvents` → `timelineHtml` (строки с событиями по часам, рассчёт end time) | render | Low |
| F3.3 | Рендер all-day событий | L3609-3614 | 6 | `dayEvents без time` → `allDayHtml` | render | Trivial |
| F3.4 | Сборка HTML и вставка в DOM | L3616-3625 | 10 | `allDayHtml, timelineHtml, d, dayNames, monthNames` → `el.innerHTML` | render | Trivial |
| F3.5 | Обработчики навигации | L3627-3646 | 20 | DOM ready → prev/next day, today, add event (обновление calDayDate, calendarMonth, calendarYear) | interaction | Low |
| F3.6 | Обработчик клика по часу | L3647-3657 | 11 | `.day-hour-content` click → showAddEventModal() с предзаполненным временем | interaction | Trivial |

## Data Flow

```
[Input: el (DOM), events[]]
    │
    ▼
┌─ F3.1 Инициализация даты ────┐
│  calDayDate (default: today)   │
│  filter + sort dayEvents       │
│  normalize time HH:MM          │
└──────┬────────────────────────┘
       │ dayEvents[]
       │
       ├──────────────────────────────┐
       ▼                              ▼
┌─ F3.2 Timeline часов ─────┐  ┌─ F3.3 All-day ──────┐
│  hours 6:00-22:00           │  │  events без time     │
│  match events → hour        │  │  → allDayHtml        │
│  calc endTime               │  └──────┬──────────────┘
│  → timelineHtml             │         │
└──────┬─────────────────────┘         │
       │                                │
       ▼                                ▼
┌─ F3.4 Сборка HTML ──────────────────────┐
│  nav + allDayHtml + timelineHtml         │
│  → el.innerHTML                          │ ──→ [DOM]
└──────┬───────────────────────────────────┘
       │
       ├──────────────────────────────┐
       ▼                              ▼
┌─ F3.5 Навигация ──────┐  ┌─ F3.6 Клик по часу ──┐
│  prev/next/today        │  │  → showAddEventModal() │
│  → loadCalendar('День') │  │  + предзаполнение time │
└────────────────────────┘  └───────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F3.2 | Timeline часов | Вычисление end time через IIFE-лямбду inline — читаемость | Вынести расчёт endTime в утилитную функцию `calcEndTime(time, duration)` | Low |
