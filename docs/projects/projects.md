# Projects (Custom Pages)

## Purpose
User-created pages with Notion-style dynamic properties. The extensibility layer of Hanni — users can create any project structure they need.

## DB Tables

| Table | Purpose |
|-------|---------|
| `custom_pages` | Page/project definitions (title, icon, description, page_type, sub_tabs, content_blocks) |
| `project_records` | Records within a project page |
| `property_definitions` | Custom field definitions (name, type, options, position) |
| `property_values` | Field values for each record |
| `page_meta` | Page metadata (emoji, title, description) |
| `view_configs` | Saved view configurations (filters, sorts) |
| `tab_page_blocks` | Block editor content per tab/sub-tab |

## Views
- Dynamic — defined by user via sub_tabs JSON
- Each sub-tab can have its own block editor content
- DatabaseView with custom columns from property_definitions

## Key Commands (Tauri)
- `create_custom_page` / `get_custom_pages` / `get_custom_page` / `update_custom_page` / `delete_custom_page` — page CRUD
- `get_project_records` / `create_project_record` / `update_project_record` / `delete_project_record` — record CRUD
- `get_property_definitions` / `create_property_definition` / `update_property_definition` — field schema
- `get_property_values` / `set_property_value` — field data
- `get_tab_blocks` / `save_tab_blocks` — block editor content
- `get_view_config` / `save_view_config` — saved views

## Relations
- **Schedule** — records can be linked to schedules via relations
- **Any tab** — block editor content can reference any tab via tab_id

## Notable
- `page_type`: 'page' (content only) vs 'project' (has records + properties)
- Property types: text, number, select, multi-select, date, checkbox, url, relation
- Notion-style column system — the foundation for future dynamic columns across all tabs
- Sub-tabs stored as JSON array in `custom_pages.sub_tabs`
