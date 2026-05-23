// health-auto-sync.js — Pull sleep/steps/HR/exercise from Health Connect
// (Android), fan into Calendar + Timeline, then push to Mac so the laptop
// catches new sleep/walks within seconds instead of waiting for the LAN
// auto-loop.

import { invoke, IS_MOBILE } from './state.js';
import { localDate } from './utils.js';

const LS_KEY = 'hc_last_sync';
// 1 min: Samsung Health writes to Health Connect at variable times after the
// watch records, so a tight throttle catches fresh data faster. The actual
// import runs only as often as the poller (and visibilitychange) calls us.
const MIN_INTERVAL_MS = 60 * 1000;

let inflight = null;

/**
 * Pull from Health Connect (no-op on non-Android), fan dates into Calendar +
 * Timeline, then trigger a one-shot LAN push so the Mac sees fresh data
 * immediately rather than waiting up to 15s for the auto-sync loop.
 *
 * On first launch (or explicit force) checks permissions and opens the
 * Health Connect system UI if any of sleep/steps/HR/exercise is denied —
 * Android wipes runtime perms on reinstall, so without this the import
 * silently returns zeros.
 */
export async function autoImportHealth(opts = {}) {
  if (!IS_MOBILE) return false;
  if (inflight) return inflight;
  if (!opts.force) {
    const last = +(localStorage.getItem(LS_KEY) || 0);
    if (Date.now() - last < MIN_INTERVAL_MS) return false;
  }
  inflight = (async () => {
    try {
      const granted = await invoke('health_has_permissions').catch(() => false);
      if (!granted) {
        // Auto-request only on first ever import or when the caller forces
        // it (Settings button) — we don't want to spam the system dialog
        // on every periodic poll.
        const firstLaunch = !localStorage.getItem(LS_KEY);
        if (!firstLaunch && !opts.force) return false;
        const ok = await invoke('health_request_permissions').catch(() => false);
        if (!ok) return false;
      }
      await invoke('import_health_connect_all');
      const dates = Array.from({ length: 7 }, (_, i) => localDate(-i));
      await Promise.all(dates.flatMap(date => [
        invoke('sync_health_to_calendar', { date }).catch(() => {}),
        invoke('sync_health_to_timeline', { date }).catch(() => {}),
      ]));
      // Push to Mac NOW so it sees fresh sleep/walks within ~1s instead of
      // waiting up to 15s for the lan_sync auto-loop tick.
      invoke('lan_sync_now').catch(() => {});
      localStorage.setItem(LS_KEY, String(Date.now()));
      return true;
    } catch (_) {
      return false;
    } finally {
      inflight = null;
    }
  })();
  return inflight;
}

/**
 * Periodic background poll. Health Connect doesn't push, so we poll every
 * 3 min while the app is in the foreground. Combined with the
 * visibilitychange + foreground hook this gets sleep/walks into Hanni —
 * and onward to the Mac — within a few minutes of HC writing.
 */
let pollHandle = null;
export function startHealthPolling() {
  if (!IS_MOBILE) return;
  if (pollHandle) return;
  pollHandle = setInterval(() => { autoImportHealth().catch(() => {}); }, 3 * 60 * 1000);
}
