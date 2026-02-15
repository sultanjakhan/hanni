# Calendar — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | CRUD событий | endpoint | create/get/delete_event | lib.rs:L3829-3871 | 43 | Simple | — | — |
| B2 | Синхронизация Apple Calendar | endpoint | sync_apple_calendar | lib.rs:L3872-4031 | 160 | Complex | 11 | ✅ |
| B3 | Парсинг RRULE + расширение | internal | parse_rrule, expand_rrule, RRule struct | lib.rs:L4034-4219 | 186 | Complex | 10 | ✅ |
| B4 | Синхронизация Google Calendar (ICS) | endpoint | sync_google_ics | lib.rs:L4221-4329 | 109 | Complex | 7 | ✅ |

## Frontend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| F1 | Месячный календарь | view | renderCalendar() | main.js:L3338-3456 | 119 | Complex | 7 | ✅ |
| F2 | Недельный календарь | view | renderWeekCalendar() | main.js:L3457-3537 | 81 | Complex | 6 | ✅ |
| F3 | Дневной календарь | view | renderDayCalendar() | main.js:L3578-3660 | 83 | Complex | 6 | ✅ |
| F4 | Список событий | view | renderCalendarList() | main.js:L3661-3733 | 73 | Medium | 5 | ✅ |
| F5 | Интеграции календаря (UI) | view | renderCalendarIntegrations() | main.js:L3734-3814 | 81 | Medium | 5 | ✅ |
| F6 | Модальное окно добавления события | view | showAddEventModal() | main.js:L3538-3577 | 40 | Medium | 3 | ✅ |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 10 |
| Simple | 1 |
| Medium | 3 |
| Complex | 6 |
| Total subfunctions | 60 |
