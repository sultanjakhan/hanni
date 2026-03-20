// ── tab-data-health.js — Health tab (metrics, habits) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Health ──
export async function loadHealth() {
  const el = document.getElementById('health-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'health', {
    title: 'Health',
    subtitle: 'Здоровье и привычки',
    icon: '❤️',
    renderDash: async (paneEl) => {
      const today = await invoke('get_health_today').catch(() => ({}));
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${today.sleep ? today.sleep + 'ч' : '—'}</div><div class="uni-dash-label">Сон</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${today.water || '—'}</div><div class="uni-dash-label">Вода (стаканов)</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${today.mood ? today.mood + '/5' : '—'}</div><div class="uni-dash-label">Настроение</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${today.weight ? today.weight + 'кг' : '—'}</div><div class="uni-dash-label">Вес</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      try {
        const today = await invoke('get_health_today').catch(() => ({}));
        const habits = await invoke('get_habits_today').catch(() => []);
        renderHealth(paneEl, today, habits);
      } catch (e) {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить данные здоровья</div>';
      }
    },
  });
}

function renderHealth(el, today, habits) {
  const sleep = today.sleep || null;
  const water = today.water || null;
  const mood = today.mood || null;
  const weight = today.weight || null;

  function metricClass(type, val) {
    if (val === null) return '';
    if (type === 'sleep') return val >= 7 ? 'good' : val >= 5 ? 'warning' : 'bad';
    if (type === 'water') return val >= 8 ? 'good' : val >= 4 ? 'warning' : 'bad';
    if (type === 'mood') return val >= 4 ? 'good' : val >= 3 ? 'warning' : 'bad';
    return '';
  }

  el.innerHTML = `
    <div class="health-metrics" style="margin-bottom:var(--space-4);">
      <div class="health-metric ${metricClass('sleep', sleep)}" data-type="sleep">
        <div class="health-metric-icon">&#x1F634;</div>
        <div class="health-metric-value">${sleep !== null ? sleep + 'ч' : '\u2014'}</div>
        <div class="health-metric-label">Сон</div>
      </div>
      <div class="health-metric ${metricClass('water', water)}" data-type="water">
        <div class="health-metric-icon">&#x1F4A7;</div>
        <div class="health-metric-value">${water !== null ? water : '\u2014'}</div>
        <div class="health-metric-label">Вода (стаканов)</div>
      </div>
      <div class="health-metric ${metricClass('mood', mood)}" data-type="mood">
        <div class="health-metric-icon">${mood >= 4 ? '&#x1F60A;' : mood >= 3 ? '&#x1F610;' : mood ? '&#x1F641;' : '&#x1F636;'}</div>
        <div class="health-metric-value">${mood !== null ? mood + '/5' : '\u2014'}</div>
        <div class="health-metric-label">Настроение</div>
      </div>
      <div class="health-metric" data-type="weight">
        <div class="health-metric-icon">&#x2696;</div>
        <div class="health-metric-value">${weight !== null ? weight + 'кг' : '\u2014'}</div>
        <div class="health-metric-label">Вес</div>
      </div>
    </div>
    <div id="habits-dbv"></div>`;

  const dbvEl = el.querySelector('#habits-dbv');
  const dbv = new DatabaseView(dbvEl, {
    tabId: 'health',
    recordTable: 'habits',
    records: habits,
    availableViews: ['table', 'list'],
    fixedColumns: [
      { key: 'done', label: '', render: r => `<div class="habit-check${r.completed ? ' checked' : ''}" style="cursor:pointer;" data-hid="${r.id}">${r.completed ? '&#10003;' : ''}</div>` },
      { key: 'name', label: 'Привычка', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'frequency', label: 'Частота', render: r => `<span class="badge badge-gray">${r.frequency || 'daily'}</span>` },
      { key: 'streak', label: 'Серия', render: r => r.streak > 0 ? `<span class="badge badge-green">${r.streak} дн.</span>` : '<span style="color:var(--text-faint);font-size:12px;">—</span>' },
    ],
    idField: 'id',
    addButton: '+ Привычка',
    onQuickAdd: async (name) => {
      await invoke('create_habit', { name, icon: '', frequency: 'daily' });
      loadHealth();
    },
    reloadFn: () => loadHealth(),
  });
  dbv.render();

  // Click on metric to log
  el.querySelectorAll('.health-metric').forEach(m => {
    m.addEventListener('click', () => {
      const type = m.dataset.type;
      const labels = { sleep: 'Сон (часы)', water: 'Вода (стаканы)', mood: 'Настроение (1-5)', weight: 'Вес (кг)' };
      const val = prompt(labels[type] + ':');
      if (val) {
        invoke('log_health', { healthType: type, value: parseFloat(val), notes: null }).then(() => loadHealth()).catch(e => alert(e));
      }
    });
  });

  // Delegate habit check clicks
  el.addEventListener('click', async (e) => {
    const check = e.target.closest('[data-hid]');
    if (!check) return;
    await invoke('check_habit', { habitId: parseInt(check.dataset.hid), date: null }).catch(() => {});
    loadHealth();
  });
}
