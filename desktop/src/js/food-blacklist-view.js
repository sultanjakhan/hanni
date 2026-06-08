// ── food-blacklist-view.js — Food preferences screen ──
// Level sub-tabs (🚫 Не ем / 👎 Не люблю / 💚 Люблю) show one level at a time.
// Inside, entries are grouped by type (categories / subgroups / products /
// dishes), each a uniform product-card. A single "+" adds to the active level.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { CAT_LABELS, CAT_ORDER, loadCatalog, invalidateBlacklistCache } from './food-recipe-filters.js';
import { renderProductCard } from './food-product-card.js';
import { applyBlacklist, BL_LEVELS } from './food-blacklist-menu.js';
import { openAddPopover } from './food-blacklist-add.js';

const KIND_LABEL = { category: 'категория', tag: 'подгруппа', product: 'продукт', recipe: 'блюдо' };
const TYPE_GROUPS = [
  { type: 'category', label: 'Категории' },
  { type: 'tag', label: 'Подгруппы' },
  { type: 'product', label: 'Продукты' },
  { type: 'recipe', label: 'Блюда' },
];

let activeLevel = 'hard'; // persists across reloads within the session

export async function renderBlacklistPane(el) {
  const [entries, recipes] = await Promise.all([
    invoke('list_food_blacklist').catch(() => []),
    invoke('get_recipes', { search: null }).catch(() => []),
  ]);
  const catalog = await loadCatalog();
  const reload = () => renderBlacklistPane(el);

  // One flat candidate list for the "+" popover; each option carries its type.
  const allOptions = [
    ...CAT_ORDER.filter(c => catalog.some(p => p.category === c))
      .map(c => ({ type: 'category', value: c, label: CAT_LABELS[c] || c, kindLabel: KIND_LABEL.category })),
    ...[...new Set(catalog.map(p => (p.subgroup || '').trim()).filter(Boolean))].sort()
      .map(sg => ({ type: 'tag', value: sg, label: sg, kindLabel: KIND_LABEL.tag })),
    ...catalog.map(p => ({ type: 'product', value: p.name, label: p.name, catalogId: p.id, kindLabel: KIND_LABEL.product })),
    ...recipes.map(r => ({ type: 'recipe', value: r.name, label: r.name, kindLabel: KIND_LABEL.recipe })),
  ];

  el.innerHTML = `<div class="bl-pane">
    <div class="bl-levelbar">
      <div class="bl-levels"></div>
      <span class="bl-count"></span>
      <button class="bl-add-one">+ Добавить</button>
    </div>
    <div class="bl-body"></div>
  </div>`;

  el.querySelector('.bl-add-one').onclick = (ev) => openAddPopover(ev.currentTarget,
    { placeholder: 'Категория, подгруппа, продукт или блюдо…', options: allOptions },
    (o) => applyBlacklist(o.type, o.value, activeLevel, o.catalogId || null, reload));

  function addRemove(node, id) {
    const btn = document.createElement('button');
    btn.className = 'bl-remove';
    btn.textContent = '×';
    btn.title = 'Убрать из предпочтений';
    btn.onclick = (ev) => { ev.stopPropagation(); removeEntry(id); };
    node.appendChild(btn);
  }

  const lvlMod = (level) => level === 'hard' ? '--blocked' : level === 'love' ? '--love' : '--soft';

  function entryTile(e, level) {
    if (e.type === 'product') {
      const prod = catalog.find(p => p.name.toLowerCase() === e.value.toLowerCase())
        || { name: e.value, category: 'other', tags: '' };
      const card = renderProductCard(prod, { productLevel: level });
      card.querySelector('.bl-quick')?.remove();
      addRemove(card, e.id);
      return card;
    }
    let name, pill;
    if (e.type === 'category') {
      name = CAT_LABELS[e.value] || e.value;
      pill = `${KIND_LABEL.category} · ${catalog.filter(p => p.category === e.value).length}`;
    } else if (e.type === 'recipe') {
      name = e.value;
      pill = KIND_LABEL.recipe;
    } else { // 'tag' = catalog subgroup
      name = e.value;
      pill = `${KIND_LABEL.tag} · ${catalog.filter(p => (p.subgroup || '').toLowerCase() === e.value.toLowerCase()).length}`;
    }
    const card = document.createElement('div');
    card.className = `product-card product-card${lvlMod(level)}`;
    card.innerHTML = `<div class="product-card-name">${escapeHtml(name)}</div>
      <div class="product-card-tags"><span class="product-card-cat product-cat-gray">${escapeHtml(pill)}</span></div>`;
    addRemove(card, e.id);
    return card;
  }

  function renderLevels() {
    el.querySelector('.bl-levels').innerHTML = BL_LEVELS.map(l =>
      `<button class="bl-level-tab${l.level === activeLevel ? ' active' : ''}" data-lvl="${l.level}">${l.icon} ${l.label}</button>`).join('');
    el.querySelectorAll('.bl-level-tab').forEach(b =>
      b.onclick = () => { activeLevel = b.dataset.lvl; renderLevels(); renderBody(); });
  }

  function renderBody() {
    const box = el.querySelector('.bl-body');
    box.innerHTML = '';
    const cntEl = el.querySelector('.bl-count');
    const lvlTotal = entries.filter(e => (e.level || 'hard') === activeLevel).length;
    if (cntEl) cntEl.textContent = lvlTotal ? `Записей: ${lvlTotal}` : '';
    let any = false;
    for (const g of TYPE_GROUPS) {
      const items = entries.filter(e => (e.level || 'hard') === activeLevel && e.type === g.type);
      if (!items.length) continue;
      any = true;
      box.insertAdjacentHTML('beforeend', `<div class="bl-subhead"><span>${g.label}</span></div>`);
      const grid = document.createElement('div');
      grid.className = 'recipe-grid';
      for (const e of items) grid.appendChild(entryTile(e, activeLevel));
      box.appendChild(grid);
    }
    if (!any) box.innerHTML = '<div class="bl-empty">Пусто — нажми «+ Добавить».</div>';
  }

  async function removeEntry(id) {
    await invoke('remove_food_blacklist', { id }).catch(() => {});
    invalidateBlacklistCache();
    reload();
  }

  renderLevels();
  renderBody();
}
