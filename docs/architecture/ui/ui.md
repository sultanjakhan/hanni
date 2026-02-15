# Module 14: UI

## Description

Foundation layer: tab system, navigation, styling (Notion Dark theme), HTML structure, skeleton loaders, and the call mode overlay. All other modules depend on this for rendering. Includes `styles.css` (the largest single file), `index.html`, and the navigation/initialization code in `main.js`.

## Overview

| Attribute        | Value                                                                                  |
|------------------|----------------------------------------------------------------------------------------|
| Domain           | Tabs, navigation, styling, HTML structure                                              |
| Total LOC        | ~4370                                                                                  |
| Frontend         | `main.js` (L1-40, L59-115, L132-162, L392-428, L450-700, L964-976)                   |
| Styling          | `styles.css` (L1-3563)                                                                |
| HTML             | `index.html` (L1-116)                                                                 |
| Frontend functions | 17                                                                                  |
| Complexity       | Simple: 8, Medium: 2, Complex: 6, Data: 1                                             |

## Files

| File                      | Lines          | Role                             |
|---------------------------|----------------|----------------------------------|
| `desktop/src/main.js`     | L1-40          | Globals, DOM refs, APP_VERSION    |
| `desktop/src/main.js`     | L59-115        | Icons, tab registry, descriptions |
| `desktop/src/main.js`     | L132-162       | Tab state, save/restore, init     |
| `desktop/src/main.js`     | L392-428       | Skeleton loaders, escapeHtml      |
| `desktop/src/main.js`     | L450-700       | Tab bar, sub-sidebar, navigation  |
| `desktop/src/main.js`     | L964-976       | Chat scroll utility               |
| `desktop/src/styles.css`  | L1-3563        | Full application styling           |
| `desktop/src/index.html`  | L1-116         | HTML skeleton                      |

## Dependencies

| Direction | Module      | Relationship                              |
|-----------|-------------|-------------------------------------------|
| Uses      | none        | Foundation layer, no upstream dependencies |
| Used by   | ALL modules | Every module renders through this framework |

## Key Concepts

- **Tab System**: Browser-like tab bar with open/close, Cmd+W/Cmd+T/Cmd+1..9 shortcuts, localStorage persistence.
- **Sub-sidebar**: Per-tab secondary navigation with sub-tabs and goals widget integration.
- **Notion Dark Theme**: All colors defined as CSS custom properties in `:root`. Dark background, muted borders, accent colors.
- **Skeleton Loaders**: Pre-built loading states (grid, list, page, settings) shown during async data fetches.
- **Layout Components**: Sidebar, content area, tab bar, chat container — all CSS Grid/Flexbox based.
- **Call Mode Overlay**: Full-screen overlay with phase animations for voice call mode.
- **HTML Structure**: Minimal `index.html` (116 lines) with container divs — all content injected by JS.
