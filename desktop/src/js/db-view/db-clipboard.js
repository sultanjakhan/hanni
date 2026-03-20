// ── db-view/db-clipboard.js — Copy/paste cells (Ctrl+C / Ctrl+V) ──

import { invoke } from '../state.js';

/** Bind clipboard events on the table container */
export function bindClipboard(container, ctx) {
  const { recordTable, reloadFn } = ctx;

  container.addEventListener('keydown', (e) => {
    // Ctrl+C — copy focused cell value
    if ((e.ctrlKey || e.metaKey) && e.key === 'c' && !e.target.closest('.inline-editor')) {
      const cell = container.querySelector('.cell-focused') || container.querySelector('.cell-editable:focus');
      if (!cell) return;
      e.preventDefault();
      const raw = cell.dataset.rawValue || cell.textContent.trim();
      navigator.clipboard.writeText(raw).catch(() => {});
      cell.classList.add('cell-copied');
      setTimeout(() => cell.classList.remove('cell-copied'), 400);
    }

    // Ctrl+V — paste into focused cell
    if ((e.ctrlKey || e.metaKey) && e.key === 'v' && !e.target.closest('.inline-editor')) {
      const cell = container.querySelector('.cell-focused') || container.querySelector('.cell-editable:focus');
      if (!cell || !cell.dataset.propId) return;
      const autoTypes = ['created_time', 'last_edited', 'unique_id'];
      if (autoTypes.includes(cell.dataset.propType)) return;
      e.preventDefault();
      navigator.clipboard.readText().then(text => {
        if (!text) return;
        const recordId = parseInt(cell.dataset.recordId);
        const propId = parseInt(cell.dataset.propId);
        invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: text })
          .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
      }).catch(() => {});
    }

    // Delete / Backspace — clear focused cell
    if ((e.key === 'Delete' || e.key === 'Backspace') && !e.target.closest('.inline-editor')) {
      const cell = container.querySelector('.cell-focused') || container.querySelector('.cell-editable:focus');
      if (!cell || !cell.dataset.propId) return;
      const autoTypes = ['created_time', 'last_edited', 'unique_id'];
      if (autoTypes.includes(cell.dataset.propType)) return;
      e.preventDefault();
      const recordId = parseInt(cell.dataset.recordId);
      const propId = parseInt(cell.dataset.propId);
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: null })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    }
  });
}
