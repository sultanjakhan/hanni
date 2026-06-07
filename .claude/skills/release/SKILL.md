---
name: release
description: Automate the Hanni release flow — auto-pick a cheap web-only OTA update (frontend-only changes) or a full native release (Rust/Kotlin/native assets), then ship it.
allowed-tools: Read, Edit, Bash, Grep, Glob
argument-hint: "[X.Y.Z]  (version only needed for a full native release)"
user-invocable: true
---

# Hanni Release Automation

Two release paths, chosen automatically from what changed since the **latest GitHub
release** (the version clients actually run):

- **web** — only frontend (JS/CSS/HTML under `desktop/src/`, minus `sounds/`+`assets/`)
  changed. Ships a few-MB web bundle to the existing release **without a new tag or
  native-version bump**, so the native updater stays silent — clients pull only the
  changed web files, not the ~100MB app. (`web_assets.rs` OTA channel.)
- **native** — Rust/Kotlin/Cargo/icons/native assets changed. Full release: bump
  version, tag, push → CI builds the full bundles + web manifest.

All the logic lives in `scripts/release.sh` (`classify` / `web` / `native`) and
`scripts/publish-web-update.sh`. This skill orchestrates: classify → confirm → run.

Respond in **Russian**. Never force-push. Tag format `vX.Y.Z`.

## Steps

1. **Classify (read-only):**
   ```bash
   bash scripts/release.sh classify
   ```
   Show the user the `DECISION`, `BASE`, `BRANCH`, `DIRTY`, the per-file buckets
   (WEB/NATIVE/SKIP), and the macOS web-OTA health line. Then:
   - **Warn** if `BRANCH` is not `main` (releases normally come from main).
   - **Warn** if the macOS web-OTA line is not `live` — a web-only update won't
     reach this Mac until it's repaired (a native release re-runs the origin
     migration); for Mac-reaching changes prefer a native release.

2. **If `DECISION=none`:** tell the user there's nothing shippable (only SKIP files
   changed) and stop.

3. **If `DECISION=web`:**
   - Confirm with the user (show the WEB file list). The web path auto-derives the
     version (no input needed).
   - Ensure a **clean tree**: `scripts/release.sh web` refuses a dirty tree. If
     there are uncommitted WEB files, `git add` exactly the WEB-classified paths and
     commit with a concise message (focus on WHY; mirror `git log` style), e.g.
     `git commit -m "web: <what changed>"`. Never `git add -A`.
   - Run:
     ```bash
     bash scripts/release.sh web
     ```
   - Report the new `web_version`, the bundle size, and that clients get only the
     few-MB web update (no full download).

4. **If `DECISION=native`:**
   - Decide the version: if `$ARGUMENTS` has an `X.Y.Z`, use it. Else suggest the
     next number from the commit log since the base tag (minor for `feat`, patch for
     `fix` only) and **confirm with the user**:
     ```bash
     git log $(git describe --tags --abbrev=0)..HEAD --oneline
     ```
   - Ensure feature work is committed (the native path only bumps the two version
     files; uncommitted WIP won't be in the release). If the tree has uncommitted
     feature changes the user wants shipped, help commit them first (surgical
     `git add <files>`, not `-A`).
   - Run:
     ```bash
     bash scripts/release.sh native X.Y.Z
     ```
     (bumps `Cargo.toml`+`tauri.conf.json` → commit → tag `vX.Y.Z` locally → push
     branch → push tag → CI builds bundles + web manifest.)
   - Report the version and that CI is building.

## Rules

- **Russian** responses; confirm the path (and version, for native) before running.
- Never run a web-only push concurrently with a tag push (manifest `--clobber` race).
- Never `git add -A`; stage only the classified files.
- The pre-push hook blocks a version bump without a matching tag — `release.sh native`
  tags locally **before** pushing the branch, which satisfies it. Don't `--no-verify`.
- If `gh release view` and the repo's latest tag disagree (a tag pushed but its
  release missing/failed), flag it — `BASE` follows the real release, not the tag.
