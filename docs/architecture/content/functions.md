# Module 10: Content — Function Index

## Backend (lib.rs)

| ID | Function Group            | Type     | Key Functions                                                    | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|----|--------------------------|----------|------------------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B1 | CRUD notes               | endpoint | `create_note`, `update_note`, `delete_note`, `get_note`, `get_notes` | lib.rs:L3727-3828 | 102 | Medium     | 8 | ✅ |
| B2 | CRUD projects and tasks  | endpoint | `create_project`, `create_task`, `get_projects`, `get_tasks`, `update_task_status` | lib.rs:L4330-4408 | 79  | Medium     | 5 | ✅ |
| B3 | CRUD learning items      | endpoint | `create_learning_item`, `get_learning_items`                     | lib.rs:L4408-4457 | 50  | Simple     | — | — |

## Frontend (main.js)

| ID | Function Group                    | Type | Key Functions                                        | Location           | LOC | Complexity | Subfuncs | Phase 3 |
|----|----------------------------------|------|------------------------------------------------------|--------------------|-----|------------|----------|---------|
| F1 | Notes (list + editor)            | view | `loadNotes`, `renderNotesList`, `openNote`           | main.js:L3162-3287 | 126 | Complex    | 7 | ✅ |
| F2 | Projects and tasks (UI)          | view | `loadWork`, `renderWork`                             | main.js:L3815-3889 | 75  | Medium     | 6 | ✅ |
| F3 | Learning (courses/skills)        | view | `loadDevelopment`, `renderDevelopment`, `showAddLearningModal` | main.js:L3890-3975 | 86  | Medium     | 5 | ✅ |

## Summary

- **Total functions**: 6 (3 backend + 3 frontend)
- **Simple**: 1 (B3)
- **Medium**: 4 (B1, B2, F2, F3)
- **Complex**: 1 (F1)
- **Total subfunctions**: 31
