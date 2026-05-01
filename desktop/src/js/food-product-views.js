// ── food-product-views.js — drill-down views: category → [parent|subgroup] → product ──
import { CAT_LABELS, CAT_ORDER, isIngredientBlocked, isTagBlocked, isCategoryBlocked } from './food-recipe-filters.js';
import { renderProductCard } from './food-product-card.js';

const CAT_EMOJI = { meat:'🥩', fish:'🐟', veg:'🥬', fruit:'🍎', grain:'🌾', dairy:'🧀',
  legumes:'🫘', nuts:'🌰', spice:'🌶️', oil:'🫒', bakery:'🥖', drinks:'🥤', other:'📦' };

// Categories that show parent → children hierarchy instead of flat subgroups.
export const HIERARCHICAL_CATS = new Set(['meat', 'fish']);

export function renderCategoryGrid(el, catalog, blacklist, onPick) {
  const frag = document.createDocumentFragment();
  for (const cat of CAT_ORDER) {
    const items = catalog.filter(p => p.category === cat);
    if (!items.length) continue;
    const blocked = isCategoryBlocked(cat, blacklist);
    const tile = document.createElement('div');
    tile.className = 'cat-tile' + (blocked ? ' cat-tile--blocked' : '');
    tile.innerHTML = `<div class="cat-tile-emoji">${CAT_EMOJI[cat] || '📦'}</div>
      <div class="cat-tile-name">${CAT_LABELS[cat] || cat}${blocked ? ' 🚫' : ''}</div>
      <div class="cat-tile-count">${items.length}</div>`;
    tile.onclick = () => onPick(cat);
    frag.appendChild(tile);
  }
  el.innerHTML = '';
  el.classList.remove('sg-grid', 'recipe-grid');
  el.classList.add('cat-grid');
  el.appendChild(frag);
}

export function renderSubgroupGrid(el, catalog, category, blacklist, onPick) {
  const items = catalog.filter(p => p.category === category);
  const groups = new Map();
  for (const p of items) {
    const sg = p.subgroup || '';
    if (!groups.has(sg)) groups.set(sg, []);
    groups.get(sg).push(p);
  }
  const sorted = [...groups.entries()].sort((a, b) => {
    if (!a[0]) return 1; if (!b[0]) return -1;
    return a[0].localeCompare(b[0], 'ru');
  });
  const frag = document.createDocumentFragment();
  for (const [sg, list] of sorted) {
    const label = sg || 'Без подгруппы';
    const blocked = sg && isTagBlocked(sg, blacklist);
    const tile = document.createElement('div');
    tile.className = 'sg-tile' + (blocked ? ' sg-tile--blocked' : '');
    tile.innerHTML = `<div class="sg-tile-name">${label}${blocked ? ' 🚫' : ''}</div>
      <div class="sg-tile-count">${list.length}</div>`;
    tile.onclick = () => onPick(sg);
    frag.appendChild(tile);
  }
  el.innerHTML = '';
  el.classList.remove('cat-grid', 'recipe-grid');
  el.classList.add('sg-grid');
  el.appendChild(frag);
}

export function renderProductsGrid(el, catalog, category, subgroup, blacklist, onOpen) {
  const items = catalog
    .filter(p => p.category === category && (p.subgroup || '') === (subgroup || ''))
    .sort((a, b) => a.name.localeCompare(b.name, 'ru'));
  el.innerHTML = '';
  el.classList.remove('cat-grid', 'sg-grid');
  el.classList.add('recipe-grid');
  if (!items.length) { el.innerHTML = '<div class="empty-state">Нет продуктов</div>'; return; }
  const blockedTagSet = new Set(blacklist.filter(e => e.type === 'tag').map(e => e.value.toLowerCase()));
  for (const p of items) {
    const productBlocked = isIngredientBlocked(p.name, blacklist);
    const blockedCategory = isCategoryBlocked(p.category, blacklist);
    const card = renderProductCard(p, { productBlocked, blockedTags: blockedTagSet, blockedCategory });
    card.onclick = () => onOpen(p);
    el.appendChild(card);
  }
}

// Parent-level view for hierarchical categories.
// Shows tiles: real parents (have children) first, then orphan groups by subgroup.
// onPick receives either { id, name } for real parents or { orphanSubgroup, name } for orphan buckets.
export function renderParentGrid(el, catalog, category, blacklist, onPick) {
  const items = catalog.filter(p => p.category === category);
  const childrenByParent = new Map();
  const tops = [];
  for (const p of items) {
    if (p.parent_id) {
      if (!childrenByParent.has(p.parent_id)) childrenByParent.set(p.parent_id, []);
      childrenByParent.get(p.parent_id).push(p);
    } else {
      tops.push(p);
    }
  }
  const parents = tops.filter(p => childrenByParent.has(p.id))
    .sort((a, b) => a.name.localeCompare(b.name, 'ru'));
  const orphans = tops.filter(p => !childrenByParent.has(p.id));

  const blockedTagSet = new Set(blacklist.filter(e => e.type === 'tag').map(e => e.value.toLowerCase()));

  el.innerHTML = '';
  el.classList.remove('cat-grid', 'recipe-grid');
  el.classList.add('sg-grid');
  const frag = document.createDocumentFragment();

  for (const par of parents) {
    const kids = childrenByParent.get(par.id) || [];
    const count = kids.length + 1;
    const blocked = isIngredientBlocked(par.name, blacklist)
      || (par.subgroup && blockedTagSet.has(par.subgroup.toLowerCase()));
    const tile = document.createElement('div');
    tile.className = 'sg-tile' + (blocked ? ' sg-tile--blocked' : '');
    tile.innerHTML = `<div class="sg-tile-name">${esc(par.name)}${blocked ? ' 🚫' : ''}</div>
      <div class="sg-tile-count">${count}</div>`;
    tile.onclick = () => onPick({ id: par.id, name: par.name });
    frag.appendChild(tile);
  }

  if (orphans.length) {
    const groups = new Map();
    for (const p of orphans) {
      const sg = p.subgroup || '';
      if (!groups.has(sg)) groups.set(sg, []);
      groups.get(sg).push(p);
    }
    const sorted = [...groups.entries()].sort((a, b) => {
      if (!a[0]) return 1; if (!b[0]) return -1;
      return a[0].localeCompare(b[0], 'ru');
    });
    for (const [sg, list] of sorted) {
      const label = sg || 'Без подгруппы';
      const blocked = sg && isTagBlocked(sg, blacklist);
      const tile = document.createElement('div');
      tile.className = 'sg-tile' + (blocked ? ' sg-tile--blocked' : '');
      tile.innerHTML = `<div class="sg-tile-name">${label}${blocked ? ' 🚫' : ''}</div>
        <div class="sg-tile-count">${list.length}</div>`;
      tile.onclick = () => onPick({ orphanSubgroup: sg, name: label });
      frag.appendChild(tile);
    }
  }
  el.appendChild(frag);
}

// Children-level view: parent + its children, OR an orphan-subgroup bucket of flat items.
export function renderChildrenGrid(el, catalog, parent, blacklist, onOpen) {
  let items;
  if (parent.orphanSubgroup !== undefined) {
    items = catalog.filter(p => !p.parent_id && (p.subgroup || '') === (parent.orphanSubgroup || ''));
  } else {
    const root = catalog.find(c => c.id === parent.id);
    const kids = catalog.filter(c => c.parent_id === parent.id);
    items = root ? [root, ...kids] : kids;
  }
  items.sort((a, b) => a.name.localeCompare(b.name, 'ru'));
  el.innerHTML = '';
  el.classList.remove('cat-grid', 'sg-grid');
  el.classList.add('recipe-grid');
  if (!items.length) { el.innerHTML = '<div class="empty-state">Нет продуктов</div>'; return; }
  const blockedTagSet = new Set(blacklist.filter(e => e.type === 'tag').map(e => e.value.toLowerCase()));
  for (const p of items) {
    const card = renderProductCard(p, {
      productBlocked: isIngredientBlocked(p.name, blacklist),
      blockedTags: blockedTagSet,
      blockedCategory: isCategoryBlocked(p.category, blacklist),
    });
    card.onclick = () => onOpen(p);
    el.appendChild(card);
  }
}

function esc(s) { return (s || '').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }

export function renderBreadcrumb(el, parts, onNavigate) {
  el.innerHTML = '';
  el.classList.add('pp-breadcrumb');
  parts.forEach((label, i) => {
    const isLast = i === parts.length - 1;
    const span = document.createElement('span');
    span.className = 'pp-crumb' + (isLast ? ' pp-crumb-active' : '');
    span.textContent = label;
    if (!isLast) span.onclick = () => onNavigate(i);
    el.appendChild(span);
    if (!isLast) {
      const sep = document.createElement('span');
      sep.className = 'pp-crumb-sep';
      sep.textContent = ' / ';
      el.appendChild(sep);
    }
  });
}
