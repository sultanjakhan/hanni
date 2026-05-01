// Calendar Day "Список" — flat task list with checkboxes and timer start/stop.

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml } from './utils.js';

const SCH_CAT_ICONS = { health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡', practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽' };

function todayStr() {
  const t = new Date();
  return `${t.getFullYear()}-${String(t.getMonth()+1).padStart(2,'0')}-${String(t.getDate()).padStart(2,'0')}`;
}
function shiftDate(dateStr, days) {
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

async function loadDayItems(date) {
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
    items.push({ kind: 'event', id: e.id, title: e.title || 'Без названия', sortKey: e.time || '99:99', icon: '📅', done: !!e.completed, block: blockBySrc.get(`event:${e.id}`) });
  }
  for (const s of (scheds || []).filter(s => scheduleMatchesDate(s, date))) {
    items.push({ kind: 'schedule', id: s.id, title: s.title || 'Без названия', sortKey: s.time_of_day || '99:99', icon: SCH_CAT_ICONS[s.category] || '🔁', done: completedIds.has(s.id), block: blockBySrc.get(`schedule:${s.id}`) });
  }
  for (const t of (tasks || []).filter(t => t.due_date === date)) {
    items.push({ kind: 'note', id: t.id, title: t.title || 'Без названия', sortKey: '99:99', icon: '📝', done: t.status === 'done', block: blockBySrc.get(`note:${t.id}`) });
  }
  items.sort((a, b) => a.sortKey.localeCompare(b.sortKey));
  return items;
}

function renderItemRow(item, dateStr) {
  const active = item.block?.is_active;
  const done = !!item.done;
  const cls = ['ctl-row', done && 'ctl-done', active && 'ctl-active'].filter(Boolean).join(' ');
  const durBadge = item.block?.duration_minutes ? `<span class="ctl-duration">${item.block.duration_minutes} мин</span>` : '';
  const trackBtn = active
    ? `<button class="ctl-track ctl-stop" data-ctl-stop="${item.block.id}" title="Завершить">■</button>`
    : `<button class="ctl-track ctl-start" data-ctl-start title="Запустить">▶</button>`;
  return `<div class="${cls}" data-kind="${item.kind}" data-id="${item.id}" data-date="${dateStr || ''}">
    <div class="ctl-check${done ? ' done' : ''}" data-ctl-check>${done ? '✓' : ''}</div>
    <span class="ctl-icon">${item.icon}</span>
    <span class="ctl-title">${escapeHtml(item.title)}</span>
    ${durBadge}
    ${trackBtn}
  </div>`;
}

function renderToolbar(dayLabel, isToday) {
  return `<div class="cal-list-toolbar">
    <div class="cal-list-nav">
      <button class="calendar-nav-btn" id="ctl-prev">&lt;</button>
      <div class="calendar-month-label">${escapeHtml(dayLabel)}</div>
      <button class="calendar-nav-btn" id="ctl-next">&gt;</button>
      ${!isToday ? `<button class="cal-today-btn" id="ctl-today">Сегодня</button>` : ''}
      <button class="btn-primary" id="ctl-add" style="margin-left:auto;">+ Событие</button>
    </div>
    <div class="day-mode-tabs dev-filters">
      <button class="dev-filter-btn" data-day-mode="grid">📅 Календарь</button>
      <button class="dev-filter-btn active" data-day-mode="list">📋 Список</button>
    </div>
  </div>`;
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

function setDayMode(mode) {
  S.calDayMode = mode;
  try { localStorage.setItem('hanni_calendar_day_mode', mode); } catch {}
}

function wire(el) {
  el.querySelectorAll('[data-day-mode]').forEach(btn => {
    btn.addEventListener('click', () => {
      if (btn.dataset.dayMode === 'list') return;
      setDayMode('grid');
      window.dispatchEvent(new Event('task-state-changed'));
    });
  });
  el.querySelector('#ctl-prev')?.addEventListener('click', () => { S.calDayDate = shiftDate(S.calDayDate || todayStr(), -1); renderCalendarTaskList(el); });
  el.querySelector('#ctl-next')?.addEventListener('click', () => { S.calDayDate = shiftDate(S.calDayDate || todayStr(), 1); renderCalendarTaskList(el); });
  el.querySelector('#ctl-today')?.addEventListener('click', () => { S.calDayDate = todayStr(); renderCalendarTaskList(el); });
  const openAdd = () => { S.selectedCalendarDate = S.calDayDate || todayStr(); tabLoaders.openCalendarAddEvent?.(); };
  el.querySelector('#ctl-add')?.addEventListener('click', openAdd);
  el.querySelector('#ctl-add-empty')?.addEventListener('click', openAdd);

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

export async function renderCalendarTaskList(el) {
  if (!S.calDayDate) S.calDayDate = todayStr();
  const date = S.calDayDate;
  const d = new Date(date + 'T12:00:00');
  const monthsGen = ['января','февраля','марта','апреля','мая','июня','июля','августа','сентября','октября','ноября','декабря'];
  const dayNames = ['Воскресенье','Понедельник','Вторник','Среда','Четверг','Пятница','Суббота'];
  const dayLabel = `${d.getDate()} ${monthsGen[d.getMonth()]} · ${dayNames[d.getDay()]}`;
  const isToday = date === todayStr();

  const items = await loadDayItems(date);
  const bodyHtml = items.length
    ? items.map(it => renderItemRow(it, date)).join('')
    : `<div class="ctl-empty">
        <div class="ctl-empty-title">${isToday ? 'Сегодня свободно' : 'На этот день ничего не запланировано'}</div>
        <button class="btn-primary" id="ctl-add-empty">+ Запланировать</button>
      </div>`;
  el.innerHTML = renderToolbar(dayLabel, isToday) + `<div class="ctl-body">${bodyHtml}</div>`;
  wire(el);
}
