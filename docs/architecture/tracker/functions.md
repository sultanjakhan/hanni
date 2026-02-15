# Module 9: Tracker — Function Index

## Backend (lib.rs)

| ID | Function Group                          | Type     | Key Functions                                                      | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|----|----------------------------------------|----------|--------------------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B1 | Life Tracker CRUD (JSON)               | endpoint | `tracker_add_purchase`, `tracker_add_time`, `tracker_add_goal`, `tracker_add_note`, `tracker_get_stats`, `tracker_get_recent` | lib.rs:L2937-3057 | 121 | Medium     | 6 | ✅ |
| B2 | Screen Time (knowledgeC.db)            | endpoint | `get_activity_summary`                                             | lib.rs:L3058-3152 | 95  | Complex    | 5 | ✅ |
| B3 | macOS context (events, music, browser) | endpoint | `get_calendar_events`, `get_now_playing`, `get_browser_tab`        | lib.rs:L3154-3243 | 90  | Medium     | 5 | ✅ |
| B4 | Activities (start/stop)                | endpoint | `start_activity`, `stop_activity`, `get_current_activity`, `get_activity_log` | lib.rs:L3614-3726 | 113 | Medium     | 5 | ✅ |
| B5 | Dashboard aggregation                  | endpoint | `get_dashboard_data`                                               | lib.rs:L4675-4736 | 62  | Medium     | 5 | ✅ |

## Frontend (main.js)

| ID | Function Group      | Type | Key Functions      | Location           | LOC | Complexity | Subfuncs | Phase 3 |
|----|---------------------|------|--------------------|--------------------|-----|------------|----------|---------|
| F1 | Dashboard loading   | view | `loadDashboard()`  | main.js:L2986-3055 | 70  | Medium     | 5 | ✅ |

## Summary

- **Total functions**: 6 (5 backend + 1 frontend)
- **Simple**: 0
- **Medium**: 5 (B1, B3, B4, B5, F1)
- **Complex**: 1 (B2)
- **Total subfunctions**: 31
- **Phase 3 complete**: 6/6
