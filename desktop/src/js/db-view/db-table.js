// ── db-view/db-table.js — Table view renderer (Notion-style) ──

import { S, invoke, getTypeIcon } from '../state.js';
import { escapeHtml } from '../utils.js';
import { formatPropValue, startInlineEdit } from './db-cell-editors.js';
import { renderFilterBar, applyFilters, loadFiltersFromViewConfig } from './db-filters.js';
import { showAddPropertyPopover, showColumnMenu } from './db-properties.js';
import { bindCheckboxes, renderBulkBar } from './db-select.js';
import { bindRowContextMenu } from './db-row-menu.js';
import { bindClipboard } from './db-clipboard.js';
import { enableRowDrag } from './db-drag-rows.js';
import { enableColumnDrag } from './db-col-drag.js';
import { wireColumnResize } from './db-col-resize.js';

export async function renderTableView(el, ctx) {
  const {
    tabId, recordTable, records, fixedColumns = [], idField = 'id',
    customProps = [], valuesMap = {}, reloadFn, onRowClick, onAdd, addButton, onSort,
    onDelete, onDuplicate,
  } = ctx;

  if (!S.dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filtered = applyFilters(records, valuesMap, S.dbvFilters[tabId], idField, tabId);
  const visProps = customProps.filter(p => p.visible !== false);
  const hasActions = !!(onDelete || onDuplicate);
  const reload = reloadFn || (() => {});
  const colCount = (hasActions ? 1 : 0) + fixedColumns.length + visProps.length + 1;

  // Header — with initial widths
  const W = { done: 30, title: 180, projectName: 100, priority: 100, status: 100, date: 100, tags: 120, name: 180, category: 100, quantity: 70, location: 100, needed: 90 };
  const thCheck = hasActions ? '<th class="col-check-header"><input type="checkbox"></th>' : '';
  const thFixed = fixedColumns.map(c => {
    const w = W[c.key] || 140;
    if (!c.label) return `<th class="sortable-header" data-sort="${c.key}" style="width:${w}px"></th>`;
    return `<th class="sortable-header" data-sort="${c.key}" style="width:${w}px"><div class="th-content"><span class="col-type-icon">${getTypeIcon(c.editType || 'text')}</span>${c.label}</div></th>`;
  }).join('');
  const thCustom = visProps.map(p =>
    `<th class="sortable-header prop-header" data-sort="prop_${p.id}" data-prop-id="${p.id}" style="width:180px"><div class="th-content"><span class="col-type-icon">${getTypeIcon(p.type)}</span>${escapeHtml(p.name)}</div></th>`
  ).join('');

  // Body
  let tbody = '';
  for (const r of filtered) {
    const rid = r[idField];
    const tdCheck = hasActions ? '<td class="col-check"><input type="checkbox"></td>' : '';
    const tdFixed = fixedColumns.map(c => {
      const val = c.render ? c.render(r) : escapeHtml(String(r[c.key] ?? ''));
      return `<td>${val}</td>`;
    }).join('');
    const tdCustom = visProps.map(p => {
      const autoVals = { created_time: r.created_at, last_edited: r.updated_at, unique_id: rid };
      const raw = autoVals[p.type] ?? valuesMap[rid]?.[p.id] ?? '';
      return `<td class="cell-editable" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}' data-raw-value="${escapeHtml(String(raw))}">${formatPropValue(raw, p)}</td>`;
    }).join('');
    tbody += `<tr class="data-table-row" data-id="${rid}">${tdCheck}${tdFixed}${tdCustom}<td></td></tr>`;
  }

  if (filtered.length === 0) {
    tbody = `<tr><td colspan="${colCount}" style="text-align:center;color:var(--text-faint);padding:24px;">Пока пусто</td></tr>`;
  }

  // Add-row + footer
  const addRowHtml = onAdd ? `<tr class="dbv-add-row"><td colspan="${colCount}"><div class="dbv-add-row-label"><span class="dbv-add-row-plus">+</span> Новая запись</div></td></tr>` : '';
  const footerHtml = `<div class="dbv-table-footer"><span>Записей: ${filtered.length}</span></div>`;

  el.innerHTML = `<div class="dbv-table-wrap"><table class="data-table database-view"><thead><tr>${thCheck}${thFixed}${thCustom}<th class="add-prop-col dbv-add-prop-col" title="Добавить свойство">+</th></tr></thead><tbody>${tbody}${addRowHtml}</tbody></table>${footerHtml}</div>`;

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
  if (tableEl) enableColumnDrag(tableEl, tabId, reload);

  // Add-row click
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

  // Inline editing
  el.querySelectorAll('.cell-editable').forEach(cell => {
    cell.setAttribute('tabindex', '0');
    cell.addEventListener('click', (e) => { e.stopPropagation(); focusCell(el, cell); startInlineEdit(cell, recordTable, reload); });
    cell.addEventListener('focus', () => focusCell(el, cell));
  });

  // Column interactions
  el.querySelector('.dbv-add-prop-col')?.addEventListener('click', (e) => showAddPropertyPopover(tabId, e.target, reload));
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      e.stopPropagation();
      const prop = customProps.find(p => p.id === parseInt(th.dataset.propId));
      if (prop) showColumnMenu(prop, th.getBoundingClientRect(), tabId, reload, onSort);
    });
  });

  // Sorting on fixed headers
  el.querySelectorAll('.sortable-header:not(.prop-header)').forEach(th => {
    th.addEventListener('click', (e) => {
      if (e.target.closest('.col-resize-handle')) return;
      const dir = th.dataset.dir === 'asc' ? 'desc' : 'asc';
      el.querySelectorAll('.sortable-header').forEach(h => { h.dataset.dir = 'none'; h.classList.remove('sort-asc', 'sort-desc'); });
      th.dataset.dir = dir; th.classList.add(`sort-${dir}`);
      if (onSort) onSort(th.dataset.sort, dir);
    });
  });
}

function focusCell(container, cell) {
  container.querySelectorAll('.cell-focused').forEach(c => c.classList.remove('cell-focused'));
  cell.classList.add('cell-focused');
}
