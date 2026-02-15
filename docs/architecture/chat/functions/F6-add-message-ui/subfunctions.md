# F6 — Добавление сообщения в UI: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F6: Добавление сообщения в UI |
| Файл(ы) | main.js:L978-1076 |
| LOC | 99 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F6.1 | Рендер сообщения бота (addMsg bot) | L978-994 | 17 | role='bot', text → wrapper + div + TTS btn в DOM | render | Low |
| F6.2 | Рендер сообщения пользователя (addMsg other) | L995-1002 | 8 | role, text → div в DOM | render | Trivial |
| F6.3 | Создание кнопок обратной связи (addFeedbackButtons) | L1004-1047 | 44 | wrapper, conversationId, messageIndex → thumbUp/thumbDown в DOM | interaction | Medium |
| F6.4 | Обработка прикрепления файла | L1049-1068 | 20 | file input change → attachedFile state + preview | interaction | Low |
| F6.5 | Отмена прикрепления | L1070-1073 | 4 | click preview → attachedFile=null | interaction | Trivial |

Примечание: F6.3 содержит async-обработчики кликов с invoke('rate_message') и toggle-логикой active/inactive — наиболее сложная часть.

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F6.3 | Кнопки обратной связи | 44 LOC с вложенным async handleClick — громоздкий closure | Вынести handleClick в отдельную именованную функцию | Low |
