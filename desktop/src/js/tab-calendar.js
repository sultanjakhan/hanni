// ── js/tab-calendar.js — Calendar tab: month/week/day/list views + integrations ──

import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml, renderPageHeader, setupPageHeaderControls, skeletonPage, loadTabBlockEditor } from './utils.js';

// ── Calendar ──
async function loadCalendar(subTab) {
  const el = document.getElementById('calendar-content');
  if (!el) return;

  // Ensure page header exists
  if (!el.querySelector('.page-header')) {
    el.innerHTML = renderPageHeader('calendar') + '<div id="calendar-view-content" class="page-content calendar-full-width"></div>';
    setupPageHeaderControls('calendar');
  }
  const viewEl = document.getElementById('calendar-view-content') || el;

  if (subTab === 'Интеграции') {
    await renderCalendarIntegrations(viewEl);
    const sect = document.createElement('div');
    sect.className = 'tab-block-section';
    sect.innerHTML = '<div class="tab-block-section-header">Заметки</div>';
    viewEl.appendChild(sect);
    loadTabBlockEditor('calendar', subTab, sect);
    return;
  }
  if (subTab === 'Список') {
    await renderCalendarList(viewEl);
    const sect = document.createElement('div');
    sect.className = 'tab-block-section';
    sect.innerHTML = '<div class="tab-block-section-header">Заметки</div>';
    viewEl.appendChild(sect);
    loadTabBlockEditor('calendar', subTab, sect);
    return;
  }

  // Auto-sync when navigating to a month not yet synced
  const monthKey = `${S.calendarYear}-${S.calendarMonth + 1}`;
  if (!S.syncedMonths.has(monthKey)) {
    S.syncedMonths.add(monthKey);
    const autoSync = await invoke('get_app_setting', { key: 'calendar_autosync' }).catch(() => 'false');
    if (autoSync === 'true') {
      const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
      const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');
      const syncAndRefresh = async () => {
        try {
          if (appleEnabled !== 'false') {
            const r = await invoke('sync_apple_calendar', { month: S.calendarMonth + 1, year: S.calendarYear });
            if (r.error) console.warn('Apple Calendar:', r.error);
          }
          if (googleUrl) await invoke('sync_google_ics', { url: googleUrl, month: S.calendarMonth + 1, year: S.calendarYear });
          // Refresh view after background sync completes
          const freshEvents = await invoke('get_events', { month: S.calendarMonth + 1, year: S.calendarYear }).catch(() => []);
          const calViewEl = document.getElementById('calendar-view-content');
          if (calViewEl && subTab === 'Список') renderCalendarList(calViewEl);
          else if (calViewEl && !subTab || subTab === 'Месяц') renderCalendar(calViewEl, freshEvents || []);
          else if (calViewEl && subTab === 'Неделя') renderWeekCalendar(calViewEl, freshEvents || []);
          else if (calViewEl && subTab === 'День') renderDayCalendar(calViewEl, freshEvents || []);
        } catch (e) { console.error('Auto-sync error:', e); }
      };
      syncAndRefresh(); // fire and forget — non-blocking
    }
  }

  try {
    const events = await invoke('get_events', { month: S.calendarMonth + 1, year: S.calendarYear }).catch(() => []);
    const tasks = await invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []);
    if (subTab === 'Неделя') {
      renderWeekCalendar(viewEl, events || []);
    } else if (subTab === 'День') {
      renderDayCalendar(viewEl, events || []);
    } else {
      renderCalendar(viewEl, events || [], tasks || []);
    }
  } catch (e) {
    if (subTab === 'Неделя') renderWeekCalendar(viewEl, []);
    else if (subTab === 'День') renderDayCalendar(viewEl, []);
    else renderCalendar(viewEl, []);
  }
  // Add block editor for calendar notes
  const sect = document.createElement('div');
  sect.className = 'tab-block-section';
  sect.innerHTML = '<div class="tab-block-section-header">Заметки</div>';
  viewEl.appendChild(sect);
  loadTabBlockEditor('calendar', subTab || 'Месяц', sect);
}

function renderCalendar(el, events, tasks) {
  tasks = tasks || [];
  const monthNames = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
  const weekdays = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];

  const firstDay = new Date(S.calendarYear, S.calendarMonth, 1);
  const lastDay = new Date(S.calendarYear, S.calendarMonth + 1, 0);
  let startDay = firstDay.getDay() - 1;
  if (startDay < 0) startDay = 6;

  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;

  // Group events by date
  const eventsByDate = {};
  for (const ev of events) {
    const d = ev.date;
    if (!eventsByDate[d]) eventsByDate[d] = [];
    eventsByDate[d].push(ev);
  }

  // Group tasks by due_date
  const tasksByDate = {};
  for (const t of tasks) {
    if (!t.due_date) continue;
    if (!tasksByDate[t.due_date]) tasksByDate[t.due_date] = [];
    tasksByDate[t.due_date].push(t);
  }

  let daysHtml = '';
  // Prev month days
  const prevLast = new Date(S.calendarYear, S.calendarMonth, 0).getDate();
  for (let i = startDay - 1; i >= 0; i--) {
    daysHtml += `<div class="calendar-day other-month"><span class="calendar-day-number">${prevLast - i}</span></div>`;
  }
  // Current month days
  for (let d = 1; d <= lastDay.getDate(); d++) {
    const dateStr = `${S.calendarYear}-${String(S.calendarMonth+1).padStart(2,'0')}-${String(d).padStart(2,'0')}`;
    const isToday = dateStr === todayStr;
    const isSelected = dateStr === S.selectedCalendarDate;
    const dayEvents = eventsByDate[dateStr] || [];
    const dayTasks = tasksByDate[dateStr] || [];
    const dots = dayEvents.slice(0, 3).map(e => `<span class="calendar-event-dot" style="background:${e.color || 'var(--accent-blue)'}"></span>`).join('');
    const taskDots = dayTasks.slice(0, 2).map(t => `<span class="calendar-task-dot${t.status === 'done' ? ' done' : ''}"></span>`).join('');
    daysHtml += `<div class="calendar-day${isToday ? ' today' : ''}${isSelected ? ' selected' : ''}" data-date="${dateStr}">
      <span class="calendar-day-number">${d}</span>
      <div class="calendar-day-dots">${dots}${taskDots}</div>
    </div>`;
  }
  // Next month days
  const totalCells = startDay + lastDay.getDate();
  const remaining = totalCells % 7 === 0 ? 0 : 7 - (totalCells % 7);
  for (let i = 1; i <= remaining; i++) {
    daysHtml += `<div class="calendar-day other-month"><span class="calendar-day-number">${i}</span></div>`;
  }

  let dayPanelHtml = '';
  if (S.selectedCalendarDate) {
    const dayEvts = eventsByDate[S.selectedCalendarDate] || [];
    const dayTasks = tasksByDate[S.selectedCalendarDate] || [];
    const hasContent = dayEvts.length > 0 || dayTasks.length > 0;

    const eventsSection = dayEvts.length > 0 ? `
      <div class="calendar-day-panel-section">События</div>
      ${dayEvts.map(e => `<div class="calendar-event-item">
        <span class="calendar-event-time">${e.time || ''}</span>
        <span class="calendar-event-title">${escapeHtml(e.title)}</span>
        ${e.source && e.source !== 'manual' ? `<span class="badge badge-gray">${e.source === 'apple' ? '🍎' : '📅'}</span>` : ''}
      </div>`).join('')}` : '';

    const tasksSection = dayTasks.length > 0 ? `
      <div class="calendar-day-panel-section">Задачи</div>
      ${dayTasks.map(t => {
        const statusIcon = t.status === 'done' ? '☑' : '☐';
        const tagsHtml = (t.tags || '').split(',').map(tg => tg.trim()).filter(Boolean)
          .map(tg => `<span class="note-tag badge-${S.tagColorMap[tg] || 'blue'}">${escapeHtml(tg)}</span>`).join('');
        return `<div class="calendar-task-item" data-id="${t.id}">
          <span class="note-status-icon" data-action="toggle-task">${statusIcon}</span>
          <span class="calendar-task-title" data-action="open-task">${escapeHtml(t.title || 'Без названия')}</span>
          ${tagsHtml}
        </div>`;
      }).join('')}` : '';

    dayPanelHtml = `<div class="calendar-day-panel">
      <div class="calendar-day-panel-header">
        <div class="calendar-day-panel-title">${S.selectedCalendarDate}</div>
        <button class="btn-sm btn-secondary" id="cal-add-task-btn">+ Задача</button>
      </div>
      ${eventsSection}${tasksSection}
      ${!hasContent ? '<div class="calendar-day-panel-empty">Нет событий</div>' : ''}
    </div>`;
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="cal-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[S.calendarMonth]} ${S.calendarYear}</div>
      <button class="calendar-nav-btn" id="cal-next">&gt;</button>
      <button class="btn-primary" id="cal-add-event" style="margin-left:16px;">+ Событие</button>
      <button class="btn-secondary" id="cal-sync" style="margin-left:8px;">&#x21BB; Синхр.</button>
    </div>
    <div class="calendar-weekdays">${weekdays.map(d => `<div class="calendar-weekday">${d}</div>`).join('')}</div>
    <div class="calendar-grid">${daysHtml}</div>
    ${dayPanelHtml}`;

  document.getElementById('cal-prev')?.addEventListener('click', () => {
    S.calendarMonth--;
    if (S.calendarMonth < 0) { S.calendarMonth = 11; S.calendarYear--; }
    loadCalendar();
  });
  document.getElementById('cal-next')?.addEventListener('click', () => {
    S.calendarMonth++;
    if (S.calendarMonth > 11) { S.calendarMonth = 0; S.calendarYear++; }
    loadCalendar();
  });
  document.querySelectorAll('.calendar-day:not(.other-month)').forEach(day => {
    day.addEventListener('click', () => {
      S.selectedCalendarDate = day.dataset.date;
      loadCalendar();
    });
  });
  document.getElementById('cal-add-event')?.addEventListener('click', () => showAddEventModal());
  document.getElementById('cal-sync')?.addEventListener('click', async () => {
    const btn = document.getElementById('cal-sync');
    if (btn) { btn.textContent = '...'; btn.disabled = true; }
    try {
      const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
      const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');
      let total = 0;
      let syncError = null;
      if (appleEnabled !== 'false') {
        const r = await invoke('sync_apple_calendar', { month: S.calendarMonth + 1, year: S.calendarYear });
        if (r.error) syncError = r.error;
        else total += r.synced || 0;
      }
      if (googleUrl) {
        const r = await invoke('sync_google_ics', { url: googleUrl, month: S.calendarMonth + 1, year: S.calendarYear });
        total += r.synced || 0;
      }
      if (syncError) {
        if (btn) { btn.textContent = '✗'; btn.title = syncError; }
        console.error('Calendar sync:', syncError);
      } else {
        if (btn) btn.textContent = `✓ ${total}`;
      }
      loadCalendar();
    } catch (e) {
      if (btn) btn.textContent = '✗';
      console.error('Calendar sync error:', e);
    }
    setTimeout(() => { if (btn) { btn.textContent = '↻ Синхр.'; btn.disabled = false; } }, 2000);
  });

  // Calendar task interactions
  document.querySelectorAll('.calendar-task-item').forEach(item => {
    item.querySelector('[data-action="toggle-task"]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = parseInt(item.dataset.id);
      const task = tasks.find(t => t.id === id);
      if (!task) return;
      const nextStatus = task.status === 'done' ? 'task' : 'done';
      await invoke('update_note_status', { id, status: nextStatus }).catch(err => console.error('cal task toggle:', err));
      loadCalendar();
    });
    item.querySelector('[data-action="open-task"]')?.addEventListener('click', () => {
      const id = parseInt(item.dataset.id);
      tabLoaders.switchTab?.('notes');
      setTimeout(() => {
        S.currentNoteId = id;
        S.notesViewMode = 'edit';
        const notesEl = document.getElementById('notes-content');
        if (notesEl) tabLoaders.renderNoteEditor?.(notesEl, id);
      }, 100);
    });
  });

  document.getElementById('cal-add-task-btn')?.addEventListener('click', async () => {
    try {
      const id = await invoke('create_note', { title: '', content: '', tags: '', status: 'task', tabName: null, dueDate: S.selectedCalendarDate, reminderAt: null });
      tabLoaders.switchTab?.('notes');
      setTimeout(() => {
        S.currentNoteId = id;
        S.notesViewMode = 'edit';
        const notesEl = document.getElementById('notes-content');
        if (notesEl) tabLoaders.renderNoteEditor?.(notesEl, id);
      }, 100);
    } catch (err) { console.error('cal add task:', err); }
  });
}

function renderWeekCalendar(el, events) {
  const weekdays = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
  const today = new Date();
  // Get start of current week (Monday)
  const dayOfWeek = today.getDay() || 7;
  const weekStart = new Date(today);
  weekStart.setDate(today.getDate() - dayOfWeek + 1 + (S.calWeekOffset || 0) * 7);

  const eventsByDate = {};
  for (const ev of events) {
    if (!eventsByDate[ev.date]) eventsByDate[ev.date] = [];
    eventsByDate[ev.date].push(ev);
  }

  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
  const hours = Array.from({length: 16}, (_, i) => i + 7); // 7:00 - 22:00

  let daysHeader = '';
  let dayDates = [];
  for (let i = 0; i < 7; i++) {
    const d = new Date(weekStart);
    d.setDate(weekStart.getDate() + i);
    const dateStr = `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
    dayDates.push(dateStr);
    const isToday = dateStr === todayStr;
    daysHeader += `<div class="week-header-day${isToday ? ' today' : ''}">
      <div class="week-header-weekday">${weekdays[i]}</div>
      <div class="week-header-date">${d.getDate()}</div>
    </div>`;
  }

  let gridHtml = '';
  for (const h of hours) {
    gridHtml += `<div class="week-time-label">${String(h).padStart(2,'0')}:00</div>`;
    for (let i = 0; i < 7; i++) {
      const dayEvts = (eventsByDate[dayDates[i]] || []).filter(e => {
        if (!e.time) return h === 9; // No time = show at 9
        const hour = parseInt(e.time.split(':')[0]);
        return hour === h;
      });
      gridHtml += `<div class="week-cell" data-date="${dayDates[i]}" data-hour="${h}">
        ${dayEvts.map(e => `<div class="week-event">${escapeHtml(e.title)}</div>`).join('')}
      </div>`;
    }
  }

  const startLabel = `${weekStart.getDate()} ${['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'][weekStart.getMonth()]}`;
  const endDate = new Date(weekStart);
  endDate.setDate(weekStart.getDate() + 6);
  const endLabel = `${endDate.getDate()} ${['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'][endDate.getMonth()]}`;

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="week-prev">&lt;</button>
      <div class="calendar-month-label">${startLabel} \u2014 ${endLabel} ${weekStart.getFullYear()}</div>
      <button class="calendar-nav-btn" id="week-next">&gt;</button>
      <button class="btn-secondary" id="week-today" style="margin-left:8px;">Сегодня</button>
      <button class="btn-primary" id="week-add-event" style="margin-left:8px;">+ Событие</button>
    </div>
    <div class="week-grid">
      <div class="week-time-label"></div>
      ${daysHeader}
      ${gridHtml}
    </div>`;

  document.getElementById('week-prev')?.addEventListener('click', () => { S.calWeekOffset = (S.calWeekOffset || 0) - 1; loadCalendar('Неделя'); });
  document.getElementById('week-next')?.addEventListener('click', () => { S.calWeekOffset = (S.calWeekOffset || 0) + 1; loadCalendar('Неделя'); });
  document.getElementById('week-today')?.addEventListener('click', () => { S.calWeekOffset = 0; loadCalendar('Неделя'); });
  document.getElementById('week-add-event')?.addEventListener('click', () => showAddEventModal());
  el.querySelectorAll('.week-cell').forEach(cell => {
    cell.addEventListener('click', () => {
      S.selectedCalendarDate = cell.dataset.date;
      showAddEventModal();
      setTimeout(() => {
        const timeInput = document.getElementById('event-time');
        if (timeInput) timeInput.value = `${String(cell.dataset.hour).padStart(2,'0')}:00`;
      }, 50);
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
      loadCalendar();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Day View ──
function renderDayCalendar(el, events) {
  const today = new Date();
  if (!S.calDayDate) S.calDayDate = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
  const dayEvents = events.filter(e => e.date === S.calDayDate).map(e => {
    // Normalize time to HH:MM (pad single-digit hour)
    if (e.time && /^\d:\d{2}$/.test(e.time)) e.time = '0' + e.time;
    return e;
  }).sort((a, b) => (a.time || '').localeCompare(b.time || ''));
  const d = new Date(S.calDayDate + 'T00:00:00');
  const dayNames = ['Воскресенье', 'Понедельник', 'Вторник', 'Среда', 'Четверг', 'Пятница', 'Суббота'];
  const monthNames = ['Января', 'Февраля', 'Марта', 'Апреля', 'Мая', 'Июня', 'Июля', 'Августа', 'Сентября', 'Октября', 'Ноября', 'Декабря'];

  const hours = Array.from({length: 17}, (_, i) => i + 6); // 6:00 - 22:00
  let timelineHtml = hours.map(h => {
    const timeStr = `${String(h).padStart(2,'0')}:`;
    const hourEvents = dayEvents.filter(e => e.time && e.time.startsWith(timeStr.slice(0,2)));
    const evtHtml = hourEvents.map(e => {
      const srcBadge = e.source && e.source !== 'manual' ? `<span class="badge badge-gray" style="margin-left:6px;">${e.source === 'apple' ? '🍎' : '📅'}</span>` : '';
      const endMin = (() => { const [hh,mm] = (e.time||'00:00').split(':').map(Number); const t = hh*60+mm+(e.duration_minutes||60); return `${String(Math.floor(t/60)%24).padStart(2,'0')}:${String(t%60).padStart(2,'0')}`; })();
      return `<div class="day-event" style="border-left:3px solid ${e.color || 'var(--text-secondary)'};">
        <span class="day-event-time">${e.time} – ${endMin}</span>
        <span class="day-event-title">${escapeHtml(e.title)}</span>${srcBadge}
        <span class="day-event-dur">${e.duration_minutes || 60} мин</span>
      </div>`;
    }).join('');
    return `<div class="day-hour-row">
      <div class="day-hour-label">${String(h).padStart(2,'0')}:00</div>
      <div class="day-hour-content" data-date="${S.calDayDate}" data-hour="${h}">${evtHtml}</div>
    </div>`;
  }).join('');

  // All-day events (no time)
  const allDay = dayEvents.filter(e => !e.time);
  const allDayHtml = allDay.length ? `<div class="day-allday">
    <div class="day-hour-label">Весь день</div>
    <div class="day-hour-content">${allDay.map(e => `<div class="day-event" style="border-left:3px solid ${e.color || 'var(--text-secondary)'};"><span class="day-event-title">${escapeHtml(e.title)}</span></div>`).join('')}</div>
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
    const dd = new Date(S.calDayDate + 'T00:00:00'); dd.setDate(dd.getDate() - 1);
    S.calDayDate = dd.toISOString().slice(0, 10);
    S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear();
    loadCalendar('День');
  });
  document.getElementById('day-next')?.addEventListener('click', () => {
    const dd = new Date(S.calDayDate + 'T00:00:00'); dd.setDate(dd.getDate() + 1);
    S.calDayDate = dd.toISOString().slice(0, 10);
    S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear();
    loadCalendar('День');
  });
  document.getElementById('day-today')?.addEventListener('click', () => {
    S.calDayDate = null; S.calendarMonth = today.getMonth(); S.calendarYear = today.getFullYear();
    loadCalendar('День');
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
    loadCalendar('Список');
  });
  document.getElementById('list-next')?.addEventListener('click', () => {
    S.calendarMonth++;
    if (S.calendarMonth > 11) { S.calendarMonth = 0; S.calendarYear++; }
    loadCalendar('Список');
  });
  document.getElementById('list-add-event')?.addEventListener('click', () => showAddEventModal());
  el.querySelectorAll('.cal-list-row').forEach(row => {
    row.addEventListener('click', () => {
      const ev = events.find(e => e.id === Number(row.dataset.id));
      if (ev) { S.selectedCalendarDate = ev.date; S.calDayDate = ev.date; const dd = new Date(ev.date); S.calendarMonth = dd.getMonth(); S.calendarYear = dd.getFullYear(); loadCalendar('День'); }
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
