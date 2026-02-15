# F1 — Список разговоров: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Список разговоров |
| Файл(ы) | main.js:L271-334 |
| LOC | 64 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Загрузка данных (search или get_all) | L271-280 | 10 | searchQuery → convs[] | bridge | Low |
| F1.2 | Группировка по дате | L281-296 | 16 | convs[] → groups {today, yesterday, week, earlier} | transform | Low |
| F1.3 | Рендер групп и элементов | L298-314 | 17 | groups → DOM (conv-list) | render | Low |
| F1.4 | Обработчик клика (загрузка) | L315-318 | 4 | click → loadConversation(id) | interaction | Trivial |
| F1.5 | Обработчик удаления | L319-331 | 13 | click delete → invoke('delete_conversation') + reload | interaction | Low |

## Проблемные зоны

Нет критичных проблем.
