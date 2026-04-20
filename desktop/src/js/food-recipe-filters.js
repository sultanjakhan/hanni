// ── food-recipe-filters.js — Filter logic and data for recipes ──
import { invoke } from './state.js';
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
export const CAT_LABELS = { meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты', grain: 'Крупы', dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи', spice: 'Специи', oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', other: 'Другое' };
export const CAT_ORDER = ['meat', 'fish', 'veg', 'fruit', 'grain', 'dairy', 'legumes', 'nuts', 'spice', 'oil', 'bakery', 'drinks', 'other'];

// Ingredient catalog cache — lookup category by name from DB
let _catalogCache = null;
export async function loadCatalog() {
  if (_catalogCache) return _catalogCache;
  try { _catalogCache = await invoke('get_ingredient_catalog'); } catch { _catalogCache = []; }
  return _catalogCache;
}
export function getCatalogCache() { return _catalogCache || []; }
export function invalidateCatalogCache() { _catalogCache = null; }
export function catalogCat(name) {
  if (!_catalogCache) return '';
  const lc = name.toLowerCase();
  const item = _catalogCache.find(c => c.name.toLowerCase() === lc);
  return item ? item.category : '';
}

let _blacklistCache = null;
export async function getBlacklist() {
  if (_blacklistCache) return _blacklistCache;
  try { _blacklistCache = await invoke('list_food_blacklist'); } catch { _blacklistCache = []; }
  return _blacklistCache;
}
export function invalidateBlacklistCache() { _blacklistCache = null; }

function catalogByName(name) {
  if (!_catalogCache) return null;
  const lc = name.toLowerCase();
  return _catalogCache.find(c => c.name.toLowerCase() === lc) || null;
}
function catalogNamesForTag(tag) {
  if (!_catalogCache) return [];
  const lc = tag.toLowerCase();
  return _catalogCache
    .filter(c => (c.tags || '').split(',').map(t => t.trim().toLowerCase()).includes(lc))
    .map(c => c.name.toLowerCase());
}

export function isIngredientBlocked(name, bl) {
  if (!bl || !bl.length || !name) return false;
  const lc = name.toLowerCase();
  const cat = catalogByName(name);
  const catTags = cat ? (cat.tags || '').split(',').map(t => t.trim().toLowerCase()).filter(Boolean) : [];
  const catCategory = cat ? cat.category : null;
  const catSubgroup = cat ? (cat.subgroup || '').toLowerCase() : '';
  for (const e of bl) {
    const v = (e.value || '').toLowerCase();
    if (!v) continue;
    if (e.type === 'keyword' && (lc.includes(v) || catSubgroup.includes(v) || catTags.some(t => t.includes(v)))) return true;
    if (e.type === 'product' && lc === v) return true;
    if (e.type === 'tag' && (catTags.includes(v) || catSubgroup === v)) return true;
    if (e.type === 'category' && catCategory === v) return true;
  }
  return false;
}

export function isTagBlocked(tag, bl) {
  if (!bl || !bl.length || !tag) return false;
  const lc = tag.toLowerCase();
  return bl.some(e => {
    const v = (e.value || '').toLowerCase();
    return v && ((e.type === 'tag' && v === lc) || (e.type === 'keyword' && lc.includes(v)));
  });
}

export function isCategoryBlocked(cat, bl) {
  if (!bl || !bl.length || !cat) return false;
  const lc = cat.toLowerCase();
  return bl.some(e => e.type === 'category' && (e.value || '').toLowerCase() === lc);
}

export function matchBL(r, bl) {
  if (!bl || !bl.length) return false;
  const hay = `${r.name} ${r.ingredients || ''}`.toLowerCase();
  const names = getIngrNames(r);
  for (const e of bl) {
    const v = (e.value || '').toLowerCase();
    if (!v) continue;
    if (e.type === 'keyword' && hay.includes(v)) return true;
    if (e.type === 'product' && names.some(n => n.toLowerCase() === v)) return true;
    if (e.type === 'tag') {
      const blocked = catalogNamesForTag(v);
      if (blocked.some(bn => names.some(n => n.toLowerCase() === bn))) return true;
    }
    if (e.type === 'category' && names.some(n => (catalogByName(n)?.category || '') === v)) return true;
  }
  return false;
}
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
  const cmp = { calories: (a, b) => (a.calories || 0) - (b.calories || 0), health: (a, b) => (b.health_score || 5) - (a.health_score || 5), price: (a, b) => (a.price_score || 5) - (b.price_score || 5), name: (a, b) => (a.name || '').localeCompare(b.name || '') };
  return cmp[key] ? [...arr].sort(cmp[key]) : arr;
}

export function collectIngredients(recipes) {
  const map = {};
  for (const r of recipes) {
    for (const n of getIngrNames(r)) {
      const lc = n.toLowerCase(), cat = catalogCat(n) || 'other';
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
