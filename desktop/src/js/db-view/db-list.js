// ── db-view/db-list.js — List view renderer ──

import { escapeHtml } from '../utils.js';

/**
 * Render a list view into a container element.
 *
 * @param {HTMLElement} el - Container element
 * @param {object} ctx - Context: { records, idField, fixedColumns, onRowClick, onAdd, addButton }
 */
export function renderListView(el, ctx) {
  const {
    records, idField = 'id', fixedColumns = [],
    onRowClick, onAdd, addButton,
  } = ctx;

  if (records.length === 0) {
    el.innerHTML = `${addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : ''}
      <div class="empty-state">
        <div class="empty-state-icon">\ud83d\udcdd</div>
        <div class="empty-state-text">Пока пусто</div>
      </div>`;
    if (addButton && onAdd) el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
    return;
  }

  const titleCol = fixedColumns[0];
  const metaCols = fixedColumns.slice(1);

  const listHtml = records.map(rec => {
    const title = titleCol
      ? (titleCol.render ? titleCol.render(rec) : escapeHtml(String(rec[titleCol.key] ?? '')))
      : escapeHtml(String(rec.title || rec.name || rec[idField]));

    const metaHtml = metaCols.map(c => {
      const val = c.render ? c.render(rec) : escapeHtml(String(rec[c.key] ?? ''));
      return val ? `<span class="dbv-list-meta">${val}</span>` : '';
    }).join('');

    return `<div class="dbv-list-item" data-id="${rec[idField]}">
      <div class="dbv-list-title">${title}</div>
      ${metaHtml ? `<div class="dbv-list-meta-row">${metaHtml}</div>` : ''}
    </div>`;
  }).join('');

  el.innerHTML = `${addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : ''}
    <div class="dbv-list">${listHtml}</div>`;

  // Row click
  if (onRowClick) {
    el.querySelectorAll('.dbv-list-item').forEach(item => {
      item.addEventListener('click', () => {
        const rid = item.dataset.id;
        const rec = records.find(r => String(r[idField]) === rid);
        if (rec) onRowClick(rec);
      });
    });
  }

  // Add button
  if (addButton && onAdd) {
    el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
  }
}
