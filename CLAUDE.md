# Hanni — Claude Code Rules

**CRITICAL: Read memory files FIRST.** Before starting any work, read `~/.claude/projects/-Users-sultanbekjakhanov-hanni/memory/MEMORY.md` and load relevant memories. They contain hard-won lessons — ignoring them wastes the user's time.

**CRITICAL: Communicate in Russian.** The user speaks Russian. All questions, plans, and explanations — in Russian.

## Project Manifest (`projects.yaml`)

`~/hanni/projects.yaml` — карта артефактов проектов: какие файлы, таблицы, миграции и memory-заметки относятся к каждому из 17 табов (плюс cross-cutting `share`, `sync`, `android` и общая инфра `_shared`). **Это single source of truth для структуры проекта.**

### Lazy-load protocol
- **Триггер**: пользователь упомянул проект по имени (Food, Sports, calendar и т.д.) или тегу `/<проект>` → **сначала Read `projects.yaml`**, затем точечный Read нужных артефактов из секции. Не грeпай репу вслепую
- **Не загружай манифест автоматом каждую сессию** — только когда нужен (экономия контекста)
- **Правка `_shared` файла** (state.js, main.js, base.css, automation API, build) → предупреди: «эта правка затронет проекты X, Y, Z — проверить?»

### Maintenance (договор)
- При **создании** нового JS/CSS/Rust файла или таблицы — обнови `projects.yaml` в **той же** задаче, не «потом»
- При **удалении** — удали запись из манифеста тем же коммитом
- Если файл cross-cutting и неочевидно, к какому проекту относится — **спроси**, не угадывай

### Статус
- `_meta.bootstrap_status: complete` (с 2026-05-25) — 100% покрытие 424 tracked файлов
- Если найдёшь файл, которого нет в манифесте — это баг манифеста (или ты в `docs/`/`.claude/` — они вне scope), скажи пользователю

## Workflow: Feature Requests

**ОБЯЗАТЕЛЬНО** следуй этим шагам. НЕ НАЧИНАЙ писать код без уточнения. Это БЛОКИРУЮЩЕЕ требование.

1. **Clarify** — ОБЯЗАТЕЛЬНО задай вопросы (см. чеклист ниже). **НЕ НАЧИНАЙ код пока не получишь ответы**
2. **Research** — look up docs/APIs if unsure how something works (context7, Nia, web search)
3. **Plan** — read affected files, then present a short plan (which files change, what changes)
4. **Confirm** — wait for OK before coding
5. **Implement** — write code, following all rules below
6. **Verify** — run stack-specific checks (see the CLAUDE.md in the folder you edited)

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
- **UI separated from logic** — always
- **Известные pre-existing нарушения** (commands_data.rs, db.rs, tab-data.js и др.) трекаются в `hanni-tasks#20` — это долг, НЕ флагай их как баг. Новые нарушения не вводить.

### Style
- **Only change what was requested.** No drive-by refactors, no "improvements" to adjacent code
- **No unnecessary abstractions** — 3 similar lines > premature helper
- **Comments in English**, user communication in Russian
- **No hardcoded data lists** — product categories, ingredient lists, cuisines etc. must come from DB, never from JS/Rust constants

Stack-specific style rules (CSS variables, `S` state, Settings UI layout, Rust function size) live in the folder CLAUDE.md files.

### Pre-commit Checklist
- [ ] No `console.log` / `dbg!()` / `print()` left behind
- [ ] No files over 230 lines introduced

Stack-specific pre-commit checks (`cargo check`, `node --check`, command registration) live in the folder CLAUDE.md files.

## Tools Strategy

### Prefer native tools
Use Read/Edit/Grep/Glob/Bash for all code tasks. They are faster and more reliable.

**When to use what (priority):** native (Read/Edit/Grep) → `context7`/Nia для незнакомого API/сигнатур → web search только для не-библиотечного/свежего (то, чего нет в docs) → `hanni-mcp` для запросов к SQLite (facts/events/run_sql) → `serena` find_symbol для навигации по Rust-символам. claude.ai-хостед Notion/Gmail/Calendar/Drive к кодингу Hanni **нерелевантны** — не трогай.

### MCP / tools
- **playwright** — only for external web pages, never for Hanni itself
- **context7 / Nia** — library/API documentation lookup before coding
- **css/a11y** — only when explicitly requested for audit
(Для скриншотов и DOM-ops смотри секцию «Hanni app interaction» ниже.)

### Skills strategy
When a task is non-trivial, consider which skill fits during the clarification step:

| Skill | When to use |
|-------|-------------|
| `architect` | Restructuring modules, splitting files, managing boundaries |
| `design` | UI/UX improvements, visual consistency, animations |
| `debug` | Systematic bug investigation |
| `qa` | Testing edge cases, verifying error handling |
| `add-tab` / `add-command` / `add-action` | Scaffolding new features |
| `refactor` | Safe renaming, extraction, with dependency tracking |
| `perf` | Profiling, optimization |
| `commit` / `release` / `changelog` | Git workflow |
| `improve` | Code review from a specific perspective |
| `security` | Hardening, vulnerability audit, input validation |
| `deps` | Audit/update Cargo, pip, npm dependencies |
| `db-migrate` / `docs` | Schema migrations, architecture docs |

For architectural decisions or large features, use `architect` or the Plan agent first.

**Overlap & combos:** `code-review` ловит баги в диффе; `improve` — экспертный ревью-совет (до/после кода); `architect` — стратегический рестракт + границы модулей, `refactor` — тактическое безопасное извлечение/переименование. Частые связки: новый таб → `add-tab` + `design`; «тормозит» → `perf` (профиль) → `architect`/`refactor` если причина структурная; крупный сплит файла → `architect` (план) → `refactor` (правки). Не уверен какой скилл — спроси, не угадывай.

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
- **Graceful quit**: `osascript -e 'tell application "Hanni" to quit'`. Re-open — попросить пользователя (никогда не `open -a Hanni` / activate, см. `feedback_safety.md`). Never kill/pkill/killall

## Verification

**Юнит-тестов в проекте нет** (ни `cargo test`, ни JS-сьюта). Что считается проверкой:
- **Rust**: `UPDATER_GITHUB_TOKEN=dummy cargo check` — компиляция + регистрация команд. PostToolUse-хук гоняет это автоматически после правки `.rs`.
- **JS**: `node --check <file>` — синтаксис ОДНОГО файла. НЕ резолвит ES-импорты → при переносе/переключении импортов отдельно проверь Grep/Glob, что все `import from` существуют (битый импорт = белый экран, см. `feedback_split_files.md`).
- **Python (hanni-mcp)**: `python3 -m py_compile server.py`.
- **Поведение**: smoke через `/auto/eval` :8236 (dev) — выполни команду / проверь стейт. UI — `screenshot.sh` + показать юзеру.
- **Свежая БД** (после правок миграций в `db.rs`): `cargo check` НЕ проверяет порядок миграций — запусти dev и убедись, что они прошли.

## Safety & Rollback

- **NEVER use `kill`, `pkill`, `killall` or any process-killing commands.** This can kill system processes and crash/reboot macOS. To stop Hanni: `osascript -e 'tell application "Hanni" to quit'`. To stop cargo dev: ask the user to Ctrl+C it. No exceptions.
- **Before risky changes** (multi-file refactor, migration, delete) — `git stash` or commit current state first
- **If something breaks** — `git diff` to see what changed, revert the broken part, don't pile fixes on top of fixes
- **Never force-push to main**
- **Test incrementally** — change one thing, verify, then next. Not 5 changes at once

## Conflict Resolution

If a request conflicts with existing architecture — **ask, don't decide**. Example: "You're asking for a global var, but we use the S pattern — should I use S?"
