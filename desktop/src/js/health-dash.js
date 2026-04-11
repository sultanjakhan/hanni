// health-dash.js — Health dashboard: sleep, steps, heart rate overview + trends
import { invoke } from './state.js';
import { renderSleepAnalysis } from './health-analysis.js';

export async function renderHealthDash(paneEl) {
  const today = new Date();
  const fmt = d => d.toISOString().slice(0, 10);
  const todayStr = fmt(today);
  const weekAgo = new Date(today); weekAgo.setDate(weekAgo.getDate() - 7);
  const weekStr = fmt(weekAgo);

  const [summary, sleepSessions, steps7, hr7] = await Promise.all([
    invoke('get_health_summary', { days: 7 }).catch(() => ({})),
    invoke('get_sleep_sessions', { from: weekStr, to: todayStr }).catch(() => []),
    getStepsWeek(weekStr, todayStr),
    invoke('get_heart_rate_samples', { from: weekStr, to: todayStr }).catch(() => []),
  ]);

  const avgSleepH = Math.floor((summary.avg_sleep_minutes || 0) / 60);
  const avgSleepM = Math.round((summary.avg_sleep_minutes || 0) % 60);
  const avgSteps = Math.round(summary.avg_steps || 0);
  const avgHr = Math.round(summary.avg_resting_hr || 0);

  // Sleep quality: deep+REM ratio from last session
  const lastSleep = sleepSessions[0];
  const sleepQuality = lastSleep ? calcSleepScore(lastSleep) : null;

  // Sleep debt: diff from 8h target over 7 days
  const totalSleepMin = sleepSessions.reduce((a, s) => a + s.duration_minutes, 0);
  const debtMin = Math.max(0, (8 * 60 * sleepSessions.length) - totalSleepMin);
  const debtH = Math.floor(debtMin / 60);

  // Bedtime consistency: stddev of sleep start times
  const bedtimes = sleepSessions.map(s => timeToMin(s.start_time));
  const consistency = bedtimes.length > 1 ? calcStdDev(bedtimes) : 0;
  const consistencyLabel = consistency < 30 ? 'Стабильно' : consistency < 60 ? 'Нормально' : 'Нестабильно';
  const consistencyColor = consistency < 30 ? 'green' : consistency < 60 ? 'yellow' : 'red';

  paneEl.innerHTML = `
    <div class="uni-dash-grid">
      <div class="uni-dash-card purple">
        <div class="uni-dash-value">${avgSleepH}ч ${avgSleepM}м</div>
        <div class="uni-dash-label">🌙 Средний сон (7д)</div>
      </div>
      <div class="uni-dash-card blue">
        <div class="uni-dash-value">${avgSteps.toLocaleString()}</div>
        <div class="uni-dash-label">👟 Средние шаги (7д)</div>
      </div>
      <div class="uni-dash-card ${avgHr ? 'red' : 'gray'}">
        <div class="uni-dash-value">${avgHr || '—'}</div>
        <div class="uni-dash-label">❤️ Средний пульс</div>
      </div>
      ${sleepQuality !== null ? `<div class="uni-dash-card ${sleepQuality >= 70 ? 'green' : sleepQuality >= 40 ? 'yellow' : 'red'}">
        <div class="uni-dash-value">${sleepQuality}</div>
        <div class="uni-dash-label">💤 Качество сна</div>
      </div>` : ''}
    </div>

    <div style="font-size:13px;font-weight:600;margin:16px 0 8px;color:var(--text-primary);">Паттерн сна</div>
    <div class="uni-dash-grid">
      <div class="uni-dash-card ${consistencyColor}">
        <div class="uni-dash-value">${consistencyLabel}</div>
        <div class="uni-dash-label">⏰ Режим засыпания</div>
      </div>
      <div class="uni-dash-card ${debtH > 3 ? 'red' : debtH > 0 ? 'yellow' : 'green'}">
        <div class="uni-dash-value">${debtH > 0 ? debtH + 'ч' : 'Ок'}</div>
        <div class="uni-dash-label">😴 Сонный долг (неделя)</div>
      </div>
    </div>

    ${renderSleepBars(sleepSessions)}
    ${renderStepsBars(steps7)}
    <div id="health-analysis-container"></div>
  `;

  // Render sleep analysis async (non-blocking)
  const analysisEl = paneEl.querySelector('#health-analysis-container');
  if (analysisEl) renderSleepAnalysis(analysisEl);
}

async function getStepsWeek(from, to) {
  try {
    const rows = await invoke('get_health_today');
    // get_health_today only returns today — for weekly, query health_log
    return [];
  } catch { return []; }
}

function calcSleepScore(session) {
  if (!session.stages?.length) return null;
  const totalMin = session.duration_minutes || 1;
  let deep = 0, rem = 0, awake = 0;
  for (const st of session.stages) {
    const d = stageDur(st);
    if (st.stage === 'deep') deep += d;
    else if (st.stage === 'rem') rem += d;
    else if (st.stage === 'awake') awake += d;
  }
  const deepPct = deep / totalMin;
  const remPct = rem / totalMin;
  const durScore = Math.min(1, totalMin / 480) * 40;
  const deepScore = Math.min(1, deepPct / 0.2) * 30;
  const remScore = Math.min(1, remPct / 0.25) * 30;
  return Math.round(durScore + deepScore + remScore);
}

function stageDur(st) {
  try {
    const s = new Date(st.start_time), e = new Date(st.end_time);
    return Math.max(0, (e - s) / 60000);
  } catch { return 0; }
}

function timeToMin(iso) {
  try {
    const d = new Date(iso);
    let m = d.getHours() * 60 + d.getMinutes();
    if (m > 720) m -= 1440; // normalize: 23:00 = -60, 01:00 = 60
    return m;
  } catch { return 0; }
}

function calcStdDev(arr) {
  const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
  const sq = arr.reduce((a, b) => a + (b - avg) ** 2, 0) / arr.length;
  return Math.round(Math.sqrt(sq));
}

function renderSleepBars(sessions) {
  if (!sessions.length) return '';
  const bars = sessions.slice(0, 7).reverse().map(s => {
    const h = s.duration_minutes / 60;
    const pct = Math.min(100, (h / 10) * 100);
    const color = h >= 7 ? 'var(--color-green)' : h >= 6 ? 'var(--color-yellow)' : 'var(--color-red)';
    return `<div class="health-bar-col">
      <div class="health-bar-fill" style="height:${pct}%;background:${color}"></div>
      <div class="health-bar-label">${s.date.slice(5)}</div>
    </div>`;
  }).join('');
  return `<div style="font-size:13px;font-weight:600;margin:16px 0 8px;color:var(--text-primary);">Сон за неделю</div>
    <div class="health-bar-chart">${bars}</div>`;
}

function renderStepsBars(steps) {
  if (!steps.length) return '';
  const max = Math.max(...steps.map(s => s.value), 10000);
  const bars = steps.slice(-7).map(s => {
    const pct = Math.min(100, (s.value / max) * 100);
    return `<div class="health-bar-col">
      <div class="health-bar-fill" style="height:${pct}%;background:var(--accent-blue)"></div>
      <div class="health-bar-label">${s.date?.slice(5) || ''}</div>
    </div>`;
  }).join('');
  return `<div style="font-size:13px;font-weight:600;margin:16px 0 8px;color:var(--text-primary);">Шаги за неделю</div>
    <div class="health-bar-chart">${bars}</div>`;
}
