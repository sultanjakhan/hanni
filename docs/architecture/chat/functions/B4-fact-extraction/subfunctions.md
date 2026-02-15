# B4 — Извлечение фактов из разговора: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Извлечение фактов из разговора |
| Файл(ы) | lib.rs:L3499-3611 |
| LOC | 113 |
| Подфункций | 7 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B4.1 | Подготовка текста разговора | L3499-3512 | 14 | messages[] → conv_text | — | transform | Trivial |
| B4.2 | Формирование промпта для извлечения | L3514-3528 | 15 | conv_text → prompt string | — | transform | Trivial |
| B4.3 | Вызов LLM (non-stream) | L3530-3551 | 22 | prompt → NonStreamResponse | HTTP POST (MLX_URL) | external | Medium |
| B4.4 | Извлечение raw-ответа и очистка от think-тегов | L3553-3559 | 7 | NonStreamResponse → cleaned text | — | parse | Trivial |
| B4.5 | Парсинг JSON из ответа | L3561-3583 | 23 | cleaned text → ExtractionResult | — | parse | Low |
| B4.6 | Обновление summary разговора | L3585-3596 | 12 | summary, conversation_id → () | DB UPDATE conversations | persist | Trivial |
| B4.7 | Сохранение извлечённых фактов | L3598-3610 | 13 | facts[] → () | DB INSERT/UPDATE facts (UPSERT) | persist | Low |

## Data Flow

```
[Input: messages[], conversation_id, app]
    │
    ▼
┌─ B4.1 Build conv_text ─────┐
│  filter user+assistant msgs  │
│  join "role: content"        │
└──────┬──────────────────────┘
       │ conv_text
       ▼
┌─ B4.2 Build Prompt ────────┐
│  extraction instructions     │
│  categories + examples       │
│  append conv_text + /no_think│
└──────┬──────────────────────┘
       │ prompt
       ▼
┌─ B4.3 LLM Call ────────────┐
│  POST MLX_URL (stream=false) │ ──→ [External: MLX Server]
│  max_tokens=800, temp=0.3    │
└──────┬──────────────────────┘
       │ NonStreamResponse
       ▼
┌─ B4.4 Clean Response ──────┐
│  extract raw text            │
│  strip <think>...</think>    │
└──────┬──────────────────────┘
       │ cleaned text
       ▼
┌─ B4.5 Parse JSON ──────────┐
│  find { ... } in text        │
│  deserialize ExtractionResult│
│  { summary, facts[] }       │
└──────┬──────────────────────┘
       │ ExtractionResult
       ▼
┌─ B4.6 Update Summary ──────┐
│  UPDATE conversations        │ ──→ [DB: conversations]
│  SET summary, ended_at       │
└──────┬──────────────────────┘
       │
       ▼
┌─ B4.7 Save Facts ──────────┐
│  for each fact:              │
│    INSERT INTO facts         │ ──→ [DB: facts]
│    ON CONFLICT DO UPDATE     │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B4.5 | Парсинг JSON | Поиск JSON через find('{') / rfind('}') — хрупкий, может сломаться на вложенных скобках | Использовать более надёжный JSON-экстрактор (напр. regex для ```json блоков) | Medium |
| B4.3 | Вызов LLM | Нет retry при ошибке HTTP — в отличие от chat() | Добавить retry (1-2 попытки) | Low |
| B4.7 | Сохранение фактов | Ошибки INSERT игнорируются (let _ =) — факт может молча не сохраниться | Логировать ошибки хотя бы в stderr | Low |
