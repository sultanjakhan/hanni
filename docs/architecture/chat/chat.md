# Chat Module

Протокол чата, LLM streaming, управление диалогами, выполнение действий.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~1340 |
| Backend functions | 4 |
| Frontend functions | 6 |
| Complexity | Simple: 1, Medium: 5, Complex: 4 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L1052-1093 | Типы протокола чата |
| Backend | `lib.rs` | L2695-2883 | Стриминг чата с LLM |
| Backend | `lib.rs` | L3375-3611 | CRUD разговоров, извлечение фактов |
| Frontend | `main.js` | L271-446 | Список и загрузка разговоров |
| Frontend | `main.js` | L978-1076 | Добавление сообщений в UI |
| Frontend | `main.js` | L1105-1696 | Выполнение действий, отправка, стриминг |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB, types), memory (context), voice (TTS integration) |
| **Used by** | proactive, voice (call mode uses chat) |
