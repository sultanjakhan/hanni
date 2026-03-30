# Calendar

## Purpose
Calendar with events, recurring schedules, and integrations with Apple Calendar and Google ICS feeds.

## DB Tables

| Table | Purpose |
|-------|---------|
| `events` | One-time and recurring events (title, date, time, recurrence rule, category) |
| `schedules` | Recurring routines (shared with Schedule tab) |
| `schedule_completions` | Daily completion records for schedules |

## Views
- **Month** — grid calendar with event dots
- **Week** — 7-day column view
- **Day** — single day timeline
- **List** — chronological event list
- **Table** — DatabaseView tabular format

## Key Commands (Tauri)
- `get_events` / `create_event` / `update_event` / `delete_event` — event CRUD
- `get_schedules` / `get_schedule_completions` / `toggle_schedule_completion` — schedule tracking
- `get_activity_log` — focus session history overlay

## Relations
- **Schedule** — shares `schedules` + `schedule_completions` tables
- **Focus** — activity log can overlay on calendar
- **Notes** — tasks with due dates appear on calendar

## Notable
- Apple Calendar sync via `EventKit` (Rust native)
- Google Calendar sync via ICS URL import
- RRULE support for complex recurrence patterns
- Events can have categories: work, personal, health, social, etc.
