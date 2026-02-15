# B3 — CRUD разговоров: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: CRUD разговоров |
| Файл(ы) | lib.rs:L3375-3497 |
| LOC | 123 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.1 | Сохранение нового разговора (save_conversation) | L3375-3390 | 16 | messages[] → conversation_id | DB INSERT conversations | persist | Low |
| B3.2 | Обновление разговора (update_conversation) | L3392-3408 | 17 | id, messages[] → () | DB UPDATE conversations | persist | Low |
| B3.3 | Список разговоров (get_conversations) | L3410-3432 | 23 | limit → Vec<Value> | DB SELECT conversations | query | Low |
| B3.4 | Получение разговора по ID (get_conversation) | L3434-3453 | 20 | id → Value (messages + meta) | DB SELECT conversations | query | Low |
| B3.5 | Удаление разговора (delete_conversation) | L3455-3464 | 10 | id → () | DB DELETE conversations | persist | Trivial |
| B3.6 | Поиск разговоров FTS5 (search_conversations) | L3466-3497 | 32 | query, limit → Vec<Value> | DB SELECT conversations_fts JOIN conversations | query | Medium |

Примечание: подфункций 6, т.к. функция содержит 6 отдельных Tauri-команд, но они логически объединены в один CRUD-модуль. Итого: 16+17+23+20+10+32 = 118 LOC (остальные 5 строк — пробелы/атрибуты).

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B3.6 | Поиск FTS5 | Фильтрация слов len > 1 может пропускать значимые двухбуквенные слова (напр. "AI") | Рассмотреть более умную токенизацию | Low |
| B3.1 | save_conversation | Нет дедупликации — повторный вызов создаст дубликат | Не критично (контролируется фронтом) | Low |
