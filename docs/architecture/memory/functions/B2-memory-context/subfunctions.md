# B2 — Построение контекста памяти для LLM: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Построение контекста памяти для LLM |
| Файл(ы) | lib.rs:L933-1025 |
| LOC | 93 |
| Подфункций | 4 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Загрузка core-фактов (user/preferences) | L933-956 | 24 | conn, limit → lines[], seen_ids | DB SELECT facts WHERE category IN ('user','preferences') | query | Low |
| B2.2 | FTS5-поиск по сообщению пользователя | L958-990 | 33 | user_msg, remaining → lines[] (доп. релевантные факты) | DB SELECT facts_fts JOIN facts | query | Medium |
| B2.3 | Дозаполнение последними фактами | L992-1017 | 26 | remaining, seen_ids → lines[] (до лимита) | DB SELECT facts ORDER BY updated_at DESC | query | Low |
| B2.4 | Форматирование результата | L1019-1024 | 6 | lines[] → joined string or empty | — | transform | Trivial |

## Data Flow

```
[Input: conn, user_msg, limit]
    │
    ▼
┌─ B2.1 Core Facts ──────────┐
│  SELECT FROM facts            │ ←── [DB: facts]
│  WHERE category IN            │
│    ('user', 'preferences')    │
│  → lines[], seen_ids          │
└──────┬──────────────────────┘
       │ lines, seen_ids, remaining = limit - len
       ▼
┌─ B2.2 FTS5 Search ─────────┐
│  split user_msg → words       │
│  join(" OR ") → fts_query     │
│  SELECT FROM facts_fts        │ ←── [DB: facts_fts]
│  JOIN facts                   │
│  → доп. строки (dedupe)      │
└──────┬──────────────────────┘
       │ lines, seen_ids, remaining
       ▼
┌─ B2.3 Fill Remaining ──────┐
│  SELECT FROM facts            │ ←── [DB: facts]
│  ORDER BY updated_at DESC     │
│  → заполнить до limit         │
└──────┬──────────────────────┘
       │ lines[]
       ▼
┌─ B2.4 Format Output ───────┐
│  lines.join("\n") or ""       │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.2 | FTS5 Search | Фильтрация слов len > 2 — теряет важные короткие слова ("AI", "JS", "ПК") | Снизить порог до len > 1 или использовать whitelist для коротких значимых слов | Low |
| B2.3 | Fill Remaining | Запрос LIMIT = remaining + seen_ids.len() — overfetch для дедупликации; может быть неточен при большом кол-ве дубликатов | Не критично при limit=80 | Low |
