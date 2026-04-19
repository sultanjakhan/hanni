// ── sport-catalog.js — Exercise catalog pane for Sports tab ──
import { invoke } from './state.js';
import { renderExerciseCard } from './sport-catalog-card.js';
import { MUSCLE_GROUPS, EXERCISE_TYPES, matchMuscle, matchType, matchSearch } from './sport-catalog-filters.js';

function chips(items, cur, group) {
  return items.map(o =>
    `<button class="rf-chip${cur === o.id ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}

export async function renderCatalogPane(el) {
  const F = { muscle: 'all', type: 'all', q: '' };
  let allExercises = [], panelOpen = false, built = false;

  async function loadData() {
    allExercises = await invoke('get_exercise_catalog', { search: null }).catch(() => []);
  }

  function getFiltered() {
    return allExercises.filter(ex => matchMuscle(ex, F.muscle) && matchType(ex, F.type) && matchSearch(ex, F.q));
  }

  function buildShell() {
    el.innerHTML = `<div class="recipe-pane">
      <div class="recipe-filter-bar">
        <button class="rf-toggle" title="Фильтр">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M1.5 2h13L9.5 8.5V13l-3 1.5V8.5z"/></svg>
          <span class="rf-badge" style="display:none"></span></button>
        <input class="recipe-search" type="text" placeholder="Поиск упражнений...">
        <button class="btn-primary recipe-add-btn">+ Упражнение</button>
      </div>
      <div class="rf-panel" style="display:none"></div>
      <div class="recipe-grid"></div></div>`;
    el.querySelector('.rf-toggle').onclick = () => {
      panelOpen = !panelOpen;
      el.querySelector('.rf-panel').style.display = panelOpen ? '' : 'none';
    };
    el.querySelector('.recipe-search').oninput = (e) => { F.q = e.target.value.trim().toLowerCase(); updateGrid(); };
    el.querySelector('.recipe-add-btn').onclick = async () => {
      const { showExerciseModal } = await import('./sport-catalog-modal.js');
      showExerciseModal(fullReload);
    };
  }

  function updatePanel() {
    const panel = el.querySelector('.rf-panel');
    const mActive = F.muscle !== 'all' ? 1 : 0;
    const tActive = F.type !== 'all' ? 1 : 0;
    panel.innerHTML = `
      <div class="rf-section"><span class="rf-title">Группа мышц</span>${chips(MUSCLE_GROUPS, F.muscle, 'muscle')}</div>
      <div class="rf-section"><span class="rf-title">Тип</span>${chips(EXERCISE_TYPES, F.type, 'type')}</div>`;
    panel.querySelectorAll('.rf-chip').forEach(btn => btn.onclick = () => {
      const g = btn.dataset.group, v = btn.dataset.val;
      F[g] = v;
      updatePanel(); updateGrid(); updateBadge();
    });
    updateBadge();
  }

  function updateBadge() {
    const ac = [F.muscle !== 'all', F.type !== 'all'].filter(Boolean).length;
    const badge = el.querySelector('.rf-badge'), toggle = el.querySelector('.rf-toggle');
    badge.textContent = ac; badge.style.display = ac ? '' : 'none';
    toggle.classList.toggle('rf-active', ac > 0);
  }

  function updateGrid() {
    const list = getFiltered(), grid = el.querySelector('.recipe-grid');
    grid.innerHTML = '';
    if (!list.length) { grid.innerHTML = '<div class="uni-empty">Нет упражнений. Добавьте первое!</div>'; return; }
    for (const ex of list) {
      const card = renderExerciseCard(ex);
      card.onclick = async () => {
        const { showExerciseModal } = await import('./sport-catalog-modal.js');
        showExerciseModal(fullReload, ex);
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
