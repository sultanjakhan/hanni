// calendar-event-modal.js — Create / edit calendar event modal with
// DB-driven categories and 5-level priority picker.

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { loadCategories } from './calendar-categories.js';
import { showCategoryManager, showAddCategory } from './calendar-category-manager.js';
import { renderTemplatesAccordion, bindTemplates } from './calendar-event-templates.js';
import { markBought } from './shopping-list.js';

// Tabs that make sense to attach an event/task to. Excludes utility tabs
// (timeline = activity log, schedule = recurring rules) and Calendar itself.
// Icons override TAB_REGISTRY because some tabs (notes) ship an inline SVG
// there — useless inside an <option>. Plain emoji renders cleanly.
const PROJECT_TABS = [
  { id: 'notes',       icon: '📝', label: 'Notes' },
  { id: 'jobs',        icon: '💼', label: 'Jobs' },
  { id: 'projects',    icon: '📁', label: 'Projects' },
  { id: 'development', icon: '🚀', label: 'Development' },
  { id: 'home',        icon: '🏠', label: 'Home' },
  { id: 'hobbies',     icon: '🎮', label: 'Hobbies' },
  { id: 'sports',      icon: '⚽', label: 'Sports' },
  { id: 'health',      icon: '❤️',  label: 'Health' },
  { id: 'food',        icon: '🍔', label: 'Food' },
  { id: 'money',       icon: '💰', label: 'Money' },
  { id: 'people',      icon: '👥', label: 'People' },
];

function renderProjectPicker(current) {
  const cur = current || '';
  const opts = PROJECT_TABS
    .map(t => `<option value="${escapeHtml(t.id)}"${t.id === cur ? ' selected' : ''}>${t.icon} ${escapeHtml(t.label)}</option>`)
    .join('');
  return `<select class="form-select" id="evm-linked-tab">
    <option value=""${!cur ? ' selected' : ''}>— Без привязки —</option>
    ${opts}
  </select>`;
}

// Default time: now rounded UP to nearest 5 minutes (so it isn't already past).
function nextRoundedTime() {
  const d = new Date();
  d.setMinutes(d.getMinutes() + (5 - d.getMinutes() % 5) % 5);
  if (d.getMinutes() === new Date().getMinutes()) d.setMinutes(d.getMinutes() + 5);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

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

// "+ Новая" / "⚙️ Управление" are folded into the select itself as
// sentinel values — keeps the row visually identical to Project and
// drops two cluttering icon buttons from the form.
const CAT_ACTION_NEW = '__new__';
const CAT_ACTION_MANAGE = '__manage__';

function renderCategoryPicker(cats, current) {
  return `<select class="form-select" id="evm-cat">
    ${categoryOptions(cats, current)}
    <option disabled>──────────</option>
    <option value="${CAT_ACTION_NEW}">+ Новая категория…</option>
    <option value="${CAT_ACTION_MANAGE}">⚙️ Управление…</option>
  </select>`;
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
  const initTime = event?.time || (isEdit ? '' : nextRoundedTime());
  const initTitle = event?.title || '';
  const initDesc = event?.description || '';
  const initCat = event?.category || 'general';
  const initPri = event?.priority ?? 0;
  const initDur = event?.duration_minutes || 60;
  const initTab = event?.linked_tab || '';

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  // ctx carries side-effects opened by a template (e.g. shopping picker
  // pre-selects items; on Save we mark them bought_at).
  const ctx = { shoppingPickedIds: null };
  overlay.innerHTML = `<div class="modal modal-compact evm-modal">
    ${isEdit ? '<button type="button" class="evm-del-corner" id="evm-del" title="Удалить событие">🗑</button>' : ''}
    ${isEdit ? '' : renderTemplatesAccordion()}
    <div class="form-row evm-title-row">
      <span class="evm-tpl-prefix" id="evm-tpl-prefix" hidden></span>
      <input class="form-input" id="evm-title" placeholder="Название" value="${escapeHtml(initTitle)}">
    </div>
    <div class="form-row evm-when-row">
      <label class="evm-field">
        <span class="evm-field-label">Дата</span>
        <input class="form-input" id="evm-date" type="date" value="${initDate}">
      </label>
      <label class="evm-field">
        <span class="evm-field-label">Время</span>
        <input class="form-input" id="evm-time" type="time" value="${initTime}">
      </label>
      <label class="evm-field evm-field-dur">
        <span class="evm-field-label">Длительность</span>
        <span class="evm-dur-wrap"><input class="form-input" id="evm-dur" type="number" min="5" step="5" value="${initDur}"><span class="evm-dur-unit">мин</span></span>
      </label>
    </div>
    <div class="evm-classify">
      <div class="evm-classify-col">
        <div class="evm-label">🏷️ Категория</div>
        ${renderCategoryPicker(cats, initCat)}
      </div>
      <div class="evm-classify-col">
        <div class="evm-label">📂 Проект</div>
        ${renderProjectPicker(initTab)}
      </div>
    </div>
    <div class="form-row evm-label">Важность</div>
    ${renderPriorityPicker(initPri)}
    <details class="evm-desc-wrap"${initDesc ? ' open' : ''}>
      <summary class="evm-desc-toggle">＋ Описание${initDesc ? '' : ' (необязательно)'}</summary>
      <textarea class="form-textarea" id="evm-desc" placeholder="Заметки, контекст…" rows="2">${escapeHtml(initDesc)}</textarea>
    </details>
    <div class="modal-actions evm-actions">
      <button class="evm-cancel-link" id="evm-cancel">Отмена</button>
      <button class="btn-primary evm-save-btn" id="evm-save">${isEdit ? 'Сохранить' : 'Создать'}</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  // Close on backdrop click — use mousedown (fires before any pending
  // select-dropdown close re-routes the mouseup into the backdrop) and
  // guard against secondary modals layered on top (add-category, etc.).
  overlay.addEventListener('mousedown', (e) => {
    if (e.target !== overlay) return;
    if (document.querySelectorAll('.modal-overlay').length > 1) return;
    overlay.remove();
  });
  overlay.querySelector('#evm-cancel')?.addEventListener('click', () => overlay.remove());

  // ESC closes the modal — AbortController lets us detach the global
  // keydown listener as soon as the modal element is removed from DOM.
  const keyCtl = new AbortController();
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { overlay.remove(); keyCtl.abort(); }
  }, { signal: keyCtl.signal });
  new MutationObserver((_, obs) => {
    if (!document.body.contains(overlay)) { keyCtl.abort(); obs.disconnect(); }
  }).observe(document.body, { childList: true });
  const titleInput = overlay.querySelector('#evm-title');
  titleInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); overlay.querySelector('#evm-save')?.click(); }
  });
  // Autofocus title for fast capture: open modal → type → Enter.
  setTimeout(() => titleInput?.focus(), 50);

  if (!isEdit) bindTemplates(overlay, ctx);

  // Priority pill clicks
  overlay.querySelectorAll('.evm-pri-pill').forEach(p => {
    p.addEventListener('click', () => {
      overlay.querySelectorAll('.evm-pri-pill').forEach(x => x.classList.remove('active'));
      p.classList.add('active');
      const wrap = overlay.querySelector('.evm-priority');
      if (wrap) wrap.dataset.evmPriority = p.dataset.pri;
    });
  });

  // Category select: +/⚙️ are sentinel options. On pick → run the
  // action, then re-render options keeping the prior real selection
  // (so the form value doesn't get stuck on a sentinel).
  const refreshCatOptions = (keepValue) => {
    const sel = overlay.querySelector('#evm-cat');
    if (!sel) return;
    const html = renderCategoryPicker(cats, keepValue);
    const tmp = document.createElement('div'); tmp.innerHTML = html;
    sel.innerHTML = tmp.firstElementChild.innerHTML;
    sel.dataset.prev = sel.value;
  };
  const catSel = overlay.querySelector('#evm-cat');
  if (catSel) catSel.dataset.prev = catSel.value;
  catSel?.addEventListener('change', (e) => {
    const v = e.target.value;
    const prev = e.target.dataset.prev || 'general';
    if (v === CAT_ACTION_NEW) {
      e.target.value = prev;
      showAddCategory(async (newName) => {
        cats = await loadCategories(true);
        refreshCatOptions(newName);
      });
    } else if (v === CAT_ACTION_MANAGE) {
      e.target.value = prev;
      showCategoryManager(async () => {
        cats = await loadCategories(true);
        refreshCatOptions(prev);
      });
    } else {
      e.target.dataset.prev = v;
    }
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
    const linkedTab = overlay.querySelector('#evm-linked-tab')?.value || '';
    const catColor = (cats.find(c => c.name === cat)?.color) || '#9B9B9B';

    try {
      if (isEdit) {
        await invoke('update_event', {
          id: Number(eventId),
          title, description: desc, date, time,
          durationMinutes: dur, category: cat, color: catColor,
          completed: null, priority: pri, linkedTab,
        });
      } else {
        await invoke('create_event', {
          title, description: desc, date, time,
          durationMinutes: dur, category: cat, color: catColor, priority: pri, linkedTab,
        });
        if (ctx.shoppingPickedIds?.length) {
          await markBought(ctx.shoppingPickedIds).catch(() => {});
        }
      }
      overlay.remove();
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
    } catch (err) { alert('Ошибка: ' + err); }
  });
}
