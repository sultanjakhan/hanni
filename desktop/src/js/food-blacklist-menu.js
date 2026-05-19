// ── food-blacklist-menu.js — Quick add to food blacklist (context menu + hover) ──
import { invoke } from './state.js';
import { confirmModal } from './utils.js';
import { invalidateBlacklistCache } from './food-recipe-filters.js';

export const BL_LEVELS = [
  { level: 'hard', icon: '🚫', label: 'Не ем' },
  { level: 'soft', icon: '👎', label: 'Не люблю' },
];

// Add/switch a blacklist entry. hard → confirm + delete matching recipes; soft → silent.
// Returns true if applied. Re-adding an existing entry switches its level (upsert).
export async function applyBlacklist(type, value, level, catalogId, onChange) {
  if (level === 'hard') {
    const hits = await invoke('find_recipes_matching_blacklist', { entryType: type, value }).catch(() => []);
    const preview = hits.length ? `\n\nБудет удалено ${hits.length} рецептов.` : '';
    if (!await confirmModal(`«${value}» — не ем?${preview}`, 'Заблокировать')) return false;
    await invoke('add_food_blacklist', { entryType: type, value, level, catalogId: catalogId ?? null }).catch(() => {});
    if (hits.length) await invoke('delete_recipes_matching_blacklist', { entryType: type, value }).catch(() => {});
  } else {
    await invoke('add_food_blacklist', { entryType: type, value, level, catalogId: catalogId ?? null }).catch(() => {});
  }
  invalidateBlacklistCache();
  if (onChange) onChange();
  return true;
}

let _menu = null;
export function closeBlacklistMenu() { if (_menu) { _menu.remove(); _menu = null; } }

// target: { type: 'product'|'category'|'tag'|'keyword', value, catalogId? }
export function showBlacklistContextMenu(x, y, target, onChange) {
  closeBlacklistMenu();
  const menu = document.createElement('div');
  menu.className = 'inline-dropdown blacklist-menu';
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  const head = document.createElement('div');
  head.className = 'blacklist-menu-head';
  head.textContent = target.value;
  menu.appendChild(head);

  for (const { level, icon, label } of BL_LEVELS) {
    const opt = document.createElement('div');
    opt.className = 'inline-dd-option';
    opt.innerHTML = `<span style="margin-right:6px;">${icon}</span>${label}`;
    opt.addEventListener('click', async () => {
      closeBlacklistMenu();
      await applyBlacklist(target.type, target.value, level, target.catalogId, onChange);
    });
    menu.appendChild(opt);
  }

  document.body.appendChild(menu);
  _menu = menu;
  const rect = menu.getBoundingClientRect();
  if (rect.right > window.innerWidth) menu.style.left = `${x - rect.width}px`;
  if (rect.bottom > window.innerHeight) menu.style.top = `${y - rect.height}px`;

  setTimeout(() => document.addEventListener('mousedown', function onOutside(ev) {
    if (_menu && !_menu.contains(ev.target)) { closeBlacklistMenu(); document.removeEventListener('mousedown', onOutside); }
  }), 10);
}
