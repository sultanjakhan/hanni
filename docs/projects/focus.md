# Focus

## Purpose
Deep work timer with activity categorization, session history, and focus mode overlay.

## DB Tables

| Table | Purpose |
|-------|---------|
| `activities` | Focus sessions (type, start/end time, duration, notes) |

## Views
- **Current** — active timer with controls (start/stop/pause)
- **History** — past sessions list with stats

## Key Commands (Tauri)
- `start_activity` / `stop_activity` / `get_current_activity` — timer control
- `get_all_activities` / `get_activity_log` — history
- `update_activity` / `delete_activity` — edit past sessions
- `toggle_focus_overlay` — fullscreen focus mode

## Relations
- **Calendar** — sessions appear as activity log overlay
- **Health** — focus blocking can be toggled

## Notable
- Activity types: work, study, sport, rest, hobby
- Pomodoro-style timer support
- Focus overlay blocks distractions (fullscreen mode)
- Session duration tracking with automatic stop
