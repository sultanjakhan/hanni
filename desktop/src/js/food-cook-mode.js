// ── food-cook-mode.js — Guided cooking: prep → step-by-step timers → log ──
// Opened from the recipe "🍳 Готовить" button (or "Что приготовить"). Starts with a
// "Подготовка" screen listing all ingredients (mise en place), then walks each step
// with a manual-start ring countdown (+1 мин / pause). "Все ингредиенты" reopens the
// full list on any step; finishing opens the cooking log (rating + fridge deduction).
// Modal card over the dimmed tab.
import { escapeHtml } from './utils.js';
import { invoke } from './state.js';
import { parseSteps } from './food-recipe-modals.js';
import { ingredientListHtml, chime, createCookEvent, headHtml, dotsHtml } from './food-cook-helpers.js';

const RING_R = 54;
const CIRC = 2 * Math.PI * RING_R; // 339.29 — matches .cook-ring-progress dasharray
let session = null; // one active cook at a time

export function startCookMode(recipe, opts = {}) {
  const steps = parseSteps(recipe.instructions || '');
  if (!steps.length) {
    import('./food-cooking-log.js').then(m => m.showCookingLogModal(opts.date ?? null, opts.onSaved, recipe));
    return;
  }
  closeCookMode();
  session = {
    recipe, steps, idx: 0, phase: 'prep', onSaved: opts.onSaved, date: opts.date ?? null,
    eventId: null, eventPromise: null, intervalId: null, remaining: 0, total: 0, running: false, started: false,
  };
  const root = document.createElement('div');
  root.className = 'cook-root';
  root.addEventListener('click', (e) => { if (e.target === root) requestExit(); });
  document.body.appendChild(root);
  session.root = root;
  renderPrep();
}

export function closeCookMode() {
  if (!session) return;
  if (session.intervalId) clearInterval(session.intervalId);
  session.root?.remove();
  session = null;
}

const fmt = (sec) => `${String(Math.floor(sec / 60)).padStart(2, '0')}:${String(sec % 60).padStart(2, '0')}`;
const q = (s) => session.root.querySelector(s);

function renderPrep() {
  session.root.innerHTML = `<div class="cook-card">
    ${headHtml(session.recipe.name, 'Подготовка · достань всё')}
    ${ingredientListHtml(session.recipe)}
    <div class="cook-controls"><button class="btn-primary cook-btn cook-btn-next" id="cook-begin">Начать готовить →</button></div>
    <div class="cook-dots">${dotsHtml(session.steps, session.phase, session.idx)}</div>
  </div>`;
  q('#cook-close').onclick = requestExit;
  q('#cook-begin').onclick = () => {
    session.phase = 'cook';
    loadStep(0);
    // Record the cook in the calendar at the start (enriched on finish, removed on cancel).
    session.eventPromise = createCookEvent(session.recipe, session.steps, session.date).then(id => {
      if (session) { session.eventId = id; window.dispatchEvent(new CustomEvent('hanni:calendar-refresh')); }
      return id;
    });
  };
}

function loadStep(idx) {
  if (session.intervalId) { clearInterval(session.intervalId); session.intervalId = null; }
  session.idx = idx;
  session.total = (session.steps[idx].min || 0) * 60;
  session.remaining = session.total;
  session.running = false;
  session.started = false;
  renderStep();
}

function renderStep() {
  const { steps, idx } = session;
  const step = steps[idx];
  const last = idx === steps.length - 1;
  const hasTimer = (step.min || 0) > 0;

  const stage = hasTimer ? `
    <div class="cook-ring-wrap">
      <svg class="cook-ring" viewBox="0 0 120 120">
        <circle class="cook-ring-bg" cx="60" cy="60" r="${RING_R}" />
        <circle class="cook-ring-progress" id="cook-ring" cx="60" cy="60" r="${RING_R}" />
      </svg>
      <div class="cook-ring-inner"><div class="cook-time" id="cook-time">${fmt(session.remaining)}</div></div>
    </div>` : '<div class="cook-no-timer">Без таймера</div>';

  const ingr = step.ingredients.length
    ? `<div class="cook-ingredients">${step.ingredients.map(n => `<span class="cook-ingr">${escapeHtml(n)}</span>`).join('')}</div>` : '';

  const timerBtns = hasTimer ? `
    <button class="btn-secondary cook-btn" id="cook-addtime">+1 мин</button>
    <button class="btn-secondary cook-btn" id="cook-startpause">▶ Старт</button>` : '';

  session.root.innerHTML = `<div class="cook-card">
    ${headHtml(session.recipe.name, `Шаг ${idx + 1} из ${steps.length}`)}
    <div class="cook-stage">${stage}</div>
    <div class="cook-step-text">${escapeHtml(step.text)}</div>
    ${ingr}
    <button class="cook-allingr" id="cook-allingr">🧺 Все ингредиенты</button>
    <div class="cook-controls">
      ${idx > 0 ? '<button class="btn-secondary cook-btn cook-btn-back" id="cook-back">←</button>' : ''}
      ${timerBtns}
      <button class="btn-primary cook-btn cook-btn-next" id="cook-next">${last ? 'Готово ✓' : 'Дальше →'}</button>
    </div>
    <div class="cook-dots">${dotsHtml(steps, session.phase, idx)}</div>
  </div>`;

  q('#cook-close').onclick = requestExit;
  q('#cook-allingr').onclick = showSheet;
  q('#cook-next').onclick = advance;
  if (idx > 0) q('#cook-back').onclick = () => loadStep(idx - 1);
  if (hasTimer) {
    q('#cook-addtime').onclick = addTime;
    q('#cook-startpause').onclick = toggleTimer;
    paintRing();
  }
}

// Slide-over with the full ingredient list, reachable from any step.
function showSheet() {
  if (q('.cook-sheet-wrap')) return;
  const w = document.createElement('div');
  w.className = 'cook-sheet-wrap';
  w.innerHTML = `<div class="cook-sheet">
    <div class="cook-sheet-head"><span>Все ингредиенты</span><button class="cook-close" id="cook-sheet-close" title="Закрыть">✕</button></div>
    ${ingredientListHtml(session.recipe)}
  </div>`;
  session.root.appendChild(w);
  const close = () => w.remove();
  w.querySelector('#cook-sheet-close').onclick = close;
  w.addEventListener('click', (e) => { if (e.target === w) close(); });
}

// From the prep screen nothing is recorded yet → leave at once. Once cooking has
// started (event in the calendar) → ask: stay / keep the record / cancel it.
function requestExit() {
  if (!session) return;
  if (session.phase === 'prep') { closeCookMode(); return; }
  if (q('.cook-confirm')) return;
  const w = document.createElement('div');
  w.className = 'cook-sheet-wrap cook-confirm';
  w.innerHTML = `<div class="cook-sheet">
    <div class="cook-confirm-text">Выйти из готовки?</div>
    <div class="cook-exit-actions">
      <button class="btn-secondary cook-btn" id="cook-stay">Остаться</button>
      <button class="btn-secondary cook-btn" id="cook-keep">Выйти, оставить запись</button>
      <button class="btn-danger cook-btn" id="cook-cancel">Отменить готовку</button>
    </div></div>`;
  session.root.appendChild(w);
  w.querySelector('#cook-stay').onclick = () => w.remove();
  w.querySelector('#cook-keep').onclick = closeCookMode;
  w.querySelector('#cook-cancel').onclick = cancelCook;
}

// Close and remove the calendar event (awaits its creation in case it's in flight).
function cancelCook() {
  const p = session.eventPromise;
  closeCookMode();
  Promise.resolve(p).then(id => id && invoke('delete_event', { id })).catch(() => {})
    .then(() => window.dispatchEvent(new CustomEvent('hanni:calendar-refresh')));
}

function paintRing() {
  const ring = q('#cook-ring');
  if (!ring) return;
  const frac = session.total ? session.remaining / session.total : 1;
  ring.style.strokeDasharray = CIRC;
  ring.style.strokeDashoffset = CIRC * frac; // full offset = empty; 0 = full ring
}

function tick() {
  session.remaining = Math.max(0, session.remaining - 1);
  const t = q('#cook-time');
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
  const btn = q('#cook-startpause');
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
  const t = q('#cook-time');
  if (t) t.textContent = fmt(session.remaining);
  paintRing();
}

function onTimerDone() {
  q('.cook-card')?.classList.add('cook-ringdone');
  const btn = q('#cook-startpause');
  if (btn) btn.textContent = '▶ Старт';
  chime();
  invoke('send_notification', { title: '🍳 Готовка', body: `Шаг ${session.idx + 1} готов` }).catch(() => {});
}

function advance() {
  if (session.idx < session.steps.length - 1) {
    loadStep(session.idx + 1);
  } else {
    const { recipe, onSaved, date, eventId } = session;
    closeCookMode();
    import('./food-cooking-log.js').then(m => m.showCookingLogModal(date ?? null, onSaved, recipe, eventId));
  }
}
