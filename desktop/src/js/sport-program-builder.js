// ── sport-program-builder.js — Create/edit program modal with a day-list editor ──
import { invoke } from './state.js';
import { escapeHtml, chips } from './utils.js';
import { PROGRAM_KINDS } from './sport-program-filters.js';
import { MUSCLE_GROUPS } from './sport-catalog-filters.js';

function multiChips(items, selected, group) {
  return items.filter(o => o.id !== 'all').map(o =>
    `<button class="rf-chip${selected.has(o.id) ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}

function templateOptions(templates, selId) {
  const opts = ['<option value="">— нет (отдых/пусто) —</option>'];
  for (const t of templates) opts.push(`<option value="${t.id}"${t.id === selId ? ' selected' : ''}>${escapeHtml(t.name)}</option>`);
  return opts.join('');
}

function dayRowHtml(i, day, templates) {
  return `<div class="program-day-row" data-i="${i}">
    <span class="program-day-idx">${i + 1}</span>
    <input class="form-input program-day-label" placeholder="Метка (Push / Ноги…)" value="${escapeHtml(day.label || '')}">
    <select class="form-input program-day-template">${templateOptions(templates, day.template_id)}</select>
    <button class="rf-chip program-day-rest${day.is_rest ? ' active' : ''}" type="button">Отдых</button>
    <button class="program-day-remove" type="button" title="Удалить день">✕</button>
  </div>`;
}

function collectDays(container) {
  return [...container.querySelectorAll('.program-day-row')].map((row, i) => {
    const tid = row.querySelector('.program-day-template').value;
    return {
      label: row.querySelector('.program-day-label').value.trim(),
      template_id: tid ? parseInt(tid, 10) : null,
      is_rest: row.querySelector('.program-day-rest').classList.contains('active'),
      day_index: i, order_index: i,
    };
  });
}

export async function showProgramBuilder(onSaved, editData) {
  const isEdit = !!editData;
  const templates = await invoke('get_workout_templates', { search: null }).catch(() => []);
  let selKind = editData?.kind || 'split';
  const selMuscles = new Set((editData?.target_muscle_groups || '').split(',').map(m => m.trim()).filter(Boolean));
  const initialDays = editData?.days?.length ? editData.days.map(d => ({ ...d })) : [{ label: '', template_id: null, is_rest: false }];

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:640px;max-height:88vh;overflow-y:auto">
    <div class="modal-title">${isEdit ? 'Редактировать' : 'Новая'} программа</div>
    <input class="form-input" id="pg-name" placeholder="Название" value="${escapeHtml(editData?.name || '')}">
    <div class="form-label" style="margin-top:8px">Тип</div>
    <div class="rf-chip-row" id="pg-kind">${chips(PROGRAM_KINDS, selKind, 'kind', true)}</div>
    <div class="program-builder-nums">
      <label class="form-label">Цикл (дней)<input class="form-input" id="pg-cycle" type="number" min="1" value="${editData?.cycle_length_days || initialDays.length || 7}"></label>
      <label class="form-label">Длительность (недель, 0=∞)<input class="form-input" id="pg-weeks" type="number" min="0" value="${editData?.duration_weeks || 0}"></label>
    </div>
    <div class="form-label" style="margin-top:8px">Целевые мышцы (для баланса)</div>
    <div class="rf-chip-row" id="pg-muscles">${multiChips(MUSCLE_GROUPS, selMuscles, 'muscles')}</div>
    <div class="form-label" style="margin-top:12px">Дни программы</div>
    <div id="pg-days"></div>
    <button class="btn-secondary" id="pg-add-day" type="button" style="margin-top:6px">+ День</button>
    <textarea class="form-textarea" id="pg-notes" placeholder="Заметки" rows="2" style="margin-top:8px">${escapeHtml(editData?.notes || '')}</textarea>
    <div class="modal-actions">
      <button class="btn-secondary" id="pg-cancel">Отмена</button>
      <button class="btn-primary" id="pg-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#pg-cancel').onclick = () => overlay.remove();

  const daysEl = overlay.querySelector('#pg-days');
  const renderDays = (days) => { daysEl.innerHTML = days.map((d, i) => dayRowHtml(i, d, templates)).join(''); };
  renderDays(initialDays);

  daysEl.addEventListener('click', (e) => {
    const rest = e.target.closest('.program-day-rest');
    if (rest) { rest.classList.toggle('active'); return; }
    const rm = e.target.closest('.program-day-remove');
    if (rm) { const days = collectDays(daysEl); days.splice(+rm.closest('.program-day-row').dataset.i, 1); renderDays(days.length ? days : [{ label: '', template_id: null, is_rest: false }]); }
  });
  overlay.querySelector('#pg-add-day').onclick = () => { const days = collectDays(daysEl); days.push({ label: '', template_id: null, is_rest: false }); renderDays(days); };

  overlay.querySelector('#pg-kind').addEventListener('click', (e) => {
    const c = e.target.closest('.rf-chip'); if (!c) return;
    selKind = c.dataset.val;
    overlay.querySelectorAll('#pg-kind .rf-chip').forEach(b => b.classList.toggle('active', b.dataset.val === selKind));
  });
  overlay.querySelector('#pg-muscles').addEventListener('click', (e) => {
    const c = e.target.closest('.rf-chip'); if (!c) return;
    const v = c.dataset.val;
    selMuscles.has(v) ? selMuscles.delete(v) : selMuscles.add(v);
    c.classList.toggle('active');
  });

  overlay.querySelector('#pg-save').onclick = async () => {
    const name = overlay.querySelector('#pg-name').value.trim();
    if (!name) return;
    const days = collectDays(daysEl);
    const payload = {
      name, kind: selKind,
      cycleLengthDays: parseInt(overlay.querySelector('#pg-cycle').value, 10) || days.length || 7,
      durationWeeks: parseInt(overlay.querySelector('#pg-weeks').value, 10) || 0,
      targetMuscleGroups: [...selMuscles].join(','),
      notes: overlay.querySelector('#pg-notes').value.trim(), days,
    };
    try {
      if (isEdit) await invoke('update_workout_program', { id: editData.id, ...payload });
      else await invoke('create_workout_program', payload);
      overlay.remove();
      onSaved();
    } catch (err) { alert('Ошибка: ' + err); }
  };
}
