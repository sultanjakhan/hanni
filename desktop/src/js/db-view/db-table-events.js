// ── db-view/db-table-events.js — Table event wiring ──

import { S } from '../state.js';
import { startInlineEdit, startFixedCellEdit } from './db-cell-editors.js';
import { showAddPropertyPopover, showColumnMenu, showFixedColumnMenu } from './db-properties.js';
import { clearCellFocus } from './db-cell-nav.js';
import { showSidePeek } from './db-side-peek.js';

/** Wire all table events: row clicks, inline editing, add row, sorting, navigation */
export function wireTableEvents(el, ctx, filtered, visProp) {
  const {
    idField = 'id', recordTable, reloadFn,
    onRowClick, onQuickAdd, onSort, onCellEdit, tabId, customProps = [],
  } = ctx;

  // Clear cell focus when clicking outside cells
  el.addEventListener('click', (e) => {
    if (!e.target.closest('.cell-editable') && !e.target.closest('.cell-fixed-edit')) {
      clearCellFocus(el);
    }
  });

  wireRowClicks(el, filtered, idField, onRowClick);
  wireCellEditing(el, recordTable, reloadFn, onCellEdit);
  wireAddProperty(el, tabId, reloadFn);
  wireColumnMenus(el, customProps, tabId, reloadFn, onSort);
  wireFixedColumnMenus(el, tabId, reloadFn, onSort);
  wireAddRow(el, tabId, onQuickAdd);
  wireRowInsert(el, tabId, onQuickAdd);
  wireOpenButtons(el, filtered, ctx);
  wireFixedSorting(el, onSort);
}

function wireRowClicks(el, filtered, idField, onRowClick) {
  if (!onRowClick) return;
  el.querySelectorAll('.data-table-row').forEach(row => {
    row.style.cursor = 'pointer';
    row.addEventListener('click', (e) => {
      if (e.target.closest('.cell-editable') && e.target.closest('.inline-editor')) return;
      if (e.target.closest('.col-check') || e.target.closest('.col-drag')) return;
      if (e.target.closest('.cell-editable') || e.target.closest('.cell-fixed-edit')) return;
      const rec = filtered.find(r => r[idField] === parseInt(row.dataset.id));
      if (rec) onRowClick(rec);
    });
  });
}

function wireCellEditing(el, recordTable, reloadFn, onCellEdit) {
  // Excel-style: single click = immediately edit
  el.querySelectorAll('.cell-editable:not(.cell-fixed-edit)').forEach(cell => {
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      if (cell.querySelector('.inline-editor') || cell.querySelector('.inline-dropdown')) return;
      clearCellFocus(el);
      startInlineEdit(cell, recordTable, reloadFn);
    });
  });

  el.querySelectorAll('.cell-fixed-edit').forEach(cell => {
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      if (cell.querySelector('.inline-editor')) return;
      clearCellFocus(el);
      startFixedCellEdit(cell, reloadFn);
    });
  });

  el.addEventListener('fixed-cell-save', async (e) => {
    const { recordId, key, value, skipReload } = e.detail;
    if (onCellEdit) {
      try { await onCellEdit(parseInt(recordId), key, value, skipReload); } catch {}
    }
  });
}

function wireAddProperty(el, tabId, reloadFn) {
  const addPropTh = el.querySelector('.dbv-add-prop-col');
  addPropTh?.addEventListener('click', () => {
    showAddPropertyPopover(tabId, addPropTh, reloadFn);
  });
}

function wireColumnMenus(el, customProps, tabId, reloadFn, onSort) {
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      e.stopPropagation();
      const prop = customProps.find(p => p.id === parseInt(th.dataset.propId));
      if (!prop) return;
      showColumnMenu(prop, th.getBoundingClientRect(), tabId, reloadFn, onSort);
    });
  });
}

function wireAddRow(el, tabId, onQuickAdd) {
  const addRowTr = el.querySelector('.data-table-add-row-tr');
  if (!addRowTr || !onQuickAdd) return;
  addRowTr.addEventListener('click', async () => {
    addRowTr.style.pointerEvents = 'none';
    addRowTr.style.opacity = '0.5';
    S._focusNewRow = tabId;
    try {
      await onQuickAdd();
    } catch {
      S._focusNewRow = null;
      addRowTr.style.pointerEvents = '';
      addRowTr.style.opacity = '';
    }
  });
}

function wireOpenButtons(el, filtered, ctx) {
  const { idField = 'id', fixedColumns, customProps, valuesMap, recordTable, reloadFn } = ctx;
  el.querySelectorAll('.row-open-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const row = btn.closest('.data-table-row');
      if (!row) return;
      const rec = filtered.find(r => r[idField] === parseInt(row.dataset.id));
      if (rec) showSidePeek(rec, { fixedColumns, customProps, valuesMap, recordTable, idField, reloadFn });
    });
  });
}

function wireRowInsert(el, tabId, onQuickAdd) {
  if (!onQuickAdd) return;
  el.querySelectorAll('.row-insert-btn').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      S._focusNewRow = tabId;
      try { await onQuickAdd(); } catch { S._focusNewRow = null; }
    });
  });
}

function wireFixedColumnMenus(el, tabId, reloadFn, onSort) {
  el.querySelectorAll('.fixed-header').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      e.stopPropagation();
      showFixedColumnMenu(th.dataset.fixedKey, th.dataset.fixedLabel || th.dataset.fixedKey, th.getBoundingClientRect(), tabId, reloadFn, onSort);
    });
  });
}

function wireFixedSorting(el, onSort) {
  el.querySelectorAll('.sortable-header:not(.prop-header):not(.fixed-header)').forEach(th => {
    th.addEventListener('click', () => {
      const key = th.dataset.sort;
      const dir = th.dataset.dir === 'asc' ? 'desc' : 'asc';
      el.querySelectorAll('.sortable-header').forEach(h => {
        h.dataset.dir = 'none';
        h.classList.remove('sort-asc', 'sort-desc');
      });
      th.dataset.dir = dir;
      th.classList.add(`sort-${dir}`);
      if (onSort) onSort(key, dir);
    });
  });
}
