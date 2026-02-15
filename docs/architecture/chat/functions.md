# Chat — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Типы протокола чата | internal | ChatMessage, ChatRequest, Delta, etc. | lib.rs:L1052-1093 | 42 | Simple | — | — |
| B2 | Стриминг чата с LLM | endpoint | chat() | lib.rs:L2695-2883 | 189 | Complex | 8 | ✅ |
| B3 | CRUD разговоров | endpoint | save/update/get/delete/search_conversation | lib.rs:L3375-3497 | 123 | Medium | 6 | ✅ |
| B4 | Извлечение фактов из разговора | endpoint | process_conversation_end() | lib.rs:L3499-3611 | 113 | Complex | 7 | ✅ |

## Frontend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| F1 | Список разговоров | view | loadConversationsList() | main.js:L271-334 | 64 | Medium | 5 | ✅ |
| F2 | Загрузка разговора | view | loadConversation() | main.js:L336-390 | 55 | Medium | 5 | ✅ |
| F3 | Выполнение действий LLM | handler | executeAction() | main.js:L1105-1382 | 278 | Complex | 8 | ✅ |
| F4 | Отправка сообщения | handler | send() | main.js:L1522-1673 | 152 | Complex | 8 | ✅ |
| F5 | Потоковый чат | handler | streamChat() | main.js:L1421-1462 | 42 | Medium | 4 | ✅ |
| F6 | Добавление сообщения в UI | utility | addMsg(), addFeedbackButtons() | main.js:L978-1076 | 99 | Medium | 5 | ✅ |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 10 |
| Simple | 1 |
| Medium | 5 |
| Complex | 4 |
| Total subfunctions | 56 |
| Phase 3 completed | 9/9 (all Medium+Complex) |
