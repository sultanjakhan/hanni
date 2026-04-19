// ── sport-templates.js — Workout templates pane for Sports tab ──
import { invoke } from './state.js';
import { renderTemplateCard } from './sport-template-card.js';
import { WORKOUT_TYPES, DIFFS, matchType, matchDiff, matchSearch, matchMuscle, collectMuscleGroups } from './sport-template-filters.js';
import { MUSCLE_GROUPS } from './sport-catalog-filters.js';

function chips(items, cur, group) {
  return items.map(o =>
    `<button class="rf-chip${cur === o.id ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}

export async function renderTemplatesPane(el) {
  const F = { type: 'all', diff: 'all', muscle: 'all', fav: false, q: '' };
  let allTemplates = [], panelOpen = false, built = false;

  async function loadData() {
    allTemplates = await invoke('get_workout_templates', { search: null }).catch(() => []);
  }

  function getFiltered() {
    return allTemplates.filter(t => matchType(t, F.type) && matchDiff(t, F.diff)
      && matchSearch(t, F.q) && matchMuscle(t, F.muscle) && (!F.fav || t.favorite === 1));
  }

  function buildShell() {
    el.innerHTML = `<div class="recipe-pane">
      <div class="recipe-filter-bar">
        <button class="rf-toggle" title="Фильтр">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M1.5 2h13L9.5 8.5V13l-3 1.5V8.5z"/></svg>
          <span class="rf-badge" style="display:none"></span></button>
        <input class="recipe-search" type="text" placeholder="Поиск шаблонов...">
        <button class="btn-primary recipe-add-btn">+ Шаблон</button>
      </div>
      <div class="rf-panel" style="display:none"></div>
      <div class="recipe-grid"></div></div>`;
    el.querySelector('.rf-toggle').onclick = () => {
      panelOpen = !panelOpen;
      el.querySelector('.rf-panel').style.display = panelOpen ? '' : 'none';
    };
    el.querySelector('.recipe-search').oninput = (e) => { F.q = e.target.value.trim().toLowerCase(); updateGrid(); };
    el.querySelector('.recipe-add-btn').onclick = async () => {
      const { showAddTemplateModal } = await import('./sport-template-add.js');
      showAddTemplateModal(fullReload);
    };
  }

  function updatePanel() {
    const panel = el.querySelector('.rf-panel');
    const usedMuscles = collectMuscleGroups(allTemplates);
    const muscleChips = MUSCLE_GROUPS.filter(m => m.id === 'all' || usedMuscles.has(m.id));
    panel.innerHTML = `
      <div class="rf-section"><span class="rf-title">Тип</span>${chips(WORKOUT_TYPES, F.type, 'type')}</div>
      <div class="rf-section"><span class="rf-title">Сложность</span>${chips(DIFFS, F.diff, 'diff')}</div>
      ${muscleChips.length > 1 ? `<div class="rf-section"><span class="rf-title">Мышцы</span>${chips(muscleChips, F.muscle, 'muscle')}</div>` : ''}
      <div class="rf-section">
        <button class="rf-chip${F.fav ? ' active' : ''}" data-group="fav" data-val="toggle">★ Избранное</button>
      </div>`;
    panel.querySelectorAll('.rf-chip').forEach(btn => btn.onclick = () => {
      const g = btn.dataset.group, v = btn.dataset.val;
      if (g === 'fav') F.fav = !F.fav;
      else F[g] = v;
      updatePanel(); updateGrid(); updateBadge();
    });
    updateBadge();
  }

  function updateBadge() {
    const ac = [F.type !== 'all', F.diff !== 'all', F.muscle !== 'all', F.fav].filter(Boolean).length;
    const badge = el.querySelector('.rf-badge'), toggle = el.querySelector('.rf-toggle');
    badge.textContent = ac; badge.style.display = ac ? '' : 'none';
    toggle.classList.toggle('rf-active', ac > 0);
  }

  function updateGrid() {
    const list = getFiltered(), grid = el.querySelector('.recipe-grid');
    grid.innerHTML = '';
    if (!list.length) { grid.innerHTML = '<div class="uni-empty">Нет шаблонов. Создайте первый!</div>'; return; }
    for (const t of list) {
      const card = renderTemplateCard(t);
      card.onclick = async () => {
        const { showTemplateDetail } = await import('./sport-template-modals.js');
        showTemplateDetail(t.id, fullReload);
      };
      grid.appendChild(card);
    }
  }

  async function fullReload() {
    await loadData();
    if (!built) { buildShell(); built = true; }
    updatePanel(); updateGrid();
  }
  await fullReload();
}
