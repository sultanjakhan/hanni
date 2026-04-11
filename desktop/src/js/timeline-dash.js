// timeline-dash.js — Timeline dashboard with stats and trends
import { invoke } from './state.js';

export async function renderTimelineDash(paneEl) {
  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;

  const weekStart = new Date(today);
  const dow = weekStart.getDay() || 7;
  weekStart.setDate(weekStart.getDate() - dow + 1);
  const weekStartStr = `${weekStart.getFullYear()}-${String(weekStart.getMonth()+1).padStart(2,'0')}-${String(weekStart.getDate()).padStart(2,'0')}`;

  const [dayStats, rangeStats, healthSummary] = await Promise.all([
    invoke('get_timeline_day_stats', { date: todayStr }).catch(() => ({ per_type: [], total_minutes: 0 })),
    invoke('get_timeline_range_stats', { startDate: weekStartStr, endDate: todayStr }).catch(() => ({ current: [], previous_totals: {}, days: 7 })),
    invoke('get_health_summary', { days: 7 }).catch(() => null),
  ]);

  const tracked = dayStats.total_minutes;
  const todayCards = dayStats.per_type.filter(t => t.minutes > 0).map(t =>
    `<div class="uni-dash-card blue">
      <div class="uni-dash-value">${fmtMin(t.minutes)}</div>
      <div class="uni-dash-label">${t.icon} ${t.name}</div>
    </div>`
  ).join('');

  const weekCards = rangeStats.current?.map(t => {
    const avg = rangeStats.days > 0 ? Math.round(t.total_minutes / rangeStats.days) : 0;
    const prevTotal = rangeStats.previous_totals?.[String(t.id)] || 0;
    const prevAvg = rangeStats.days > 0 ? Math.round(prevTotal / rangeStats.days) : 0;
    const delta = avg - prevAvg;
    const deltaStr = delta !== 0 ? ` <span style="font-size:12px;color:${delta > 0 ? 'var(--color-green)' : 'var(--color-red)'}">${delta > 0 ? '+' : ''}${delta}м</span>` : '';
    return `<div class="uni-dash-card green">
      <div class="uni-dash-value">${fmtMin(avg)}${deltaStr}</div>
      <div class="uni-dash-label">${t.icon} ${t.name} (сред./день)</div>
    </div>`;
  }).join('') || '';

  paneEl.innerHTML = `
    <div class="uni-dash-grid">
      <div class="uni-dash-card purple">
        <div class="uni-dash-value">${fmtMin(tracked)}</div>
        <div class="uni-dash-label">Размечено сегодня</div>
      </div>
      ${todayCards}
    </div>
    ${weekCards ? `<div style="font-size:13px;font-weight:600;margin:16px 0 8px;color:var(--text-primary);">Среднее за неделю (vs прошлая)</div>
    <div class="uni-dash-grid">${weekCards}</div>` : ''}
    ${healthSummary ? renderHealthCards(healthSummary) : ''}`;
}

function renderHealthCards(h) {
  if (!h.sleep_sessions && !h.days_with_steps && !h.hr_samples) return '';
  const sleepH = Math.floor(h.avg_sleep_minutes / 60);
  const sleepM = Math.round(h.avg_sleep_minutes % 60);
  const cards = [];
  if (h.sleep_sessions > 0) cards.push(`<div class="uni-dash-card purple"><div class="uni-dash-value">${sleepH}ч ${sleepM}м</div><div class="uni-dash-label">🌙 Средний сон (7д)</div></div>`);
  if (h.days_with_steps > 0) cards.push(`<div class="uni-dash-card blue"><div class="uni-dash-value">${Math.round(h.avg_steps).toLocaleString()}</div><div class="uni-dash-label">👟 Средние шаги (7д)</div></div>`);
  if (h.hr_samples > 0) cards.push(`<div class="uni-dash-card red"><div class="uni-dash-value">${Math.round(h.avg_resting_hr)}</div><div class="uni-dash-label">❤️ Средний пульс</div></div>`);
  if (!cards.length) return '';
  return `<div style="font-size:13px;font-weight:600;margin:16px 0 8px;color:var(--text-primary);">Здоровье (Samsung Health)</div><div class="uni-dash-grid">${cards.join('')}</div>`;
}

function fmtMin(m) {
  if (m < 60) return `${m} мин`;
  const h = Math.floor(m / 60);
  const min = m % 60;
  return min > 0 ? `${h}ч ${min}м` : `${h}ч`;
}
