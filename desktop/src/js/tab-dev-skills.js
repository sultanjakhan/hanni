// ── tab-dev-skills.js — Skills list pane for Development tab ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderSkillPage } from './tab-dev-skill-page.js';

function scoreColor(score) {
  if (score >= 4) return 'var(--green)';
  if (score === 3) return 'var(--yellow)';
  if (score >= 1) return 'var(--red)';
  return 'var(--text-faint)';
}

function scoreLabel(score) {
  const labels = ['Не оценено', 'Начинающий', 'Базовый', 'Средний', 'Продвинутый', 'Эксперт'];
  return labels[score] || labels[0];
}

export async function renderSkillsPane(el, projectId, reloadFn) {
  if (S._devOpenSkill) {
    await renderSkillPage(el, S._devOpenSkill, projectId, reloadFn);
    return;
  }

  const skills = await invoke('get_dev_skills', { projectId }).catch(() => []);
  if (!skills.length) {
    el.innerHTML = '<div class="uni-empty">Нет навыков</div>';
    return;
  }

  el.innerHTML = `<div class="dev-skills-list">${skills.map(s => `
    <div class="dev-skill-row" data-id="${s.id}">
      <div class="dev-skill-score-dot" style="background:${scoreColor(s.score)}"></div>
      <span class="dev-skill-name">${escapeHtml(s.name)}</span>
      <span class="dev-skill-level" style="color:${scoreColor(s.score)}">${scoreLabel(s.score)}</span>
      <span class="dev-skill-cases-count">${s.solved_count}/${s.case_count}</span>
      <span class="dev-skill-arrow">\u2192</span>
    </div>
  `).join('')}</div>
  <button class="btn-secondary dev-add-skill">+ Навык</button>`;

  el.querySelectorAll('.dev-skill-row').forEach(row => {
    row.addEventListener('click', () => {
      S._devOpenSkill = parseInt(row.dataset.id);
      reloadFn();
    });
  });

  el.querySelector('.dev-add-skill')?.addEventListener('click', () => showAddSkillModal(projectId, reloadFn));
}

function showAddSkillModal(projectId, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить навык</div>
    <div class="form-group"><label class="form-label">Название</label>
      <input class="form-input" id="dev-skill-name" placeholder="e.g. SQL"></div>
    <div class="form-group"><label class="form-label">Описание</label>
      <textarea class="form-textarea" id="dev-skill-desc" rows="2"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="dev-skill-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('dev-skill-save')?.addEventListener('click', async () => {
    const name = document.getElementById('dev-skill-name')?.value?.trim();
    if (!name) return;
    const desc = document.getElementById('dev-skill-desc')?.value || '';
    await invoke('create_dev_skill', { projectId, name, description: desc });
    overlay.remove();
    reloadFn();
  });
}
