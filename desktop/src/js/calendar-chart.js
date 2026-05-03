// Calendar Dashboard — Chart sub-tab.
// One bar per day = completion percent. Color tier (red/yellow/green).
// Filters: types + categories. Summary: avg, best, total, streak, missed, trend.
// Click on column → opens Day-view list mode for that date.

import { S } from './state.js';
import { escapeHtml } from './utils.js';
import {
  RU_DOW, computeDays, loadChartData,
  computeStreak, computeMissed, computeAvg, computeTrend,
} from './calendar-chart-stats.js';
import {
  getFilters, isFilterActive, renderFilterPopup, wireFilters, wireFilterPopup,
} from './calendar-chart-filters.js';

const PERIODS = [7, 30, 90];

function getPeriod() {
  if (PERIODS.includes(S.calChartPeriod)) return S.calChartPeriod;
  try { S.calChartPeriod = parseInt(localStorage.getItem('hanni_cal_chart_period') || '7', 10); } catch { S.calChartPeriod = 7; }
  return PERIODS.includes(S.calChartPeriod) ? S.calChartPeriod : 7;
}
function setPeriod(p) {
  S.calChartPeriod = p;
  try { localStorage.setItem('hanni_cal_chart_period', String(p)); } catch {}
}

function dayLabel(s) {
  const d = new Date(s + 'T12:00:00');
  return `${d.getDate()}.${String(d.getMonth()+1).padStart(2,'0')}`;
}
function dayLabelDow(s) {
  const d = new Date(s + 'T12:00:00');
  return `${RU_DOW[d.getDay()]} ${d.getDate()}.${String(d.getMonth()+1).padStart(2,'0')}`;
}
function dowShort(s) { return RU_DOW[new Date(s + 'T12:00:00').getDay()]; }

function shouldShowLabel(i, n) {
  if (n <= 7) return true;
  if (n <= 30) return i % 3 === 0 || i === n - 1;
  return i % 7 === 0 || i === n - 1;
}

function openDay(day) {
  S.calDayDate = day;
  S._calendarInner = 'day';
  S.calDayMode = 'list';
  try { localStorage.setItem('hanni_calendar_day_mode', 'list'); } catch {}
  document.querySelector('.uni-tab[data-pane="table"]')?.click();
}

function renderBars(area, data) {
  const n = data.length;
  const showDow = n <= 7;
  const cols = data.map((d, i) => {
    const isToday = i === n - 1;
    const showLabel = shouldShowLabel(i, n);
    const tip = d.planned > 0
      ? `${dayLabelDow(d.day)} · ${d.done}/${d.planned} (${d.pct}%) · события ${d.ev.done}/${d.ev.total} · задачи ${d.tk.done}/${d.tk.total} · расписание ${d.sc.done}/${d.sc.total}\n[клик — открыть день]`
      : `${dayLabelDow(d.day)} · ничего не запланировано\n[клик — открыть день]`;
    const heightPct = d.planned > 0 ? Math.max(d.pct === 0 ? 1.5 : 6, d.pct) : 0;
    const fill = d.planned > 0
      ? `<div class="cal-chart-bar-fill tier-${d.tier}" style="height:${heightPct}%"></div>`
      : '';
    const pctBelow = (d.planned > 0 && showLabel)
      ? `<div class="cal-chart-pct">${d.pct}%</div>`
      : '<div class="cal-chart-pct empty">·</div>';
    const dowEl = showDow ? `<div class="cal-chart-dow">${dowShort(d.day)}</div>` : '';
    const dateEl = showLabel ? dayLabel(d.day) : '·';
    return `<div class="cal-chart-col${isToday ? ' is-today' : ''}" data-day="${d.day}" title="${escapeHtml(tip)}">
      <div class="cal-chart-bar">${fill}</div>
      ${dowEl}
      <div class="cal-chart-xlabel${showLabel ? '' : ' faint'}">${dateEl}</div>
      ${pctBelow}
    </div>`;
  }).join('');
  const yaxis = `<div class="cal-chart-yaxis">
    <span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0%</span>
  </div>`;
  area.innerHTML = `<div class="cal-chart-plot${showDow ? ' has-dow' : ''}">${yaxis}<div class="cal-chart-bars">${cols}</div></div>`;
  area.querySelectorAll('.cal-chart-col').forEach(col => {
    col.addEventListener('click', () => openDay(col.dataset.day));
  });
}

function renderSummary(data, streak, missed, trend) {
  const filled = data.filter(d => d.planned > 0);
  if (!filled.length) {
    return `<div class="cal-chart-summary"><span class="cal-chart-summary-empty">За период ничего не запланировано</span></div>`;
  }
  const totalPlanned = filled.reduce((s, d) => s + d.planned, 0);
  const totalDone = filled.reduce((s, d) => s + d.done, 0);
  const avgPct = computeAvg(data);
  const best = filled.reduce((b, d) => (d.pct > b.pct ? d : b), filled[0]);
  const trendStr = trend == null ? '—' : (trend === 0 ? '0' : (trend > 0 ? `+${trend}` : `${trend}`)) + ' п.п.';
  const trendCls = trend == null ? '' : (trend > 0 ? ' positive' : (trend < 0 ? ' negative' : ''));
  const card = (label, value, cls = '') => `<div class="cal-chart-summary-item${cls}">
    <div class="cal-chart-summary-label">${label}</div>
    <div class="cal-chart-summary-value">${value}</div>
  </div>`;
  return `<div class="cal-chart-summary">
    ${card('Средний', avgPct + '%')}
    ${card('Лучший день', `${dayLabel(best.day)} · ${best.pct}%`)}
    ${card('Всего', `${totalDone}/${totalPlanned}`)}
    ${card('Серия ≥50%', streak ? `${streak} дн.` : '—')}
    ${card('Пропущено (0%)', missed ? `${missed} дн.` : '—')}
    ${card('Тренд', trendStr, trendCls)}
  </div>`;
}


export async function renderCalendarChart(el) {
  const period = getPeriod();
  const filters = getFilters();
  const filterDot = isFilterActive(filters) ? '<span class="cal-chart-filter-dot"></span>' : '';
  el.innerHTML = `
    <div class="cal-chart-toolbar">
      <div class="dev-filters">
        ${PERIODS.map(p => `<button class="dev-filter-btn${p === period ? ' active' : ''}" data-chart-period="${p}">${p} дней</button>`).join('')}
      </div>
      <div class="cal-chart-filter-wrap">
        <button class="btn-sm btn-secondary cal-chart-filter-btn">🔧 Фильтр${filterDot}</button>
        <div id="cal-chart-filter-popup-slot"></div>
      </div>
      <div class="cal-chart-hint">Процент выполнения · клик по столбику = открыть день</div>
    </div>
    <div class="cal-chart-area"><div class="cal-chart-loading">Загружаю…</div></div>
    <div id="cal-chart-summary-slot"></div>`;

  el.querySelectorAll('[data-chart-period]').forEach(btn => {
    btn.addEventListener('click', () => { setPeriod(parseInt(btn.dataset.chartPeriod, 10)); renderCalendarChart(el); });
  });

  const days = computeDays(period);
  const { data, availableCats } = await loadChartData(days, filters);

  const popupSlot = el.querySelector('#cal-chart-filter-popup-slot');
  popupSlot.innerHTML = renderFilterPopup(filters, availableCats);
  wireFilters(popupSlot, filters, availableCats, () => renderCalendarChart(el));
  wireFilterPopup(el.querySelector('.cal-chart-filter-wrap'));

  renderBars(el.querySelector('.cal-chart-area'), data);

  const streak = computeStreak(data, 50);
  const missed = computeMissed(data);
  const trend = await computeTrend(data, period, filters).catch(() => null);
  el.querySelector('#cal-chart-summary-slot').innerHTML = renderSummary(data, streak, missed, trend);
}
