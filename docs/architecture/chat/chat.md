# Chat Module

Протокол чата, LLM streaming, управление диалогами, выполнение действий, автокатегоризация.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~1400 |
| Backend functions | 4 |
| Frontend functions | 6 |
| Complexity | Simple: 1, Medium: 5, Complex: 4 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L1052-1093 | Типы протокола чата |
| Backend | `lib.rs` | L2695-2883 | Стриминг чата с LLM |
| Backend | `lib.rs` | L3375-3611 | CRUD разговоров, извлечение фактов + категоризация |
| Frontend | `main.js` | L271-446 | Список и загрузка разговоров |
| Frontend | `main.js` | L978-1076 | Добавление сообщений в UI |
| Frontend | `main.js` | L1105-1696 | Выполнение действий, отправка, стриминг |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB, types), memory (context), voice (TTS integration) |
| **Used by** | proactive, voice (call mode uses chat) |

## Key Improvements (v0.18.5+)

### Stability Fixes
- **LLM semaphore** in `process_conversation_end` — prevents concurrent MLX inference
- **Proactive race guard** — `if (busy) return;` blocks proactive messages during streaming
- **Skip embed/rerank in call_mode** — 150-500ms savings per voice request (FTS5 only)
- **scrollDown() debounce** — global requestAnimationFrame throttle (was 20-40 reflow/sec)

### New Features
- **CH7: Action indicator** — animated "Выполняю: web_search..." during tool execution
- **CH8: Auto-categorize** — conversations get category (chat/work/health/money/food/hobby/planning/personal) from extraction prompt

### DB Schema
- `conversations.category TEXT` — auto-assigned by extraction LLM

## Improvements Status

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| CH1 | Markdown + code highlighting | S | ✅ |
| CH2 | Regenerate response | S | ✅ |
| CH3 | Copy message button | S | ✅ |
| CH4 | Edit user message + resend | M | ✅ |
| CH5 | Адаптивная история (лимит по токенам) | S | ⬜ |
| CH6 | Implicit feedback loop (retry → training data) | M | ⬜ |
| CH7 | Streaming action indicator | S | ✅ |
| CH8 | Auto-categorize conversations | S | ✅ |
| CH9 | Smart context injection | M | ✅ |
