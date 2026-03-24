// ── db-view/db-col-clicks.js — Column header click handlers ──

import { showAddPropertyPopover, showColumnMenu, showFixedColumnMenu, highlightColumn, clearColumnHighlight } from './db-properties.js';

/** Wire all column header interactions (click, inline edit, highlight) */
export function wireColumnClicks(el, { tabId, customProps, reload, onSort, onRowClick, onAdd, filtered, idField, recordTable }) {
  const tableEl = el.querySelector('.data-table');

  // Add property column
  el.querySelector('.dbv-add-prop-col')?.addEventListener('click', (e) => showAddPropertyPopover(tabId, e.target, reload));

  // Custom property headers
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      e.stopPropagation();
      highlightColumn(tableEl, Array.from(th.parentElement.children).indexOf(th));
      const prop = customProps.find(p => p.id === parseInt(th.dataset.propId));
      if (prop) showColumnMenu(prop, th.getBoundingClientRect(), tabId, reload, onSort);
    });
  });

  // Fixed column headers
  el.querySelectorAll('.fixed-header').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      e.stopPropagation();
      highlightColumn(tableEl, Array.from(th.parentElement.children).indexOf(th));
      showFixedColumnMenu(th.dataset.fixedKey, th.dataset.fixedLabel || th.dataset.fixedKey, th.getBoundingClientRect(), tabId, reload, onSort);
    });
  });

  // Clear highlight on click outside
  el.addEventListener('click', (e) => {
    if (!e.target.closest('.prop-header') && !e.target.closest('.fixed-header')) clearColumnHighlight(tableEl);
  });

  // Add-row
  el.querySelector('.dbv-add-row')?.addEventListener('click', () => { if (onAdd) onAdd(); });

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
