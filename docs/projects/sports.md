# Sports

## Purpose
Fitness and workout tracking — gym sessions, cardio, yoga, martial arts. Body metrics and exercise logging.

## DB Tables

| Table | Purpose |
|-------|---------|
| `workouts` | Workout sessions (type, date, duration, calories, notes) |
| `exercises` | Individual exercises within a workout (name, sets, reps, weight) |
| `body_records` | Body zone tracking — pain/injury records (v0.28.0+) |

## Views
- **Workouts** — session list with exercise details
- **Martial Arts** — specialized martial arts training view
- **Stats** — body zones summary, workout statistics

## Key Commands (Tauri)
- `get_workouts` / `create_workout` / `update_workout` / `delete_workout` — workout CRUD
- `get_workout_stats` — aggregated statistics
- `get_body_zones_summary` / `get_body_records` / `update_body_record` / `delete_body_record` — body tracking

## Relations
- **Health** — complements health metrics (sports = active, health = passive)
- **Schedule** — workout routines can be tracked via Schedule
- **Focus** — sport activity type in focus timer

## Notable
- Workout types: gym, cardio, yoga, swimming, martial arts
- Exercise tracking: sets, reps, weight per exercise
- Body zone mapping for pain/injury tracking (v0.28.0+)
- Calorie burn estimation
