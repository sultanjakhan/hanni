// ── db-view/db-kanban.js — Notion-style kanban view ──

import { escapeHtml } from '../utils.js';

/**
 * Render a kanban board.
 *
 * @param {HTMLElement} el - Container
 * @param {object} ctx - { records, idField, kanban, fixedColumns, onRowClick, onAdd, addButton, onDrop }
 *   kanban: { groupByField, columns: [{ key, label, icon?, color? }] }
 */
export function renderKanbanView(el, ctx) {
  const {
    records, idField = 'id', kanban = {}, fixedColumns = [],
    onRowClick, onAdd, addButton, onDrop,
  } = ctx;

  const { groupByField = 'status', columns = [] } = kanban;

  if (columns.length === 0) {
    el.innerHTML = '<div class="empty-state"><div class="empty-state-icon">📋</div><div class="empty-state-text">Канбан не настроен</div></div>';
    return;
  }

  // Group records by column
  const grouped = {};
  for (const col of columns) grouped[col.key] = [];
  for (const rec of records) {
    const val = rec[groupByField] ?? '';
    const colKey = columns.find(c => c.key === val)?.key || columns[0]?.key;
    if (grouped[colKey]) grouped[colKey].push(rec);
  }

  const boardHtml = columns.map(col => {
    const items = grouped[col.key] || [];
    const colorDot = col.color ? `<span class="dbv-col-dot" style="background:var(--color-${col.color}, ${col.color})"></span>` : '';
    const icon = col.icon ? `<span class="dbv-col-icon">${col.icon}</span>` : '';
    const cardsHtml = items.length > 0
      ? items.map(rec => renderCard(rec, fixedColumns, idField)).join('')
      : '<div class="dbv-kanban-empty">Нет записей</div>';

    return `<div class="dbv-kanban-column" data-group="${escapeHtml(col.key)}">
      <div class="dbv-kanban-col-header">
        <div class="dbv-kanban-col-title">${colorDot}${icon}${escapeHtml(col.label)}</div>
        <span class="dbv-kanban-col-count">${items.length}</span>
      </div>
      <div class="dbv-kanban-cards" data-group="${escapeHtml(col.key)}">
        ${cardsHtml}
      </div>
    </div>`;
  }).join('');

  el.innerHTML = `${addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : ''}
    <div class="dbv-kanban-board">${boardHtml}</div>`;

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

  // Drag & drop
  setupKanbanDnD(el, records, idField, groupByField, onDrop);
}

function renderCard(rec, fixedColumns, idField) {
  const titleCol = fixedColumns[0];
  const title = titleCol
    ? (titleCol.render ? titleCol.render(rec) : escapeHtml(String(rec[titleCol.key] ?? '')))
    : escapeHtml(String(rec.title || rec.name || rec[idField]));

  const badges = fixedColumns.slice(1).map(c => {
    const val = c.render ? c.render(rec) : escapeHtml(String(rec[c.key] ?? ''));
    if (!val) return '';
    // If the render already returns HTML with classes, use as-is; otherwise wrap in a badge
    if (val.includes('class=')) return `<span class="dbv-card-badge">${val}</span>`;
    return `<span class="dbv-card-badge badge badge-gray">${val}</span>`;
  }).filter(Boolean).join('');

  return `<div class="dbv-kanban-card" data-id="${rec[idField]}" draggable="true">
    <div class="dbv-kanban-card-title">${title}</div>
    ${badges ? `<div class="dbv-kanban-card-meta">${badges}</div>` : ''}
  </div>`;
}

function setupKanbanDnD(el, records, idField, groupByField, onDrop) {
  el.querySelectorAll('.dbv-kanban-card').forEach(card => {
    card.addEventListener('dragstart', (e) => {
      e.dataTransfer.setData('text/plain', card.dataset.id);
      card.classList.add('dragging');
    });
    card.addEventListener('dragend', () => card.classList.remove('dragging'));
  });

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
