// ── db-view/db-col-state.js — Column state persistence (localStorage) ──

// ── Hidden fixed columns ──

export function getHiddenFixedCols(tabId) {
  try { return JSON.parse(localStorage.getItem(`dbv_hidden_fixed_${tabId}`) || '[]'); } catch { return []; }
}

export function setHiddenFixedCols(tabId, keys) {
  localStorage.setItem(`dbv_hidden_fixed_${tabId}`, JSON.stringify(keys));
}

// ── Fixed column custom display names ──

export function getFixedColName(tabId, key, fallback) {
  try {
    const names = JSON.parse(localStorage.getItem(`dbv_fixed_names_${tabId}`) || '{}');
    return names[key] || fallback;
  } catch { return fallback; }
}

export function setFixedColName(tabId, key, name) {
  let names = {};
  try { names = JSON.parse(localStorage.getItem(`dbv_fixed_names_${tabId}`) || '{}'); } catch {}
  if (name) names[key] = name; else delete names[key];
  localStorage.setItem(`dbv_fixed_names_${tabId}`, JSON.stringify(names));
}

// ── Unified column order (fixed keys + "prop_ID") ──

export function getColumnOrder(tabId) {
  try { return JSON.parse(localStorage.getItem(`dbv_col_order_${tabId}`) || '[]'); } catch { return []; }
}

export function setColumnOrder(tabId, order) {
  localStorage.setItem(`dbv_col_order_${tabId}`, JSON.stringify(order));
}

// ── Text wrap toggle (localStorage) ──

function getWrapState(tabId) {
  try { return JSON.parse(localStorage.getItem(`dbv_wrap_${tabId}`) || '{}'); } catch { return {}; }
}

export function isColumnWrapped(tabId, colId) {
  return !!getWrapState(tabId)[colId];
}

export function toggleWrap(tabId, colId) {
  const s = getWrapState(tabId);
  s[colId] = !s[colId];
  localStorage.setItem(`dbv_wrap_${tabId}`, JSON.stringify(s));
}

// ── Frozen columns ──

export function getFrozenCols(tabId) {
  try { return JSON.parse(localStorage.getItem(`dbv_frozen_${tabId}`) || '[]'); } catch { return []; }
}

export function toggleFreezeCol(tabId, colId) {
  const frozen = getFrozenCols(tabId);
  const idx = frozen.indexOf(colId);
  if (idx >= 0) frozen.splice(idx, 1); else frozen.push(colId);
  localStorage.setItem(`dbv_frozen_${tabId}`, JSON.stringify(frozen));
}

export function isColumnFrozen(tabId, colId) {
  return getFrozenCols(tabId).includes(colId);
}

// ── Column highlight ──

export function highlightColumn(tableEl, colIndex) {
  clearColumnHighlight(tableEl);
  if (colIndex < 0) return;
  tableEl.querySelectorAll('tr').forEach(tr => {
    const cell = tr.children[colIndex];
    if (cell) cell.classList.add('col-selected');
  });
}

export function clearColumnHighlight(tableEl) {
  if (!tableEl) return;
  tableEl.querySelectorAll('.col-selected').forEach(c => c.classList.remove('col-selected'));
}
