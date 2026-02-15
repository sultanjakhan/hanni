# B4 — Веб-поиск DuckDuckGo: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Веб-поиск (DuckDuckGo) |
| Файл(ы) | lib.rs:L2082-2130 |
| LOC | 49 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B4.1 | Создание HTTP клиента | L2082-2087 | 6 | -- -> Client | -- | control | Trivial |
| B4.2 | Формирование URL запроса | L2089-2093 | 5 | query: String -> url: String | -- | transform | Trivial |
| B4.3 | Выполнение HTTP запроса | L2095-2102 | 8 | url -> html: String | Network: GET html.duckduckgo.com | external | Low |
| B4.4 | Парсинг HTML результатов (regex) | L2104-2116 | 13 | html -> titles[], snippets[], urls[] | -- | parse | Medium |
| B4.5 | Форматирование результатов | L2118-2129 | 12 | titles, snippets, urls -> String | -- | transform | Trivial |

## Data Flow

```
[Input: query: String]
    │
    ▼
┌─ B4.1 HTTP Client ─┐
│  timeout: 10s        │
└──────┬──────────────┘
       │
       ▼
┌─ B4.2 Build URL ─────────────┐
│  html.duckduckgo.com/html/    │
│  replace(' ', '+')            │
│  encode & and #               │
└──────┬───────────────────────┘
       │ url
       ▼
┌─ B4.3 HTTP GET ──────────┐
│  User-Agent: Mozilla/5.0  │ ──→ [External: DuckDuckGo]
│  .send().await             │
│  .text().await             │
└──────┬───────────────────┘
       │ html
       ▼
┌─ B4.4 Parse HTML ─────────────────┐
│  re_title: class="result__a"       │
│  re_snippet: class="result__snippet│
│  re_url: class="result__url"       │
│  Strip HTML tags from snippets     │
└──────┬────────────────────────────┘
       │ titles[], snippets[], urls[]
       ▼
┌─ B4.5 Format ──────┐
│  Top 5 results       │
│  "{i}. {title} — ..."│
│  or "No results"     │
└──────┬──────────────┘
       │
       ▼
[Output: Result<String, String>]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B4.4 | Парсинг HTML | Regex парсинг HTML хрупок — DDG может сменить разметку | Использовать HTML-парсер (scraper) или API | Low |
| B4.3 | HTTP запрос | Нет retry при сетевых ошибках | Добавить retry с backoff | Low |
