# Core Module

Инициализация приложения, схема БД, константы, типы, миграции, entry point, health monitor.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~1900 |
| Backend functions | 7 (5 + health_check + get_model_info) |
| Frontend functions | 0 |
| Complexity | Simple: 2, Medium: 2, Complex: 2 |

## Files

| File | Lines | Description |
|------|-------|-------------|
| `lib.rs` | L1-180 | Системный промпт (SYSTEM_PROMPT, SYSTEM_PROMPT_LITE), константы, типы |
| `lib.rs` | L274-879 | Миграция данных, инициализация БД (45+ таблиц, 16 индексов) |
| `lib.rs` | L8648-8750 | ModelInfo, HealthStatus, health_check, get_model_info |
| `lib.rs` | L7358-8201 | Обновление, entry point `run()` |
| `main.rs` | — | Точка входа Tauri |
| `build.rs` | — | Build script |
| `Cargo.toml` | — | Зависимости Rust |
| `tauri.conf.json` | — | Конфигурация Tauri (v0.18.4) |
| `capabilities/default.json` | — | Разрешения Tauri v2 |
| `package.json` | — | Зависимости фронтенда |
| `Entitlements.plist` | — | macOS entitlements |
| `Info.plist` | — | macOS app metadata |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | rusqlite, serde, chrono, dirs, tauri, reqwest |
| **Used by** | ALL modules (provides DB, types, app bootstrap) |

## Key Features

### System Prompt (v0.18.5+)
- Структурирован по секциям: ИНСТРУМЕНТЫ / СТИЛЬ / КАЧЕСТВО / АНТИ-ГАЛЛЮЦИНАЦИИ
- 5 few-shot примеров (включая "не знаю" и web_search)
- Все промпты на русском (dedup, extraction, profile synthesis)
- C1: Адаптивная инъекция имени пользователя из facts в system prompt
- C5: Адаптивная температура по типу запроса (factual=0.4, creative=0.85, default=0.7/0.6)

### DB Schema
- 45+ таблиц + 16 индексов (events, transactions, food_log, media_items, etc.)
- Integrity check через `PRAGMA integrity_check`

### Health Monitor (C4)
- `health_check` command: MLX status, voice_server status, DB integrity, table/fact/conversation counts, DB size
- `get_model_info` command: model name, server URL, online status

## Improvements Status

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| C1 | Адаптивный system prompt (имя из facts) | S | ✅ |
| C2 | Prompt versioning + A/B тестирование | M | ⬜ |
| C3 | DB миграции как отдельная система | M | ⬜ |
| C4 | Health monitor (MLX, voice_server, DB) | S | ✅ |
| C5 | Adaptive model params (temperature по типу запроса) | S | ✅ |
