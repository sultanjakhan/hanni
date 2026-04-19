// ── sport-catalog-card.js — Exercise catalog card rendering ──
import { escapeHtml } from './utils.js';
import { MUSCLE_LABELS, MUSCLE_COLORS, TYPE_LABELS, TYPE_COLORS } from './sport-catalog-filters.js';

export function renderExerciseCard(ex) {
  const div = document.createElement('div');
  div.className = 'sport-card';
  div.dataset.id = ex.id;
  const mLabel = MUSCLE_LABELS[ex.muscle_group] || ex.muscle_group;
  const mColor = MUSCLE_COLORS[ex.muscle_group] || 'gray';
  const tLabel = TYPE_LABELS[ex.type] || ex.type;
  const tColor = TYPE_COLORS[ex.type] || 'gray';
  div.innerHTML = `
    <div class="sport-card-header">
      <span class="sport-card-name">${escapeHtml(ex.name)}</span>
    </div>
    <div class="sport-card-meta">
      <span class="badge badge-${mColor}">${mLabel}</span>
      <span class="badge badge-${tColor}">${tLabel}</span>
      ${ex.equipment ? `<span class="sport-card-equip">${escapeHtml(ex.equipment)}</span>` : ''}
    </div>
    ${ex.description ? `<div class="sport-card-desc">${escapeHtml(ex.description)}</div>` : ''}`;
  return div;
}
