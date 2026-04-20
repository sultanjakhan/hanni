// ── food-product-catalog.js — Drill-down product wiki: category → subgroup → product ──
import { CAT_LABELS, invalidateCatalogCache, loadCatalog, getBlacklist,
  isIngredientBlocked, isCategoryBlocked } from './food-recipe-filters.js';
import { showProductModal } from './food-product-modal.js';
import { renderProductCard } from './food-product-card.js';
import { renderCategoryGrid, renderSubgroupGrid, renderProductsGrid, renderBreadcrumb } from './food-product-views.js';

let allProducts = [], blacklist = [], rootEl;
const nav = { view: 'category', category: null, subgroup: null, q: '' };

async function loadData() {
  invalidateCatalogCache();
  [allProducts, blacklist] = await Promise.all([
    loadCatalog().catch(() => []),
    getBlacklist().catch(() => []),
  ]);
}

function buildShell(el) {
  el.innerHTML = `<div class="recipe-pane">
    <div class="recipe-filter-bar">
      <button class="rf-back" style="display:none;font-size:13px;padding:6px 10px;">← Назад</button>
      <div class="bc-wrap" style="flex:1;min-width:0;"></div>
      <input class="rf-search" placeholder="Поиск...">
      <button class="btn-primary rf-add" style="font-size:13px;padding:6px 12px;">+ Продукт</button>
    </div>
    <div class="drill-content"></div>
  </div>`;
  el.querySelector('.rf-back').onclick = goBack;
  el.querySelector('.rf-search').oninput = (e) => { nav.q = e.target.value.trim(); render(); };
  el.querySelector('.rf-add').onclick = () => showProductModal(fullReload, null, {
    category: nav.category || 'other', subgroup: nav.subgroup || '',
  });
}

function goBack() {
  if (nav.view === 'products') { nav.view = 'subgroup'; nav.subgroup = null; }
  else if (nav.view === 'subgroup') { nav.view = 'category'; nav.category = null; }
  render();
}

function renderFlatList(el, items) {
  el.innerHTML = '';
  el.classList.remove('cat-grid', 'sg-grid');
  el.classList.add('recipe-grid');
  if (!items.length) { el.innerHTML = '<div class="empty-state">Ничего не найдено</div>'; return; }
  const blockedTags = new Set(blacklist.filter(e => e.type === 'tag').map(e => e.value.toLowerCase()));
  for (const p of items.slice(0, 200)) {
    const card = renderProductCard(p, {
      productBlocked: isIngredientBlocked(p.name, blacklist),
      blockedTags,
      blockedCategory: isCategoryBlocked(p.category, blacklist),
    });
    card.onclick = () => openProduct(p);
    el.appendChild(card);
  }
}

function render() {
  const content = rootEl.querySelector('.drill-content');
  const back = rootEl.querySelector('.rf-back');
  const bc = rootEl.querySelector('.bc-wrap');
  back.style.display = (nav.view === 'category' && !nav.q) ? 'none' : '';

  const crumbs = [];
  if (!nav.q) {
    if (nav.view !== 'category') crumbs.push(CAT_LABELS[nav.category] || nav.category);
    if (nav.view === 'products') crumbs.push(nav.subgroup || 'Без подгруппы');
    renderBreadcrumb(bc, crumbs.length ? ['Все', ...crumbs] : [], navigate);
  } else {
    renderBreadcrumb(bc, [`Поиск: «${nav.q}»`], () => {});
  }

  if (nav.q) {
    const q = nav.q.toLowerCase();
    renderFlatList(content, allProducts.filter(p =>
      p.name.toLowerCase().includes(q) || (p.subgroup || '').toLowerCase().includes(q)
    ));
    return;
  }

  if (nav.view === 'category') renderCategoryGrid(content, allProducts, blacklist, pickCategory);
  else if (nav.view === 'subgroup') renderSubgroupGrid(content, allProducts, nav.category, blacklist, pickSubgroup);
  else renderProductsGrid(content, allProducts, nav.category, nav.subgroup, blacklist, openProduct);
}

function navigate(idx) {
  if (idx === 0) { nav.view = 'category'; nav.category = null; nav.subgroup = null; }
  else if (idx === 1) { nav.view = 'subgroup'; nav.subgroup = null; }
  render();
}

function pickCategory(cat) { nav.view = 'subgroup'; nav.category = cat; render(); }
function pickSubgroup(sg) { nav.view = 'products'; nav.subgroup = sg; render(); }
function openProduct(p) { showProductModal(fullReload, p); }

async function fullReload() {
  await loadData();
  if (rootEl) buildShell(rootEl);
  render();
}

export async function renderProductCatalogPane(el) {
  rootEl = el;
  nav.view = 'category'; nav.category = null; nav.subgroup = null; nav.q = '';
  await loadData();
  buildShell(el);
  render();
}
