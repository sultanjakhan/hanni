// timeline-dayweek.js — Day/Week grid renderer for Timeline
import { invoke } from './state.js';

let hasScrolled = false;

export function resetScroll() { hasScrolled = false; }

export async function renderDayWeekGrid(paneEl, dates, mode) {
  const allBlocks = {};
  for (const d of dates) {
    allBlocks[d] = await invoke('get_timeline_blocks', { date: d }).catch(() => []);
  }

  const hours = [];
  for (let h = 0; h < 24; h++) {
    hours.push(`${String(h).padStart(2, '0')}:00`);
    hours.push(`${String(h).padStart(2, '0')}:30`);
  }

  const todayStr = localDateStr(new Date());
  const dayHeaders = dates.map(d => {
    const dt = new Date(d + 'T12:00:00');
    const label = mode === 'day'
      ? dt.toLocaleDateString('ru', { day: 'numeric', month: 'short' })
      : dt.toLocaleDateString('ru', { weekday: 'short', day: 'numeric' });
    const isToday = d === todayStr;
    return `<th class="tl-day-header${isToday ? ' tl-today' : ''}">${label}</th>`;
  }).join('');

  const rows = hours.map((time, i) => {
    const showLabel = i % 2 === 0;
    const cells = dates.map(d => {
      const block = findBlockAt(allBlocks[d], time);
      if (block) {
        if (block.start_time !== time) return '';
        const span = calcSlotSpan(block.start_time, block.end_time);
        return `<td class="tl-cell tl-block-cell" rowspan="${span}" style="background:${block.type_color}22;border-left:3px solid ${block.type_color};" data-block-id="${block.id}" data-date="${d}" title="${block.type_name}: ${block.start_time}–${block.end_time}">
          <span class="tl-block-label">${block.type_icon} ${block.type_name}</span>
        </td>`;
      }
      if (isCoveredByBlock(allBlocks[d], time)) return '';
      return `<td class="tl-cell tl-empty-cell" data-date="${d}" data-time="${time}"></td>`;
    }).join('');
    return `<tr class="tl-row${showLabel ? '' : ' tl-row-half'}">
      <td class="tl-time-label">${showLabel ? time : ''}</td>${cells}
    </tr>`;
  }).join('');

  // Now-line
  const now = new Date();
  const nowMin = now.getHours() * 60 + now.getMinutes();
  const nowTop = (nowMin / 30) * 20 + 30;
  const todayIdx = dates.indexOf(todayStr);
  const nowLine = todayIdx >= 0
    ? `<div class="tl-now-line" style="top:${nowTop}px;left:calc(50px + ${todayIdx} * (100% - 50px) / ${dates.length});width:calc((100% - 50px) / ${dates.length});"></div>`
    : '';

  // Save scroll position before re-render
  const scrollParent = paneEl.closest('.tab-content') || paneEl.parentElement;
  const savedScroll = scrollParent?.scrollTop || 0;

  paneEl.innerHTML = `<div class="tl-grid-wrap">
    <table class="tl-grid"><thead><tr><th class="tl-time-label"></th>${dayHeaders}</tr></thead><tbody>${rows}</tbody></table>
    ${nowLine}
  </div>`;

  // Auto-scroll to current time on first render, restore position on re-renders
  if (!hasScrolled) {
    const nowLineEl = paneEl.querySelector('.tl-now-line');
    if (nowLineEl) { nowLineEl.scrollIntoView({ block: 'center', behavior: 'instant' }); hasScrolled = true; }
  } else if (scrollParent && savedScroll > 0) {
    scrollParent.scrollTop = savedScroll;
  }

  // Click handlers
  paneEl.querySelectorAll('.tl-empty-cell').forEach(cell => {
    cell.addEventListener('click', async () => {
      const { showBlockModal } = await import('./timeline-blocks.js');
      await showBlockModal(cell.dataset.date, cell.dataset.time);
      // Re-render is handled by parent
      paneEl.dispatchEvent(new Event('tl-refresh'));
    });
  });
  paneEl.querySelectorAll('.tl-block-cell').forEach(cell => {
    cell.addEventListener('click', async () => {
      const { showBlockModal } = await import('./timeline-blocks.js');
      await showBlockModal(cell.dataset.date, null, parseInt(cell.dataset.blockId));
      paneEl.dispatchEvent(new Event('tl-refresh'));
    });
  });
}

function findBlockAt(blocks, time) {
  return blocks?.find(b => b.start_time === time);
}

function isCoveredByBlock(blocks, time) {
  if (!blocks) return false;
  const t = timeToMinutes(time);
  return blocks.some(b => t > timeToMinutes(b.start_time) && t < timeToMinutes(b.end_time));
}

function calcSlotSpan(start, end) {
  const s = timeToMinutes(start);
  const e = timeToMinutes(end);
  return Math.max(1, Math.round(((e > s ? e - s : 1440 - s + e)) / 30));
}

function timeToMinutes(t) {
  const [h, m] = t.split(':').map(Number);
  return h * 60 + m;
}

function localDateStr(d) {
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}
