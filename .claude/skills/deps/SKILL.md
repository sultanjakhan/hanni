---
name: deps
description: Dependency Manager — audit, update, and manage Cargo, pip, and npm dependencies.
allowed-tools: Read, Edit, Grep, Glob, Bash, Task
argument-hint: [task]
user-invocable: true
---

# Hanni Dependency Manager

You audit, update, and manage all project dependencies.

## Tasks

| Task | What it does |
|------|-------------|
| `audit` | Full audit — outdated, unused, vulnerable, heavy dependencies |
| `update` | Update dependencies to latest compatible versions |
| `unused` | Find and remove unused dependencies |
| `heavy` | Identify heaviest dependencies by compile time / binary size |
| `security` | Check for known vulnerabilities |
| `license` | Check dependency licenses for compatibility |

If no task specified, default to `audit`.

## Dependency Files

- **Rust**: `desktop/src-tauri/Cargo.toml` + `Cargo.lock`
- **Python**: `desktop/voice_server.py` (imports), `requirements.txt` if exists
- **JavaScript**: `desktop/package.json` if exists (Tauri frontend)

## How to Work

### For `audit`:
1. **Rust dependencies:**
   ```bash
   cd desktop/src-tauri && cargo outdated 2>/dev/null || echo "cargo-outdated not installed"
   ```
   If cargo-outdated not available, read Cargo.toml and check versions manually.

2. **Check Cargo.lock for dependency count:**
   ```bash
   grep -c '^\[\[package\]\]' desktop/src-tauri/Cargo.lock
   ```

3. **Python dependencies:**
   - Read imports in `voice_server.py`
   - Check installed versions:
     ```bash
     pip3 list 2>/dev/null | grep -E "silero|whisper|mlx|edge-tts|fastapi|uvicorn"
     ```

4. **Evaluate each dependency:**
   - Is it actively maintained?
   - Is it the right tool for the job?
   - Could it be replaced with something lighter?
   - Are we using a small fraction of a large library?

### For `update`:
1. Read current versions from Cargo.toml
2. Check for breaking changes in changelogs
3. Update conservatively:
   - Patch updates: safe, apply all
   - Minor updates: usually safe, check changelog
   - Major updates: propose to user, don't auto-apply
4. After updating Rust deps:
   ```bash
   cd desktop/src-tauri && UPDATER_GITHUB_TOKEN=dummy cargo check
   ```

### For `unused`:
- Rust: check each dependency in Cargo.toml against `use` statements in code
- Python: check each import against pip installed packages
- Look for feature flags that enable unused functionality

### For `heavy`:
- Check compile times per crate (if possible)
- Check binary size contribution
- Identify deps that pull in many transitive dependencies

## Output Format

```markdown
## Dependency Audit

### Rust (N зависимостей)
| Пакет | Текущая | Последняя | Статус |
|-------|---------|-----------|--------|
| serde | 1.0.190 | 1.0.195 | ⬆️ minor update |
| tauri | 2.0.0 | 2.1.0 | ⚠️ review changelog |

### Python (N пакетов)
| Пакет | Версия | Заметки |
|-------|--------|---------|
| edge-tts | 6.1.9 | ✅ current |

### Рекомендации
1. [Что обновить] — [Почему]
2. [Что удалить] — [Почему]
```

## Rules

- Respond in Russian
- Never auto-update major versions without user approval
- After any Rust dependency change, run `cargo check`
- Consider compile time impact (Rust compilation is slow)
- Consider binary size impact
- Don't suggest replacing core dependencies (tauri, serde, rusqlite)
- Consider macOS ARM64 compatibility
