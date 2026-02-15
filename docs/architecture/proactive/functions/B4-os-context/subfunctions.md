# B4 — Сбор контекста ОС: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B4: Сбор контекста ОС |
| Файл(ы) | lib.rs:L6483-6561 |
| LOC | 79 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B4.1 | Инициализация + время | L6483-6485 | 3 | — → ctx с текущим временем | — | transform | Trivial |
| B4.2 | Screen Time (SQLite) | L6487-6490 | 4 | — → activity summary | DB SELECT (KnowledgeC.db) | query | Trivial |
| B4.3 | Календарные события (AppleScript) | L6492-6516 | 25 | — → calendar events string | osascript (Calendar.app) | external | Medium |
| B4.4 | Now Playing + Browser tab | L6518-6528 | 11 | — → music + browser strings | osascript (Music.app, browser) | external | Low |
| B4.5 | Active App + Focus time + distraction alert | L6530-6542 | 13 | — → frontmost app + focus minutes + alert | osascript + DB query | external | Low |
| B4.6 | Upcoming events (within 60 min) | L6544-6549 | 6 | — → upcoming events string | DB query (events table) | query | Trivial |
| B4.7 | Morning digest (условный, 8-10 утра) | L6551-6559 | 9 | hour → morning digest data | DB queries (events, mood, health, goals) | query | Low |

## Data Flow

```
[Input: none (reads system state)]
    │
    ▼
┌─ B4.1 Init Time ───────────┐
│  chrono::Local::now()        │
│  format "HH:MM Day, Date"   │
└──────┬──────────────────────┘
       │ ctx = "Current time: ..."
       ▼
┌─ B4.2 Screen Time ─────────┐
│  gather_screen_time()        │ ←── [DB: KnowledgeC.db]
│  append to ctx               │
└──────┬──────────────────────┘
       │
       ▼
┌─ B4.3 Calendar ────────────┐
│  check_calendar_access()     │
│  run_osascript(cal_script)   │ ──→ [System: Calendar.app]
│  append to ctx               │
└──────┬──────────────────────┘
       │
       ▼
┌─ B4.4 Music + Browser ─────┐
│  get_now_playing_sync()      │ ──→ [System: Music.app]
│  get_browser_url()           │ ──→ [System: Browser]
│  append to ctx               │
└──────┬──────────────────────┘
       │
       ▼
┌─ B4.5 Active App ──────────┐
│  get_frontmost_app()         │ ──→ [System: frontmost app]
│  get_app_focus_minutes()     │ ←── [DB: KnowledgeC.db]
│  check distraction threshold │
│  append to ctx               │
└──────┬──────────────────────┘
       │
       ▼
┌─ B4.6 Upcoming Events ─────┐
│  get_upcoming_events_soon()  │ ←── [DB: events]
│  append to ctx               │
└──────┬──────────────────────┘
       │
       ▼
┌─ B4.7 Morning Digest ──────┐
│  if hour 8-10:               │
│    gather_morning_digest()   │ ←── [DB: events, mood, health, goals]
│    append to ctx             │
└──────┬──────────────────────┘
       │
       ▼
[Output: ctx string with all OS context]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B4.3 | Calendar | AppleScript может зависнуть при проблемах с Calendar.app — timeout 30s на уровне gather_context | Добавить отдельный timeout на каждый osascript вызов | Low |
| B4.5 | Active App | Hardcoded список "distracting" приложений — не настраивается пользователем | Вынести в настройки или в память | Low |
