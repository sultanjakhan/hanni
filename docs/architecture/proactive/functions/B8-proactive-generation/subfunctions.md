# B8 — Генерация проактивного сообщения (LLM): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B8: Генерация проактивного сообщения (LLM) |
| Файл(ы) | lib.rs:L6765-6905 |
| LOC | 141 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B8.1 | Получение сегодняшних проактивных сообщений | L6765-6780 | 16 | conn → msgs[] | DB SELECT proactive_history | query | Low |
| B8.2 | Получение имени пользователя из памяти | L6782-6788 | 7 | conn → user_name | DB SELECT facts | query | Trivial |
| B8.3 | Построение системного промпта с именем | L6790-6811 | 22 | enabled_styles, user_name → sys_prompt | — | transform | Low |
| B8.4 | Формирование user_content: память + контекст | L6813-6832 | 20 | memory_context, context, delta, triggers → user_content | — | transform | Low |
| B8.5 | Добавление истории: chat snippet + recent msgs + todays | L6834-6857 | 24 | recent_messages, todays_messages, engagement_rate → user_content (дополнен) | — | transform | Low |
| B8.6 | Вызов LLM (non-stream) | L6859-6886 | 28 | sys_prompt, user_content → raw response | HTTP POST (MLX_URL) | external | Medium |
| B8.7 | Очистка ответа (strip think-тегов) | L6888-6896 | 9 | raw → cleaned text | — | parse | Trivial |
| B8.8 | Решение: SKIP или сообщение | L6898-6903 | 6 | cleaned text → Option<String> | — | control | Trivial |

## Data Flow

```
[Input: client, context, recent_messages, skips, memory, delta, triggers, chat_snippet, engagement, user_name, todays, styles]
    │
    ▼
┌─ B8.1 Today's Messages ────┐
│  SELECT FROM proactive_history│ ←── [DB: proactive_history]
│  WHERE sent_at >= today       │
└──────┬──────────────────────┘
       │ todays_messages[]
       ▼
┌─ B8.2 User Name ───────────┐
│  SELECT FROM facts            │ ←── [DB: facts]
│  WHERE key LIKE '%имя%'       │
└──────┬──────────────────────┘
       │ user_name
       ▼
┌─ B8.3 Build System Prompt ─┐
│  build_proactive_system_prompt│
│  prepend user name if known  │
└──────┬──────────────────────┘
       │ sys_prompt
       ▼
┌─ B8.4 Build User Content ──┐
│  [Your memories] + context   │
│  + [Changes since last]      │
│  + [Active triggers]         │
│  + [Recent conversation]     │
└──────┬──────────────────────┘
       │ user_content (partial)
       ▼
┌─ B8.5 Add History Context ─┐
│  [Your recent proactive msgs]│
│  [Topics covered today]      │
│  Skips + engagement rate     │
│  + /no_think                 │
└──────┬──────────────────────┘
       │ user_content (complete)
       ▼
┌─ B8.6 LLM Call ────────────┐
│  POST MLX_URL                │ ──→ [External: MLX Server]
│  stream=false, temp=0.85     │
│  max_tokens=300              │
└──────┬──────────────────────┘
       │ raw response
       ▼
┌─ B8.7 Clean Response ──────┐
│  strip <think>...</think>    │
│  trim whitespace             │
└──────┬──────────────────────┘
       │ cleaned text
       ▼
┌─ B8.8 Skip or Emit ────────┐
│  if "[SKIP]" or empty → None │
│  else → Some(text)           │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B8.6 | LLM Call | Нет retry — если MLX занят, запрос просто падает | Добавить retry 1-2 попытки (MLX single-threaded) | Medium |
| B8.4-B8.5 | Content Building | 44 LOC суммарно на формирование prompt — множество условных блоков | Вынести в отдельную fn build_proactive_user_content() | Low |
| B8.7 | Clean Response | regex::Regex::new() вызывается на каждый вызов — можно закешировать | Использовать lazy_static или OnceLock | Low |
