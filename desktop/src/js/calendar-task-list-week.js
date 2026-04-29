// ── js/calendar-task-list-week.js — Week mode for "Список задач" ──
// 7 day sections, today highlighted, prev/next-week navigation.

import { S } from './state.js';
import { escapeHtml } from './utils.js';
import {
  todayStr, shiftDate, loadDayItems, renderItemRow,
  renderToolbar, wireToolbar, wireRowActions,
} from './calendar-task-list.js';

const MONTHS_GEN = ['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'];
const WEEKDAYS = ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'];

function weekStartDate(offset) {
  const t = new Date();
  const dow = t.getDay() || 7;
  t.setDate(t.getDate() - dow + 1 + offset * 7);
  return `${t.getFullYear()}-${String(t.getMonth()+1).padStart(2,'0')}-${String(t.getDate()).padStart(2,'0')}`;
}

function weekDates(offset) {
  const start = weekStartDate(offset);
  return Array.from({length: 7}, (_, i) => shiftDate(start, i));
}

function fmtDayLabel(dateStr, idx) {
  const d = new Date(dateStr + 'T12:00:00');
  return `${WEEKDAYS[idx]} · ${d.getDate()} ${MONTHS_GEN[d.getMonth()]}`;
}

function fmtWeekLabel(dates) {
  const a = new Date(dates[0] + 'T12:00:00');
  const b = new Date(dates[6] + 'T12:00:00');
  const same = a.getMonth() === b.getMonth();
  return same
    ? `${a.getDate()}–${b.getDate()} ${MONTHS_GEN[a.getMonth()]} ${a.getFullYear()}`
    : `${a.getDate()} ${MONTHS_GEN[a.getMonth()]} – ${b.getDate()} ${MONTHS_GEN[b.getMonth()]} ${a.getFullYear()}`;
}

function sortByTime(items) {
  return [...items].sort((a, b) => (a.time || '99:99').localeCompare(b.time || '99:99'));
}

export async function renderWeekList(el) {
  const offset = S.calWeekOffset || 0;
  const dates = weekDates(offset);
  const today = todayStr();
  const isCurrentWeek = offset === 0;
  const weekLabel = fmtWeekLabel(dates);

  const itemsPerDay = await Promise.all(dates.map(d => loadDayItems(d)));
  const totalCount = itemsPerDay.reduce((a, list) => a + list.length, 0);

  let bodyHtml;
  if (totalCount === 0) {
    bodyHtml = `<div class="ctl-empty">
      <div class="ctl-empty-title">Нет задач на эту неделю</div>
      <button class="btn-primary" id="ctl-add-empty">+ Запланировать</button>
    </div>`;
  } else {
    bodyHtml = dates.map((date, i) => {
      const items = sortByTime(itemsPerDay[i]);
      const isToday = date === today;
      const isPast = date < today;
      const cls = ['ctl-section', 'ctl-week-day', isToday && 'ctl-week-today', isPast && 'ctl-week-past'].filter(Boolean).join(' ');
      const headerLabel = fmtDayLabel(date, i);
      const rowsHtml = items.length
        ? items.map(it => renderItemRow(it, date)).join('')
        : `<div class="ctl-week-empty">— нет задач</div>`;
      return `<div class="${cls}">
        <div class="ctl-section-title ctl-week-header" data-jump-date="${date}">${escapeHtml(headerLabel)}${isToday ? ' · сегодня' : ''}</div>
        ${rowsHtml}
      </div>`;
    }).join('');
  }

  el.innerHTML = renderToolbar('week', weekLabel, isCurrentWeek) + `<div class="ctl-body">${bodyHtml}</div>`;
  wireToolbar(el, 'week');
  wireRowActions(el);

  el.querySelectorAll('[data-jump-date]').forEach(h => {
    h.addEventListener('click', async () => {
      S.calDayDate = h.dataset.jumpDate;
      try { localStorage.setItem('hanni_calendar_list_mode', 'day'); } catch {}
      const { renderCalendarTaskList } = await import('./calendar-task-list.js');
      renderCalendarTaskList(el);
    });
  });
}
