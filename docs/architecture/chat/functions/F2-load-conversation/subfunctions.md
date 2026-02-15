# F2 — Загрузка разговора: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: Загрузка разговора |
| Файл(ы) | main.js:L336-390 |
| LOC | 55 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Автосохранение текущего + загрузка нового | L336-343 | 8 | id → conv object, state reset | bridge | Low |
| F2.2 | Загрузка рейтингов сообщений | L345-352 | 8 | conversation_id → ratingsMap | bridge | Low |
| F2.3 | Рендер всех сообщений с feedback | L353-373 | 21 | history[], ratingsMap → DOM messages | render | Low |
| F2.4 | Обновление UI (скролл + список) | L374-375 | 2 | — → scroll + sidebar refresh | render | Trivial |
| F2.5 | Автосохранение разговора (autoSaveConversation) | L381-390 | 10 | history, currentConversationId → saved/created | bridge | Low |

## Проблемные зоны

Нет критичных проблем.
