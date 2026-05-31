# Hanni — Claude Code Rules

> Дополняет глобальный `~/.claude/CLAUDE.md` (workflow, clarify-чеклист, coding-style, git, reversibility). Здесь — Hanni-специфика и предохранители.

**CRITICAL: сначала прочитай память** — `~/.claude/projects/-Users-sultanbekjakhanov-hanni/memory/MEMORY.md` + релевантные заметки. Там выстраданные уроки.
**CRITICAL: общение по-русски** — вопросы, планы, объяснения. Комментарии в коде — английский.

## Safety (Hanni) — императивы; обоснование в `memory/feedback_safety.md`
- **Никогда** `kill`/`pkill`/`killall` — может ребутнуть macOS. Стоп Hanni: `osascript -e 'tell application "Hanni" to quit'`. Стоп `cargo tauri dev` — попросить юзера Ctrl+C.
- **Не трогать prod Hanni.** Тестировать только в `cargo tauri dev`.
- **Не активировать окно** (`open -a Hanni`, `osascript activate`) — прерывает работу юзера. Нужен фокус для скрина → попроси сделать вручную.
- **Порты: 8236 dev / 8235 prod.** Максимум 1 dev + 1 prod. Dev уже отвечает на `:8236` — **не рестартить**: JS/CSS подхватит `auto-reload`, для Rust спроси «нужен рестарт?». Детали: `memory/feedback_dev_no_restart.md`.

## Clarify (дельта к global-чеклисту)
- **ГДЕ** = какой таб → sub-view → элемент. **Данные** = SQLite-таблица / localStorage / объект `S`. Нужна ли миграция?
- Не уверен хотя бы в одном пункте — спроси, не додумывай. (Полный чеклист — в global.)

## Project Manifest (`projects.yaml`)
SSOT структуры проекта: 17 табов + cross-cutting `share`/`sync`/`android` + общая инфра `_shared`. **Lazy-load:**
- Юзер упомянул проект (Food, calendar, `/share`…) → **сначала Read `projects.yaml`**, затем точечно нужные артефакты. Не грепай репу вслепую. Не грузи манифест каждую сессию.
- Правка `_shared` (state.js, main.js, base.css, automation API, build) → предупреди: «затронет проекты X, Y, Z — проверить?».
- Создал/удалил JS/CSS/Rust-файл или таблицу → обнови манифест **в той же** задаче. `docs/`, `.claude/` — вне scope.

## Coding (Hanni-дельта; общий стиль — в global)
- **Файл/компонент ≤230 строк** (Rust `pub fn` ≤80) — иначе сплит. UI отделён от логики.
- **Известные pre-existing нарушения** (commands_data.rs, db.rs, tab-data.js и др.) — долг в `hanni-tasks#20`, **НЕ флагай как баг**; новых не вводить.
- **Никаких хардкод-списков данных** (категории, ингредиенты, кухни) — только из БД, не из JS/Rust констант.
- Синтаксис JS/Python и `cargo check` — авто (PostToolUse hook). За собой чистить `console.log`/`dbg!()`/`print()`.
- Стек-правила (CSS-переменные, `S`-стейт, Settings layout, Python venv) — в folder-CLAUDE.md.

## Тулзы и выбор агента/скилла
**Приоритет тулзов:** native (Read/Edit/Grep) → `context7`/Nia для незнакомого API/сигнатур → web search только для не-библиотечного/свежего (то, чего нет в docs) → `hanni-mcp` для SQLite (facts/events/run_sql) → `serena` find_symbol для Rust-символов. **playwright** — только внешние страницы, не Hanni. claude.ai Notion/Gmail/Calendar/Drive к кодингу Hanni **нерелевантны** — не трогай.

**Скиллы:** полная карта (тип задачи → built-in agent / субагент / скилл + decision-tree) — **`docs/AGENT_ROUTING.md`**. Связки: новый таб → `add-tab`+`design`; «тормозит» → `perf` (профиль) → `architect`/`refactor` если причина структурная; крупный сплит файла → `architect` (план) → `refactor` (правки). `improve` = ревью-совет (read-only); role-скиллы (`architect`/`security`/`perf`) — правят. Не уверен какой — спроси.

## Hanni app interaction
- **Скриншот**: `desktop/tools/screenshot.sh /tmp/out.png [port]` → Read. Silent (`-x`), работает свёрнутым (html2canvas).
- **DOM/eval**: `POST 127.0.0.1:<port>/auto/eval` c `{"script":"…"}` (порты см. Safety). Клик/тайп/навигация — через eval (`element.click()`, `MouseEvent`, `KeyboardEvent`). Сложные скрипты с кавычками — `python3 urllib`. Токен: `cat ~/Library/Application\ Support/Hanni/api_token.txt`.
- **Никогда** MCP screenshot / tauri-automation (виснут на macOS).
- После UI-правок — скриншот юзеру на сверку.

## Verification (юнит-тестов в проекте нет)
- **Rust**: `UPDATER_GITHUB_TOKEN=dummy cargo check` — компиляция + регистрация команд (PostToolUse-хук гоняет авто после правки `.rs`).
- **JS**: `node --check` — синтаксис ОДНОГО файла. НЕ резолвит ES-импорты → при переносе/переключении импортов проверь Grep/Glob, что все `import from` существуют (битый импорт = белый экран, `memory/feedback_split_files.md`).
- **Python (hanni-mcp)**: синтаксис проверяет PostToolUse-хук.
- **Поведение**: smoke через `/auto/eval` :8236 (dev) — выполни команду / проверь стейт. UI — `screenshot.sh` + показать юзеру.
- **Свежая БД** (после правок миграций в `db.rs`): `cargo check` НЕ проверяет порядок миграций — запусти dev и убедись, что прошли.

## Архитектура (кратко; полное — `docs/architecture/quick-reference.md`)
Tauri (Rust) + vanilla JS (ES-модули) + SQLite. Стейт — объект `S` в `state.js` + реестр `tabLoaders`. Rust: `types.rs` через `use types::*`, команды `pub` для `generate_handler![]`, регистрация в `invoke_handler`. SSE-события: `chat-token` / `chat-done` / `chat-reasoning` / `chat-reasoning-done`. LLM: локальный MLX `127.0.0.1:8234`. MCP hanni: `hanni-mcp/server.py` (CRUD по SQLite). Build/dev-check: `UPDATER_GITHUB_TOKEN=dummy cargo check`.
