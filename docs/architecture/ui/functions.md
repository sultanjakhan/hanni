# Module 14: UI — Function Index

## Frontend (main.js)

| ID  | Function Group              | Type    | Key Functions                                       | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|---------|-----------------------------------------------------|-------------------|-----|------------|----------|---------|
| F1  | App initialization          | utility | Globals, DOM elements, `APP_VERSION`                | main.js:L1-40     | 40  | Simple     | —        | —       |
| F2  | Icons and tab registry      | data    | `TAB_ICONS`, `TAB_REGISTRY`, `TAB_DESCRIPTIONS`    | main.js:L59-115   | 57  | Simple     | —        | —       |
| F3  | Tab state management        | utility | `openTabs`, `activeTab`, `saveTabs`, tab init       | main.js:L132-162  | 31  | Simple     | —        | —       |
| F4  | Tab bar rendering           | view    | `renderTabBar()`                                    | main.js:L450-481  | 32  | Medium     | 3        | ✅       |
| F5  | Sub-sidebar rendering       | view    | `renderSubSidebar()`                                | main.js:L483-530  | 48  | Medium     | 4        | ✅       |
| F6  | Tab navigation              | handler | `openTab`, `closeTab`, `switchTab`, `activateView`  | main.js:L602-643  | 42  | Simple     | —        | —       |
| F7  | Sub-tab content loading     | view    | `loadSubTabContent()`                               | main.js:L644-700  | 57  | Complex    | 4        | ✅       |
| F8  | Skeleton loaders            | utility | `escapeHtml`, `skeletonSettings`, `skeletonGrid`, `skeletonList`, `skeletonPage` | main.js:L392-428 | 37 | Simple | — | — |
| F9  | Chat scroll                 | utility | `scrollDown()`                                      | main.js:L964-976  | 13  | Simple     | —        | —       |
| F10 | Update handler              | event   | `listen 'update-available'`                         | main.js:L156-162  | 7   | Simple     | —        | —       |

## Styling (styles.css)

| ID  | Function Group              | Type  | Scope                                              | Location            | LOC  | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|-------|----------------------------------------------------|---------------------|------|------------|----------|---------|
| F11 | CSS theme and variables     | style | `:root` variables, Notion Dark palette              | styles.css:L7-65    | 59   | Simple     | —        | n/a     |
| F12 | Layout components           | style | `body`, sidebar, content area, tab bar              | styles.css:L79-743  | 665  | Complex    | —        | n/a     |
| F13 | Chat styles                 | style | Chat container, messages, input area                | styles.css:L416-743 | 328  | Complex    | —        | n/a     |
| F14 | Per-tab styles              | style | Focus, notes, calendar, dashboard, media, etc.      | styles.css:L1154-2638 | 1485 | Complex  | —        | n/a     |
| F15 | Data components             | style | Modals, database views, inline edit, properties     | styles.css:L2638-3145 | 508 | Complex   | —        | n/a     |
| F16 | Call mode (CSS)             | style | Call overlay, phases, animations                    | styles.css:L3145-3563 | 419 | Complex   | —        | n/a     |

## HTML (index.html)

| ID  | Function Group              | Type      | Scope          | Location        | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|-----------|----------------|-----------------|-----|------------|----------|---------|
| F17 | HTML structure              | structure | Full page shell | index.html:L1-116 | 116 | Simple    | —        | n/a     |

## Summary

- **Total functions**: 17 (10 JS + 6 CSS + 1 HTML)
- **Simple**: 8 (F1, F2, F3, F6, F8, F9, F10, F11, F17)
- **Medium**: 2 (F4, F5)
- **Complex**: 6 (F7, F12, F13, F14, F15, F16)
- **Data**: 1 (F2)
- **Total subfunctions**: 11 (JS only; CSS/HTML — n/a)
