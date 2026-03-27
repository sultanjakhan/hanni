// ── js/tab-focus.js — Focus tab (unified layout) + Focus floating widget ──

import { S, invoke, listen, emit, tabLoaders } from './state.js';
import { escapeHtml, renderPageHeader, setupPageHeaderControls, skeletonPage, loadTabBlockEditor } from './utils.js';

// ── Focus (unified layout) ──

async function loadFocus(subTab) {
  const el = document.getElementById('focus-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'focus', {
    title: 'Focus',
    subtitle: 'Глубокая работа',
    icon: '🎯',
    renderDash: async (paneEl) => {
      // Dashboard: today's stats
      const log = await invoke('get_activity_log', { date: null }).catch(() => []);
      const current = await invoke('get_current_activity').catch(() => null);
      const totalMin = log.reduce((sum, item) => {
        const match = (item.duration || '').match(/(\d+)ч\s*(\d+)м|(\d+)м/);
        if (match) return sum + (parseInt(match[1] || 0) * 60) + parseInt(match[2] || match[3] || 0);
        return sum;
      }, 0);
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${Math.floor(totalMin / 60)}ч ${totalMin % 60}м</div><div class="uni-dash-label">Всего сегодня</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${log.length}</div><div class="uni-dash-label">Сессий</div></div>
          <div class="uni-dash-card ${current ? 'yellow' : 'red'}"><div class="uni-dash-value">${current ? '🔥' : '—'}</div><div class="uni-dash-label">${current ? escapeHtml(current.title) : 'Нет активности'}</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      // Inner pill navigation: Текущая / История
      const views = [
        { id: 'current', label: 'Текущая' },
        { id: 'history', label: 'История' },
      ];
      const activeView = S._focusInner || 'current';
      paneEl.innerHTML = `
        <div class="dev-filters">
          ${views.map(v => `<button class="dev-filter-btn${v.id === activeView ? ' active' : ''}" data-focusview="${v.id}">${v.label}</button>`).join('')}
        </div>
        <div id="focus-inner-content"></div>`;

      paneEl.querySelectorAll('[data-focusview]').forEach(btn => {
        btn.addEventListener('click', () => {
          S._focusInner = btn.dataset.focusview;
          loadFocus();
        });
      });

      const innerEl = paneEl.querySelector('#focus-inner-content');
      if (activeView === 'history') await renderFocusHistory(innerEl);
      else await renderFocusCurrent(innerEl);
    },
  });
}

async function renderFocusCurrent(el) {
  el.innerHTML = skeletonPage();
  try {
    const current = await invoke('get_current_activity').catch(() => null);
    const log = await invoke('get_activity_log', { date: null }).catch(() => []);
    const focusStatus = await invoke('get_focus_status').catch(() => ({ active: false }));

    let timerHtml = '';
    if (current) {
      // Active session — show ring timer
      timerHtml = `<div class="focus-current">
        <div class="focus-ring-container">
          <svg class="focus-ring" viewBox="0 0 120 120">
            <circle class="focus-ring-bg" cx="60" cy="60" r="54" />
            <circle class="focus-ring-progress" id="focus-ring-progress" cx="60" cy="60" r="54" />
          </svg>
          <div class="focus-ring-inner">
            <div class="focus-current-timer" id="focus-timer">00:00</div>
            <div class="focus-current-activity">${escapeHtml(current.title)}</div>
          </div>
        </div>
        ${focusStatus.active ? '<div class="focus-blocking-badge">🛡 Блокировка активна</div>' : ''}
        <div class="focus-actions">
          <button class="btn-danger" id="stop-activity-btn">Завершить</button>
        </div>
      </div>`;
    } else if (S.pomodoroState.active) {
      // Pomodoro break/work countdown
      const modeLabel = S.pomodoroState.mode === 'work' ? 'Работа' : 'Перерыв';
      timerHtml = `<div class="focus-current">
        <div class="focus-ring-container">
          <svg class="focus-ring" viewBox="0 0 120 120">
            <circle class="focus-ring-bg" cx="60" cy="60" r="54" />
            <circle class="focus-ring-progress ${S.pomodoroState.mode === 'break' ? 'break' : ''}" id="focus-ring-progress" cx="60" cy="60" r="54" />
          </svg>
          <div class="focus-ring-inner">
            <div class="focus-current-timer" id="focus-timer">00:00</div>
            <div class="focus-current-activity">${modeLabel}</div>
          </div>
        </div>
        <div class="focus-actions">
          <button class="btn-secondary" id="pomo-skip-btn">Пропустить</button>
          <button class="btn-danger" id="pomo-stop-btn">Стоп</button>
        </div>
      </div>`;
    } else {
      // Idle — show start form + pomodoro
      timerHtml = `<div class="focus-current">
        <div class="focus-ring-container idle">
          <svg class="focus-ring" viewBox="0 0 120 120">
            <circle class="focus-ring-bg" cx="60" cy="60" r="54" />
          </svg>
          <div class="focus-ring-inner">
            <div class="focus-current-timer" style="color:var(--text-muted);">00:00</div>
            <div class="focus-current-activity" style="color:var(--text-faint);">Готов</div>
          </div>
        </div>
        <input id="activity-title" class="form-input focus-title-input" placeholder="Название активности..." autocomplete="off">
        <div class="focus-presets" id="activity-presets">
          <button class="focus-preset" data-category="work">Работа</button>
          <button class="focus-preset" data-category="study">Учёба</button>
          <button class="focus-preset" data-category="sport">Спорт</button>
          <button class="focus-preset" data-category="rest">Отдых</button>
          <button class="focus-preset" data-category="hobby">Хобби</button>
          <button class="focus-preset" data-category="other">Другое</button>
        </div>
        <div class="focus-start-row">
          <label class="focus-check-label"><input type="checkbox" id="focus-block-check"> Блокировать отвлечения</label>
        </div>
        <div class="focus-actions">
          <button class="btn-primary" id="start-activity-btn">Начать</button>
          <button class="btn-secondary" id="start-pomo-btn" title="Pomodoro 25/5">🍅 Помодоро</button>
        </div>
      </div>`;
    }

    // Today's stats summary
    const totalMin = log.reduce((sum, item) => {
      const match = (item.duration || '').match(/(\d+)ч\s*(\d+)м|(\d+)м/);
      if (match) return sum + (parseInt(match[1] || 0) * 60) + parseInt(match[2] || match[3] || 0);
      return sum;
    }, 0);
    const categories = {};
    log.forEach(item => { categories[item.category || 'other'] = (categories[item.category || 'other'] || 0) + 1; });
    const catLabels = { work: 'Работа', study: 'Учёба', sport: 'Спорт', rest: 'Отдых', hobby: 'Хобби', other: 'Другое' };

    const statsHtml = `<div class="focus-today-stats">
      <div class="focus-stat"><div class="focus-stat-value">${Math.floor(totalMin / 60)}ч ${totalMin % 60}м</div><div class="focus-stat-label">Всего сегодня</div></div>
      <div class="focus-stat"><div class="focus-stat-value">${log.length}</div><div class="focus-stat-label">Сессий</div></div>
      ${Object.entries(categories).slice(0, 3).map(([cat, count]) =>
        `<div class="focus-stat"><div class="focus-stat-value">${count}</div><div class="focus-stat-label">${catLabels[cat] || cat}</div></div>`
      ).join('')}
    </div>`;

    const logHtml = log.length > 0 ? `
      <div class="focus-log-header">Сегодня</div>
      <div class="focus-log">
        ${log.map(item => `<div class="focus-log-item">
          <span class="focus-log-time">${item.time || ''}</span>
          <span class="focus-log-title">${escapeHtml(item.title)}</span>
          <span class="focus-log-category">${item.category || ''}</span>
          <span class="focus-log-duration">${item.duration || ''}</span>
        </div>`).join('')}
      </div>` : '';

    el.innerHTML = `<div class="page-content">${timerHtml}${statsHtml}${logHtml}</div>`;

    // Bind events
    let selectedCategory = 'other';
    document.querySelectorAll('#activity-presets .focus-preset')?.forEach(btn => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('#activity-presets .focus-preset').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        selectedCategory = btn.dataset.category;
        const titleInput = document.getElementById('activity-title');
        if (titleInput && !titleInput.value) titleInput.value = btn.textContent;
      });
    });

    document.getElementById('start-activity-btn')?.addEventListener('click', async () => {
      const title = document.getElementById('activity-title')?.value?.trim() || selectedCategory;
      const focusMode = document.getElementById('focus-block-check')?.checked || false;
      try {
        await invoke('start_activity', { title, category: selectedCategory, focusMode, duration: null, apps: null, sites: null });
        loadFocus('Current');
      } catch (err) { alert('Ошибка: ' + err); }
    });

    document.getElementById('stop-activity-btn')?.addEventListener('click', async () => {
      try { await invoke('stop_activity'); loadFocus('Current'); }
      catch (err) { alert('Ошибка: ' + err); }
    });

    // Pomodoro buttons
    document.getElementById('start-pomo-btn')?.addEventListener('click', () => {
      const title = document.getElementById('activity-title')?.value?.trim() || 'Помодоро';
      const focusMode = document.getElementById('focus-block-check')?.checked || false;
      startPomodoro(title, selectedCategory, focusMode);
    });

    document.getElementById('pomo-skip-btn')?.addEventListener('click', () => {
      if (S.pomodoroState.mode === 'work') {
        invoke('stop_activity').catch(() => {});
        S.pomodoroState.mode = 'break';
        S.pomodoroState.startedAt = Date.now();
        S.pomodoroState.totalSec = S.pomodoroState.breakMin * 60;
      } else {
        S.pomodoroState.mode = 'work';
        S.pomodoroState.startedAt = Date.now();
        S.pomodoroState.totalSec = S.pomodoroState.workMin * 60;
        invoke('start_activity', { title: 'Помодоро', category: 'work', focusMode: false, duration: null, apps: null, sites: null }).catch(() => {});
      }
      loadFocus('Current');
    });

    document.getElementById('pomo-stop-btn')?.addEventListener('click', () => {
      S.pomodoroState.active = false;
      invoke('stop_activity').catch(() => {});
      loadFocus('Current');
    });

    // Update timer
    startFocusTimer(current);

  } catch (e) {
    tabLoaders.showStub?.('focus-content', '🎯', 'Фокус', 'Глубокая работа и трекинг активности');
  }
}

function startFocusTimer(current) {
  if (S.focusTimerInterval) clearInterval(S.focusTimerInterval);
  const circumference = 2 * Math.PI * 54;

  if (current && current.started_at) {
    const startedAt = new Date(current.started_at).getTime();
    const updateTimer = () => {
      const timerEl = document.getElementById('focus-timer');
      const ringEl = document.getElementById('focus-ring-progress');
      if (!timerEl || S.activeTab !== 'focus') { clearInterval(S.focusTimerInterval); return; }
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      const h = Math.floor(elapsed / 3600);
      const m = Math.floor((elapsed % 3600) / 60);
      const s = elapsed % 60;
      timerEl.textContent = h > 0 ? `${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}` : `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
      // Ring: animate based on minutes (full circle = 60 min)
      if (ringEl) {
        const progress = Math.min(1, elapsed / 3600);
        ringEl.style.strokeDasharray = circumference;
        ringEl.style.strokeDashoffset = circumference * (1 - progress);
      }
    };
    updateTimer();
    S.focusTimerInterval = setInterval(updateTimer, 1000);
  } else if (S.pomodoroState.active) {
    const updatePomo = () => {
      const timerEl = document.getElementById('focus-timer');
      const ringEl = document.getElementById('focus-ring-progress');
      if (!timerEl || S.activeTab !== 'focus') { clearInterval(S.focusTimerInterval); return; }
      const elapsed = Math.floor((Date.now() - S.pomodoroState.startedAt) / 1000);
      const remaining = Math.max(0, S.pomodoroState.totalSec - elapsed);
      const m = Math.floor(remaining / 60);
      const s = remaining % 60;
      timerEl.textContent = `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
      if (ringEl) {
        const progress = 1 - (remaining / S.pomodoroState.totalSec);
        ringEl.style.strokeDasharray = circumference;
        ringEl.style.strokeDashoffset = circumference * (1 - progress);
      }
      if (remaining <= 0) {
        clearInterval(S.focusTimerInterval);
        if (S.pomodoroState.mode === 'work') {
          invoke('stop_activity').catch(() => {});
          invoke('send_notification', { title: 'Помодоро', body: 'Время перерыва! 5 минут.' }).catch(() => {});
          S.pomodoroState.mode = 'break';
          S.pomodoroState.startedAt = Date.now();
          S.pomodoroState.totalSec = S.pomodoroState.breakMin * 60;
        } else {
          invoke('send_notification', { title: 'Помодоро', body: 'Перерыв окончен! Поехали.' }).catch(() => {});
          S.pomodoroState.mode = 'work';
          S.pomodoroState.startedAt = Date.now();
          S.pomodoroState.totalSec = S.pomodoroState.workMin * 60;
          invoke('start_activity', { title: 'Помодоро', category: 'work', focusMode: false, duration: null, apps: null, sites: null }).catch(() => {});
        }
        loadFocus('Current');
      }
    };
    updatePomo();
    S.focusTimerInterval = setInterval(updatePomo, 1000);
  }
}

async function startPomodoro(title, category, focusMode) {
  S.pomodoroState = { active: true, mode: 'work', workMin: 25, breakMin: 5, startedAt: Date.now(), totalSec: 25 * 60 };
  try {
    await invoke('start_activity', { title: title || 'Помодоро', category: category || 'work', focusMode, duration: 25, apps: null, sites: null });
  } catch (_) {}
  loadFocus('Current');
}

async function renderFocusHistory(el) {
  el.innerHTML = skeletonPage();
  try {
    const { DatabaseView } = await import('./db-view/db-view.js');
    const activities = await invoke('get_all_activities').catch(() => []);
    const catLabels = { work: 'Работа', study: 'Учёба', sport: 'Спорт', rest: 'Отдых', hobby: 'Хобби', other: 'Другое' };
    const catOptions = Object.entries(catLabels).map(([v, l]) => ({ value: v, label: l }));

    const formatDuration = (min) => {
      if (!min) return '—';
      return min >= 60 ? `${Math.floor(min/60)}ч ${min%60}м` : `${min}м`;
    };
    const formatTime = (str) => str && str.length >= 16 ? str.slice(11, 16) : '—';
    const formatDate = (str) => str ? str.slice(0, 10) : '—';

    el.innerHTML = '';
    const dbvEl = document.createElement('div');
    el.appendChild(dbvEl);
    const reload = () => renderFocusHistory(el);

    const dbv = new DatabaseView(dbvEl, {
      tabId: 'focus_activities',
      recordTable: 'activities',
      records: activities,
      idField: 'id',
      fixedColumns: [
        { key: 'title', label: 'Название', editable: true, editType: 'text',
          render: r => `<span class="data-table-title">${escapeHtml(r.title || '')}</span>` },
        { key: 'category', label: 'Категория', editable: true, editType: 'select', editOptions: catOptions,
          render: r => `<span class="badge badge-${r.category || 'other'}">${catLabels[r.category] || r.category || '—'}</span>` },
        { key: 'started_at', label: 'Начало',
          render: r => `<span>${formatDate(r.started_at)} ${formatTime(r.started_at)}</span>` },
        { key: 'duration_minutes', label: 'Длительность',
          render: r => `<span>${formatDuration(r.duration_minutes)}</span>` },
        { key: 'notes', label: 'Заметки', editable: true, editType: 'text',
          render: r => `<span>${escapeHtml(r.notes || '') || '—'}</span>` },
      ],
      onCellEdit: async (recordId, key, value) => {
        const params = { id: recordId, title: null, category: null, notes: null };
        params[key] = value || null;
        await invoke('update_activity', params);
        reload();
      },
      onDelete: async (id) => { await invoke('delete_activity', { id }); },
      reloadFn: reload,
      availableViews: ['table'],
    });
    await dbv.render();
  } catch (e) {
    tabLoaders.showStub?.('focus-content', '📊', 'История', 'История активностей');
  }
}

// ── Focus Floating Widget ──

function createFocusWidget() {
  const existing = document.getElementById('focus-widget');
  if (existing) existing.remove();
  const widget = document.createElement('div');
  widget.id = 'focus-widget';
  widget.className = 'focus-widget';
  widget.innerHTML = `
    <div class="focus-widget-popover hidden" id="fw-popover">
      <h4>Быстрый старт</h4>
      <input class="fw-input" id="fw-input" placeholder="Название..." />
      <input class="fw-input" id="fw-duration" type="number" min="1" max="480" placeholder="Длительность (мин)..." />
      <div class="fw-presets">
        <span class="fw-preset" data-cat="work">Работа</span>
        <span class="fw-preset" data-cat="study">Учёба</span>
        <span class="fw-preset" data-cat="sport">Спорт</span>
        <span class="fw-preset" data-cat="rest">Отдых</span>
        <span class="fw-preset" data-cat="hobby">Хобби</span>
        <span class="fw-preset" data-cat="other">Другое</span>
      </div>
      <button class="fw-start-btn" id="fw-start-btn">Начать</button>
    </div>
    <div class="focus-widget-active hidden" id="fw-active">
      <span class="fw-pulse-dot"></span>
      <span class="fw-activity-name" id="fw-activity-name"></span>
      <span class="fw-timer" id="fw-timer">00:00</span>
      <span class="fw-popout-btn" id="fw-popout-btn" title="Окно поверх всех"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg></span>
      <span class="fw-stop-btn" id="fw-stop-btn">■</span>
    </div>
    <div class="focus-widget-btn" id="fw-idle-btn">◎</div>
  `;
  document.getElementById('content-area').appendChild(widget);
  bindFocusWidgetEvents();
}

function bindFocusWidgetEvents() {
  const idleBtn = document.getElementById('fw-idle-btn');
  const popover = document.getElementById('fw-popover');
  const startBtn = document.getElementById('fw-start-btn');
  const stopBtn = document.getElementById('fw-stop-btn');
  const presets = document.querySelectorAll('.fw-preset');
  let selectedCat = 'other';

  idleBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleFocusWidgetPopover();
  });

  presets.forEach(p => {
    p.addEventListener('click', (e) => {
      e.stopPropagation();
      presets.forEach(x => x.classList.remove('selected'));
      p.classList.add('selected');
      selectedCat = p.dataset.cat;
      const fwInput = document.getElementById('fw-input');
      if (!fwInput.value.trim()) fwInput.value = p.textContent;
    });
  });

  startBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const fwInput = document.getElementById('fw-input');
    const durationInput = document.getElementById('fw-duration');
    const title = fwInput.value.trim() || 'Без названия';
    const durMin = parseInt(durationInput.value);
    const duration = durMin > 0 ? durMin : null;
    try {
      await invoke('start_activity', {
        title, category: selectedCat, focusMode: false, duration, apps: null, sites: null,
      });
    } catch (_) {}
    fwInput.value = '';
    durationInput.value = '';
    presets.forEach(x => x.classList.remove('selected'));
    selectedCat = 'other';
    popover.classList.add('hidden');
    S.focusWidgetOpen = false;
    await updateFocusWidget();
  });

  stopBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try { await invoke('stop_activity'); } catch (_) {}
    await updateFocusWidget();
  });

  document.getElementById('fw-popout-btn')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    try { await invoke('toggle_focus_overlay'); } catch (err) { console.error('Focus overlay error:', err); }
  });

  document.getElementById('fw-active').addEventListener('click', (e) => {
    if (e.target.closest('.fw-stop-btn') || e.target.closest('.fw-popout-btn')) return;
    tabLoaders.switchTab?.('focus');
  });

  document.addEventListener('click', (e) => {
    const widget = document.getElementById('focus-widget');
    if (S.focusWidgetOpen && widget && !widget.contains(e.target)) {
      popover.classList.add('hidden');
      S.focusWidgetOpen = false;
    }
  });
}

function toggleFocusWidgetPopover() {
  const popover = document.getElementById('fw-popover');
  if (!popover) return;
  if (S.focusWidgetActivity) return;
  S.focusWidgetOpen = !S.focusWidgetOpen;
  popover.classList.toggle('hidden', !S.focusWidgetOpen);
  if (S.focusWidgetOpen) {
    const fwInput = document.getElementById('fw-input');
    if (fwInput) setTimeout(() => fwInput.focus(), 50);
  }
}

async function updateFocusWidget() {
  const widget = document.getElementById('focus-widget');
  if (!widget) return;

  let activity = null;
  try {
    activity = await invoke('get_current_activity');
  } catch (_) {}

  const idleBtn = document.getElementById('fw-idle-btn');
  const activeBar = document.getElementById('fw-active');
  const popover = document.getElementById('fw-popover');

  if (activity && activity.id) {
    const changed = !S.focusWidgetActivity || S.focusWidgetActivity.id !== activity.id;
    S.focusWidgetActivity = activity;
    idleBtn.classList.add('hidden');
    popover.classList.add('hidden');
    S.focusWidgetOpen = false;
    activeBar.classList.remove('hidden');
    document.getElementById('fw-activity-name').textContent = activity.title || 'Активность';
    if (changed) startFocusWidgetTimer(activity.started_at);
    updateSidebarFocusIndicator(true);
    emit('focus-state', { active: true, title: activity.title, started_at: activity.started_at });
  } else {
    S.focusWidgetActivity = null;
    stopFocusWidgetTimer();
    activeBar.classList.add('hidden');
    idleBtn.classList.remove('hidden');
    updateSidebarFocusIndicator(false);
    emit('focus-state', { active: false });
  }

  updateFocusWidgetVisibility();
}

function startFocusWidgetTimer(startedAt) {
  stopFocusWidgetTimer();
  const start = new Date(startedAt).getTime();
  const timerEl = document.getElementById('fw-timer');
  function tick() {
    const elapsed = Math.floor((Date.now() - start) / 1000);
    const h = Math.floor(elapsed / 3600);
    const m = Math.floor((elapsed % 3600) / 60);
    const s = elapsed % 60;
    if (timerEl) timerEl.textContent = h > 0
      ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
      : `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
  tick();
  S.focusWidgetTimerInterval = setInterval(tick, 1000);
}

function stopFocusWidgetTimer() {
  if (S.focusWidgetTimerInterval) {
    clearInterval(S.focusWidgetTimerInterval);
    S.focusWidgetTimerInterval = null;
  }
}

function updateSidebarFocusIndicator(active) {
  const focusTab = document.querySelector('.tab-item[data-tab-id="focus"]');
  if (!focusTab) return;
  const existing = focusTab.querySelector('.tab-focus-dot');
  if (active && !existing) {
    const dot = document.createElement('span');
    dot.className = 'tab-focus-dot';
    focusTab.appendChild(dot);
  } else if (!active && existing) {
    existing.remove();
  }
}

function updateFocusWidgetVisibility() {
  const widget = document.getElementById('focus-widget');
  if (widget) widget.classList.toggle('hidden', S.activeTab === 'focus');
}

// Listen for focus-state from overlay window (e.g. stop from overlay)
listen('focus-state', (ev) => {
  if (!ev.payload.active && S.focusWidgetActivity) {
    updateFocusWidget();
  }
});

export {
  loadFocus,
  createFocusWidget,
  updateFocusWidget,
  updateFocusWidgetVisibility,
  toggleFocusWidgetPopover,
  startPomodoro,
  startFocusWidgetTimer,
  stopFocusWidgetTimer,
  bindFocusWidgetEvents,
  updateSidebarFocusIndicator,
};
