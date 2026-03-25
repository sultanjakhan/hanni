// ── db-view/db-table.js — Table view renderer (Notion-style) ──

import { S, invoke, getTypeIcon } from '../state.js';
import { escapeHtml } from '../utils.js';
import { formatPropValue, startInlineEdit } from './db-cell-editors.js';
import { renderFilterBar, applyFilters, loadFiltersFromViewConfig } from './db-filters.js';
import { getHiddenFixedCols, getFixedColName, getColumnOrder } from './db-properties.js';
import { loadColState } from './db-col-state.js';
import { bindCheckboxes, renderBulkBar } from './db-select.js';
import { bindRowContextMenu } from './db-row-menu.js';
import { bindClipboard } from './db-clipboard.js';
import { enableRowDrag } from './db-drag-rows.js';
import { enableColumnDrag } from './db-col-drag.js';
import { wireColumnResize } from './db-col-resize.js';
import { wireColumnClicks } from './db-col-clicks.js';

export async function renderTableView(el, ctx) {
  const {
    tabId, recordTable, records, fixedColumns = [], idField = 'id',
    customProps = [], valuesMap = {}, reloadFn, onRowClick, onAdd, onQuickAdd, addButton, onSort,
    onDelete, onDuplicate,
  } = ctx;

  await loadColState(tabId);
  if (!S.dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filtered = applyFilters(records, valuesMap, S.dbvFilters[tabId], idField, tabId);
  const visProps = customProps.filter(p => p.visible !== false);
  const hiddenFixed = getHiddenFixedCols(tabId);
  const visFixedColumns = fixedColumns.filter(c => !hiddenFixed.includes(c.key));
  const hasActions = !!(onDelete || onDuplicate);
  const reload = reloadFn || (() => {});

  // Build unified column order (fixed keys + "prop_ID")
  const unifiedCols = buildUnifiedColumns(tabId, visFixedColumns, visProps);
  const colCount = (hasActions ? 1 : 0) + unifiedCols.length + 1;

  // Header
  const W = { done: 30, title: 180, projectName: 100, priority: 100, status: 100, date: 100, tags: 120, name: 180, category: 100, quantity: 70, location: 100, needed: 90 };
  const thCheck = hasActions ? '<th class="col-check-header"><input type="checkbox"></th>' : '';
  const thCols = unifiedCols.map(col => {
    if (col.kind === 'fixed') {
      const c = col.def;
      const w = W[c.key] || 140;
      const displayName = getFixedColName(tabId, c.key, c.label);
      if (!displayName) return `<th class="sortable-header fixed-header draggable-col" data-sort="${c.key}" data-fixed-key="${c.key}" data-col-id="${c.key}" style="width:${w}px"></th>`;
      return `<th class="sortable-header fixed-header draggable-col" data-sort="${c.key}" data-fixed-key="${c.key}" data-fixed-label="${escapeHtml(c.label)}" data-col-id="${c.key}" style="width:${w}px"><div class="th-content">${escapeHtml(displayName)}</div></th>`;
    } else {
      const p = col.def;
      return `<th class="sortable-header prop-header draggable-col" data-sort="prop_${p.id}" data-prop-id="${p.id}" data-col-id="prop_${p.id}" style="width:180px"><div class="th-content">${escapeHtml(p.name)}</div></th>`;
    }
  }).join('');

  // If a new row was just created, move it to the end so it appears at the bottom
  if (S._focusNewRow === tabId) {
    let maxId = -1, maxIdx = -1;
    filtered.forEach((r, i) => { const id = r[idField]; if (id > maxId) { maxId = id; maxIdx = i; } });
    if (maxIdx > -1 && maxIdx < filtered.length - 1) {
      const [newRec] = filtered.splice(maxIdx, 1);
      filtered.push(newRec);
    }
  }

  // Body
  let tbody = '';
  for (const r of filtered) {
    const rid = r[idField];
    const tdCheck = hasActions ? '<td class="col-check"><input type="checkbox"></td>' : '';
    const tdCols = unifiedCols.map(col => {
      if (col.kind === 'fixed') {
        const c = col.def;
        const val = c.render ? c.render(r) : escapeHtml(String(r[c.key] ?? ''));
        return `<td>${val}</td>`;
      } else {
        const p = col.def;
        const autoVals = { created_time: r.created_at, last_edited: r.updated_at, unique_id: rid };
        const raw = autoVals[p.type] ?? valuesMap[rid]?.[p.id] ?? '';
        return `<td class="cell-editable" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}' data-raw-value="${escapeHtml(String(raw))}">${formatPropValue(raw, p)}</td>`;
      }
    }).join('');
    tbody += `<tr class="data-table-row" data-id="${rid}">${tdCheck}${tdCols}<td></td></tr>`;
  }

  if (filtered.length === 0) {
    tbody = `<tr><td colspan="${colCount}" style="text-align:center;color:var(--text-faint);padding:24px;">Пока пусто</td></tr>`;
  }

  // Add-row + footer
  const addRowHtml = (onAdd || onQuickAdd) ? `<tr class="dbv-add-row"><td colspan="${colCount}"><div class="dbv-add-row-label"><span class="dbv-add-row-plus">+</span></div></td></tr>` : '';
  const footerHtml = `<div class="dbv-table-footer"><span>Записей: ${filtered.length}</span></div>`;

  el.innerHTML = `<div class="dbv-table-wrap"><table class="data-table database-view"><thead><tr>${thCheck}${thCols}<th class="add-prop-col dbv-add-prop-col" title="Добавить свойство">+</th></tr></thead><tbody>${tbody}${addRowHtml}</tbody></table>${footerHtml}</div>`;

  // Wire features
  wireColumnResize(el, tabId);
  if (customProps.length > 0) renderFilterBar(el, tabId, customProps, reload);
  if (hasActions) {
    bindCheckboxes(el, tabId, filtered, idField, { ...ctx, reloadFn: reload, records: filtered });
    renderBulkBar(el, tabId, { ...ctx, reloadFn: reload, records: filtered });
    bindRowContextMenu(el, { records: filtered, idField, onDelete, onDuplicate, reloadFn: reload });
  }
  bindClipboard(el, { recordTable, reloadFn: reload });
  enableRowDrag(el, filtered, idField);
  const tableEl = el.querySelector('.data-table');
  if (tableEl) enableColumnDrag(tableEl, tabId, reload, visProps);

  // Inline editing
  el.querySelectorAll('.cell-editable').forEach(cell => {
    cell.setAttribute('tabindex', '0');
    cell.addEventListener('click', (e) => { e.stopPropagation(); focusCell(el, cell); startInlineEdit(cell, recordTable, reload); });
    cell.addEventListener('focus', () => focusCell(el, cell));
  });

  // Column clicks, row clicks, add-row
  wireColumnClicks(el, { tabId, customProps, reload, onSort, onRowClick, onAdd, onQuickAdd, filtered, idField, recordTable });

  // Auto-focus new row (find row with highest ID = most recently created)
  if (S._focusNewRow === tabId) {
    S._focusNewRow = null;
    const rows = el.querySelectorAll('.data-table-row');
    let newRow = null;
    let maxId = -1;
    rows.forEach(r => { const id = parseInt(r.dataset.id); if (id > maxId) { maxId = id; newRow = r; } });
    if (newRow) {
      newRow.classList.add('dbv-row-new');
      newRow.scrollIntoView({ block: 'nearest' });
      const firstCell = newRow.querySelector('.cell-editable') || newRow.querySelector('.cell-fixed-edit');
      if (firstCell) setTimeout(() => firstCell.click(), 50);
    }
  }
}

function focusCell(container, cell) {
  container.querySelectorAll('.cell-focused').forEach(c => c.classList.remove('cell-focused'));
  cell.classList.add('cell-focused');
}

/** Merge fixed + custom columns in persisted order */
function buildUnifiedColumns(tabId, visFixed, visProps) {
  const savedOrder = getColumnOrder(tabId);
  const fixedMap = Object.fromEntries(visFixed.map(c => [c.key, c]));
  const propMap = Object.fromEntries(visProps.map(p => [`prop_${p.id}`, p]));
  const allIds = new Set([...visFixed.map(c => c.key), ...visProps.map(p => `prop_${p.id}`)]);
  const result = [];
  const placed = new Set();

  // Place columns in saved order
  for (const id of savedOrder) {
    if (!allIds.has(id)) continue;
    if (fixedMap[id]) result.push({ kind: 'fixed', def: fixedMap[id] });
    else if (propMap[id]) result.push({ kind: 'prop', def: propMap[id] });
    placed.add(id);
  }
  // Append any new columns not in saved order
  for (const id of allIds) {
    if (placed.has(id)) continue;
    if (fixedMap[id]) result.push({ kind: 'fixed', def: fixedMap[id] });
    else if (propMap[id]) result.push({ kind: 'prop', def: propMap[id] });
  }
  return result;
}
