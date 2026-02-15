# F5 — Интеграции календаря UI: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F5: Интеграции календаря (UI) |
| Файл(ы) | main.js:L3734-3814 |
| LOC | 81 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F5.1 | Загрузка настроек | L3734-3738 | 5 | — → `appleEnabled, googleUrl` (из app_settings) | bridge | Trivial |
| F5.2 | Рендер секций настроек | L3740-3774 | 35 | `appleEnabled, googleUrl, autosync` → `el.innerHTML` (Apple toggle, Google ICS URL input, autosync toggle) | render | Medium |
| F5.3 | Обработчики toggle-переключателей | L3776-3781 | 6 | `#calint-apple`, `#calint-autosync` change → `set_app_setting()` | interaction | Trivial |
| F5.4 | Обработчик синхронизации Apple | L3782-3795 | 14 | `#calint-sync-apple` click → `sync_apple_calendar()`, обновление статуса | interaction | Low |
| F5.5 | Обработчик сохранения и синхронизации Google | L3796-3811 | 16 | `#calint-save-google` click → `set_app_setting()` + `sync_google_ics()`, обновление статуса | interaction | Low |

## Проблемные зоны

Нет критичных проблем.
