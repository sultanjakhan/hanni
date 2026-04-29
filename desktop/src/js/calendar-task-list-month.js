// Month mode for "Список задач" — collapsible weekly sections.

import { S } from './state.js';
import { escapeHtml } from './utils.js';
import {
  todayStr, shiftDate, loadDayItems, renderItemRow,
  renderToolbar, wireToolbar, wireRowActions,
} from './calendar-task-list.js';

const MONTH_NAMES = ['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
const MONTHS_GEN = ['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'];
const WEEKDAYS = ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'];
const LS_EXPANDED = 'hanni_calendar_list_month_expanded';

function loadExpanded() { try { return new Set(JSON.parse(localStorage.getItem(LS_EXPANDED) || '[]')); } catch { return new Set(); } }
function saveExpanded(set) { try { localStorage.setItem(LS_EXPANDED, JSON.stringify([...set])); } catch {} }

function fmtDate(d) {
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}

function monthDates(year, month) {
  const last = new Date(year, month + 1, 0).getDate();
  return Array.from({length: last}, (_, i) => fmtDate(new Date(year, month, i + 1)));
}

// Find Monday of the ISO week for a given date string.
function weekStartFor(dateStr) {
  const d = new Date(dateStr + 'T12:00:00');
  const dow = d.getDay() || 7;
  d.setDate(d.getDate() - dow + 1);
  return fmtDate(d);
}

function groupByWeek(dates) {
  const groups = new Map();
  for (const d of dates) {
    const ws = weekStartFor(d);
    if (!groups.has(ws)) groups.set(ws, []);
    groups.get(ws).push(d);
  }
  return [...groups.entries()].map(([start, days]) => ({ start, days }));
}

function fmtWeekRangeLabel(weekStart) {
  const a = new Date(weekStart + 'T12:00:00');
  const b = new Date(a); b.setDate(a.getDate() + 6);
  const same = a.getMonth() === b.getMonth();
  return same
    ? `${a.getDate()}–${b.getDate()} ${MONTHS_GEN[a.getMonth()]}`
    : `${a.getDate()} ${MONTHS_GEN[a.getMonth()]} – ${b.getDate()} ${MONTHS_GEN[b.getMonth()]}`;
}

function fmtDayHeader(dateStr, dow) {
  const d = new Date(dateStr + 'T12:00:00');
  return `${WEEKDAYS[dow]} · ${d.getDate()} ${MONTHS_GEN[d.getMonth()]}`;
}

async function renderWeekSection(week, expanded, today) {
  const headerLabel = `Неделя · ${fmtWeekRangeLabel(week.start)}`;
  const cls = ['ctl-section', 'ctl-month-week', expanded && 'ctl-expanded'].filter(Boolean).join(' ');
  if (!expanded) {
    return `<div class="${cls}" data-week="${week.start}">
      <div class="ctl-week-header ctl-month-week-header" data-toggle-week="${week.start}">${escapeHtml(headerLabel)} <span class="ctl-month-chevron">▸</span></div>
    </div>`;
  }
  const itemsPerDay = await Promise.all(week.days.map(d => loadDayItems(d)));
  const totalCount = itemsPerDay.reduce((a, list) => a + list.length, 0);
  const bodyHtml = totalCount === 0
    ? `<div class="ctl-week-empty">— нет задач на этой неделе</div>`
    : week.days.map((date, i) => {
        const items = [...itemsPerDay[i]].sort((a, b) => (a.time || '99:99').localeCompare(b.time || '99:99'));
        if (!items.length) return '';
        const isToday = date === today;
        const hdrCls = ['ctl-month-day-header', isToday && 'ctl-week-today'].filter(Boolean).join(' ');
        return `<div class="ctl-month-day">
          <div class="${hdrCls}" data-jump-date="${date}">${escapeHtml(fmtDayHeader(date, i))}${isToday ? ' · сегодня' : ''}</div>
          ${items.map(it => renderItemRow(it, date)).join('')}
        </div>`;
      }).join('');
  return `<div class="${cls}" data-week="${week.start}">
    <div class="ctl-week-header ctl-month-week-header" data-toggle-week="${week.start}">${escapeHtml(headerLabel)} <span class="ctl-month-chevron">▾</span></div>
    ${bodyHtml}
  </div>`;
}

export async function renderMonthList(el) {
  const today = new Date();
  if (S.calendarMonth == null) S.calendarMonth = today.getMonth();
  if (S.calendarYear == null) S.calendarYear = today.getFullYear();
  const year = S.calendarYear, month = S.calendarMonth;
  const monthLabel = `${MONTH_NAMES[month]} ${year}`;
  const atCurrent = month === today.getMonth() && year === today.getFullYear();
  const todayDate = todayStr();

  const dates = monthDates(year, month);
  const weeks = groupByWeek(dates);

  // Default expanded: week containing today if current month, else first week
  const expanded = loadExpanded();
  if (!expanded.size) {
    const initial = atCurrent ? weekStartFor(todayDate) : weeks[0]?.start;
    if (initial) { expanded.add(initial); saveExpanded(expanded); }
  }

  const sectionsHtml = (await Promise.all(
    weeks.map(w => renderWeekSection(w, expanded.has(w.start), todayDate))
  )).join('');

  el.innerHTML = renderToolbar('month', monthLabel, atCurrent) + `<div class="ctl-body">${sectionsHtml}</div>`;
  wireToolbar(el, 'month');
  wireRowActions(el);

  el.querySelectorAll('[data-toggle-week]').forEach(h => {
    h.addEventListener('click', () => {
      const ws = h.dataset.toggleWeek;
      const set = loadExpanded();
      if (set.has(ws)) set.delete(ws); else set.add(ws);
      saveExpanded(set);
      renderMonthList(el);
    });
  });
  el.querySelectorAll('[data-jump-date]').forEach(h => {
    h.addEventListener('click', (e) => {
      e.stopPropagation();
      S.calDayDate = h.dataset.jumpDate;
      try { localStorage.setItem('hanni_calendar_list_mode', 'day'); } catch {}
      import('./calendar-task-list.js').then(({ renderCalendarTaskList }) => renderCalendarTaskList(el));
    });
  });
}
