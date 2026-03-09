// ── db-view/db-gallery.js — Notion-style gallery view ──

import { escapeHtml } from '../utils.js';

/**
 * Render a gallery grid.
 *
 * @param {HTMLElement} el - Container
 * @param {object} ctx - { records, idField, fixedColumns, onRowClick, onAdd, addButton, gallery }
 *   gallery: { renderCard?, minCardWidth? }
 */
export function renderGalleryView(el, ctx) {
  const {
    records, idField = 'id', fixedColumns = [],
    onRowClick, onAdd, addButton,
    gallery = {},
  } = ctx;

  const { renderCard, minCardWidth = 200 } = gallery;

  if (records.length === 0) {
    el.innerHTML = `${addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : ''}
      <div class="empty-state">
        <div class="empty-state-icon">🖼</div>
        <div class="empty-state-text">Пока пусто</div>
      </div>`;
    if (addButton && onAdd) el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
    return;
  }

  const cardsHtml = records.map(rec => {
    if (renderCard) {
      return `<div class="dbv-gallery-card" data-id="${rec[idField]}">${renderCard(rec)}</div>`;
    }
    return renderDefaultCard(rec, fixedColumns, idField);
  }).join('');

  el.innerHTML = `${addButton ? `<div class="dbv-header"><button class="btn-primary dbv-add-btn">${addButton}</button></div>` : ''}
    <div class="dbv-gallery-grid" style="grid-template-columns:repeat(auto-fill,minmax(${minCardWidth}px,1fr));">${cardsHtml}</div>`;

  if (onRowClick) {
    el.querySelectorAll('.dbv-gallery-card').forEach(card => {
      card.addEventListener('click', () => {
        const rid = card.dataset.id;
        const rec = records.find(r => String(r[idField]) === rid);
        if (rec) onRowClick(rec);
      });
    });
  }

  if (addButton && onAdd) {
    el.querySelector('.dbv-add-btn')?.addEventListener('click', onAdd);
  }
}

function renderDefaultCard(rec, fixedColumns, idField) {
  const titleCol = fixedColumns[0];
  const title = titleCol
    ? (titleCol.render ? titleCol.render(rec) : escapeHtml(String(rec[titleCol.key] ?? '')))
    : escapeHtml(String(rec.title || rec.name || rec[idField]));

  const badges = fixedColumns.slice(1, 4).map(c => {
    const val = c.render ? c.render(rec) : escapeHtml(String(rec[c.key] ?? ''));
    if (!val) return '';
    if (val.includes('class=')) return `<span class="dbv-card-badge">${val}</span>`;
    return `<span class="dbv-card-badge badge badge-gray">${val}</span>`;
  }).filter(Boolean).join('');

  return `<div class="dbv-gallery-card" data-id="${rec[idField]}">
    <div class="dbv-gallery-card-title">${title}</div>
    ${badges ? `<div class="dbv-gallery-card-meta">${badges}</div>` : ''}
  </div>`;
}
