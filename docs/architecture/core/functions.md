# Core — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Системный промпт и константы | internal | SYSTEM_PROMPT, MLX_URL, MODEL, ACTION_KEYWORDS | lib.rs:L14-155 | 142 | Medium | 5 | ✅ |
| B2 | Инициализация БД (40+ таблиц) | internal | init_db() | lib.rs:L346-879 | 534 | Complex | 12 | ✅ |
| B3 | Миграция старых данных | internal | migrate_old_data_dir() | lib.rs:L296-345 | 50 | Simple | — | — |
| B4 | Обновление приложения | internal | updater_with_headers(), get_app_version() | lib.rs:L7358-7386 | 29 | Simple | — | — |
| B5 | Запуск приложения (entry point) | internal | run() | lib.rs:L7391-8201 | 811 | Complex | 14 | ✅ |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 5 |
| Simple | 2 |
| Medium | 1 |
| Complex | 2 |
| Total subfunctions | 31 |
