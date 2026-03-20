// ── db-view/db-side-peek.js — Side panel for record detail (Notion-style) ──

import { escapeHtml } from '../utils.js';
import { invoke } from '../state.js';

// Format ISO date to human-readable Russian
const MONTHS = ['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'];
function fmtDate(d) {
  if (!d) return '';
  const p = d.split('-');
  if (p.length !== 3) return d;
  return `${parseInt(p[2])} ${MONTHS[parseInt(p[1])-1]} ${p[0]}`;
}

/**
 * Show a side-peek panel for a record.
 * @param {Object} rec - The record data
 * @param {Object} ctx - { fixedColumns, customProps, valuesMap, recordTable, idField, reloadFn }
 */
export function showSidePeek(rec, ctx) {
  closeSidePeek();
  const { fixedColumns = [], customProps = [], valuesMap = {}, recordTable, idField = 'id', reloadFn } = ctx;
  const rid = rec[idField];

  const panel = document.createElement('div');
  panel.className = 'dbv-side-peek';
  panel.innerHTML = buildPeekContent(rec, rid, fixedColumns, customProps, valuesMap);

  // Close button
  const closeBtn = panel.querySelector('.dbv-sp-close');
  closeBtn?.addEventListener('click', closeSidePeek);

  // Inline editing on property values
  panel.querySelectorAll('.dbv-sp-value[contenteditable]').forEach(el => {
    el.addEventListener('blur', async () => {
      const key = el.dataset.key;
      const val = el.textContent.trim();
      if (el.dataset.propId) {
        await invoke('set_property_value', {
          recordId: rid, recordTable, propertyId: parseInt(el.dataset.propId), value: val || null,
        }).catch(() => {});
      }
      if (reloadFn) reloadFn();
    });
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); el.blur(); }
      if (e.key === 'Escape') { closeSidePeek(); }
    });
  });

  // Overlay
  const overlay = document.createElement('div');
  overlay.className = 'dbv-sp-overlay';
  overlay.addEventListener('click', closeSidePeek);

  document.body.appendChild(overlay);
  document.body.appendChild(panel);
  requestAnimationFrame(() => { panel.classList.add('open'); overlay.classList.add('open'); });
}

export function closeSidePeek() {
  document.querySelector('.dbv-side-peek')?.remove();
  document.querySelector('.dbv-sp-overlay')?.remove();
}

function buildPeekContent(rec, rid, fixedColumns, customProps, valuesMap) {
  const titleCol = fixedColumns[0];
  const title = titleCol ? escapeHtml(String(rec[titleCol.key] || '')) : `#${rid}`;

  let propsHtml = '';
  // Fixed columns (skip first = title)
  for (const col of fixedColumns.slice(1)) {
    const val = rec[col.key] ?? '';
    propsHtml += `<div class="dbv-sp-row">
      <div class="dbv-sp-label">${escapeHtml(col.label)}</div>
      <div class="dbv-sp-value" data-key="${col.key}">${escapeHtml(String(val))}</div>
    </div>`;
  }

  // Custom properties
  const visProp = customProps.filter(p => p.visible !== false);
  for (const p of visProp) {
    const val = valuesMap[rid]?.[p.id] ?? '';
    const isDate = p.type === 'date';
    const display = isDate && val ? fmtDate(String(val)) : escapeHtml(String(val));
    const editable = isDate ? '' : ' contenteditable="true"';
    propsHtml += `<div class="dbv-sp-row">
      <div class="dbv-sp-label">${escapeHtml(p.name)}</div>
      <div class="dbv-sp-value"${editable} data-prop-id="${p.id}" data-key="prop_${p.id}">${display}</div>
    </div>`;
  }

  return `
    <div class="dbv-sp-header">
      <div class="dbv-sp-title">${title}</div>
      <button class="dbv-sp-close dbv-action-btn">&times;</button>
    </div>
    <div class="dbv-sp-body">${propsHtml || '<div class="dbv-sp-empty">Нет свойств</div>'}</div>`;
}
