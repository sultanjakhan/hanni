---
name: refactor
description: Safe refactoring with dependency tracking — rename, extract, move, simplify code with confidence.
allowed-tools: Read, Write, Edit, Grep, Glob, Task, Bash
argument-hint: <operation> [target]
user-invocable: true
---

# Safe Refactoring

Perform refactoring operations with full dependency tracking to avoid breaking changes.

## Operations

| Operation | What it does |
|-----------|-------------|
| `extract` | Extract a function/method from inline code |
| `rename` | Rename a function/variable/constant across the codebase |
| `move` | Move a function to a different location (with reference updates) |
| `simplify` | Simplify complex logic without changing behavior |
| `dedupe` | Find and eliminate duplicated code |
| `inline` | Inline a function/variable that's used only once |
| `cleanup` | Remove dead code, unused variables, unreachable branches |

Second argument is the target (function name, file, or area).

## Steps (for any operation)

1. **Locate the target** — find the code to refactor using Grep/Read

2. **Map all references** — find every place that uses/calls/imports the target:
   ```
   Grep for function name, variable name, etc.
   Check both lib.rs and main.js (Tauri invoke calls cross the boundary)
   Check HTML for DOM IDs/classes referenced in JS
   Check CSS for selectors referenced in HTML/JS
   ```

3. **Understand the context** — read surrounding code to understand:
   - What the code does
   - Why it's structured this way
   - What invariants must be preserved

4. **Plan the refactoring** — describe exactly what will change:
   - What code moves where
   - What gets renamed to what
   - What references need updating
   - What could break

5. **Get user approval** — present the plan before executing

6. **Execute** — make all changes atomically (all or nothing)

7. **Verify** — check that all references are updated:
   - Grep for old names (should find 0 results)
   - Check for compilation issues (Rust: `cargo check`)
   - Check for obvious JS issues

## Safety Rules

### For `rename`:
- Search for ALL occurrences (string matching, not just symbol matching)
- Consider: Tauri invoke names (JS string must match Rust function name)
- Consider: CSS class/ID names referenced in JS
- Consider: localStorage keys
- Consider: SQL table/column names in strings

### For `extract`:
- Identify all variables used in the extracted code
- Determine which become parameters vs. which stay in scope
- Maintain the same error handling pattern
- Place the new function near related functions

### For `move`:
- Update ALL callers
- Update imports if applicable
- Maintain the same visibility/access level

### For `simplify`:
- Never change behavior, only structure
- Test mentally with edge cases
- If unsure, ask the user

### For `dedupe`:
- Only deduplicate if the code is truly identical in purpose (not just structure)
- Extract to a well-named shared function
- Consider if the duplication was intentional (different evolution paths)

## Cross-boundary Awareness

Hanni has a Rust↔JS boundary. Refactoring must consider:
- `#[tauri::command]` function names in Rust = `invoke('name')` strings in JS
- Tauri event names emitted in Rust = listened in JS
- Database column names in Rust SQL = field names in JS
- CSS class names in styles.css = used in JS DOM manipulation

## Rules

- Respond in Russian
- ALWAYS show the plan before executing
- ALWAYS search for all references before changing anything
- If a refactoring is risky, propose smaller safe steps
- Prefer mechanical (safe) refactoring over creative (risky) refactoring
- After refactoring, verify with `cargo check` if Rust code was changed
- Don't refactor code that's about to be deleted or rewritten
