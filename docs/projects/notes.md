# Notes

## Purpose
Quick notes with tagging, status tracking, multiple view modes. Lightweight task/note hybrid.

## DB Tables

| Table | Purpose |
|-------|---------|
| `notes` | Notes (title, content, status, due_date, tab_id, pinned, archived) |
| `note_tags` | Tag definitions with colors |

## Views
- **List** — simple note list with filters
- **Kanban** — columns by status (task/done/archive)
- **Timeline** — chronological view
- **Table** — DatabaseView tabular format
- **Gallery** — card grid view

## Key Commands (Tauri)
- `get_notes` / `create_note` / `update_note` / `delete_note` — CRUD
- `get_note_tags` / `set_note_tag_color` — tag management
- `toggle_note_pin` / `toggle_note_archive` — quick actions
- `update_note_status` / `reorder_notes` — organization

## Relations
- **Calendar** — notes with due dates appear on calendar
- Notes can be linked to any tab via `tab_id`

## Notable
- Status flow: task → done → archive
- Pin/archive for quick organization
- Full-text search via content
- Tags with custom colors
- Drag-and-drop reordering in Kanban view
