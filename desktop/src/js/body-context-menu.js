// ── body-context-menu.js — Right-click context menu for 3D body zones ──
import { invoke } from './state.js';

const RECORD_TYPES = [
  { id: 'pain',        icon: '🔴', label: 'Боль' },
  { id: 'workout',     icon: '💪', label: 'Тренировка' },
  { id: 'goal',        icon: '🎯', label: 'Цель' },
  { id: 'treatment',   icon: '💊', label: 'Лечение' },
  { id: 'measurement', icon: '📏', label: 'Замер' },
  { id: 'note',        icon: '📝', label: 'Заметка' },
  { id: 'history',     icon: '📋', label: 'История' },
];

const PAIN_TYPES = ['острая', 'тупая', 'ноющая', 'стреляющая', 'жгучая'];
const GOAL_TYPES = ['накачать', 'растянуть', 'вылечить', 'реабилитация'];
const TREATMENT_TYPES = ['таблетки', 'упражнения', 'визит к врачу', 'массаж', 'другое'];

/** Show context menu at mouse position for a body zone */
export function showBodyContextMenu(x, y, zone, zoneLabel, callbacks) {
  closeBodyContextMenu();
  const menu = document.createElement('div');
  menu.className = 'inline-dropdown body-context-menu';
  menu.style.left = x + 'px';
  menu.style.top = y + 'px';

  const header = document.createElement('div');
  header.className = 'body-ctx-header';
  header.textContent = zoneLabel || zone;
  menu.appendChild(header);

  RECORD_TYPES.forEach(rt => {
    const opt = document.createElement('div');
    opt.className = 'inline-dd-option';
    opt.dataset.action = rt.id;
    opt.innerHTML = `<span style="margin-right:6px;">${rt.icon}</span>${rt.label}`;
    opt.addEventListener('click', () => {
      menu.remove();
      if (rt.id === 'history') {
        callbacks.onHistory?.(zone, zoneLabel);
      } else {
        showRecordModal(rt.id, zone, zoneLabel, callbacks.onSaved);
      }
    });
    menu.appendChild(opt);
  });

  document.body.appendChild(menu);
  // Keep menu on screen
  const rect = menu.getBoundingClientRect();
  if (rect.right > window.innerWidth) menu.style.left = (x - rect.width) + 'px';
  if (rect.bottom > window.innerHeight) menu.style.top = (y - rect.height) + 'px';

  setTimeout(() => document.addEventListener('mousedown', (ev) => {
    if (!menu.contains(ev.target)) menu.remove();
  }, { once: true }), 10);
}

export function closeBodyContextMenu() {
  document.querySelectorAll('.body-context-menu').forEach(m => m.remove());
}

/** Show modal for creating a body record */
function showRecordModal(type, zone, zoneLabel, onSaved) {
  const overlay = document.createElement('div');
  overlay.className = 'body-modal-overlay';
  const typeInfo = RECORD_TYPES.find(r => r.id === type);
  const title = `${typeInfo?.icon || ''} ${typeInfo?.label || type} — ${zoneLabel || zone}`;
  overlay.innerHTML = `<div class="body-modal">
    <div class="body-modal-title">${title}</div>
    <div class="body-modal-body">${getModalFields(type)}</div>
    <div class="body-modal-actions">
      <button class="btn-secondary body-modal-cancel">Отмена</button>
      <button class="btn-primary body-modal-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);

  // Live update for intensity slider
  const slider = overlay.querySelector('.body-modal-intensity');
  const valSpan = overlay.querySelector('.intensity-val');
  if (slider && valSpan) slider.oninput = () => { valSpan.textContent = slider.value; };

  overlay.querySelector('.body-modal-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  overlay.querySelector('.body-modal-save').onclick = async () => {
    const data = collectModalData(type, overlay);
    try {
      await invoke('create_body_record', {
        zone, zoneLabel, recordType: type, ...data,
      });
      overlay.remove();
      onSaved?.();
    } catch (err) {
      console.error('Failed to save body record:', err);
    }
  };
}

function getModalFields(type) {
  const noteField = '<textarea class="body-modal-note" placeholder="Заметка..." rows="2"></textarea>';
  switch (type) {
    case 'pain':
      return `<label>Интенсивность: <span class="intensity-val">5</span>/10</label>
        <input type="range" class="body-modal-intensity" min="1" max="10" value="5">
        <label>Тип боли</label>
        <select class="body-modal-pain-type">${PAIN_TYPES.map(t => `<option value="${t}">${t}</option>`).join('')}</select>
        ${noteField}`;
    case 'workout':
      return `<p class="body-modal-hint">Отметить эту зону как тренированную сегодня</p>${noteField}`;
    case 'goal':
      return `<label>Тип цели</label>
        <select class="body-modal-goal-type">${GOAL_TYPES.map(t => `<option value="${t}">${t}</option>`).join('')}</select>
        ${noteField}`;
    case 'treatment':
      return `<label>Тип лечения</label>
        <select class="body-modal-treatment-type">${TREATMENT_TYPES.map(t => `<option value="${t}">${t}</option>`).join('')}</select>
        ${noteField}`;
    case 'measurement':
      return `<div style="display:flex;gap:8px;align-items:center">
        <input type="number" class="body-modal-value" placeholder="Значение" step="0.1" style="width:100px">
        <select class="body-modal-unit"><option value="см">см</option><option value="кг">кг</option><option value="мм">мм</option></select>
      </div>${noteField}`;
    case 'note':
      return noteField;
    default: return noteField;
  }
}

function collectModalData(type, overlay) {
  const note = overlay.querySelector('.body-modal-note')?.value || '';
  const base = { note, date: null };
  switch (type) {
    case 'pain':
      return { ...base, intensity: parseInt(overlay.querySelector('.body-modal-intensity')?.value || '5'),
        painType: overlay.querySelector('.body-modal-pain-type')?.value || null };
    case 'goal':
      return { ...base, goalType: overlay.querySelector('.body-modal-goal-type')?.value || null };
    case 'treatment':
      return { ...base, treatmentType: overlay.querySelector('.body-modal-treatment-type')?.value || null };
    case 'measurement':
      return { ...base, value: parseFloat(overlay.querySelector('.body-modal-value')?.value) || null,
        unit: overlay.querySelector('.body-modal-unit')?.value || null };
    default: return base;
  }
}
