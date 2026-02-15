# Core Module

Инициализация приложения, схема БД, константы, типы, миграции, entry point.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~1800 |
| Backend functions | 5 |
| Frontend functions | 0 |
| Complexity | Simple: 2, Medium: 1, Complex: 2 |

## Files

| File | Lines | Description |
|------|-------|-------------|
| `lib.rs` | L1-180 | Системный промпт, константы, типы |
| `lib.rs` | L274-879 | Миграция данных, инициализация БД (40+ таблиц) |
| `lib.rs` | L7358-8201 | Обновление, entry point `run()` |
| `main.rs` | — | Точка входа Tauri |
| `build.rs` | — | Build script |
| `Cargo.toml` | — | Зависимости Rust |
| `tauri.conf.json` | — | Конфигурация Tauri |
| `capabilities/default.json` | — | Разрешения Tauri v2 |
| `package.json` | — | Зависимости фронтенда |
| `Entitlements.plist` | — | macOS entitlements |
| `Info.plist` | — | macOS app metadata |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | rusqlite, serde, chrono, dirs, tauri |
| **Used by** | ALL modules (provides DB, types, app bootstrap) |
