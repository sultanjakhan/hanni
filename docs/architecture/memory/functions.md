# Memory — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Миграция памяти JSON->SQLite | internal | migrate_memory_json() | lib.rs:L880-932 | 53 | Simple | — | — |
| B2 | Построение контекста памяти для LLM | internal | build_memory_context_from_db() | lib.rs:L933-1025 | 93 | Medium | 4 | ✅ |
| B3 | CRUD фактов памяти | endpoint | memory_remember/recall/forget() | lib.rs:L3246-3318 | 73 | Simple | — | — |
| B4 | Поиск в памяти (FTS5) | endpoint | memory_search() | lib.rs:L3320-3374 | 55 | Medium | 4 | ✅ |
| B5 | Браузер памяти | endpoint | get_all_memories/delete/update_memory() | lib.rs:L4738-4800 | 63 | Simple | — | — |

## Frontend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| F1 | Вкладка памяти (все факты) | view | loadAllFacts() | main.js:L2505-2608 | 104 | Complex | 5 | ✅ |
| F2 | Рендер списка фактов | view | renderMemoryList() | main.js:L2612-2672 | 61 | Medium | 4 | ✅ |
| F3 | Поиск памяти (UI) | view | loadMemorySearch() | main.js:L2737-2760 | 24 | Simple | — | — |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 8 |
| Simple | 4 |
| Medium | 2 |
| Complex | 1 |
| Total subfunctions | 17 |
| Phase 3 completed | 4/4 (all Medium+Complex) |
