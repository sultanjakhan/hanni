// ── food-cook-mode.js — Guided cooking mode: step-by-step with per-step timer ──
// Opened from the recipe detail "🍳 Готовить" button. Walks recipe steps one at a
// time; each step with a `min` duration gets a manual-start ring countdown with a
// "+1 мин" extra-time button. Finishing the last step opens the cooking log
// (rating + fridge deduction), closing the cook → consume loop. Shown as a modal
// card over the dimmed tab.
import { escapeHtml } from './utils.js';
import { invoke } from './state.js';
import { parseSteps } from './food-recipe-modals.js';

const RING_R = 54;
const CIRC = 2 * Math.PI * RING_R; // 339.29 — matches .cook-ring-progress dasharray

let session = null; // one active cook at a time

export function startCookMode(recipe, opts = {}) {
  const steps = parseSteps(recipe.instructions || '');
  if (!steps.length) {
    // Nothing to walk — go straight to logging the cook.
    import('./food-cooking-log.js').then(m => m.showCookingLogModal(opts.date ?? null, opts.onSaved, recipe));
    return;
  }
  closeCookMode();
  session = {
    recipe, steps, idx: 0, onSaved: opts.onSaved, date: opts.date ?? null,
    intervalId: null, remaining: 0, total: 0, running: false, started: false,
  };
  const root = document.createElement('div');
  root.className = 'cook-root';
  root.addEventListener('click', (e) => { if (e.target === root) closeCookMode(); });
  document.body.appendChild(root);
  session.root = root;
  loadStep(0);
}

export function closeCookMode() {
  if (!session) return;
  if (session.intervalId) clearInterval(session.intervalId);
  session.root?.remove();
  session = null;
}

function loadStep(idx) {
  if (session.intervalId) { clearInterval(session.intervalId); session.intervalId = null; }
  session.idx = idx;
  const step = session.steps[idx];
  session.total = (step.min || 0) * 60;
  session.remaining = session.total;
  session.running = false;
  session.started = false;
  render();
}

function fmt(sec) {
  const m = Math.floor(sec / 60), s = sec % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

function render() {
  const { steps, idx, recipe } = session;
  const step = steps[idx];
  const last = idx === steps.length - 1;
  const hasTimer = (step.min || 0) > 0;

  const ingredients = step.ingredients.length
    ? `<div class="cook-ingredients">${step.ingredients.map(n => `<span class="cook-ingr">${escapeHtml(n)}</span>`).join('')}</div>`
    : '';

  const stage = hasTimer ? `
    <div class="cook-ring-wrap">
      <svg class="cook-ring" viewBox="0 0 120 120">
        <circle class="cook-ring-bg" cx="60" cy="60" r="${RING_R}" />
        <circle class="cook-ring-progress" id="cook-ring" cx="60" cy="60" r="${RING_R}" />
      </svg>
      <div class="cook-ring-inner"><div class="cook-time" id="cook-time">${fmt(session.remaining)}</div></div>
    </div>` : `<div class="cook-no-timer">Без таймера</div>`;

  const dots = steps.map((_, i) =>
    `<span class="cook-dot${i === idx ? ' on' : ''}${i < idx ? ' done' : ''}"></span>`).join('');

  const timerBtns = hasTimer ? `
    <button class="btn-secondary cook-btn" id="cook-addtime">+1 мин</button>
    <button class="btn-secondary cook-btn" id="cook-startpause">▶ Старт</button>` : '';

  session.root.innerHTML = `<div class="cook-card">
    <div class="cook-head">
      <div class="cook-title">${escapeHtml(recipe.name)}</div>
      <div class="cook-progress-label">Шаг ${idx + 1} из ${steps.length}</div>
      <button class="cook-close" id="cook-close" title="Закрыть">✕</button>
    </div>
    <div class="cook-stage">${stage}</div>
    <div class="cook-step-text">${escapeHtml(step.text)}</div>
    ${ingredients}
    <div class="cook-controls">
      ${idx > 0 ? '<button class="btn-secondary cook-btn cook-btn-back" id="cook-back">←</button>' : ''}
      ${timerBtns}
      <button class="btn-primary cook-btn cook-btn-next" id="cook-next">${last ? 'Готово ✓' : 'Дальше →'}</button>
    </div>
    <div class="cook-dots">${dots}</div>
  </div>`;

  session.root.querySelector('#cook-close').onclick = closeCookMode;
  session.root.querySelector('#cook-next').onclick = advance;
  const back = session.root.querySelector('#cook-back');
  if (back) back.onclick = () => loadStep(idx - 1);
  if (hasTimer) {
    session.root.querySelector('#cook-addtime').onclick = addTime;
    session.root.querySelector('#cook-startpause').onclick = toggleTimer;
    paintRing();
  }
}

function paintRing() {
  const ring = session.root.querySelector('#cook-ring');
  if (!ring) return;
  const frac = session.total ? session.remaining / session.total : 1;
  ring.style.strokeDasharray = CIRC;
  ring.style.strokeDashoffset = CIRC * frac; // full offset = empty; 0 = full ring
}

function tick() {
  session.remaining = Math.max(0, session.remaining - 1);
  const t = session.root.querySelector('#cook-time');
  if (t) t.textContent = fmt(session.remaining);
  paintRing();
  if (session.remaining <= 0) {
    clearInterval(session.intervalId);
    session.intervalId = null;
    session.running = false;
    onTimerDone();
  }
}

function toggleTimer() {
  const btn = session.root.querySelector('#cook-startpause');
  if (session.running) {
    clearInterval(session.intervalId);
    session.intervalId = null;
    session.running = false;
    btn.textContent = '▶ Продолжить';
  } else {
    if (session.remaining <= 0) { session.remaining = session.total = 60; paintRing(); }
    session.running = true;
    session.started = true;
    btn.textContent = '⏸ Пауза';
    session.intervalId = setInterval(tick, 1000);
  }
}

function addTime() {
  session.remaining += 60;
  session.total = Math.max(session.total, session.remaining);
  const t = session.root.querySelector('#cook-time');
  if (t) t.textContent = fmt(session.remaining);
  paintRing();
}

function onTimerDone() {
  const card = session.root.querySelector('.cook-card');
  card?.classList.add('cook-ringdone');
  const btn = session.root.querySelector('#cook-startpause');
  if (btn) btn.textContent = '▶ Старт';
  beep();
  invoke('send_notification', { title: '🍳 Готовка', body: `Шаг ${session.idx + 1} готов` }).catch(() => {});
}

function advance() {
  if (session.idx < session.steps.length - 1) {
    loadStep(session.idx + 1);
  } else {
    const { recipe, onSaved, date } = session;
    closeCookMode();
    import('./food-cooking-log.js').then(m => m.showCookingLogModal(date ?? null, onSaved, recipe));
  }
}

// Short completion chime — Web Audio, no bundled asset.
function beep() {
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const o = ctx.createOscillator(), g = ctx.createGain();
    o.connect(g); g.connect(ctx.destination);
    o.type = 'sine'; o.frequency.value = 880;
    g.gain.setValueAtTime(0.0001, ctx.currentTime);
    g.gain.exponentialRampToValueAtTime(0.2, ctx.currentTime + 0.02);
    g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.6);
    o.start(); o.stop(ctx.currentTime + 0.62);
    setTimeout(() => ctx.close(), 800);
  } catch { /* audio unavailable — silent */ }
}
