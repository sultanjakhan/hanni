// ── db-view/db-clipboard.js — Copy/paste/delete cells (single + range) ──

import { invoke } from '../state.js';
import { getSelectedCells, isSelectionActive, buildTSV } from './db-selection.js';

const AUTO_TYPES = ['created_time', 'last_edited', 'unique_id'];

/** Bind clipboard events on the table container */
export function bindClipboard(container, ctx) {
  const { recordTable, reloadFn } = ctx;
  const table = () => container.querySelector('.data-table');

  container.addEventListener('keydown', (e) => {
    if (e.target.closest('.inline-editor')) return;

    // Ctrl+C — copy
    if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
      const t = table();
      if (isSelectionActive() && t) {
        e.preventDefault();
        const tsv = buildTSV(t);
        navigator.clipboard.writeText(tsv).catch(() => {});
        getSelectedCells(t).forEach(c => { c.classList.add('cell-copied'); setTimeout(() => c.classList.remove('cell-copied'), 400); });
        return;
      }
      const cell = container.querySelector('.cell-focused');
      if (!cell) return;
      e.preventDefault();
      navigator.clipboard.writeText(cell.dataset.rawValue || cell.textContent.trim()).catch(() => {});
      cell.classList.add('cell-copied');
      setTimeout(() => cell.classList.remove('cell-copied'), 400);
    }

    // Ctrl+V — paste into focused cell
    if ((e.ctrlKey || e.metaKey) && e.key === 'v') {
      const cell = container.querySelector('.cell-focused');
      if (!cell || !cell.dataset.propId) return;
      if (AUTO_TYPES.includes(cell.dataset.propType)) return;
      e.preventDefault();
      navigator.clipboard.readText().then(text => {
        if (!text) return;
        invoke('set_property_value', { recordId: parseInt(cell.dataset.recordId), recordTable, propertyId: parseInt(cell.dataset.propId), value: text })
          .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
      }).catch(() => {});
    }

    // Delete / Backspace — clear selected cells
    if (e.key === 'Delete' || e.key === 'Backspace') {
      const t = table();
      const cells = isSelectionActive() && t ? getSelectedCells(t) : [];
      if (cells.length > 0) {
        e.preventDefault();
        clearCells(cells, recordTable, reloadFn);
        return;
      }
      const cell = container.querySelector('.cell-focused');
      if (cell) { e.preventDefault(); clearCells([cell], recordTable, reloadFn); }
    }
  });
}

function clearCells(cells, recordTable, reloadFn) {
  const promises = [];
  for (const cell of cells) {
    if (AUTO_TYPES.includes(cell.dataset.propType)) continue;
    if (cell.dataset.propId) {
      promises.push(invoke('set_property_value', { recordId: parseInt(cell.dataset.recordId), recordTable, propertyId: parseInt(cell.dataset.propId), value: null }));
    } else if (cell.dataset.editKey) {
      cell.dispatchEvent(new CustomEvent('fixed-cell-save', { bubbles: true, detail: { recordId: cell.dataset.recordId, key: cell.dataset.editKey, value: null } }));
    }
  }
  if (promises.length > 0) Promise.all(promises).then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
}
