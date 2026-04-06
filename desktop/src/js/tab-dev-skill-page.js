// ── tab-dev-skill-page.js — Individual skill page view ──

import { S, invoke } from './state.js';
import { escapeHtml, renderMarkdown } from './utils.js';

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

function renderStars(score) {
  return Array.from({ length: 5 }, (_, i) =>
    `<span class="dev-star interactive ${i < score ? 'filled' : ''}" data-val="${i + 1}">★</span>`
  ).join('');
}

export async function renderSkillPage(el, skillId, projectId, reloadFn) {
  const skills = await invoke('get_dev_skills', { projectId }).catch(() => []);
  const skill = skills.find(s => s.id === skillId);
  if (!skill) { S._devOpenSkill = null; reloadFn(); return; }

  const cases = await invoke('get_dev_cases', { skillId, projectId: null }).catch(() => []);
  const theoryHtml = skill.theory ? renderMarkdown(skill.theory) : '<span class="text-faint">Нет теории</span>';

  const casesHtml = cases.length ? cases.map(c => `
    <div class="dev-case-row" data-id="${c.id}">
      <div class="dev-case-main">
        ${c.url ? `<a href="${escapeHtml(c.url)}" target="_blank" class="dev-case-title">${escapeHtml(c.title)}</a>` : `<span class="dev-case-title">${escapeHtml(c.title)}</span>`}
        ${c.score ? `<span class="badge badge-${c.score >= 4 ? 'green' : c.score === 3 ? 'yellow' : 'red'}">${c.score}/5</span>` : ''}
        ${c.solved_at ? '<span class="badge badge-green">done</span>' : ''}
      </div>
      ${c.notes ? `<div class="dev-case-notes">${escapeHtml(c.notes)}</div>` : ''}
      <button class="dev-case-del" title="Удалить">×</button>
    </div>
  `).join('') : '<div class="text-faint" style="padding:8px 0">Нет кейсов</div>';

  el.innerHTML = `
    <button class="dev-back-btn">\u2190 Все навыки</button>
    <div class="dev-page-header">
      <h2 class="dev-page-title">${escapeHtml(skill.name)}</h2>
      <div class="dev-page-subtitle">${escapeHtml(skill.description)}</div>
    </div>
    <div class="dev-page-score">
      <span class="dev-page-score-label">Уровень:</span>
      <span class="dev-skill-stars" data-id="${skill.id}">${renderStars(skill.score)}</span>
      <span class="dev-skill-level" style="color:${scoreColor(skill.score)}">${scoreLabel(skill.score)}</span>
    </div>
    <div class="dev-page-sections">
      <details class="dev-section" open>
        <summary class="dev-section-title">Теория</summary>
        <div class="dev-section-body dev-theory-content">${theoryHtml}</div>
      </details>
      <details class="dev-section" open>
        <summary class="dev-section-title">Кейсы (${cases.length})</summary>
        <div class="dev-section-body">
          <div class="dev-cases-list">${casesHtml}</div>
          <button class="btn-secondary dev-add-case-btn">+ Кейс</button>
        </div>
      </details>
    </div>`;

  wireSkillPageEvents(el, skillId, skill.name, reloadFn);
}

function wireSkillPageEvents(el, skillId, skillName, reloadFn) {
  el.querySelector('.dev-back-btn')?.addEventListener('click', () => {
    S._devOpenSkill = null;
    reloadFn();
  });

  el.querySelectorAll('.dev-star.interactive').forEach(star => {
    star.addEventListener('click', async () => {
      const val = parseInt(star.dataset.val);
      await invoke('update_dev_skill', { id: skillId, name: null, description: null, theory: null, score: val });
      reloadFn();
    });
  });

  el.querySelectorAll('.dev-case-del').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = parseInt(btn.closest('.dev-case-row').dataset.id);
      await invoke('delete_dev_case', { id });
      reloadFn();
    });
  });

  el.querySelector('.dev-add-case-btn')?.addEventListener('click', () => {
    showAddCaseForSkill(skillId, skillName, reloadFn);
  });
}

function showAddCaseForSkill(skillId, skillName, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Кейс: ${escapeHtml(skillName)}</div>
    <div class="form-group"><label class="form-label">Название</label>
      <input class="form-input" id="dev-case-title"></div>
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
    const url = document.getElementById('dev-case-url')?.value || '';
    const description = document.getElementById('dev-case-desc')?.value || '';
    await invoke('create_dev_case', { skillId, title, url, description });
    overlay.remove();
    reloadFn();
  });
}
