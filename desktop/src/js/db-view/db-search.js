// ── db-view/db-search.js — Quick search bar for table view ──

import { S } from '../state.js';

/** Render a search input in the filter bar area */
export function renderSearchBar(container, tabId, onSearch) {
  let bar = container.querySelector('.dbv-filter-bar');
  if (!bar) {
    bar = document.createElement('div');
    bar.className = 'dbv-filter-bar';
    container.prepend(bar);
  }

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'dbv-search-input';
  input.placeholder = 'Search...';
  input.value = S._dbvSearch?.[tabId] || '';
  bar.prepend(input);

  let timer;
  input.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (!S._dbvSearch) S._dbvSearch = {};
      S._dbvSearch[tabId] = input.value;
      if (onSearch) onSearch();
    }, 200);
  });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { input.value = ''; if (!S._dbvSearch) S._dbvSearch = {}; S._dbvSearch[tabId] = ''; if (onSearch) onSearch(); }
  });
}

/** Filter records by search query across all visible values */
export function applySearch(records, valuesMap, idField, tabId, fixedColumns) {
  const q = (S._dbvSearch?.[tabId] || '').toLowerCase().trim();
  if (!q) return records;
  return records.filter(r => {
    const rid = r[idField];
    // Check fixed columns
    for (const c of fixedColumns) {
      const val = String(r[c.key] ?? '').toLowerCase();
      if (val.includes(q)) return true;
    }
    // Check custom property values
    const vals = valuesMap[rid];
    if (vals) {
      for (const v of Object.values(vals)) {
        if (String(v).toLowerCase().includes(q)) return true;
      }
    }
    return false;
  });
}
