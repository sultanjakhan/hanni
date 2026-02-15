# Memory Module

Факты памяти, FTS5 поиск, контекст для LLM, браузер памяти.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~620 |
| Backend functions | 5 |
| Frontend functions | 3 |
| Complexity | Simple: 4, Medium: 2, Complex: 1 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L880-1025 | Миграция JSON, построение контекста |
| Backend | `lib.rs` | L3246-3374 | CRUD фактов, FTS5 поиск |
| Backend | `lib.rs` | L4738-4800 | Браузер памяти (все факты) |
| Frontend | `main.js` | L2498-2773 | Вкладка памяти, рендер, поиск |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB) |
| **Used by** | chat (memory context injection), proactive (user name lookup) |
