// timeline-grid.js — Timeline tracking: sub-tabs (Day/Week/Month) + navigation
import { S, invoke } from './state.js';

let gridMode = S._tlView || 'week';
let gridOffset = 0;

export async function renderTimelineGrid(paneEl) {
  const views = [
    { id: 'day', label: 'День' },
    { id: 'week', label: 'Неделя' },
    { id: 'month', label: 'Месяц' },
  ];

  const navLabel = buildNavLabel();

  paneEl.innerHTML = `
    <div class="dev-filters" id="tl-view-tabs">
      ${views.map(v => `<button class="dev-filter-btn${v.id === gridMode ? ' active' : ''}" data-tlview="${v.id}">${v.label}</button>`).join('')}
    </div>
    <div class="tl-toolbar">
      <div style="display:flex;align-items:center;gap:6px;">
        <button class="tl-nav-btn" data-dir="-1">◀</button>
        <span class="tl-nav-label">${navLabel}</span>
        <button class="tl-nav-btn" data-dir="1">▶</button>
        ${gridOffset !== 0 ? '<button class="tl-nav-btn tl-today-btn" data-dir="0">Сегодня</button>' : ''}
      </div>
    </div>
    <div id="tl-inner-content"></div>`;

  // Sub-tab clicks
  paneEl.querySelectorAll('[data-tlview]').forEach(btn => {
    btn.addEventListener('click', () => {
      gridMode = btn.dataset.tlview;
      S._tlView = gridMode;
      gridOffset = 0;
      renderTimelineGrid(paneEl);
    });
  });

  // Navigation
  paneEl.querySelectorAll('.tl-nav-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const dir = parseInt(btn.dataset.dir);
      gridOffset = dir === 0 ? 0 : gridOffset + dir;
      renderTimelineGrid(paneEl);
    });
  });

  // Render inner content
  const innerEl = paneEl.querySelector('#tl-inner-content');
  await renderInner(innerEl, paneEl);
}

async function renderInner(innerEl, paneEl) {
  if (gridMode === 'month') {
    const { renderMonthGrid } = await import('./timeline-month.js');
    const today = new Date();
    const m = today.getMonth() + gridOffset;
    const year = today.getFullYear() + Math.floor(m / 12);
    const month = ((m % 12) + 12) % 12;
    await renderMonthGrid(innerEl, year, month);
    innerEl.addEventListener('tl-goto-day', (e) => {
      gridMode = 'day';
      S._tlView = 'day';
      const target = new Date(e.detail + 'T12:00:00');
      const today = new Date();
      const diff = Math.round((target - new Date(today.getFullYear(), today.getMonth(), today.getDate())) / 86400000);
      gridOffset = diff;
      renderTimelineGrid(paneEl);
    });
  } else {
    const { renderDayWeekGrid } = await import('./timeline-dayweek.js');
    const dates = buildDateRange(new Date(), gridMode, gridOffset);
    await renderDayWeekGrid(innerEl, dates, gridMode);
    innerEl.addEventListener('tl-refresh', () => renderTimelineGrid(paneEl));
  }
}

function buildNavLabel() {
  const today = new Date();
  if (gridMode === 'month') {
    const m = today.getMonth() + gridOffset;
    const year = today.getFullYear() + Math.floor(m / 12);
    const month = ((m % 12) + 12) % 12;
    return new Date(year, month, 1).toLocaleDateString('ru', { month: 'long', year: 'numeric' });
  }
  const dates = buildDateRange(today, gridMode, gridOffset);
  if (gridMode === 'day') return fmtDate(dates[0]);
  return `${fmtDate(dates[0])} — ${fmtDate(dates[dates.length - 1])}`;
}

function buildDateRange(today, mode, offset) {
  const dates = [];
  if (mode === 'day') {
    const d = new Date(today); d.setDate(d.getDate() + offset);
    dates.push(localDateStr(d));
  } else {
    const d = new Date(today);
    const dow = d.getDay() || 7;
    d.setDate(d.getDate() - dow + 1 + offset * 7);
    for (let i = 0; i < 7; i++) {
      const di = new Date(d); di.setDate(d.getDate() + i);
      dates.push(localDateStr(di));
    }
  }
  return dates;
}

function fmtDate(d) {
  return new Date(d + 'T12:00:00').toLocaleDateString('ru', { day: 'numeric', month: 'short' });
}

function localDateStr(d) {
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}
