// Coalesce health view invalidation without replacing the active tab or editors.
import { S, listen, invoke, IS_MOBILE } from './state.js';

let started = false, pending = false, scheduled = false, pointerDown = false;
let errorRetry = null;
let sleepPane = null, refreshSleep = null, revision = null, checking = false;
const rendered = new WeakMap();
const reads = new WeakMap();

export function beginHealthViewRead(el) {
  const version = (reads.get(el) || 0) + 1;
  reads.set(el, version);
  return () => el.isConnected && reads.get(el) === version;
}

function visibleTarget() {
  if (document.visibilityState !== 'visible') return null;
  const view = document.getElementById(`view-${S.activeTab}`);
  if (!view?.classList.contains('active') || S.activeSubTab[S.activeTab] === 'Настройки') return null;
  if (S.activeTab === 'calendar') return view.querySelector('[data-calendar-records], #calendar-inner-content');
  if (S.activeTab === 'health' && S._unifiedPane?.health === 'sleep' && sleepPane?.isConnected && sleepPane.querySelector('.sleep-list')) return sleepPane;
  return null;
}

export function canRefreshHealthView(el) {
  const target = visibleTarget();
  if (!target || !el?.isConnected || (!target.contains(el) && !el.contains(target))) return false;
  if (pointerDown || document.querySelector('dialog[open], .modal-overlay, .cal-event-pop, .dragging')) return false;
  return !document.activeElement?.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])');
}

// Check again after asynchronous reads: an editor may have opened meanwhile.
// Fingerprints remain in memory, never in telemetry, storage or event payloads.
export function mayCommitHealthView(el, fingerprint, quiet = false) {
  if (quiet && !canRefreshHealthView(el)) { requestHealthViewRefresh(); return false; }
  if (quiet && rendered.get(el) === fingerprint) return false;
  rendered.set(el, fingerprint);
  return true;
}

export function registerSleepHealthView(el, refresh) {
  sleepPane = el; refreshSleep = refresh;
}

export function requestHealthViewRefresh() {
  pending = true;
  schedule();
}

// A failed view read keeps existing rows and retries once after a delay.
// A newer real change can still request an immediate refresh independently.
export function retryHealthViewRefresh() {
  if (errorRetry !== null) return;
  errorRetry = window.setTimeout(() => { errorRetry = null; pending = true; schedule(); }, 15_000);
}

function schedule() {
  if (!pending || scheduled) return;
  scheduled = true;
  // One deferred turn also lets pointer-up/click and dialog-close finish.
  window.setTimeout(() => {
    scheduled = false;
    const target = visibleTarget();
    if (!pending || !target || !canRefreshHealthView(target)) return;
    pending = false;
    if (S.activeTab === 'calendar') {
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh', { detail: { quietHealth: true } }));
    } else {
      Promise.resolve(refreshSleep?.()).catch(() => {});
    }
  }, 0);
}

async function checkProjectionRevision() {
  const target = visibleTarget();
  if (!IS_MOBILE || checking || !target || !canRefreshHealthView(target)) return;
  checking = true;
  try {
    const status = await invoke('cloud_relay_status');
    if (typeof status?.projection?.projection_revision !== 'string') return;
    if (revision !== status.projection.projection_revision) {
      revision = status.projection.projection_revision;
      requestHealthViewRefresh();
    }
  } catch (_) { /* Keep the last proven revision; the next visible check retries. */ }
  finally { checking = false; }
}

export function startHealthViewRefresh() {
  if (started) return;
  started = true;
  listen('cloud-relay-updated', event => {
    if (event.payload?.views_changed === true) requestHealthViewRefresh();
  }).catch(() => {});
  document.addEventListener('pointerdown', () => { pointerDown = true; }, true);
  const released = () => { pointerDown = false; schedule(); };
  document.addEventListener('pointerup', released, true);
  document.addEventListener('pointercancel', released, true);
  document.addEventListener('focusout', schedule, true);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') { schedule(); void checkProjectionRevision(); }
    else pointerDown = false;
  });
  window.addEventListener('focus', () => { schedule(); void checkProjectionRevision(); });
  // Only pending invalidations inspect DOM changes, so closing an editor or
  // switching back to the pane releases the deferred refresh without polling.
  new MutationObserver(() => { if (pending) schedule(); }).observe(document.body, {
    childList: true, subtree: true, attributes: true, attributeFilter: ['open', 'class', 'hidden'],
  });
  // Android background JNI has no WebView event handle. This is a read-only
  // visible-pane revision check; it neither reads HC nor starts sync work.
  if (IS_MOBILE) window.setInterval(checkProjectionRevision, 15_000);
  void checkProjectionRevision();
}
