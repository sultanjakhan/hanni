// ── dev-matrix-search.js — toolbar (search + chips + expand-all) for the matrix ──

import { S } from './state.js';

function mx() {
  if (!S._devMx) S._devMx = { q: '', weak: false, prio: false, noTheory: false };
  return S._devMx;
}

export function matrixToolbarHtml() {
  const s = mx();
  const chip = (key, label) => `<button class="pill dev-mx-chip${s[key] ? ' active' : ''}" data-chip="${key}">${label}</button>`;
  return `<div class="dev-mx-toolbar">
    <input class="dev-mx-search" type="text" placeholder="Поиск по матрице…" value="${s.q.replace(/"/g, '&quot;')}">
    <div class="dev-mx-chips">
      ${chip('weak', 'Слабые')}${chip('prio', '⚑ Приоритет')}${chip('noTheory', 'Без теории')}
    </div>
    <button class="dev-mx-expand" type="button">Развернуть всё</button>
  </div>`;
}

/** Show/hide nodes per current search + chip state; auto-expand while filtering. */
export function applyMatrixFilter(el) {
  const s = mx();
  const q = s.q.trim().toLowerCase();
  const skillChips = s.weak || s.prio;
  const filtering = !!q || skillChips || s.noTheory;

  el.querySelectorAll('.dev-mx-area').forEach(area => {
    const aName = area.querySelector('.dev-mx-area-head .dev-mx-name')?.textContent.toLowerCase() || '';
    const aMatch = !!q && aName.includes(q);
    let areaVisible = false;

    area.querySelectorAll('.dev-mx-comp').forEach(comp => {
      const cName = comp.querySelector('.dev-mx-comp-head .dev-mx-name')?.textContent.toLowerCase() || '';
      const cMatch = !!q && cName.includes(q);
      const empty = comp.dataset.empty === '1';
      let compHasSkill = false;

      comp.querySelectorAll('.dev-mx-skill').forEach(sk => {
        const sName = sk.querySelector('.dev-mx-name')?.textContent.toLowerCase() || '';
        const score = parseInt(sk.querySelector('.dev-mx-skill-score')?.textContent) || 0;
        const prio = sk.querySelector('.dev-mx-prio')?.classList.contains('on');
        const textOk = !q || sName.includes(q) || cMatch || aMatch;
        const chipOk = !skillChips || (s.weak && score > 0 && score < 4) || (s.prio && prio);
        const vis = textOk && chipOk && !s.noTheory;
        sk.style.display = vis ? '' : 'none';
        if (vis) compHasSkill = true;
      });

      let compVisible;
      if (!filtering) compVisible = true;
      else if (s.noTheory) compVisible = empty && (!q || cMatch || aMatch);
      else if (skillChips) compVisible = compHasSkill;
      else compVisible = compHasSkill || cMatch || aMatch;

      comp.style.display = compVisible ? '' : 'none';
      if (compVisible) {
        areaVisible = true;
        if (filtering) comp.open = true;
      }
    });

    if (!filtering) areaVisible = true;
    else if (aMatch) areaVisible = true;
    area.style.display = areaVisible ? '' : 'none';
    if (areaVisible && filtering) area.open = true;
  });

  // Hide add-buttons while filtering to keep results clean.
  el.querySelectorAll('.dev-mx-add').forEach(b => { b.style.display = filtering ? 'none' : ''; });
}

export function wireMatrixToolbar(el) {
  const s = mx();
  const search = el.querySelector('.dev-mx-search');
  search?.addEventListener('input', () => { s.q = search.value; applyMatrixFilter(el); });

  el.querySelectorAll('.dev-mx-chip').forEach(b => b.addEventListener('click', () => {
    const k = b.dataset.chip;
    s[k] = !s[k];
    b.classList.toggle('active', s[k]);
    applyMatrixFilter(el);
  }));

  const expand = el.querySelector('.dev-mx-expand');
  expand?.addEventListener('click', () => {
    const areas = [...el.querySelectorAll('.dev-mx-area')];
    const anyOpen = areas.some(a => a.open);
    areas.forEach(a => { a.open = !anyOpen; });
    expand.textContent = anyOpen ? 'Развернуть всё' : 'Свернуть всё';
  });

  applyMatrixFilter(el);
}
