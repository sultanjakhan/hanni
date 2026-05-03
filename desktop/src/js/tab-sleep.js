// tab-sleep.js — Sleep sessions pane for Health tab
import { invoke, IS_MOBILE } from './state.js';
import { escapeHtml } from './utils.js';
import { autoImportHealth } from './health-auto-sync.js';

const STAGE_COLORS = {
  deep: 'var(--color-purple)', rem: 'var(--accent-blue)',
  light: 'var(--color-green)', awake: 'var(--color-orange, #d9730d)',
  sleeping: 'var(--text-muted)', out_of_bed: 'var(--color-red)',
  unknown: 'var(--text-faint)',
};
const STAGE_LABELS = {
  deep: 'Глубокий', rem: 'REM', light: 'Лёгкий',
  awake: 'Бодрствование', sleeping: 'Сон', out_of_bed: 'Встал',
};

export async function renderSleepPane(el) {
  const today = new Date();
  const from = new Date(today); from.setDate(from.getDate() - 30);
  const toStr = fmt(today), fromStr = fmt(from);

  let sessions = [], stats = { avg_duration_minutes: 0, avg_deep_minutes: 0, avg_rem_minutes: 0, total_sessions: 0 };
  try {
    [sessions, stats] = await Promise.all([
      invoke('get_sleep_sessions', { from: fromStr, to: toStr }),
      invoke('get_sleep_stats', { days: 30 }),
    ]);
  } catch(e) { console.error('sleep load:', e); }

  // Check Health Connect permission state on Android — drives whether we show
  // "Разрешить" or "Импорт" as the primary action
  let hcGranted = false;
  if (IS_MOBILE) {
    try { hcGranted = !!(await invoke('health_has_permissions')); } catch(_) {}
  }

  const avgH = Math.floor(stats.avg_duration_minutes / 60);
  const avgM = Math.round(stats.avg_duration_minutes % 60);

  const importBtnHtml = hcGranted
    ? `<button class="btn-smallall mobile-only" id="sleep-import-btn">📱 Импорт из Samsung Health</button>`
    : `<button class="btn-smallall mobile-only" id="sleep-grant-btn">🔓 Разрешить доступ к Health Connect</button>`;

  el.innerHTML = `
    <div class="sleep-stats">
      <div class="sleep-stat"><div class="sleep-stat-value">${avgH}ч ${avgM}м</div><div class="sleep-stat-label">Среднее</div></div>
      <div class="sleep-stat"><div class="sleep-stat-value">${Math.round(stats.avg_deep_minutes)}м</div><div class="sleep-stat-label">Глубокий</div></div>
      <div class="sleep-stat"><div class="sleep-stat-value">${Math.round(stats.avg_rem_minutes)}м</div><div class="sleep-stat-label">REM</div></div>
      <div class="sleep-stat"><div class="sleep-stat-value">${stats.total_sessions}</div><div class="sleep-stat-label">Записей</div></div>
    </div>
    <div class="sleep-actions">
      <button class="btn-smallall" id="sleep-add-btn">+ Добавить</button>
      ${importBtnHtml}
    </div>
    <div class="sleep-list">${renderSessionList(sessions)}</div>`;

  el.querySelector('#sleep-add-btn')?.addEventListener('click', () => addManualSleep(el));
  el.querySelector('#sleep-import-btn')?.addEventListener('click', () => importFromHealthConnect(el));
  el.querySelector('#sleep-grant-btn')?.addEventListener('click', () => grantAndImport(el));

  // Lazy auto-import on enter (throttled inside autoImportHealth) — runs in
  // the background so the pane stays responsive
  if (IS_MOBILE && hcGranted) {
    autoImportHealth().then(ok => { if (ok) renderSleepPane(el); });
  }
}

function renderSessionList(sessions) {
  if (!sessions.length) return '<div class="uni-empty">Нет записей о сне</div>';
  return sessions.map(s => {
    const h = Math.floor(s.duration_minutes / 60), m = s.duration_minutes % 60;
    const stagesHtml = s.stages.length ? renderStagesBar(s.stages) : '';
    return `<div class="sleep-session">
      <div class="sleep-session-date">${s.date}</div>
      <div class="sleep-session-time">${shortTime(s.start_time)} — ${shortTime(s.end_time)}</div>
      <div class="sleep-session-dur">${h}ч ${m}м</div>
      <div class="sleep-session-source">${s.source === 'health_connect' ? '📱' : '✏️'}</div>
      ${stagesHtml}
    </div>`;
  }).join('');
}

function renderStagesBar(stages) {
  if (!stages.length) return '';
  const total = stages.reduce((a, s) => a + dur(s), 0);
  if (!total) return '';
  const bars = stages.map(s => {
    const pct = (dur(s) / total * 100).toFixed(1);
    const color = STAGE_COLORS[s.stage] || STAGE_COLORS.unknown;
    const label = STAGE_LABELS[s.stage] || s.stage;
    return `<div class="sleep-bar-seg" style="width:${pct}%;background:${color}" title="${label}: ${Math.round(dur(s))}м"></div>`;
  }).join('');
  return `<div class="sleep-bar">${bars}</div>`;
}

function dur(stage) {
  const s = new Date(stage.start_time), e = new Date(stage.end_time);
  return (e - s) / 60000;
}

function shortTime(iso) {
  try {
    const d = new Date(iso);
    return `${String(d.getHours()).padStart(2,'0')}:${String(d.getMinutes()).padStart(2,'0')}`;
  } catch { return iso?.slice(11, 16) || ''; }
}

function fmt(d) { return d.toISOString().slice(0, 10); }

async function addManualSleep(el) {
  const date = prompt('Дата (YYYY-MM-DD):', fmt(new Date()));
  if (!date) return;
  const start = prompt('Начало сна (HH:MM):', '23:00');
  if (!start) return;
  const end = prompt('Конец сна (HH:MM):', '07:00');
  if (!end) return;
  const startDt = new Date(`${date}T${start}:00`);
  let endDt = new Date(`${date}T${end}:00`);
  if (endDt <= startDt) endDt.setDate(endDt.getDate() + 1);
  const durMin = Math.round((endDt - startDt) / 60000);
  try {
    await invoke('add_sleep_session', { session: {
      date, start_time: startDt.toISOString(), end_time: endDt.toISOString(),
      duration_minutes: durMin, stages: [], source: 'manual', quality_score: null, notes: '',
    }});
    renderSleepPane(el);
  } catch(e) { alert('Ошибка: ' + e); }
}

async function importFromHealthConnect(el) {
  try {
    await invoke('import_health_connect_sleep');
    renderSleepPane(el);
  } catch(e) { alert(e); }
}

async function grantAndImport(el) {
  try {
    const granted = await invoke('health_request_permissions');
    if (granted) {
      await autoImportHealth({ force: true });
    }
    renderSleepPane(el);
  } catch(e) { alert(e); }
}
