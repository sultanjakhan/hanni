// ── tab-dev-skills.js — Skills pane for Development tab ──

import { S, invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';

function scoreColor(score) {
  if (score >= 4) return 'var(--green)';
  if (score === 3) return 'var(--yellow)';
  if (score >= 1) return 'var(--red)';
  return 'var(--border)';
}

function scoreLabel(score) {
  const labels = ['Не оценено', 'Начинающий', 'Базовый', 'Средний', 'Продвинутый', 'Эксперт'];
  return labels[score] || labels[0];
}

function renderStars(score) {
  return Array.from({ length: 5 }, (_, i) =>
    `<span class="dev-star ${i < score ? 'filled' : ''}" data-val="${i + 1}">★</span>`
  ).join('');
}

export async function renderSkillsPane(el, projectId, reloadFn) {
  const skills = await invoke('get_dev_skills', { projectId }).catch(() => []);
  if (!skills.length) {
    el.innerHTML = '<div class="uni-empty">Нет навыков</div>';
    return;
  }

  el.innerHTML = `<div class="dev-skills-grid">${skills.map(s => `
    <div class="dev-skill-card" data-id="${s.id}">
      <div class="dev-skill-header">
        <span class="dev-skill-name">${escapeHtml(s.name)}</span>
        <span class="dev-skill-level" style="color:${scoreColor(s.score)}">${scoreLabel(s.score)}</span>
      </div>
      <div class="dev-skill-stars" data-id="${s.id}">${renderStars(s.score)}</div>
      <div class="dev-skill-meta">
        <span class="dev-skill-cases">${s.solved_count}/${s.case_count} кейсов</span>
      </div>
      ${s.description ? `<div class="dev-skill-desc">${escapeHtml(s.description)}</div>` : ''}
    </div>
  `).join('')}</div>
  <button class="btn-secondary dev-add-skill">+ Навык</button>`;

  // Star click → update score
  el.querySelectorAll('.dev-skill-stars').forEach(container => {
    container.querySelectorAll('.dev-star').forEach(star => {
      star.addEventListener('click', async () => {
        const id = parseInt(container.dataset.id);
        const val = parseInt(star.dataset.val);
        await invoke('update_dev_skill', { id, name: null, description: null, score: val });
        reloadFn();
      });
    });
  });

  // Add skill button
  el.querySelector('.dev-add-skill')?.addEventListener('click', () => {
    showAddSkillModal(projectId, reloadFn);
  });
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
