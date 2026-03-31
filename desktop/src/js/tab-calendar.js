// ── js/tab-calendar.js — Calendar tab: unified layout with sub-views ──

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml, renderPageHeader, setupPageHeaderControls, skeletonPage, loadTabBlockEditor } from './utils.js';

// Helper: check if a schedule should appear on a given date
function scheduleMatchesDate(sch, dateStr) {
  if (!sch.is_active) return false;
  if (sch.frequency === 'daily') return true;
  const d = new Date(dateStr + 'T12:00:00');
  const dow = d.getDay() || 7; // 1=Mon..7=Sun
  if (sch.frequency === 'weekly') {
    const days = sch.frequency_days ? sch.frequency_days.split(',').map(Number) : [1];
    return days.includes(dow);
  }
  if (sch.frequency === 'custom' && sch.frequency_days) {
    return sch.frequency_days.split(',').map(Number).includes(dow);
  }
  return false;
}

const SCH_CAT_ICONS = { health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡', practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽' };

// ── Calendar (unified layout) ──
async function loadCalendar(subTab) {
  const el = document.getElementById('calendar-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'calendar', {
    title: 'Calendar',
    subtitle: 'Расписание и события',
    icon: '📅',
    renderDash: async (paneEl) => {
      const today = new Date();
      const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
      const events = await invoke('get_events', { month: today.getMonth() + 1, year: today.getFullYear() }).catch(() => []);
      const todayEvents = events.filter(e => e.date === todayStr);
      const tasks = await invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []);
      const todayTasks = tasks.filter(t => t.due_date === todayStr);
      const allSchedules = await invoke('get_schedules', { category: null }).catch(() => []);
      const todaySchedules = allSchedules.filter(s => scheduleMatchesDate(s, todayStr));
      const todayCompletions = await invoke('get_schedule_completions', { date: todayStr }).catch(() => []);
      const schDone = todaySchedules.filter(s => todayCompletions.some(c => c.schedule_id === s.id && c.completed)).length;
      const focusLog = await invoke('get_activity_log', { date: null }).catch(() => []);

      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${todayEvents.length}</div><div class="uni-dash-label">Событий</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${todayTasks.length}</div><div class="uni-dash-label">Задач</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${schDone}/${todaySchedules.length}</div><div class="uni-dash-label">Расписание</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${focusLog.length}</div><div class="uni-dash-label">Фокус</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const views = [
        { id: 'month', label: 'Месяц' },
        { id: 'week', label: 'Неделя' },
        { id: 'day', label: 'День' },
        { id: 'list', label: 'Список' },
        { id: 'table', label: 'Таблица' },
      ];
      const activeView = S._calendarInner || 'month';
      paneEl.innerHTML = `
        <div class="dev-filters" id="calendar-view-tabs">
          ${views.map(v => `<button class="dev-filter-btn${v.id === activeView ? ' active' : ''}" data-calview="${v.id}">${v.label}</button>`).join('')}
        </div>
        <div id="calendar-inner-content" class="calendar-full-width"></div>`;

      paneEl.querySelectorAll('[data-calview]').forEach(btn => {
        btn.addEventListener('click', () => {
          S._calendarInner = btn.dataset.calview;
          // Update active tab pill without re-rendering layout
          paneEl.querySelectorAll('[data-calview]').forEach(b => b.classList.toggle('active', b.dataset.calview === btn.dataset.calview));
          refreshCalendarInner();
        });
      });

      await autoSyncCalendar(activeView);
      await refreshCalendarInner();
    },
  });
}

// Lightweight refresh: only updates #calendar-inner-content, no layout re-render
async function refreshCalendarInner() {
  const innerEl = document.getElementById('calendar-inner-content');
  if (!innerEl) return;
  const activeView = S._calendarInner || 'month';
  if (activeView === 'integrations') {
    await renderCalendarIntegrations(innerEl);
  } else if (activeView === 'table') {
    await renderCalendarTable(innerEl);
  } else if (activeView === 'list') {
    await renderCalendarList(innerEl);
  } else {
    const events = await invoke('get_events', { month: S.calendarMonth + 1, year: S.calendarYear }).catch(() => []);
    const tasks = await invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []);
    if (activeView === 'week') await renderWeekCalendar(innerEl, events || []);
    else if (activeView === 'day') await renderDayCalendar(innerEl, events || []);
    else await renderCalendar(innerEl, events || [], tasks || []);
  }
}

async function renderCalendarTable(el) {
  const { DatabaseView } = await import('./db-view/db-view.js');
  const events = await invoke('get_all_events').catch(() => []);
  const calCatColors = { general: 'gray', work: 'yellow', personal: 'blue', health: 'green', education: 'purple', social: 'pink', travel: 'orange' };
  const catOptions = ['general', 'work', 'personal', 'health', 'education', 'social', 'travel'].map(v => ({ value: v, label: v, color: calCatColors[v] || 'gray' }));

  el.innerHTML = '';
  const dbvEl = document.createElement('div');
  el.appendChild(dbvEl);
  const reload = () => renderCalendarTable(el);

  const dbv = new DatabaseView(dbvEl, {
    tabId: 'calendar_events',
    recordTable: 'events',
    records: events,
    idField: 'id',
    fixedColumns: [
      { key: 'title', label: 'Название', editable: true, editType: 'text',
        render: r => `<span class="data-table-title">${escapeHtml(r.title || '')}</span>` },
      { key: 'date', label: 'Дата', editable: true, editType: 'date',
        render: r => `<span>${r.date || '—'}</span>` },
      { key: 'time', label: 'Время', editable: true, editType: 'text',
        render: r => `<span>${r.time || '—'}</span>` },
      { key: 'duration_minutes', label: 'Длительность', editable: true, editType: 'number',
        render: r => `<span>${r.duration_minutes ? r.duration_minutes + ' мин' : '—'}</span>` },
      { key: 'category', label: 'Категория', editable: true, editType: 'select', editOptions: catOptions,
        render: r => `<span class="badge badge-${calCatColors[r.category] || 'gray'}">${escapeHtml(r.category || '—')}</span>` },
      { key: 'description', label: 'Описание', editable: true, editType: 'text',
        render: r => `<span>${escapeHtml(r.description || '') || '—'}</span>` },
    ],
    onCellEdit: async (recordId, key, value) => {
      const params = { id: recordId, title: null, description: null, date: null, time: null, durationMinutes: null, category: null, color: null, completed: null };
      if (key === 'duration_minutes') params.durationMinutes = parseInt(value) || null;
      else params[key] = value || null;
      await invoke('update_event', params);
      reload();
    },
    onDelete: async (id) => { await invoke('delete_event', { id }); },
    reloadFn: reload,
    availableViews: ['table'],
  });
  await dbv.render();
}

async function autoSyncCalendar(viewName) {
  const monthKey = `${S.calendarYear}-${S.calendarMonth + 1}`;
  if (S.syncedMonths.has(monthKey)) return;
  S.syncedMonths.add(monthKey);
  const autoSync = await invoke('get_app_setting', { key: 'calendar_autosync' }).catch(() => 'false');
  if (autoSync !== 'true') return;
  const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
  const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');
  try {
    if (appleEnabled !== 'false') {
      const r = await invoke('sync_apple_calendar', { month: S.calendarMonth + 1, year: S.calendarYear });
      if (r.error) console.warn('Apple Calendar:', r.error);
    }
    if (googleUrl) await invoke('sync_google_ics', { url: googleUrl, month: S.calendarMonth + 1, year: S.calendarYear });
  } catch (e) { console.error('Auto-sync error:', e); }
}


// Category → color class mapping
const CAT_COLORS = { health: 'blue', sport: 'green', hygiene: 'pink', practice: 'purple', challenge: 'red', growth: 'orange', work: 'yellow', home: 'gray', other: 'blue' };

async function renderCalendar(el, events, tasks) {
  tasks = tasks || [];
  const monthNames = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
  const weekdays = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
  const MAX_PILLS = 4;

  const schedules = await invoke('get_schedules', { category: null }).catch(() => []);
  const selDate = S.selectedCalendarDate;
  const selCompletions = selDate ? await invoke('get_schedule_completions', { date: selDate }).catch(() => []) : [];
  const completedIds = new Set(selCompletions.filter(c => c.completed).map(c => c.schedule_id));
  // Challenges: load completions for previous day
  let challengeDoneIds = completedIds;
  if (selDate) {
    const prevD = new Date(selDate + 'T12:00:00'); prevD.setDate(prevD.getDate() - 1);
    const prevDate = `${prevD.getFullYear()}-${String(prevD.getMonth()+1).padStart(2,'0')}-${String(prevD.getDate()).padStart(2,'0')}`;
    const prevComps = await invoke('get_schedule_completions', { date: prevDate }).catch(() => []);
    challengeDoneIds = new Set(prevComps.filter(c => c.completed).map(c => c.schedule_id));
  }

  const firstDay = new Date(S.calendarYear, S.calendarMonth, 1);
  const lastDay = new Date(S.calendarYear, S.calendarMonth + 1, 0);
  let startDay = firstDay.getDay() - 1;
  if (startDay < 0) startDay = 6;

  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;

  const eventsByDate = {};
  for (const ev of events) { if (!eventsByDate[ev.date]) eventsByDate[ev.date] = []; eventsByDate[ev.date].push(ev); }
  const tasksByDate = {};
  for (const t of tasks) { if (!t.due_date) continue; if (!tasksByDate[t.due_date]) tasksByDate[t.due_date] = []; tasksByDate[t.due_date].push(t); }

  // Build pills for a date
  function dayPills(dateStr) {
    const items = [];
    const dayScheds = schedules.filter(s => scheduleMatchesDate(s, dateStr));
    for (const s of dayScheds) items.push({ title: s.title, icon: SCH_CAT_ICONS[s.category] || '📌', color: CAT_COLORS[s.category] || 'blue' });
    for (const e of (eventsByDate[dateStr] || [])) items.push({ title: e.title, icon: '', color: 'orange' });
    for (const t of (tasksByDate[dateStr] || [])) items.push({ title: t.title || 'Задача', icon: '', color: 'green' });
    const visible = items.slice(0, MAX_PILLS);
    const extra = items.length - MAX_PILLS;
    let html = visible.map(it =>
      `<div class="cal-pill cal-pill-${it.color}">${it.icon ? it.icon + ' ' : ''}${escapeHtml(it.title)}</div>`
    ).join('');
    if (extra > 0) html += `<div class="cal-pill-more">+${extra} ещё</div>`;
    return html;
  }

  let daysHtml = '';
  const prevLast = new Date(S.calendarYear, S.calendarMonth, 0).getDate();
  for (let i = startDay - 1; i >= 0; i--) {
    daysHtml += `<div class="cal-day other"><span class="cal-day-num">${prevLast - i}</span></div>`;
  }
  for (let d = 1; d <= lastDay.getDate(); d++) {
    const dateStr = `${S.calendarYear}-${String(S.calendarMonth+1).padStart(2,'0')}-${String(d).padStart(2,'0')}`;
    const cls = [dateStr === todayStr && 'today', dateStr === S.selectedCalendarDate && 'selected'].filter(Boolean).join(' ');
    daysHtml += `<div class="cal-day${cls ? ' ' + cls : ''}" data-date="${dateStr}">
      <span class="cal-day-num">${d}</span>
      <div class="cal-pills">${dayPills(dateStr)}</div>
    </div>`;
  }
  const totalCells = startDay + lastDay.getDate();
  const remaining = totalCells % 7 === 0 ? 0 : 7 - (totalCells % 7);
  for (let i = 1; i <= remaining; i++) {
    daysHtml += `<div class="cal-day other"><span class="cal-day-num">${i}</span></div>`;
  }

  // Day panel
  let dayPanelHtml = '';
  if (selDate) {
    const dayEvts = eventsByDate[selDate] || [];
    const dayTasks = tasksByDate[selDate] || [];
    const dayScheds = schedules.filter(s => scheduleMatchesDate(s, selDate));

    const panelDate = new Date(selDate + 'T12:00:00');
    const dayNames = ['Вс','Пн','Вт','Ср','Чт','Пт','Сб'];
    const dateLabel = `${panelDate.getDate()} ${monthNames[panelDate.getMonth()].toLowerCase().slice(0,-1)}я · ${dayNames[panelDate.getDay()]}`;

    const schHtml = dayScheds.map(s => {
      const done = s.marks_previous_day ? challengeDoneIds.has(s.id) : completedIds.has(s.id);
      const icon = SCH_CAT_ICONS[s.category] || '◽';
      return `<div class="cal-panel-item" data-sch-toggle="${s.id}" data-sch-cat="${s.category || ''}" data-sch-prev="${s.marks_previous_day ? '1' : ''}">
        <div class="cal-panel-check${done ? ' done' : ''}">${done ? '✓' : ''}</div>
        <span class="cal-panel-time">${s.time_of_day || ''}</span>
        <span class="cal-panel-icon">${icon}</span>
        <span class="cal-panel-title${done ? ' done' : ''}">${escapeHtml(s.title)}</span>
      </div>`;
    }).join('');

    const evtHtml = dayEvts.map(e => `<div class="cal-panel-item">
      <span class="cal-panel-time">${e.time || ''}</span>
      <span class="cal-panel-title">${escapeHtml(e.title)}</span>
      ${e.source && e.source !== 'manual' ? `<span class="cal-panel-badge">${e.source === 'apple' ? '🍎' : '📅'}</span>` : ''}
    </div>`).join('');

    const taskHtml = dayTasks.map(t => {
      const done = t.status === 'done';
      return `<div class="cal-panel-item" data-id="${t.id}">
        <div class="cal-panel-check${done ? ' done' : ''}" data-action="toggle-task">${done ? '✓' : ''}</div>
        <span class="cal-panel-title${done ? ' done' : ''}" data-action="open-task">${escapeHtml(t.title || 'Без названия')}</span>
      </div>`;
    }).join('');

    const hasAny = dayScheds.length + dayEvts.length + dayTasks.length > 0;
    dayPanelHtml = `<div class="cal-panel">
      <div class="cal-panel-header">
        <span class="cal-panel-date">${dateLabel}</span>
        <button class="btn-sm btn-secondary" id="cal-add-task-btn">+ Задача</button>
      </div>
      ${dayScheds.length ? '<div class="cal-panel-section">Расписание</div>' + schHtml : ''}
      ${dayEvts.length ? '<div class="cal-panel-section">События</div>' + evtHtml : ''}
      ${dayTasks.length ? '<div class="cal-panel-section">Задачи</div>' + taskHtml : ''}
      ${!hasAny ? '<div class="cal-panel-empty">Нет событий на этот день</div>' : ''}
    </div>`;
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="cal-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[S.calendarMonth]} ${S.calendarYear}</div>
      <button class="calendar-nav-btn" id="cal-next">&gt;</button>
      <button class="cal-today-btn" id="cal-go-today">Сегодня</button>
      <button class="btn-primary" id="cal-add-event">+ Событие</button>
    </div>
    <div class="cal-weekdays">${weekdays.map(d => `<div class="cal-weekday">${d}</div>`).join('')}</div>
    <div class="cal-grid">${daysHtml}</div>
    ${dayPanelHtml}`;

  // Nav
  document.getElementById('cal-prev')?.addEventListener('click', () => {
    S.calendarMonth--;
    if (S.calendarMonth < 0) { S.calendarMonth = 11; S.calendarYear--; }
    S.selectedCalendarDate = null;
    refreshCalendarInner();
  });
  document.getElementById('cal-next')?.addEventListener('click', () => {
    S.calendarMonth++;
    if (S.calendarMonth > 11) { S.calendarMonth = 0; S.calendarYear++; }
    S.selectedCalendarDate = null;
    refreshCalendarInner();
  });
  document.getElementById('cal-go-today')?.addEventListener('click', () => {
    S.calendarMonth = today.getMonth(); S.calendarYear = today.getFullYear();
    S.selectedCalendarDate = todayStr;
    refreshCalendarInner();
  });
  document.getElementById('cal-add-event')?.addEventListener('click', () => showAddEventModal());

  // Day click
  el.querySelectorAll('.cal-day:not(.other)').forEach(day => {
    day.addEventListener('click', () => {
      S.selectedCalendarDate = day.dataset.date;
      refreshCalendarInner();
    });
  });

  // Schedule completion toggles
  el.querySelectorAll('[data-sch-toggle]').forEach(item => {
    item.addEventListener('click', async () => {
      const schId = parseInt(item.dataset.schToggle);
      let date = selDate;
      if (item.dataset.schPrev === '1') {
        const prev = new Date(selDate + 'T12:00:00'); prev.setDate(prev.getDate() - 1);
        date = `${prev.getFullYear()}-${String(prev.getMonth()+1).padStart(2,'0')}-${String(prev.getDate()).padStart(2,'0')}`;
      }
      await invoke('toggle_schedule_completion', { scheduleId: schId, date }).catch(e => console.error('toggle schedule:', e));
      refreshCalendarInner();
    });
  });

  // Task interactions
  el.querySelectorAll('.cal-panel-item[data-id]').forEach(item => {
    item.querySelector('[data-action="toggle-task"]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = parseInt(item.dataset.id);
      const task = tasks.find(t => t.id === id);
      if (!task) return;
      await invoke('update_note_status', { id, status: task.status === 'done' ? 'task' : 'done' }).catch(err => console.error(err));
      refreshCalendarInner();
    });
    item.querySelector('[data-action="open-task"]')?.addEventListener('click', () => {
      tabLoaders.switchTab?.('notes');
      setTimeout(() => { S.currentNoteId = parseInt(item.dataset.id); S.notesViewMode = 'edit'; const n = document.getElementById('notes-content'); if (n) tabLoaders.renderNoteEditor?.(n, S.currentNoteId); }, 100);
    });
  });

  document.getElementById('cal-add-task-btn')?.addEventListener('click', async () => {
    try {
      const id = await invoke('create_note', { title: '', content: '', tags: '', status: 'task', tabName: null, dueDate: selDate, reminderAt: null });
      tabLoaders.switchTab?.('notes');
      setTimeout(() => { S.currentNoteId = id; S.notesViewMode = 'edit'; const n = document.getElementById('notes-content'); if (n) tabLoaders.renderNoteEditor?.(n, id); }, 100);
    } catch (err) { console.error('cal add task:', err); }
  });
}

async function renderWeekCalendar(el, events) {
  const weekdays = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
  const monthsShort = ['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'];
  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
  const currentHour = today.getHours();
  const currentMin = today.getMinutes();
  const dayOfWeek = today.getDay() || 7;
  const weekStart = new Date(today);
  weekStart.setDate(today.getDate() - dayOfWeek + 1 + (S.calWeekOffset || 0) * 7);
  const isCurrentWeek = (S.calWeekOffset || 0) === 0;

  const eventsByDate = {};
  for (const ev of events) { if (!eventsByDate[ev.date]) eventsByDate[ev.date] = []; eventsByDate[ev.date].push(ev); }
  const schedules = await invoke('get_schedules', { category: null }).catch(() => []);

  // Build day dates & header
  const dayDates = [];
  let headerHtml = '<div class="wk-corner"></div>';
  for (let i = 0; i < 7; i++) {
    const d = new Date(weekStart);
    d.setDate(weekStart.getDate() + i);
    const dateStr = `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
    dayDates.push(dateStr);
    const isToday = dateStr === todayStr;
    headerHtml += `<div class="wk-hdr${isToday ? ' wk-today' : ''}">
      <span class="wk-hdr-wd">${weekdays[i]}</span>
      <span class="wk-hdr-d${isToday ? ' wk-hdr-d-today' : ''}">${d.getDate()}</span>
    </div>`;
  }

  // All-day items (no time)
  let allDayHtml = '<div class="wk-allday-label">Весь день</div>';
  let hasAllDay = false;
  for (let i = 0; i < 7; i++) {
    const dateStr = dayDates[i];
    const isToday = dateStr === todayStr;
    const noTimeScheds = schedules.filter(s => scheduleMatchesDate(s, dateStr) && !s.time_of_day);
    const noTimeEvts = (eventsByDate[dateStr] || []).filter(e => !e.time);
    let cellHtml = '';
    for (const s of noTimeScheds) {
      const icon = SCH_CAT_ICONS[s.category] || '📌';
      cellHtml += `<div class="wk-ev wk-ev-${CAT_COLORS[s.category] || 'blue'}">${icon} ${escapeHtml(s.title)}</div>`;
    }
    for (const e of noTimeEvts) {
      cellHtml += `<div class="wk-ev wk-ev-orange">${escapeHtml(e.title)}</div>`;
    }
    if (cellHtml) hasAllDay = true;
    allDayHtml += `<div class="wk-allday-cell${isToday ? ' wk-col-today' : ''}">${cellHtml}</div>`;
  }

  // Hour grid (0:00 - 23:00)
  const hours = Array.from({length: 24}, (_, i) => i);
  let gridHtml = '';
  for (const h of hours) {
    gridHtml += `<div class="wk-time">${String(h).padStart(2,'0')}:00</div>`;
    for (let i = 0; i < 7; i++) {
      const dateStr = dayDates[i];
      const isToday = dateStr === todayStr;
      // Events at this hour
      const hourEvts = (eventsByDate[dateStr] || []).filter(e => e.time && parseInt(e.time.split(':')[0]) === h);
      // Schedules at this hour
      const hourScheds = schedules.filter(s => scheduleMatchesDate(s, dateStr) && s.time_of_day && parseInt(s.time_of_day.split(':')[0]) === h);

      let cellHtml = '';
      for (const s of hourScheds) {
        const icon = SCH_CAT_ICONS[s.category] || '📌';
        const min = s.time_of_day.split(':')[1] || '00';
        cellHtml += `<div class="wk-ev wk-ev-${CAT_COLORS[s.category] || 'blue'}"><span class="wk-ev-min">:${min}</span> ${icon} ${escapeHtml(s.title)}</div>`;
      }
      for (const e of hourEvts) {
        const min = e.time.split(':')[1] || '00';
        cellHtml += `<div class="wk-ev wk-ev-orange"><span class="wk-ev-min">:${min}</span> ${escapeHtml(e.title)}</div>`;
      }

      // Current time indicator
      let nowLine = '';
      if (isToday && isCurrentWeek && h === currentHour) {
        const pct = (currentMin / 60) * 100;
        nowLine = `<div class="wk-now-line" style="top:${pct}%"><div class="wk-now-dot"></div></div>`;
      }

      gridHtml += `<div class="wk-cell${isToday ? ' wk-col-today' : ''}" data-date="${dateStr}" data-hour="${h}">${nowLine}${cellHtml}</div>`;
    }
  }

  const startLabel = `${weekStart.getDate()} ${monthsShort[weekStart.getMonth()]}`;
  const endDate = new Date(weekStart);
  endDate.setDate(weekStart.getDate() + 6);
  const endLabel = `${endDate.getDate()} ${monthsShort[endDate.getMonth()]}`;

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="week-prev">&lt;</button>
      <div class="calendar-month-label">${startLabel} — ${endLabel} ${weekStart.getFullYear()}</div>
      <button class="calendar-nav-btn" id="week-next">&gt;</button>
      <button class="cal-today-btn" id="week-today">Сегодня</button>
      <button class="btn-primary" id="week-add-event">+ Событие</button>
    </div>
    <div class="wk-header">${headerHtml}</div>
    ${hasAllDay ? `<div class="wk-allday">${allDayHtml}</div>` : ''}
    <div class="wk-scroll">
      <div class="wk-grid">${gridHtml}</div>
    </div>`;

  // Auto-scroll to current hour
  if (isCurrentWeek) {
    const scrollEl = el.querySelector('.wk-scroll');
    if (scrollEl) {
      const rowH = 48;
      scrollEl.scrollTop = Math.max(0, (currentHour - 6) * rowH - 60);
    }
  }

  document.getElementById('week-prev')?.addEventListener('click', () => { S.calWeekOffset = (S.calWeekOffset || 0) - 1; refreshCalendarInner(); });
  document.getElementById('week-next')?.addEventListener('click', () => { S.calWeekOffset = (S.calWeekOffset || 0) + 1; refreshCalendarInner(); });
  document.getElementById('week-today')?.addEventListener('click', () => { S.calWeekOffset = 0; refreshCalendarInner(); });
  document.getElementById('week-add-event')?.addEventListener('click', () => showAddEventModal());

  // Click cell → open day view
  el.querySelectorAll('.wk-cell').forEach(cell => {
    cell.addEventListener('click', () => {
      S.calDayDate = cell.dataset.date;
      S._calendarInner = 'day';
      const dd = new Date(cell.dataset.date);
      S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear();
      document.querySelectorAll('[data-calview]').forEach(b => b.classList.toggle('active', b.dataset.calview === 'day'));
      refreshCalendarInner();
    });
  });
}

function showAddEventModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новое событие</div>
    <div class="form-row"><input class="form-input" id="event-title" placeholder="Название"></div>
    <div class="form-row">
      <input class="form-input" id="event-date" type="date" value="${S.selectedCalendarDate || new Date().toISOString().split('T')[0]}">
      <input class="form-input" id="event-time" type="time" style="max-width:120px;">
    </div>
    <textarea class="form-textarea" id="event-desc" placeholder="Описание (необязательно)" rows="2"></textarea>
    <div class="modal-actions">
      <button class="btn-secondary" id="event-cancel">Отмена</button>
      <button class="btn-primary" id="event-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('event-cancel')?.addEventListener('click', () => overlay.remove());
  document.getElementById('event-save')?.addEventListener('click', async () => {
    const title = document.getElementById('event-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_event', {
        title,
        description: document.getElementById('event-desc')?.value || '',
        date: document.getElementById('event-date')?.value || '',
        time: document.getElementById('event-time')?.value || '',
        durationMinutes: 60,
        category: 'general',
        color: '#9B9B9B',
      });
      overlay.remove();
      refreshCalendarInner();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Day View ──
async function renderDayCalendar(el, events) {
  const today = new Date();
  if (!S.calDayDate) S.calDayDate = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
  const dayEvents = events.filter(e => e.date === S.calDayDate).map(e => {
    if (e.time && /^\d:\d{2}$/.test(e.time)) e.time = '0' + e.time;
    return e;
  }).sort((a, b) => (a.time || '').localeCompare(b.time || ''));
  const d = new Date(S.calDayDate + 'T00:00:00');
  const dayNames = ['Воскресенье', 'Понедельник', 'Вторник', 'Среда', 'Четверг', 'Пятница', 'Суббота'];
  const monthNames = ['Января', 'Февраля', 'Марта', 'Апреля', 'Мая', 'Июня', 'Июля', 'Августа', 'Сентября', 'Октября', 'Ноября', 'Декабря'];

  // Load schedules for this day
  const schedules = await invoke('get_schedules', { category: null }).catch(() => []);
  const dayScheds = schedules.filter(s => scheduleMatchesDate(s, S.calDayDate));
  const completions = await invoke('get_schedule_completions', { date: S.calDayDate }).catch(() => []);
  const completedIds = new Set(completions.filter(c => c.completed).map(c => c.schedule_id));
  // Challenges mark completion for previous day
  const prevD = new Date(S.calDayDate + 'T12:00:00'); prevD.setDate(prevD.getDate() - 1);
  const prevDate = `${prevD.getFullYear()}-${String(prevD.getMonth()+1).padStart(2,'0')}-${String(prevD.getDate()).padStart(2,'0')}`;
  const prevComps = await invoke('get_schedule_completions', { date: prevDate }).catch(() => []);
  const challengeDoneIds = new Set(prevComps.filter(c => c.completed).map(c => c.schedule_id));

  const hours = Array.from({length: 24}, (_, i) => i); // 0:00 - 23:00
  let timelineHtml = hours.map(h => {
    const timeStr = `${String(h).padStart(2,'0')}:`;
    const hourEvents = dayEvents.filter(e => e.time && e.time.startsWith(timeStr.slice(0,2)));
    // Schedules with matching time
    const hourScheds = dayScheds.filter(s => s.time_of_day && s.time_of_day.startsWith(timeStr.slice(0,2)));
    const evtHtml = hourEvents.map(e => {
      const srcBadge = e.source && e.source !== 'manual' ? `<span class="badge badge-gray" style="margin-left:6px;">${e.source === 'apple' ? '🍎' : '📅'}</span>` : '';
      const endMin = (() => { const [hh,mm] = (e.time||'00:00').split(':').map(Number); const t = hh*60+mm+(e.duration_minutes||60); return `${String(Math.floor(t/60)%24).padStart(2,'0')}:${String(t%60).padStart(2,'0')}`; })();
      return `<div class="day-event" style="border-left:3px solid ${e.color || 'var(--text-secondary)'};">
        <span class="day-event-time">${e.time} – ${endMin}</span>
        <span class="day-event-title">${escapeHtml(e.title)}</span>${srcBadge}
        <span class="day-event-dur">${e.duration_minutes || 60} мин</span>
      </div>`;
    }).join('');
    const schHtml = hourScheds.map(s => {
      const done = s.marks_previous_day ? challengeDoneIds.has(s.id) : completedIds.has(s.id);
      const icon = SCH_CAT_ICONS[s.category] || '◽';
      return `<div class="day-event" data-day-sch="${s.id}" data-sch-cat="${s.category || ''}" data-sch-prev="${s.marks_previous_day ? '1' : ''}" style="border-left:3px solid var(--color-purple);cursor:pointer;${done ? 'opacity:0.5;' : ''}">
        <span class="day-event-time">${s.time_of_day} ${icon}</span>
        <span class="day-event-title" style="${done ? 'text-decoration:line-through;' : ''}">${escapeHtml(s.title)}</span>
        <span style="font-size:13px;">${done ? '✅' : '⬜'}</span>
      </div>`;
    }).join('');
    return `<div class="day-hour-row">
      <div class="day-hour-label">${String(h).padStart(2,'0')}:00</div>
      <div class="day-hour-content" data-date="${S.calDayDate}" data-hour="${h}">${evtHtml}${schHtml}</div>
    </div>`;
  }).join('');

  // Schedules without time → show in "all day" section
  const noTimeScheds = dayScheds.filter(s => !s.time_of_day);

  // All-day events (no time)
  const allDay = dayEvents.filter(e => !e.time);
  const noTimeSchHtml = noTimeScheds.map(s => {
    const done = s.marks_previous_day ? challengeDoneIds.has(s.id) : completedIds.has(s.id);
    const icon = SCH_CAT_ICONS[s.category] || '◽';
    return `<div class="day-event" data-day-sch="${s.id}" data-sch-cat="${s.category || ''}" data-sch-prev="${s.marks_previous_day ? '1' : ''}" style="border-left:3px solid var(--color-purple);cursor:pointer;${done ? 'opacity:0.5;' : ''}">
      <span class="day-event-time">${icon}</span>
      <span class="day-event-title" style="${done ? 'text-decoration:line-through;' : ''}">${escapeHtml(s.title)}</span>
      <span style="font-size:13px;">${done ? '✅' : '⬜'}</span>
    </div>`;
  }).join('');
  const allDayEvts = allDay.map(e => `<div class="day-event" style="border-left:3px solid ${e.color || 'var(--text-secondary)'};"><span class="day-event-title">${escapeHtml(e.title)}</span></div>`).join('');
  const allDayHtml = (allDay.length || noTimeScheds.length) ? `<div class="day-allday">
    <div class="day-hour-label">Весь день</div>
    <div class="day-hour-content">${allDayEvts}${noTimeSchHtml}</div>
  </div>` : '';

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="day-prev">&lt;</button>
      <div class="calendar-month-label">${d.getDate()} ${monthNames[d.getMonth()]} · ${dayNames[d.getDay()]}</div>
      <button class="calendar-nav-btn" id="day-next">&gt;</button>
      <button class="btn-secondary" id="day-today" style="margin-left:8px;">Сегодня</button>
      <button class="btn-primary" id="day-add-event" style="margin-left:8px;">+ Событие</button>
    </div>
    ${allDayHtml}
    <div class="day-timeline">${timelineHtml}</div>`;

  document.getElementById('day-prev')?.addEventListener('click', () => {
    const dd = new Date(S.calDayDate + 'T12:00:00'); dd.setDate(dd.getDate() - 1);
    S.calDayDate = `${dd.getFullYear()}-${String(dd.getMonth()+1).padStart(2,'0')}-${String(dd.getDate()).padStart(2,'0')}`;
    S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear();
    refreshCalendarInner();
  });
  document.getElementById('day-next')?.addEventListener('click', () => {
    const dd = new Date(S.calDayDate + 'T12:00:00'); dd.setDate(dd.getDate() + 1);
    S.calDayDate = `${dd.getFullYear()}-${String(dd.getMonth()+1).padStart(2,'0')}-${String(dd.getDate()).padStart(2,'0')}`;
    S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear();
    refreshCalendarInner();
  });
  document.getElementById('day-today')?.addEventListener('click', () => {
    S.calDayDate = null; S.calendarMonth = today.getMonth(); S.calendarYear = today.getFullYear();
    refreshCalendarInner();
  });
  document.getElementById('day-add-event')?.addEventListener('click', () => {
    S.selectedCalendarDate = S.calDayDate;
    showAddEventModal();
  });
  el.querySelectorAll('.day-hour-content').forEach(cell => {
    cell.addEventListener('click', (e) => {
      if (e.target.closest('.day-event')) return;
      S.selectedCalendarDate = cell.dataset.date;
      showAddEventModal();
      setTimeout(() => {
        const ti = document.getElementById('event-time');
        if (ti) ti.value = `${String(cell.dataset.hour).padStart(2,'0')}:00`;
      }, 50);
    });
  });
  // Schedule completion toggles in day view
  el.querySelectorAll('[data-day-sch]').forEach(item => {
    item.addEventListener('click', async (e) => {
      e.stopPropagation();
      const schId = parseInt(item.dataset.daySch);
      let date = S.calDayDate;
      if (item.dataset.schPrev === '1') {
        const prev = new Date(S.calDayDate + 'T12:00:00'); prev.setDate(prev.getDate() - 1);
        date = `${prev.getFullYear()}-${String(prev.getMonth()+1).padStart(2,'0')}-${String(prev.getDate()).padStart(2,'0')}`;
      }
      await invoke('toggle_schedule_completion', { scheduleId: schId, date }).catch(err => console.error('day sch:', err));
      refreshCalendarInner();
    });
  });
}

// ── Calendar List view (Notion-style table) ──
async function renderCalendarList(el) {
  const monthNames = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
  const events = await invoke('get_events', { month: S.calendarMonth + 1, year: S.calendarYear }).catch(() => []) || [];

  const sourceLabel = (s) => s === 'apple' ? '🍎 Apple' : s === 'google' ? '📅 Google' : '✏️ Вручную';
  const sourceColor = (s) => s === 'apple' ? '#4F9768' : s === 'google' ? '#447ACB' : 'var(--text-secondary)';

  let rowsHtml = '';
  if (events.length === 0) {
    rowsHtml = '<tr><td colspan="6" style="text-align:center;color:var(--text-muted);padding:24px;">Нет событий</td></tr>';
  } else {
    for (const ev of events) {
      const endTime = ev.time && ev.duration_minutes ? (() => {
        const [h, m] = ev.time.split(':').map(Number);
        const total = h * 60 + m + ev.duration_minutes;
        return `${String(Math.floor(total / 60) % 24).padStart(2,'0')}:${String(total % 60).padStart(2,'0')}`;
      })() : '';
      const timeRange = ev.time ? (endTime ? `${ev.time} – ${endTime}` : ev.time) : 'Весь день';
      rowsHtml += `<tr class="cal-list-row" data-id="${ev.id}">
        <td style="color:var(--text-primary);font-weight:500;">${escapeHtml(ev.title)}</td>
        <td>${ev.date}</td>
        <td>${timeRange}</td>
        <td>${ev.duration_minutes ? ev.duration_minutes + ' мин' : '—'}</td>
        <td><span style="color:${sourceColor(ev.source)};font-size:12px;">${sourceLabel(ev.source)}</span></td>
        <td style="color:var(--text-muted);font-size:12px;">${escapeHtml(ev.category || '')}</td>
      </tr>`;
    }
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="list-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[S.calendarMonth]} ${S.calendarYear}</div>
      <button class="calendar-nav-btn" id="list-next">&gt;</button>
      <button class="btn-primary" id="list-add-event" style="margin-left:16px;">+ Событие</button>
      <span style="color:var(--text-muted);font-size:12px;margin-left:auto;">${events.length} событий</span>
    </div>
    <div style="overflow-x:auto;">
      <table class="cal-list-table">
        <thead>
          <tr>
            <th>Название</th>
            <th>Дата</th>
            <th>Время</th>
            <th>Длит.</th>
            <th>Источник</th>
            <th>Категория</th>
          </tr>
        </thead>
        <tbody>${rowsHtml}</tbody>
      </table>
    </div>`;

  document.getElementById('list-prev')?.addEventListener('click', () => {
    S.calendarMonth--;
    if (S.calendarMonth < 0) { S.calendarMonth = 11; S.calendarYear--; }
    refreshCalendarInner();
  });
  document.getElementById('list-next')?.addEventListener('click', () => {
    S.calendarMonth++;
    if (S.calendarMonth > 11) { S.calendarMonth = 0; S.calendarYear++; }
    refreshCalendarInner();
  });
  document.getElementById('list-add-event')?.addEventListener('click', () => showAddEventModal());
  el.querySelectorAll('.cal-list-row').forEach(row => {
    row.addEventListener('click', () => {
      const ev = events.find(e => e.id === Number(row.dataset.id));
      if (ev) { S.selectedCalendarDate = ev.date; S.calDayDate = ev.date; const dd = new Date(ev.date); S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear(); S._calendarInner = 'day'; refreshCalendarInner(); }
    });
  });
}

// ── Calendar Integrations sub-tab ──
async function renderCalendarIntegrations(el) {
  el.innerHTML = skeletonPage();
  try {
    const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
    const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');

    el.innerHTML = `
      <div class="settings-section">
        <div class="settings-section-title">Apple Calendar</div>
        <div class="settings-row">
          <span class="settings-label">Синхронизация с Calendar.app</span>
          <label class="toggle"><input type="checkbox" id="calint-apple" ${appleEnabled !== 'false' ? 'checked' : ''}><span class="toggle-track"></span></label>
        </div>
        <div class="settings-row">
          <span class="settings-label" style="color:var(--text-faint);font-size:12px;">Включает все календари добавленные в macOS (iCloud, Google, Exchange и др.)</span>
        </div>
        <div class="settings-row">
          <button class="btn-primary" id="calint-sync-apple">Синхронизировать сейчас</button>
          <span class="settings-value" id="calint-apple-status">—</span>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Google Calendar (ICS)</div>
        <div class="settings-row">
          <span class="settings-label" style="color:var(--text-faint);font-size:12px;">Приватный ICS URL: Google Calendar → Настройки → Настройки календаря → Секретный адрес в формате iCal</span>
        </div>
        <div style="display:flex;gap:8px;padding:8px 0;">
          <input class="form-input" id="calint-google-url" placeholder="https://calendar.google.com/...basic.ics" value="${escapeHtml(googleUrl)}" style="flex:1">
        </div>
        <div class="settings-row">
          <button class="btn-primary" id="calint-save-google">Сохранить и синхронизировать</button>
          <span class="settings-value" id="calint-google-status">—</span>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Автосинхронизация</div>
        <div class="settings-row">
          <span class="settings-label">Синхронизировать при открытии календаря</span>
          <label class="toggle"><input type="checkbox" id="calint-autosync" ${(await invoke('get_app_setting', { key: 'calendar_autosync' }).catch(() => 'false')) === 'true' ? 'checked' : ''}><span class="toggle-track"></span></label>
        </div>
      </div>`;

    document.getElementById('calint-apple')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'apple_calendar_enabled', value: e.target.checked ? 'true' : 'false' });
    });
    document.getElementById('calint-autosync')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'calendar_autosync', value: e.target.checked ? 'true' : 'false' });
    });
    document.getElementById('calint-sync-apple')?.addEventListener('click', async () => {
      const btn = document.getElementById('calint-sync-apple');
      const status = document.getElementById('calint-apple-status');
      if (btn) { btn.textContent = 'Синхронизация...'; btn.disabled = true; }
      try {
        const r = await invoke('sync_apple_calendar', { month: new Date().getMonth() + 1, year: new Date().getFullYear() });
        if (r.error) {
          if (status) { status.textContent = '✗ ' + r.error; status.style.color = 'var(--color-red)'; }
        } else {
          if (status) { status.textContent = `✓ ${r.synced} событий`; status.style.color = ''; }
        }
      } catch (e) { if (status) { status.textContent = '✗ ' + e; status.style.color = 'var(--color-red)'; } }
      setTimeout(() => { if (btn) { btn.textContent = 'Синхронизировать сейчас'; btn.disabled = false; } }, 2000);
    });
    document.getElementById('calint-save-google')?.addEventListener('click', async () => {
      const url = document.getElementById('calint-google-url')?.value.trim() || '';
      const btn = document.getElementById('calint-save-google');
      const status = document.getElementById('calint-google-status');
      await invoke('set_app_setting', { key: 'google_calendar_ics_url', value: url });
      if (!url) { if (status) status.textContent = 'URL удалён'; return; }
      if (btn) { btn.textContent = 'Синхронизация...'; btn.disabled = true; }
      try {
        const r = await invoke('sync_google_ics', { url, month: new Date().getMonth() + 1, year: new Date().getFullYear() });
        if (status) status.textContent = `✓ ${r.synced} событий`;
      } catch (e) { if (status) status.textContent = '✗ ' + e; }
      setTimeout(() => { if (btn) { btn.textContent = 'Сохранить и синхронизировать'; btn.disabled = false; } }, 2000);
    });
  } catch (e) {
    el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`;
  }
}

export { loadCalendar };
