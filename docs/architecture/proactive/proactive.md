# Proactive Module

Проактивные сообщения, сбор контекста ОС, стили, engagement tracking.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~690 |
| Backend functions | 8 |
| Frontend functions | 2 |
| Complexity | Simple: 4, Medium: 3, Complex: 2 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L181-271 | Типы и настройки проактивности |
| Backend | `lib.rs` | L1026-1050 | Сохранение/загрузка настроек |
| Backend | `lib.rs` | L6385-6905 | Промпт, контекст ОС, screen time, LLM loop |
| Frontend | `main.js` | L41-56 | Константы |
| Frontend | `main.js` | L164-198 | Обработчик сообщений, tracking набора |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB), memory (user name), tracker (context data), system (osascript) |
| **Used by** | chat (proactive triggers) |
