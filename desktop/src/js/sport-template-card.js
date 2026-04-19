// ── sport-template-card.js — Workout template card rendering ──
import { escapeHtml } from './utils.js';
import { TYPE_COLORS, DIFF_COLORS } from './sport-template-filters.js';
import { MUSCLE_LABELS, MUSCLE_COLORS } from './sport-catalog-filters.js';

const TYPE_LABELS = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };
const DIFF_LABELS = { easy: 'Лёгкий', medium: 'Средний', hard: 'Сложный' };

export function renderTemplateCard(t) {
  const div = document.createElement('div');
  div.className = 'sport-card' + (t.favorite === 1 ? ' recipe-fav' : '');
  div.dataset.id = t.id;
  const tColor = TYPE_COLORS[t.type] || 'gray';
  const tLabel = TYPE_LABELS[t.type] || t.type;
  const dColor = DIFF_COLORS[t.difficulty] || 'gray';
  const dLabel = DIFF_LABELS[t.difficulty] || t.difficulty;
  const muscles = (t.target_muscle_groups || '').split(',').filter(m => m.trim()).slice(0, 4);
  const muscleHtml = muscles.map(m => {
    const mt = m.trim();
    return `<span class="badge badge-${MUSCLE_COLORS[mt] || 'gray'}">${MUSCLE_LABELS[mt] || mt}</span>`;
  }).join('');

  div.innerHTML = `
    <div class="sport-card-header">
      <span class="sport-card-name">${t.favorite === 1 ? '★ ' : ''}${escapeHtml(t.name)}</span>
      <span class="sport-card-count">${t.exercise_count || 0} упр.</span>
    </div>
    <div class="sport-card-meta">
      <span class="badge badge-${tColor}">${tLabel}</span>
      <span class="badge badge-${dColor}">${dLabel}</span>
    </div>
    ${muscleHtml ? `<div class="sport-card-muscles">${muscleHtml}</div>` : ''}`;
  return div;
}
