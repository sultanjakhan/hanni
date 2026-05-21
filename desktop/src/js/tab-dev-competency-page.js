// ── tab-dev-competency-page.js — competency detail: theory, skills, cases ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderWikiMarkdown, buildNodeIndex } from './wiki-markdown.js';
import { wireWikiLinks } from './tab-dev-wiki.js';

function scoreColor(v) {
  if (v >= 7) return 'var(--green)';
  if (v >= 4) return 'var(--yellow)';
  if (v >= 1) return 'var(--red)';
  return 'var(--text-faint)';
}

export async function renderCompetencyPage(el, nodeId, projectId, reloadFn) {
  const nodes = await invoke('get_dev_nodes', { projectId }).catch(() => []);
  const comp = nodes.find(n => n.id === nodeId);
  if (!comp) { S._devOpenNode = null; reloadFn(); return; }
  const skills = nodes.filter(n => n.parent_id === nodeId && n.kind === 'skill');
  const cases = await invoke('get_dev_cases', { nodeId, projectId: null }).catch(() => []);
  const nodeIndex = buildNodeIndex(nodes);

  const theoryHtml = comp.theory
    ? renderWikiMarkdown(comp.theory, nodeIndex)
    : '<span class="text-faint">Теория пока не заполнена</span>';

  const skillsHtml = skills.length ? skills.map(s => `
    <div class="dev-cp-skill">
      ${s.priority ? '<span class="dev-cp-prio" title="Приоритет изучения">⚑</span>' : ''}
      <span class="dev-cp-skill-name">${escapeHtml(s.name)}</span>
      <span class="dev-cp-skill-score" style="color:${scoreColor(s.score)}">${s.score || 0}/10</span>
    </div>`).join('') : '<div class="text-faint" style="padding:6px 0">Нет навыков</div>';

  const casesHtml = cases.length ? cases.map(c => `
    <div class="dev-case-row" data-id="${c.id}">
      <div class="dev-case-main">
        ${c.url
          ? `<a href="${escapeHtml(c.url)}" target="_blank" class="dev-case-title">${escapeHtml(c.title)}</a>`
          : `<span class="dev-case-title">${escapeHtml(c.title)}</span>`}
        ${c.solved_at ? '<span class="badge badge-green">done</span>' : ''}
      </div>
      ${c.description ? `<div class="dev-case-notes">${escapeHtml(c.description)}</div>` : ''}
      <button class="dev-case-del" title="Удалить">×</button>
    </div>`).join('') : '<div class="text-faint" style="padding:6px 0">Нет кейсов</div>';

  el.innerHTML = `
    <button class="dev-back-btn">← Матрица</button>
    <div class="dev-page-header">
      <h2 class="dev-page-title">${escapeHtml(comp.name)}</h2>
    </div>
    <div class="dev-page-sections">
      <details class="dev-section" open>
        <summary class="dev-section-title">Теория <button class="dev-theory-edit" title="Редактировать теорию">✏️</button></summary>
        <div class="dev-section-body dev-theory-content">${theoryHtml}</div>
      </details>
      <details class="dev-section" open>
        <summary class="dev-section-title">Навыки (${skills.length})</summary>
        <div class="dev-section-body"><div class="dev-cp-skills">${skillsHtml}</div></div>
      </details>
      <details class="dev-section" open>
        <summary class="dev-section-title">Кейсы (${cases.length})</summary>
        <div class="dev-section-body">
          <div class="dev-cases-list">${casesHtml}</div>
          <button class="btn-secondary dev-add-case-btn">+ Кейс</button>
        </div>
      </details>
    </div>`;

  el.querySelector('.dev-back-btn').addEventListener('click', () => { S._devOpenNode = null; reloadFn(); });
  el.querySelector('.dev-theory-edit')?.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    showTheoryEditor(comp, reloadFn);
  });
  el.querySelectorAll('.dev-case-del').forEach(b => b.addEventListener('click', async () => {
    await invoke('delete_dev_case', { id: parseInt(b.closest('.dev-case-row').dataset.id) });
    reloadFn();
  }));
  el.querySelector('.dev-add-case-btn')?.addEventListener('click', () => showAddCase(nodeId, comp.name, reloadFn));
  wireWikiLinks(el, reloadFn);
}

function showTheoryEditor(comp, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-wide">
    <div class="modal-title">Теория: ${escapeHtml(comp.name)}</div>
    <div class="form-group">
      <label class="form-label">Markdown — термины пиши как [[Название компетенции]]</label>
      <textarea class="form-textarea" id="dev-theory-text"></textarea>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" data-cancel>Отмена</button>
      <button class="btn-primary" data-save>Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#dev-theory-text').value = comp.theory || '';
  const close = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) close(); });
  overlay.querySelector('[data-cancel]').addEventListener('click', close);
  overlay.querySelector('[data-save]').addEventListener('click', async () => {
    const theory = overlay.querySelector('#dev-theory-text').value;
    await invoke('update_dev_node', { id: comp.id, name: null, score: null, theory, material: null, priority: null });
    close();
    reloadFn();
  });
}

function showAddCase(nodeId, compName, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Кейс: ${escapeHtml(compName)}</div>
    <div class="form-group"><label class="form-label">Название</label>
      <input class="form-input" id="dev-case-title"></div>
    <div class="form-group"><label class="form-label">Ссылка</label>
      <input class="form-input" id="dev-case-url" placeholder="https://..."></div>
    <div class="form-group"><label class="form-label">Описание</label>
      <textarea class="form-textarea" id="dev-case-desc" rows="3"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" data-cancel>Отмена</button>
      <button class="btn-primary" data-save>Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  const close = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) close(); });
  overlay.querySelector('[data-cancel]').addEventListener('click', close);
  overlay.querySelector('[data-save]').addEventListener('click', async () => {
    const title = overlay.querySelector('#dev-case-title').value.trim();
    if (!title) return;
    const url = overlay.querySelector('#dev-case-url').value || '';
    const description = overlay.querySelector('#dev-case-desc').value || '';
    await invoke('create_dev_case', { nodeId, title, url, description });
    close();
    reloadFn();
  });
}
