// ── tab-food-recipes.js — Recipe book pane for Food tab ──
import { invoke } from './state.js';
import { escapeHtml, chips } from './utils.js';
import { renderCard, getIngrNames } from './food-recipe-card.js';
import { showBlacklistContextMenu } from './food-blacklist-menu.js';
import {
  MEALS, DIFFS, SORTS, CAT_LABELS, CAT_ORDER, getCuisineChips, loadCatalog,
  getBlacklist, matchBL, recipeBlockLevel, matchMeal, matchCuisine, matchDiff,
  matchSearch, matchIngr, matchRating, matchTime, sortRecipes, collectIngredients,
} from './food-recipe-filters.js';

const RATINGS = [{ id: 4, label: '★ 4+' }, { id: 5, label: '★ 5' }];
const TIMES = [{ id: 15, label: '≤15 мин' }, { id: 30, label: '≤30 мин' }, { id: 60, label: '≤60 мин' }];

function accordionRow(title, group, content, activeCount) {
  const badge = activeCount ? `<span class="rf-acc-badge">${activeCount}</span>` : '';
  return `<div class="rf-acc" data-acc="${group}">
    <div class="rf-acc-header">${title}${badge}<span class="rf-acc-arrow">▸</span></div>
    <div class="rf-acc-body" style="display:none">${content}</div></div>`;
}

export async function renderRecipesPane(el) {
  const F = { meal: 'all', cuisine: 'all', diff: 'all', fav: false, cookable: false, q: '', sort: 'name', minRating: 0, maxTime: 0 };
  const selIngr = new Set();
  let allRecipes = [], blacklist = [], cuisineChips = [], panelOpen = false, built = false;

  async function loadData() {
    let products;
    [allRecipes, blacklist, , products] = await Promise.all([
      invoke('get_recipes', { search: null }).catch(() => []), getBlacklist(), loadCatalog(),
      invoke('get_products', {}).catch(() => []),
    ]);
    // Tag each recipe with how many of its ingredients are NOT in the fridge,
    // so cards can show "есть всё / не хватает N" and the "cookable" filter works.
    const have = new Set((products || []).map(p => String(p.name || '').trim().toLowerCase()));
    for (const r of allRecipes) r._missing = getIngrNames(r).filter(n => !have.has(n.trim().toLowerCase())).length;
  }

  function getFiltered() {
    let list = allRecipes.filter(r => !matchBL(r, blacklist) && matchMeal(r, F.meal)
      && matchCuisine(r, F.cuisine) && matchDiff(r, F.diff)
      && matchSearch(r, F.q) && (!F.fav || r.favorite === 1)
      && matchIngr(r, selIngr) && matchRating(r, F.minRating) && matchTime(r, F.maxTime)
      && (!F.cookable || r._missing === 0));
    const sorted = sortRecipes(list, F.sort);
    // soft-blocked recipes ("не люблю") sink to the bottom of the list.
    const soft = [], normal = [];
    for (const r of sorted) (recipeBlockLevel(r, blacklist) === 'soft' ? soft : normal).push(r);
    return [...normal, ...soft];
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
      <div class="rf-popup-overlay" style="display:none"><div class="rf-panel"></div></div>
      <div class="rf-active-bar"></div>
      <div class="recipe-grid"></div></div>`;
    const popup = el.querySelector('.rf-popup-overlay');
    const closePopup = () => { panelOpen = false; popup.style.display = 'none'; };
    el.querySelector('.rf-toggle').onclick = () => { panelOpen = !panelOpen; popup.style.display = panelOpen ? 'flex' : 'none'; };
    popup.onclick = (e) => { if (e.target === popup) closePopup(); };
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && panelOpen) closePopup(); });
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
    cuisineChips = await getCuisineChips();
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
      <div class="rf-sort-row"><span class="rf-sort-label">Сортировка</span>${chips(SORTS, F.sort, 'sort')}</div>
      ${accordionRow('Приём пищи', 'meal', chips(MEALS, F.meal, 'meal'), mealActive)}
      ${accordionRow('Кухня', 'cuisine', chips(cuisineChips, F.cuisine, 'cuisine'), cuisineActive)}
      ${accordionRow('Сложность', 'diff', chips(DIFFS, F.diff, 'diff'), diffActive)}
      ${accordionRow('Ингредиенты', 'ingr', ingrContent, ingrActive)}
      ${accordionRow('Оценка и время', 'extra', `${chips(RATINGS, F.minRating, 'rating')}${chips(TIMES, F.maxTime, 'time')}`, (F.minRating ? 1 : 0) + (F.maxTime ? 1 : 0))}
      <div class="rf-acc-fav">
        <button class="rf-chip${F.fav ? ' active' : ''}" data-group="fav" data-val="toggle">★ Избранное</button>
        <button class="rf-chip${F.cookable ? ' active' : ''}" data-group="cookable" data-val="toggle">🍲 Могу приготовить</button>
        <button class="rf-reset" data-group="reset">Сбросить</button>
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
    panel.querySelectorAll('.rf-chip, .rf-reset').forEach(btn => btn.onclick = async () => {
      const g = btn.dataset.group, v = btn.dataset.val;
      if (g === 'fav') F.fav = !F.fav;
      else if (g === 'cookable') F.cookable = !F.cookable;
      else if (g === 'ingr') { selIngr.has(v) ? selIngr.delete(v) : selIngr.add(v); }
      else if (g === 'rating') F.minRating = (F.minRating === +v ? 0 : +v);
      else if (g === 'time') F.maxTime = (F.maxTime === +v ? 0 : +v);
      else if (g === 'sort') F.sort = v;
      else if (g === 'reset') { F.meal = 'all'; F.cuisine = 'all'; F.diff = 'all'; F.fav = false; F.cookable = false; F.minRating = 0; F.maxTime = 0; F.sort = 'name'; selIngr.clear(); }
      else F[g] = v;
      await updatePanel(); updateGrid(); updateBadge(); updateActive();
    });
    updateBadge();
  }

  function updateActive() {
    const bar = el.querySelector('.rf-active-bar');
    if (!bar) return;
    const items = [];
    if (F.meal !== 'all') items.push(['meal', MEALS.find(m => m.id === F.meal)?.label || F.meal]);
    if (F.cuisine !== 'all') items.push(['cuisine', cuisineChips.find(c => c.id === F.cuisine)?.label || F.cuisine]);
    if (F.diff !== 'all') items.push(['diff', DIFFS.find(d => d.id === F.diff)?.label || F.diff]);
    if (F.minRating) items.push(['rating', `★${F.minRating}+`]);
    if (F.maxTime) items.push(['time', `≤${F.maxTime} мин`]);
    if (F.fav) items.push(['fav', '★ Избранное']);
    if (F.cookable) items.push(['cookable', '🍲 Могу приготовить']);
    for (const ingr of selIngr) items.push([`ingr:${ingr}`, ingr]);
    bar.innerHTML = items.map(([k, label]) => `<button class="rf-active-chip" data-k="${escapeHtml(k)}">${escapeHtml(label)} ×</button>`).join('');
    bar.querySelectorAll('.rf-active-chip').forEach(c => c.onclick = async () => {
      const k = c.dataset.k;
      if (k.startsWith('ingr:')) selIngr.delete(k.slice(5));
      else if (k === 'meal') F.meal = 'all'; else if (k === 'cuisine') F.cuisine = 'all';
      else if (k === 'diff') F.diff = 'all'; else if (k === 'rating') F.minRating = 0;
      else if (k === 'time') F.maxTime = 0; else if (k === 'fav') F.fav = false; else if (k === 'cookable') F.cookable = false;
      await updatePanel(); updateGrid(); updateBadge(); updateActive();
    });
  }

  function updateBadge() {
    const ac = [F.meal !== 'all', F.cuisine !== 'all', F.diff !== 'all', F.fav, F.cookable, selIngr.size > 0, F.minRating > 0, F.maxTime > 0].filter(Boolean).length;
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
      card.oncontextmenu = (e) => {
        e.preventDefault();
        showBlacklistContextMenu(e.clientX, e.clientY, { type: 'recipe', value: r.name }, fullReload);
      };
      const lvl = recipeBlockLevel(r, blacklist);
      if (lvl === 'soft') card.classList.add('recipe-card--soft');
      else if (lvl === 'love') card.classList.add('recipe-card--love');
      grid.appendChild(card);
    }
  }

  async function fullReload() {
    await loadData();
    if (!built) { buildShell(); built = true; }
    await updatePanel(); updateGrid(); updateActive();
  }
  await fullReload();
}
