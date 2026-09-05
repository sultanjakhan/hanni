// health-auto-sync.js — Pull sleep/steps/HR/exercise from Health Connect
// (Android), fan into Calendar + Timeline, then push to Mac so the laptop
// catches new sleep/walks within seconds instead of waiting for the LAN
// auto-loop.

import { invoke, IS_MOBILE } from './state.js';
import { localDate } from './utils.js';
import { requestHealthViewRefresh } from './health-view-refresh.js';

const LS_KEY = 'hc_last_sync';
const PERMISSION_PROMPT_KEY = 'hc_permission_prompted_at';
const BG_PROMPT_KEY = 'hc_bg_asked';
const PROMPT_RETRY_MS = 24 * 60 * 60 * 1000;
// 1 min: Samsung Health writes to Health Connect at variable times after the
// watch records, so a tight throttle catches fresh data faster. The actual
// import runs only as often as the poller (and visibilitychange) calls us.
const MIN_INTERVAL_MS = 60 * 1000;

let inflight = null;
let permissionRequest = null;
let rawInflight = null;
let rawContinuation = null;

function promptIsDue(key) {
  const last = Number(localStorage.getItem(key) || 0);
  return !Number.isFinite(last) || Date.now() - last >= PROMPT_RETRY_MS;
}

async function requestPermissionsIfDue(key) {
  if (permissionRequest) return permissionRequest;
  if (!promptIsDue(key) || !promptIsDue(PERMISSION_PROMPT_KEY) || !promptIsDue(BG_PROMPT_KEY)) return null;
  // One combined request covers available record types, history and background.
  // Stamp both paths before opening the system UI to avoid a second dialog on resume.
  for (const promptKey of [PERMISSION_PROMPT_KEY, BG_PROMPT_KEY]) {
    localStorage.setItem(promptKey, String(Date.now()));
  }
  permissionRequest = invoke('health_request_permissions').catch(() => null);
  try { return await permissionRequest; } finally { permissionRequest = null; }
}

async function importRawHealth() {
  if (document.visibilityState !== 'visible') return false;
  if (rawInflight) return rawInflight;
  clearTimeout(rawContinuation);
  rawContinuation = null;
  rawInflight = (async () => {
    try {
      const result = await invoke('health_import_raw');
      if (result?.more_pending && document.visibilityState === 'visible') {
        // Drain bounded pages without waiting for the next foreground poll.
        // A real error backs off; ordinary backlog continues immediately.
        rawContinuation = setTimeout(() => { importRawHealth().catch(() => {}); },
          result.retry_needed ? 30_000 : 1000);
      }
      const viewsChanged = (result?.projection?.records || 0) > 0;
      if (viewsChanged) requestHealthViewRefresh();
      return (result?.modified_records || 0) > 0 || viewsChanged;
    } catch (_) { return false; }
    finally { rawInflight = null; }
  })();
  return rawInflight;
}

/**
 * Pull from Health Connect (no-op on non-Android), fan dates into Calendar +
 * Timeline, then trigger a one-shot LAN push so the Mac sees fresh data
 * immediately rather than waiting up to 15s for the auto-sync loop.
 *
 * Checks permissions before importing and periodically re-opens the Health
 * Connect system UI if any read access was revoked. The prompt is rate-limited
 * separately from the import throttle so a stale successful sync cannot leave
 * the pipeline permanently disabled.
 */
export async function autoImportHealth(opts = {}) {
  if (!IS_MOBILE || document.visibilityState !== 'visible') return false;
  if (inflight) return inflight;
  if (!opts.force) {
    const last = +(localStorage.getItem(LS_KEY) || 0);
    if (Date.now() - last < MIN_INTERVAL_MS) return false;
  }
  inflight = (async () => {
    try {
      const granted = await invoke('health_has_permissions').catch(() => false);
      if (!granted) {
        // Health Connect can revoke individual read permissions after an app
        // update or a period without background access. Retry the system UI at
        // most once a day instead of letting an old hc_last_sync value suppress
        // permission recovery forever.
        await requestPermissionsIfDue(PERMISSION_PROMPT_KEY);
      }
      // Archive and Calendar projections have independent permissions/progress.
      // A failed four-type projection must not stop the remaining archive types.
      const archived = await importRawHealth();
      const imported = await invoke('import_health_connect_all').catch(() => null);
      if (!imported?.successful_types?.length) return archived;
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
 * Permission recovery for READ_HEALTH_DATA_IN_BACKGROUND. The 15-min WorkManager
 * sync (HanniHealthWorker) can only read Health Connect in the background with
 * this permission on Android 14+. Without it HC only ever sees foreground
 * access and eventually auto-revokes sleep/steps — the "permission resets"
 * complaint. Foreground access is unaffected, so we only ask when foreground
 * is already granted and rate-limit repeated prompts to once per day.
 */
export async function maybeRequestHealthBackground() {
  if (!IS_MOBILE) return;
  // If foreground isn't granted yet, the normal grant flow already bundles the
  // background permission — nothing extra to do here.
  const fg = await invoke('health_has_permissions').catch(() => false);
  if (!fg) return;
  const st = await invoke('health_background_status').catch(() => null);
  if (!st?.available || st.granted) return;
  // Re-check the actual permission before consulting the cooldown. A previous
  // "asked" marker must not permanently hide a later Android auto-revocation.
  await requestPermissionsIfDue(BG_PROMPT_KEY);
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
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') autoImportHealth({ force: true }).catch(() => {});
    else { clearTimeout(rawContinuation); rawContinuation = null; }
  });
  pollHandle = setInterval(() => { autoImportHealth().catch(() => {}); }, 3 * 60 * 1000);
}
