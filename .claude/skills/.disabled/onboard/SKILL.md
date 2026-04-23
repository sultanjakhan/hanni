---
name: onboard
description: Onboard into the Hanni codebase — quickly understand architecture, key files, patterns, and current state for a new session.
allowed-tools: Read, Grep, Glob, Task, Bash
argument-hint: [scope]
user-invocable: true
---

# Hanni Codebase Onboarding

Quickly orient yourself (or a new conversation context) in the Hanni codebase. Builds a mental model of the current state.

## Input

Optional scope to focus on: `full`, `chat`, `voice`, `memory`, `actions`, `ui`, `db`, `recent`, or empty for `full`.

## Steps

### For `full` (default):

1. **Project structure** — scan top-level directories:
   ```bash
   ls -la
   ls desktop/src-tauri/src/
   ls desktop/src/
   ```

2. **Recent activity** — what's been worked on:
   ```bash
   git log --oneline -15
   git diff --stat HEAD~5
   ```

3. **Key files overview:**
   - `lib.rs` — list all `#[tauri::command]` functions (just names)
   - `main.js` — list major sections/functions
   - `styles.css` — check theme variables
   - `voice_server.py` — list endpoints
   - `tauri.conf.json` — check version, permissions

4. **Current state:**
   - Any uncommitted changes?
   - What version are we on?
   - Are there open issues/TODOs in code?

5. **Architecture summary** — produce a concise map:
   ```
   User → [Tauri WebView (main.js)]
            ↕ invoke/events
          [Tauri Backend (lib.rs)]
            ↕ HTTP/SQLite
          [MLX Server :8234] [Voice Server :8235] [SQLite DB]
   ```

### For `recent`:
Focus only on what changed since last tag:
```bash
git log $(git describe --tags --abbrev=0)..HEAD --oneline
git diff $(git describe --tags --abbrev=0)..HEAD --stat
```

### For specific scope (chat/voice/memory/etc.):
1. Find all related functions in `lib.rs` and `main.js`
2. Map the data flow for that module
3. List DB tables involved
4. Note any known issues or TODOs

## Output Format

```markdown
## Hanni — Ориентация [scope]

### Версия: vX.Y.Z
### Последние изменения: [краткий список]

### Архитектура
[диаграмма или описание]

### Ключевые файлы
- `lib.rs` — N команд, основные: ...
- `main.js` — N строк, модули: ...

### Текущее состояние
- Незакоммиченные изменения: да/нет
- Открытые задачи: ...

### Готов к работе ✅
```

## Rules

- Respond in Russian
- Be concise — this is orientation, not deep dive
- Focus on what's needed to START working, not exhaustive documentation
- If there are uncommitted changes, highlight them prominently
- Read MEMORY.md and any Serena memories for prior context
- Don't modify any files — this is read-only
