// ── food-cook-helpers.js — pure helpers for the cooking mode ──
// Ingredient-list rendering (mise en place / all-ingredients sheet), the
// step-complete chime, and the "cooking started" calendar event. Kept free of
// session state so food-cook-mode.js stays lean.
import { escapeHtml } from './utils.js';
import { invoke } from './state.js';

// Full recipe ingredients — items with amounts, else legacy "name: amount" string.
export function recipeIngredients(recipe) {
  const items = recipe.ingredient_items || [];
  if (items.length) return items.map(i => ({ name: i.name, amt: i.amount ? `${i.amount}${i.unit || ''}` : '' }));
  return String(recipe.ingredients || '').split(',').map(s => s.trim()).filter(Boolean)
    .map(p => { const c = p.split(':'); return { name: (c[0] || p).trim(), amt: (c[1] || '').trim() }; });
}

export function ingredientListHtml(recipe) {
  const list = recipeIngredients(recipe);
  if (!list.length) return '<div class="cook-no-timer">Ингредиенты не указаны</div>';
  return `<div class="cook-prep-list">${list.map(i =>
    `<div class="cook-prep-item"><span>${escapeHtml(i.name)}</span>${i.amt ? `<span class="cook-prep-amt">${escapeHtml(i.amt)}</span>` : ''}</div>`).join('')}</div>`;
}

export function headHtml(recipeName, label) {
  return `<div class="cook-head">
    <div class="cook-title">${escapeHtml(recipeName)}</div>
    <div class="cook-progress-label">${label}</div>
    <button class="cook-close" id="cook-close" title="Выйти">✕ Выйти</button>
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
