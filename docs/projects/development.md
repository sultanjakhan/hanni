# Development

## Purpose
Learning resource tracker — courses, books, skills, articles. Track progress and status of educational content.

## DB Tables

| Table | Purpose |
|-------|---------|
| `learning_items` | Learning resources (title, type, status, progress, url, notes) |

## Views
- **All** — all items
- **Courses** / **Books** / **Skills** / **Articles** — filtered by type
- Kanban board view (by status)

## Key Commands (Tauri)
- `get_learning_items` / `create_learning_item` / `update_learning_item` / `delete_learning_item` — CRUD
- `update_learning_item_status` — quick status change

## Relations
- **Schedule** — periodic learning goals can be tracked via Schedule

## Notable
- Item types: course, book, skill, article
- Statuses: planned, in_progress, completed
- Progress percentage tracking
- URL field for linking to external resources
