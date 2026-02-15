# F5 — Потоковый чат: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F5: Потоковый чат (streamChat) |
| Файл(ы) | main.js:L1421-1462 |
| LOC | 42 |
| Подфункций | 4 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F5.1 | Инициализация курсора и переменных | L1421-1428 | 8 | botDiv, t0 → cursor DOM, state vars | render | Trivial |
| F5.2 | Подписка на события chat-token / chat-done | L1430-1439 | 10 | listen('chat-token') → token append to DOM + accumulate | stream | Low |
| F5.3 | Вызов backend chat с обработкой ошибок | L1441-1453 | 13 | history[-20] → invoke('chat') | bridge | Medium |
| F5.4 | Cleanup и возврат результата | L1455-1459 | 5 | — → unlisten, cursor.remove, return {fullReply, tokens, firstToken} | control | Trivial |

## Проблемные зоны

Нет критичных проблем.
