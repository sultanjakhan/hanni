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

// Render selected items into the description field of a "Закупка" event —
// one per line so editing in calendar stays readable.
export function itemsToDescription(items) {
  return items.map(i => {
    const qty = i.qty ? ` — ${i.qty}` : '';
    const note = i.note ? ` (${i.note})` : '';
    return `• ${i.name}${qty}${note}`;
  }).join('\n');
}
