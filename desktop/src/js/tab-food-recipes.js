// ── tab-food-recipes.js — Recipe book pane for Food tab ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderCard } from './food-recipe-card.js';
import {
  MEALS, DIFFS, CAT_LABELS, CAT_ORDER, getCuisineChips, loadCatalog,
  getBlacklist, matchBL, matchMeal, matchCuisine, matchDiff,
  matchSearch, matchIngr, sortRecipes, collectIngredients,
} from './food-recipe-filters.js';

function chips(items, cur, group) {
  return items.map(o =>
    `<button class="rf-chip${cur === o.id ? ' active' : ''}" data-group="${group}" data-val="${o.id}">${o.label}</button>`
  ).join('');
}

function accordionRow(title, group, content, activeCount) {
  const badge = activeCount ? `<span class="rf-acc-badge">${activeCount}</span>` : '';
  return `<div class="rf-acc" data-acc="${group}">
    <div class="rf-acc-header">${title}${badge}<span class="rf-acc-arrow">▸</span></div>
    <div class="rf-acc-body" style="display:none">${content}</div></div>`;
}

export async function renderRecipesPane(el) {
  const F = { meal: 'all', cuisine: 'all', diff: 'all', fav: false, q: '' };
  const selIngr = new Set();
  let allRecipes = [], blacklist = [], panelOpen = false, built = false;

  async function loadData() {
    [allRecipes, blacklist] = await Promise.all([
      invoke('get_recipes', { search: null }).catch(() => []), getBlacklist(), loadCatalog(),
    ]);
  }

  function getFiltered() {
    let list = allRecipes.filter(r => !matchBL(r, blacklist) && matchMeal(r, F.meal)
      && matchCuisine(r, F.cuisine) && matchDiff(r, F.diff)
      && matchSearch(r, F.q) && (!F.fav || r.favorite === 1)
      && matchIngr(r, selIngr));
    return sortRecipes(list, 'name');
  }

  function buildShell() {
    el.innerHTML = `<div class="recipe-pane">
      <div class="recipe-filter-bar">
        <button class="rf-toggle" title="Фильтр">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M1.5 2h13L9.5 8.5V13l-3 1.5V8.5z"/></svg>
          <span class="rf-badge" style="display:none"></span></button>
        <input class="recipe-search" type="text" placeholder="Поиск...">
        <button class="btn-primary recipe-add-btn">+ Рецепт</button>
      </div>
      <div class="rf-panel" style="display:none"></div>
      <div class="recipe-grid"></div></div>`;
    el.querySelector('.rf-toggle').onclick = () => {
      panelOpen = !panelOpen;
      el.querySelector('.rf-panel').style.display = panelOpen ? '' : 'none';
    };
    el.querySelector('.recipe-search').oninput = (e) => {
      F.q = e.target.value.trim().toLowerCase(); updateGrid();
    };
    el.querySelector('.recipe-add-btn').onclick = async () => {
      const { showAddRecipeModal } = await import('./food-recipe-modals.js');
      showAddRecipeModal(fullReload);
    };
  }

  async function updatePanel() {
    const panel = el.querySelector('.rf-panel');
    const cuisineChips = await getCuisineChips();
    const grouped = collectIngredients(allRecipes.filter(r => !matchBL(r, blacklist)));
    const ingrContent = CAT_ORDER.filter(c => grouped[c]?.length).map(cat =>
      `<div class="rf-section"><span class="rf-title rf-title-${cat}">${CAT_LABELS[cat]}</span>${grouped[cat].map(i =>
        `<button class="rf-chip rf-ingr-chip ingr-cat-${cat}${selIngr.has(i.name.toLowerCase()) ? ' active' : ''}" data-group="ingr" data-val="${escapeHtml(i.name.toLowerCase())}">${escapeHtml(i.name)}</button>`
      ).join('')}</div>`).join('');
    const mealActive = F.meal !== 'all' ? 1 : 0;
    const cuisineActive = F.cuisine !== 'all' ? 1 : 0;
    const diffActive = F.diff !== 'all' ? 1 : 0;
    const ingrActive = selIngr.size || 0;
    panel.innerHTML = `
      ${accordionRow('Приём пищи', 'meal', chips(MEALS, F.meal, 'meal'), mealActive)}
      ${accordionRow('Кухня', 'cuisine', chips(cuisineChips, F.cuisine, 'cuisine'), cuisineActive)}
      ${accordionRow('Сложность', 'diff', chips(DIFFS, F.diff, 'diff'), diffActive)}
      ${accordionRow('Ингредиенты', 'ingr', ingrContent, ingrActive)}
      <div class="rf-acc-fav">
        <button class="rf-chip${F.fav ? ' active' : ''}" data-group="fav" data-val="toggle">★ Избранное</button>
      </div>`;
    // Accordion toggle
    panel.querySelectorAll('.rf-acc-header').forEach(hdr => hdr.onclick = () => {
      const acc = hdr.parentElement;
      const body = acc.querySelector('.rf-acc-body');
      const open = body.style.display !== 'none';
      body.style.display = open ? 'none' : '';
      acc.querySelector('.rf-acc-arrow').textContent = open ? '▸' : '▾';
    });
    // Chip clicks
    panel.querySelectorAll('.rf-chip').forEach(btn => btn.onclick = async () => {
      const g = btn.dataset.group, v = btn.dataset.val;
      if (g === 'fav') F.fav = !F.fav;
      else if (g === 'ingr') { selIngr.has(v) ? selIngr.delete(v) : selIngr.add(v); }
      else F[g] = v;
      await updatePanel(); updateGrid(); updateBadge();
    });
    updateBadge();
  }

  function updateBadge() {
    const ac = [F.meal !== 'all', F.cuisine !== 'all', F.diff !== 'all', F.fav, selIngr.size > 0].filter(Boolean).length;
    const badge = el.querySelector('.rf-badge'), toggle = el.querySelector('.rf-toggle');
    badge.textContent = ac; badge.style.display = ac ? '' : 'none';
    toggle.classList.toggle('rf-active', ac > 0);
  }

  function updateGrid() {
    const list = getFiltered(), grid = el.querySelector('.recipe-grid');
    grid.innerHTML = '';
    if (!list.length) { grid.innerHTML = '<div class="uni-empty">Нет рецептов</div>'; return; }
    for (const r of list) {
      const card = renderCard(
        r,
        (ingr) => {
          F.q = ingr.toLowerCase(); el.querySelector('.recipe-search').value = F.q; updateGrid();
        },
        async (id) => {
          const { duplicateRecipe } = await import('./food-recipe-add.js');
          try { await duplicateRecipe(id, fullReload); } catch (e) { alert('Ошибка: ' + (e.message || e)); }
        },
      );
      card.onclick = async () => {
        const { showRecipeDetail } = await import('./food-recipe-modals.js');
        showRecipeDetail(parseInt(card.dataset.id), fullReload);
      };
      grid.appendChild(card);
    }
  }

  async function fullReload() {
    await loadData();
    if (!built) { buildShell(); built = true; }
    await updatePanel(); updateGrid();
  }
  await fullReload();
}
