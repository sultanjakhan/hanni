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

async function fire() {
  if (!_rawInvoke) return;
  if (inFlight) { pendingAfterFlight = true; return; }
  inFlight = true;
  try {
    // cloud_owner_push is backend-agnostic (Firestore OR GitHub — push_inner
    // dispatches internally). The LAN transport is independent, so fan out to
    // it too when configured: a local write then propagates over Wi-Fi/Tailscale
    // within the debounce window instead of waiting for the 15s auto-loop.
    const jobs = [_rawInvoke('cloud_owner_push').catch(() => {})];
    try {
      const lan = await _rawInvoke('lan_sync_get_config');
      if (lan && lan.enabled && lan.peer) {
        jobs.push(_rawInvoke('lan_sync_now').catch(() => {}));
      }
    } catch (_) {}
    await Promise.allSettled(jobs);
  } finally {
    inFlight = false;
    if (pendingAfterFlight) {
      pendingAfterFlight = false;
      requestPush();
    }
  }
}

export function requestPush() {
  if (!_rawInvoke) return;
  if (pushTimer) clearTimeout(pushTimer);
  pushTimer = setTimeout(() => {
    pushTimer = null;
    fire();
  }, DEBOUNCE_MS);
}
