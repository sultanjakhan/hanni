#!/bin/bash
# dev.sh — Single-instance dev launcher for Hanni with auto-restart
#
# Features:
#   - Only 1 dev instance at a time (PID-based lock)
#   - If another is running, waits in queue
#   - Detects stale locks (crashed process)
#   - Auto-restarts on crash / manual quit (loop mode)
#   - Ctrl+C to stop completely
#
# Usage:
#   ./dev.sh          # run once
#   ./dev.sh --loop   # auto-restart on exit (hot loop)

LOCKFILE="/tmp/hanni-dev.pid"
LOOP=false
[[ "$1" == "--loop" ]] && LOOP=true

cd "$(dirname "$0")/.." || exit 1

is_alive() {
  [[ -f "$LOCKFILE" ]] && kill -0 "$(cat "$LOCKFILE" 2>/dev/null)" 2>/dev/null
}

acquire_lock() {
  while true; do
    if [[ -f "$LOCKFILE" ]]; then
      local pid; pid=$(cat "$LOCKFILE" 2>/dev/null)
      if kill -0 "$pid" 2>/dev/null; then
        echo "[dev] Instance running (PID $pid). Waiting..."
        # Wait until that PID exits
        while kill -0 "$pid" 2>/dev/null; do sleep 2; done
        echo "[dev] Previous instance exited."
      else
        echo "[dev] Stale lock (PID $pid dead). Removing."
        rm -f "$LOCKFILE"
      fi
    fi
    # Try to claim
    echo $$ > "$LOCKFILE"
    # Verify we own it (race protection)
    if [[ "$(cat "$LOCKFILE" 2>/dev/null)" == "$$" ]]; then
      break
    fi
  done
  trap 'rm -f "$LOCKFILE"; exit' EXIT INT TERM
}

run_dev() {
  echo "[dev] Starting cargo tauri dev (PID $$)..."
  UPDATER_GITHUB_TOKEN=dummy cargo tauri dev 2>&1
  local code=$?
  echo "[dev] Exited with code $code."
  return $code
}

# ── Main ──
# Prod and dev don't conflict — prod on 8235/8239, dev on 8236/8240.
if pgrep -if "Hanni.app" >/dev/null 2>&1; then
  echo "[dev] Note: production Hanni is running (separate ports — no conflict)."
fi
acquire_lock

if $LOOP; then
  echo "[dev] Loop mode — auto-restart on exit. Ctrl+C to stop."
  while true; do
    run_dev
    echo "[dev] Restarting in 3s... (Ctrl+C to stop)"
    sleep 3
  done
else
  run_dev
fi
