// timeline-dash.js — Timeline dashboard with stats, goals, and trends
import { invoke } from './state.js';

export async function renderTimelineDash(paneEl) {
  const today = new Date();
  const todayStr = today.toISOString().slice(0, 10);

  // Date ranges
  const weekStart = new Date(today);
  const dow = weekStart.getDay() || 7;
  weekStart.setDate(weekStart.getDate() - dow + 1);
  const weekStartStr = weekStart.toISOString().slice(0, 10);

  const [dayStats, rangeStats, goals] = await Promise.all([
    invoke('get_timeline_day_stats', { date: todayStr }).catch(() => ({ per_type: [], total_minutes: 0 })),
    invoke('get_timeline_range_stats', { startDate: weekStartStr, endDate: todayStr }).catch(() => ({ current: [], previous_totals: {}, days: 7 })),
    invoke('get_timeline_goals').catch(() => []),
  ]);

  // Today cards
  const todayCards = dayStats.per_type.filter(t => t.minutes > 0).map(t =>
    `<div class="tl-dash-card">
      <div class="tl-dash-card-label">${t.icon} ${t.name}</div>
      <div class="tl-dash-card-value" style="color:${t.color}">${formatMinutes(t.minutes)}</div>
    </div>`
  ).join('');

  // Weekly averages with comparison
  const weekCards = rangeStats.current?.map(t => {
    const avg = rangeStats.days > 0 ? Math.round(t.total_minutes / rangeStats.days) : 0;
    const prevTotal = rangeStats.previous_totals?.[String(t.id)] || 0;
    const prevAvg = rangeStats.days > 0 ? Math.round(prevTotal / rangeStats.days) : 0;
    const delta = avg - prevAvg;
    const deltaClass = delta > 0 ? 'positive' : delta < 0 ? 'negative' : '';
    const deltaStr = delta !== 0 ? `<span class="tl-dash-card-delta ${deltaClass}">${delta > 0 ? '+' : ''}${delta} мин</span>` : '';
    return `<div class="tl-dash-card">
      <div class="tl-dash-card-label">${t.icon} ${t.name} (сред./день)</div>
      <div class="tl-dash-card-value" style="color:${t.color}">${formatMinutes(avg)}${deltaStr}</div>
    </div>`;
  }).join('') || '';

  // Goals progress
  const goalCards = goals.filter(g => g.active).map(g => {
    const actual = dayStats.per_type.find(t => t.id === g.type_id)?.minutes || 0;
    const target = g.target_minutes;
    const pct = target > 0 ? Math.min(100, Math.round(actual / target * 100)) : 0;
    const met = g.operator === '>=' ? actual >= target : g.operator === '<=' ? actual <= target : actual === target;
    const barColor = met ? 'var(--color-green)' : 'var(--color-red)';
    const opLabel = g.operator === '>=' ? '≥' : g.operator === '<=' ? '≤' : '=';
    return `<div class="tl-dash-card">
      <div class="tl-dash-card-label">${g.type_icon} ${g.type_name}: ${opLabel} ${formatMinutes(target)}</div>
      <div class="tl-dash-card-value" style="color:${met ? 'var(--color-green)' : 'var(--color-red)'}">${formatMinutes(actual)}</div>
      <div class="tl-bar-wrap"><div class="tl-bar-fill" style="width:${pct}%;background:${barColor};"></div></div>
    </div>`;
  }).join('');

  const tracked = dayStats.total_minutes;
  const untracked = 1440 - tracked;

  paneEl.innerHTML = `
    <div style="margin-bottom:16px;">
      <div style="font-size:13px;font-weight:600;margin-bottom:8px;color:var(--text-primary);">Сегодня — ${formatMinutes(tracked)} из 24ч размечено</div>
      <div class="tl-dash-grid">${todayCards || '<div style="color:var(--text-faint);font-size:13px;">Нет данных на сегодня</div>'}</div>
    </div>
    ${weekCards ? `<div style="margin-bottom:16px;">
      <div style="font-size:13px;font-weight:600;margin-bottom:8px;color:var(--text-primary);">Среднее за неделю (vs прошлая)</div>
      <div class="tl-dash-grid">${weekCards}</div>
    </div>` : ''}
    ${goalCards ? `<div style="margin-bottom:16px;">
      <div style="font-size:13px;font-weight:600;margin-bottom:8px;color:var(--text-primary);">Цели на сегодня</div>
      <div class="tl-dash-grid">${goalCards}</div>
    </div>` : '<div style="color:var(--text-faint);font-size:13px;padding:12px 0;">Нет целей. Добавьте их во вкладке «Цели».</div>'}
    <div style="margin-top:8px;">
      <button id="tl-manage-types" style="padding:6px 12px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-muted);cursor:pointer;font-size:12px;">⚙ Типы активности</button>
      <button id="tl-sync-afk" style="padding:6px 12px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-muted);cursor:pointer;font-size:12px;margin-left:6px;">🔄 Sync АФК</button>
    </div>`;

  paneEl.querySelector('#tl-manage-types')?.addEventListener('click', async () => {
    const { showTypesModal } = await import('./timeline-types.js');
    await showTypesModal();
    renderTimelineDash(paneEl);
  });

  paneEl.querySelector('#tl-sync-afk')?.addEventListener('click', async () => {
    const count = await invoke('sync_afk_blocks', { date: todayStr }).catch(() => 0);
    alert(`Синхронизировано АФК блоков: ${count}`);
    renderTimelineDash(paneEl);
  });
}

function formatMinutes(m) {
  if (m < 60) return `${m} мин`;
  const h = Math.floor(m / 60);
  const min = m % 60;
  return min > 0 ? `${h}ч ${min}м` : `${h}ч`;
}
