# B5 — Анализ утренних данных: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: Анализ утренних данных |
| Файл(ы) | lib.rs:L6609-6659 |
| LOC | 51 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B5.1 | Инициализация подключения к БД | L6609-6613 | 5 | — → conn (rusqlite::Connection) | File read (hanni.db) | query | Trivial |
| B5.2 | Подсчёт сегодняшних событий | L6615-6622 | 8 | today → event_count | DB SELECT COUNT events | query | Trivial |
| B5.3 | Вчерашнее настроение | L6624-6634 | 11 | yesterday → mood/5 + note | DB SELECT mood_log | query | Low |
| B5.4 | Вчерашний сон | L6636-6645 | 10 | yesterday → sleep hours | DB SELECT health_log | query | Low |
| B5.5 | Активные цели | L6647-6658 | 12 | — → goals_count | DB SELECT COUNT goals WHERE progress < target | query | Trivial |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B5.1 | Init DB | Открывает собственное подключение (Connection::open) вместо использования HanniDb state | Не критично — вызывается из blocking context без доступа к AppHandle | Low |
