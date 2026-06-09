// ── food-cook-mode.js — Guided cooking: prep → step-by-step timers → log ──
// Opened from the recipe "🍳 Готовить" button (or "Что приготовить"). Starts with a
// "Подготовка" screen listing all ingredients (mise en place), then walks each step
// with a manual-start ring countdown (+1 мин / pause). "Все ингредиенты" reopens the
// full list on any step; finishing opens the cooking log (rating + fridge deduction).
// Modal card over the dimmed tab.
import { escapeHtml } from './utils.js';
import { invoke } from './state.js';
import { parseSteps } from './food-recipe-modals.js';
import { ingredientListHtml, createCookEvent, headHtml, dotsHtml, bindPrepChecklist, recipeIngredients, requestWakeLock, releaseWakeLock, saveCook, loadCook, clearCook } from './food-cook-helpers.js';
import { loadCatalog } from './food-recipe-filters.js';
import { RING_R, fmt, paintRing, toggleTimer, addTime } from './food-cook-timer.js';

let session = null; // one active cook at a time

export function startCookMode(recipe, opts = {}) {
  const steps = parseSteps(recipe.instructions || '');
  if (!steps.length) {
    import('./food-cooking-log.js').then(m => m.showCookingLogModal(opts.date ?? null, opts.onSaved, recipe));
    return;
  }
  closeCookMode();
  const saved = loadCook(recipe.id);
  session = {
    recipe, steps, idx: 0, phase: 'prep', onSaved: opts.onSaved, date: opts.date ?? null,
    baseServ: recipe.servings || 1, servings: saved?.servings || recipe.servings || 1,
    checked: new Set(), eventId: saved?.eventId ?? null, eventPromise: null,
    intervalId: null, remaining: 0, total: 0, running: false, started: false,
  };
  const root = document.createElement('div');
  root.className = 'cook-root';
  root.addEventListener('click', (e) => { if (e.target === root) requestExit(); });
  document.body.appendChild(root);
  session.root = root;
  requestWakeLock();
  session.onVis = () => { if (document.visibilityState === 'visible') requestWakeLock(); };
  document.addEventListener('visibilitychange', session.onVis);
  // Resume the saved cook (same recipe) at its step; else start at prep.
  if (saved && saved.idx < steps.length) { session.phase = 'cook'; loadStep(saved.idx); }
  else renderPrep();
  loadCatalog().then(() => { if (session && session.phase === 'prep') renderPrep(); }).catch(() => {});
}

export function closeCookMode() {
  if (!session) return;
  if (session.intervalId) clearInterval(session.intervalId);
  if (session.onVis) document.removeEventListener('visibilitychange', session.onVis);
  releaseWakeLock();
  session.root?.remove();
  session = null;
}

const q = (s) => session.root.querySelector(s);

function renderPrep() {
  const total = recipeIngredients(session.recipe).length;
  const ratio = session.servings / session.baseServ;
  session.root.innerHTML = `<div class="cook-card">
    ${headHtml(session.recipe.name, 'Подготовка · достань всё')}
    <div class="cook-prep-head">
      <span class="cook-prep-count" id="cook-prep-count">Собрано ${session.checked.size} из ${total}</span>
      <span class="cook-prep-serv">Порций <button class="cook-serv-btn" data-d="-1">−</button><b id="cook-serv">${session.servings}</b><button class="cook-serv-btn" data-d="1">+</button></span>
    </div>
    ${ingredientListHtml(session.recipe, session.checked, ratio)}
    <div class="cook-controls"><button class="btn-primary cook-btn cook-btn-next" id="cook-begin">Начать готовить →</button></div>
    <div class="cook-dots">${dotsHtml(session.steps, session.phase, session.idx)}</div>
  </div>`;
  q('#cook-close').onclick = requestExit;
  q('#cook-begin').onclick = () => {
    session.phase = 'cook';
    loadStep(0);
    // Record the cook in the calendar at the start (enriched on finish, removed on cancel).
    session.eventPromise = createCookEvent(session.recipe, session.steps, session.date).then(id => {
      if (session) { session.eventId = id; saveCook(session); window.dispatchEvent(new CustomEvent('hanni:calendar-refresh')); }
      return id;
    });
  };
  session.root.querySelectorAll('.cook-serv-btn').forEach(b => b.onclick = (e) => {
    e.stopPropagation();
    const n = session.servings + Number(b.dataset.d);
    if (n >= 1 && n <= 40) { session.servings = n; renderPrep(); }
  });
  bindPrepChecklist(session.root, session.checked, total);
}

function loadStep(idx) {
  if (session.intervalId) { clearInterval(session.intervalId); session.intervalId = null; }
  session.idx = idx;
  session.total = (session.steps[idx].min || 0) * 60;
  session.remaining = session.total;
  session.running = false;
  session.started = false;
  saveCook(session); // persist position for resume
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
    q('#cook-addtime').onclick = () => addTime(session);
    q('#cook-startpause').onclick = () => toggleTimer(session);
    paintRing(session);
  }
}

// Slide-over with the full ingredient list, reachable from any step.
function showSheet() {
  if (q('.cook-sheet-wrap')) return;
  const w = document.createElement('div');
  w.className = 'cook-sheet-wrap';
  w.innerHTML = `<div class="cook-sheet">
    <div class="cook-sheet-head"><span>Все ингредиенты</span><button class="cook-close" id="cook-sheet-close" title="Закрыть">✕</button></div>
    ${ingredientListHtml(session.recipe, new Set(), session.servings / session.baseServ)}
  </div>`;
  session.root.appendChild(w);
  const close = () => w.remove();
  w.querySelector('#cook-sheet-close').onclick = close;
  w.addEventListener('click', (e) => { if (e.target === w) close(); });
}

// Prep: nothing recorded yet → leave at once. Cooking started → stay / keep / cancel.
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

function cancelCook() {
  const p = session.eventPromise;
  clearCook();
  closeCookMode();
  Promise.resolve(p).then(id => id && invoke('delete_event', { id })).catch(() => {})
    .then(() => window.dispatchEvent(new CustomEvent('hanni:calendar-refresh')));
}

function advance() {
  if (session.idx < session.steps.length - 1) {
    loadStep(session.idx + 1);
  } else {
    const { recipe, onSaved, date, eventId } = session;
    clearCook();
    closeCookMode();
    import('./food-cooking-log.js').then(m => m.showCookingLogModal(date ?? null, onSaved, recipe, eventId));
  }
}
