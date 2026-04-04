// ── tab-dev-cases.js — Cases pane for Development tab ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';

function scoreBadge(score) {
  if (!score) return '<span class="badge badge-gray">—</span>';
  const colors = { 1: 'red', 2: 'red', 3: 'yellow', 4: 'green', 5: 'green' };
  return `<span class="badge badge-${colors[score] || 'gray'}">${score}/5</span>`;
}

export async function renderCasesPane(el, projectId, reloadFn) {
  const [cases, skills] = await Promise.all([
    invoke('get_dev_cases', { skillId: null, projectId }).catch(() => []),
    invoke('get_dev_skills', { projectId }).catch(() => []),
  ]);

  const activeSkill = S._devCaseFilter || 'all';

  // Skill filter pills
  const filterHtml = `<div class="dev-filters">
    <button class="pill${activeSkill === 'all' ? ' active' : ''}" data-filter="all">Все</button>
    ${skills.map(s => `<button class="pill${activeSkill === String(s.id) ? ' active' : ''}" data-filter="${s.id}">${escapeHtml(s.name)}</button>`).join('')}
  </div>`;

  const filtered = activeSkill === 'all' ? cases : cases.filter(c => String(c.skill_id) === activeSkill);

  const listHtml = filtered.length ? filtered.map(c => `
    <div class="dev-case-row" data-id="${c.id}">
      <div class="dev-case-main">
        ${c.url ? `<a href="${escapeHtml(c.url)}" target="_blank" class="dev-case-title">${escapeHtml(c.title)}</a>` : `<span class="dev-case-title">${escapeHtml(c.title)}</span>`}
        <span class="badge badge-purple">${escapeHtml(c.skill_name)}</span>
        ${scoreBadge(c.score)}
        ${c.solved_at ? '<span class="badge badge-green">✓</span>' : ''}
      </div>
      ${c.notes ? `<div class="dev-case-notes">${escapeHtml(c.notes)}</div>` : ''}
      <button class="dev-case-del" title="Удалить">×</button>
    </div>
  `).join('') : '<div class="uni-empty">Нет кейсов</div>';

  el.innerHTML = `${filterHtml}<div class="dev-cases-list">${listHtml}</div>
    <button class="btn-secondary dev-add-case">+ Кейс</button>`;

  // Filter clicks
  el.querySelectorAll('.dev-filters .pill').forEach(btn => {
    btn.addEventListener('click', () => {
      S._devCaseFilter = btn.dataset.filter;
      reloadFn();
    });
  });

  // Delete
  el.querySelectorAll('.dev-case-del').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = parseInt(btn.closest('.dev-case-row').dataset.id);
      await invoke('delete_dev_case', { id });
      reloadFn();
    });
  });

  // Add case
  el.querySelector('.dev-add-case')?.addEventListener('click', () => {
    showAddCaseModal(skills, reloadFn);
  });
}

function showAddCaseModal(skills, reloadFn) {
  if (!skills.length) { alert('Сначала добавьте навыки'); return; }
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить кейс</div>
    <div class="form-group"><label class="form-label">Навык</label>
      <select class="form-select" id="dev-case-skill" style="width:100%;">
        ${skills.map(s => `<option value="${s.id}">${escapeHtml(s.name)}</option>`).join('')}
      </select></div>
    <div class="form-group"><label class="form-label">Название</label>
      <input class="form-input" id="dev-case-title" placeholder="Product sense: Uber"></div>
    <div class="form-group"><label class="form-label">URL</label>
      <input class="form-input" id="dev-case-url" placeholder="https://..."></div>
    <div class="form-group"><label class="form-label">Описание</label>
      <textarea class="form-textarea" id="dev-case-desc" rows="2"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="dev-case-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('dev-case-save')?.addEventListener('click', async () => {
    const title = document.getElementById('dev-case-title')?.value?.trim();
    if (!title) return;
    const skillId = parseInt(document.getElementById('dev-case-skill')?.value);
    const url = document.getElementById('dev-case-url')?.value || '';
    const description = document.getElementById('dev-case-desc')?.value || '';
    await invoke('create_dev_case', { skillId, title, url, description });
    overlay.remove();
    reloadFn();
  });
}
