# B7 — macOS интеграция AppleScript: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B7: macOS интеграция (AppleScript) |
| Файл(ы) | lib.rs:L2607-2918 |
| LOC | 312 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B7.1 | Флаги доступа к календарю (static AtomicBool) | L2601-2605 | 5 | -- -> AtomicBool | -- | control | Trivial |
| B7.2 | Проверка доступа к Calendar.app | L2607-2629 | 23 | -- -> bool | Shell: osascript; State: CALENDAR_ACCESS_DENIED | validation | Low |
| B7.3 | Выполнение osascript с таймаутом | L2631-2671 | 41 | script: &str -> Result<String> | Shell: osascript spawn+wait; Process: kill on timeout | external | Medium |
| B7.4 | Классификация приложений (productive/distraction) | L2673-2691 | 19 | name: &str -> &str | -- | process | Low |
| B7.5 | Команда chat (обёртка с LlmBusy) | L2693-2707 | 15 | messages -> Result<String> | State: LlmBusy; Network: LLM call | bridge | Low |
| B7.6 | Функция chat_inner: построение промпта | L2709-2805 | 97 | messages, call_mode -> chat_messages[] | DB: read facts (memory context) | transform | High |
| B7.7 | Функция chat_inner: LLM вызов и стриминг | L2807-2878 | 72 | request -> full_reply | Network: POST MLX_URL (SSE stream); Event: chat-token, chat-done | stream | High |
| B7.8 | Файловые команды (read_file, list_dir) | L2883-2914 | 32 | path -> content/entries | File: read/list | query | Low |

## Data Flow

```
[check_calendar_access()]
    │
    ├── APPLE_CALENDAR_DISABLED? ──→ false
    ├── CALENDAR_ACCESS_DENIED?  ──→ false
    └── run_osascript("count of calendars")
            │
            ▼
┌─ B7.3 run_osascript() ──────────────────┐
│  [Input: AppleScript string]             │
│                                          │
│  Command::new("osascript")               │
│    .args(["-e", script])                 │
│    .spawn()                              │ ──→ [Shell: osascript]
│                                          │
│  ┌─ Timeout loop (10s) ─┐               │
│  │  try_wait()            │               │
│  │  Some(status) → read   │               │
│  │  None → sleep 50ms     │               │
│  │  deadline → kill()     │               │
│  └───────────────────────┘               │
│                                          │
│  [Output: Result<String, String>]        │
└──────────────────────────────────────────┘

[classify_app()]
    │
    ▼
┌─ B7.4 ─────────────────────────────────┐
│  productive: code, cursor, terminal...  │
│  distraction: telegram, discord, tiktok │
│  lower.contains() -> "productive" /     │
│  "distraction" / "neutral"              │
└─────────────────────────────────────────┘

[chat() -> chat_inner()]
    │
    ▼
┌─ B7.6 Build Prompt ──────────────────────────────┐
│  1. Вычислить дату/время + 14 дней вперёд         │
│  2. needs_full_prompt() -> выбрать SYSTEM_PROMPT   │
│     или SYSTEM_PROMPT_LITE или call_mode prompt    │
│  3. build_memory_context_from_db() ←── [DB: facts] │
│  4. Обрезать историю (6 msgs для lite)             │
│  5. Собрать ChatRequest                            │
└──────┬───────────────────────────────────────────┘
       │ request
       ▼
┌─ B7.7 LLM Stream ───────────────────────┐
│  POST MLX_URL (retry 3x)                 │ ──→ [Network: MLX]
│  bytes_stream()                           │
│  parse SSE "data: " lines                 │
│  filter <think>...</think>                │
│  emit("chat-token") per token             │ ──→ [Event]
│  emit("chat-done")                        │
│  accumulate full_reply                    │
└──────┬───────────────────────────────────┘
       │
       ▼
[Output: Result<String, String>]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B7.6 | Build Prompt | 97 LOC — построение промпта, memory context, адаптивный выбор — слишком много логики | Вынести в отдельные функции: build_date_context(), select_prompt(), trim_history() | Medium |
| B7.7 | LLM Stream | 72 LOC — retry + SSE парсинг + think-фильтрация + event emit | Вынести SSE парсинг в отдельную функцию | Medium |
| B7.3 | osascript | Timeout 10s — может замедлить UI при частых вызовах | Приемлемо, но стоит мониторить частоту вызовов | Low |
| B7.* | Весь блок | chat/chat_inner не относятся к macOS интеграции — они в диапазоне строк, но логически относятся к модулю chat | Перенести chat_inner в отдельный модуль | Medium |
