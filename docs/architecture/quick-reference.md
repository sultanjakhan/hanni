# Architecture Quick Reference

Полная версия ориентира, вынесенная из `CLAUDE.md` (там оставлен сжатый стаб). Read-once для онбординга — не нужно держать в каждой сессии.

## Stack
- **Tauri** (Rust backend) + **vanilla JS** (ES-модули, без фреймворка) + **SQLite**.

## Frontend state
- Общий объект **`S`** в `desktop/src/js/state.js` — только персистентные данные, без temp-переменных.
- Реестр **`tabLoaders`** — для кросс-модульных вызовов между табами.
- **db-view** (`desktop/src/js/db-view/`, ~34 модуля) — общий движок таблиц/канбана/галереи/календаря для 12 data-табов; грузится лениво вместе с `tab-data.js` при первом входе в data-таб.

## Rust pattern
- `types.rs` шарится через `use types::*`.
- Команды — `pub` для `generate_handler![]`.
- Новые Tauri-команды регистрировать в `invoke_handler`.
- Одна `pub fn` = одна задача, ≤80 строк.

## LLM
- Локальный **MLX-сервер** на `127.0.0.1:8234` (Qwen3.5-35B-A3B) — дефолт.
- Переопределение: app_settings `llm_server_url` / `llm_model` (Чат → Настройки → Инструменты → «LLM сервер») — любой OpenAI-совместимый эндпоинт, напр. GPU-машина по Tailscale. Применяется без перезапуска (`types::llm_chat_url()`).
- Стриминг — **SSE-события**: `chat-token`, `chat-done`, `chat-reasoning`, `chat-reasoning-done`.
- **Сейчас MLX офлайн**: табы Chat и Focus закомментированы в `TAB_REGISTRY` (`state.js`), дефолтный таб — Calendar. Вернуть при включении MLX.

## HTTP API (automation)
- `127.0.0.1:8235` (prod), `127.0.0.1:8236` (dev, `cfg!(debug_assertions)`).
- Endpoint `POST /auto/eval` — DOM-eval, используется `screenshot.sh` и Claude.
- Токен: `~/Library/Application Support/Hanni/api_token.txt`.

## MCP / сервисы
- **MCP hanni**: Python-сервер `desktop/hanni-mcp/server.py` — CRUD по той же SQLite (facts, events, SQL).
- **Voice**: `voice_server.py` — фоновый LaunchAgent для речи.

## Build / dev
- Dev-проверка: `UPDATER_GITHUB_TOKEN=dummy cargo check`.
- Dev-инстанс: `cargo tauri dev` (порт 8236). JS/CSS hot-reload — `desktop/tools/auto-reload.mjs` (watcher → reload через `/auto/eval`).
- Graceful quit: `osascript -e 'tell application "Hanni" to quit'`. Re-open — попросить пользователя (никогда `open -a Hanni` / activate). Never kill/pkill/killall.

См. также: `docs/architecture/WIKI.md`, `docs/MODULE_MAP.md`, и per-module папки (`calendar/`, `chat/`, `core/`, `voice/` …).
