# B1 — Системный промпт и константы: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Системный промпт и константы |
| Файл(ы) | lib.rs:L14-162 |
| LOC | 149 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B1.1 | Константы подключения (MLX_URL, MODEL) | L15-16 | 2 | -- -> &str | -- | transform | Trivial |
| B1.2 | Системный промпт SYSTEM_PROMPT | L18-138 | 121 | -- -> &str | -- | transform | Medium |
| B1.3 | Системный промпт SYSTEM_PROMPT_LITE | L140-143 | 4 | -- -> &str | -- | transform | Trivial |
| B1.4 | Ключевые слова ACTION_KEYWORDS | L145-156 | 12 | -- -> &[&str] | -- | transform | Trivial |
| B1.5 | Функция needs_full_prompt() | L158-162 | 5 | user_msg: &str -> bool | -- | validation | Trivial |

## Data Flow

```
[Константы MLX_URL, MODEL]
    │
    ├──> используются в chat_inner(), start_mlx_server()
    │
[SYSTEM_PROMPT] ──────────────┐
    │                          │
[SYSTEM_PROMPT_LITE] ─────────┤
    │                          ▼
[ACTION_KEYWORDS]          [needs_full_prompt()]
    │                          │
    └──────────────────────────┘
                │
                ▼
        chat_inner() выбирает промпт:
        use_full=true  -> SYSTEM_PROMPT
        use_full=false -> SYSTEM_PROMPT_LITE
        call_mode=true -> специальный voice prompt
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.2 | SYSTEM_PROMPT | 121 строк в одной константе, тяжело поддерживать и тестировать | Вынести в отдельный файл или модуль | Low |
| B1.4 | ACTION_KEYWORDS | Дублирование ключевых слов с логикой в system prompt | Синхронизировать списки автоматически | Low |
