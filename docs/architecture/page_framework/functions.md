# Module 13: Page Framework — Function Index

## Backend (lib.rs)

| ID | Function Group                        | Type     | Key Functions                                          | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|----|--------------------------------------|----------|--------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B1 | Training stats and export            | endpoint | `get_training_stats`, `export_training_data`           | lib.rs:L2133-2293 | 161 | Complex    | 9        | ✅       |
| B2 | Message ratings                      | endpoint | `rate_message`, `get_message_ratings`                  | lib.rs:L2295-2319 | 25  | Simple     | —        | —       |
| B3 | CRUD goals (per-tab)                 | endpoint | `create_goal`, `get_goals`, `update_goal`, `delete_goal` | lib.rs:L5660-5716 | 57  | Simple     | —        | —       |
| B4 | App settings (key-value)             | endpoint | `set_app_setting`, `get_app_setting`                   | lib.rs:L5717-5741 | 25  | Simple     | —        | —       |
| B5 | Page metadata                        | endpoint | `get_page_meta`, `update_page_meta`                    | lib.rs:L5959-5993 | 35  | Simple     | —        | —       |
| B6 | CRUD properties (definitions+values) | endpoint | Property definitions CRUD + property values CRUD       | lib.rs:L5994-6105 | 112 | Medium     | 7        | ✅       |
| B7 | View configurations                  | endpoint | `get_view_config`, `create_view_config`, `update_view_config` | lib.rs:L6106-6384 | 279 | Complex    | 5        | ✅       |
| B8 | Integrations info                    | endpoint | `get_integrations`, `get_model_info`                   | lib.rs:L6151-6345 | 195 | Complex    | 8        | ✅       |

## Frontend (main.js)

| ID | Function Group                   | Type    | Key Functions                                                             | Location           | LOC | Complexity | Subfuncs | Phase 3 |
|----|----------------------------------|---------|---------------------------------------------------------------------------|--------------------|----|------------|----------|---------|
| F1 | Goals widget                     | view    | `loadGoalsWidget`, `showAddGoalModal`                                     | main.js:L532-601   | 70  | Medium     | 5        | ✅       |
| F2 | Chat settings (proactive, TTS)   | view    | `loadChatSettings`                                                        | main.js:L714-963   | 250 | Complex    | 9        | ✅       |
| F3 | Integrations and settings        | view    | `loadIntegrations`, `loadSettings`, `loadBlocklist`, `loadAbout`          | main.js:L2774-2985 | 212 | Complex    | 9        | ✅       |
| F4 | Database view framework          | view    | `renderDatabaseView`, `startInlineEdit`                                   | main.js:L3976-4202 | 227 | Complex    | 10       | ✅       |
| F5 | Property management (UI)         | view    | `showAddPropertyModal`, `showColumnMenu`                                  | main.js:L4213-4428 | 216 | Complex    | 8        | ✅       |
| F6 | Filters and sorting              | handler | `renderFilterBar`, `showFilterBuilderModal`, `applyFilters`, `saveFilters`, `loadFilters` | main.js:L4429-4597 | 169 | Complex    | 7        | ✅       |

## Summary

- **Total functions**: 14 (8 backend + 6 frontend)
- **Simple**: 4 (B2, B3, B4, B5)
- **Medium**: 2 (B6, F1)
- **Complex**: 8 (B1, B7, B8, F2, F3, F4, F5, F6)
- **Total subfunctions**: 77 (29 backend + 48 frontend)
