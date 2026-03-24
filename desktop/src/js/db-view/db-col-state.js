// ── db-view/db-col-state.js — Column state persistence (SQLite via ui_state) ──
// Uses in-memory cache for sync reads, persists to DB for durability across updates.

import { invoke } from '../state.js';

const _cache = {};

function _get(key, fallback) {
  if (key in _cache) { try { return JSON.parse(_cache[key]); } catch { return fallback; } }
  return fallback;
}

function _set(key, value) {
  const json = JSON.stringify(value);
  _cache[key] = json;
  invoke('set_ui_state', { key, value: json }).catch(() => {});
}

/** Load all ui_state for a tab into cache (call once on tab render) */
export async function loadColState(tabId) {
  const keys = [
    `dbv_hidden_fixed_${tabId}`, `dbv_fixed_names_${tabId}`,
    `dbv_col_order_${tabId}`, `dbv_wrap_${tabId}`, `dbv_frozen_${tabId}`,
  ];
  for (const k of keys) {
    if (k in _cache) continue;
    try {
      const v = await invoke('get_ui_state', { key: k });
      if (v != null) _cache[k] = v;
    } catch {}
  }
  // Migrate from localStorage if DB is empty
  for (const k of keys) {
    if (k in _cache) continue;
    const ls = localStorage.getItem(k);
    if (ls) { _cache[k] = ls; invoke('set_ui_state', { key: k, value: ls }).catch(() => {}); }
  }
}

// ── Hidden fixed columns ──

export function getHiddenFixedCols(tabId) { return _get(`dbv_hidden_fixed_${tabId}`, []); }

export function setHiddenFixedCols(tabId, keys) { _set(`dbv_hidden_fixed_${tabId}`, keys); }

// ── Fixed column custom display names ──

export function getFixedColName(tabId, key, fallback) {
  const names = _get(`dbv_fixed_names_${tabId}`, {});
  return names[key] || fallback;
}

export function setFixedColName(tabId, key, name) {
  const names = _get(`dbv_fixed_names_${tabId}`, {});
  if (name) names[key] = name; else delete names[key];
  _set(`dbv_fixed_names_${tabId}`, names);
}

// ── Unified column order (fixed keys + "prop_ID") ──

export function getColumnOrder(tabId) { return _get(`dbv_col_order_${tabId}`, []); }

export function setColumnOrder(tabId, order) { _set(`dbv_col_order_${tabId}`, order); }

// ── Text wrap toggle ──

export function isColumnWrapped(tabId, colId) { return !!_get(`dbv_wrap_${tabId}`, {})[colId]; }

export function toggleWrap(tabId, colId) {
  const s = _get(`dbv_wrap_${tabId}`, {});
  s[colId] = !s[colId];
  _set(`dbv_wrap_${tabId}`, s);
}

// ── Frozen columns ──

export function getFrozenCols(tabId) { return _get(`dbv_frozen_${tabId}`, []); }

export function toggleFreezeCol(tabId, colId) {
  const frozen = getFrozenCols(tabId);
  const idx = frozen.indexOf(colId);
  if (idx >= 0) frozen.splice(idx, 1); else frozen.push(colId);
  _set(`dbv_frozen_${tabId}`, frozen);
}

export function isColumnFrozen(tabId, colId) { return getFrozenCols(tabId).includes(colId); }

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
