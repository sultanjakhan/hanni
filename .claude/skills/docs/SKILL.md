---
name: docs
description: Update architecture documentation based on current code state. Scans code and syncs docs/architecture/.
allowed-tools: Read, Write, Edit, Grep, Glob, Task, Bash
argument-hint: [module]
user-invocable: true
---

# Update Architecture Documentation

Scan the codebase and update/create architecture documentation.

## Input

Optional module to focus on: `chat`, `voice`, `memory`, `actions`, `ui`, `db`, `core`, or empty for all.

## Documentation Structure

```
docs/architecture/
├── README.md              — High-level architecture overview
├── chat/
│   └── functions.md       — Chat module functions & flows
├── voice/
│   └── functions.md       — Voice pipeline functions
├── memory/
│   └── functions.md       — Memory system functions
├── ml_devops/
│   └── functions.md       — ML/DevOps functions
└── CHANGELOG-vX.Y.Z.md   — Version changelogs
```

## Steps

1. **Read existing docs** for the target module(s)

2. **Scan current code** — use Grep/Read to find:
   - All `#[tauri::command]` functions in `lib.rs` (with signatures)
   - All major JS functions in `main.js`
   - All DB tables and their schemas
   - All action types
   - All event types (Tauri events)
   - Voice server endpoints

3. **Compare code vs docs** — find:
   - Functions in code but not in docs (new)
   - Functions in docs but not in code (removed)
   - Functions with changed signatures
   - New features/modules not documented

4. **Update docs:**
   - Add missing functions with signatures and descriptions
   - Remove references to deleted functions
   - Update changed signatures
   - Add new sections for new modules
   - Keep format consistent with existing docs

5. **Report** what was updated/added/removed

## Documentation Format per Function

```markdown
### `function_name(param1, param2) -> ReturnType`
Brief description of what it does.
- **Params**: param1 (Type) — description
- **Returns**: description
- **Side effects**: DB writes, events emitted, etc.
```

## Rules

- Respond in Russian
- Don't document internal/private helpers unless they're architecturally significant
- Group functions by domain (chat, voice, memory, etc.)
- Include data flow descriptions where relevant
- Note any important invariants or constraints
