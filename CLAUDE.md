# Hanni — Claude Code Rules

## Workflow: Feature Requests

When the user asks for a feature or change:

1. **Clarify** — ask targeted questions to understand exactly what and where:
   - What behavior is expected?
   - Which tab/view/component is affected?
   - Any edge cases to handle?
   - Only proceed when confident there are no ambiguities
2. **Plan** — read affected files, then present a short plan (which files change, what changes)
3. **Confirm** — wait for OK before coding
4. **Implement** — write code, following all rules below
5. **Verify** — `UPDATER_GITHUB_TOKEN=dummy cargo check` for Rust changes

Skip clarification only for trivial/obvious tasks (typo fix, one-line change).

## Coding Rules

### Size Limits
- **No file/component longer than 150 lines.** If it exceeds — split automatically
- **Rust: one `pub fn` = one task**, max 80 lines
- **UI separated from logic** — always

### Style
- **Only change what was requested.** No drive-by refactors, no "improvements" to adjacent code
- **No unnecessary abstractions** — 3 similar lines > premature helper
- **Comments in English**, user communication in Russian
- **CSS: always use variables from `base.css`** — never hardcode colors, spacing, radii, shadows
- **JS state: only persistent data in `S` object** — no temp vars
- **Settings UI: `settings-row` / `settings-label` / toggle layout** — never raw dropdowns

### Pre-commit Checklist
- [ ] `UPDATER_GITHUB_TOKEN=dummy cargo check` passes
- [ ] **`node --check` on every modified JS file** — catch SyntaxError before runtime
- [ ] No hardcoded colors/sizes (use CSS vars)
- [ ] New Tauri commands registered in `invoke_handler`
- [ ] No `console.log` / `dbg!()` left behind
- [ ] No files over 150 lines introduced
- [ ] No duplicate `const`/`let` declarations in the same scope

## Tools Strategy

### Prefer native tools
Use Read/Edit/Grep/Glob/Bash for all code tasks. They are faster and more reliable.

### MCP usage
- **tauri-automation** — ALWAYS use for testing the Hanni app (NOT playwright)
- **playwright** — only for external web pages, never for Hanni itself
- **context7** — library documentation lookup
- **screenshot** — visual verification
- **css/a11y** — only when explicitly requested for audit

### Skills strategy
When a task is non-trivial, consider which skill fits during the clarification step:

| Skill | When to use |
|-------|-------------|
| `architect` | Restructuring modules, splitting files, managing boundaries |
| `decompose` | Understanding full project structure before big changes |
| `design` | UI/UX improvements, visual consistency, animations |
| `debug` / `debug-llm` | Systematic bug investigation |
| `qa` | Testing edge cases, verifying error handling |
| `add-tab` / `add-command` / `add-action` | Scaffolding new features |
| `refactor` | Safe renaming, extraction, with dependency tracking |
| `perf` | Profiling, optimization |
| `commit` / `release` / `changelog` | Git workflow |
| `improve` | Code review from a specific perspective |

For architectural decisions or large features, use `architect` or the Plan agent first.

## Architecture Quick Reference

- **Stack**: Tauri (Rust) + vanilla JS (ES modules) + SQLite
- **State**: shared `S` object in `state.js`, `tabLoaders` registry for cross-module calls
- **Rust pattern**: `types.rs` shared via `use types::*`, commands `pub` for `generate_handler![]`
- **LLM**: local MLX server at `127.0.0.1:8234` (Qwen3.5-35B-A3B)
- **Streaming**: SSE events — `chat-token`, `chat-done`, `chat-reasoning`, `chat-reasoning-done`
- **Build**: `UPDATER_GITHUB_TOKEN=dummy cargo check` for dev

## Conflict Resolution

If a request conflicts with existing architecture — **ask, don't decide**. Example: "You're asking for a global var, but we use the S pattern — should I use S?"
