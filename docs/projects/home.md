# Home

## Purpose
Household supplies and inventory management. Track what you have, what you need to buy, quantities and locations.

## DB Tables

| Table | Purpose |
|-------|---------|
| `home_items` | Household items (name, category, quantity, unit, location, needed, notes) |

## Views
- **Dashboard** — item count, "need to buy" count
- **Table** — DatabaseView with all items, inline editing

## Key Commands (Tauri)
- `get_home_items` / `add_home_item` / `update_home_item` / `delete_home_item` — CRUD
- `toggle_home_item_needed` — quick "need to buy" toggle

## Relations
- Standalone — no direct relations to other tabs

## Notable
- Categories: cleaning, hygiene, household, electronics, tools, other
- "Needed" flag acts as shopping list
- Quantity + unit tracking (e.g., "3 rolls", "500 ml")
- Location field for organizing by room/storage
