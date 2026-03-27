// ── db-view/db-col-clicks.js — Column header click handlers ──

import { S } from '../state.js';
import { showAddPropertyPopover, showColumnMenu, showFixedColumnMenu, highlightColumn, clearColumnHighlight } from './db-properties.js';
import { selectColumn, extendColumn, clearSelection } from './db-selection.js';

/** Wire all column header interactions (click, inline edit, highlight) */
export function wireColumnClicks(el, { tabId, customProps, reload, onSort, onRowClick, onAdd, onQuickAdd, filtered, idField, recordTable }) {
  const tableEl = el.querySelector('.data-table');

  // Add property column
  el.querySelector('.dbv-add-prop-col')?.addEventListener('click', (e) => showAddPropertyPopover(tabId, e.target, reload));

  // Custom property headers
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      e.stopPropagation();
      const colIdx = Array.from(th.parentElement.children).indexOf(th);
      highlightColumn(tableEl, colIdx);
      if (tableEl) { e.shiftKey ? extendColumn(colIdx, tableEl) : selectColumn(colIdx, tableEl); }
      if (!e.shiftKey) {
        const prop = customProps.find(p => p.id === parseInt(th.dataset.propId));
        if (prop) showColumnMenu(prop, th.getBoundingClientRect(), tabId, reload, onSort);
      }
    });
  });

  // Fixed column headers
  el.querySelectorAll('.fixed-header').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      e.stopPropagation();
      const colIdx = Array.from(th.parentElement.children).indexOf(th);
      highlightColumn(tableEl, colIdx);
      if (tableEl) { e.shiftKey ? extendColumn(colIdx, tableEl) : selectColumn(colIdx, tableEl); }
      if (!e.shiftKey) showFixedColumnMenu(th.dataset.fixedKey, th.dataset.fixedLabel || th.dataset.fixedKey, th.getBoundingClientRect(), tabId, reload, onSort);
    });
  });

  // Clear highlight + selection on click outside
  el.addEventListener('click', (e) => {
    if (!e.target.closest('.prop-header') && !e.target.closest('.fixed-header')) {
      clearColumnHighlight(tableEl);
      if (!e.target.closest('.cell-editable, .cell-fixed-edit, .cell-readonly') && tableEl) clearSelection(tableEl);
    }
  });

  // Add-row: prefer inline creation (onQuickAdd), fallback to modal (onAdd)
  el.querySelector('.dbv-add-row')?.addEventListener('click', async () => {
    if (onQuickAdd) {
      S._focusNewRow = tabId;
      try { await onQuickAdd(); } catch { S._focusNewRow = null; }
    } else if (onAdd) { onAdd(); }
  });

  // Row click
  if (onRowClick) el.querySelectorAll('.data-table-row').forEach(row => {
    row.style.cursor = 'pointer';
    row.addEventListener('click', (e) => {
      if (e.target.closest('.cell-editable,.inline-editor,.col-check')) return;
      const rec = filtered.find(r => r[idField] === parseInt(row.dataset.id));
      if (rec) onRowClick(rec);
    });
  });
}
