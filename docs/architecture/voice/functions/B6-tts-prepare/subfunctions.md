# B6 — TTS подготовка и очистка текста: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B6: TTS подготовка и очистка текста |
| Файл(ы) | lib.rs:L6906-7056 |
| LOC | 151 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B6.1 | Адаптивная скорость TTS по длине текста | L6906-6918 | 13 | text: &str → rate: String | — | transform | Trivial |
| B6.2 | Удаление action/think/URL блоков (regex) | L6986-6995 | 10 | text: &str → cleaned String | — | parse | Trivial |
| B6.3 | Удаление markdown-форматирования | L6996-6999 | 4 | String → String | — | transform | Trivial |
| B6.4 | Удаление скобок и квадратных скобок | L7000-7005 | 6 | String → String | — | parse | Trivial |
| B6.5 | Фильтрация emoji/символов (Unicode ranges) | L7006-7039 | 34 | String → String (только Latin+Cyrillic+digits) | — | transform | Medium |
| B6.6 | Схлопывание пробелов и trim | L7040-7054 | 15 | String → String | — | transform | Trivial |

## Data Flow

```
[Input: raw LLM response text]
    │
    ▼
┌─ B6.2 Удаление блоков ──────────────┐
│  regex: ```action...``` → удалить     │
│  regex: <think>...</think> → удалить │
│  regex: https?://... → удалить       │
└──────┬───────────────────────────────┘
       │
       ▼
┌─ B6.3 Markdown cleanup ────────────┐
│  " → ', ``` → "", ** → "", # → ""  │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ B6.4 Скобки ──────────────────────┐
│  regex: (...) → удалить             │
│  regex: [...] → удалить             │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ B6.5 Emoji фильтр ───────────────┐
│  char filter: keep Latin, Cyrillic, │
│  digits, basic punctuation          │
│  Remove: U+1F600-1FAFF, U+2600+    │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ B6.6 Пробелы + trim ─────────────┐
│  Collapse whitespace → single space │
│  .trim()                            │
└──────┬──────────────────────────────┘
       │
       ▼
[Output: clean text for TTS]

[Отдельно] B6.1: word_count → rate string (+10%..+35%)
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B6.5 | Emoji фильтр | 34 LOC hardcoded Unicode ranges — может пропускать новые emoji | Использовать unicode crate или regex \p{Emoji} | Low |
