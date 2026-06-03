// Calendar Day "Список" — flat task list with checkboxes and timer start/stop.

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml } from './utils.js';
import { SECTION_DEFS, renderItemRow, renderSection } from './calendar-task-list-row.js';
import { timeToMin } from './task-picker-sort.js';
import { effectivePriority, loadCategoryWeights } from './effective-priority.js';

// A timed item counts as past-time today within the same 3h grace as the picker.
// `enabled` is the opt-in flag (track_overdue for schedules; true for events —
// missed meeting is missed). Schedules without track_overdue are not flagged.
function isPastTimeToday(timeStr, done, isToday, enabled = true) {
  if (!isToday || done || !enabled) return false;
  const t = timeToMin(timeStr);
  if (t === null) return false;
  const now = new Date();
  const nowMin = now.getHours() * 60 + now.getMinutes();
  return t < nowMin && (nowMin - t) <= 180;
}

const SCH_CAT_ICONS = { health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡', practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽' };

// visible_from ("HH:MM"): on today, hide the schedule from the tasker until that
// time so evening items don't clutter the morning. Past/future days ignore it.
function hiddenUntilLater(visibleFrom, isToday) {
  if (!isToday || !visibleFrom) return false;
  const t = timeToMin(visibleFrom);
  if (t === null) return false;
  const now = new Date();
  return (now.getHours() * 60 + now.getMinutes()) < t;
}

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
  const skippedIds = new Set((completions || []).filter(c => c.status === 'skipped').map(c => c.schedule_id));
  // Closed yesterday = done OR explicitly "не выполнено" (skipped). Either way a
  // reflection / overdue schedule drops out of the tasker — it's been answered.
  const yClosedIds = new Set((yCompletions || []).filter(c => c.completed || c.status === 'skipped').map(c => c.schedule_id));
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
  // Overdue (only on today): overdue notes + track_overdue schedules missed
  // yesterday. Reflections are NOT overdue — they sit in today's normal list
  // (below) and only ever ask about yesterday, never older days.
  if (isViewingToday) {
    for (const t of (tasks || []).filter(t => t.status === 'task' && t.due_date && t.due_date < date)) {
      const bi = blockInfo('note', t.id);
      groups.overdue.push({ kind: 'note', id: t.id, title: t.title || 'Без названия', sortKey: t.due_date, icon: '⚠️', done: false, priority: t.priority || 0, overdueDate: t.due_date, completionDate: t.due_date, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: null });
    }
    for (const s of (scheds || []).filter(s => !s.chain_only && !s.marks_previous_day && s.track_overdue && scheduleMatchesDate(s, yesterday) && !yClosedIds.has(s.id))) {
      const bi = blockInfo('schedule', s.id);
      groups.overdue.push({ kind: 'schedule', id: s.id, title: s.title || 'Без названия', sortKey: yesterday, icon: SCH_CAT_ICONS[s.category] || '🔁', done: false, priority: 0, overdueDate: yesterday, completionDate: yesterday, status_extra: 'overdue', category: s.category, planned_time: s.time_of_day, marks_previous_day: false, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: s.target_minutes || null, trackingMode: s.tracking_mode || 'track' });
    }
  }
  // Today's list = normal schedules for `date` + reflections (за вчера). A
  // reflection marks yesterday, drops once answered (✓/✗), and only ever asks
  // about yesterday — never accumulates older days. visible_from hides not-yet-
  // due evening items from today's tasker.
  for (const s of (scheds || [])) {
    if (s.chain_only) continue;                       // lives only inside its chain run
    const isRefl = isViewingToday && !!s.marks_previous_day;
    const cdate = isRefl ? yesterday : date;          // reflection completion is yesterday's
    if (!scheduleMatchesDate(s, cdate)) continue;
    if (hiddenUntilLater(s.visible_from, isViewingToday)) continue;
    if (isRefl && yClosedIds.has(s.id)) continue;     // answered yesterday → gone
    const bi = blockInfo('schedule', s.id);
    const done = isRefl ? false : completedIds.has(s.id);
    const skipped = isRefl ? false : skippedIds.has(s.id);
    groups.schedule.push({ kind: 'schedule', id: s.id, title: s.title || 'Без названия', sortKey: s.time_of_day || '99:99', icon: SCH_CAT_ICONS[s.category] || '🔁', done, skipped, priority: 0, category: s.category, planned_time: s.time_of_day, marks_previous_day: !!s.marks_previous_day, completionDate: cdate, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: s.target_minutes || null, trackingMode: s.tracking_mode || 'track', pastTime: isPastTimeToday(s.time_of_day, done || skipped, isViewingToday, !!s.track_overdue) });
  }
  for (const e of (events || []).filter(e => e.date === date && e.source !== 'auto_health')) {
    const bi = blockInfo('event', e.id);
    const done = !!e.completed;
    groups.event.push({ kind: 'event', id: e.id, title: e.title || 'Без названия', sortKey: e.time || '99:99', icon: '📅', done, priority: e.priority || 0, category: e.category, planned_time: e.time, completionDate: e.date, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: null, pastTime: isPastTimeToday(e.time, done, isViewingToday) });
  }
  for (const t of (tasks || []).filter(t => t.due_date === date)) {
    const bi = blockInfo('note', t.id);
    groups.note.push({ kind: 'note', id: t.id, title: t.title || 'Без названия', sortKey: '99:99', icon: '📝', done: t.status === 'done', priority: t.priority || 0, category: 'task', completionDate: t.due_date, block: bi.activeBlock, actualMinutes: bi.actualMinutes, targetMinutes: null });
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
      <button class="ctl-hide-btn${S.calHideDone ? ' ctl-hide-on' : ''}" id="ctl-hide-done" style="margin-left:auto;" title="Показывать только невыполненные">${S.calHideDone ? '☑ Готовые скрыты' : '☐ Скрыть готовые'}</button>
      <div class="ctl-add-wrap">
        <button class="btn-primary" id="ctl-add">+ Добавить</button>
        <div class="ctl-add-menu" id="ctl-add-menu" hidden>
          <button class="ctl-add-menu-item" data-add-kind="event">📅 Событие</button>
          <button class="ctl-add-menu-item" data-add-kind="note">📝 Задача</button>
          <button class="ctl-add-menu-item" data-add-kind="cooking">🍳 Готовка</button>
          <button class="ctl-add-menu-item" data-add-kind="shopping">🛒 Покупки</button>
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

// Full re-render rebuilds every row, so doing it after each tap drops fast
// successive taps (the row is replaced mid-tap). Debounce it: the DB write fires
// per tap, the disruptive rebuild waits until tapping settles.
function scheduleCtlRefresh(el) {
  clearTimeout(el.__ctlRefresh);
  el.__ctlRefresh = setTimeout(() => {
    renderCalendarTaskList(el);
    window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
  }, 450);
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

function setHideDone(v) {
  S.calHideDone = v;
  try { localStorage.setItem('hanni_cal_hide_done', v ? '1' : '0'); } catch {}
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
  el.querySelector('#ctl-hide-done')?.addEventListener('click', () => { setHideDone(!S.calHideDone); renderCalendarTaskList(el); });

  const date = () => S.calDayDate || todayStr();
  const openEvent = () => { S.selectedCalendarDate = date(); tabLoaders.openCalendarAddEvent?.(); };
  const openCooking = async () => {
    const { showCookingLogModal } = await import('./food-cooking-log.js');
    showCookingLogModal(date(), () => renderCalendarTaskList(el));
  };
  const openShopping = async () => {
    const { showShoppingManager } = await import('./shopping-list-modal.js');
    showShoppingManager();
  };
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
      else if (btn.dataset.addKind === 'cooking') openCooking();
      else if (btn.dataset.addKind === 'shopping') openShopping();
    });
  });
  el.querySelector('#ctl-add-empty')?.addEventListener('click', openEvent);

  el.querySelectorAll('.ctl-row').forEach(row => {
    const kind = row.dataset.kind;
    // Schedules use UUIDv7 string ids — never parseInt (yields garbage like 19).
    const id = kind === 'schedule' ? row.dataset.id : parseInt(row.dataset.id);
    // completionDate differs from the view date for reflections (writes to yesterday).
    const cdate = row.dataset.completionDate || row.dataset.date || S.calDayDate || todayStr();
    row.querySelector('[data-ctl-check]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const willBeDone = !row.classList.contains('ctl-done');
      row.classList.toggle('ctl-done', willBeDone);  // optimistic — no mid-tap rebuild
      try { await toggleDone(kind, id, cdate, willBeDone); }
      catch (err) { console.error('ctl toggle:', err); }
      // Debounced rebuild so rapid successive taps aren't lost to a re-render.
      scheduleCtlRefresh(el);
    });
    // "Не выполнено" — schedule-only; toggles skipped/planned on the completion date.
    row.querySelector('[data-ctl-skip]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      try { await invoke('skip_schedule_completion', { scheduleId: id, date: cdate }); }
      catch (err) { console.error('ctl skip:', err); }
      renderCalendarTaskList(el);
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
    });
    row.querySelector('[data-ctl-start]')?.addEventListener('click', async (e) => {
      e.stopPropagation(); e.currentTarget.disabled = true;
      try { await invoke('start_task_block', { sourceType: kind, sourceId: String(id) }); }
      catch (err) { console.error('ctl start:', err); }
      renderCalendarTaskList(el);
    });
    row.querySelector('[data-ctl-pause]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const blockId = parseInt(e.currentTarget.dataset.ctlPause);
      e.currentTarget.disabled = true;
      try {
        await invoke('pause_task_block', { blockId });
        if (kind === 'schedule') await maybeAutoCheckSchedule(id, cdate);
      } catch (err) { console.error('ctl pause:', err); }
      renderCalendarTaskList(el);
    });
    row.querySelector('[data-ctl-finish]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const blockId = parseInt(e.currentTarget.dataset.ctlFinish);
      const title = row.querySelector('.ctl-title')?.textContent || '';
      const { showFinishModal } = await import('./calendar-task-list-finish-modal.js');
      showFinishModal(blockId, title, async () => {
        if (kind === 'schedule') await maybeAutoCheckSchedule(id, cdate);
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
  if (S.calHideDone === undefined) { try { S.calHideDone = localStorage.getItem('hanni_cal_hide_done') === '1'; } catch { S.calHideDone = false; } }
  const date = S.calDayDate;
  const d = new Date(date + 'T12:00:00');
  const monthsGen = ['января','февраля','марта','апреля','мая','июня','июля','августа','сентября','октября','ноября','декабря'];
  const dayNames = ['Воскресенье','Понедельник','Вторник','Среда','Четверг','Пятница','Суббота'];
  const dayLabel = `${d.getDate()} ${monthsGen[d.getMonth()]} · ${dayNames[d.getDay()]}`;
  const isToday = date === todayStr();

  const groups = await loadDayItems(date);
  const totalCount = groups.schedule.length + groups.event.length + groups.note.length;

  // effective_priority = manual priority + boosters. Sort by it (desc),
  // then by time (asc) so the morning-block still flows naturally.
  // Top-3 across all not-done items get a 🔥 badge.
  const weights = await loadCategoryWeights(invoke);
  const now = new Date();
  const nowMin = now.getHours() * 60 + now.getMinutes();
  const allItems = [...groups.overdue, ...groups.schedule, ...groups.event, ...groups.note];
  for (const it of allItems) it._eff = effectivePriority(it, weights, nowMin);
  const topThreeEff = allItems.filter(it => !it.done)
    .sort((a, b) => b._eff - a._eff).slice(0, 3)
    .filter(it => it._eff >= 2)
    .map(it => `${it.kind}:${it.id}`);
  for (const it of allItems) {
    it._isTopEff = topThreeEff.includes(`${it.kind}:${it.id}`);
  }
  for (const key of ['overdue', 'schedule', 'event', 'note']) {
    groups[key].sort((a, b) => {
      if (b._eff !== a._eff) return b._eff - a._eff;
      return (a.sortKey || '99:99').localeCompare(b.sortKey || '99:99');
    });
  }

  // Hide-completed filter: show only unanswered (not done, not skipped).
  if (S.calHideDone) {
    for (const key of ['overdue', 'schedule', 'event', 'note']) {
      groups[key] = groups[key].filter(it => !it.done && !it.skipped);
    }
  }
  const remaining = groups.overdue.length + groups.schedule.length + groups.event.length + groups.note.length;
  const bodyHtml = !totalCount
    ? `<div class="ctl-empty">
        <div class="ctl-empty-title">${isToday ? 'Сегодня свободно' : 'На этот день ничего не запланировано'}</div>
        <button class="btn-primary" id="ctl-add-empty">+ Запланировать</button>
      </div>`
    : (S.calHideDone && !remaining
        ? `<div class="ctl-empty"><div class="ctl-empty-title">Всё выполнено 🎉</div></div>`
        : SECTION_DEFS.map(def => renderSection(def, groups[def.key], date)).join(''));
  el.innerHTML = renderToolbar(dayLabel, isToday) + `<div class="ctl-body">${bodyHtml}</div>`;
  wire(el);
}
