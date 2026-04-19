// ── sport-template-add.js — Add/edit workout template modal ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { WORKOUT_TYPES, DIFFS } from './sport-template-filters.js';
import { MUSCLE_GROUPS } from './sport-catalog-filters.js';
import { createExerciseRows, collectExerciseItems, invalidateAcCache } from './sport-template-exercises.js';

function chips(items, cur, group) {
  return items.filter(o => o.id !== 'all').map(o =>
    `<button class="rf-chip${cur === o.id ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}
function multiChips(items, selected, group) {
  return items.filter(o => o.id !== 'all').map(o =>
    `<button class="rf-chip${selected.has(o.id) ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}

export function showAddTemplateModal(onSaved, editData) {
  const isEdit = !!editData;
  let selType = editData?.type || 'gym';
  let selDiff = editData?.difficulty || 'easy';
  const selMuscles = new Set((editData?.target_muscle_groups || '').split(',').map(m => m.trim()).filter(Boolean));

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:600px;max-height:85vh;overflow-y:auto">
    <div class="modal-title">${isEdit ? 'Редактировать' : 'Новый'} шаблон тренировки</div>
    <input class="form-input" id="tmpl-name" placeholder="Название" value="${escapeHtml(editData?.name || '')}">
    <div class="form-label" style="margin-top:8px">Тип</div>
    <div class="rf-chip-row" id="tmpl-type">${chips(WORKOUT_TYPES, selType, 'wtype')}</div>
    <div class="form-label" style="margin-top:8px">Сложность</div>
    <div class="rf-chip-row" id="tmpl-diff">${chips(DIFFS, selDiff, 'diff')}</div>
    <div class="form-label" style="margin-top:8px">Целевые мышцы</div>
    <div class="rf-chip-row" id="tmpl-muscles">${multiChips(MUSCLE_GROUPS, selMuscles, 'muscles')}</div>
    <div class="form-label" style="margin-top:12px">Упражнения</div>
    <div id="tmpl-exercises"></div>
    <textarea class="form-textarea" id="tmpl-notes" placeholder="Заметки" rows="2" style="margin-top:8px">${escapeHtml(editData?.notes || '')}</textarea>
    <div class="modal-actions">
      <button class="btn-secondary" id="tmpl-cancel">Отмена</button>
      <button class="btn-primary" id="tmpl-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#tmpl-cancel').onclick = () => overlay.remove();

  invalidateAcCache();
  const exerciseContainer = overlay.querySelector('#tmpl-exercises');
  const initialRows = editData?.exercise_items?.length
    ? editData.exercise_items.map(e => ({ ...e }))
    : [{ name: '', sets: 3, reps: 10, weight_kg: 0, rest_seconds: 60 }];
  createExerciseRows(exerciseContainer, initialRows);

  overlay.querySelectorAll('.rf-chip').forEach(btn => btn.onclick = () => {
    const g = btn.dataset.group, v = btn.dataset.val;
    if (g === 'wtype') { selType = v; }
    else if (g === 'diff') { selDiff = v; }
    else if (g === 'muscles') { selMuscles.has(v) ? selMuscles.delete(v) : selMuscles.add(v); }
    if (g !== 'muscles') {
      btn.parentElement.querySelectorAll('.rf-chip').forEach(b => b.classList.toggle('active', b.dataset.val === v));
    } else { btn.classList.toggle('active'); }
  });

  overlay.querySelector('#tmpl-save').onclick = async () => {
    const name = overlay.querySelector('#tmpl-name').value.trim();
    if (!name) return;
    const items = collectExerciseItems(exerciseContainer);
    const muscles = [...selMuscles].join(',');
    try {
      await invoke('create_workout_template', {
        name, templateType: selType, difficulty: selDiff,
        targetMuscleGroups: muscles, notes: overlay.querySelector('#tmpl-notes').value.trim(),
        exerciseItems: items,
      });
      overlay.remove();
      onSaved();
    } catch (err) { alert('Ошибка: ' + err); }
  };
}
