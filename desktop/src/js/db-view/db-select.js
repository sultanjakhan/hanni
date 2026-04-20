// ── db-view/db-select.js — Multi-row select with checkboxes + bulk actions ──

import { S } from '../state.js';
import { confirmModal } from '../utils.js';
import { surgicalRowRemove } from './db-row-menu.js';

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
    ${ctx.onFreeze ? '<button class="dbv-action-btn dbv-bulk-freeze">❄ Заморозить</button>' : ''}
    ${ctx.onDelete ? '<button class="dbv-action-btn dbv-bulk-delete">Удалить</button>' : ''}
    ${ctx.onDuplicate ? '<button class="dbv-action-btn dbv-bulk-dup">Дубликат</button>' : ''}`;

  bar.querySelector('.dbv-bulk-deselect')?.addEventListener('click', () => {
    clearSelection(tabId);
    if (ctx.reloadFn) ctx.reloadFn();
  });

  bar.querySelector('.dbv-bulk-delete')?.addEventListener('click', async () => {
    if (!(await confirmModal(`Удалить ${sel.size} записей?`))) return;
    const ids = [...sel];
    for (const id of ids) await ctx.onDelete(id);
    clearSelection(tabId);
    for (const id of ids) surgicalRowRemove(container, id);
    bar.remove();
  });

  bar.querySelector('.dbv-bulk-freeze')?.addEventListener('click', async () => {
    const records = ctx.records || [];
    const ids = [...sel];
    for (const id of ids) {
      const rec = records.find(r => r[ctx.idField] === id);
      if (rec) await ctx.onFreeze(id, rec);
    }
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

  // Header: select/deselect all — click on the TH cell
  const headerTh = container.querySelector('.col-check-header');
  const headerCb = headerTh?.querySelector('input');
  if (headerCb) {
    headerCb.checked = filteredRecords.length > 0 && filteredRecords.every(r => sel.has(r[idField]));
    headerTh.addEventListener('click', (e) => {
      e.stopPropagation();
      const allSelected = filteredRecords.length > 0 && filteredRecords.every(r => sel.has(r[idField]));
      if (allSelected) clearSelection(tabId);
      else filteredRecords.forEach(r => sel.add(r[idField]));
      headerCb.checked = !allSelected;
      renderBulkBar(container, tabId, ctx);
      updateRowCheckboxes(container, tabId);
    });
  }

  // Row select with Shift+click range — click anywhere in .col-check cell
  const allTds = container.querySelectorAll('.col-check');
  allTds.forEach((td, i) => {
    const rid = parseInt(td.closest('tr').dataset.id);
    if (sel.has(rid)) td.classList.add('row-checked');
    td.addEventListener('click', (e) => {
      e.stopPropagation();
      if (e.shiftKey && lastChecked !== null) {
        const start = Math.min(lastChecked, i), end = Math.max(lastChecked, i);
        for (let j = start; j <= end; j++) {
          const rowId = parseInt(allTds[j].closest('tr').dataset.id);
          sel.add(rowId);
          allTds[j].classList.add('row-checked');
        }
      } else {
        const isOn = sel.has(rid);
        if (isOn) { sel.delete(rid); td.classList.remove('row-checked'); }
        else { sel.add(rid); td.classList.add('row-checked'); }
      }
      lastChecked = i;
      renderBulkBar(container, tabId, ctx);
      syncCheckAll(headerCb, filteredRecords, idField, sel);
    });
  });
}

/** Sync header "select all" state with current selection */
function syncCheckAll(headerCb, filteredRecords, idField, sel) {
  if (!headerCb) return;
  headerCb.checked = filteredRecords.length > 0 && filteredRecords.every(r => sel.has(r[idField]));
}

function updateRowCheckboxes(container, tabId) {
  const sel = getSelection(tabId);
  container.querySelectorAll('.col-check').forEach(td => {
    const rid = parseInt(td.closest('tr').dataset.id);
    td.classList.toggle('row-checked', sel.has(rid));
  });
}

/** Get selected IDs */
export function getSelectedIds(tabId) {
  return getSelection(tabId);
}
