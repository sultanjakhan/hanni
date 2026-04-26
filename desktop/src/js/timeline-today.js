// timeline-today.js — Today view: planned tasks (Calendar/Schedule/Notes) with start/stop tracking
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

let liveTimer = null;
let activeBlockStart = null;

export async function renderTimelineToday(paneEl) {
  stopLiveTimer();
  const today = localDate();
  const items = await invoke('get_today_planned', { date: today }).catch(() => []);

  const active = items.filter(i => i.is_active);
  const planned = items.filter(i => !i.is_active && !i.completed && i.status_extra !== 'skipped');
  const finished = items.filter(i => !i.is_active && (i.completed || i.status_extra === 'skipped'));

  paneEl.innerHTML = `
    <div class="tt-wrap">
      ${renderSection('Сейчас', active, 'active')}
      ${renderSection('Дальше', planned, 'planned')}
      ${renderSection('Завершено', finished, 'done')}
    </div>`;

  paneEl.querySelectorAll('[data-action="start"]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const sourceType = btn.dataset.sourceType;
      const sourceId = parseInt(btn.dataset.sourceId);
      btn.disabled = true;
      try {
        await invoke('start_task_block', { sourceType, sourceId });
      } catch (e) { alert('Не удалось стартовать: ' + e); }
      await renderTimelineToday(paneEl);
    });
  });

  paneEl.querySelectorAll('[data-action="complete"]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const blockId = parseInt(btn.dataset.blockId);
      btn.disabled = true;
      try {
        await invoke('complete_task_block', { blockId });
      } catch (e) { alert('Не удалось завершить: ' + e); }
      await renderTimelineToday(paneEl);
    });
  });

  if (active.length > 0 && active[0].actual_start) {
    activeBlockStart = parseTime(active[0].actual_start);
    startLiveTimer(paneEl);
  }
}

function renderSection(title, items, kind) {
  if (!items.length) return '';
  const cards = items.map(it => renderCard(it, kind)).join('');
  return `
    <div class="tt-section">
      <div class="tt-section-title">${title} <span class="tt-section-count">${items.length}</span></div>
      <div class="tt-cards">${cards}</div>
    </div>`;
}

function renderCard(it, kind) {
  const sourceLabel = { event: 'Событие', schedule: 'Расписание', note: 'Задача' }[it.source_type] || '';
  const color = it.color || '#3b82f6';
  const planned = it.planned_time ? `план: ${escapeHtml(it.planned_time)}` : 'без времени';

  let timeInfo = '';
  if (it.is_active) {
    timeInfo = `<span class="tt-live" data-start="${escapeHtml(it.actual_start)}">${escapeHtml(it.actual_start)} · идёт…</span>`;
  } else if (it.actual_start && it.actual_end) {
    timeInfo = `<span class="tt-actual">${escapeHtml(it.actual_start)}–${escapeHtml(it.actual_end)} (${it.actual_duration} мин)</span>`;
  }

  let actions = '';
  if (kind === 'active') {
    actions = `<button class="tt-btn tt-btn-stop" data-action="complete" data-block-id="${it.block_id}">■ Завершить</button>`;
  } else if (kind === 'planned') {
    actions = `<button class="tt-btn tt-btn-start" data-action="start" data-source-type="${it.source_type}" data-source-id="${it.source_id}">▶ Старт</button>`;
  }

  const statusBadge = it.status_extra === 'skipped' ? '<span class="tt-badge tt-badge-skip">пропущено</span>' :
                      it.completed ? '<span class="tt-badge tt-badge-done">готово</span>' : '';

  return `
    <div class="tt-card" style="--card-accent:${color}">
      <div class="tt-card-bar"></div>
      <div class="tt-card-body">
        <div class="tt-card-head">
          <div class="tt-card-title">${escapeHtml(it.title || '')}</div>
          ${statusBadge}
        </div>
        <div class="tt-card-meta">
          <span class="tt-source">${sourceLabel}</span>
          <span class="tt-planned">${planned}</span>
          ${timeInfo}
        </div>
      </div>
      <div class="tt-card-actions">${actions}</div>
    </div>`;
}

function startLiveTimer(paneEl) {
  liveTimer = setInterval(() => {
    const liveEl = paneEl.querySelector('.tt-live');
    if (!liveEl || activeBlockStart == null) return;
    const startMin = activeBlockStart;
    const now = new Date();
    const nowMin = now.getHours() * 60 + now.getMinutes();
    const delta = nowMin >= startMin ? nowMin - startMin : (24 * 60 - startMin) + nowMin;
    const start = liveEl.dataset.start || '';
    liveEl.textContent = `${start} · ${delta} мин`;
  }, 30_000);
}

function stopLiveTimer() {
  if (liveTimer) { clearInterval(liveTimer); liveTimer = null; }
  activeBlockStart = null;
}

function parseTime(hm) {
  const [h, m] = (hm || '00:00').split(':').map(Number);
  return (h || 0) * 60 + (m || 0);
}

function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}
