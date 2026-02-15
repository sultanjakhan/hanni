# Focus — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Запуск режима фокуса | endpoint | start_focus | lib.rs:L1839-1926 | 88 | Medium | 6 | ✅ |
| B2 | Остановка режима фокуса | endpoint | stop_focus | lib.rs:L1928-1947 | 20 | Simple | — | — |
| B3 | Статус режима фокуса | endpoint | get_focus_status | lib.rs:L1949-1964 | 16 | Simple | — | — |
| B4 | Обновление конфига блок-листа | endpoint | update_blocklist | lib.rs:L1966-1995 | 30 | Simple | — | — |
| B5 | CRUD persistent блок-листа | endpoint | add/remove/get/toggle_blocklist | lib.rs:L5613-5659 | 47 | Simple | — | — |

## Frontend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| F1 | UI режима фокуса | view | loadFocus() | main.js:L3056-3161 | 106 | Complex | 7 | ✅ |
| F2 | Событие завершения фокуса | event | listen 'focus-ended' | main.js:L261-267 | 7 | Simple | — | — |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 7 |
| Simple | 5 |
| Medium | 1 |
| Complex | 1 |
| Total subfunctions | 13 |
