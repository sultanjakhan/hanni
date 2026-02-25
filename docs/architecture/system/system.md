# System Module

Shell-команды, URL, clipboard, уведомления, web search, файлы, MLX сервер, API, focus mode.

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
| Backend | `lib.rs` | L3257-3303 | run_shell (whitelist + metachar blacklist) |
| Backend | `lib.rs` | L3305-3435 | open_url, send_notification, set_volume, music_control, reminders |
| Backend | `lib.rs` | L3437-3483 | web_search (DuckDuckGo HTML scrape) |
| Backend | `lib.rs` | L3072-3206 | Focus mode (block sites via /etc/hosts) |
| Backend | `lib.rs` | L5118-5139 | get_browser_tab (AppleScript, hardcoded browsers) |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (app state) |
| **Used by** | proactive (osascript context), chat (action execution) |

## Security

### Protections in Place
- **run_shell()**: 18 allowed command prefixes + shell metacharacter blacklist + 500 char limit
- **AppleScript**: All user content escaped (`\\`, `"`) before interpolation
- **Focus mode**: App/site names sanitized to `[a-zA-Z0-9.-]`
- **open_url()**: Scheme validation (http/https only)
- **SQL**: All queries parameterized (no string interpolation)
- **Plist XML**: escape_plist_xml() for LaunchAgent generation

### Key Improvements (v0.18.6)
- **XSS fix in confirmation modal** — innerHTML → textContent for user-supplied action descriptions
- **Expanded dangerous actions** — added open_app and start_focus to confirmation list
- **Descriptions for new action types** — focus and open_app show specific info in modal

## Improvements Status

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| S1 | App control (open/close/switch) | S | ✅ |
| S2 | Music control (play/pause/next) | S | ✅ |
| S3 | Timers & reminders (scheduler + push) | M | ✅ |
| S4 | Better web search (full page fetch) | M | ⬜ |
| S5 | Dangerous action confirmation (UI modal) | M | ✅ |
| S6 | Brightness / Night Shift / DND control | S | ⬜ |
| S7 | Screenshot + analysis | M | ⬜ |
