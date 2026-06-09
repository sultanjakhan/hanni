// ── food-cook-helpers.js — pure helpers for the cooking mode ──
// Ingredient-list rendering (mise en place / all-ingredients sheet), the
// step-complete chime, and the "cooking started" calendar event. Kept free of
// session state so food-cook-mode.js stays lean.
import { escapeHtml } from './utils.js';
import { invoke } from './state.js';
import { catalogCat } from './food-recipe-filters.js';

// Full recipe ingredients as {name, amount(number), unit} — items, else legacy
// "name: 500г" string (number + unit parsed out so servings can rescale them).
export function recipeIngredients(recipe) {
  const items = recipe.ingredient_items || [];
  if (items.length) return items.map(i => ({ name: i.name, amount: Number(i.amount) || 0, unit: i.unit || '' }));
  return String(recipe.ingredients || '').split(',').map(s => s.trim()).filter(Boolean).map(p => {
    const c = p.split(':'); const name = (c[0] || p).trim(); const rest = (c[1] || '').trim();
    const mm = rest.match(/^([\d.,]+)\s*(.*)$/);
    return { name, amount: mm ? parseFloat(mm[1].replace(',', '.')) || 0 : 0, unit: mm ? mm[2].trim() : rest };
  });
}

// Renders the ingredient list with a category colour dot, amount (scaled by
// `ratio` for servings), and a tap-to-check state. `checked` is a Set of indices;
// pass empty for the read-only "Все ингредиенты" sheet.
export function ingredientListHtml(recipe, checked = new Set(), ratio = 1) {
  const list = recipeIngredients(recipe);
  if (!list.length) return '<div class="cook-no-timer">Ингредиенты не указаны</div>';
  return `<div class="cook-prep-list">${list.map((it, i) => {
    const cat = catalogCat(it.name);
    const amt = it.amount ? `${Math.round(it.amount * ratio * 10) / 10}${it.unit}` : '';
    return `<div class="cook-prep-item${checked.has(i) ? ' checked' : ''}" data-i="${i}">
      <span class="cook-prep-dot${cat ? ' cat-' + cat : ''}"></span>
      <span class="cook-prep-name">${escapeHtml(it.name)}</span>
      ${amt ? `<span class="cook-prep-amt">${escapeHtml(amt)}</span>` : ''}</div>`;
  }).join('')}</div>`;
}

// Wire tap-to-check on the prep screen; updates the "Собрано N из M" counter.
export function bindPrepChecklist(root, checked, total) {
  root.querySelectorAll('.cook-prep-item').forEach(row => {
    row.onclick = () => {
      const i = Number(row.dataset.i);
      checked.has(i) ? checked.delete(i) : checked.add(i);
      row.classList.toggle('checked', checked.has(i));
      const c = root.querySelector('#cook-prep-count');
      if (c) c.textContent = `Собрано ${checked.size} из ${total}`;
    };
  });
}

export function headHtml(recipeName, label) {
  return `<div class="cook-head">
    <div class="cook-title">${escapeHtml(recipeName)}</div>
    <div class="cook-progress-label">${label}</div>
    <button class="cook-close" id="cook-close" title="Выйти">✕</button>
  </div>`;
}

export function dotsHtml(steps, phase, idx) {
  return steps.map((_, i) =>
    `<span class="cook-dot${phase === 'cook' && i === idx ? ' on' : ''}${phase === 'cook' && i < idx ? ' done' : ''}"></span>`).join('');
}

// Record a calendar event the moment cooking starts (date now, duration = sum of
// step minutes). Returns the new event id, or null on failure. The cooking log
// reuses this id on finish so the rating/note enrich the same event (no duplicate).
export async function createCookEvent(recipe, steps, date) {
  const d = date || new Date().toISOString().slice(0, 10);
  const now = new Date();
  const time = `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`;
  const duration = steps.reduce((s, x) => s + (x.min || 0), 0) || 30;
  let color = '#cb8a05';
  try { const cats = await invoke('list_event_categories'); color = cats.find(c => c.name === 'Готовка')?.color || color; } catch { /* default */ }
  return invoke('create_event', {
    title: recipe.name, description: '', date: d, time,
    durationMinutes: duration, category: 'Готовка', color, priority: null,
  }).catch(() => null);
}

// Resume support — persist the active cook (one slot) so leaving and returning to
// the same recipe continues from the same step. Cleared on finish/cancel.
const COOK_KEY = 'hanni_cook_session';
export function saveCook(s) {
  try { localStorage.setItem(COOK_KEY, JSON.stringify({ recipeId: s.recipe.id, idx: s.idx, servings: s.servings, eventId: s.eventId })); } catch { /* quota */ }
}
export function loadCook(recipeId) {
  try { const d = JSON.parse(localStorage.getItem(COOK_KEY) || 'null'); return d && d.recipeId === recipeId ? d : null; } catch { return null; }
}
export function clearCook() { try { localStorage.removeItem(COOK_KEY); } catch { /* ignore */ } }

// Keep the screen awake while cooking (Screen Wake Lock; no-op if unsupported).
let _wakeLock = null;
export async function requestWakeLock() {
  try { if ('wakeLock' in navigator) _wakeLock = await navigator.wakeLock.request('screen'); }
  catch { /* unsupported / denied — silent */ }
}
export function releaseWakeLock() {
  try { _wakeLock?.release(); } catch { /* already gone */ }
  _wakeLock = null;
}

// Short completion chime — Web Audio, no bundled asset.
export function chime() {
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
