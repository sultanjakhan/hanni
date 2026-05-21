// ── tab-dev-wiki.js — "main article" wiki pane for dev projects ──

import { S, invoke } from './state.js';
import { renderWikiMarkdown, buildNodeIndex } from './wiki-markdown.js';
import { savePaneState } from './db-view/unified-layout.js';

export async function renderWikiPane(el, projectId, reloadFn) {
  const [projects, nodes] = await Promise.all([
    invoke('get_dev_projects').catch(() => []),
    invoke('get_dev_nodes', { projectId }).catch(() => []),
  ]);
  const project = projects.find(p => p.id === projectId);
  const overview = project?.overview || '';
  const nodeIndex = buildNodeIndex(nodes);

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
    <div class="dev-wiki-content">${renderWikiMarkdown(overview, nodeIndex)}</div>
  </div>`;
  el.querySelector('.dev-wiki-edit')
    ?.addEventListener('click', () => showOverviewEditor(projectId, overview, reloadFn));
  wireWikiLinks(el, reloadFn, projectId);
}

const GLOSSARY_AREA = 'Фреймворки и термины';

/** Create a stub competency for an unresolved [[term]] under the glossary area. */
async function createTermStub(projectId, name) {
  const nodes = await invoke('get_dev_nodes', { projectId }).catch(() => []);
  let areaId = nodes.find(n => n.kind === 'area' && n.name === GLOSSARY_AREA)?.id;
  if (areaId == null) {
    areaId = await invoke('create_dev_node', { projectId, parentId: null, kind: 'area', name: GLOSSARY_AREA });
  }
  return invoke('create_dev_node', { projectId, parentId: areaId, kind: 'competency', name });
}

function openNode(id, reloadFn) {
  S._devOpenNode = id;
  S._unifiedPane.development = 'skills';
  savePaneState('development', 'skills');
  reloadFn();
}

/** Wire [[wiki-links]]: resolved ones open the page, unresolved create a stub. */
export function wireWikiLinks(el, reloadFn, projectId) {
  el.querySelectorAll('.wiki-link[data-node-id]').forEach(a => {
    a.addEventListener('click', (e) => { e.preventDefault(); openNode(parseInt(a.dataset.nodeId), reloadFn); });
  });
  el.querySelectorAll('.wiki-link-red[data-node-name]').forEach(a => {
    a.addEventListener('click', async (e) => {
      e.preventDefault();
      if (projectId == null) return;
      const id = await createTermStub(projectId, a.dataset.nodeName);
      if (id != null) openNode(id, reloadFn);
    });
  });
}

function showOverviewEditor(projectId, currentText, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-wide">
    <div class="modal-title">Главная статья вики</div>
    <div class="form-group">
      <label class="form-label">Markdown — термины пиши как [[Название компетенции]]</label>
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
