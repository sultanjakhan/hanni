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

function sourceIcon(t) {
  return t === 'event' ? '📅' : t === 'schedule' ? '🔁' : t === 'note' ? '📝' : '•';
}

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

async function openDropdown() {
  closeDropdown();
  const planned = await invoke('get_today_planned', { date: localDate() }).catch(() => []);
  const startable = planned.filter(p => !p.completed && !p.is_active && p.status_extra !== 'done');

  panel = document.createElement('div');
  panel.className = 'tw-panel';
  panel.innerHTML = `
    <div class="tw-panel-header">Запустить таск</div>
    <div class="tw-panel-body">
      ${startable.length === 0
        ? '<div class="tw-empty">Нет задач на сегодня</div>'
        : startable.map((p, i) => `
            <button class="tw-item" data-idx="${i}">
              <span class="tw-item-icon">${sourceIcon(p.source_type)}</span>
              <span class="tw-item-title">${escapeHtml(p.title)}</span>
              ${p.planned_time ? `<span class="tw-item-time">${escapeHtml(p.planned_time)}</span>` : ''}
            </button>`).join('')}
    </div>`;
  widget.appendChild(panel);

  panel.querySelectorAll('.tw-item').forEach(btn => {
    btn.addEventListener('click', async () => {
      const p = startable[parseInt(btn.dataset.idx)];
      await invoke('start_task_block', {
        sourceType: p.source_type,
        sourceId: p.source_id,
      }).catch(err => console.error('tw start:', err));
      closeDropdown();
      window.dispatchEvent(new Event('task-state-changed'));
      await refreshState();
    });
  });
}

async function onBtnClick(e) {
  e.stopPropagation();
  if (activeBlock) {
    await invoke('complete_task_block', { blockId: activeBlock.id })
      .catch(err => console.error('tw complete:', err));
    window.dispatchEvent(new Event('task-state-changed'));
    await refreshState();
    return;
  }
  if (panel) { closeDropdown(); return; }
  await openDropdown();
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
