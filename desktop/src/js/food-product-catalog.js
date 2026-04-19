// ── food-product-catalog.js — Product catalog pane in Food tab ──
import { invoke } from './state.js';
import { CAT_LABELS, CAT_ORDER, loadCatalog, invalidateCatalogCache } from './food-recipe-filters.js';
import { renderProductCard } from './food-product-card.js';
import { showProductModal } from './food-product-modal.js';

const F = { cat: 'all', q: '' };
let allProducts = [], panelOpen = false, grid, panel;

async function loadData() {
  allProducts = await invoke('get_ingredient_catalog').catch(() => []);
}

function getFiltered() {
  const list = allProducts.filter(p =>
    (F.cat === 'all' || p.category === F.cat) &&
    (!F.q || p.name.toLowerCase().includes(F.q.toLowerCase()))
  );
  const catIdx = Object.fromEntries(CAT_ORDER.map((c, i) => [c, i]));
  list.sort((a, b) => (catIdx[a.category] ?? 99) - (catIdx[b.category] ?? 99) || a.name.localeCompare(b.name, 'ru'));
  return list;
}

function chips(field, items, active) {
  return items.map(({ id, label }) =>
    `<button class="rf-chip${active === id ? ' active' : ''}" data-field="${field}" data-val="${id}">${label}</button>`
  ).join('');
}

function buildShell(el) {
  el.innerHTML = `<div class="recipe-pane">
    <div class="recipe-filter-bar">
      <button class="rf-toggle">Фильтры ▾</button>
      <input class="rf-search" placeholder="Поиск продукта...">
      <button class="btn-primary rf-add" style="margin-left:auto;font-size:13px;padding:6px 12px;">+ Продукт</button>
    </div>
    <div class="rf-panel" style="display:none">
      <div class="rf-section"><div class="rf-section-title">Категория</div>
        <div class="rf-chips">${chips('cat', [{ id: 'all', label: 'Все' }, ...CAT_ORDER.map(c => ({ id: c, label: CAT_LABELS[c] }))], F.cat)}</div>
      </div>
    </div>
    <div class="recipe-grid"></div>
  </div>`;
  grid = el.querySelector('.recipe-grid');
  panel = el.querySelector('.rf-panel');
  el.querySelector('.rf-toggle').onclick = () => {
    panelOpen = !panelOpen;
    panel.style.display = panelOpen ? '' : 'none';
  };
  el.querySelector('.rf-search').oninput = (e) => { F.q = e.target.value; updateGrid(); };
  el.querySelector('.rf-add').onclick = () => showProductModal(fullReload);
  el.querySelectorAll('.rf-chip').forEach(btn => {
    btn.onclick = () => {
      F[btn.dataset.field] = btn.dataset.val;
      btn.closest('.rf-chips').querySelectorAll('.rf-chip').forEach(b =>
        b.classList.toggle('active', b.dataset.val === F[btn.dataset.field]));
      updateGrid();
    };
  });
}

function updateGrid() {
  const list = getFiltered();
  grid.innerHTML = '';
  if (!list.length) { grid.innerHTML = '<div class="empty-state">Нет продуктов</div>'; return; }
  for (const p of list) {
    const card = renderProductCard(p);
    card.onclick = () => showProductModal(fullReload, p);
    grid.appendChild(card);
  }
}

async function fullReload() {
  await loadData();
  invalidateCatalogCache();
  updateGrid();
}

export async function renderProductCatalogPane(el) {
  await loadData();
  buildShell(el);
  updateGrid();
}
