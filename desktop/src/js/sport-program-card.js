// ── sport-program-card.js — Program card + muscle-balance bar ──
import { escapeHtml } from './utils.js';
import { KIND_LABELS, KIND_COLORS } from './sport-program-filters.js';
import { MUSCLE_GROUPS, MUSCLE_LABELS, MUSCLE_COLORS } from './sport-catalog-filters.js';

// Stacked bar of per-muscle-group training volume (SUM of sets). Zero groups
// render as muted slots so neglected muscles are visible ("соблюдать баланс").
export function renderBalanceBar(volumeMap) {
  const groups = MUSCLE_GROUPS.filter(g => g.id !== 'all' && g.id !== 'full_body');
  const total = groups.reduce((s, g) => s + (volumeMap?.[g.id] || 0), 0);
  if (!total) return '<div class="balance-empty-note">Добавьте упражнения из каталога в шаблоны, чтобы увидеть баланс</div>';
  const segs = groups.map(g => {
    const v = volumeMap?.[g.id] || 0;
    const color = v ? `var(--color-${MUSCLE_COLORS[g.id] || 'gray'}, #888)` : 'var(--border-default)';
    return `<div class="balance-seg" style="flex:${v || 0.12};background:${color}"
      title="${MUSCLE_LABELS[g.id]}: ${v}"></div>`;
  }).join('');
  const legend = groups.filter(g => volumeMap?.[g.id]).map(g =>
    `<span class="balance-leg"><i style="background:var(--color-${MUSCLE_COLORS[g.id] || 'gray'}, #888)"></i>${MUSCLE_LABELS[g.id]} ${volumeMap[g.id]}</span>`
  ).join('');
  return `<div class="balance-bar">${segs}</div><div class="balance-legend">${legend}</div>`;
}

export function renderProgramCard(p) {
  const div = document.createElement('div');
  div.className = 'sport-card' + (p.favorite === 1 ? ' recipe-fav' : '') + (p.active === 1 ? ' program-active' : '');
  div.dataset.id = p.id;
  const kColor = KIND_COLORS[p.kind] || 'gray';
  const kLabel = KIND_LABELS[p.kind] || p.kind;
  const cycle = p.cycle_length_days || 7;
  const dur = p.duration_weeks ? `${p.duration_weeks} нед.` : '∞';
  div.innerHTML = `
    <div class="sport-card-header">
      <span class="sport-card-name">${p.active === 1 ? '● ' : ''}${p.favorite === 1 ? '★ ' : ''}${escapeHtml(p.name)}</span>
      <span class="sport-card-count">${p.day_count || 0} дн.</span>
    </div>
    <div class="sport-card-meta">
      <span class="badge badge-${kColor}">${kLabel}</span>
      <span class="badge badge-gray">цикл ${cycle}д</span>
      <span class="badge badge-gray">${dur}</span>
    </div>`;
  return div;
}
