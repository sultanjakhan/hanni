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
    customProps = [], valuesMap = {},
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
    return renderDefaultCard(rec, fixedColumns, idField, customProps, valuesMap);
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

const IMG_RE = /\.(jpg|jpeg|png|gif|webp|svg|bmp)(\?|$)/i;

function findCoverUrl(rec, fixedColumns, customProps, valuesMap, idField) {
  // Check URL-type fixed columns
  for (const c of fixedColumns) {
    const v = String(rec[c.key] ?? '');
    if (v && IMG_RE.test(v)) return v;
  }
  // Check URL-type custom properties
  const rv = valuesMap[rec[idField]];
  if (rv) {
    for (const p of (customProps || []).filter(p => p.prop_type === 'url' && p.visible !== false)) {
      const v = String(rv[p.id] ?? '');
      if (v && IMG_RE.test(v)) return v;
    }
  }
  return null;
}

function renderDefaultCard(rec, fixedColumns, idField, customProps, valuesMap) {
  const titleCol = fixedColumns[0];
  const title = titleCol
    ? (titleCol.render ? titleCol.render(rec) : escapeHtml(String(rec[titleCol.key] ?? '')))
    : escapeHtml(String(rec.title || rec.name || rec[idField]));

  const coverUrl = findCoverUrl(rec, fixedColumns, customProps, valuesMap, idField);
  const coverHtml = coverUrl ? `<div class="dbv-gallery-cover"><img src="${escapeHtml(coverUrl)}" alt="" loading="lazy"></div>` : '';

  const badges = fixedColumns.slice(1, 4).map(c => {
    const val = c.render ? c.render(rec) : escapeHtml(String(rec[c.key] ?? ''));
    if (!val) return '';
    if (val.includes('class=')) return `<span class="dbv-card-badge">${val}</span>`;
    return `<span class="dbv-card-badge badge badge-gray">${val}</span>`;
  }).filter(Boolean).join('');

  return `<div class="dbv-gallery-card" data-id="${rec[idField]}">
    ${coverHtml}
    <div class="dbv-gallery-card-title">${title}</div>
    ${badges ? `<div class="dbv-gallery-card-meta">${badges}</div>` : ''}
  </div>`;
}
