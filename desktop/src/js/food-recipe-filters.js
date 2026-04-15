// ── food-recipe-filters.js — Filter logic and data for recipes ──
import { invoke } from './state.js';
import { ingrCat } from './utils.js';
import { getIngrNames } from './food-recipe-card.js';

export const MEALS = [
  { id: 'all', label: 'Все' }, { id: 'breakfast', label: 'Завтрак' },
  { id: 'lunch', label: 'Обед' }, { id: 'dinner', label: 'Ужин' },
];
// Cuisines loaded dynamically from DB
let _cuisineCache = null;
export async function loadCuisines() {
  if (_cuisineCache) return _cuisineCache;
  try {
    const rows = await invoke('get_cuisines');
    _cuisineCache = rows.map(r => ({ id: r.code, label: `${r.emoji} ${r.name}`, emoji: r.emoji, name: r.name }));
  } catch { _cuisineCache = [{ id: 'other', label: '🌍 Другая', emoji: '🌍', name: 'Другая' }]; }
  return _cuisineCache;
}
export function invalidateCuisineCache() { _cuisineCache = null; }
export async function getCuisineChips() {
  const list = await loadCuisines();
  return [{ id: 'all', label: 'Все' }, ...list.map(c => ({ id: c.id, label: c.emoji }))];
}
export const DIFFS = [
  { id: 'all', label: 'Любая' }, { id: 'easy', label: 'Лёгкий' },
  { id: 'medium', label: 'Средний' }, { id: 'hard', label: 'Сложный' },
];
export const CAT_LABELS = { meat: 'Мясо', grain: 'Крупы', veg: 'Овощи', dairy: 'Молочные', fruit: 'Фрукты', spice: 'Специи', oil: 'Масла' };
export const CAT_ORDER = ['meat', 'grain', 'veg', 'dairy', 'fruit', 'spice', 'oil'];

export async function getBlacklist() {
  try {
    const entries = await invoke('memory_list', { category: 'food', limit: 100 });
    const items = [];
    for (const e of entries) {
      const k = e.key.toLowerCase();
      if (k.includes('блэклист') || k.includes('blacklist') || k.includes('аллергия') || k.includes('allergy'))
        items.push(...e.value.split(',').map(s => s.trim().toLowerCase()).filter(Boolean));
    }
    return items;
  } catch { return []; }
}

export const matchBL = (r, bl) => bl.length && bl.some(i => `${r.name} ${r.ingredients || ''}`.toLowerCase().includes(i));
export const matchMeal = (r, f) => f === 'all' || (r.tags || '').split(',').map(t => t.trim()).includes(f);
export const matchCuisine = (r, f) => f === 'all' || (r.cuisine || 'other') === f;
export const matchDiff = (r, f) => f === 'all' || (r.difficulty || 'easy') === f;
export const matchSearch = (r, q) => !q || `${r.name} ${r.ingredients || ''}`.toLowerCase().includes(q);
export function matchIngr(r, sel) {
  if (!sel.size) return true;
  const names = getIngrNames(r);
  return [...sel].every(ingr => names.some(n => n.toLowerCase() === ingr));
}

export function sortRecipes(arr, key) {
  const cmp = { calories: (a, b) => (a.calories || 0) - (b.calories || 0),
    health: (a, b) => (b.health_score || 5) - (a.health_score || 5),
    price: (a, b) => (a.price_score || 5) - (b.price_score || 5),
    name: (a, b) => (a.name || '').localeCompare(b.name || '') };
  return cmp[key] ? [...arr].sort(cmp[key]) : arr;
}

export function collectIngredients(recipes) {
  const map = {};
  for (const r of recipes) {
    for (const n of getIngrNames(r)) {
      const lc = n.toLowerCase(), cat = ingrCat(n) || 'other';
      if (!map[lc]) map[lc] = { name: n, cat, count: 0 };
      map[lc].count++;
    }
  }
  const grouped = {};
  for (const item of Object.values(map)) {
    (grouped[item.cat] ||= []).push(item);
  }
  for (const cat of Object.keys(grouped)) grouped[cat].sort((a, b) => b.count - a.count);
  return grouped;
}
