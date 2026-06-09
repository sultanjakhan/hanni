// ── js/routine-add-modal.js — "Add task" modal for the routine graph ──
// Lets the user pick a task from schedules / calendar events / notes and
// create a graph node referencing it.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const CAT_ICONS = {
  health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡',
  practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽',
};

const SOURCES = [
  { id: 'schedule', label: '📋 Расписание' },
  { id: 'event', label: '📅 Календарь' },
  { id: 'note', label: '📝 Заметки' },
];

async function fetchTasks(srcType) {
  if (srcType === 'schedule') {
    const rows = await invoke('get_schedules', { category: null }).catch(() => []);
    return rows.map(s => ({ id: s.id, title: s.title, category: s.category || 'other' }));
  }
  if (srcType === 'note') {
    const rows = await invoke('get_notes', { filter: null, search: null }).catch(() => []);
    return rows.map(n => ({ id: n.id, title: n.title || 'Без названия', category: 'other' }));
  }
  const now = new Date();
  const rows = await invoke('get_events', { month: now.getMonth() + 1, year: now.getFullYear() })
    .catch(() => []);
  return rows.map(e => ({ id: e.id, title: e.title, category: e.category || 'other' }));
}

export function openAddTaskModal(chainId, onAdd, spawn = null) {
  if (!chainId) return;
  let src = 'schedule';
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:380px;">
    <div class="modal-title">Добавить задачу в граф</div>
    <div class="dev-filters" id="rt-add-tabs">
      ${SOURCES.map(s => `<button class="dev-filter-btn${s.id === src ? ' active' : ''}"
        data-src="${s.id}">${s.label}</button>`).join('')}
    </div>
    <div id="rt-add-list" class="rt-add-list"></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="rt-add-cancel">Закрыть</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#rt-add-cancel').addEventListener('click', () => overlay.remove());

  const listEl = overlay.querySelector('#rt-add-list');
  async function renderList() {
    listEl.innerHTML = '<div class="rt-add-empty">Загрузка…</div>';
    const tasks = await fetchTasks(src);
    if (tasks.length === 0) {
      listEl.innerHTML = '<div class="rt-add-empty">Пусто</div>';
      return;
    }
    listEl.innerHTML = tasks.map(t => `<div class="rt-add-item" data-tid="${t.id}">
      <span>${CAT_ICONS[t.category] || CAT_ICONS.other}</span>
      <span>${escapeHtml(t.title)}</span>
    </div>`).join('');
    listEl.querySelectorAll('[data-tid]').forEach(item => {
      item.addEventListener('click', async () => {
        // Schedule ids are UUID strings (cr-sqlite), event/note ids are numbers —
        // compare and send as strings (Rust side takes Option<String>).
        const task = tasks.find(t => String(t.id) === item.dataset.tid);
        if (!task) return;
        await invoke('create_routine_node', {
          chainId, sourceType: src, sourceId: String(task.id),
          title: task.title, category: task.category,
          posX: spawn?.x ?? 60, posY: spawn?.y ?? 60,
        }).catch(() => {});
        overlay.remove();
        onAdd();
      });
    });
  }

  overlay.querySelectorAll('[data-src]').forEach(btn => {
    btn.addEventListener('click', () => {
      src = btn.dataset.src;
      overlay.querySelectorAll('[data-src]').forEach(b =>
        b.classList.toggle('active', b.dataset.src === src));
      renderList();
    });
  });
  renderList();
}
