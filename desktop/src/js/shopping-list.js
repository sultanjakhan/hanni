// shopping-list.js — Thin invoke wrappers + format helpers for the
// "Купить в магазине" list backing the 🛒 Закупка event template.

import { invoke } from './state.js';

export async function listShoppingItems(includeBought = false) {
  return await invoke('list_shopping_items', { includeBought }).catch(() => []);
}

export async function addShoppingItem(name, qty = '', note = '') {
  return await invoke('add_shopping_item', { name, qty, note });
}

export async function markBought(ids) {
  if (!Array.isArray(ids) || !ids.length) return 0;
  return await invoke('mark_shopping_bought', { ids });
}

export async function deleteShoppingItem(id) {
  return await invoke('delete_shopping_item', { id: Number(id) });
}

// Gather the ingredients of the given recipes that aren't already in the
// fridge and push them onto the shopping list. Mirrors the per-recipe
// "+ в покупки" logic in cook-what, but for a whole day's meal plan.
// Match is by name only (same as cook-what); dedupes across recipes.
const slNorm = (s) => String(s == null ? '' : s).trim().toLowerCase();
const slIngredients = (str) => String(str || '').split(/[,;\n]/).map(s => s.trim()).filter(Boolean);
const slName = (n) => n.split(':')[0].trim(); // "соль: 2ч.л." → "соль"

export async function addMissingForRecipes(recipeIds) {
  const ids = [...new Set((recipeIds || []).map(Number).filter(Boolean))];
  if (!ids.length) return 0;
  const [recipes, products] = await Promise.all([
    invoke('get_recipes', { search: null }).catch(() => []),
    invoke('get_products', {}).catch(() => []),
  ]);
  const byId = new Map(recipes.map(r => [r.id, r]));
  const have = new Set(products.map(p => slNorm(p.name)));
  const seen = new Set();
  let added = 0;
  for (const id of ids) {
    const r = byId.get(id);
    if (!r) continue;
    for (const raw of slIngredients(r.ingredients)) {
      const name = slName(raw);
      const key = slNorm(name);
      if (!name || have.has(key) || seen.has(key)) continue;
      seen.add(key);
      const c = raw.split(':');
      await addShoppingItem(name, (c[1] || '').trim(), '').catch(() => {});
      added++;
    }
  }
  return added;
}

// Render selected items into the description field of a "Закупка" event —
// one per line so editing in calendar stays readable.
export function itemsToDescription(items) {
  return items.map(i => {
    const qty = i.qty ? ` — ${i.qty}` : '';
    const note = i.note ? ` (${i.note})` : '';
    return `• ${i.name}${qty}${note}`;
  }).join('\n');
}
