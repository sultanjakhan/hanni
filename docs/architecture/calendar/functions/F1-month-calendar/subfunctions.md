# F1 — Месячный календарь: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Месячный календарь |
| Файл(ы) | main.js:L3338-3456 |
| LOC | 119 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Вычисление дат и группировка событий | L3338-3356 | 19 | `events[]` → `eventsByDate{}, todayStr, firstDay, lastDay, startDay` | transform | Low |
| F1.2 | Рендер ячеек предыдущего месяца | L3358-3363 | 6 | `startDay, prevLast` → HTML ячеек other-month | render | Trivial |
| F1.3 | Рендер ячеек текущего месяца | L3365-3375 | 11 | `lastDay, eventsByDate, todayStr, selectedCalendarDate` → HTML ячеек с точками событий | render | Trivial |
| F1.4 | Рендер ячеек следующего месяца | L3377-3381 | 5 | `totalCells` → HTML оставшихся ячеек | render | Trivial |
| F1.5 | Рендер панели событий дня | L3383-3394 | 12 | `selectedCalendarDate, eventsByDate` → HTML панели с событиями выбранного дня | render | Trivial |
| F1.6 | Сборка HTML и вставка в DOM | L3396-3406 | 11 | `daysHtml, dayPanelHtml` → `el.innerHTML` | render | Trivial |
| F1.7 | Привязка обработчиков событий | L3408-3455 | 48 | DOM ready → навигация (prev/next), выбор дня, добавление события, синхронизация | interaction | Medium |

## Data Flow

```
[Input: el (DOM), events[]]
    │
    ▼
┌─ F1.1 Вычисление дат ────────┐
│  firstDay, lastDay, startDay   │
│  todayStr, eventsByDate{}      │
└──────┬────────────────────────┘
       │
       ▼
┌─ F1.2-F1.4 Рендер ячеек ─────┐
│  F1.2: prev month (other)      │
│  F1.3: current month (dots)    │
│  F1.4: next month (other)      │
│  → daysHtml                    │
└──────┬────────────────────────┘
       │
       ▼
┌─ F1.5 Панель дня ────────────┐
│  selectedCalendarDate events   │
│  → dayPanelHtml                │
└──────┬────────────────────────┘
       │
       ▼
┌─ F1.6 Сборка HTML ──────────┐
│  nav + weekdays + grid + panel │
│  → el.innerHTML                │ ──→ [DOM]
└──────┬────────────────────────┘
       │
       ▼
┌─ F1.7 Обработчики ──────────────────────┐
│  #cal-prev → calendarMonth--; loadCalendar() │
│  #cal-next → calendarMonth++; loadCalendar() │
│  .calendar-day → selectedCalendarDate        │
│  #cal-add-event → showAddEventModal()        │
│  #cal-sync → invoke('sync_apple_calendar')   │ ──→ [Backend: sync]
│              invoke('sync_google_ics')        │
└──────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F1.7 | Обработчики событий | 48 LOC — логика синхронизации (try/catch, invoke двух сервисов, обновление UI) встроена прямо в обработчик click | Вынести sync-логику в отдельную async функцию `syncCalendars()` | Medium |
