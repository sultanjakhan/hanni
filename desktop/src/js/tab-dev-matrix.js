// ── tab-dev-matrix.js — competency matrix tree (area → competency → skill) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { scoreTier, levelBarHtml, levelBadgeHtml } from './dev-level.js';
import { matrixToolbarHtml, wireMatrixToolbar } from './dev-matrix-search.js';
import { cefrChipHtml, wireCefrChips } from './dev-cefr.js';

function avg(nums) {
  const vals = nums.filter(n => n != null);
  return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : null;
}

export async function renderMatrixPane(el, projectId, reloadFn) {
  if (S._devOpenNode) {
    const { renderCompetencyPage } = await import('./tab-dev-competency-page.js');
    await renderCompetencyPage(el, S._devOpenNode, projectId, reloadFn);
    return;
  }
  const nodes = await invoke('get_dev_nodes', { projectId }).catch(() => []);
  const byParent = new Map();
  for (const n of nodes) {
    const k = n.parent_id || 0;
    if (!byParent.has(k)) byParent.set(k, []);
    byParent.get(k).push(n);
  }
  const children = (id) => byParent.get(id || 0) || [];
  const compAvg = (cid) => avg(children(cid).filter(s => s.score > 0).map(s => s.score));

  let html = matrixToolbarHtml() + '<div class="dev-matrix">';
  for (const area of children(0).filter(n => n.kind === 'area')) {
    const comps = children(area.id).filter(n => n.kind === 'competency');
    const aAvg = avg(comps.map(c => compAvg(c.id)));
    html += `<details class="dev-mx-area">
      <summary class="dev-mx-row dev-mx-area-head">
        <span class="dev-mx-name">${escapeHtml(area.name)}</span>
        <span class="dev-mx-count">${comps.length}</span>
        ${levelBadgeHtml(aAvg)}
        <button class="dev-mx-act dev-mx-ren" data-id="${area.id}" data-name="${escapeHtml(area.name)}" title="Переименовать">✎</button>
        <button class="dev-mx-act dev-mx-del" data-id="${area.id}" title="Удалить">×</button>
      </summary>`;
    for (const comp of comps) {
      const cAvg = compAvg(comp.id);
      const empty = (comp.theory || '').trim() ? '0' : '1';
      html += `<details class="dev-mx-comp" data-empty="${empty}">
        <summary class="dev-mx-row dev-mx-comp-head">
          <span class="dev-mx-name dev-mx-open" data-id="${comp.id}" title="Открыть страницу">${escapeHtml(comp.name)}</span>
          ${cefrChipHtml(comp.level, comp.id)}
          ${levelBadgeHtml(cAvg)}
          <button class="dev-mx-act dev-mx-open dev-mx-open-btn" data-id="${comp.id}" title="Открыть страницу">📖</button>
          <button class="dev-mx-act dev-mx-del" data-id="${comp.id}" title="Удалить">×</button>
        </summary>
        <div class="dev-mx-skills">`;
      for (const sk of children(comp.id).filter(n => n.kind === 'skill')) {
        html += `<div class="dev-mx-skill">
          <button class="dev-mx-prio${sk.priority ? ' on' : ''}" data-id="${sk.id}" title="Приоритет изучения">⚑</button>
          <span class="dev-mx-name dev-mx-ren" data-id="${sk.id}" data-name="${escapeHtml(sk.name)}">${escapeHtml(sk.name)}</span>
          ${sk.material ? `<a class="dev-mx-material" data-href="${escapeHtml(sk.material)}" title="Материал">🔗</a>` : ''}
          ${levelBarHtml(sk.score || null)}
          <span class="dev-mx-skill-score" data-id="${sk.id}" data-tier="${scoreTier(sk.score || null)}">${sk.score || 0}</span>
          <button class="dev-mx-act dev-mx-del" data-id="${sk.id}" title="Удалить">×</button>
        </div>`;
      }
      html += `<button class="dev-mx-add" data-parent="${comp.id}" data-kind="skill">+ навык</button>
        </div></details>`;
    }
    html += `<button class="dev-mx-add" data-parent="${area.id}" data-kind="competency">+ компетенция</button>
      </details>`;
  }
  html += `<button class="dev-mx-add dev-mx-add-area" data-kind="area">+ область знаний</button></div>`;
  el.innerHTML = html;
  wireMatrix(el, projectId, reloadFn);
  wireMatrixToolbar(el);
  wireCefrChips(el, reloadFn);
}

function wireMatrix(el, projectId, reloadFn) {
  const stop = (e) => { e.preventDefault(); e.stopPropagation(); };

  el.querySelectorAll('.dev-mx-open').forEach(b => b.addEventListener('click', (e) => {
    stop(e); S._devOpenNode = parseInt(b.dataset.id); reloadFn();
  }));

  el.querySelectorAll('.dev-mx-prio').forEach(b => b.addEventListener('click', async (e) => {
    stop(e);
    await invoke('update_dev_node', {
      id: parseInt(b.dataset.id), name: null, score: null, theory: null,
      material: null, priority: b.classList.contains('on') ? 0 : 1, level: null,
    });
    reloadFn();
  }));

  el.querySelectorAll('.dev-mx-del').forEach(b => b.addEventListener('click', async (e) => {
    stop(e);
    if (!confirm('Удалить вместе со всем содержимым?')) return;
    await invoke('delete_dev_node', { id: parseInt(b.dataset.id) });
    reloadFn();
  }));

  el.querySelectorAll('.dev-mx-ren').forEach(b => b.addEventListener('click', (e) => {
    stop(e);
    const id = parseInt(b.dataset.id);
    nodeNameModal('Переименовать', b.dataset.name, async (name) => {
      await invoke('update_dev_node', { id, name, score: null, theory: null, material: null, priority: null, level: null });
      reloadFn();
    });
  }));

  el.querySelectorAll('.dev-mx-add').forEach(b => b.addEventListener('click', (e) => {
    stop(e);
    const kind = b.dataset.kind;
    const parentId = b.dataset.parent ? parseInt(b.dataset.parent) : null;
    const label = kind === 'area' ? 'область знаний' : kind === 'competency' ? 'компетенцию' : 'навык';
    nodeNameModal(`Добавить ${label}`, '', async (name) => {
      await invoke('create_dev_node', { projectId, parentId, kind, name });
      reloadFn();
    });
  }));

  el.querySelectorAll('.dev-mx-material').forEach(a => a.addEventListener('click', (e) => {
    stop(e);
    if (a.dataset.href) invoke('open_url', { url: a.dataset.href });
  }));

  el.querySelectorAll('.dev-mx-skill-score').forEach(s => s.addEventListener('click', (e) => {
    e.stopPropagation();
    editScore(s, reloadFn);
  }));
}

function editScore(span, reloadFn) {
  const id = parseInt(span.dataset.id);
  const inp = document.createElement('input');
  inp.type = 'number'; inp.min = '0'; inp.max = '10';
  inp.value = parseInt(span.textContent) || 0;
  inp.className = 'dev-mx-score-input';
  span.replaceWith(inp);
  inp.focus(); inp.select();
  let done = false;
  inp.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') inp.blur();
    if (e.key === 'Escape') { done = true; reloadFn(); }
  });
  inp.addEventListener('blur', async () => {
    if (done) return;
    done = true;
    let v = parseInt(inp.value);
    if (isNaN(v)) v = 0;
    v = Math.max(0, Math.min(10, v));
    await invoke('update_dev_node', { id, name: null, score: v, theory: null, material: null, priority: null, level: null });
    reloadFn();
  });
}

function nodeNameModal(title, current, onSave) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">${escapeHtml(title)}</div>
    <div class="form-group"><label class="form-label">Название</label>
      <input class="form-input" id="dev-node-name"></div>
    <div class="modal-actions">
      <button class="btn-secondary" data-cancel>Отмена</button>
      <button class="btn-primary" data-save>Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  const inp = overlay.querySelector('#dev-node-name');
  inp.value = current || '';
  inp.focus();
  const close = () => overlay.remove();
  const save = async () => {
    const v = inp.value.trim();
    if (!v) return;
    close();
    await onSave(v);
  };
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  overlay.querySelector('[data-cancel]').addEventListener('click', close);
  overlay.querySelector('[data-save]').addEventListener('click', save);
  inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') save(); });
}
