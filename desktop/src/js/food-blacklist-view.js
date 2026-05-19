// ── food-blacklist-view.js — Blacklist as a catalog-style screen ──
// Blocked items render with the catalog's own components: a product is a
// product card, a subgroup / category is a tile — each at its own level.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { CAT_LABELS, CAT_ORDER, loadCatalog, invalidateBlacklistCache } from './food-recipe-filters.js';
import { renderProductCard } from './food-product-card.js';
import { CAT_EMOJI } from './food-product-views.js';
import { applyBlacklist, BL_LEVELS } from './food-blacklist-menu.js';

const KIND_ICON = { category: '📂', subgroup: '🍱', product: '🥕' };
const KIND_LABEL = { category: 'категория', subgroup: 'подгруппа', product: 'продукт' };

export async function renderBlacklistPane(el) {
  const entries = await invoke('list_food_blacklist').catch(() => []);
  const catalog = await loadCatalog();
  let addLevel = 'hard';
  const reload = () => renderBlacklistPane(el);

  // Searchable index of everything blockable: categories, subgroups, products.
  const index = [];
  for (const c of CAT_ORDER) {
    if (catalog.some(p => p.category === c)) index.push({ kind: 'category', value: c, label: CAT_LABELS[c] || c });
  }
  for (const sg of [...new Set(catalog.map(p => (p.subgroup || '').trim()).filter(Boolean))].sort()) {
    index.push({ kind: 'subgroup', value: sg, label: sg });
  }
  for (const p of catalog) index.push({ kind: 'product', value: p.name, label: p.name, catalogId: p.id });

  el.innerHTML = `<div class="bl-pane">
    <div class="bl-add">
      <div class="bl-pick bl-pick-level"></div>
      <div class="bl-search">
        <input class="form-input bl-value" placeholder="Найти продукт, подгруппу или категорию…" autocomplete="off">
        <div class="bl-ac" style="display:none"></div>
      </div>
    </div>
    <div class="bl-sections"></div>
  </div>`;

  const q = (s) => el.querySelector(s);
  const valueInp = q('.bl-value');
  const acEl = q('.bl-ac');

  function renderLevelPick() {
    q('.bl-pick-level').innerHTML = BL_LEVELS.map(l =>
      `<button class="bl-tab${l.level === addLevel ? ' active' : ''}" data-lvl="${l.level}">${l.icon} ${l.label}</button>`).join('');
    el.querySelectorAll('[data-lvl]').forEach(b =>
      b.onclick = () => { addLevel = b.dataset.lvl; renderLevelPick(); });
  }

  function addRemove(node, id) {
    const btn = document.createElement('button');
    btn.className = 'bl-remove';
    btn.textContent = '×';
    btn.title = 'Убрать из блэклиста';
    btn.onclick = (ev) => { ev.stopPropagation(); removeEntry(id); };
    node.appendChild(btn);
  }

  // One catalog-style tile/card for a blacklist entry, at its own level.
  function entryTile(e, level) {
    const mod = level === 'hard' ? '--blocked' : '--soft';
    if (e.type === 'product') {
      const prod = catalog.find(p => p.name.toLowerCase() === e.value.toLowerCase())
        || { name: e.value, category: 'other', tags: '' };
      const card = renderProductCard(prod, { productLevel: level });
      card.querySelector('.bl-quick')?.remove();
      addRemove(card, e.id);
      return card;
    }
    const tile = document.createElement('div');
    if (e.type === 'category') {
      const count = catalog.filter(p => p.category === e.value).length;
      tile.className = `cat-tile cat-tile${mod}`;
      tile.innerHTML = `<div class="cat-tile-emoji">${CAT_EMOJI[e.value] || '📦'}</div>
        <div class="cat-tile-name">${escapeHtml(CAT_LABELS[e.value] || e.value)}</div>
        <div class="cat-tile-count">${count}</div>`;
    } else { // type 'tag' = catalog subgroup
      const count = catalog.filter(p => (p.subgroup || '').toLowerCase() === e.value.toLowerCase()).length;
      tile.className = `sg-tile sg-tile${mod}`;
      tile.innerHTML = `<div class="sg-tile-name">${escapeHtml(e.value)}</div>
        <div class="sg-tile-count">${count}</div>`;
    }
    addRemove(tile, e.id);
    return tile;
  }

  function renderSections() {
    const box = q('.bl-sections');
    box.innerHTML = '';
    for (const l of BL_LEVELS) {
      const items = entries.filter(e => (e.level || 'hard') === l.level);
      const sec = document.createElement('div');
      sec.className = 'bl-section';
      sec.innerHTML = `<div class="bl-section-head">${l.icon} ${l.label}</div>`;
      if (!items.length) {
        sec.insertAdjacentHTML('beforeend', '<div class="bl-empty">пусто</div>');
      } else {
        for (const [kind, gridCls] of [['category', 'cat-grid'], ['tag', 'sg-grid'], ['product', 'recipe-grid']]) {
          const ofKind = items.filter(e => e.type === kind);
          if (!ofKind.length) continue;
          const grid = document.createElement('div');
          grid.className = gridCls;
          for (const e of ofKind) grid.appendChild(entryTile(e, l.level));
          sec.appendChild(grid);
        }
      }
      box.appendChild(sec);
    }
  }

  function renderAC() {
    const ql = valueInp.value.trim().toLowerCase();
    if (!ql) { acEl.style.display = 'none'; return; }
    const hits = index.filter(x => x.label.toLowerCase().includes(ql)).slice(0, 14);
    if (!hits.length) { acEl.style.display = 'none'; return; }
    acEl.innerHTML = hits.map((h, i) =>
      `<div class="bl-ac-row" data-i="${i}">${KIND_ICON[h.kind]} ${escapeHtml(h.label)}
        <small>${KIND_LABEL[h.kind]}</small></div>`).join('');
    acEl.style.display = '';
    acEl.querySelectorAll('.bl-ac-row').forEach(row =>
      row.onmousedown = (ev) => { ev.preventDefault(); pick(hits[parseInt(row.dataset.i)]); });
  }

  async function pick(hit) {
    const type = hit.kind === 'category' ? 'category' : hit.kind === 'subgroup' ? 'tag' : 'product';
    await applyBlacklist(type, hit.value, addLevel, hit.catalogId || null, reload);
  }

  async function removeEntry(id) {
    await invoke('remove_food_blacklist', { id }).catch(() => {});
    invalidateBlacklistCache();
    reload();
  }

  valueInp.oninput = renderAC;
  valueInp.onblur = () => setTimeout(() => { acEl.style.display = 'none'; }, 150);

  renderLevelPick();
  renderSections();
}
