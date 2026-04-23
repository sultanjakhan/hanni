# Hanni — Claude Code Rules

**CRITICAL: Read memory files FIRST.** Before starting any work, read `~/.claude/projects/-Users-sultanbekjakhanov-hanni/memory/MEMORY.md` and load relevant memories. They contain hard-won lessons — ignoring them wastes the user's time.

**CRITICAL: Communicate in Russian.** The user speaks Russian. All questions, plans, and explanations — in Russian.

## Workflow: Feature Requests

**ОБЯЗАТЕЛЬНО** следуй этим шагам. НЕ НАЧИНАЙ писать код без уточнения. Это БЛОКИРУЮЩЕЕ требование.

1. **Clarify** — ОБЯЗАТЕЛЬНО задай вопросы (см. чеклист ниже). **НЕ НАЧИНАЙ код пока не получишь ответы**
2. **Research** — look up docs/APIs if unsure how something works (context7, Nia, web search)
3. **Plan** — read affected files, then present a short plan (which files change, what changes)
4. **Confirm** — wait for OK before coding
5. **Implement** — write code, following all rules below
6. **Verify** — `UPDATER_GITHUB_TOKEN=dummy cargo check` for Rust changes

Skip clarification only for trivial/obvious tasks (typo fix, one-line change).

### Clarification checklist

**Always ask:**
1. **ЧТО** — опиши результат одним предложением
2. **ГДЕ** — какой таб → какой sub-view → какой элемент
3. **ТРИГГЕР** — клик? hover? загрузка? горячая клавиша? таймер?

**По типу задачи:**

*UI элемент:*
- Есть референс? (скрин, ссылка, "как кнопка X в Y")
- Размер/позиция: внутри чего? слева/справа/снизу от чего?
- Что при hover? Что при клике? Анимация?

*Данные/БД:*
- Откуда данные? (SQLite таблица, API, localStorage, S объект)
- Формат: текст/число/дата/enum?
- Нужна ли миграция (новая колонка/таблица)?

*Поведение/логика:*
- Что если данных нет / пусто?
- Что если ошибка?
- Сохраняется между сессиями или временно?
- Работает офлайн?

*Стиль:*
- Как существующий элемент X или новый стиль?
- Светлая/тёмная тема — оба?

**Правило:** если не уверен хотя бы в 1 пункте — **спроси, не додумывай**.

### Research before coding
- **Незнакомый API/библиотека** — сначала context7 или Nia для документации
- **Tauri/Rust API** — проверь docs.rs или context7 перед использованием
- **CSS свойство** — проверь совместимость через MCP css если сомневаешься
- **Не угадывай сигнатуры функций** — найди реальную документацию

## Coding Rules

### Size Limits
- **No file/component longer than 230 lines.** If it exceeds — split automatically
- **Rust: one `pub fn` = one task**, max 80 lines
- **UI separated from logic** — always

### Style
- **Only change what was requested.** No drive-by refactors, no "improvements" to adjacent code
- **No unnecessary abstractions** — 3 similar lines > premature helper
- **Comments in English**, user communication in Russian
- **CSS: always use variables from `base.css`** — never hardcode colors, spacing, radii, shadows
- **JS state: only persistent data in `S` object** — no temp vars
- **Settings UI: `settings-row` / `settings-label` / toggle layout** — never raw dropdowns
- **No hardcoded data lists** — product categories, ingredient lists, cuisines etc. must come from DB, never from JS/Rust constants

### Pre-commit Checklist
- [ ] `UPDATER_GITHUB_TOKEN=dummy cargo check` passes
- [ ] **`node --check` on every modified JS file** — catch SyntaxError before runtime
- [ ] No hardcoded colors/sizes (use CSS vars)
- [ ] New Tauri commands registered in `invoke_handler`
- [ ] No `console.log` / `dbg!()` left behind
- [ ] No files over 230 lines introduced
- [ ] No duplicate `const`/`let` declarations in the same scope

## Tools Strategy

### Prefer native tools
Use Read/Edit/Grep/Glob/Bash for all code tasks. They are faster and more reliable.

### MCP usage
- **screenshot.sh** — `desktop/tools/screenshot.sh` for Hanni screenshots (NOT MCP screenshot). Pass port as 2nd arg for dev: `screenshot.sh /tmp/out.png 8236`
- **HTTP API** — `127.0.0.1:8235/auto/eval` (prod) or `127.0.0.1:8236/auto/eval` (dev). Use the port of the instance you're testing against; по умолчанию — порт dev-сборки, если она запущена и правки касаются dev.
- **playwright** — only for external web pages, never for Hanni itself
- **context7 / Nia** — library/API documentation lookup before coding
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
| `security` | Hardening, vulnerability audit, input validation |
| `deps` | Audit/update Cargo, pip, npm dependencies |

For architectural decisions or large features, use `architect` or the Plan agent first.

### Hanni app interaction
- **Screenshots**: `desktop/tools/screenshot.sh /tmp/out.png [port]` → Read tool. Silent (`-x`), works minimized (html2canvas). Port 8235 prod / 8236 dev
- **DOM operations**: HTTP API at `127.0.0.1:8235` (prod) or `127.0.0.1:8236` (dev), endpoint `POST /auto/eval` with `{"script": "..."}`. Use python3 urllib for complex scripts with quotes
- **Click/type/navigate**: `element.click()`, `dispatchEvent(new MouseEvent('dblclick'))`, `KeyboardEvent` — all via `/auto/eval`
- **Token**: `cat ~/Library/Application\ Support/Hanni/api_token.txt`
- **NEVER** use MCP screenshot (hangs on macOS) or tauri-automation (broken on macOS)
- **OK**: 1 prod (port 8235) + 1 dev (port 8236) одновременно. Больше инстансов — запрещено (конфликт портов, overheat)
- **NEVER** activate/focus Hanni window (`osascript activate`, `open -a Hanni`, etc.) — it interrupts the user's workflow
- **NEVER** touch production Hanni — only use `cargo tauri dev` for testing
- If a screenshot requires window focus — **ask the user** to take it manually, do NOT activate
- **After UI changes** — take screenshot and show the user for verification

## Architecture Quick Reference

- **Stack**: Tauri (Rust) + vanilla JS (ES modules) + SQLite
- **State**: shared `S` object in `state.js`, `tabLoaders` registry for cross-module calls
- **Rust pattern**: `types.rs` shared via `use types::*`, commands `pub` for `generate_handler![]`
- **LLM**: local MLX server at `127.0.0.1:8234` (Qwen3.5-35B-A3B)
- **Streaming**: SSE events — `chat-token`, `chat-done`, `chat-reasoning`, `chat-reasoning-done`
- **HTTP API**: `127.0.0.1:8235` (prod), `127.0.0.1:8236` (dev, `cfg!(debug_assertions)`). Automation endpoint for DOM eval, used by screenshot.sh and Claude
- **MCP hanni**: Python MCP server (`hanni-mcp/server.py`) — CRUD for SQLite (facts, events, SQL)
- **Voice**: `voice_server.py` — background LaunchAgent for speech
- **Build**: `UPDATER_GITHUB_TOKEN=dummy cargo check` for dev
- **Graceful quit**: `osascript -e 'tell application "Hanni" to quit'`. Re-open — попросить пользователя (никогда не `open -a Hanni` / activate, см. `feedback_no_activate`). Never kill/pkill/killall

## Safety & Rollback

- **NEVER use `kill`, `pkill`, `killall` or any process-killing commands.** This can kill system processes and crash/reboot macOS. To stop Hanni: `osascript -e 'tell application "Hanni" to quit'`. To stop cargo dev: ask the user to Ctrl+C it. No exceptions.
- **Before risky changes** (multi-file refactor, migration, delete) — `git stash` or commit current state first
- **If something breaks** — `git diff` to see what changed, revert the broken part, don't pile fixes on top of fixes
- **Never force-push to main**
- **Test incrementally** — change one thing, verify, then next. Not 5 changes at once

## Conflict Resolution

If a request conflicts with existing architecture — **ask, don't decide**. Example: "You're asking for a global var, but we use the S pattern — should I use S?"
