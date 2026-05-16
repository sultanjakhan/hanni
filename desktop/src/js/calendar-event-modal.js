// calendar-event-modal.js — Create / edit calendar event modal with
// DB-driven categories and 5-level priority picker.

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { loadCategories } from './calendar-categories.js';
import { showCategoryManager, showAddCategory } from './calendar-category-manager.js';

const PRIORITY_LEVELS = [
  { v: 0, label: 'Нет',          hex: 'var(--bg-hover)',     text: 'var(--text-secondary)' },
  { v: 1, label: '1 · низкий',   hex: 'var(--color-green)',  text: '#fff' },
  { v: 2, label: '2',            hex: 'var(--color-lime)',   text: '#fff' },
  { v: 3, label: '3 · средний',  hex: 'var(--color-yellow)', text: '#fff' },
  { v: 4, label: '4',            hex: 'var(--color-orange)', text: '#fff' },
  { v: 5, label: '5 · срочно',   hex: 'var(--color-red)',    text: '#fff' },
];

export function priorityHex(v) {
  const lv = PRIORITY_LEVELS.find(l => l.v === Number(v));
  return lv ? lv.hex : 'var(--bg-hover)';
}

function renderPriorityPicker(current) {
  const cur = Number(current || 0);
  return `<div class="evm-priority" data-evm-priority="${cur}">
    ${PRIORITY_LEVELS.map(l => `
      <button type="button" class="evm-pri-pill${l.v === cur ? ' active' : ''}" data-pri="${l.v}"
        style="--pri-bg:${l.hex};--pri-fg:${l.text};" title="${l.label}">${l.v === 0 ? '·' : l.v}</button>
    `).join('')}
  </div>`;
}

function categoryOptions(cats, current) {
  return cats.map(c =>
    `<option value="${escapeHtml(c.name)}"${c.name === current ? ' selected' : ''}>${escapeHtml(c.icon)} ${escapeHtml(c.name)}</option>`
  ).join('');
}

function renderCategoryPicker(cats, current) {
  return `<div class="evm-cat-row">
    <select class="form-select" id="evm-cat">${categoryOptions(cats, current)}</select>
    <button type="button" class="btn-icon" id="evm-cat-new" title="Новая категория">+</button>
    <button type="button" class="btn-icon" id="evm-cat-manage" title="Управление категориями">⚙️</button>
  </div>`;
}

export async function showEventModal(eventId = null) {
  const isEdit = eventId != null;
  let cats = await loadCategories();
  let event = null;
  if (isEdit) {
    // No single get_event command — fetch all events and find by id.
    const all = await invoke('get_all_events').catch(() => []);
    event = (all || []).find(e => e.id === Number(eventId));
    if (!event) { alert('Событие не найдено'); return; }
  }

  const initDate = event?.date || S.selectedCalendarDate || new Date().toISOString().split('T')[0];
  const initTime = event?.time || '';
  const initTitle = event?.title || '';
  const initDesc = event?.description || '';
  const initCat = event?.category || 'general';
  const initPri = event?.priority ?? 0;
  const initDur = event?.duration_minutes || 60;

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">${isEdit ? 'Редактировать событие' : 'Новое событие'}</div>
    <div class="form-row"><input class="form-input" id="evm-title" placeholder="Название" value="${escapeHtml(initTitle)}"></div>
    <div class="form-row">
      <input class="form-input" id="evm-date" type="date" value="${initDate}">
      <input class="form-input" id="evm-time" type="time" style="max-width:120px;" value="${initTime}">
      <input class="form-input" id="evm-dur" type="number" min="5" step="5" style="max-width:90px;" value="${initDur}" title="Длительность, мин">
    </div>
    <div class="form-row evm-label">Категория</div>
    ${renderCategoryPicker(cats, initCat)}
    <div class="form-row evm-label">Важность</div>
    ${renderPriorityPicker(initPri)}
    <textarea class="form-textarea" id="evm-desc" placeholder="Описание (необязательно)" rows="2">${escapeHtml(initDesc)}</textarea>
    <div class="modal-actions">
      ${isEdit ? '<button class="btn-secondary evm-del" id="evm-del">Удалить</button>' : ''}
      <button class="btn-secondary" id="evm-cancel">Отмена</button>
      <button class="btn-primary" id="evm-save">${isEdit ? 'Сохранить' : 'Создать'}</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#evm-cancel')?.addEventListener('click', () => overlay.remove());

  // Priority pill clicks
  overlay.querySelectorAll('.evm-pri-pill').forEach(p => {
    p.addEventListener('click', () => {
      overlay.querySelectorAll('.evm-pri-pill').forEach(x => x.classList.remove('active'));
      p.classList.add('active');
      const wrap = overlay.querySelector('.evm-priority');
      if (wrap) wrap.dataset.evmPriority = p.dataset.pri;
    });
  });

  // Category: + new / manage
  overlay.querySelector('#evm-cat-new')?.addEventListener('click', () => {
    showAddCategory(async (newName) => {
      cats = await loadCategories(true);
      const sel = overlay.querySelector('#evm-cat');
      if (sel) sel.innerHTML = categoryOptions(cats, newName);
    });
  });
  overlay.querySelector('#evm-cat-manage')?.addEventListener('click', () => {
    showCategoryManager(async () => {
      cats = await loadCategories(true);
      const sel = overlay.querySelector('#evm-cat');
      if (sel) sel.innerHTML = categoryOptions(cats, sel.value);
    });
  });

  // Delete (edit-mode only)
  overlay.querySelector('#evm-del')?.addEventListener('click', async () => {
    if (!confirm('Удалить событие безвозвратно?')) return;
    try {
      await invoke('delete_event', { id: Number(eventId) });
      overlay.remove();
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
    } catch (err) { alert('Ошибка: ' + err); }
  });

  overlay.querySelector('#evm-save')?.addEventListener('click', async () => {
    const title = overlay.querySelector('#evm-title')?.value?.trim();
    if (!title) return;
    const date = overlay.querySelector('#evm-date')?.value || '';
    const time = overlay.querySelector('#evm-time')?.value || '';
    const dur = parseInt(overlay.querySelector('#evm-dur')?.value || '60', 10) || 60;
    const cat = overlay.querySelector('#evm-cat')?.value || 'general';
    const pri = parseInt(overlay.querySelector('.evm-priority')?.dataset.evmPriority || '0', 10);
    const desc = overlay.querySelector('#evm-desc')?.value || '';
    const catColor = (cats.find(c => c.name === cat)?.color) || '#9B9B9B';

    try {
      if (isEdit) {
        await invoke('update_event', {
          id: Number(eventId),
          title, description: desc, date, time,
          durationMinutes: dur, category: cat, color: catColor,
          completed: null, priority: pri,
        });
      } else {
        await invoke('create_event', {
          title, description: desc, date, time,
          durationMinutes: dur, category: cat, color: catColor, priority: pri,
        });
      }
      overlay.remove();
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
    } catch (err) { alert('Ошибка: ' + err); }
  });
}
