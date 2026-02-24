---
name: changelog
description: Generate a changelog from git commits since the last tag, grouped by type (feat, fix, refactor, etc.).
allowed-tools: Bash, Read, Grep, Glob, Write, Edit
argument-hint: [since-tag]
user-invocable: true
---

# Generate Changelog

Analyze git history and generate a structured changelog.

## Input

- Optional argument: tag to start from (e.g., `v0.17.0`). If not provided, uses the latest tag.

## Steps

1. **Find the starting point:**
   ```bash
   git describe --tags --abbrev=0
   ```
   Or use the provided tag.

2. **Get all commits since that tag:**
   ```bash
   git log {tag}..HEAD --oneline --no-merges
   ```

3. **Categorize commits** by prefix:
   - `feat:` → New Features
   - `fix:` → Bug Fixes
   - `refactor:` → Refactoring
   - `perf:` → Performance
   - `docs:` → Documentation
   - `style:` → Style
   - `chore:` → Chores
   - `release:` → (skip)
   - No prefix → Other

4. **Generate changelog** in this format:

   ```markdown
   # Changelog vX.Y.Z

   ## New Features
   - Feature description (commit hash)

   ## Bug Fixes
   - Fix description (commit hash)

   ## Refactoring
   - Refactor description (commit hash)
   ```

5. **Check for existing changelog** at `docs/architecture/CHANGELOG-vX.Y.Z.md`

6. **Present to user** — show the changelog and ask if they want it saved to a file

## Rules

- Respond in Russian
- Keep descriptions concise — one line per change
- Highlight breaking changes prominently
- Include commit hashes as short refs (7 chars)
- Group related commits together
- If a commit touches voice, chat, memory, etc. — add a tag like [voice], [chat]
