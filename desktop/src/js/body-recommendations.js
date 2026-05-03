// ── body-recommendations.js — modal listing exercises tagged for a body zone ──
import { invoke } from './state.js';
import { MUSCLE_LABELS, TYPE_LABELS, MUSCLE_COLORS, TYPE_COLORS } from './sport-catalog-filters.js';

export async function showBodyRecommendationsModal(zone, zoneLabel) {
  const overlay = document.createElement('div');
  overlay.className = 'body-modal-overlay';
  const title = `🏋️ Рекомендации — ${zoneLabel || zone}`;
  overlay.innerHTML = `<div class="body-modal" style="max-width:520px">
    <div class="body-modal-title">${escapeHtml(title)}</div>
    <div class="body-modal-body" id="body-recs-body" style="max-height:60vh;overflow-y:auto">
      <div class="body-modal-hint">Загрузка…</div>
    </div>
    <div class="body-modal-actions">
      <button class="btn-secondary body-recs-close">Закрыть</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);

  overlay.querySelector('.body-recs-close').onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  let exercises = [];
  try {
    exercises = await invoke('get_exercises_by_body_zone', { zone });
  } catch (err) {
    console.error('Failed to fetch recommendations:', err);
  }

  const body = overlay.querySelector('#body-recs-body');
  if (!exercises || exercises.length === 0) {
    body.innerHTML = `<div class="body-modal-hint">
      Нет упражнений для зоны «${escapeHtml(zoneLabel || zone)}».<br>
      Откройте Спорт → Каталог, создайте/отредактируйте упражнение и тегните эту зону.
    </div>`;
    return;
  }

  body.innerHTML = exercises.map(renderExerciseRow).join('');
}

function renderExerciseRow(ex) {
  const muscleLabel = MUSCLE_LABELS[ex.muscle_group] || ex.muscle_group || '';
  const typeLabel = TYPE_LABELS[ex.type] || ex.type || '';
  const muscleColor = MUSCLE_COLORS[ex.muscle_group] || 'gray';
  const typeColor = TYPE_COLORS[ex.type] || 'gray';
  const equipment = ex.equipment ? `<span class="badge badge-gray">${escapeHtml(ex.equipment)}</span>` : '';
  const desc = ex.description ? `<div class="body-recs-desc">${escapeHtml(ex.description)}</div>` : '';
  return `<div class="body-recs-row">
    <div class="body-recs-name">${escapeHtml(ex.name)}</div>
    <div class="body-recs-badges">
      <span class="badge badge-${muscleColor}">${escapeHtml(muscleLabel)}</span>
      <span class="badge badge-${typeColor}">${escapeHtml(typeLabel)}</span>
      ${equipment}
    </div>
    ${desc}
  </div>`;
}

function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"']/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
  ));
}
