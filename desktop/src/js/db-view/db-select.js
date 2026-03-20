// ── db-view/db-select.js — Multi-row select with checkboxes + bulk actions ──

import { S } from '../state.js';
import { confirmModal } from '../utils.js';

/** Get or init selection set for a tab */
function getSelection(tabId) {
  if (!S._dbvSelected) S._dbvSelected = {};
  if (!S._dbvSelected[tabId]) S._dbvSelected[tabId] = new Set();
  return S._dbvSelected[tabId];
}

/** Clear selection for a tab */
export function clearSelection(tabId) {
  if (S._dbvSelected) S._dbvSelected[tabId] = new Set();
}

/** Render the bulk actions bar above the table */
export function renderBulkBar(container, tabId, ctx) {
  container.querySelector('.dbv-bulk-bar')?.remove();
  const sel = getSelection(tabId);
  if (sel.size === 0) return;

  const bar = document.createElement('div');
  bar.className = 'dbv-bulk-bar';
  bar.innerHTML = `<span class="dbv-bulk-count">${sel.size} выбрано</span>
    <button class="dbv-action-btn dbv-bulk-deselect">Снять</button>
    ${ctx.onDelete ? '<button class="dbv-action-btn dbv-bulk-delete">Удалить</button>' : ''}
    ${ctx.onDuplicate ? '<button class="dbv-action-btn dbv-bulk-dup">Дубликат</button>' : ''}`;

  bar.querySelector('.dbv-bulk-deselect')?.addEventListener('click', () => {
    clearSelection(tabId);
    if (ctx.reloadFn) ctx.reloadFn();
  });

  bar.querySelector('.dbv-bulk-delete')?.addEventListener('click', async () => {
    if (!(await confirmModal(`Удалить ${sel.size} записей?`))) return;
    for (const id of sel) await ctx.onDelete(id);
    clearSelection(tabId);
    if (ctx.reloadFn) ctx.reloadFn();
  });

  bar.querySelector('.dbv-bulk-dup')?.addEventListener('click', async () => {
    const records = ctx.records || [];
    for (const id of sel) {
      const rec = records.find(r => r[ctx.idField] === id);
      if (rec) await ctx.onDuplicate(rec);
    }
    clearSelection(tabId);
    if (ctx.reloadFn) ctx.reloadFn();
  });

  const table = container.querySelector('.data-table');
  if (table) table.before(bar);
}

/** Bind checkbox events on table rows */
export function bindCheckboxes(container, tabId, filteredRecords, idField, ctx) {
  const sel = getSelection(tabId);
  let lastChecked = null;

  // Header checkbox: select/deselect all
  const headerCb = container.querySelector('.col-check-header input');
  if (headerCb) {
    headerCb.checked = filteredRecords.length > 0 && filteredRecords.every(r => sel.has(r[idField]));
    headerCb.addEventListener('change', () => {
      if (headerCb.checked) filteredRecords.forEach(r => sel.add(r[idField]));
      else clearSelection(tabId);
      renderBulkBar(container, tabId, ctx);
      updateRowCheckboxes(container, tabId);
    });
  }

  // Row checkboxes with Shift+click range select
  container.querySelectorAll('.col-check input[type="checkbox"]').forEach((cb, i) => {
    const rid = parseInt(cb.closest('tr').dataset.id);
    cb.checked = sel.has(rid);
    cb.addEventListener('click', (e) => {
      e.stopPropagation();
      if (e.shiftKey && lastChecked !== null) {
        const start = Math.min(lastChecked, i), end = Math.max(lastChecked, i);
        const rows = container.querySelectorAll('.col-check input[type="checkbox"]');
        for (let j = start; j <= end; j++) {
          const rowId = parseInt(rows[j].closest('tr').dataset.id);
          sel.add(rowId);
          rows[j].checked = true;
        }
      } else {
        if (cb.checked) sel.add(rid); else sel.delete(rid);
      }
      lastChecked = i;
      renderBulkBar(container, tabId, ctx);
      if (headerCb) headerCb.checked = filteredRecords.every(r => sel.has(r[idField]));
    });
  });
}

function updateRowCheckboxes(container, tabId) {
  const sel = getSelection(tabId);
  container.querySelectorAll('.col-check input[type="checkbox"]').forEach(cb => {
    const rid = parseInt(cb.closest('tr').dataset.id);
    cb.checked = sel.has(rid);
  });
}

/** Get selected IDs */
export function getSelectedIds(tabId) {
  return getSelection(tabId);
}
