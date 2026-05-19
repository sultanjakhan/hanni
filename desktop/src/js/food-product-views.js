// ── food-product-views.js — drill-down views: category → [parent|subgroup] → product ──
import { CAT_LABELS, CAT_ORDER, ingredientBlockLevel, tagBlockLevel, categoryBlockLevel } from './food-recipe-filters.js';
import { renderProductCard } from './food-product-card.js';

const CAT_EMOJI = { meat:'🥩', fish:'🐟', veg:'🥬', fruit:'🍎', grain:'🌾', dairy:'🧀',
  legumes:'🫘', nuts:'🌰', spice:'🌶️', oil:'🫒', bakery:'🥖', drinks:'🥤', other:'📦' };

// Categories that show parent → children hierarchy instead of flat subgroups.
export const HIERARCHICAL_CATS = new Set(['meat', 'fish']);

// Blacklist level → tile modifier class / icon. hard hides nothing here — the
// catalog is a wiki, so blocked items stay visible (hard red, soft dimmed).
const blkCls = (lvl, base) => lvl === 'hard' ? ` ${base}--blocked` : lvl === 'soft' ? ` ${base}--soft` : '';
const blkIcon = (lvl) => lvl === 'hard' ? ' 🚫' : lvl === 'soft' ? ' 👎' : '';

// Render product cards by blacklist level — soft ("не люблю") sinks to the bottom.
function appendProductCards(el, items, blacklist, onOpen) {
  const blockedTagSet = new Set(blacklist.filter(e => e.type === 'tag').map(e => (e.value || '').toLowerCase()));
  const withLvl = items.map(p => ({ p, lvl: ingredientBlockLevel(p.name, blacklist) }));
  withLvl.sort((a, b) =>
    (a.lvl === 'soft' ? 1 : 0) - (b.lvl === 'soft' ? 1 : 0)
    || a.p.name.localeCompare(b.p.name, 'ru'));
  for (const { p, lvl } of withLvl) {
    const card = renderProductCard(p, {
      productLevel: lvl, blockedTags: blockedTagSet,
      blockedCategory: !!categoryBlockLevel(p.category, blacklist),
    });
    card.onclick = (e) => { if (!e.target.closest('.bl-quick')) onOpen(p); };
    el.appendChild(card);
  }
}

export function renderCategoryGrid(el, catalog, blacklist, onPick) {
  const frag = document.createDocumentFragment();
  for (const cat of CAT_ORDER) {
    const items = catalog.filter(p => p.category === cat);
    if (!items.length) continue;
    const lvl = categoryBlockLevel(cat, blacklist);
    const tile = document.createElement('div');
    tile.className = 'cat-tile' + blkCls(lvl, 'cat-tile');
    tile.dataset.cat = cat;
    tile.innerHTML = `<div class="cat-tile-emoji">${CAT_EMOJI[cat] || '📦'}</div>
      <div class="cat-tile-name">${CAT_LABELS[cat] || cat}${blkIcon(lvl)}</div>
      <div class="cat-tile-count">${items.length}</div>
      <button class="bl-quick" title="В блэклист">⊘</button>`;
    tile.onclick = (e) => { if (!e.target.closest('.bl-quick')) onPick(cat); };
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
    const lvl = sg ? tagBlockLevel(sg, blacklist) : '';
    const tile = document.createElement('div');
    tile.className = 'sg-tile' + blkCls(lvl, 'sg-tile');
    tile.innerHTML = `<div class="sg-tile-name">${label}${blkIcon(lvl)}</div>
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
    .filter(p => p.category === category && (p.subgroup || '') === (subgroup || ''));
  el.innerHTML = '';
  el.classList.remove('cat-grid', 'sg-grid');
  el.classList.add('recipe-grid');
  if (!items.length) { el.innerHTML = '<div class="empty-state">Нет продуктов</div>'; return; }
  appendProductCards(el, items, blacklist, onOpen);
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

  // Fold orphans whose subgroup matches a parent's subgroup into that parent,
  // so we don't render a duplicate tile (e.g. parent "говядина" + bucket "говядина").
  const parentBySubgroup = new Map();
  for (const par of parents) {
    const sg = (par.subgroup || '').trim();
    if (sg && !parentBySubgroup.has(sg)) parentBySubgroup.set(sg, par.id);
  }
  const foldedCount = new Map();
  const looseOrphans = [];
  for (const p of orphans) {
    const hostId = parentBySubgroup.get((p.subgroup || '').trim());
    if (hostId) foldedCount.set(hostId, (foldedCount.get(hostId) || 0) + 1);
    else looseOrphans.push(p);
  }

  el.innerHTML = '';
  el.classList.remove('cat-grid', 'recipe-grid');
  el.classList.add('sg-grid');
  const frag = document.createDocumentFragment();

  for (const par of parents) {
    const kids = childrenByParent.get(par.id) || [];
    const count = kids.length + (foldedCount.get(par.id) || 0) + 1;
    const lvl = ingredientBlockLevel(par.name, blacklist);
    const tile = document.createElement('div');
    tile.className = 'sg-tile' + blkCls(lvl, 'sg-tile');
    tile.innerHTML = `<div class="sg-tile-name">${esc(par.name)}${blkIcon(lvl)}</div>
      <div class="sg-tile-count">${count}</div>`;
    tile.onclick = () => onPick({ id: par.id, name: par.name });
    frag.appendChild(tile);
  }

  if (looseOrphans.length) {
    const groups = new Map();
    for (const p of looseOrphans) {
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
      const lvl = sg ? tagBlockLevel(sg, blacklist) : '';
      const tile = document.createElement('div');
      tile.className = 'sg-tile' + blkCls(lvl, 'sg-tile');
      tile.innerHTML = `<div class="sg-tile-name">${label}${blkIcon(lvl)}</div>
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
    // Include orphans folded into this parent by matching subgroup (see renderParentGrid).
    const sg = ((root && root.subgroup) || '').trim();
    const hasChildren = new Set(catalog.filter(c => c.parent_id).map(c => c.parent_id));
    const folded = sg ? catalog.filter(c => !c.parent_id && c.id !== parent.id
      && !hasChildren.has(c.id) && (c.subgroup || '').trim() === sg) : [];
    items = root ? [root, ...kids, ...folded] : [...kids, ...folded];
  }
  el.innerHTML = '';
  el.classList.remove('cat-grid', 'sg-grid');
  el.classList.add('recipe-grid');
  if (!items.length) { el.innerHTML = '<div class="empty-state">Нет продуктов</div>'; return; }
  appendProductCards(el, items, blacklist, onOpen);
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
