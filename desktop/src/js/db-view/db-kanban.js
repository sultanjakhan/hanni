// ── db-view/db-kanban.js — Generic kanban view renderer ──

import { escapeHtml } from '../utils.js';

/**
 * Render a kanban view into a container element.
 *
 * @param {HTMLElement} el - Container element
 * @param {object} ctx - Context: { records, idField, kanban, fixedColumns, onRowClick, onAdd, addButton, onDrop }
 *   kanban: { groupByField, columns: [{ key, label, icon?, color? }] }
 */
export function renderKanbanView(el, ctx) {
  const {
    records, idField = 'id', kanban = {}, fixedColumns = [],
    onRowClick, onAdd, addButton, onDrop,
  } = ctx;

  const { groupByField = 'status', columns = [] } = kanban;

  if (columns.length === 0) {
    el.innerHTML = '<div style="color:var(--text-faint);padding:24px;text-align:center;">Kanban requires column config</div>';
    return;
  }

  // Group records
  const grouped = {};
  for (const col of columns) grouped[col.key] = [];
  for (const rec of records) {
    const val = rec[groupByField] ?? '';
    const colKey = columns.find(c => c.key === val)?.key || columns[0]?.key;
    if (grouped[colKey]) grouped[colKey].push(rec);
  }

  // Render columns
  const boardHtml = columns.map(col => {
    const items = grouped[col.key] || [];
    const icon = col.icon || '';
    const cardsHtml = items.map(rec => renderKanbanCard(rec, fixedColumns, idField)).join('');

    return `<div class="dbv-kanban-column" data-group="${escapeHtml(col.key)}">
      <div class="kanban-column-header">
        <span>${icon ? icon + ' ' : ''}${escapeHtml(col.label)}</span>
        <span class="kanban-column-count">${items.length}</span>
      </div>
      <div class="dbv-kanban-cards" data-group="${escapeHtml(col.key)}">
        ${cardsHtml}
      </div>
    </div>`;
  }).join('');

  el.innerHTML = `${addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : ''}
    <div class="kanban-board dbv-kanban-board">${boardHtml}</div>`;

  // Card clicks
  if (onRowClick) {
    el.querySelectorAll('.dbv-kanban-card').forEach(card => {
      card.addEventListener('click', () => {
        const rid = card.dataset.id;
        const rec = records.find(r => String(r[idField]) === rid);
        if (rec) onRowClick(rec);
      });
    });
  }

  // Add button
  if (addButton && onAdd) {
    el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
  }

  // Drag & drop between columns
  setupKanbanDnD(el, records, idField, groupByField, onDrop);
}

function renderKanbanCard(rec, fixedColumns, idField) {
  const titleCol = fixedColumns[0];
  const title = titleCol
    ? (titleCol.render ? titleCol.render(rec) : escapeHtml(String(rec[titleCol.key] ?? '')))
    : escapeHtml(String(rec.title || rec.name || rec[idField]));

  const badges = fixedColumns.slice(1).map(c => {
    const val = c.render ? c.render(rec) : escapeHtml(String(rec[c.key] ?? ''));
    return val ? `<span class="dbv-kanban-card-meta">${val}</span>` : '';
  }).join('');

  return `<div class="dbv-kanban-card card" data-id="${rec[idField]}" draggable="true">
    <div class="dbv-kanban-card-title">${title}</div>
    ${badges ? `<div class="dbv-kanban-card-badges">${badges}</div>` : ''}
  </div>`;
}

function setupKanbanDnD(el, records, idField, groupByField, onDrop) {
  // Card drag start
  el.querySelectorAll('.dbv-kanban-card').forEach(card => {
    card.addEventListener('dragstart', (e) => {
      e.dataTransfer.setData('text/plain', card.dataset.id);
      card.classList.add('dragging');
    });
    card.addEventListener('dragend', () => card.classList.remove('dragging'));
  });

  // Column drop zones
  el.querySelectorAll('.dbv-kanban-cards').forEach(col => {
    col.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      col.classList.add('drag-over');
    });
    col.addEventListener('dragleave', (e) => {
      if (!col.contains(e.relatedTarget)) col.classList.remove('drag-over');
    });
    col.addEventListener('drop', (e) => {
      e.preventDefault();
      e.stopPropagation();
      col.classList.remove('drag-over');
      const rid = e.dataTransfer.getData('text/plain');
      const targetGroup = col.dataset.group;
      if (rid && targetGroup && onDrop) {
        onDrop(rid, groupByField, targetGroup);
      }
    });
  });
}
