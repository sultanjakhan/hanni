// ── js/routine-node-modal.js — Node detail popup for the routine graph ──
// Click a node → see what task it is, its source (schedule/event/note) info,
// and edit title / priority / requirement.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const CAT_ICONS = {
  health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡',
  practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽',
};
const SRC_LABEL = { schedule: 'Расписание', note: 'Заметка', event: 'Событие' };
const TRIGGER_INFO = {
  sleep_end: { name: 'Конец сна', desc: 'Цепочка запускается автоматически, когда Health Connect зафиксирует, что ты проснулся.' },
  time: { name: 'По времени', desc: 'Цепочка запускается в заданное время суток.' },
  manual: { name: 'Вручную', desc: 'Цепочку ты запускаешь сам — кнопкой.' },
};

// Start node = a trigger, not a task — show a trigger info card.
function openTriggerModal(node, chain) {
  const info = TRIGGER_INFO[chain.trigger_type] || { name: 'Триггер запуска', desc: '' };
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal rt-node-modal">
    <div class="rt-nm-head">
      <span class="rt-nm-icon">⏰</span>
      <div class="rt-nm-title-static">${escapeHtml(node.title)}</div>
    </div>
    <div class="rt-nm-trigger">
      <div class="rt-nm-row"><span class="rt-nm-label">Тип</span><span>${info.name}</span></div>
      <div class="rt-nm-row"><span class="rt-nm-label">Источник</span><span class="rt-nm-src-tag">Health Connect</span></div>
      <div class="rt-nm-trigger-desc">${info.desc}</div>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" id="rt-nm-close">Закрыть</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#rt-nm-close').addEventListener('click', () => overlay.remove());
}

// Pull human-readable info about the node's linked source.
async function fetchSourceInfo(sourceType, sourceId) {
  if (!sourceId) return null;
  try {
    if (sourceType === 'schedule') {
      const rows = await invoke('get_schedules', { category: null });
      const s = rows.find(r => r.id === sourceId);
      return s ? { detail: s.details || '', extra: s.time_of_day || s.frequency || '' } : null;
    }
    if (sourceType === 'note') {
      const rows = await invoke('get_notes', { filter: null, search: null });
      const n = rows.find(r => r.id === sourceId);
      return n ? { detail: (n.content || '').slice(0, 200), extra: '' } : null;
    }
    const now = new Date();
    const rows = await invoke('get_events', { month: now.getMonth() + 1, year: now.getFullYear() });
    const e = rows.find(r => r.id === sourceId);
    return e ? { detail: e.description || '', extra: [e.date, e.time].filter(Boolean).join(' ') } : null;
  } catch { return null; }
}

export async function openNodeModal(node, chain, refresh) {
  if (node.is_start) { openTriggerModal(node, chain); return; }
  let priority = node.priority;
  let requirement = node.requirement;

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal rt-node-modal">
    <div class="rt-nm-head">
      <span class="rt-nm-icon">${CAT_ICONS[node.category] || CAT_ICONS.other}</span>
      <input class="rt-nm-title" id="rt-nm-title" value="${escapeHtml(node.title)}">
    </div>
    <div class="rt-nm-source" id="rt-nm-source">Загрузка…</div>
    <div class="rt-nm-row">
      <span class="rt-nm-label">Важность</span>
      <span class="rt-nm-dots" id="rt-nm-dots"></span>
    </div>
    <div class="rt-nm-row">
      <span class="rt-nm-label">Тип</span>
      <span class="rt-nm-req" id="rt-nm-req"></span>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary rt-nm-del" id="rt-nm-del">Удалить узел</button>
      <button class="btn-primary" id="rt-nm-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  // source info
  const srcEl = overlay.querySelector('#rt-nm-source');
  if (!node.source_id) {
    srcEl.innerHTML = `<span class="rt-nm-src-tag">${SRC_LABEL[node.source_type] || 'Узел'}</span>
      <span class="rt-nm-src-empty">Не привязан к источнику — своя задача рутины</span>`;
  } else {
    const info = await fetchSourceInfo(node.source_type, node.source_id);
    srcEl.innerHTML = `<span class="rt-nm-src-tag">${SRC_LABEL[node.source_type] || ''}</span>
      ${info?.extra ? `<span class="rt-nm-src-extra">${escapeHtml(info.extra)}</span>` : ''}
      <div class="rt-nm-src-detail">${info?.detail ? escapeHtml(info.detail) : 'Без описания'}</div>`;
  }

  // priority dots
  const dotsEl = overlay.querySelector('#rt-nm-dots');
  const drawDots = () => {
    dotsEl.innerHTML = [1, 2, 3, 4, 5]
      .map(i => `<span class="rt-nm-dot${i <= priority ? ' on' : ''}" data-p="${i}"></span>`).join('');
    dotsEl.querySelectorAll('[data-p]').forEach(d =>
      d.addEventListener('click', () => { priority = parseInt(d.dataset.p); drawDots(); }));
  };
  drawDots();

  // requirement toggle
  const reqEl = overlay.querySelector('#rt-nm-req');
  const drawReq = () => {
    reqEl.innerHTML = ['required', 'optional'].map(v =>
      `<button class="rt-nm-req-btn${v === requirement ? ' active' : ''}" data-r="${v}">
        ${v === 'required' ? 'Обязательно' : 'Опционально'}</button>`).join('');
    reqEl.querySelectorAll('[data-r]').forEach(b =>
      b.addEventListener('click', () => { requirement = b.dataset.r; drawReq(); }));
  };
  drawReq();

  overlay.querySelector('#rt-nm-del').addEventListener('click', async () => {
    await invoke('delete_routine_node', { id: node.id }).catch(() => {});
    overlay.remove();
    refresh();
  });
  overlay.querySelector('#rt-nm-save').addEventListener('click', async () => {
    await invoke('update_routine_node', {
      id: node.id,
      title: overlay.querySelector('#rt-nm-title').value.trim() || node.title,
      priority, requirement,
    }).catch(() => {});
    overlay.remove();
    refresh();
  });
}
