# Screen Time (АФК/Компьютер)

## Purpose
Automatic screen time tracking and productivity analysis. Understand how you spend time at the computer — what's productive, what's not, and when you're most focused.

**Key difference from Focus:** Focus = manual deep work sessions. Screen Time = passive background tracking of everything.

## DB Tables

| Table | Purpose |
|-------|---------|
| `activity_snapshots` | Automatic snapshots every ~1 min (app, url, idle, category) |

### Schema

| Column | Type | Purpose |
|--------|------|---------|
| `captured_at` | TEXT | Timestamp |
| `hour` | INTEGER | Hour of day (0-23) |
| `weekday` | INTEGER | Day of week |
| `frontmost_app` | TEXT | Active application |
| `browser_url` | TEXT | Current URL if in browser |
| `window_title` | TEXT | Window title |
| `music_playing` | TEXT | Currently playing track |
| `productive_min` | REAL | Productive minutes (0 if AFK) |
| `distraction_min` | REAL | Distraction minutes (0 if AFK) |
| `idle_secs` | REAL | Seconds of inactivity |
| `category` | TEXT | `afk`, `productive`, `distraction`, `other` |
| `screen_locked` | INTEGER | Screen locked flag |

## Views (planned)

- **Dashboard** — screen time today, productive %, trend vs last week
- **Apps** — top apps by usage time, categorized
- **Sites** — top visited sites by time
- **Heatmap** — productivity by hour of day (when you're most focused)
- **Table** — raw data in DatabaseView format

## AFK Logic (already implemented)

- Idle >= 2 min OR screen locked → `category = "afk"`, 0 prod/distraction
- Idle > 30 min → not recorded (saves DB space)
- Data retained for 30 days, auto-cleanup

## Status
- Data collection: **working** (background task in lib.rs)
- UI tab: **not built yet**
- Tab ID: `screen-time` (planned)
