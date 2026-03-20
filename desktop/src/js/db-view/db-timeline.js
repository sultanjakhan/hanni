// ── db-view/db-timeline.js — Horizontal timeline view grouped by date ──

import { escapeHtml } from '../utils.js';
import { formatPropValue } from './db-cell-editors.js';

/**
 * Render a horizontal timeline view.
 * Groups records by a date field (first date column found).
 */
export function renderTimelineView(el, ctx) {
  const { records, fixedColumns, customProps = [], valuesMap = {}, idField = 'id', onRowClick } = ctx;

  // Find date field: custom prop or fixed column
  const dateProp = customProps.find(p => p.type === 'date' && p.visible !== false);
  const dateFixed = fixedColumns.find(c => c.editType === 'date' || c.key === 'date' || c.key === 'deadline');
  const dateKey = dateProp ? `prop_${dateProp.id}` : (dateFixed ? dateFixed.key : null);

  if (!dateKey) {
    el.innerHTML = `<div class="dbv-timeline-empty">Добавьте поле с типом «Дата» для таймлайна</div>`;
    return;
  }

  // Extract dates and group
  const groups = {};
  const noDate = [];
  for (const rec of records) {
    const dateVal = dateKey.startsWith('prop_')
      ? valuesMap[rec[idField]]?.[parseInt(dateKey.substring(5))] || ''
      : rec[dateKey] || '';
    if (!dateVal) { noDate.push(rec); continue; }
    const d = dateVal.substring(0, 10); // YYYY-MM-DD
    if (!groups[d]) groups[d] = [];
    groups[d].push(rec);
  }

  const sortedDates = Object.keys(groups).sort();
  if (sortedDates.length === 0) {
    el.innerHTML = `<div class="dbv-timeline-empty">Нет записей с датами</div>`;
    return;
  }

  // Calculate date range for layout
  const titleCol = fixedColumns[0];
  const titleKey = titleCol ? titleCol.key : idField;

  let html = '<div class="dbv-timeline-wrap"><div class="dbv-timeline-track">';
  // Month headers
  html += '<div class="dbv-timeline-months">';
  let prevMonth = '';
  for (const d of sortedDates) {
    const month = formatMonth(d);
    if (month !== prevMonth) {
      html += `<div class="dbv-timeline-month">${month}</div>`;
      prevMonth = month;
    }
  }
  html += '</div>';

  // Date columns with items
  html += '<div class="dbv-timeline-cols">';
  for (const d of sortedDates) {
    const day = parseInt(d.substring(8));
    const weekday = new Date(d).toLocaleDateString('ru', { weekday: 'short' });
    html += `<div class="dbv-timeline-col">
      <div class="dbv-timeline-day"><span class="dbv-timeline-daynum">${day}</span><span class="dbv-timeline-weekday">${weekday}</span></div>
      <div class="dbv-timeline-items">`;
    for (const rec of groups[d]) {
      const title = escapeHtml(String(rec[titleKey] || `#${rec[idField]}`));
      html += `<div class="dbv-timeline-item" data-id="${rec[idField]}">${title}</div>`;
    }
    html += '</div></div>';
  }
  html += '</div></div></div>';

  el.innerHTML = html;

  // Wire clicks
  if (onRowClick) {
    el.querySelectorAll('.dbv-timeline-item').forEach(item => {
      item.addEventListener('click', () => {
        const rec = records.find(r => r[idField] === parseInt(item.dataset.id));
        if (rec) onRowClick(rec);
      });
    });
  }
}

function formatMonth(dateStr) {
  const months = ['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
  const [y, m] = dateStr.split('-');
  return `${months[parseInt(m) - 1]} ${y}`;
}
