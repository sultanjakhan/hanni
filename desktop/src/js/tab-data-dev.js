// ── tab-data-dev.js — Development tab (learning items, skills) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Development ──
export async function loadDevelopment() {
  const el = document.getElementById('development-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'development', {
    title: 'Development',
    subtitle: 'Обучение и навыки',
    icon: '🚀',
    renderDash: async (paneEl) => {
      const items = await invoke('get_learning_items', { typeFilter: null }).catch(() => []);
      const inProgress = items.filter(i => i.status === 'in_progress').length;
      const completed = items.filter(i => i.status === 'completed').length;
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${items.length}</div><div class="uni-dash-label">Всего</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${inProgress}</div><div class="uni-dash-label">В процессе</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${completed}</div><div class="uni-dash-label">Завершено</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      try {
        const items = await invoke('get_learning_items', { typeFilter: S.devFilter === 'all' ? null : S.devFilter }).catch(() => []);
        renderDevelopment(paneEl, items || []);
      } catch (e) {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить</div>';
      }
    },
  });
}

function renderDevelopment(el, items) {
  const filters = ['all', 'course', 'book', 'skill', 'article'];
  const filterLabels = { all: 'Все', course: 'Курсы', book: 'Книги', skill: 'Навыки', article: 'Статьи' };
  const statusLabels = { planned: 'Запланировано', in_progress: 'В процессе', completed: 'Завершено' };
  const statusColors = { planned: 'badge-gray', in_progress: 'badge-blue', completed: 'badge-green' };

  const filterBar = `<div class="dev-filters">
    ${filters.map(f => `<button class="pill${S.devFilter === f ? ' active' : ''}" data-filter="${f}">${filterLabels[f]}</button>`).join('')}
  </div>`;

  const fixedColumns = [
    { key: 'title', label: 'Title', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
    { key: 'type', label: 'Type', render: r => `<span class="badge badge-purple">${filterLabels[r.type] || r.type}</span>` },
    { key: 'status', label: 'Status', render: r => `<span class="badge ${statusColors[r.status] || 'badge-gray'}">${statusLabels[r.status] || r.status}</span>` },
    { key: 'progress', label: 'Progress', render: r => `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${r.progress || 0}%"></div></div> <span style="font-size:11px;color:var(--text-faint);">${r.progress || 0}%</span>` },
  ];

  el.innerHTML = filterBar + '<div id="dev-dbv"></div>';
  const dbvEl = document.getElementById('dev-dbv');

  const dbv = new DatabaseView(dbvEl, {
    tabId: 'development',
    recordTable: 'learning_items',
    records: items,
    fixedColumns,
    idField: 'id',
    availableViews: ['table', 'kanban', 'list'],
    defaultView: 'table',
    addButton: '+ Добавить',
    onAdd: () => showAddLearningModal(),
    onQuickAdd: async (title) => {
      await invoke('create_learning_item', { itemType: 'course', title, description: '', url: '' });
      loadDevelopment();
    },
    reloadFn: () => loadDevelopment(),
    kanban: {
      groupByField: 'status',
      columns: [
        { key: 'planned', label: 'Запланировано', icon: '\ud83d\udccb' },
        { key: 'in_progress', label: 'В процессе', icon: '\u25b6' },
        { key: 'completed', label: 'Завершено', icon: '\u2705' },
      ],
    },
    onDrop: async (recordId, field, newValue) => {
      try {
        await invoke('update_learning_item_status', { id: parseInt(recordId), status: newValue });
        loadDevelopment();
      } catch (err) { console.error('kanban drop:', err); }
    },
  });
  S.dbViews.development = dbv;
  dbv.render();

  el.querySelectorAll('.dev-filters .pill').forEach(btn => {
    btn.addEventListener('click', () => { S.devFilter = btn.dataset.filter; loadDevelopment(); });
  });
}

function showAddLearningModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить</div>
    <div class="form-group"><label class="form-label">Тип</label>
      <select class="form-select" id="learn-type" style="width:100%;">
        <option value="course">Курс</option><option value="book">Книга</option>
        <option value="skill">Навык</option><option value="article">Статья</option>
      </select></div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="learn-title"></div>
    <div class="form-group"><label class="form-label">Описание</label><textarea class="form-textarea" id="learn-desc"></textarea></div>
    <div class="form-group"><label class="form-label">URL</label><input class="form-input" id="learn-url"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="learn-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('learn-save')?.addEventListener('click', async () => {
    const title = document.getElementById('learn-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_learning_item', {
        itemType: document.getElementById('learn-type')?.value || 'course',
        title,
        description: document.getElementById('learn-desc')?.value || '',
        url: document.getElementById('learn-url')?.value || '',
      });
      overlay.remove();
      loadDevelopment();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}
