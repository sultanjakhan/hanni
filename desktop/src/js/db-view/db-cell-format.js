// ── db-view/db-cell-format.js — Cell display formatting ──

import { escapeHtml } from '../utils.js';

const BADGE_COLORS = ['blue', 'green', 'yellow', 'red', 'purple', 'orange', 'pink', 'gray'];

function badgeColor(val, prop) {
  let opts = []; try { opts = JSON.parse(prop.options || '[]'); } catch {}
  const idx = opts.indexOf(val);
  return BADGE_COLORS[idx >= 0 ? idx % BADGE_COLORS.length : 0];
}

export function formatPropValue(val, prop) {
  if (!val && val !== 0) return '<span class="text-faint">\u2014</span>';
  if (prop.type === 'checkbox') return `<span class="cell-check-round${val === 'true' ? ' checked' : ''}"></span>`;
  if (prop.type === 'select') return `<span class="badge badge-${badgeColor(val, prop)}">${escapeHtml(val)}</span>`;
  if (prop.type === 'multi_select') {
    try { return JSON.parse(val).map(i => `<span class="badge badge-${badgeColor(i, prop)}">${escapeHtml(i)}</span>`).join(' '); }
    catch { return escapeHtml(val); }
  }
  if (prop.type === 'url') return `<a href="${escapeHtml(val)}" target="_blank" class="cell-link">${escapeHtml(val.length > 30 ? val.substring(0, 30) + '...' : val)}</a>`;
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
