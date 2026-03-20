// ── tab-data-work.js — Work tab (tasks with full DatabaseView features) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

export { loadProjects } from './tab-data-work-projects.js';

export async function loadWork() {
  const el = document.getElementById('work-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'work', {
    title: 'Work',
    subtitle: 'Проекты и задачи',
    icon: '💼',
    renderDash: async (paneEl) => {
      const projects = await invoke('get_projects').catch(() => []);
      const totalTasks = projects.reduce((sum, p) => sum + (p.task_count || 0), 0);
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${projects.length}</div><div class="uni-dash-label">Проекты</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${totalTasks}</div><div class="uni-dash-label">Всего задач</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      try {
        const projects = await invoke('get_projects').catch(() => []);
        let allTasks = [];
        for (const p of projects) {
          const tasks = await invoke('get_tasks', { projectId: p.id }).catch(() => []);
          allTasks.push(...tasks.map(t => ({ ...t, projectName: p.name, projectColor: p.color })));
        }
        renderWorkTasks(paneEl, allTasks, projects);
      } catch {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить задачи</div>';
      }
    },
  });
}

const STATUS = { todo: 'To Do', in_progress: 'В работе', done: 'Готово' };
const S_COLORS = { todo: 'badge-gray', in_progress: 'badge-blue', done: 'badge-green' };
const P_COLORS = { high: 'badge-red', normal: 'badge-gray', low: 'badge-gray' };

function renderWorkTasks(el, tasks, projects) {
  const dbv = new DatabaseView(el, {
    tabId: 'work',
    recordTable: 'tasks',
    records: tasks,
    fixedColumns: [
      { key: 'done', label: '', render: r => `<div class="work-task-check${r.status === 'done' ? ' done' : ''}" data-tid="${r.id}" style="cursor:pointer;"></div>` },
      { key: 'title', label: 'Задача', editable: true, editType: 'text', render: r => `<span class="data-table-title" style="${r.status === 'done' ? 'text-decoration:line-through;opacity:0.5;' : ''}">${escapeHtml(r.title)}</span>` },
      { key: 'projectName', label: 'Проект', editable: false, render: r => `<span style="color:${r.projectColor || 'var(--text-secondary)'};font-size:12px;">${escapeHtml(r.projectName || '')}</span>` },
      { key: 'priority', label: 'Приоритет', editable: true, editType: 'select', editOptions: [
        { value: 'low', label: 'low' }, { value: 'normal', label: 'normal' }, { value: 'high', label: 'high' },
      ], render: r => `<span class="badge ${P_COLORS[r.priority] || 'badge-gray'}">${r.priority || 'normal'}</span>` },
      { key: 'status', label: 'Статус', editable: true, editType: 'select', editOptions: [
        { value: 'todo', label: 'To Do' }, { value: 'in_progress', label: 'В работе' }, { value: 'done', label: 'Готово' },
      ], render: r => `<span class="badge ${S_COLORS[r.status] || 'badge-gray'}">${STATUS[r.status] || r.status}</span>` },
    ],
    idField: 'id',
    availableViews: ['table', 'kanban', 'list', 'gallery'],
    defaultView: 'table',
    addButton: '+ Задача',
    onAdd: () => showAddTaskModal(projects),
    onQuickAdd: async (title) => {
      let pid = projects[0]?.id;
      if (!pid) pid = await invoke('create_project', { name: 'Входящие', description: '', color: '#9B9B9B' });
      await invoke('create_task', { projectId: pid, title, description: '', priority: 'normal', dueDate: null });
      loadWork();
    },
    onCellEdit: async (recordId, key, value, skipReload) => {
      await invoke('update_task_field', { id: recordId, field: key, value });
      if (!skipReload) loadWork();
    },
    onDelete: async (recordId) => {
      await invoke('delete_task', { id: recordId });
      loadWork();
    },
    onDuplicate: async (recordId) => {
      const t = tasks.find(r => r.id === recordId);
      if (!t) return;
      await invoke('create_task', { projectId: t.project_id, title: t.title + ' (копия)', description: t.description || '', priority: t.priority || 'normal', dueDate: t.due_date || null });
      loadWork();
    },
    reloadFn: () => loadWork(),
    kanban: {
      groupByField: 'status',
      columns: [
        { key: 'todo', label: 'To Do', icon: '📋' },
        { key: 'in_progress', label: 'В работе', icon: '▶' },
        { key: 'done', label: 'Готово', icon: '✅' },
      ],
    },
    onDrop: async (recordId, field, newValue) => {
      await invoke('update_task_status', { id: parseInt(recordId), status: newValue }).catch(() => {});
      loadWork();
    },
  });
  dbv.render();

  // Delegate click for task checkboxes
  el.addEventListener('click', async (e) => {
    const check = e.target.closest('[data-tid]');
    if (!check) return;
    const id = parseInt(check.dataset.tid);
    const task = tasks.find(t => t.id === id);
    await invoke('update_task_status', { id, status: task?.status === 'done' ? 'todo' : 'done' }).catch(() => {});
    loadWork();
  });
}

function showAddTaskModal(projects) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новая задача</div>
    <div class="form-group"><label class="form-label">Задача</label><input class="form-input" id="wt-title" placeholder="Название задачи"></div>
    <div class="form-group"><label class="form-label">Проект</label>
      <select class="form-input" id="wt-project">
        ${projects.map(p => `<option value="${p.id}">${escapeHtml(p.name)}</option>`).join('')}
        ${projects.length === 0 ? '<option value="">Нет проектов</option>' : ''}
      </select>
    </div>
    <div class="form-group"><label class="form-label">Приоритет</label>
      <select class="form-input" id="wt-priority">
        <option value="normal">Normal</option><option value="high">High</option><option value="low">Low</option>
      </select>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" id="wt-cancel">Отмена</button>
      <button class="btn-primary" id="wt-save">Создать</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('wt-cancel')?.addEventListener('click', () => overlay.remove());
  document.getElementById('wt-save')?.addEventListener('click', async () => {
    const title = document.getElementById('wt-title')?.value?.trim();
    if (!title) return;
    const projectId = parseInt(document.getElementById('wt-project')?.value);
    const priority = document.getElementById('wt-priority')?.value || 'normal';
    if (!projectId) { alert('Сначала создайте проект'); return; }
    await invoke('create_task', { projectId, title, description: '', priority, dueDate: null }).catch(e => alert(e));
    overlay.remove();
    loadWork();
  });
}
