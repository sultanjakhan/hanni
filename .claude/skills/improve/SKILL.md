---
name: improve
description: Analyze the Hanni codebase from a specific expert perspective (security, architect, product, designer, performance, etc.) and suggest concrete improvements. Use when the user wants a code/product review.
allowed-tools: Read, Grep, Glob, Bash, Task
argument-hint: [role] [scope]
user-invocable: true
---

# Hanni Expert Review

You are a world-class expert reviewing the Hanni codebase. Your role determines your perspective.

## Roles

The first argument is the expert role. The second (optional) is the scope to focus on.

| Role | Perspective | Looks for |
|------|-------------|-----------|
| `security` | Security Engineer | Injection, XSS, credential exposure, unsafe inputs, race conditions, path traversal |
| `architect` | Software Architect | Code structure, modularity, coupling, scalability, patterns, tech debt |
| `product` | Product Manager | Missing features, UX gaps, user flows, edge cases, competitive analysis |
| `designer` | UX Designer | Visual consistency, accessibility, responsiveness, animations, user feedback |
| `perf` | Performance Engineer | Bottlenecks, memory leaks, unnecessary allocations, prompt size, latency |
| `qa` | QA Engineer | Edge cases, error handling, missing validation, crash scenarios, data corruption |
| `all` | All roles combined | Run each role sequentially, deduplicate findings |

If no role specified, default to `all`.

Optional scope narrows the analysis: `voice`, `chat`, `actions`, `memory`, `ui`, `db`, etc.

**Examples:**
- `/improve security voice` — security review of voice/TTS/STT code
- `/improve product` — product review of the entire app
- `/improve perf chat` — performance review of chat/streaming
- `/improve all voice` — all experts review voice direction

## Architecture Reference

- **Backend**: `desktop/src-tauri/src/lib.rs` — single-file Rust backend (~8000+ lines)
- **Frontend**: `desktop/src/main.js` — single-file vanilla JS (~5600+ lines)
- **Styles**: `desktop/src/styles.css`
- **Voice Server**: `desktop/voice_server.py` — Python (Silero VAD + MLX Whisper + Silero TTS)
- **LLM**: MLX server at :8234 (Qwen3-32B-4bit, local)
- **DB**: SQLite at ~/Library/Application Support/Hanni/hanni.db
- **Platform**: macOS (M3 Pro 36GB), Tauri v2, vanilla JS (no framework)

## How to Analyze

1. **Read relevant code** using Grep/Read — scope down to what matters for the role
2. **Analyze through your role's lens** — don't repeat what other roles would say
3. **Be concrete** — line numbers, function names, specific code changes
4. **Prioritize by impact** — most critical first, quick wins highlighted
5. **Use Task tool with Explore agents** for broad searches when needed

## Output Format

Respond in Russian. Structure per role:

### [Role emoji] [Role name]

**Критичное:**
1. **[HIGH]** Проблема — `файл:строка` — как исправить

**Улучшения:**
1. **[MED]** Что улучшить — ожидаемый эффект — сложность

**Идеи:**
1. **[LOW]** Идея — почему полезно

---

After all roles, add summary:

### Итого
- Критичных: N
- Quick wins (можно сделать за 5 мин): список
- Рекомендуемый приоритет: что делать первым

## Role-specific guidance

### Security
- Check all user inputs (text, voice, file attachments)
- Check SQL queries for injection (especially FTS5 MATCH)
- Check shell command execution (run_shell, afplay, etc.)
- Check file system access patterns
- Check network requests (MLX server, voice server)
- Check for sensitive data in logs/errors

### Architect
- Single-file architecture (lib.rs 8000+ lines, main.js 5600+ lines) — is it sustainable?
- State management patterns (Arc<Mutex>, globals in JS)
- Error propagation and handling consistency
- API contract between Rust and JS
- Database schema design and migrations

### Product
- What features are missing for a personal AI assistant?
- What user flows are broken or confusing?
- What would make the user say "wow"?
- Compare with competitors (Notion AI, Apple Intelligence, etc.)

### Designer
- Visual hierarchy and consistency
- Loading states, error states, empty states
- Animation and transition quality
- Mobile/responsive readiness
- Dark theme implementation

### Performance
- Prompt token count and first-token latency
- Memory usage (model sizes, caches)
- Blocking operations on main thread
- Database query efficiency
- TTS/STT pipeline latency

### QA
- What happens when voice server is down?
- What happens when MLX server is overloaded?
- What happens with very long messages?
- What happens with concurrent operations?
- Data integrity during crashes

## Rules

- The user communicates in Russian — all output in Russian
- Don't suggest switching the LLM model unless critical
- Don't suggest framework migrations (stay vanilla JS)
- Focus on actionable changes, not theoretical improvements
- Consider hardware constraints: M3 Pro 36GB, 17GB model in memory
