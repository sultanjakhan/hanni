#!/usr/bin/env bash
# release.sh — smart release entry. Classifies what changed since the latest
# GitHub RELEASE (the version clients actually run) and routes to either:
#   - a cheap web-only OTA update (frontend-only change -> clients pull a few-MB
#     web bundle, no new tag, no native bump -> native updater stays silent), or
#   - a full native release (Rust/Kotlin/native assets changed -> full build).
#
# Base = `gh release view` (releases/latest), NOT `git describe` — a tag can be
# pushed without a release (CI failure), and clients read releases/latest.
#
# Subcommands:
#   classify          read-only. Prints DECISION=web|native|none, the changed
#                     files (committed since base + working tree), and macOS
#                     web-OTA health. Always exits 0 unless it errors.
#   web               publish a web-only OTA update. Requires a clean tree and
#                     refuses if any change classifies NATIVE.
#   native <X.Y.Z>    full release: bump versions, commit, tag, push.
#
# Normally invoked by the /release skill, which adds confirmation + reporting.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

base_tag() { gh release view --json tagName -q .tagName; }

# Print "<BUCKET> <path>" for every changed path: committed since base UNION the
# working tree (staged + unstaged + untracked), so the decision reflects what a
# release would actually ship — not just what's committed.
classify_files() {
  local base="$1"
  {
    git diff --name-only "$base"..HEAD
    git status --porcelain | awk '{ if ($0 ~ / -> /) { sub(/.* -> /, ""); print } else { print substr($0, 4) } }'
  } | sort -u | while IFS= read -r f; do
    [ -z "$f" ] && continue
    case "$f" in
      # Heavy static dirs are excluded from the web bundle -> embedded-only -> a
      # change there needs a full native build.
      desktop/src/sounds/*|desktop/src/assets/*) echo "NATIVE $f" ;;
      */CLAUDE.md)                               echo "SKIP   $f" ;;
      desktop/src/*)                             echo "WEB    $f" ;;
      # Dev-only / not shipped in the app bundle.
      docs/*|.claude/*|.github/*|scripts/*)      echo "SKIP   $f" ;;
      README.md|TASKS.md|projects.yaml)          echo "SKIP   $f" ;;
      # Everything else (Rust, Cargo, gen/android, libs, icons, tauri*.conf.json,
      # build.rs, package.json, separate services) -> conservative: NATIVE.
      *)                                         echo "NATIVE $f" ;;
    esac
  done
}

macos_ota_health() {
  local f="$HOME/Library/Application Support/com.sultanjakhan.hanni/web/origin_stage"
  local v="$HOME/Library/Application Support/com.sultanjakhan.hanni/web/version.txt"
  if [ -f "$f" ]; then
    local stage applied
    stage="$(cat "$f" 2>/dev/null)"
    applied="$(cat "$v" 2>/dev/null || echo '?')"
    echo "macOS web-OTA: ${stage:-empty} (applied web=$applied)"
  else
    echo "macOS web-OTA: not initialized on this Mac (origin_stage absent)"
  fi
}

cmd_classify() {
  local base lines decision
  base="$(base_tag)"
  lines="$(classify_files "$base")"
  if echo "$lines" | grep -q '^NATIVE '; then
    decision=native
  elif echo "$lines" | grep -q '^WEB '; then
    decision=web
  else
    decision=none
  fi
  echo "DECISION=$decision"
  echo "BASE=$base"
  echo "BRANCH=$(git branch --show-current)"
  echo "DIRTY=$([ -n "$(git status --porcelain)" ] && echo yes || echo no)"
  echo "--- changed since $base (committed + working tree) ---"
  if [ -n "$lines" ]; then echo "$lines"; else echo "(nothing)"; fi
  echo "--- $(macos_ota_health) ---"
}

cmd_web() {
  if [ -n "$(git status --porcelain)" ]; then
    echo "refuse: working tree not clean — commit the web files first, then re-run." >&2
    exit 1
  fi
  local base lines
  base="$(base_tag)"
  lines="$(classify_files "$base")"
  if echo "$lines" | grep -q '^NATIVE '; then
    echo "refuse: native changes present since $base — use: scripts/release.sh native <X.Y.Z>" >&2
    echo "$lines" | grep '^NATIVE ' >&2
    exit 1
  fi
  if ! echo "$lines" | grep -q '^WEB '; then
    echo "nothing to ship (no web changes since $base)." >&2
    exit 0
  fi
  bash "$ROOT/scripts/publish-web-update.sh"
  # Keep the repo in sync with what shipped (no version bump -> pre-push hook ok).
  git push origin "$(git branch --show-current)"
  echo "✓ web-only update published; no native download for clients."
}

cmd_native() {
  local ver="${1:?usage: scripts/release.sh native <X.Y.Z>}"
  echo "$ver" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "bad version: $ver (want X.Y.Z)" >&2; exit 1; }
  python3 - "$ver" <<'PY'
import sys, re
ver = sys.argv[1]
def bump(path, pat, repl):
    lines = open(path).read().splitlines(keepends=True)
    for i, l in enumerate(lines):
        if re.match(pat, l):
            lines[i] = repl(l); open(path, "w").writelines(lines); return
    sys.exit("version line not found in " + path)
bump("desktop/src-tauri/Cargo.toml", r'^version = "', lambda l: f'version = "{ver}"\n')
bump("desktop/src-tauri/tauri.conf.json", r'^\s*"version":\s*"',
     lambda l: re.sub(r'"version":\s*"[^"]*"', f'"version": "{ver}"', l))
PY
  git add desktop/src-tauri/Cargo.toml desktop/src-tauri/tauri.conf.json
  git commit -m "release: v$ver"
  # Tag locally BEFORE pushing main so the pre-push hook sees the bump has a tag.
  git tag "v$ver"
  git push origin "$(git branch --show-current)"
  git push origin "v$ver"
  echo "✓ full release v$ver pushed; CI builds the bundles + web manifest."
}

case "${1:-}" in
  classify) cmd_classify ;;
  web)      cmd_web ;;
  native)   shift; cmd_native "$@" ;;
  *) echo "usage: scripts/release.sh {classify|web|native <X.Y.Z>}" >&2; exit 2 ;;
esac
