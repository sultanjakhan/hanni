// ── sport-catalog-modal.js — Add/edit exercise modal ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { MUSCLE_GROUPS, EXERCISE_TYPES, invalidateCatalogCache } from './sport-catalog-filters.js';

function chips(items, cur, group) {
  return items.filter(o => o.id !== 'all').map(o =>
    `<button class="rf-chip${cur === o.id ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}

export function showExerciseModal(onSaved, editData) {
  const isEdit = !!editData;
  const mg = editData?.muscle_group || 'full_body';
  const et = editData?.type || 'strength';
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">${isEdit ? 'Редактировать' : 'Новое'} упражнение</div>
    <input class="form-input" id="ex-name" placeholder="Название" value="${escapeHtml(editData?.name || '')}">
    <div class="form-label" style="margin-top:8px">Группа мышц</div>
    <div class="rf-chip-row" id="ex-muscle">${chips(MUSCLE_GROUPS, mg, 'muscle')}</div>
    <div class="form-label" style="margin-top:8px">Тип</div>
    <div class="rf-chip-row" id="ex-type">${chips(EXERCISE_TYPES, et, 'type')}</div>
    <input class="form-input" id="ex-equip" placeholder="Оборудование" value="${escapeHtml(editData?.equipment || '')}" style="margin-top:8px">
    <textarea class="form-textarea" id="ex-desc" placeholder="Описание (необязательно)" rows="2" style="margin-top:8px">${escapeHtml(editData?.description || '')}</textarea>
    <div class="modal-actions">
      <button class="btn-secondary" id="ex-cancel">Отмена</button>
      ${isEdit ? `<button class="btn-danger" id="ex-delete">Удалить</button>` : ''}
      <button class="btn-primary" id="ex-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#ex-cancel').onclick = () => overlay.remove();

  let selMuscle = mg, selType = et;
  overlay.querySelectorAll('.rf-chip').forEach(btn => btn.onclick = () => {
    const g = btn.dataset.group, v = btn.dataset.val;
    if (g === 'muscle') selMuscle = v;
    else selType = v;
    const row = btn.parentElement;
    row.querySelectorAll('.rf-chip').forEach(b => b.classList.toggle('active', b.dataset.val === v));
  });

  if (isEdit) {
    overlay.querySelector('#ex-delete')?.addEventListener('click', async () => {
      await invoke('delete_exercise_catalog', { id: editData.id });
      invalidateCatalogCache();
      overlay.remove();
      onSaved();
    });
  }

  overlay.querySelector('#ex-save').onclick = async () => {
    const name = overlay.querySelector('#ex-name').value.trim();
    if (!name) return;
    try {
      if (isEdit) {
        await invoke('update_exercise_catalog', {
          id: editData.id, name, muscleGroup: selMuscle, equipment: overlay.querySelector('#ex-equip').value.trim(),
          exerciseType: selType, description: overlay.querySelector('#ex-desc').value.trim(),
        });
      } else {
        await invoke('add_exercise_to_catalog', {
          name, muscleGroup: selMuscle, equipment: overlay.querySelector('#ex-equip').value.trim(),
          exerciseType: selType, description: overlay.querySelector('#ex-desc').value.trim(),
        });
      }
      invalidateCatalogCache();
      overlay.remove();
      onSaved();
    } catch (err) { alert('Ошибка: ' + err); }
  };
}
