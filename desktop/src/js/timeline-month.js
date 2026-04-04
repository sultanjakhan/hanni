// timeline-month.js — Month grid renderer for Timeline
import { invoke } from './state.js';

export async function renderMonthGrid(paneEl, year, month) {
  const types = await invoke('get_activity_types').catch(() => []);
  const firstDay = new Date(year, month, 1);
  const lastDay = new Date(year, month + 1, 0);
  const startDow = firstDay.getDay() || 7; // Mon=1

  // Fetch all blocks for the month
  const blocksByDate = {};
  for (let d = 1; d <= lastDay.getDate(); d++) {
    const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
    blocksByDate[dateStr] = await invoke('get_timeline_blocks', { date: dateStr }).catch(() => []);
  }

  const now = new Date();
  const todayStr = `${now.getFullYear()}-${String(now.getMonth()+1).padStart(2,'0')}-${String(now.getDate()).padStart(2,'0')}`;
  const dayNames = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
  const headerRow = dayNames.map(d => `<th class="tl-month-header">${d}</th>`).join('');

  // Build weeks
  let cells = '';
  let dayNum = 1;
  const totalCells = Math.ceil((startDow - 1 + lastDay.getDate()) / 7) * 7;

  for (let i = 0; i < totalCells; i++) {
    if (i % 7 === 0) cells += '<tr>';
    if (i < startDow - 1 || dayNum > lastDay.getDate()) {
      cells += '<td class="tl-month-cell tl-month-empty"></td>';
    } else {
      const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(dayNum).padStart(2, '0')}`;
      const blocks = blocksByDate[dateStr] || [];
      const isToday = dateStr === todayStr;
      const totalMin = blocks.reduce((sum, b) => sum + (b.duration_minutes || 0), 0);
      const bars = buildTypeBars(blocks, types);
      cells += `<td class="tl-month-cell${isToday ? ' tl-today' : ''}" data-date="${dateStr}">
        <div class="tl-month-day-num">${dayNum}</div>
        <div class="tl-month-bars">${bars}</div>
        ${totalMin > 0 ? `<div class="tl-month-total">${fmtMin(totalMin)}</div>` : ''}
      </td>`;
      dayNum++;
    }
    if (i % 7 === 6) cells += '</tr>';
  }

  paneEl.innerHTML = `<div class="tl-month-wrap">
    <table class="tl-month-grid"><thead><tr>${headerRow}</tr></thead><tbody>${cells}</tbody></table>
  </div>`;

  // Click day → switch to day view
  paneEl.querySelectorAll('.tl-month-cell[data-date]').forEach(cell => {
    cell.addEventListener('click', () => {
      paneEl.dispatchEvent(new CustomEvent('tl-goto-day', { detail: cell.dataset.date }));
    });
  });
}

function buildTypeBars(blocks, types) {
  const byType = {};
  for (const b of blocks) {
    byType[b.type_id] = (byType[b.type_id] || 0) + (b.duration_minutes || 0);
  }
  return Object.entries(byType).map(([tid, min]) => {
    const t = types.find(t => t.id === parseInt(tid));
    const color = t?.color || '#999';
    const pct = Math.min(100, (min / 1440) * 100);
    return `<div class="tl-month-bar" style="background:${color};width:${Math.max(8, pct)}%" title="${t?.name || ''}: ${fmtMin(min)}"></div>`;
  }).join('');
}

function fmtMin(m) {
  if (m >= 60) return `${Math.floor(m / 60)}ч${m % 60 ? ' ' + (m % 60) + 'м' : ''}`;
  return `${m}м`;
}
