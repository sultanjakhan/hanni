# Schedule

## Purpose
Central hub for all recurring routines, habits, and periodic tasks. The backbone of daily tracking across all life areas.

## DB Tables

| Table | Purpose |
|-------|---------|
| `schedules` | Routine definitions (name, frequency, time_of_day, category, project link) |
| `schedule_completions` | Daily completion records (schedule_id, date, status, time, note) |

## Views
- **Table** — all routines with inline completion toggles
- **Tracking** — heatmap/streak visualization of completions

## Key Commands (Tauri)
- `get_schedules` / `create_schedule` / `update_schedule` / `delete_schedule` — routine CRUD
- `get_schedule_completions` / `toggle_schedule_completion` — daily tracking
- `get_schedule_stats` — streak and completion statistics

## Relations
- **Calendar** — schedules appear on calendar as recurring events
- **Health** — medication courses tracked here
- **Sports** — workout routines tracked here
- **Food** — meal prep routines tracked here
- **Development** — learning schedules tracked here
- **All tabs** — "Project" column links schedule items to their source tab

## Notable
- **Central tracking hub** — other tabs store reference data, Schedule handles the daily tracking
- Frequency: daily, weekly, specific days, custom intervals
- Time of day: morning, afternoon, evening, anytime
- "Project" column links to source tab (Health, Sports, etc.)
- Streak visualization for motivation
- Designed for cross-project sharing (e.g., show doctor only Health-related schedules)
