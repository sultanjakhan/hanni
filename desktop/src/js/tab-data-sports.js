// ── tab-data-sports.js — Sports tab (workouts, martial arts, stats, HomeFit) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';
import { loadHomeFit } from './integration-homefit.js';

// ── Sports ──
export async function loadSports(subTab) {
  const el = document.getElementById('sports-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'sports', {
    title: 'Sports',
    subtitle: 'Тренировки и физическая активность',
    icon: '💪',
    renderDash: async (paneEl) => {
      const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${stats.count || 0}</div><div class="uni-dash-label">Тренировок</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${stats.total_minutes || 0}м</div><div class="uni-dash-label">Общее время</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${stats.total_calories || 0}</div><div class="uni-dash-label">Калории</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._sportsInner || 'workouts';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'workouts' ? ' active' : ''}" data-inner="workouts">Тренировки</button>
          <button class="pill${activeInner === 'martial_arts' ? ' active' : ''}" data-inner="martial_arts">Единоборства</button>
          <button class="pill${activeInner === 'stats' ? ' active' : ''}" data-inner="stats">Статистика</button>
          <button class="pill${activeInner === 'homefit' ? ' active' : ''}" data-inner="homefit">🏋️ HomeFit</button>
        </div>
        <div id="sports-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#sports-inner-content');
      if (activeInner === 'martial_arts') await loadMartialArts(innerEl);
      else if (activeInner === 'stats') await loadSportsStats(innerEl);
      else if (activeInner === 'homefit') await loadHomeFit(innerEl);
      else {
        const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
        const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
        renderSports(innerEl, workouts || [], stats);
      }
      paneEl.querySelectorAll('[data-inner]').forEach(btn => {
        btn.addEventListener('click', () => { S._sportsInner = btn.dataset.inner; loadSports(); });
      });
    },
  });
}

async function loadMartialArts(el) {
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const ma = (workouts || []).filter(w => w.type === 'martial_arts');
    const dbv = new DatabaseView(el, {
      tabId: 'sports', recordTable: 'workouts', records: ma,
      availableViews: ['table', 'list'],
      fixedColumns: [
        { key: 'date', label: 'Дата', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.date || '—'}</span>` },
        { key: 'title', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.title || 'Единоборства')}</span>` },
        { key: 'duration_minutes', label: 'Время', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.duration_minutes || 0} мин</span>` },
        { key: 'calories', label: 'Калории', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories || '—'}</span>` },
      ],
      idField: 'id', addButton: '+ Тренировка',
      onQuickAdd: async (title) => { await invoke('create_workout', { workoutType: 'martial_arts', title, durationMinutes: 60, calories: null, notes: '' }); loadSports(); },
      reloadFn: () => loadSports(),
      onDelete: async (id) => { await invoke('delete_workout', { id }); },
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);">Error: ${e}</div>`; }
}

async function loadSportsStats(el) {
  try {
    const [stats, workouts] = await Promise.all([
      invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 })),
      invoke('get_workouts', { dateRange: null }).catch(() => []),
    ]);
    const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };
    const byType = {};
    for (const w of (workouts || [])) byType[w.type] = (byType[w.type] || 0) + 1;
    el.innerHTML = `
      <div class="uni-dash-grid">
        <div class="uni-dash-card blue"><div class="uni-dash-value">${stats.count || 0}</div><div class="uni-dash-label">Тренировок</div></div>
        <div class="uni-dash-card green"><div class="uni-dash-value">${stats.total_minutes || 0}м</div><div class="uni-dash-label">Общее время</div></div>
        <div class="uni-dash-card yellow"><div class="uni-dash-value">${stats.total_calories || 0}</div><div class="uni-dash-label">Калории</div></div>
      </div>
      <div style="margin-top:16px;">
        <h3 style="font-size:14px;color:var(--text-muted);margin-bottom:8px;">По типам</h3>
        ${Object.entries(byType).map(([t, c]) => `<div style="display:flex;justify-content:space-between;padding:4px 0;font-size:14px;color:var(--text-secondary);border-bottom:1px solid var(--bg-hover);"><span>${typeLabels[t] || t}</span><span style="color:var(--text-muted);">${c}</span></div>`).join('') || '<div style="color:var(--text-faint);font-size:14px;">No data yet</div>'}
      </div>`;
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);">Error: ${e}</div>`; }
}

function renderSports(el, workouts, stats) {
  const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };
  const dbv = new DatabaseView(el, {
    tabId: 'sports', recordTable: 'workouts', records: workouts,
    fixedColumns: [
      { key: 'date', label: 'Дата', render: r => `<span style="font-size:12px;color:var(--text-muted);font-variant-numeric:tabular-nums;">${r.date || '—'}</span>` },
      { key: 'title', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.title || typeLabels[r.type] || r.type)}</span>` },
      { key: 'type', label: 'Тип', render: r => `<span class="badge badge-purple">${typeLabels[r.type] || r.type}</span>` },
      { key: 'duration_minutes', label: 'Время', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.duration_minutes || 0} мин</span>` },
      { key: 'calories', label: 'Калории', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories || '—'}</span>` },
    ],
    idField: 'id', availableViews: ['table', 'list'], defaultView: 'table',
    addButton: '+ Тренировка', onAdd: () => showAddWorkoutModal(),
    onQuickAdd: async (title) => { await invoke('create_workout', { workoutType: 'gym', title, durationMinutes: 60, calories: null, notes: '' }); loadSports(); },
    reloadFn: () => loadSports(),
    onDelete: async (id) => { await invoke('delete_workout', { id }); },
  });
  dbv.render();
}

function showAddWorkoutModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новая тренировка</div>
    <div class="form-row">
      <select class="form-select" id="workout-type"><option value="gym">Зал</option><option value="cardio">Кардио</option><option value="yoga">Йога</option><option value="swimming">Плавание</option><option value="martial_arts">Единоборства</option><option value="other">Другое</option></select>
      <input class="form-input" id="workout-title" placeholder="Название">
    </div>
    <div class="form-row">
      <input class="form-input" id="workout-duration" type="number" value="60" placeholder="Минуты" style="max-width:100px;"><span class="form-hint">мин</span>
      <input class="form-input" id="workout-calories" type="number" placeholder="Калории" style="max-width:100px;"><span class="form-hint">ккал</span>
    </div>
    <textarea class="form-textarea" id="workout-notes" placeholder="Заметки" rows="2"></textarea>
    <div class="modal-actions"><button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button><button class="btn-primary" id="workout-save">Сохранить</button></div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('workout-save')?.addEventListener('click', async () => {
    try {
      await invoke('create_workout', {
        workoutType: document.getElementById('workout-type')?.value || 'other',
        title: document.getElementById('workout-title')?.value?.trim() || '',
        durationMinutes: parseInt(document.getElementById('workout-duration')?.value || '60'),
        calories: parseInt(document.getElementById('workout-calories')?.value || '0') || null,
        notes: document.getElementById('workout-notes')?.value || '',
      });
      overlay.remove();
      loadSports();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}
