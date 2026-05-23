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
  if (prop.type === 'overdue') {
    return val === 'overdue'
      ? '<span class="cell-overdue" title="\u0412\u0440\u0435\u043c\u044f \u043f\u0440\u043e\u0448\u043b\u043e, \u0437\u0430\u0434\u0430\u0447\u0430 \u043d\u0435 \u0432\u044b\u043f\u043e\u043b\u043d\u0435\u043d\u0430">\u26a0\ufe0f \u041f\u0440\u043e\u0441\u0440\u043e\u0447\u0435\u043d\u043e</span>'
      : '<span class="text-faint">\u2014</span>';
  }
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
  if (prop.type === 'minutes') {
    const n = parseInt(val);
    return isNaN(n) ? '<span class="text-faint">—</span>' : `<span class="cell-minutes">${n} мин</span>`;
  }
  return escapeHtml(val);
}
