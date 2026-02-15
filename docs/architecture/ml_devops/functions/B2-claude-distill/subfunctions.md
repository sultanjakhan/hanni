# B2 — Claude distillation: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Claude distillation (extract + refine) |
| Файл(ы) | scripts/claude_distill.sh:L1-274 |
| LOC | 274 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

### Backend (Script)

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B2.1 | Парсинг аргументов и валидация | L1-43 | 43 | CLI args -> FOCUS, COUNT | -- | validation | Low |
| B2.2 | Извлечение промптов из БД (Python inline) | L47-148 | 102 | DB conversations -> prompts.json | DB: SELECT conversations; File: write prompts.json | query | High |
| B2.3 | Валидация извлечённых промптов | L151-158 | 8 | prompts.json -> PROMPT_COUNT | File: read prompts.json | validation | Trivial |
| B2.4 | Подготовка distillation prompt для Claude | L161-203 | 43 | system_prompt + context -> claude_prompt template | File: read system_prompt.txt | transform | Medium |
| B2.5 | Обработка промптов через Claude CLI | L205-233 | 29 | prompt, claude_prompt -> response | Shell: subprocess claude -p; Rate limit: sleep 1s | external | Medium |
| B2.6 | Формирование training examples из ответов | L212-220 | 9 | response + prompt -> example dict | -- | transform | Trivial |
| B2.7 | Сохранение distilled.jsonl | L239-246 | 8 | results[] -> distilled.jsonl | File: write distilled.jsonl | persist | Trivial |
| B2.8 | Мерж с существующими данными и пересплит | L248-272 | 25 | distilled + existing -> train/valid/test.jsonl | File: read/write JSONL | persist | Low |

## Data Flow

```
[Entry: ./claude_distill.sh --focus agentic --count 15]
    │
    ▼
┌─ B2.1 Parse Args ──────────┐
│  --focus: agentic|personality│
│  --count: 15                 │
│  check claude CLI exists     │
└──────┬──────────────────────┘
       │
       ▼
┌─ B2.2 Extract Prompts (Python) ──────────────────────┐
│  sqlite3 -> conversations (message_count >= 2)        │ ←── [DB: hanni.db]
│  extract user messages                                │
│  clean [Current context] prefix                       │
│  deduplicate by first 50 chars                        │
│  score by action_words (if focus=agentic)             │
│  take top N                                           │
│  + synthetic fallback prompts                         │
│  save prompts.json                                    │ ──→ [File: distillation/]
└──────┬───────────────────────────────────────────────┘
       │
       ▼
┌─ B2.3 Validate ─────────┐
│  Check prompts.json exists│
│  Count prompts            │
└──────┬───────────────────┘
       │
       ▼
┌─ B2.4-B2.6 Process via Claude ───────────────────────┐
│  for each prompt:                                     │
│    B2.4: Build distillation prompt                    │
│      "You are helping train Hanni..."                 │
│      + system_prompt + context + user message         │
│                                                       │
│    B2.5: subprocess.run(["claude", "-p", ...])        │ ──→ [External: Claude CLI]
│      timeout: 60s                                     │
│      sleep(1) between calls                           │
│                                                       │
│    B2.6: Create {messages: [system, user, assistant]} │
│      append to results[]                              │
└──────┬───────────────────────────────────────────────┘
       │ results[]
       ▼
┌─ B2.7 Save Distilled ─────────┐
│  write distilled.jsonl          │ ──→ [File: distillation/]
└──────┬─────────────────────────┘
       │
       ▼
┌─ B2.8 Merge & Resplit ──────────────────────────┐
│  read existing train.jsonl                       │ ←── [File: training/]
│  merged = existing + results                     │
│  80/10/10 split                                  │
│  write train.jsonl, valid.jsonl, test.jsonl      │ ──→ [File: training/]
└──────────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.2 | Извлечение промптов | 102 LOC Python внутри bash heredoc — тяжело отлаживать | Вынести в отдельный .py файл | Medium |
| B2.5 | Claude CLI call | Нет retry при ошибках Claude — просто skip | Добавить retry (1-2 попытки) | Low |
| B2.8 | Мерж данных | Нет дедупликации при мерже — одинаковые примеры могут повториться | Добавить дедупликацию по user message | Medium |
| B2.* | Весь скрипт | Два inline Python блока в bash — архитектурный антипаттерн | Переписать как один Python скрипт с CLI args | Medium |
