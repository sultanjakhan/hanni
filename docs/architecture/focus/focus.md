# Focus Module

Режим фокуса, блокировка сайтов/приложений через /etc/hosts, persistent blocklist.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~350 |
| Backend functions | 5 |
| Frontend functions | 2 |
| Complexity | Simple: 5, Medium: 1, Complex: 1 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L1805-1995 | Запуск/остановка фокуса, blocklist config |
| Backend | `lib.rs` | L5613-5659 | CRUD persistent блок-листа |
| Frontend | `main.js` | L261-267 | Событие завершения фокуса |
| Frontend | `main.js` | L3056-3161 | UI режима фокуса |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB), system (osascript for sudo) |
| **Used by** | tracker (focus status in dashboard) |
