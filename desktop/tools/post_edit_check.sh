#!/bin/bash
# PostToolUse hook: syntax/build checks after Claude edits files.
# Receives JSON on stdin. Two channels by cost:
#   - JS/Python syntax: synchronous, fed back to Claude via exit 2 + stderr.
#     Must run before auto-reload.mjs reloads the WebView with broken JS
#     (white-screen failure mode). Zero false positives, so blocking is safe.
#   - cargo check: slow → async, deduped by mkdir-lock, surfaces via macOS
#     notification (does not block Claude).
# Debug-leftover / file-length are intentionally NOT checked here: too many
# false positives in this repo (lib.rs ~8k lines, legit console logging) would
# train Claude to ignore hook output. Those stay as CLAUDE.md judgment rules.

set +e

INPUT=$(cat)
FILE=$(echo "$INPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('tool_input', {}).get('file_path', ''))
except Exception:
    pass
" 2>/dev/null)

[ -z "$FILE" ] && exit 0
[ ! -f "$FILE" ] && exit 0

# Only handle files inside the Hanni repo
case "$FILE" in
  /Users/sultanbekjakhanov/hanni/*) ;;
  *) exit 0 ;;
esac

NAME=$(basename "$FILE")

# --- Synchronous, blocking syntax checks (exit 2 → fed back to Claude) ---
case "$FILE" in
  *.js|*.mjs|*.cjs)
    if ! ERR=$(node --check "$FILE" 2>&1); then
      echo "JS syntax error in $NAME (fix before continuing — auto-reload will white-screen the WebView):" >&2
      echo "$ERR" >&2
      exit 2
    fi
    ;;
  *.py)
    # ast.parse instead of py_compile: checks syntax without writing __pycache__
    if ! ERR=$(python3 -c "import ast,sys; ast.parse(open(sys.argv[1]).read(), sys.argv[1])" "$FILE" 2>&1); then
      echo "Python syntax error in $NAME:" >&2
      echo "$ERR" >&2
      exit 2
    fi
    ;;
esac

# --- Async heavy check: cargo (slow) → macOS notification only ---
case "$FILE" in
  /Users/sultanbekjakhanov/hanni/desktop/src-tauri/*.rs)
    (
      lock=/tmp/hanni-cargo-check.lock
      if mkdir "$lock" 2>/dev/null; then
        trap "rmdir $lock 2>/dev/null" EXIT
        cd /Users/sultanbekjakhanov/hanni/desktop/src-tauri || exit 0
        if ! UPDATER_GITHUB_TOKEN=dummy cargo check 2>/tmp/hanni-cargo-check.log; then
          osascript -e 'display notification "cargo check failed" with title "Hanni hook" subtitle "see /tmp/hanni-cargo-check.log"' 2>/dev/null
        fi
      fi
    ) > /dev/null 2>&1 &
    disown 2>/dev/null
    ;;
esac

exit 0
