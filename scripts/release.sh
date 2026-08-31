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
#   web <A.B.C.N>     publish a web-only OTA update. Requires a clean tree and
#                     refuses if any change classifies NATIVE.
#   native <X.Y.Z>    full release: bump versions, commit, tag, push.
#
# Normally invoked by the /release skill, which adds confirmation + reporting.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PYTHON=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 \
     && "$candidate" -c 'import sys; raise SystemExit(sys.version_info < (3, 9))' \
          >/dev/null 2>&1; then
    PYTHON="$candidate"
    break
  fi
done
if [ -z "$PYTHON" ]; then
  echo "refuse: a working Python 3.9+ interpreter is required." >&2
  exit 1
fi

base_tag() { gh release view --json tagName -q .tagName; }

require_clean_main_at_origin() {
  local branch head remote
  branch="$(git branch --show-current)"
  if [ "$branch" != main ]; then
    echo "refuse: releases are allowed only from main (current: ${branch:-detached})." >&2
    exit 1
  fi
  if [ -n "$(git status --porcelain)" ]; then
    echo "refuse: working tree is not clean." >&2
    exit 1
  fi
  git fetch --prune origin main --tags
  head="$(git rev-parse HEAD)"
  remote="$(git rev-parse refs/remotes/origin/main)"
  if [ "$head" != "$remote" ]; then
    echo "refuse: local main must exactly match origin/main before release." >&2
    exit 1
  fi
}

require_no_active_native_release() {
  local active
  active="$(gh run list --workflow release.yml --limit 100 \
    --json databaseId,status \
    --jq '[.[] | select(.status != "completed")] | length')"
  if [ "$active" != 0 ]; then
    echo "refuse: a native release workflow is already queued or running." >&2
    exit 1
  fi
}

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
  local web_version="${1:?usage: scripts/release.sh web <A.B.C.N>}"
  require_clean_main_at_origin
  require_no_active_native_release
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
  bash "$ROOT/scripts/publish-web-update.sh" "$web_version"
  echo "✓ web-only update verified on the release; no native download for clients."
}

cmd_native() {
  local ver="${1:?usage: scripts/release.sh native <X.Y.Z>}"
  echo "$ver" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "bad version: $ver (want X.Y.Z)" >&2; exit 1; }
  require_clean_main_at_origin
  require_no_active_native_release
  if git show-ref --verify --quiet "refs/tags/v$ver" \
     || git ls-remote --exit-code --tags origin "refs/tags/v$ver" >/dev/null 2>&1 \
     || gh release view "v$ver" >/dev/null 2>&1; then
    echo "refuse: tag or release v$ver already exists." >&2
    exit 1
  fi
  "$PYTHON" - "$ver" <<'PY'
import json, pathlib, re, sys

ver = sys.argv[1]
conf_path = pathlib.Path("desktop/src-tauri/tauri.conf.json")
cargo_path = pathlib.Path("desktop/src-tauri/Cargo.toml")
conf_version = json.loads(conf_path.read_text())["version"]
cargo_match = re.search(
    r'(?ms)^\[package\]\s*$.*?^version\s*=\s*"([^"]+)"', cargo_path.read_text()
)
if not cargo_match:
    raise SystemExit("Cargo.toml package version not found")
cargo_version = cargo_match.group(1)
if conf_version != cargo_version:
    raise SystemExit(
        f"current versions disagree: tauri.conf.json={conf_version}, Cargo.toml={cargo_version}"
    )
if tuple(map(int, ver.split("."))) <= tuple(map(int, conf_version.split("."))):
    raise SystemExit(f"new version {ver} must be greater than current version {conf_version}")

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
  cargo check --manifest-path desktop/src-tauri/Cargo.toml
  cargo check --locked --manifest-path desktop/src-tauri/Cargo.toml
  "$PYTHON" - "$ver" <<'PY'
import pathlib, re, sys

expected = sys.argv[1]
lock = pathlib.Path("desktop/src-tauri/Cargo.lock").read_text()
versions = []
for block in re.split(r'(?m)^\[\[package\]\]\s*$', lock):
    name = re.search(r'(?m)^name\s*=\s*"([^"]+)"', block)
    version = re.search(r'(?m)^version\s*=\s*"([^"]+)"', block)
    if name and version and name.group(1) == "hanni":
        versions.append(version.group(1))
if versions != [expected]:
    raise SystemExit(f"Cargo.lock does not contain hanni {expected}: {versions}")
PY
  unexpected="$(git status --porcelain | awk '{ print substr($0, 4) }' \
    | grep -Ev '^(desktop/src-tauri/Cargo.toml|desktop/src-tauri/Cargo.lock|desktop/src-tauri/tauri.conf.json)$' || true)"
  if [ -n "$unexpected" ]; then
    echo "refuse: release preparation changed unexpected files:" >&2
    echo "$unexpected" >&2
    exit 1
  fi
  require_no_active_native_release
  git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/tauri.conf.json
  git commit -m "release: v$ver"
  # Tag locally BEFORE pushing main so the pre-push hook sees the bump has a tag.
  git tag "v$ver"
  git push --atomic origin main "v$ver"
  echo "✓ full release v$ver pushed; CI builds the bundles + web manifest."
}

case "${1:-}" in
  classify) cmd_classify ;;
  web)      shift; cmd_web "$@" ;;
  native)   shift; cmd_native "$@" ;;
  *) echo "usage: scripts/release.sh {classify|web <A.B.C.N>|native <X.Y.Z>}" >&2; exit 2 ;;
esac
