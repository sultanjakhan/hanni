# Hobbies (Media Collections)

## Purpose
Track media consumption — movies, anime, series, books, games, music, manga, podcasts, cartoons, and custom types. Rate, review, and organize into lists.

## DB Tables

| Table | Purpose |
|-------|---------|
| `media_items` | Media entries (title, type, status, rating, progress, episodes, notes) |
| `user_lists` | Custom lists (e.g., "Top 10 Anime", "Watch Later") |
| `list_items` | Items within user lists |

## Views
- **By media type** — 10 types: music, movies, anime, series, books, games, manga, podcast, cartoon, custom
- **Gallery** — cover image grid
- **Kanban** — columns by status
- **Table** — DatabaseView tabular format

## Key Commands (Tauri)
- `get_media_items` / `add_media_item` / `update_media_item` / `delete_media_item` — CRUD
- `get_media_stats` — statistics per type
- `create_user_list` / `get_user_lists` — custom list management

## Relations
- Standalone — no direct relations to other tabs

## Notable
- Statuses: planned, in_progress, completed, on_hold, dropped
- Episode/chapter progress tracking for serial media
- Rating system (1-10)
- Cover images support
- Custom media types beyond the 10 built-in
