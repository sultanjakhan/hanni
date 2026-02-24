---
name: release
description: Automate the full Hanni release flow — bump version, commit, push, tag, and push tag. Optionally generates changelog.
allowed-tools: Read, Edit, Bash, Grep, Glob
argument-hint: <version>
user-invocable: true
---

# Hanni Release Automation

Automate the full release pipeline for Hanni.

## Input

The argument is the new version number (e.g., `0.18.0`). If not provided, ask the user.

## Steps

1. **Validate version format** — must be semver (X.Y.Z)

2. **Bump version in all places:**
   - `desktop/src-tauri/Cargo.toml` → `version = "X.Y.Z"`
   - `desktop/src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
   - Verify both files have been updated correctly

3. **Generate brief changelog** from commits since last tag:
   ```bash
   git log $(git describe --tags --abbrev=0)..HEAD --oneline
   ```
   Group by type: feat, fix, refactor, etc.

4. **Commit:**
   ```bash
   git add desktop/src-tauri/Cargo.toml desktop/src-tauri/tauri.conf.json
   git commit -m "release: v{VERSION}"
   ```

5. **Push + Tag + Push tag:**
   ```bash
   git push origin main
   git tag v{VERSION}
   git push origin v{VERSION}
   ```

6. **Report** the release summary to the user.

## Rules

- Respond in Russian
- Always confirm the version with the user before making changes
- If there are uncommitted changes, warn the user first
- Never force push
- The tag format is `vX.Y.Z` (with `v` prefix)
- After pushing the tag, GitHub Actions will handle the rest (build, publish, update gist)
