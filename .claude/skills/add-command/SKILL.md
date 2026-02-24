---
name: add-command
description: Scaffold a new Tauri command — Rust function in lib.rs, JS invoke wrapper in main.js, and register in invoke_handler.
allowed-tools: Read, Edit, Grep, Glob, Task
argument-hint: <command_name> [description]
user-invocable: true
---

# Scaffold New Tauri Command

Create a new Tauri command end-to-end: Rust backend + JS frontend + registration.

## Input

- First argument: command name in snake_case (e.g., `get_shopping_list`)
- Optional: description of what the command does

## Steps

1. **Understand the command** — what it does, what params it takes, what it returns

2. **Read current patterns** — scan `lib.rs` for a similar existing command to match the style:
   - Find a relevant `#[tauri::command]` function
   - Note the pattern: error handling, state access, return type

3. **Create Rust function** in `desktop/src-tauri/src/lib.rs`:
   - Add `#[tauri::command]` annotated async fn
   - Use existing patterns for state access (`State<'_, Arc<Mutex<...>>>`)
   - Use existing error handling patterns
   - Place it near related commands (group logically)

4. **Register in invoke_handler** — find `.invoke_handler(tauri::generate_handler![...])` and add the new command

5. **Create JS wrapper** in `desktop/src/main.js`:
   ```javascript
   async function commandName(params) {
     return await invoke('command_name', { params });
   }
   ```
   - Place near related functions
   - Use camelCase for JS function name

6. **Report** what was created and where

## Rules

- Respond in Russian
- Match existing code style exactly (indentation, error handling, naming)
- If the command needs a new DB table, suggest using `/db-migrate` first
- Don't add features beyond what was requested
- Use `Arc<Mutex<>>` for async state, `std::sync::Mutex` for HanniDb
