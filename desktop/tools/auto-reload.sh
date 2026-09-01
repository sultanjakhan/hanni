#!/bin/bash
# auto-reload.sh — watches desktop/src for JS/CSS/HTML changes and reloads
# the running debug Hanni WebView via the fixed /auto/reload action.
#
# Usage:
#   HANNI_DEV_RELOAD_TOKEN=<uuid> ./tools/auto-reload.sh
#
# Run once and leave it in a background terminal. Edit JS/CSS, see the change.

if [ "$#" -ne 0 ]; then
  echo "auto-reload accepts no arguments and targets debug port 8236 only" >&2
  exit 2
fi
if [ -z "${HANNI_DEV_RELOAD_TOKEN:-}" ]; then
  echo "HANNI_DEV_RELOAD_TOKEN is required" >&2
  exit 1
fi
HERE="$(cd "$(dirname "$0")" && pwd)"
exec node "$HERE/auto-reload.mjs"
