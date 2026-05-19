// ── tab-dev-wiki.js — "main article" wiki pane for dev projects ──

import { S, invoke } from './state.js';
import { renderWikiMarkdown, buildSkillIndex } from './wiki-markdown.js';
import { savePaneState } from './db-view/unified-layout.js';

export async function renderWikiPane(el, projectId, reloadFn) {
  const [projects, skills] = await Promise.all([
    invoke('get_dev_projects').catch(() => []),
    invoke('get_dev_skills', { projectId }).catch(() => []),
  ]);
  const project = projects.find(p => p.id === projectId);
  const overview = project?.overview || '';
  const skillIndex = buildSkillIndex(skills);

  if (!overview.trim()) {
    el.innerHTML = `<div class="dev-wiki-pane">
      <div class="dev-wiki-empty">Главная статья пока не заполнена.</div>
      <div style="text-align:center">
        <button class="btn-secondary dev-wiki-edit-empty">Редактировать</button>
      </div>
    </div>`;
    el.querySelector('.dev-wiki-edit-empty')
      ?.addEventListener('click', () => showOverviewEditor(projectId, overview, reloadFn));
    return;
  }

  el.innerHTML = `<div class="dev-wiki-pane">
    <button class="dev-wiki-edit" title="Редактировать статью">✏️</button>
    <div class="dev-wiki-content">${renderWikiMarkdown(overview, skillIndex)}</div>
  </div>`;
  el.querySelector('.dev-wiki-edit')
    ?.addEventListener('click', () => showOverviewEditor(projectId, overview, reloadFn));
  wireWikiLinks(el, reloadFn);
}

/** Make resolved [[wiki-links]] open the matching skill page. */
export function wireWikiLinks(el, reloadFn) {
  el.querySelectorAll('.wiki-link[data-skill-id]').forEach(a => {
    a.addEventListener('click', (e) => {
      e.preventDefault();
      S._devOpenSkill = parseInt(a.dataset.skillId);
      S._unifiedPane.development = 'skills';
      savePaneState('development', 'skills');
      reloadFn();
    });
  });
}

function showOverviewEditor(projectId, currentText, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-wide">
    <div class="modal-title">Главная статья вики</div>
    <div class="form-group">
      <label class="form-label">Markdown — термины пиши как [[Название страницы]]</label>
      <textarea class="form-textarea" id="dev-wiki-text"></textarea>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" data-cancel>Отмена</button>
      <button class="btn-primary" data-save>Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#dev-wiki-text').value = currentText;
  const close = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) close(); });
  overlay.querySelector('[data-cancel]').addEventListener('click', close);
  overlay.querySelector('[data-save]').addEventListener('click', async () => {
    const overview = overlay.querySelector('#dev-wiki-text').value;
    await invoke('update_dev_project', { id: projectId, name: null, icon: null, overview });
    close();
    reloadFn();
  });
}
