// sync-trigger.js — debounced push trigger for local writes.
//
// Wired from state.js: when invoke('add_*' / 'create_*' / 'delete_*' /
// 'update_*' / 'save_*' / 'toggle_*' / 'archive_*') resolves, requestPush()
// schedules a single cloud_owner_push run after a short debounce. Bursts of
// writes coalesce into one push; outbound latency drops from the auto-loop
// interval to ~debounce window. Auto-loop polling still runs and covers
// commands the prefix list misses (start_/stop_/complete_/...).
//
// Uses raw Tauri invoke to avoid an import cycle with state.js.

const _rawInvoke = window.__TAURI__?.core?.invoke;
const DEBOUNCE_MS = 800;

let pushTimer = null;
let inFlight = false;
let pendingAfterFlight = false;

function fire() {
  if (!_rawInvoke) return;
  if (inFlight) { pendingAfterFlight = true; return; }
  inFlight = true;
  _rawInvoke('cloud_owner_push')
    .catch(() => {})
    .finally(() => {
      inFlight = false;
      if (pendingAfterFlight) {
        pendingAfterFlight = false;
        requestPush();
      }
    });
}

export function requestPush() {
  if (!_rawInvoke) return;
  if (pushTimer) clearTimeout(pushTimer);
  pushTimer = setTimeout(() => {
    pushTimer = null;
    fire();
  }, DEBOUNCE_MS);
}
