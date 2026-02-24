---
name: add-tab
description: Scaffold a new tab in Hanni — HTML structure, CSS styles, JS logic, tab bar registration, and sub-sidebar support.
allowed-tools: Read, Edit, Grep, Glob, Task
argument-hint: <tab_name> [sub-tabs...]
user-invocable: true
---

# Scaffold New Tab

Create a new tab with full integration into Hanni's tab system.

## Input

- First argument: tab name (e.g., `fitness`, `travel`)
- Optional: list of sub-tabs (e.g., `workouts plans stats`)

## Steps

1. **Read current tab patterns** — study an existing tab implementation:
   - Find tab registration in `main.js` (tab bar, switching logic)
   - Find HTML structure pattern for tab content
   - Find sub-sidebar pattern if sub-tabs are needed
   - Check `styles.css` for tab-specific styles

2. **Add tab to tab bar** in `main.js`:
   - Add tab definition (id, label, icon)
   - Add to the tab switching logic

3. **Create tab HTML structure** in `index.html` or dynamically in `main.js`:
   - Main content container with the tab's ID
   - Sub-sidebar if sub-tabs provided
   - Follow existing DOM structure patterns

4. **Add tab CSS** in `desktop/src/styles.css`:
   - Tab content styles
   - Sub-tab styles if needed
   - Follow Notion Dark theme variables

5. **Add JS logic** in `desktop/src/main.js`:
   - Tab initialization function
   - Sub-tab switching if applicable
   - Data loading/rendering functions
   - Event listeners

6. **Add Rust backend if needed** — if the tab needs data persistence:
   - Suggest DB tables via `/db-migrate`
   - Create Tauri commands via `/add-command`

7. **Report** the full structure: what was created and where

## Rules

- Respond in Russian
- Follow existing tab patterns exactly (don't invent new patterns)
- Use CSS variables from `:root` in styles.css
- Sub-sidebar follows the existing sub-sidebar component pattern
- Tab should have proper empty state
- Keyboard shortcuts: suggest Cmd+N if slot available
