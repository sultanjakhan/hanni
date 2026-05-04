// health-auto-sync.js — Pull sleep/steps/HR/exercise from Health Connect (Android),
// then sync the recent days into Calendar Day-view + Timeline. Mac receives the
// same data via CR-SQLite + Firestore CRDT, no extra work needed.

import { invoke, IS_MOBILE } from './state.js';

const LS_KEY = 'hc_last_sync';
const MIN_INTERVAL_MS = 5 * 60 * 1000; // throttle: at most once per 5 min

let inflight = null;

function localDate(offsetDays = 0) {
  const d = new Date();
  d.setDate(d.getDate() + offsetDays);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/**
 * Pull from Health Connect (no-op on non-Android), then fan out the latest
 * dates into Calendar + Timeline so the user sees fresh sleep blocks
 * immediately after wake.
 *
 * @param {{force?: boolean}} opts
 * @returns {Promise<boolean>} true on successful import, false otherwise
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
      await invoke('import_health_connect_all');
      // Sync the last 7 days into Calendar + Timeline so freshly imported
      // sleep sessions land everywhere, not just today/yesterday.
      const dates = Array.from({ length: 7 }, (_, i) => localDate(-i));
      await Promise.all(dates.flatMap(date => [
        invoke('sync_health_to_calendar', { date }).catch(() => {}),
        invoke('sync_health_to_timeline', { date }).catch(() => {}),
      ]));
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
 * 15 min while the app is in the foreground. Combined with visibilitychange
 * + foreground hook this gets sleep into Hanni within minutes of HC writing.
 */
let pollHandle = null;
export function startHealthPolling() {
  if (!IS_MOBILE) return;
  if (pollHandle) return;
  pollHandle = setInterval(() => { autoImportHealth().catch(() => {}); }, 15 * 60 * 1000);
}
