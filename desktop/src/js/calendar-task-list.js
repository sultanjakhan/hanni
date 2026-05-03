// Calendar Day "Список" — flat task list with checkboxes and timer start/stop.

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml } from './utils.js';
import { SECTION_DEFS, renderItemRow, renderSection } from './calendar-task-list-row.js';

const SCH_CAT_ICONS = { health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡', practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽' };

const fmtDate = (d) => `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
const todayStr = () => fmtDate(new Date());
function shiftDate(dateStr, days) {
  const d = new Date(dateStr + 'T12:00:00'); d.setDate(d.getDate() + days); return fmtDate(d);
}
function scheduleMatchesDate(sch, dateStr) {
  if (!sch.is_active) return false;
  if (sch.until_date && dateStr > sch.until_date) return false;
  if (sch.frequency === 'daily') return true;
  const dow = (new Date(dateStr + 'T12:00:00').getDay()) || 7;
  if (sch.frequency === 'weekly') {
    const days = sch.frequency_days ? sch.frequency_days.split(',').map(Number) : [1];
    return days.includes(dow);
  }
  if (sch.frequency === 'custom' && sch.frequency_days) return sch.frequency_days.split(',').map(Number).includes(dow);
  return false;
}

async function loadDayItems(date) {
  const d = new Date(date + 'T12:00:00');
  const isViewingToday = date === todayStr();
  const yesterday = isViewingToday ? shiftDate(date, -1) : null;
  const [events, scheds, completions, tasks, blocks, yCompletions] = await Promise.all([
    invoke('get_events', { month: d.getMonth() + 1, year: d.getFullYear() }).catch(() => []),
    invoke('get_schedules', { category: null }).catch(() => []),
    invoke('get_schedule_completions', { date }).catch(() => []),
    invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []),
    invoke('get_timeline_blocks', { date }).catch(() => []),
    yesterday ? invoke('get_schedule_completions', { date: yesterday }).catch(() => []) : Promise.resolve([]),
  ]);
  const completedIds = new Set((completions || []).filter(c => c.completed).map(c => c.schedule_id));
  const yCompletedIds = new Set((yCompletions || []).filter(c => c.completed).map(c => c.schedule_id));
  // Group all blocks by source: each source can have multiple blocks per day (target_minutes feature)
  const blocksBySrc = new Map();
  for (const b of (blocks || [])) {
    if (!b.source_type || b.source_id == null) continue;
    const key = `${b.source_type}:${b.source_id}`;
    if (!blocksBySrc.has(key)) blocksBySrc.set(key, []);
    blocksBySrc.get(key).push(b);
  }
  const blockInfo = (kind, id) => {
    const arr = blocksBySrc.get(`${kind}:${id}`) || [];
    const active = arr.find(b => b.is_active);
    const actualMinutes = arr.filter(b => !b.is_active).reduce((sum, b) => sum + (b.duration_minutes || 0), 0);
    return { activeBlock: active, actualMinutes };
  };

  const groups = { overdue: [], schedule: [], event: [], note: [] };
  // Overdue (only on today): notes with due_date<today + schedules with track_overdue missed yesterday
  if (isViewingToday) {
    for (const t of (tasks || []).filter(t => t.status === 'task' && t.due_date && t.due_date < date)) {
      const bi = blockInfo('note', t.id);
      groups.overdue.push({ kind: 'note', id: t.id, title: t.title || 'Без названия', sortKey: t.due_date, icon: '⚠️', done: false, priority: t.priority || 0, overdueDate: t.due_date, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: null });
    }
    for (const s of (scheds || []).filter(s => s.track_overdue && scheduleMatchesDate(s, yesterday) && !yCompletedIds.has(s.id))) {
      const bi = blockInfo('schedule', s.id);
      groups.overdue.push({ kind: 'schedule', id: s.id, title: s.title || 'Без названия', sortKey: yesterday, icon: SCH_CAT_ICONS[s.category] || '🔁', done: false, priority: 0, overdueDate: yesterday, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: s.target_minutes || null });
    }
  }
  for (const s of (scheds || []).filter(s => scheduleMatchesDate(s, date))) {
    const bi = blockInfo('schedule', s.id);
    groups.schedule.push({ kind: 'schedule', id: s.id, title: s.title || 'Без названия', sortKey: s.time_of_day || '99:99', icon: SCH_CAT_ICONS[s.category] || '🔁', done: completedIds.has(s.id), priority: 0, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: s.target_minutes || null });
  }
  for (const e of (events || []).filter(e => e.date === date)) {
    const bi = blockInfo('event', e.id);
    groups.event.push({ kind: 'event', id: e.id, title: e.title || 'Без названия', sortKey: e.time || '99:99', icon: '📅', done: !!e.completed, priority: e.priority || 0, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: null });
  }
  for (const t of (tasks || []).filter(t => t.due_date === date)) {
    const bi = blockInfo('note', t.id);
    groups.note.push({ kind: 'note', id: t.id, title: t.title || 'Без названия', sortKey: '99:99', icon: '📝', done: t.status === 'done', priority: t.priority || 0, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: null });
  }
  for (const k of Object.keys(groups)) {
    groups[k].sort((a, b) => (b.priority - a.priority) || a.sortKey.localeCompare(b.sortKey));
  }
  return groups;
}


function renderToolbar(dayLabel, isToday) {
  return `<div class="cal-list-toolbar">
    <div class="cal-list-nav">
      <button class="calendar-nav-btn" id="ctl-prev">&lt;</button>
      <div class="calendar-month-label">${escapeHtml(dayLabel)}</div>
      <button class="calendar-nav-btn" id="ctl-next">&gt;</button>
      ${!isToday ? `<button class="cal-today-btn" id="ctl-today">Сегодня</button>` : ''}
      <div class="ctl-add-wrap" style="margin-left:auto;">
        <button class="btn-primary" id="ctl-add">+ Добавить</button>
        <div class="ctl-add-menu" id="ctl-add-menu" hidden>
          <button class="ctl-add-menu-item" data-add-kind="event">📅 Событие</button>
          <button class="ctl-add-menu-item" data-add-kind="note">📝 Задача</button>
        </div>
      </div>
    </div>
    <div class="day-mode-tabs dev-filters" data-view-toggle="day">
      <button class="dev-filter-btn" data-view-mode="grid">📅 Календарь</button>
      <button class="dev-filter-btn active" data-view-mode="list">📋 Список</button>
    </div>
  </div>`;
}

// Auto-tick schedule completion when target_minutes reached.
// Called after pause/finish for schedule items; no-op if no target or already done.
async function maybeAutoCheckSchedule(scheduleId, date) {
  try {
    const [scheds, completions, blocks] = await Promise.all([
      invoke('get_schedules', { category: null }).catch(() => []),
      invoke('get_schedule_completions', { date }).catch(() => []),
      invoke('get_timeline_blocks', { date }).catch(() => []),
    ]);
    const sch = scheds.find(s => s.id === scheduleId);
    if (!sch || !sch.target_minutes) return;
    const total = blocks
      .filter(b => b.source_type === 'schedule' && b.source_id === scheduleId && !b.is_active)
      .reduce((sum, b) => sum + (b.duration_minutes || 0), 0);
    const isDone = completions.some(c => c.schedule_id === scheduleId && c.completed);
    if (total >= sch.target_minutes && !isDone) {
      await invoke('toggle_schedule_completion', { scheduleId, date });
    }
  } catch (e) { console.error('auto-check:', e); }
}

async function toggleDone(kind, id, date, willBeDone) {
  if (kind === 'event') {
    await invoke('update_event', { id, title: null, description: null, date: null, time: null, durationMinutes: null, category: null, color: null, completed: willBeDone });
  } else if (kind === 'schedule') {
    await invoke('toggle_schedule_completion', { scheduleId: id, date });
  } else if (kind === 'note') {
    await invoke('update_note_status', { id, status: willBeDone ? 'done' : 'task' });
  }
}

function setDayViewMode(mode) {
  if (!S.calViewMode) S.calViewMode = {};
  S.calViewMode.day = mode;
  try { localStorage.setItem('hanni_calendar_view_mode_day', mode); } catch {}
}

function wire(el) {
  el.querySelectorAll('[data-view-toggle="day"] [data-view-mode]').forEach(btn => {
    btn.addEventListener('click', () => {
      if (btn.dataset.viewMode === 'list') return;
      setDayViewMode('grid');
      window.dispatchEvent(new Event('task-state-changed'));
    });
  });
  el.querySelector('#ctl-prev')?.addEventListener('click', () => { S.calDayDate = shiftDate(S.calDayDate || todayStr(), -1); renderCalendarTaskList(el); });
  el.querySelector('#ctl-next')?.addEventListener('click', () => { S.calDayDate = shiftDate(S.calDayDate || todayStr(), 1); renderCalendarTaskList(el); });
  el.querySelector('#ctl-today')?.addEventListener('click', () => { S.calDayDate = todayStr(); renderCalendarTaskList(el); });

  const date = () => S.calDayDate || todayStr();
  const openEvent = () => { S.selectedCalendarDate = date(); tabLoaders.openCalendarAddEvent?.(); };
  const openNewNote = async () => {
    try {
      const id = await invoke('create_note', { title: '', content: '', tags: '', status: 'task', tabName: null, dueDate: date(), reminderAt: null });
      tabLoaders.switchTab?.('notes');
      setTimeout(() => { S.currentNoteId = id; S.notesViewMode = 'edit'; const n = document.getElementById('notes-content'); if (n) tabLoaders.renderNoteEditor?.(n, id); }, 100);
    } catch (err) { console.error('ctl new note:', err); }
  };

  const menu = el.querySelector('#ctl-add-menu');
  const wrap = el.querySelector('.ctl-add-wrap');
  const closeOnOutside = (ev) => {
    if (!menu || menu.hidden) return;
    if (wrap && wrap.contains(ev.target)) return;
    menu.hidden = true;
  };
  const closeOnEsc = (ev) => { if (ev.key === 'Escape' && menu) menu.hidden = true; };
  el.querySelector('#ctl-add')?.addEventListener('click', (e) => {
    e.stopPropagation();
    if (!menu) return;
    menu.hidden = !menu.hidden;
    if (!menu.hidden) {
      document.addEventListener('mousedown', closeOnOutside);
      document.addEventListener('keydown', closeOnEsc);
    } else {
      document.removeEventListener('mousedown', closeOnOutside);
      document.removeEventListener('keydown', closeOnEsc);
    }
  });
  el.querySelectorAll('[data-add-kind]').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      if (menu) menu.hidden = true;
      document.removeEventListener('mousedown', closeOnOutside);
      document.removeEventListener('keydown', closeOnEsc);
      if (btn.dataset.addKind === 'event') openEvent();
      else if (btn.dataset.addKind === 'note') openNewNote();
    });
  });
  el.querySelector('#ctl-add-empty')?.addEventListener('click', openEvent);

  el.querySelectorAll('.ctl-row').forEach(row => {
    const kind = row.dataset.kind;
    const id = parseInt(row.dataset.id);
    const date = row.dataset.date || S.calDayDate || todayStr();
    row.querySelector('[data-ctl-check]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      try { await toggleDone(kind, id, date, !row.classList.contains('ctl-done')); }
      catch (err) { console.error('ctl toggle:', err); }
      renderCalendarTaskList(el);
    });
    row.querySelector('[data-ctl-start]')?.addEventListener('click', async (e) => {
      e.stopPropagation(); e.currentTarget.disabled = true;
      try { await invoke('start_task_block', { sourceType: kind, sourceId: id }); }
      catch (err) { console.error('ctl start:', err); }
      renderCalendarTaskList(el);
    });
    row.querySelector('[data-ctl-pause]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const blockId = parseInt(e.currentTarget.dataset.ctlPause);
      e.currentTarget.disabled = true;
      try {
        await invoke('pause_task_block', { blockId });
        if (kind === 'schedule') await maybeAutoCheckSchedule(id, date);
      } catch (err) { console.error('ctl pause:', err); }
      renderCalendarTaskList(el);
    });
    row.querySelector('[data-ctl-finish]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const blockId = parseInt(e.currentTarget.dataset.ctlFinish);
      const title = row.querySelector('.ctl-title')?.textContent || '';
      const { showFinishModal } = await import('./calendar-task-list-finish-modal.js');
      showFinishModal(blockId, title, async () => {
        if (kind === 'schedule') await maybeAutoCheckSchedule(id, date);
        renderCalendarTaskList(el);
      });
    });
    row.querySelector('.ctl-title')?.addEventListener('click', (e) => {
      e.stopPropagation();
      if (kind === 'note') {
        tabLoaders.switchTab?.('notes');
        setTimeout(() => { S.currentNoteId = id; S.notesViewMode = 'edit'; const n = document.getElementById('notes-content'); if (n) tabLoaders.renderNoteEditor?.(n, id); }, 100);
      }
    });
  });
}


export async function renderCalendarTaskList(el, opts) {
  if (opts && opts.start && opts.end && opts.start !== opts.end) {
    const { renderPeriodMode } = await import('./calendar-task-list-period.js');
    return renderPeriodMode(el, opts);
  }
  if (!S.calDayDate) S.calDayDate = todayStr();
  const date = S.calDayDate;
  const d = new Date(date + 'T12:00:00');
  const monthsGen = ['января','февраля','марта','апреля','мая','июня','июля','августа','сентября','октября','ноября','декабря'];
  const dayNames = ['Воскресенье','Понедельник','Вторник','Среда','Четверг','Пятница','Суббота'];
  const dayLabel = `${d.getDate()} ${monthsGen[d.getMonth()]} · ${dayNames[d.getDay()]}`;
  const isToday = date === todayStr();

  const groups = await loadDayItems(date);
  const totalCount = groups.schedule.length + groups.event.length + groups.note.length;
  const bodyHtml = totalCount
    ? SECTION_DEFS.map(def => renderSection(def, groups[def.key], date)).join('')
    : `<div class="ctl-empty">
        <div class="ctl-empty-title">${isToday ? 'Сегодня свободно' : 'На этот день ничего не запланировано'}</div>
        <button class="btn-primary" id="ctl-add-empty">+ Запланировать</button>
      </div>`;
  el.innerHTML = renderToolbar(dayLabel, isToday) + `<div class="ctl-body">${bodyHtml}</div>`;
  wire(el);
}
