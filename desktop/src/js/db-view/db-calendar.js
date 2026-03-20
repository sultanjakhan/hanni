// ── db-view/db-calendar.js — Monthly calendar grid view ──

import { escapeHtml } from '../utils.js';

const MONTH_NAMES = ['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
const WEEKDAYS = ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'];

/**
 * Render a monthly calendar grid with records placed on their date.
 */
export function renderCalendarView(el, ctx) {
  const { records, fixedColumns, customProps = [], valuesMap = {}, idField = 'id', onRowClick } = ctx;

  // Find date field
  const dateProp = customProps.find(p => p.type === 'date' && p.visible !== false);
  const dateFixed = fixedColumns.find(c => c.editType === 'date' || c.key === 'date' || c.key === 'deadline');
  const dateKey = dateProp ? `prop_${dateProp.id}` : (dateFixed ? dateFixed.key : null);

  if (!dateKey) {
    el.innerHTML = `<div class="dbv-cal-empty">Добавьте поле с типом «Дата» для календаря</div>`;
    return;
  }

  // State: current month
  const now = new Date();
  let year = now.getFullYear(), month = now.getMonth();

  const render = () => {
    const titleCol = fixedColumns[0];
    const titleKey = titleCol ? titleCol.key : idField;

    // Group records by date
    const byDate = {};
    for (const rec of records) {
      const dv = dateKey.startsWith('prop_')
        ? valuesMap[rec[idField]]?.[parseInt(dateKey.substring(5))] || ''
        : rec[dateKey] || '';
      if (!dv) continue;
      const d = dv.substring(0, 10);
      if (!byDate[d]) byDate[d] = [];
      byDate[d].push(rec);
    }

    // Build calendar grid
    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const startWeekday = (firstDay.getDay() + 6) % 7; // Monday = 0

    let html = `<div class="dbv-cal">
      <div class="dbv-cal-header">
        <button class="dbv-cal-nav dbv-action-btn" data-dir="-1">&larr;</button>
        <span class="dbv-cal-title">${MONTH_NAMES[month]} ${year}</span>
        <button class="dbv-cal-nav dbv-action-btn" data-dir="1">&rarr;</button>
        <button class="dbv-cal-nav dbv-action-btn dbv-cal-today">Сегодня</button>
      </div>
      <div class="dbv-cal-grid">`;

    // Weekday headers
    for (const wd of WEEKDAYS) {
      html += `<div class="dbv-cal-weekday">${wd}</div>`;
    }

    // Empty cells before first day
    for (let i = 0; i < startWeekday; i++) html += '<div class="dbv-cal-cell empty"></div>';

    const todayStr = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;

    for (let d = 1; d <= lastDay.getDate(); d++) {
      const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
      const isToday = dateStr === todayStr;
      const items = byDate[dateStr] || [];
      html += `<div class="dbv-cal-cell${isToday ? ' today' : ''}">
        <div class="dbv-cal-daynum">${d}</div>`;
      for (const rec of items.slice(0, 3)) {
        html += `<div class="dbv-cal-item" data-id="${rec[idField]}">${escapeHtml(String(rec[titleKey] || ''))}</div>`;
      }
      if (items.length > 3) html += `<div class="dbv-cal-more">+${items.length - 3}</div>`;
      html += '</div>';
    }

    html += '</div></div>';
    el.innerHTML = html;

    // Wire navigation
    el.querySelectorAll('.dbv-cal-nav').forEach(btn => {
      btn.addEventListener('click', () => {
        if (btn.classList.contains('dbv-cal-today')) { year = now.getFullYear(); month = now.getMonth(); }
        else { month += parseInt(btn.dataset.dir); if (month < 0) { month = 11; year--; } if (month > 11) { month = 0; year++; } }
        render();
      });
    });

    // Wire item clicks
    if (onRowClick) {
      el.querySelectorAll('.dbv-cal-item').forEach(item => {
        item.addEventListener('click', () => {
          const rec = records.find(r => r[idField] === parseInt(item.dataset.id));
          if (rec) onRowClick(rec);
        });
      });
    }
  };

  render();
}
