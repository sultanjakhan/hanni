# Module 9: Tracker

## Description

Life tracker (JSON-based), activities (start/stop), screen time via knowledgeC.db, macOS context (calendar, music, browser), and dashboard aggregation. Provides the data layer that feeds proactive messaging and the dashboard UI.

## Overview

| Attribute        | Value                                                                 |
|------------------|-----------------------------------------------------------------------|
| Domain           | Life tracking, screen time, activities, dashboard                     |
| Total LOC        | ~600                                                                  |
| Backend          | `lib.rs` (L168-180, L2918-3057, L3058-3243, L3614-3726, L4675-4736) |
| Frontend         | `main.js` (L2986-3055)                                               |
| Backend functions | 5                                                                    |
| Frontend functions | 1                                                                  |
| Complexity       | Simple: 0, Medium: 5, Complex: 1                                     |

## Files

| File                              | Lines          | Role                              |
|-----------------------------------|----------------|-----------------------------------|
| `desktop/src-tauri/src/lib.rs`    | L168-180       | Tracker struct definitions         |
| `desktop/src-tauri/src/lib.rs`    | L2918-3057     | Life Tracker CRUD (JSON)           |
| `desktop/src-tauri/src/lib.rs`    | L3058-3243     | Screen time + macOS context        |
| `desktop/src-tauri/src/lib.rs`    | L3614-3726     | Activity start/stop/log            |
| `desktop/src-tauri/src/lib.rs`    | L4675-4736     | Dashboard aggregation              |
| `desktop/src/main.js`             | L2986-3055     | Dashboard UI                       |

## Dependencies

| Direction | Module     | Relationship                                      |
|-----------|------------|---------------------------------------------------|
| Uses      | core       | DB access, JSON file storage                       |
| Uses      | system     | AppleScript for apps, music, browser context       |
| Used by   | proactive  | Supplies context data for proactive LLM calls      |
| Used by   | dashboard  | Dashboard aggregation consumes tracker data        |

## Key Concepts

- **Life Tracker CRUD**: Purchases, time entries, goals, and notes stored as JSON. Endpoints for add/get/stats.
- **Screen Time**: Reads macOS `knowledgeC.db` to extract per-app usage summaries.
- **macOS Context**: AppleScript-based queries for calendar events, Now Playing music, and active browser tab.
- **Activities**: Start/stop timer model with activity log and current-activity state.
- **Dashboard**: Aggregates data from all tracker sources into a single `get_dashboard_data` response.
