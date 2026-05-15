// task-control-widget.js — Floating button above quotes widget
// Idle: + opens dropdown with planned tasks → click = start
// Active: ■ pulsing red → click = stop
import { invoke } from './state.js';

let widget = null;
let panel = null;
let activeBlock = null;
let pollTimer = null;

const PLUS_SVG = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
  stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>`;

const STOP_SVG = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
  <rect x="6" y="6" width="12" height="12" rx="1"/></svg>`;

function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}

function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"']/g, c => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
  ));
}

// Mirrors SCH_CAT_ICONS in tab-calendar.js / calendar-task-list.js. Keep in sync.
const SCH_CAT_ICONS = {
  health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡',
  practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽'
};

function taskIcon(p) {
  if (p.source_type === 'schedule') return SCH_CAT_ICONS[p.category] || SCH_CAT_ICONS.other;
  if (p.source_type === 'event') return '📅';
  if (p.source_type === 'note') return '📝';
  return '•';
}

const GROUP_TITLES = { event: 'События', schedule: 'Расписание', note: 'Заметки' };

async function refreshState() {
  const blocks = await invoke('get_timeline_blocks', { date: localDate() }).catch(() => []);
  activeBlock = blocks.find(b => b.is_active) || null;
  render();
}

function render() {
  if (!widget) return;
  const btn = widget.querySelector('.tw-btn');
  if (!btn) return;
  if (activeBlock) {
    btn.classList.add('tw-active');
    btn.innerHTML = STOP_SVG;
    const label = activeBlock.notes || activeBlock.type_name || 'таск';
    btn.title = `Идёт: ${label} с ${activeBlock.start_time}`;
  } else {
    btn.classList.remove('tw-active');
    btn.innerHTML = PLUS_SVG;
    btn.title = 'Запустить таск';
  }
}

function closeDropdown() {
  if (panel) { panel.remove(); panel = null; }
}

async function openStartDropdown() {
  closeDropdown();
  const planned = await invoke('get_today_planned', { date: localDate() }).catch(() => []);
  const startable = planned.filter(p => !p.completed && !p.is_active && p.status_extra !== 'done');

  const groups = {
    event:    startable.filter(p => p.source_type === 'event'),
    schedule: startable.filter(p => p.source_type === 'schedule')
                       .sort((a, b) => (a.category || '').localeCompare(b.category || '')),
    note:     startable.filter(p => p.source_type === 'note'),
  };
  const nonEmpty = Object.entries(groups).filter(([, items]) => items.length > 0);
  const showHeaders = nonEmpty.length > 1;
  const orderedItems = nonEmpty.flatMap(([, items]) => items);

  panel = document.createElement('div');
  panel.className = 'tw-panel';
  panel.innerHTML = `
    <div class="tw-panel-header">Запустить таск</div>
    <div class="tw-panel-body">
      ${orderedItems.length === 0
        ? '<div class="tw-empty">Нет задач на сегодня</div>'
        : nonEmpty.map(([key, items]) => `
            ${showHeaders ? `<div class="tw-group-header">${GROUP_TITLES[key]}</div>` : ''}
            ${items.map(p => {
              const idx = orderedItems.indexOf(p);
              return `
                <button class="tw-item" data-idx="${idx}">
                  <span class="tw-item-icon">${taskIcon(p)}</span>
                  <span class="tw-item-title">${escapeHtml(p.title)}</span>
                </button>`;
            }).join('')}
          `).join('')}
    </div>`;
  widget.appendChild(panel);

  panel.querySelectorAll('.tw-item').forEach(btn => {
    btn.addEventListener('click', async () => {
      const p = orderedItems[parseInt(btn.dataset.idx)];
      const isCheck = p.tracking_mode === 'check';
      try {
        if (isCheck && p.source_type === 'schedule') {
          await invoke('toggle_schedule_completion', { scheduleId: p.source_id, date: localDate() });
        } else {
          await invoke('start_task_block', { sourceType: p.source_type, sourceId: p.source_id });
        }
      } catch (err) { console.error('tw item click:', err); }
      closeDropdown();
      window.dispatchEvent(new Event('task-state-changed'));
      await refreshState();
    });
  });
}

function openActiveActions() {
  closeDropdown();
  if (!activeBlock) return;
  const label = activeBlock.notes || activeBlock.type_name || 'таск';
  panel = document.createElement('div');
  panel.className = 'tw-panel tw-panel-actions';
  panel.innerHTML = `
    <div class="tw-panel-header">Идёт: ${escapeHtml(label)} с ${activeBlock.start_time}</div>
    <div class="tw-panel-body">
      <button class="tw-action tw-action-pause" data-action="pause">
        <span class="tw-action-icon">⏸</span>
        <span class="tw-action-label">Пауза</span>
        <span class="tw-action-hint">блок закрывается, статус не меняется</span>
      </button>
      <button class="tw-action tw-action-finish" data-action="finish">
        <span class="tw-action-icon">✓</span>
        <span class="tw-action-label">Завершить</span>
        <span class="tw-action-hint">отметить как сделано</span>
      </button>
      <button class="tw-action tw-action-cancel" data-action="cancel">
        <span class="tw-action-icon">✕</span>
        <span class="tw-action-label">Отмена</span>
        <span class="tw-action-hint">удалить блок без зачёта времени</span>
      </button>
    </div>`;
  widget.appendChild(panel);

  panel.querySelectorAll('[data-action]').forEach(btn => {
    btn.addEventListener('click', async () => {
      if (!activeBlock) return;
      const id = activeBlock.id;
      const action = btn.dataset.action;
      try {
        if (action === 'pause') await invoke('pause_task_block', { blockId: id });
        else if (action === 'finish') await invoke('complete_task_block', { blockId: id });
        else if (action === 'cancel') await invoke('delete_timeline_block', { id });
      } catch (err) { console.error('tw action:', err); }
      closeDropdown();
      window.dispatchEvent(new Event('task-state-changed'));
      await refreshState();
    });
  });
}

async function onBtnClick(e) {
  e.stopPropagation();
  if (panel) { closeDropdown(); return; }
  if (activeBlock) {
    openActiveActions();
    return;
  }
  await openStartDropdown();
}

export function initTaskControlWidget() {
  const existing = document.getElementById('task-control-widget');
  if (existing) existing.remove();

  widget = document.createElement('div');
  widget.id = 'task-control-widget';
  widget.className = 'task-widget';
  widget.innerHTML = `<button class="tw-btn">${PLUS_SVG}</button>`;
  document.getElementById('content-area').appendChild(widget);

  widget.querySelector('.tw-btn').addEventListener('click', onBtnClick);

  document.addEventListener('click', (e) => {
    if (panel && !widget.contains(e.target)) closeDropdown();
  });

  window.addEventListener('task-state-changed', refreshState);

  refreshState();
  if (pollTimer) clearInterval(pollTimer);
  pollTimer = setInterval(refreshState, 30000);
}
