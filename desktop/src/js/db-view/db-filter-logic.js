// ── db-view/db-filter-logic.js — Filter application logic ──

import { S, invoke } from '../state.js';

/** Apply filters to records. Supports AND/OR mode. */
export function applyFilters(records, valuesMap, filters, idField) {
  if (!filters || filters.length === 0) return records;
  const mode = filters._mode || 'and';
  const list = Array.isArray(filters) ? filters : filters.rules || [];
  if (list.length === 0) return records;

  return records.filter(r => {
    const rid = r[idField];
    const results = list.map(f => {
      if (typeof f !== 'object' || !f.condition) return true;
      const val = getFilterValue(r, rid, f, valuesMap);
      return matchCondition(val, f.condition, f.value || '');
    });
    return mode === 'or' ? results.some(Boolean) : results.every(Boolean);
  });
}

function getFilterValue(r, rid, f, valuesMap) {
  if (f.filterKey?.startsWith('prop_')) return valuesMap[rid]?.[parseInt(f.filterKey.substring(5))] ?? '';
  if (f.propId) return valuesMap[rid]?.[f.propId] ?? '';
  return String(r[f.filterKey] ?? '');
}

function matchCondition(val, condition, target) {
  const s = String(val).toLowerCase();
  const t = target.toLowerCase();
  switch (condition) {
    case 'eq': return s === t;
    case 'neq': return s !== t;
    case 'contains': return s.includes(t);
    case 'not_contains': return !s.includes(t);
    case 'starts_with': return s.startsWith(t);
    case 'ends_with': return s.endsWith(t);
    case 'empty': return !val;
    case 'not_empty': return !!val;
    case 'gt': return parseFloat(val) > parseFloat(target);
    case 'lt': return parseFloat(val) < parseFloat(target);
    case 'gte': return parseFloat(val) >= parseFloat(target);
    case 'lte': return parseFloat(val) <= parseFloat(target);
    case 'before': return val && val < target;
    case 'after': return val && val > target;
    case 'this_week': return isThisWeek(val);
    case 'this_month': return isThisMonth(val);
    case 'last_7_days': return isLastNDays(val, 7);
    case 'last_30_days': return isLastNDays(val, 30);
    default: return true;
  }
}

function isThisWeek(val) {
  if (!val) return false;
  const d = new Date(val), now = new Date();
  const dayOfWeek = now.getDay() || 7;
  const weekStart = new Date(now);
  weekStart.setDate(now.getDate() - dayOfWeek + 1);
  weekStart.setHours(0, 0, 0, 0);
  const weekEnd = new Date(weekStart);
  weekEnd.setDate(weekStart.getDate() + 7);
  return d >= weekStart && d < weekEnd;
}

function isThisMonth(val) {
  if (!val) return false;
  const d = new Date(val), now = new Date();
  return d.getMonth() === now.getMonth() && d.getFullYear() === now.getFullYear();
}

function isLastNDays(val, n) {
  if (!val) return false;
  const d = new Date(val), now = new Date(), cutoff = new Date(now);
  cutoff.setDate(now.getDate() - n);
  return d >= cutoff && d <= now;
}

/** Persist filters */
export async function saveFiltersToViewConfig(tabId) {
  const filters = S.dbvFilters[tabId] || [];
  try {
    const configs = await invoke('get_view_configs', { tabId });
    const json = JSON.stringify(filters);
    if (configs.length > 0) await invoke('update_view_config', { id: configs[0].id, viewType: null, filterJson: json, sortJson: null, visibleColumns: null });
    else { const id = await invoke('create_view_config', { tabId, name: 'Default', viewType: 'table' }); await invoke('update_view_config', { id, viewType: null, filterJson: json, sortJson: null, visibleColumns: null }); }
  } catch {}
}

/** Load filters */
export async function loadFiltersFromViewConfig(tabId) {
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs[0]?.filter_json) S.dbvFilters[tabId] = JSON.parse(configs[0].filter_json);
  } catch {}
}
