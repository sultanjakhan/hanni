# Module 11: Media — Function Index

## Backend (lib.rs)

| ID | Function Group         | Type     | Key Functions                                                            | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|----|------------------------|----------|--------------------------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B1 | CRUD hobbies           | endpoint | `create_hobby`, `get_hobbies`, `log_hobby_entry`, `get_hobby_entries`    | lib.rs:L4457-4521 | 65  | Simple     | — | — |
| B2 | CRUD media items       | endpoint | `add_media_item`, `update_media_item`, `delete_media_item`, `get_media_items`, `hide_media_item`, `unhide_media_item` | lib.rs:L4800-4910 | 111 | Medium     | 7 | ✅ |
| B3 | User lists             | endpoint | `create_user_list`, `get_user_lists`, `add_list_item`, `remove_list_item`, `get_list_items` | lib.rs:L4911-4983 | 73  | Medium     | 5 | ✅ |
| B4 | Media statistics       | endpoint | `get_media_stats`                                                        | lib.rs:L4984-5012 | 29  | Simple     | — | — |

## Frontend (main.js)

| ID | Function Group               | Type | Key Functions                             | Location           | LOC | Complexity | Subfuncs | Phase 3 |
|----|------------------------------|------|-------------------------------------------|--------------------|-----|------------|----------|---------|
| F1 | Hobbies overview             | view | `loadHobbies`, `loadHobbiesOverview`      | main.js:L4598-4634 | 37  | Simple     | — | — |
| F2 | Media list by type           | view | `loadMediaList`                           | main.js:L4635-4680 | 46  | Medium     | 4 | ✅ |
| F3 | Add and detail media modals  | view | `showAddMediaModal`, `showMediaDetail`    | main.js:L4681-4773 | 93  | Medium     | 6 | ✅ |

## Summary

- **Total functions**: 7 (4 backend + 3 frontend)
- **Simple**: 3 (B1, B4, F1)
- **Medium**: 4 (B2, B3, F2, F3)
- **Complex**: 0
- **Total subfunctions**: 22
