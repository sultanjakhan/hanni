# People

## Purpose
Contact and relationship management. Store contact info, mark favorites, block contacts, track birthdays.

## DB Tables

| Table | Purpose |
|-------|---------|
| `contacts` | Contact records (name, phone, email, birthday, notes, favorite, blocked) |
| `contact_blocks` | Blocked contact entries |

## Views
- **All** — full contact list
- **Favorites** — starred contacts
- **Blocked** — blocked contacts

## Key Commands (Tauri)
- `get_contacts` / `add_contact` / `update_contact` / `delete_contact` — CRUD
- `toggle_contact_favorite` / `toggle_contact_blocked` — quick actions
- `add_contact_block` / `delete_contact_block` — block management

## Relations
- Standalone — no direct relations to other tabs

## Notable
- Favorite/blocked flags for quick filtering
- Birthday tracking for reminders
- Contact blocks for privacy management
