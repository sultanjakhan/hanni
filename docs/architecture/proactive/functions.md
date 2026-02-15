# Proactive — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Типы и настройки проактивности | internal | ProactiveSettings, ProactiveState | lib.rs:L181-271 | 91 | Medium | 5 | ✅ |
| B2 | Сохранение/загрузка настроек | internal | load/save_proactive_settings | lib.rs:L1026-1050 | 25 | Simple | — | — |
| B3 | Построение системного промпта | internal | build_proactive_system_prompt() | lib.rs:L6385-6436 | 52 | Medium | 4 | ✅ |
| B4 | Сбор контекста ОС | internal | gather_context_blocking() | lib.rs:L6483-6561 | 79 | Complex | 7 | ✅ |
| B5 | Анализ утренних данных | internal | gather_morning_digest() | lib.rs:L6609-6659 | 51 | Medium | 5 | ✅ |
| B6 | Анализ screen time | internal | gather_screen_time() | lib.rs:L6660-6706 | 47 | Simple | — | — |
| B7 | Дельта активности | internal | compute_activity_delta() | lib.rs:L6707-6737 | 31 | Simple | — | — |
| B8 | Генерация проактивного сообщения (LLM) | scheduled | proactive loop in run() | lib.rs:L6765-6905 | 141 | Complex | 8 | ✅ |

## Frontend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| F1 | Обработчик проактивного сообщения | event | listen 'proactive-message' | main.js:L164-188 | 25 | Simple | — | — |
| F2 | Отслеживание набора текста | event | input listener | main.js:L192-198 | 7 | Simple | — | — |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 10 |
| Simple | 4 |
| Medium | 3 |
| Complex | 2 |
| Total subfunctions | 29 |
| Phase 3 completed | 5/5 (all Medium+Complex) |
