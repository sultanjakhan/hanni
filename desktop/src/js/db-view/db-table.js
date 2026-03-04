// ── db-view/db-table.js — Table view renderer ──

import { S, invoke, getTypeIcon } from '../state.js';
import { escapeHtml } from '../utils.js';
import { formatPropValue, startInlineEdit } from './db-cell-editors.js';
import { renderFilterBar, applyFilters, loadFiltersFromViewConfig } from './db-filters.js';
import { showAddPropertyModal, showColumnMenu } from './db-properties.js';

/**
 * Render a table view into a container element.
 *
 * @param {HTMLElement} el - Container element
 * @param {object} ctx - Context: { tabId, recordTable, records, fixedColumns, idField, customProps, valuesMap, reloadFn, onRowClick, onAdd, addButton, onSort }
 */
export async function renderTableView(el, ctx) {
  const {
    tabId, recordTable, records, fixedColumns = [], idField = 'id',
    customProps = [], valuesMap = {}, reloadFn, onRowClick, onAdd, addButton, onSort,
  } = ctx;

  // Load and apply filters
  if (!S.dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filteredRecords = applyFilters(records, valuesMap, S.dbvFilters[tabId], idField);
  const visibleProps = customProps.filter(p => p.visible !== false);

  // Header
  const headerHtml = addButton
    ? `<div class="dbv-header">${addButton ? `<button class="btn-primary dbv-add-btn">${addButton}</button>` : ''}</div>`
    : '';

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

  el.innerHTML = headerHtml + `
    <table class="data-table database-view">
      <thead><tr>${thFixed}${thCustom}${thAddCol}</tr></thead>
      <tbody>${tbodyHtml}</tbody>
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

  // Inline editing
  el.querySelectorAll('.cell-editable').forEach(cell => {
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      startInlineEdit(cell, recordTable, reloadFn);
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
      const propId = parseInt(th.dataset.propId);
      const prop = customProps.find(p => p.id === propId);
      if (!prop) return;
      const rect = th.getBoundingClientRect();
      showColumnMenu(prop, rect, tabId, reloadFn, onSort);
    });
  });

  // Add button
  if (addButton && onAdd) {
    el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
  }

  // Fixed column sorting
  el.querySelectorAll('.sortable-header:not(.prop-header)').forEach(th => {
    th.addEventListener('click', () => {
      const sortKey = th.dataset.sort;
      const currentDir = th.dataset.dir || 'none';
      const newDir = currentDir === 'asc' ? 'desc' : 'asc';
      el.querySelectorAll('.sortable-header').forEach(h => { h.dataset.dir = 'none'; h.classList.remove('sort-asc', 'sort-desc'); });
      th.dataset.dir = newDir;
      th.classList.add(`sort-${newDir}`);
      if (onSort) onSort(sortKey, newDir);
    });
  });
}
