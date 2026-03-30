# Health

## Purpose
Daily health metrics and habit tracking. Monitor sleep, water intake, mood, weight. Build and track habits with streaks.

## DB Tables

| Table | Purpose |
|-------|---------|
| `habits` | Habit definitions (name, frequency, category, streak) |
| `habit_checks` | Daily habit completion records |
| `health_log` | Daily health metrics (sleep_hours, water_ml, weight_kg, mood, notes) |

## Views
- **Habits** — daily checklist with streak indicators
- **Body** — health metrics (sleep, water, weight, mood) with history

## Key Commands (Tauri)
- `get_habits_today` / `create_habit` / `update_habit` / `delete_habit` — habit management
- `check_habit` — mark habit as done for today
- `get_health_today` / `log_health` — daily health metrics

## Relations
- **Schedule** — medication courses and health routines link to Schedule
- **Sports** — complements active tracking (health = passive metrics)
- **Mindset** — mood tracking overlaps with mood_log

## Notable
- Streak tracking with automatic calculation
- Frequency options: daily, weekly, custom days
- Health metrics: sleep (hours), water (ml), weight (kg), mood (1-5)
- Designed for medication tracking via Schedule integration
