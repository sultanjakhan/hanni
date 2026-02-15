# System — Function Index

## Backend

| ID | Функция | Тип | Entry | Файл:строки | LOC | Cmplx | Subfuncs | Phase 3 |
|----|---------|-----|-------|-------------|-----|-------|----------|---------|
| B1 | Выполнение shell-команд | endpoint | run_shell | lib.rs:L1999-2030 | 32 | Simple | — | — |
| B2 | Системные команды (URL, уведомления, громкость) | endpoint | open_url, send_notification, set_volume | lib.rs:L2032-2057 | 26 | Simple | — | — |
| B3 | Буфер обмена | endpoint | get/set_clipboard | lib.rs:L2059-2078 | 20 | Simple | — | — |
| B4 | Веб-поиск (DuckDuckGo) | endpoint | web_search | lib.rs:L2082-2130 | 49 | Medium | 5 | ✅ |
| B5 | REST API сервер (port 8235) | internal | API server setup in run() | lib.rs:L2344-2521 | 178 | Complex | 9 | ✅ |
| B6 | Управление MLX сервером | internal | find_python, start_mlx_server | lib.rs:L2525-2605 | 81 | Medium | 6 | ✅ |
| B7 | macOS интеграция (AppleScript) | internal | check_calendar_access, run_osascript, classify_app | lib.rs:L2607-2918 | 312 | Complex | 8 | ✅ |
| B8 | Файловые операции | endpoint | read_file, list_dir | lib.rs:L2885-2936 | 52 | Simple | — | — |

## Summary

| Metric | Value |
|--------|-------|
| Total functions | 8 |
| Simple | 4 |
| Medium | 2 |
| Complex | 2 |
| Total subfunctions | 28 |
