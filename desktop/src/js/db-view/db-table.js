import { S, invoke, getTypeIcon } from '../state.js';
import { escapeHtml } from '../utils.js';
import { formatPropValue, startInlineEdit, startFixedCellEdit } from './db-cell-editors.js';
import { getType } from './db-type-registry.js';
import { renderFilterBar, applyFilters, loadFiltersFromViewConfig } from './db-filters.js';
import { getHiddenFixedCols, getDeletedFixedCols, getFixedColName, buildUnifiedColumns } from './db-properties.js';
import { loadColState } from './db-col-state.js';
import { bindCheckboxes, renderBulkBar } from './db-select.js';
import { bindRowContextMenu } from './db-row-menu.js';
import { bindClipboard } from './db-clipboard.js';
import { setAnchor, extendTo, clearSelection, bindDragSelection } from './db-selection.js';
import { getNextCell, getPrevCell, getCellAbove, getCellBelow } from './db-cell-nav.js';
import { enableRowDrag } from './db-drag-rows.js';
import { enableColumnDrag } from './db-col-drag.js';
import { wireColumnResize } from './db-col-resize.js';
import { wireColumnClicks } from './db-col-clicks.js';

export async function renderTableView(el, ctx) {
  const {
    tabId, recordTable, records, fixedColumns = [], idField = 'id',
    customProps = [], valuesMap = {}, reloadFn, onRowClick, onAdd, onQuickAdd, addButton, onSort,
    onDelete, onDuplicate, onCellEdit,
  } = ctx;

  await loadColState(tabId);
  if (!S.dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filtered = applyFilters(records, valuesMap, S.dbvFilters[tabId], idField, tabId);
  const visProps = customProps.filter(p => p.visible !== false);
  const hiddenFixed = getHiddenFixedCols(tabId);
  const deletedFixed = getDeletedFixedCols(tabId);
  const visFixedColumns = fixedColumns.filter(c => !hiddenFixed.includes(c.key) && !deletedFixed.includes(c.key));
  const hasActions = !!(onDelete || onDuplicate);
  const reload = reloadFn || (() => {});

  // Build unified column order (fixed keys + "prop_ID")
  const unifiedCols = buildUnifiedColumns(tabId, visFixedColumns, visProps);
  const colCount = (hasActions ? 1 : 0) + unifiedCols.length + 1;

  // Header
  const W = { done: 30, title: 180, projectName: 100, priority: 100, status: 100, date: 100, tags: 120, name: 180, category: 130, quantity: 70, location: 100, needed: 90, frequency: 110, is_active: 60 };
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

  // Always move rows with empty primary field to the bottom (new blank rows stay at end)
  if (visFixedColumns.length > 0) {
    const pk = visFixedColumns[0].key;
    const nonEmpty = filtered.filter(r => r[pk] && String(r[pk]).trim() !== '');
    const empty = filtered.filter(r => !r[pk] || String(r[pk]).trim() === '');
    filtered.length = 0;
    filtered.push(...nonEmpty, ...empty);
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
        if (c.editable) {
          const raw = escapeHtml(String(r[c.key] ?? ''));
          const opts = c.editOptions ? escapeHtml(JSON.stringify(c.editOptions)) : '[]';
          return `<td class="cell-fixed-edit" data-record-id="${rid}" data-edit-key="${c.key}" data-edit-type="${c.editType || 'text'}" data-edit-options='${opts}' data-raw-value="${raw}" tabindex="0">${val}</td>`;
        }
        return `<td>${val}</td>`;
      } else {
        const p = col.def;
        const autoVals = { created_time: r.created_at, last_edited: r.updated_at, unique_id: rid };
        const raw = autoVals[p.type] ?? valuesMap[rid]?.[p.id] ?? '';
        const isReadonly = getType(p.type).auto;
        const cellClass = isReadonly ? 'cell-readonly' : 'cell-editable';
        return `<td class="${cellClass}" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}' data-raw-value="${escapeHtml(String(raw))}">${formatPropValue(raw, p)}</td>`;
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

  // Preserve scroll position across re-renders
  const oldWrap = el.querySelector('.dbv-table-wrap');
  const scrollTop = oldWrap ? oldWrap.scrollTop : 0;
  const scrollLeft = oldWrap ? oldWrap.scrollLeft : 0;

  el.innerHTML = `<div class="dbv-table-wrap"><table class="data-table database-view"><thead><tr>${thCheck}${thCols}<th class="add-prop-col dbv-add-prop-col" title="Добавить свойство">+</th></tr></thead><tbody>${tbody}${addRowHtml}</tbody></table>${footerHtml}</div>`;

  // Restore scroll position
  const newWrap = el.querySelector('.dbv-table-wrap');
  if (newWrap) { newWrap.scrollTop = scrollTop; newWrap.scrollLeft = scrollLeft; }

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
  if (tableEl) bindDragSelection(tableEl, (cell) => focusCell(el, cell));

  // Cell selection + inline editing (click=edit for editable, click=select for readonly)
  const allCells = el.querySelectorAll('.cell-editable, .cell-fixed-edit, .cell-readonly');
  allCells.forEach(cell => {
    cell.setAttribute('tabindex', '0');
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      if (e.shiftKey && tableEl) { extendTo(cell, tableEl); return; }
      focusCell(el, cell);
      if (tableEl) setAnchor(cell, tableEl);
      if (cell.classList.contains('cell-fixed-edit')) startFixedCellEdit(cell, reload);
      else if (cell.classList.contains('cell-editable')) startInlineEdit(cell, recordTable, reload);
    });
    cell.addEventListener('focus', () => focusCell(el, cell));
  });
  // Keyboard navigation + Escape
  el.addEventListener('keydown', (e) => {
    if (e.target.closest('.inline-editor')) return;
    const focused = el.querySelector('.cell-focused');
    if (!focused) return;
    if (e.key === 'Escape' && tableEl) { clearSelection(tableEl); focusCell(el, document.createElement('div')); return; }
    const nav = { Tab: e.shiftKey ? getPrevCell : getNextCell, ArrowRight: getNextCell, ArrowLeft: getPrevCell, ArrowDown: getCellBelow, ArrowUp: getCellAbove };
    const fn = nav[e.key];
    if (fn) {
      e.preventDefault();
      const next = fn(focused);
      if (next) { focusCell(el, next); if (tableEl) setAnchor(next, tableEl); next.scrollIntoView({ block: 'nearest', inline: 'nearest' }); }
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (focused.classList.contains('cell-editable')) startInlineEdit(focused, recordTable, reload);
      else if (focused.classList.contains('cell-fixed-edit')) startFixedCellEdit(focused, reload);
    }
  });
  el.addEventListener('fixed-cell-save', async (e) => {
    const { recordId, key, value, skipReload } = e.detail;
    if (onCellEdit) {
      try { await onCellEdit(parseInt(recordId), key, value, skipReload); } catch {}
    }
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
