#!/bin/bash
# PostToolUse hook: async syntax/build check after Claude edits files.
# Receives JSON on stdin, fires fire-and-forget checks based on extension.
# Errors surface via macOS notifications, not stderr — non-blocking.

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

run_js_check() {
  if ! node --check "$FILE" 2>/tmp/hanni-jscheck.log; then
    osascript -e "display notification \"JS syntax error: ${NAME//\"/\\\"}\" with title \"Hanni hook\" subtitle \"see /tmp/hanni-jscheck.log\"" 2>/dev/null
  fi
}

run_cargo_check() {
  local lock=/tmp/hanni-cargo-check.lock
  if mkdir "$lock" 2>/dev/null; then
    trap "rmdir $lock 2>/dev/null" EXIT
    cd /Users/sultanbekjakhanov/hanni/desktop/src-tauri || exit 0
    if ! UPDATER_GITHUB_TOKEN=dummy cargo check 2>/tmp/hanni-cargo-check.log; then
      osascript -e 'display notification "cargo check failed" with title "Hanni hook" subtitle "see /tmp/hanni-cargo-check.log"' 2>/dev/null
    fi
  fi
}

case "$FILE" in
  *.js|*.mjs|*.cjs)
    (run_js_check) > /dev/null 2>&1 &
    disown 2>/dev/null
    ;;
  /Users/sultanbekjakhanov/hanni/desktop/src-tauri/*.rs)
    (run_cargo_check) > /dev/null 2>&1 &
    disown 2>/dev/null
    ;;
esac

exit 0
