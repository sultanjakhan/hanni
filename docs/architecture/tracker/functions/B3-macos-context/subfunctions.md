# B3 — macOS context (events, music, browser): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: macOS context (events, music, browser) |
| Файл(ы) | lib.rs:L3154-3243 |
| LOC | 90 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.1 | get_calendar_events — проверка доступа и AppleScript | L3154-3179 | 26 | — → Result<String> | check_calendar_access(), run_osascript() | external | Low |
| B3.2 | get_now_playing — проверка Music.app | L3181-3198 | 18 | — → Option<String> (Music info) | run_osascript: System Events check + Music query | external | Low |
| B3.3 | get_now_playing — проверка Spotify | L3200-3215 | 16 | — → Option<String> (Spotify info) | run_osascript: System Events check + Spotify query | external | Low |
| B3.4 | get_now_playing — fallback | L3217-3218 | 2 | — → String ("No music") | — | transform | Trivial |
| B3.5 | get_browser_tab — перебор браузеров | L3220-3242 | 23 | — → Result<String> | run_osascript: System Events check + browser URL/title | external | Low |

## Data Flow

```
[Tauri invoke: get_calendar_events]
    │
    ▼
┌─ B3.1 Calendar Events ──────────────────────────┐
│  check_calendar_access()                          │ ──→ [macOS: TCC/Calendar]
│  if !access → return "Access denied"              │
│  run_osascript(Calendar → events next 2 days)     │ ──→ [macOS: Calendar.app via osascript]
│  return formatted events                          │
└──────────────────────────────────────────────────┘

[Tauri invoke: get_now_playing]
    │
    ▼
┌─ B3.2 Check Music.app ──────────────────────────┐
│  osascript: processes contains "Music"            │ ──→ [macOS: System Events]
│  if running: get current track name + artist      │ ──→ [macOS: Music.app]
│  return "Apple Music: ..."                        │
└──────┬───────────────────────────────────────────┘
       │ (Music not running or paused)
       ▼
┌─ B3.3 Check Spotify ────────────────────────────┐
│  osascript: processes contains "Spotify"          │ ──→ [macOS: System Events]
│  if running: get current track name + artist      │ ──→ [macOS: Spotify]
│  return "Spotify: ..."                            │
└──────┬───────────────────────────────────────────┘
       │ (Spotify not running)
       ▼
┌─ B3.4 Fallback ─────────────────────────────────┐
│  return "No music app is currently playing."      │
└──────────────────────────────────────────────────┘

[Tauri invoke: get_browser_tab]
    │
    ▼
┌─ B3.5 Browser Tab ──────────────────────────────┐
│  for (Arc, Chrome, Safari):                       │
│    osascript: processes contains [name]            │ ──→ [macOS: System Events]
│    if running: get URL + title                    │ ──→ [macOS: browser app]
│    return "[name]: URL | title"                   │
│  Fallback: "No supported browser"                 │
└──────────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B3.2-B3.3 | Music/Spotify | Дублирование паттерна (check process → query app) | Вынести generic check_and_query_app() helper | Low |
| B3.5 | Browser tab | Каждый браузер — отдельный osascript вызов (медленно) | Один osascript с несколькими проверками | Low |
