# B2 — Стриминг чата с LLM: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Стриминг чата с LLM |
| Файл(ы) | lib.rs:L2695-2883 |
| LOC | 189 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Захват LLM-семафора (busy wait) | L2695-2707 | 13 | app, messages, call_mode → Result<String> | AtomicBool read/write | control | Trivial |
| B2.2 | Формирование даты/контекста времени | L2709-2734 | 26 | now_local → date_context string | — | transform | Low |
| B2.3 | Выбор системного промпта (adaptive) | L2735-2770 | 36 | last_user_msg, call_mode → system_content | — | transform | Medium |
| B2.4 | Инъекция контекста памяти | L2772-2794 | 23 | messages, use_full, call_mode → chat_messages + memory | DB read (facts, facts_fts) | query | Low |
| B2.5 | Построение массива сообщений | L2796-2813 | 18 | messages, use_full, call_mode → ChatRequest | — | transform | Low |
| B2.6 | HTTP-запрос к MLX с retry | L2815-2829 | 15 | ChatRequest → response | HTTP POST (MLX_URL), retry x3 | external | Medium |
| B2.7 | Парсинг SSE-потока и фильтрация think-тегов | L2831-2877 | 47 | response stream → full_reply | emit("chat-token"), emit("chat-done") | stream | High |
| B2.8 | Возврат результата | L2879-2881 | 3 | full_reply → Ok(String) | — | transform | Trivial |

## Data Flow

```
[Input: app, messages[], call_mode]
    │
    ▼
┌─ B2.1 Busy Wait ──────────┐
│  poll AtomicBool 30x500ms   │
│  set busy=true               │
└──────┬──────────────────────┘
       │ app, messages
       ▼
┌─ B2.2 Date Context ────────┐
│  chrono::Local::now()        │
│  weekday_ru + 14-day lookup  │
│  format date_context string  │
└──────┬──────────────────────┘
       │ date_context
       ▼
┌─ B2.3 Select System Prompt ─┐
│  needs_full_prompt(msg)       │
│  call_mode → voice prompt     │
│  use_full → SYSTEM_PROMPT     │
│  else → SYSTEM_PROMPT_LITE    │
└──────┬───────────────────────┘
       │ system_content
       ▼
┌─ B2.4 Memory Injection ────┐
│  build_memory_context_from_db│ ←── [DB: facts, facts_fts]
│  append as system message    │
└──────┬──────────────────────┘
       │ chat_messages[]
       ▼
┌─ B2.5 Build Messages ──────┐
│  trim history (6 or all)     │
│  build ChatRequest           │
│  set max_tokens, temperature │
└──────┬──────────────────────┘
       │ ChatRequest
       ▼
┌─ B2.6 HTTP POST + Retry ───┐
│  client.post(MLX_URL)        │ ──→ [External: MLX Server]
│  retry 3x with 2s delay     │
└──────┬──────────────────────┘
       │ response (bytes_stream)
       ▼
┌─ B2.7 SSE Stream Parse ────┐
│  for each chunk:             │
│    parse "data: " lines      │
│    skip <think>...</think>   │
│    emit("chat-token", token) │ ──→ [Event: chat-token]
│    on [DONE] → emit(chat-done│ ──→ [Event: chat-done]
│    accumulate full_reply     │
└──────┬──────────────────────┘
       │ full_reply
       ▼
┌─ B2.8 Return ──────────────┐
│  Ok(full_reply)              │
│  busy.store(false)           │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.7 | SSE Stream Parse | 47 LOC, парсинг SSE + фильтрация think + emit — смешаны в одном цикле | Вынести парсинг SSE-строк в отдельную fn, фильтрацию think в отдельную fn | Low |
| B2.1 | Busy Wait | Busy-wait loop 30x500ms — неэффективен, может блокировать 15 секунд | Использовать tokio Semaphore или Notify вместо spin-lock | Medium |
| B2.7 | SSE Stream Parse | Неполные SSE-строки могут потеряться при split('\n') — buffer.rfind('\n') частично решает | Убедиться, что неполные строки корректно буферизуются | Low |
