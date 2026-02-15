# System Module

Shell-команды, URL, clipboard, уведомления, web search, файлы, MLX сервер, API.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~540 |
| Backend functions | 8 |
| Frontend functions | 0 |
| Complexity | Simple: 4, Medium: 2, Complex: 2 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L1999-2130 | Shell, URL, clipboard, web search |
| Backend | `lib.rs` | L2322-2936 | API сервер, MLX, AppleScript, файлы |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (app state) |
| **Used by** | proactive (osascript), tracker (classify_app) |
