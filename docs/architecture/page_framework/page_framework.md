# Module 13: Page Framework

## Description

Meta-layer for pages: page metadata, property definitions and values, database view configurations (table/board/gallery/list), per-tab goals, application settings (key-value), integrations info, and the filter/sort/inline-edit UI framework. Used by all UI tabs.

## Overview

| Attribute        | Value                                                                 |
|------------------|-----------------------------------------------------------------------|
| Domain           | Page meta, properties, database views, goals, settings, integrations  |
| Total LOC        | ~1610                                                                 |
| Backend          | `lib.rs` (L2133-2319, L5660-5741, L5959-6384)                       |
| Frontend         | `main.js` (L117-130, L532-599, L2774-2985, L3976-4597)              |
| Backend functions | 8                                                                    |
| Frontend functions | 6                                                                  |
| Complexity       | Simple: 4, Medium: 2, Complex: 8                                     |

## Files

| File                              | Lines          | Role                                    |
|-----------------------------------|----------------|-----------------------------------------|
| `desktop/src-tauri/src/lib.rs`    | L2133-2319     | Training stats/export, message ratings   |
| `desktop/src-tauri/src/lib.rs`    | L5660-5741     | Goals CRUD, app settings                 |
| `desktop/src-tauri/src/lib.rs`    | L5959-6384     | Page meta, properties, views, integrations |
| `desktop/src/main.js`             | L117-130       | Tab registry constants                    |
| `desktop/src/main.js`             | L532-599       | Goals widget                              |
| `desktop/src/main.js`             | L2774-2985     | Settings, integrations, blocklist, about  |
| `desktop/src/main.js`             | L3976-4597     | Database view framework, properties, filters |

## Dependencies

| Direction | Module    | Relationship                                         |
|-----------|-----------|------------------------------------------------------|
| Uses      | core      | DB access for all CRUD and configuration endpoints    |
| Used by   | ALL UI tabs | Goals widget, property system, view configs, settings |

## Key Concepts

- **Page Metadata**: Each tab/page has metadata (title, icon, description) stored in DB.
- **Property System**: Dynamic property definitions (text, number, select, date, etc.) and per-row values. Powers the Notion-like database views.
- **Database Views**: Table, board, gallery, list layouts with configurable columns, filters, and sorts. Inline editing support.
- **Goals Widget**: Per-tab goals with title, target, current value, and progress bar rendering.
- **App Settings**: Key-value store for application-wide preferences (proactive interval, TTS voice, etc.).
- **Integrations**: Reports on connected services (MLX server, TTS, etc.) and model info.
- **Training Stats/Export**: Exports rated messages as training data for LoRA fine-tuning.
