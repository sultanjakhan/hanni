// task-control-widget.js — Floating button above quotes widget
// Idle: + opens dropdown with planned tasks → click = start
// Active: ■ pulsing red → click = stop
import { invoke } from './state.js';
import { renderRoutineSection, wireRoutineSection } from './routine-widget.js';
import { buildPickerBody, loadCategoryWeights } from './task-picker-view.js';
import { pickRecommendedTaskId, pickStartChainId, timeToMin } from './task-picker-sort.js';
import { isDanKoePractice, openDanKoeModal } from './dankoe-quick-modal.js';

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

async function openStartDropdown(preserveScroll = false) {
  // Inline interactions (check toggle, pin, routine step) re-open the picker —
  // preserve scroll position so users don't lose their place after each click.
  const savedScroll = preserveScroll && panel ? panel.scrollTop : 0;
  closeDropdown();
  const planned = await invoke('get_today_planned', { date: localDate() }).catch(() => []);
  // visible_from hides not-yet-due evening items from the picker (today only).
  const nowMin = new Date().getHours() * 60 + new Date().getMinutes();
  const notYetVisible = (p) => {
    const t = p.visible_from ? timeToMin(p.visible_from) : null;
    return t != null && nowMin < t;
  };
  const startable = planned.filter(p => !p.completed && !p.is_active && p.status_extra !== 'done' && p.status_extra !== 'skipped' && !notYetVisible(p));

  const weights = await loadCategoryWeights();
  const pins = await invoke('get_task_pins').catch(() => []);
  const avgRows = await invoke('get_task_avg_durations').catch(() => []);
  const avgDur = Object.fromEntries(avgRows.map(a => [`${a.source_type}:${a.source_id}`, a.avg_minutes]));
  const chains = await invoke('get_routine_chains').catch(() => []);
  const now = await invoke('get_routine_now', { date: localDate() }).catch(() => []);
  const completedChainIds = await invoke('get_completed_routine_chains', { date: localDate() }).catch(() => []);
  // Time/day-gate chains: a chain's "— начать" shows only when its FIRST step's
  // schedule is "к месту" now — day matches (frequency) AND now ≥ visible_from.
  // Derived from the schedules (no chain-level fields). Autonomous first step or
  // unknown → always shown. Active runs ignore this (handled in renderRoutineSection).
  const allScheds = await invoke('get_schedules', { category: null }).catch(() => []);
  const schedById = Object.fromEntries(allScheds.map(s => [String(s.id), s]));
  const dow = (new Date().getDay()) || 7;
  const schedDueNow = (s) => {
    if (!s || !s.is_active) return false;
    let dayOk;
    if (s.frequency === 'daily') dayOk = true;
    else if (s.frequency === 'weekly' || s.frequency === 'custom') dayOk = (s.frequency_days || '').split(',').map(Number).includes(dow);
    else dayOk = false;
    if (!dayOk) return false;
    const vf = s.visible_from ? timeToMin(s.visible_from) : null;
    return !(vf != null && nowMin < vf);
  };
  const chainDueNow = (c) => {
    // Explicit time trigger gates on the chain's earliest time of day. Multi-time
    // (meal) chains surface per-slot inside renderRoutineSection, not here.
    if (c.trigger_type === 'time' && c.trigger_time) {
      const ts = String(c.trigger_time).split(',').map(s => s.trim()).filter(Boolean).map(timeToMin);
      return ts.length ? nowMin >= Math.min(...ts) : true;
    }
    const start = (c.nodes || []).find(n => n.is_start);
    const startId = start ? start.id : null;
    const incoming = {};
    (c.edges || []).forEach(e => { (incoming[e.to_node_id] = incoming[e.to_node_id] || []).push(e.from_node_id); });
    const entries = (c.nodes || []).filter(n => !n.is_start &&
      ((incoming[n.id] || []).length === 0 || (incoming[n.id] || []).every(f => f === startId)));
    if (!entries.length) return true;
    return entries.some(n => !n.source_id || schedDueNow(schedById[String(n.source_id)]));
  };
  const dueChainIds = new Set((chains || []).filter(chainDueNow).map(c => c.id));
  // Recommendation precedence: active routine step → start an auto-trigger chain
  // (wake first; skip completed-today) → top regular task.
  const routineRecId = pickRecommendedTaskId(now);
  const chainRecId = routineRecId == null ? pickStartChainId(chains, now, completedChainIds.map(x => x.chain_id)) : null;
  const routineHtml = await renderRoutineSection(chains, now, routineRecId, chainRecId, completedChainIds, dueChainIds);
  const { bodyHtml, orderedItems } = await buildPickerBody({
    startable, weights, pins, avgDur, routineHtml,
    routineHasRec: routineRecId != null || chainRecId != null,
  });

  panel = document.createElement('div');
  panel.className = 'tw-panel';
  panel.innerHTML = `
    <div class="tw-panel-header"><span>Запустить таск</span></div>
    <div class="tw-panel-body">${bodyHtml}</div>
    <div class="tw-add-row">
      <input class="tw-add-input" type="text" placeholder="+ Новая задача на сегодня" maxlength="200">
    </div>`;
  widget.appendChild(panel);
  if (savedScroll > 0) panel.scrollTop = savedScroll;

  const addInput = panel.querySelector('.tw-add-input');
  addInput.addEventListener('click', (e) => e.stopPropagation());
  addInput.addEventListener('keydown', async (e) => {
    if (e.key !== 'Enter') return;
    const title = addInput.value.trim();
    if (!title) return;
    await invoke('create_note', {
      title, content: '', tags: '', status: 'task', tabName: null,
      dueDate: localDate(), reminderAt: null,
    }).catch(() => {});
    await openStartDropdown();
  });

  panel.querySelectorAll('.tw-pin[data-pin-idx]').forEach(star => {
    star.addEventListener('click', async (e) => {
      e.stopPropagation();
      const p = orderedItems[parseInt(star.dataset.pinIdx)];
      await invoke('toggle_task_pin', { sourceType: p.source_type, sourceId: p.source_id }).catch(() => {});
      await openStartDropdown(true);
    });
  });

  // ✓/✗ pair for reflections / instant schedules — mark done / "не выполнено".
  panel.querySelectorAll('.tw-check[data-check-idx]').forEach(x => {
    x.addEventListener('click', async (e) => {
      e.stopPropagation();
      const p = orderedItems[parseInt(x.dataset.checkIdx)];
      await invoke('toggle_schedule_completion', { scheduleId: p.source_id, date: p.completion_date || localDate() }).catch(() => {});
      window.dispatchEvent(new Event('task-state-changed'));
      await openStartDropdown(true);
    });
  });
  panel.querySelectorAll('.tw-skip[data-skip-idx]').forEach(x => {
    x.addEventListener('click', async (e) => {
      e.stopPropagation();
      const p = orderedItems[parseInt(x.dataset.skipIdx)];
      await invoke('skip_schedule_completion', { scheduleId: p.source_id, date: p.completion_date || localDate() }).catch(() => {});
      window.dispatchEvent(new Event('task-state-changed'));
      await openStartDropdown(true);
    });
  });

  wireRoutineSection(panel, () => openStartDropdown(true));

  panel.querySelectorAll('.tw-item[data-idx]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const p = orderedItems[parseInt(btn.dataset.idx)];
      // Dan Koe practices open a journaling modal (text + history), not a timer.
      if (p.source_type === 'schedule' && isDanKoePractice(p.title)) {
        closeDropdown();
        await openDanKoeModal(p.title, p.source_id, () => {
          window.dispatchEvent(new Event('task-state-changed'));
        });
        return;
      }
      const isCheck = p.tracking_mode === 'check';
      try {
        if (isCheck && p.source_type === 'schedule') {
          // Reflections write to yesterday (completion_date), not today.
          await invoke('toggle_schedule_completion', { scheduleId: p.source_id, date: p.completion_date || localDate() });
        } else {
          await invoke('start_task_block', { sourceType: p.source_type, sourceId: String(p.source_id) });
        }
      } catch (err) { console.error('tw item click:', err); }
      window.dispatchEvent(new Event('task-state-changed'));
      // Check-mode is a quick toggle — keep the menu open so several can be
      // marked in a row. Track-mode starts a timer; close so the timer UI shows.
      if (isCheck && p.source_type === 'schedule') {
        await openStartDropdown(true);
      } else {
        closeDropdown();
        await refreshState();
      }
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
