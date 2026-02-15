# Module 10: Content

## Description

Notes, projects, tasks, and learning items. Provides CRUD storage for user-created content and a structured task/project management system. The chat module creates notes and tasks via action execution.

## Overview

| Attribute        | Value                                                  |
|------------------|--------------------------------------------------------|
| Domain           | Notes, projects, tasks, learning                       |
| Total LOC        | ~520                                                   |
| Backend          | `lib.rs` (L3727-3828, L4330-4457)                     |
| Frontend         | `main.js` (L3162-3287, L3815-3975)                    |
| Backend functions | 3                                                     |
| Frontend functions | 3                                                   |
| Complexity       | Simple: 1, Medium: 4, Complex: 1                      |

## Files

| File                              | Lines          | Role                          |
|-----------------------------------|----------------|-------------------------------|
| `desktop/src-tauri/src/lib.rs`    | L3727-3828     | Notes CRUD                     |
| `desktop/src-tauri/src/lib.rs`    | L4330-4457     | Projects, tasks, learning CRUD |
| `desktop/src/main.js`             | L3162-3287     | Notes UI (list + editor)       |
| `desktop/src/main.js`             | L3815-3975     | Projects, tasks, learning UI   |

## Dependencies

| Direction | Module | Relationship                                       |
|-----------|--------|----------------------------------------------------|
| Uses      | core   | DB access for all CRUD operations                   |
| Used by   | chat   | Action execution creates notes and tasks            |

## Key Concepts

- **Notes**: Full CRUD with list view and inline editor. Notes are standalone documents.
- **Projects & Tasks**: Hierarchical model — projects contain tasks, tasks have status tracking.
- **Learning Items**: Courses and skills with progress tracking. Simpler model than projects.
- **Action Integration**: The chat module's action parser can invoke `create_note`, `create_task`, etc.
