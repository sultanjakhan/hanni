// ── sport-template-modals.js — Template detail modal + delete ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { TYPE_COLORS, DIFF_COLORS } from './sport-template-filters.js';
import { MUSCLE_LABELS, MUSCLE_COLORS } from './sport-catalog-filters.js';

const TYPE_LABELS = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };
const DIFF_LABELS = { easy: 'Лёгкий', medium: 'Средний', hard: 'Сложный' };

export async function showTemplateDetail(id, onChanged) {
  const t = await invoke('get_workout_template', { id }).catch(() => null);
  if (!t) return;
  const tLabel = TYPE_LABELS[t.type] || t.type;
  const dLabel = DIFF_LABELS[t.difficulty] || t.difficulty;
  const tColor = TYPE_COLORS[t.type] || 'gray';
  const dColor = DIFF_COLORS[t.difficulty] || 'gray';
  const muscles = (t.target_muscle_groups || '').split(',').filter(m => m.trim());
  const muscleHtml = muscles.map(m => {
    const mt = m.trim();
    return `<span class="badge badge-${MUSCLE_COLORS[mt] || 'gray'}">${MUSCLE_LABELS[mt] || mt}</span>`;
  }).join(' ');

  const exercisesHtml = (t.exercise_items || []).map((e, i) => {
    const detail = e.duration_seconds
      ? `${e.duration_seconds}с`
      : `${e.sets}×${e.reps}${e.weight_kg ? ` · ${e.weight_kg}кг` : ''}`;
    return `<div class="tmpl-exercise-row">
      <span class="tmpl-exercise-num">${i + 1}</span>
      <span class="tmpl-exercise-name">${escapeHtml(e.name)}</span>
      <span class="tmpl-exercise-detail">${detail}</span>
      <span class="tmpl-exercise-rest">${e.rest_seconds}с отдых</span>
    </div>`;
  }).join('');

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:520px;max-height:80vh;overflow-y:auto">
    <div class="modal-title">${t.favorite === 1 ? '★ ' : ''}${escapeHtml(t.name)}</div>
    <div class="sport-card-meta" style="margin-bottom:8px">
      <span class="badge badge-${tColor}">${tLabel}</span>
      <span class="badge badge-${dColor}">${dLabel}</span>
      ${muscleHtml}
    </div>
    ${t.notes ? `<div style="color:var(--text-secondary);font-size:13px;margin-bottom:10px">${escapeHtml(t.notes)}</div>` : ''}
    <div class="form-label">Упражнения (${(t.exercise_items || []).length})</div>
    <div class="tmpl-exercises-list">${exercisesHtml || '<div class="uni-empty">Нет упражнений</div>'}</div>
    <div class="modal-actions">
      <button class="btn-secondary" id="td-close">Закрыть</button>
      <button class="btn-secondary" id="td-fav">${t.favorite === 1 ? '★ Убрать' : '☆ Избранное'}</button>
      <button class="btn-danger" id="td-delete">Удалить</button>
      <button class="btn-primary" id="td-start">Начать тренировку</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#td-close').onclick = () => overlay.remove();

  overlay.querySelector('#td-fav').onclick = async () => {
    await invoke('toggle_favorite_template', { id });
    overlay.remove();
    onChanged();
  };
  overlay.querySelector('#td-delete').onclick = async () => {
    if (!confirm('Удалить шаблон?')) return;
    await invoke('delete_workout_template', { id });
    overlay.remove();
    onChanged();
  };
  overlay.querySelector('#td-start').onclick = async () => {
    try {
      await invoke('create_workout_from_template', { templateId: id });
      overlay.remove();
      onChanged();
    } catch (err) { alert('Ошибка: ' + err); }
  };
}
