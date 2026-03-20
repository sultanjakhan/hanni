// ── tab-data-work-projects.js — Projects sub-tab + Writer's Forge ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { loadWriterForge } from './integration-writerforge.js';

export async function loadProjects() {
  const el = document.getElementById('projects-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'projects', {
    title: 'Projects',
    subtitle: 'Проекты и их задачи',
    icon: '📁',
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
      const activeInner = S._projectsInner || 'local';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'local' ? ' active' : ''}" data-prj="local">Проекты</button>
          <button class="pill${activeInner === 'forge' ? ' active' : ''}" data-prj="forge">✍️ Writer's Forge</button>
        </div>
        <div id="projects-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#projects-inner-content');
      if (activeInner === 'forge') await loadWriterForge(innerEl);
      else {
        const projects = await invoke('get_projects').catch(() => []);
        await renderProjectsView(innerEl, projects || []);
      }
      paneEl.querySelectorAll('[data-prj]').forEach(btn => {
        btn.addEventListener('click', () => { S._projectsInner = btn.dataset.prj; loadProjects(); });
      });
    },
  });
}

async function renderProjectsView(el, projects) {
  if (!S.currentProjectId && projects.length > 0) S.currentProjectId = projects[0].id;
  const tasks = S.currentProjectId ? await invoke('get_tasks', { projectId: S.currentProjectId }).catch(() => []) : [];

  el.innerHTML = `<div class="work-layout">
    <div class="work-projects">
      <div class="work-projects-header">
        <button class="btn-primary" id="new-project-btn" style="width:100%;">+ Проект</button>
      </div>
      <div class="work-projects-list" id="work-projects-list"></div>
    </div>
    <div class="work-tasks">
      <div class="work-tasks-header">
        <h2 style="font-size:16px;color:var(--text-primary);">${S.currentProjectId ? escapeHtml(projects.find(p => p.id === S.currentProjectId)?.name || '') : 'Выберите проект'}</h2>
        ${S.currentProjectId ? '<button class="btn-primary" id="new-task-btn">+ Задача</button>' : ''}
      </div>
      <div id="work-tasks-list"></div>
    </div>
  </div>`;

  const projectList = document.getElementById('work-projects-list');
  for (const p of projects) {
    const item = document.createElement('div');
    item.className = 'work-project-item' + (p.id === S.currentProjectId ? ' active' : '');
    item.innerHTML = `<span class="work-project-dot" style="background:${p.color || 'var(--accent-blue)'}"></span>
      <span class="work-project-name">${escapeHtml(p.name)}</span>
      <span class="work-project-count">${p.task_count || 0}</span>`;
    item.addEventListener('click', () => { S.currentProjectId = p.id; loadProjects(); });
    projectList.appendChild(item);
  }

  const taskList = document.getElementById('work-tasks-list');
  for (const t of (tasks || [])) {
    const item = document.createElement('div');
    item.className = 'work-task-item';
    const isDone = t.status === 'done';
    item.innerHTML = `
      <div class="work-task-check${isDone ? ' done' : ''}" data-id="${t.id}"></div>
      <span class="work-task-title${isDone ? ' done' : ''}">${escapeHtml(t.title)}</span>
      <span class="work-task-priority priority-${t.priority || 'normal'}">${t.priority || 'normal'}</span>`;
    item.querySelector('.work-task-check').addEventListener('click', async () => {
      await invoke('update_task_status', { id: t.id, status: isDone ? 'todo' : 'done' }).catch(() => {});
      loadProjects();
    });
    taskList.appendChild(item);
  }

  document.getElementById('new-project-btn')?.addEventListener('click', () => {
    const name = prompt('Название проекта:');
    if (name) invoke('create_project', { name, description: '', color: '#9B9B9B' }).then(() => loadProjects()).catch(e => alert(e));
  });
  document.getElementById('new-task-btn')?.addEventListener('click', () => {
    const title = prompt('Задача:');
    if (title) invoke('create_task', { projectId: S.currentProjectId, title, description: '', priority: 'normal', dueDate: null }).then(() => loadProjects()).catch(e => alert(e));
  });
}
