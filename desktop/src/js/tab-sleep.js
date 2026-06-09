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

  // Lazy auto-import on enter — runs in the background so the pane stays
  // responsive. We force-bypass the 60-second throttle only if last sync
  // is older than 2 min; an unconditional force blocked Tauri's async
  // runtime for the full HC read window (~30+ s on Samsung), starving the
  // automation server. The 2-min staleness check still catches the
  // common morning case where Samsung Health writes the night's sleep to
  // HC after the user wakes up.
  if (IS_MOBILE && hcGranted) {
    const lastSync = +(localStorage.getItem('hc_last_sync') || 0);
    const stale = (Date.now() - lastSync) > 2 * 60 * 1000;
    autoImportHealth({ force: stale }).then(ok => { if (ok) renderSleepPane(el); });
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

function toMinutes(s) {
  if (!s) return 0;
  // "HH:MM" — Health Connect/local format
  if (/^\d{2}:\d{2}$/.test(s)) {
    const [h, m] = s.split(':').map(Number);
    return h * 60 + m;
  }
  // ISO timestamp — manual entry uses toISOString()
  const d = new Date(s);
  return isNaN(d) ? 0 : d.getHours() * 60 + d.getMinutes();
}

function dur(stage) {
  let m = toMinutes(stage.end_time) - toMinutes(stage.start_time);
  if (m < 0) m += 24 * 60; // crossed midnight
  return m;
}

function shortTime(s) {
  if (!s) return '';
  // Already "HH:MM" — keep as is.
  if (/^\d{2}:\d{2}$/.test(s)) return s;
  const d = new Date(s);
  if (isNaN(d)) return s.slice(11, 16) || s;
  return `${String(d.getHours()).padStart(2,'0')}:${String(d.getMinutes()).padStart(2,'0')}`;
}

function fmt(d) { return d.toISOString().slice(0, 10); }

async function addManualSleep(el) {
  // One modal with native date/time inputs instead of three chained prompts.
  const { promptModal } = await import('./prompt-modal.js');
  const res = await promptModal({
    title: 'Добавить сон вручную',
    fields: [
      { key: 'date', label: 'Дата', type: 'date', value: fmt(new Date()) },
      { key: 'start', label: 'Начало сна', type: 'time', value: '23:00' },
      { key: 'end', label: 'Конец сна', type: 'time', value: '07:00' },
    ],
  });
  if (!res || !res.date || !res.start || !res.end) return;
  const { date, start, end } = res;
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
