# DanKoe (Personal Development Protocol)

## Purpose
Daily personal development protocol inspired by Dan Koe's methodology. Four core practices for self-improvement with streak tracking.

## DB Tables

| Table | Purpose |
|-------|---------|
| `dan_koe_entries` | Practice completion records (practice_type, date, notes) |

## Views
- **Dashboard** — stats overview (streak, completion rate, total sessions)
- **Contemplation** — reflective thinking practice
- **Pattern Interrupt** — breaking habitual patterns
- **Vision** — future visualization practice
- **Integration** — combining insights into daily life

## Key Commands (Tauri)
- `get_dan_koe_stats` — dashboard statistics

## Relations
- **Mindset** — overlaps with journal (contemplation ~ reflection)
- **Schedule** — daily practice can be tracked via Schedule

## Notable
- 4 core practices: Contemplation, Pattern Interrupt, Vision, Integration
- Streak tracking for consistency
- Considered for removal/merge with Schedule (per memory notes)
- Lightweight — single table, stats-focused
