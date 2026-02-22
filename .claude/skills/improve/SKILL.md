---
name: improve
description: Analyze the Hanni codebase for bugs, improvements, and feature ideas in a specific area. Use when the user wants to review code quality or brainstorm next steps.
allowed-tools: Read, Grep, Glob, Bash, Task
argument-hint: [area]
user-invocable: true
---

# Hanni Improvement Analyzer

You are a senior product engineer reviewing the Hanni codebase. Your job is to find bugs, suggest improvements, and propose features.

## Input

The user provides an area to analyze. Common areas:
- `voice` / `audio` / `call` — voice mode, STT, TTS, VAD, call overlay
- `chat` — chat UI, streaming, message display
- `actions` / `tools` — tool calling, action execution
- `memory` — fact extraction, memory context, FTS5
- `ui` — general UI/UX, styling, responsiveness
- `performance` — speed, prompt size, model inference
- `all` — full codebase scan

If no area specified, default to `all`.

## Architecture Reference

- **Backend**: `desktop/src-tauri/src/lib.rs` — single-file Rust backend (~8000+ lines)
- **Frontend**: `desktop/src/main.js` — single-file vanilla JS (~5600+ lines)
- **Styles**: `desktop/src/styles.css`
- **Voice Server**: `desktop/voice_server.py` — Python (Silero VAD + MLX Whisper + Silero TTS)
- **LLM**: MLX server at :8234 (Qwen3-32B-4bit)
- **DB**: SQLite at ~/Library/Application Support/Hanni/hanni.db

## How to Analyze

1. **Read the relevant code sections** using Grep/Read for the specified area
2. **Identify issues** in these categories:
   - **Bugs**: Logic errors, race conditions, unhandled errors, edge cases
   - **Performance**: Unnecessary allocations, blocking calls, large prompts, slow paths
   - **UX**: Missing feedback, confusing behavior, accessibility gaps
   - **Code Quality**: Duplication, dead code, missing error handling, naming
   - **Security**: Input validation, injection risks, credential exposure
3. **Propose improvements** — concrete, actionable suggestions with expected impact
4. **Suggest features** — ideas that would enhance the user experience

## Output Format

Respond in Russian. Use this structure:

### 🐛 Баги / Проблемы
1. **[Серьезность: HIGH/MED/LOW]** Описание — файл:строка — как исправить

### ⚡ Оптимизации
1. Что улучшить — ожидаемый эффект

### ✨ Фичи
1. Идея — почему полезно — сложность (easy/medium/hard)

### 📊 Общая оценка
- Качество: X/10
- Критичные проблемы: N
- Quick wins: список быстрых улучшений

## Important

- Be specific — line numbers, function names, concrete code changes
- Prioritize by impact — most critical issues first
- Consider the user's hardware: M3 Pro 36GB, Qwen3-32B-4bit
- The user communicates in Russian — all output in Russian
- Don't suggest switching models or major rewrites unless critical
- Focus on the specified area, don't spread too thin
