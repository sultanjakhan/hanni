// ── db-view/db-cell-format.js — Cell display formatting ──

import { escapeHtml } from '../utils.js';
import { formatRecurrence } from './db-recurrence-editor.js';
import { normalizeOptions, colorForValue } from './db-dropdowns.js';

function selectBadgeColor(val, prop) {
  let raw = [];
  try { raw = JSON.parse(prop.options || '[]'); } catch {}
  return colorForValue(val, normalizeOptions(raw));
}

export function formatPropValue(val, prop) {
  if (!val && val !== 0) return '<span class="text-faint">\u2014</span>';
  if (prop.type === 'checkbox') return `<span class="cell-check-round${val === 'true' ? ' checked' : ''}"></span>`;
  if (prop.type === 'select' || prop.type === 'multi_select' || prop.type === 'status') {
    let items = [];
    try { items = JSON.parse(val); if (!Array.isArray(items)) items = [val]; } catch { items = [val]; }
    return items.map(i => `<span class="badge badge-${selectBadgeColor(i, prop)}">${escapeHtml(i)}</span>`).join(' ');
  }
  if (prop.type === 'url') return `<a href="${escapeHtml(val)}" target="_blank" class="cell-link">${escapeHtml(val.length > 30 ? val.substring(0, 30) + '...' : val)}</a>`;
  if (prop.type === 'email') return `<a href="mailto:${escapeHtml(val)}" class="cell-link">${escapeHtml(val)}</a>`;
  if (prop.type === 'phone') return `<a href="tel:${escapeHtml(val)}" class="cell-link">${escapeHtml(val)}</a>`;
  if (prop.type === 'date') { const d = new Date(val); return isNaN(d) ? escapeHtml(val) : `<span class="cell-date">${d.toLocaleDateString('ru-RU')}</span>`; }
  if (prop.type === 'created_time' || prop.type === 'last_edited') {
    const d = new Date(val); return isNaN(d) ? escapeHtml(val) : `<span class="cell-date text-faint">${d.toLocaleDateString('ru-RU')} ${d.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}</span>`;
  }
  if (prop.type === 'recurrence') return `<span class="cell-recurrence">${escapeHtml(formatRecurrence(val))}</span>`;
  if (prop.type === 'time') return `<span class="cell-time">${escapeHtml(val)}</span>`;
  if (prop.type === 'progress') {
    const n = parseInt(val) || 0;
    return `<div class="cell-progress"><div class="cell-progress-track"><div class="cell-progress-bar" style="width:${n}%"></div></div><span class="cell-progress-num">${n}%</span></div>`;
  }
  if (prop.type === 'rating') {
    const n = parseInt(val) || 0;
    return Array.from({ length: 5 }, (_, i) => `<span class="cell-star${i < n ? ' filled' : ''}">★</span>`).join('');
  }
  return escapeHtml(val);
}
