// ── js/calendar-task-list.js — Calendar "Список задач" inner view ──
// Day/Week/Month list with checkboxes and timer start/stop. Reads same
// sources as Day-grid: events + recurring schedules + notes-tasks.

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml } from './utils.js';

const SCH_CAT_ICONS = { health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡', practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽' };
const LS_MODE_KEY = 'hanni_calendar_list_mode';

function loadMode() { try { return localStorage.getItem(LS_MODE_KEY) || 'day'; } catch { return 'day'; } }
function saveMode(m) { try { localStorage.setItem(LS_MODE_KEY, m); } catch {} }

export function todayStr() {
  const t = new Date();
  return `${t.getFullYear()}-${String(t.getMonth()+1).padStart(2,'0')}-${String(t.getDate()).padStart(2,'0')}`;
}
export function shiftDate(dateStr, days) {
  const d = new Date(dateStr + 'T12:00:00'); d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
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

function fmtTimeRange(start, durMin) {
  if (!start) return '—';
  if (!durMin) return start;
  const [h, m] = start.split(':').map(Number);
  const total = h * 60 + m + durMin;
  return `${start}–${String(Math.floor(total/60)%24).padStart(2,'0')}:${String(total%60).padStart(2,'0')}`;
}

export async function loadDayItems(date) {
  const d = new Date(date + 'T12:00:00');
  const [events, scheds, completions, tasks, blocks] = await Promise.all([
    invoke('get_events', { month: d.getMonth() + 1, year: d.getFullYear() }).catch(() => []),
    invoke('get_schedules', { category: null }).catch(() => []),
    invoke('get_schedule_completions', { date }).catch(() => []),
    invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []),
    invoke('get_timeline_blocks', { date }).catch(() => []),
  ]);
  const completedIds = new Set((completions || []).filter(c => c.completed).map(c => c.schedule_id));
  const blockBySrc = new Map();
  for (const b of (blocks || [])) if (b.source_type && b.source_id != null) blockBySrc.set(`${b.source_type}:${b.source_id}`, b);

  const items = [];
  for (const e of (events || []).filter(e => e.date === date)) {
    items.push({ kind: 'event', id: e.id, title: e.title || 'Без названия', time: e.time || '', durationMinutes: e.duration_minutes || null, icon: '📅', done: !!e.completed, block: blockBySrc.get(`event:${e.id}`) });
  }
  for (const s of (scheds || []).filter(s => scheduleMatchesDate(s, date))) {
    items.push({ kind: 'schedule', id: s.id, title: s.title || 'Без названия', time: s.time_of_day || '', durationMinutes: null, icon: SCH_CAT_ICONS[s.category] || '🔁', done: completedIds.has(s.id), block: blockBySrc.get(`schedule:${s.id}`) });
  }
  for (const t of (tasks || []).filter(t => t.due_date === date)) {
    items.push({ kind: 'note', id: t.id, title: t.title || 'Без названия', time: '', durationMinutes: null, icon: '📝', done: t.status === 'done', block: blockBySrc.get(`note:${t.id}`) });
  }
  return items;
}

function bucketByPartOfDay(items) {
  const b = { morning: [], afternoon: [], evening: [], untimed: [] };
  for (const it of items) {
    if (!it.time) { b.untimed.push(it); continue; }
    const h = parseInt(it.time.split(':')[0]);
    if (h >= 6 && h < 12) b.morning.push(it);
    else if (h >= 12 && h < 18) b.afternoon.push(it);
    else b.evening.push(it);
  }
  for (const k of Object.keys(b)) b[k].sort((a, c) => (a.time || '99').localeCompare(c.time || '99'));
  return b;
}

export function renderItemRow(item, dateStr) {
  const active = item.block?.is_active;
  const done = !!item.done;
  const cls = ['ctl-row', done && 'ctl-done', active && 'ctl-active'].filter(Boolean).join(' ');
  const timeText = item.time ? fmtTimeRange(item.time, item.durationMinutes) : '—';
  const durBadge = item.block?.duration_minutes ? `<span class="ctl-duration">${item.block.duration_minutes} мин</span>` : '';
  const trackBtn = active
    ? `<button class="ctl-track ctl-stop" data-ctl-stop="${item.block.id}" title="Завершить">■</button>`
    : `<button class="ctl-track ctl-start" data-ctl-start title="Запустить">▶</button>`;
  return `<div class="${cls}" data-kind="${item.kind}" data-id="${item.id}" data-date="${dateStr || ''}">
    <div class="ctl-check${done ? ' done' : ''}" data-ctl-check>${done ? '✓' : ''}</div>
    <span class="ctl-icon">${item.icon}</span>
    <span class="ctl-title">${escapeHtml(item.title)}</span>
    <span class="ctl-time">${timeText}</span>
    ${durBadge}
    ${trackBtn}
  </div>`;
}

export function renderToolbar(mode, label, atCurrent) {
  const navHtml = (mode === 'day' || mode === 'week') ? `
    <button class="calendar-nav-btn" id="ctl-prev">&lt;</button>
    <div class="calendar-month-label">${escapeHtml(label)}</div>
    <button class="calendar-nav-btn" id="ctl-next">&gt;</button>
    ${!atCurrent ? `<button class="cal-today-btn" id="ctl-today">${mode === 'week' ? 'Эта неделя' : 'Сегодня'}</button>` : ''}
    <button class="btn-primary" id="ctl-add" style="margin-left:auto;">+ Событие</button>
  ` : `<div class="calendar-month-label">Скоро</div>`;
  return `<div class="cal-list-toolbar">
    <div class="cal-list-nav">${navHtml}</div>
    <div class="cal-list-mode dev-filters">
      <button class="dev-filter-btn${mode==='day'?' active':''}" data-ctl-mode="day">📋 День</button>
      <button class="dev-filter-btn${mode==='week'?' active':''}" data-ctl-mode="week">📋 Неделя</button>
      <button class="dev-filter-btn${mode==='month'?' active':''}" data-ctl-mode="month">📋 Месяц</button>
    </div>
  </div>`;
}

export async function toggleDone(kind, id, date, willBeDone) {
  if (kind === 'event') {
    await invoke('update_event', { id, title: null, description: null, date: null, time: null, durationMinutes: null, category: null, color: null, completed: willBeDone });
  } else if (kind === 'schedule') {
    await invoke('toggle_schedule_completion', { scheduleId: id, date });
  } else if (kind === 'note') {
    await invoke('update_note_status', { id, status: willBeDone ? 'done' : 'task' });
  }
}

async function renderDayList(el) {
  if (!S.calDayDate) S.calDayDate = todayStr();
  const date = S.calDayDate;
  const d = new Date(date + 'T12:00:00');
  const monthsGen = ['января','февраля','марта','апреля','мая','июня','июля','августа','сентября','октября','ноября','декабря'];
  const dayNames = ['Воскресенье','Понедельник','Вторник','Среда','Четверг','Пятница','Суббота'];
  const dayLabel = `${d.getDate()} ${monthsGen[d.getMonth()]} · ${dayNames[d.getDay()]}`;
  const isToday = date === todayStr();

  const items = await loadDayItems(date);
  let bodyHtml;
  if (!items.length) {
    bodyHtml = `<div class="ctl-empty">
      <div class="ctl-empty-title">${isToday ? 'Сегодня свободно' : 'На этот день ничего не запланировано'}</div>
      <button class="btn-primary" id="ctl-add-empty">+ Запланировать</button>
    </div>`;
  } else {
    const b = bucketByPartOfDay(items);
    const sec = (label, list) => list.length ? `<div class="ctl-section"><div class="ctl-section-title">${label}</div>${list.map(it => renderItemRow(it, date)).join('')}</div>` : '';
    bodyHtml = sec('Утро', b.morning) + sec('День', b.afternoon) + sec('Вечер', b.evening) + sec('Без времени', b.untimed);
  }
  el.innerHTML = renderToolbar('day', dayLabel, isToday) + `<div class="ctl-body">${bodyHtml}</div>`;
  wireToolbar(el, 'day');
  wireRowActions(el);
}

export function wireToolbar(el, currentMode) {
  el.querySelectorAll('[data-ctl-mode]').forEach(btn => {
    btn.addEventListener('click', () => {
      if (btn.dataset.ctlMode === currentMode) return;
      saveMode(btn.dataset.ctlMode);
      renderCalendarTaskList(el);
    });
  });
  const step = currentMode === 'week' ? 7 : 1;
  const onPrev = () => { if (currentMode === 'week') S.calWeekOffset = (S.calWeekOffset || 0) - 1; else S.calDayDate = shiftDate(S.calDayDate || todayStr(), -step); renderCalendarTaskList(el); };
  const onNext = () => { if (currentMode === 'week') S.calWeekOffset = (S.calWeekOffset || 0) + 1; else S.calDayDate = shiftDate(S.calDayDate || todayStr(), step); renderCalendarTaskList(el); };
  const onToday = () => { if (currentMode === 'week') S.calWeekOffset = 0; else S.calDayDate = todayStr(); renderCalendarTaskList(el); };
  el.querySelector('#ctl-prev')?.addEventListener('click', onPrev);
  el.querySelector('#ctl-next')?.addEventListener('click', onNext);
  el.querySelector('#ctl-today')?.addEventListener('click', onToday);
  const openAdd = () => { S.selectedCalendarDate = S.calDayDate || todayStr(); tabLoaders.openCalendarAddEvent?.(); };
  el.querySelector('#ctl-add')?.addEventListener('click', openAdd);
  el.querySelector('#ctl-add-empty')?.addEventListener('click', openAdd);
}

export function wireRowActions(el) {
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
    row.querySelector('[data-ctl-stop]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const blockId = parseInt(e.currentTarget.dataset.ctlStop);
      e.currentTarget.disabled = true;
      try { await invoke('complete_task_block', { blockId }); }
      catch (err) { console.error('ctl stop:', err); }
      renderCalendarTaskList(el);
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

async function renderStub(el, mode) {
  el.innerHTML = renderToolbar(mode, '', false) + `<div class="ctl-empty">
    <div class="ctl-empty-title">Скоро</div>
    <div class="ctl-empty-desc">Месяц — в следующем коммите</div>
  </div>`;
  wireToolbar(el, mode);
}

export async function renderCalendarTaskList(el) {
  const mode = loadMode();
  if (mode === 'day') return renderDayList(el);
  if (mode === 'week') {
    const { renderWeekList } = await import('./calendar-task-list-week.js');
    return renderWeekList(el);
  }
  return renderStub(el, mode);
}
