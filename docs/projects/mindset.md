# Mindset

## Purpose
Personal reflection and mental wellness. Daily journaling, mood tracking, and personal principles/values management.

## DB Tables

| Table | Purpose |
|-------|---------|
| `journal_entries` | Daily journal (gratitude, wins, struggles, reflection, date) |
| `mood_log` | Mood records with timestamp and notes |
| `principles` | Personal principles/values (title, description, category) |

## Views
- **Journal** — daily entry form with structured prompts
- **Mood** — mood history timeline/chart
- **Principles** — personal values and rules list

## Key Commands (Tauri)
- `get_journal_entry` / `get_journal_entries` / `save_journal_entry` — journal CRUD
- `get_mood_history` / `log_mood` — mood tracking
- `get_principles` / `create_principle` / `update_principle` / `delete_principle` — principles CRUD

## Relations
- **Health** — mood data complements health metrics
- **DanKoe** — journal practice overlaps with DanKoe contemplation

## Notable
- Structured journal: gratitude, wins, struggles, reflection sections
- Mood scale for quick daily logging
- Principles as long-term personal guidelines
- Historical view for pattern recognition
