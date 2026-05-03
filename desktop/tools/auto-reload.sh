#!/bin/bash
# auto-reload.sh — watches desktop/src for JS/CSS/HTML changes and reloads
# the running Hanni dev WebView via /auto/eval. No bundler, no Tauri devUrl.
#
# Usage:
#   ./tools/auto-reload.sh           # watches dev (port 8236)
#   ./tools/auto-reload.sh 8235      # watches prod (port 8235) — use carefully
#
# Run once and leave it in a background terminal. Edit JS/CSS, see the change.

PORT="${1:-8236}"
HERE="$(cd "$(dirname "$0")" && pwd)"
exec env HANNI_DEV_PORT="$PORT" node "$HERE/auto-reload.mjs"
