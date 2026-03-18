import { S, invoke, getTypeIcon } from '../state.js';
import { escapeHtml } from '../utils.js';
import { formatPropValue, startInlineEdit } from './db-cell-editors.js';
import { renderFilterBar, applyFilters, loadFiltersFromViewConfig } from './db-filters.js';
import { showAddPropertyModal, showColumnMenu } from './db-properties.js';
import { enableColumnDrag } from './db-col-drag.js';

/** Render a table view into a container element */
export async function renderTableView(el, ctx) {
  const {
    tabId, recordTable, records, fixedColumns = [], idField = 'id',
    customProps = [], valuesMap = {}, reloadFn, onRowClick, onAdd, addButton, onSort, sortRules = [],
  } = ctx;

  // Load and apply filters
  if (!S.dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filteredRecords = applyFilters(records, valuesMap, S.dbvFilters[tabId], idField);
  const visibleProps = customProps.filter(p => p.visible !== false);

  const headerHtml = addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : '';

  // Table head
  const thFixed = fixedColumns.map(c =>
    `<th class="sortable-header" data-sort="${c.key}">${c.label}</th>`
  ).join('');
  const thCustom = visibleProps.map(p =>
    `<th class="sortable-header prop-header" data-sort="prop_${p.id}" data-prop-id="${p.id}"><span class="col-type-icon">${getTypeIcon(p.type)}</span>${escapeHtml(p.name)}</th>`
  ).join('');
  const thAddCol = `<th class="add-prop-col dbv-add-prop-col" title="Добавить свойство">+</th>`;

  // Table body
  let tbodyHtml = '';
  for (const record of filteredRecords) {
    const rid = record[idField];
    const tdFixed = fixedColumns.map(c => {
      const val = c.render ? c.render(record) : escapeHtml(String(record[c.key] ?? ''));
      return `<td>${val}</td>`;
    }).join('');

    const tdCustom = visibleProps.map(p => {
      const rawVal = valuesMap[rid]?.[p.id] ?? '';
      const displayVal = formatPropValue(rawVal, p);
      return `<td class="cell-editable" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}'>${displayVal}</td>`;
    }).join('');

    tbodyHtml += `<tr class="data-table-row" data-id="${rid}">${tdFixed}${tdCustom}<td></td></tr>`;
  }

  if (filteredRecords.length === 0) {
    const colspan = fixedColumns.length + visibleProps.length + 1;
    tbodyHtml = `<tr><td colspan="${colspan}" style="text-align:center;color:var(--text-faint);padding:24px;">Пока пусто</td></tr>`;
  }

  // Inline "new row" footer
  const colspan = fixedColumns.length + visibleProps.length + 1;
  const countLabel = `<span class="table-count">Всего: ${filteredRecords.length}</span>`;
  const addBtn = onAdd ? `<span class="add-row-btn">+ Новая запись</span>` : '';
  const footerHtml = `<tfoot><tr class="add-row-inline"><td colspan="${colspan}">${addBtn}${countLabel}</td></tr></tfoot>`;

  el.innerHTML = headerHtml + `
    <table class="data-table database-view">
      <thead><tr>${thFixed}${thCustom}${thAddCol}</tr></thead>
      <tbody>${tbodyHtml}</tbody>
      ${footerHtml}
    </table>`;

  // Filter bar
  if (customProps.length > 0) {
    renderFilterBar(el, tabId, customProps, reloadFn || (() => {}));
  }

  // Row click
  if (onRowClick) {
    el.querySelectorAll('.data-table-row').forEach(row => {
      row.style.cursor = 'pointer';
      row.addEventListener('click', (e) => {
        if (e.target.closest('.cell-editable') && e.target.closest('.inline-editor')) return;
        const id = parseInt(row.dataset.id);
        const record = filteredRecords.find(r => r[idField] === id);
        if (record) onRowClick(record);
      });
    });
  }

  // Cell focus + inline editing
  const allCells = [...el.querySelectorAll('.cell-editable')];
  allCells.forEach(cell => {
    cell.setAttribute('tabindex', '0');
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      focusCell(el, cell);
      startInlineEdit(cell, recordTable, reloadFn);
    });
    cell.addEventListener('focus', () => focusCell(el, cell));
    cell.addEventListener('keydown', (e) => {
      if (cell.querySelector('.inline-editor')) return;
      const idx = allCells.indexOf(cell);
      const cols = visibleProps.length;
      const nav = { Tab: e.shiftKey ? -1 : 1, Enter: cols, ArrowRight: 1, ArrowLeft: -1, ArrowDown: cols, ArrowUp: -cols };
      const offset = nav[e.key];
      if (offset == null) return;
      e.preventDefault();
      const next = allCells[idx + offset];
      if (next) { next.focus(); if (e.key === 'Tab' || e.key === 'Enter') next.click(); }
    });
  });

  // Add property column
  el.querySelector('.dbv-add-prop-col')?.addEventListener('click', () => {
    showAddPropertyModal(tabId, reloadFn);
  });

  // Property header context menu
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      e.stopPropagation();
      const prop = customProps.find(p => p.id === parseInt(th.dataset.propId));
      if (prop) showColumnMenu(prop, th.getBoundingClientRect(), tabId, reloadFn, onSort);
    });
  });
  // Add buttons
  if (addButton && onAdd) el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
  el.querySelector('.add-row-btn')?.addEventListener('click', () => { if (onAdd) onAdd(); });

  // Column drag-and-drop reorder
  const tableEl = el.querySelector('.data-table');
  if (tableEl) enableColumnDrag(tableEl, tabId, reloadFn);

  // Sort headers — Shift+click for multi-level sort
  applySortIndicators(el, sortRules);
  el.querySelectorAll('.sortable-header:not(.prop-header)').forEach(th => {
    th.addEventListener('click', (e) => {
      const key = th.dataset.sort;
      const cur = sortRules.find(r => r.key === key);
      const dir = cur?.dir === 'asc' ? 'desc' : 'asc';
      if (onSort) onSort(key, dir, e.shiftKey);
    });
  });
}

function focusCell(container, cell) {
  container.querySelectorAll('.cell-focused').forEach(c => c.classList.remove('cell-focused'));
  cell.classList.add('cell-focused');
}

function applySortIndicators(el, rules) {
  el.querySelectorAll('.sortable-header').forEach(h => { h.classList.remove('sort-asc', 'sort-desc'); delete h.dataset.sortLevel; });
  rules.forEach((r, i) => { const th = el.querySelector(`[data-sort="${r.key}"]`); if (th) { th.classList.add(`sort-${r.dir}`); if (rules.length > 1) th.dataset.sortLevel = i + 1; } });
}
