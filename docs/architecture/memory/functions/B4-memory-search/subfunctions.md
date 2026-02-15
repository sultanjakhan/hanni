# B4 — Поиск в памяти (FTS5): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Поиск в памяти (FTS5) |
| Файл(ы) | lib.rs:L3320-3374 |
| LOC | 55 |
| Подфункций | 4 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B4.1 | Подготовка запроса (токенизация) | L3320-3335 | 16 | query string, limit → words[], fts_query | — | parse | Low |
| B4.2 | FTS5 MATCH поиск | L3336-3352 | 17 | fts_query, max → results[] | DB SELECT facts_fts JOIN facts | query | Low |
| B4.3 | Fallback LIKE поиск | L3354-3366 | 13 | query, max → results[] | DB SELECT facts WHERE LIKE | query | Low |
| B4.4 | Форматирование результата | L3368-3374 | 7 | results[] → joined string or "No memories found." | — | transform | Trivial |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B4.2 | FTS5 MATCH | При ошибке FTS5 prepare/query — тихо пропускает (if let Ok), падает на fallback | Логировать ошибку FTS5 для диагностики | Low |
