// ── food-cook-timer.js — per-step ring countdown for the cooking mode ──
// Operates on the cook `session` object (passed in) so food-cook-mode.js stays the
// thin controller. Manual start; "+1 мин" adds time; on zero → chime + notification.
import { chime } from './food-cook-helpers.js';
import { invoke } from './state.js';

export const RING_R = 54;
const CIRC = 2 * Math.PI * RING_R; // 339.29 — matches .cook-ring-progress dasharray
export const fmt = (sec) => `${String(Math.floor(sec / 60)).padStart(2, '0')}:${String(sec % 60).padStart(2, '0')}`;

export function paintRing(s) {
  const ring = s.root.querySelector('#cook-ring');
  if (!ring) return;
  const frac = s.total ? s.remaining / s.total : 1;
  ring.style.strokeDasharray = CIRC;
  ring.style.strokeDashoffset = CIRC * frac; // full offset = empty; 0 = full ring
}

export function toggleTimer(s) {
  const btn = s.root.querySelector('#cook-startpause');
  if (s.running) {
    clearInterval(s.intervalId); s.intervalId = null; s.running = false;
    btn.textContent = '▶ Продолжить';
  } else {
    if (s.remaining <= 0) { s.remaining = s.total = 60; paintRing(s); }
    s.running = true; s.started = true; btn.textContent = '⏸ Пауза';
    s.intervalId = setInterval(() => tick(s), 1000);
  }
}

function tick(s) {
  s.remaining = Math.max(0, s.remaining - 1);
  const t = s.root.querySelector('#cook-time');
  if (t) t.textContent = fmt(s.remaining);
  paintRing(s);
  if (s.remaining <= 0) {
    clearInterval(s.intervalId); s.intervalId = null; s.running = false;
    onTimerDone(s);
  }
}

export function addTime(s) {
  s.remaining += 60;
  s.total = Math.max(s.total, s.remaining);
  const t = s.root.querySelector('#cook-time');
  if (t) t.textContent = fmt(s.remaining);
  paintRing(s);
}

function onTimerDone(s) {
  s.root.querySelector('.cook-card')?.classList.add('cook-ringdone');
  const btn = s.root.querySelector('#cook-startpause');
  if (btn) btn.textContent = '▶ Старт';
  chime();
  invoke('send_notification', { title: '🍳 Готовка', body: `Шаг ${s.idx + 1} готов` }).catch(() => {});
}
