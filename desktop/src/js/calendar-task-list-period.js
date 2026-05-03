// Calendar Week/Month list — events + tasks for the period (no schedules).

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml } from './utils.js';

const MONTHS_SHORT = ['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'];
const MONTHS_FULL = ['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
const WEEKDAYS = ['Воскресенье','Понедельник','Вторник','Среда','Четверг','Пятница','Суббота'];

const pad2 = (n) => String(n).padStart(2, '0');
const fmtDate = (d) => `${d.getFullYear()}-${pad2(d.getMonth()+1)}-${pad2(d.getDate())}`;

async function loadPeriodItems(start, end) {
  const sd = new Date(start + 'T12:00:00');
  const ed = new Date(end + 'T12:00:00');
  const months = new Set();
  let cur = new Date(sd.getFullYear(), sd.getMonth(), 1);
  const lastMonthStart = new Date(ed.getFullYear(), ed.getMonth(), 1);
  while (cur <= lastMonthStart) {
    months.add(`${cur.getFullYear()}-${cur.getMonth()+1}`);
    cur.setMonth(cur.getMonth() + 1);
  }
  const eventsArrs = await Promise.all([...months].map(k => {
    const [y, m] = k.split('-').map(Number);
    return invoke('get_events', { month: m, year: y }).catch(() => []);
  }));
  const events = eventsArrs.flat().filter(e => e.date >= start && e.date <= end);
  const tasks = (await invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []))
    .filter(t => t.due_date && t.due_date >= start && t.due_date <= end);
  return { events, tasks };
}

function renderPeriodRow(item) {
  const done = !!item.done;
  const cls = ['ctl-row', done && 'ctl-done'].filter(Boolean).join(' ');
  return `<div class="${cls}" data-kind="${item.kind}" data-id="${item.id}" data-date="${item.date}">
    <span class="ctl-priority"></span>
    <div class="ctl-check${done ? ' done' : ''}" data-ctl-check>${done ? '✓' : ''}</div>
    <span class="ctl-icon">${item.icon}</span>
    ${item.time ? `<span class="ctl-time">${escapeHtml(item.time)}</span>` : ''}
    <span class="ctl-title">${escapeHtml(item.title)}</span>
  </div>`;
}

function renderDayBlock(date, dayEvts, dayTasks, todayStr) {
  if (!dayEvts.length && !dayTasks.length) return '';
  const d = new Date(date + 'T12:00:00');
  const dateLabel = `${d.getDate()} ${MONTHS_SHORT[d.getMonth()]} · ${WEEKDAYS[d.getDay()]}`;
  const isToday = date === todayStr;
  const items = [
    ...dayEvts.map(e => ({ kind: 'event', id: e.id, title: e.title || 'Без названия', icon: '📅', time: e.time || '', done: !!e.completed, date })),
    ...dayTasks.map(t => ({ kind: 'note', id: t.id, title: t.title || 'Без названия', icon: '📝', time: '', done: t.status === 'done', date })),
  ];
  items.sort((a, b) => (a.time || '99:99').localeCompare(b.time || '99:99'));
  const rows = items.map(renderPeriodRow).join('');
  return `<div class="ctl-day-block">
    <div class="ctl-day-header${isToday ? ' ctl-day-today' : ''}" data-day-jump="${date}">${dateLabel}</div>
    ${rows}
  </div>`;
}

function renderPeriodToolbar(view, start, end) {
  const sd = new Date(start + 'T12:00:00');
  const ed = new Date(end + 'T12:00:00');
  const label = view === 'week'
    ? `${sd.getDate()} ${MONTHS_SHORT[sd.getMonth()]} — ${ed.getDate()} ${MONTHS_SHORT[ed.getMonth()]} ${ed.getFullYear()}`
    : `${MONTHS_FULL[sd.getMonth()]} ${sd.getFullYear()}`;
  return `<div class="cal-list-toolbar">
    <div class="cal-list-nav">
      <button class="calendar-nav-btn" data-period-prev>&lt;</button>
      <div class="calendar-month-label">${escapeHtml(label)}</div>
      <button class="calendar-nav-btn" data-period-next>&gt;</button>
      <button class="cal-today-btn" data-period-today>Сегодня</button>
    </div>
    <div class="day-mode-tabs dev-filters" data-view-toggle="${view}">
      <button class="dev-filter-btn" data-view-mode="grid">📅 Календарь</button>
      <button class="dev-filter-btn active" data-view-mode="list">📋 Список</button>
    </div>
  </div>`;
}

export async function renderPeriodMode(el, opts) {
  const { start, end, view } = opts;
  const { events, tasks } = await loadPeriodItems(start, end);
  const byDate = {};
  for (const e of events) (byDate[e.date] ||= { evts: [], tasks: [] }).evts.push(e);
  for (const t of tasks) (byDate[t.due_date] ||= { evts: [], tasks: [] }).tasks.push(t);

  const todayStr = fmtDate(new Date());
  const days = [];
  const sd = new Date(start + 'T12:00:00');
  const ed = new Date(end + 'T12:00:00');
  for (let d = new Date(sd); d <= ed; d.setDate(d.getDate() + 1)) days.push(fmtDate(d));

  const blocks = days
    .map(date => renderDayBlock(date, byDate[date]?.evts || [], byDate[date]?.tasks || [], todayStr))
    .filter(Boolean).join('');
  const empty = !blocks
    ? `<div class="ctl-empty"><div class="ctl-empty-title">${view === 'week' ? 'На этой неделе ничего не запланировано' : 'В этом месяце ничего не запланировано'}</div></div>`
    : '';

  el.innerHTML = renderPeriodToolbar(view, start, end) + `<div class="ctl-body ctl-body-period">${blocks}${empty}</div>`;
  wirePeriod(el, view);
}

function setViewMode(view, mode) {
  if (!S.calViewMode) S.calViewMode = {};
  S.calViewMode[view] = mode;
  try { localStorage.setItem(`hanni_calendar_view_mode_${view}`, mode); } catch {}
}

function wirePeriod(el, view) {
  el.querySelectorAll(`[data-view-toggle="${view}"] [data-view-mode="grid"]`).forEach(btn => {
    btn.addEventListener('click', () => {
      setViewMode(view, 'grid');
      window.dispatchEvent(new Event('task-state-changed'));
    });
  });
  el.querySelector('[data-period-prev]')?.addEventListener('click', () => {
    if (view === 'week') S.calWeekOffset = (S.calWeekOffset || 0) - 1;
    else { S.calendarMonth--; if (S.calendarMonth < 0) { S.calendarMonth = 11; S.calendarYear--; } }
    window.dispatchEvent(new Event('task-state-changed'));
  });
  el.querySelector('[data-period-next]')?.addEventListener('click', () => {
    if (view === 'week') S.calWeekOffset = (S.calWeekOffset || 0) + 1;
    else { S.calendarMonth++; if (S.calendarMonth > 11) { S.calendarMonth = 0; S.calendarYear++; } }
    window.dispatchEvent(new Event('task-state-changed'));
  });
  el.querySelector('[data-period-today]')?.addEventListener('click', () => {
    const t = new Date();
    if (view === 'week') S.calWeekOffset = 0;
    else { S.calendarMonth = t.getMonth(); S.calendarYear = t.getFullYear(); }
    window.dispatchEvent(new Event('task-state-changed'));
  });
  el.querySelectorAll('[data-day-jump]').forEach(h => {
    h.addEventListener('click', () => {
      const date = h.dataset.dayJump;
      S.calDayDate = date;
      S._calendarInner = 'day';
      const d = new Date(date + 'T12:00:00');
      S.calendarMonth = d.getMonth(); S.calendarYear = d.getFullYear();
      setViewMode('day', 'list');
      document.querySelectorAll('[data-calview]').forEach(b => b.classList.toggle('active', b.dataset.calview === 'day'));
      window.dispatchEvent(new Event('task-state-changed'));
    });
  });
  el.querySelectorAll('.ctl-row').forEach(row => {
    const kind = row.dataset.kind;
    const id = parseInt(row.dataset.id);
    row.querySelector('[data-ctl-check]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const willBeDone = !row.classList.contains('ctl-done');
      try {
        if (kind === 'event') {
          await invoke('update_event', { id, title: null, description: null, date: null, time: null, durationMinutes: null, category: null, color: null, completed: willBeDone });
        } else if (kind === 'note') {
          await invoke('update_note_status', { id, status: willBeDone ? 'done' : 'task' });
        }
      } catch (err) { console.error('period toggle:', err); }
      window.dispatchEvent(new Event('task-state-changed'));
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
