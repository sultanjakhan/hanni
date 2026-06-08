// ── food-cook-timer.js — parallel per-step ring countdowns for cooking mode ──
// Each step can hold its own timer in `session.timers[idx]`, so several run at
// once (rice + eggs + meat). Timers live in the session object — independent of
// the DOM, so navigating between steps never stops them. The big ring shows the
// CURRENT step's timer; a tray lists every active timer. On zero → chime + notify.
import { chime } from './food-cook-helpers.js';
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export const RING_R = 54;
const CIRC = 2 * Math.PI * RING_R; // 339.29 — matches .cook-ring-progress dasharray
export const fmt = (sec) => `${String(Math.floor(sec / 60)).padStart(2, '0')}:${String(sec % 60).padStart(2, '0')}`;

const timers = (s) => (s.timers || (s.timers = {}));
const stepSeconds = (s, idx) => (s.steps[idx].min || 0) * 60 || 60; // fall back to 1 min

// Paint the big ring for the step currently shown (idx). Falls back to the
// step's full duration when that step has no live timer yet.
export function paintRing(s, idx) {
  const ring = s.root.querySelector('#cook-ring');
  if (!ring) return;
  const t = timers(s)[idx];
  const total = t ? t.total : (s.steps[idx].min || 0) * 60;
  const remaining = t ? t.remaining : total;
  const frac = total ? remaining / total : 1;
  ring.style.strokeDasharray = CIRC;
  ring.style.strokeDashoffset = CIRC * frac; // full offset = empty; 0 = full ring
}

// Start / pause / resume the timer for step `idx` (toggle). onChange re-renders.
export function toggleTimer(s, idx, onChange) {
  let t = timers(s)[idx];
  if (t && t.intervalId) {                 // running → pause
    clearInterval(t.intervalId); t.intervalId = null;
  } else {                                 // start or resume
    if (!t) t = timers(s)[idx] = { remaining: stepSeconds(s, idx), total: stepSeconds(s, idx), intervalId: null, done: false };
    if (t.remaining <= 0) { t.remaining = t.total = stepSeconds(s, idx); t.done = false; } // restart finished
    t.intervalId = setInterval(() => tick(s, idx, onChange), 1000);
  }
  onChange?.();
}

function tick(s, idx, onChange) {
  const t = timers(s)[idx];
  if (!t) return;
  t.remaining = Math.max(0, t.remaining - 1);
  updateDisplays(s, idx);
  if (t.remaining <= 0) {
    clearInterval(t.intervalId); t.intervalId = null; t.done = true;
    onTimerDone(s, idx);
    onChange?.();
  }
}

export function addTime(s, idx, onChange) {
  let t = timers(s)[idx];
  if (!t) t = timers(s)[idx] = { remaining: 0, total: 0, intervalId: null, done: false };
  t.remaining += 60; t.total = Math.max(t.total, t.remaining); t.done = false;
  updateDisplays(s, idx);
  onChange?.();
}

// Per-second update: ring + time only when idx is the visible step, plus the
// tray chip (always), avoiding a full re-render on every tick.
function updateDisplays(s, idx) {
  const t = timers(s)[idx];
  if (idx === s.idx) {
    const el = s.root.querySelector('#cook-time'); if (el && t) el.textContent = fmt(t.remaining);
    paintRing(s, idx);
  }
  const chip = s.root.querySelector(`#cook-tray-${idx} .ctray-time`);
  if (chip && t) chip.textContent = fmt(t.remaining);
}

function onTimerDone(s, idx) {
  if (idx === s.idx) s.root.querySelector('.cook-card')?.classList.add('cook-ringdone');
  chime();
  invoke('send_notification', { title: '🍳 Готовка', body: `Шаг ${idx + 1} готов` }).catch(() => {});
}

// Short label for a tray chip — step number + its first ingredient as a hint.
function trayLabel(step, i) {
  const ing = (step.ingredients && step.ingredients[0]) ? ' · ' + escapeHtml(step.ingredients[0]) : '';
  return `Шаг ${i + 1}${ing}`;
}

// Tray of every live timer (running / paused / finished-not-dismissed).
export function trayHtml(s) {
  const map = timers(s);
  const idxs = Object.keys(map).map(Number).filter(i => map[i]).sort((a, b) => a - b);
  if (!idxs.length) return '';
  return `<div class="cook-tray">${idxs.map(i => {
    const t = map[i];
    const cls = t.done ? 'is-done' : (t.intervalId ? 'is-run' : 'is-pause');
    const cur = i === s.idx ? ' is-current' : ''; // the timer shown in the big ring
    return `<button type="button" class="ctray ${cls}${cur}" id="cook-tray-${i}" data-tray="${i}">
      <span class="ctray-label">${trayLabel(s.steps[i], i)}</span>
      <span class="ctray-time">${fmt(t.remaining)}</span>
      <span class="ctray-x" data-trayx="${i}" title="Убрать таймер">×</span>
    </button>`;
  }).join('')}</div>`;
}

export function cancelTimer(s, idx, onChange) {
  const t = timers(s)[idx];
  if (!t) return;
  if (t.intervalId) clearInterval(t.intervalId);
  delete timers(s)[idx];
  onChange?.();
}

export function clearAllTimers(s) {
  for (const k of Object.keys(s.timers || {})) {
    const t = s.timers[k];
    if (t && t.intervalId) clearInterval(t.intervalId);
  }
  s.timers = {};
}
