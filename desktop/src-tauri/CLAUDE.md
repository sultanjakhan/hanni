> Дополняет корневой ~/hanni/CLAUDE.md — здесь только специфика Rust/Tauri backend.

# src-tauri — Rust / Tauri backend

## Size
- Одна `pub fn` = одна задача, макс 80 строк

## Rust pattern
- `types.rs` шарится через `use types::*`
- Команды — `pub` для `generate_handler![]`
- Новые Tauri-команды регистрировать в `invoke_handler`

## Research
- Tauri/Rust API — проверять docs.rs или context7 перед использованием, не угадывать сигнатуры

## Pre-commit
- [ ] `UPDATER_GITHUB_TOKEN=dummy cargo check` проходит
- [ ] Новые команды зарегистрированы в `invoke_handler`
- [ ] Не осталось `dbg!()`
