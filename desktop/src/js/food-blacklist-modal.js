// ── food-blacklist-modal.js — Manage the food blacklist (catalog-level blocks) ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { CAT_LABELS, CAT_ORDER, loadCatalog, invalidateBlacklistCache } from './food-recipe-filters.js';
import { applyBlacklist, BL_LEVELS } from './food-blacklist-menu.js';

// Blacklist references the catalog's own hierarchy: a whole category, a subgroup,
// or a single product. Subgroup blocks are stored as type='tag' (the detector
// matches 'tag' entries by catalog subgroup) — no free-text "keyword" type.
const KIND_ICON = { category: '📂', subgroup: '🍱', product: '🥕' };
const KIND_LABEL = { category: 'категория', subgroup: 'подгруппа', product: 'продукт' };
const chipIcon = (type) => type === 'category' ? '📂' : type === 'tag' ? '🍱' : '🥕';

export async function showBlacklistModal(onChange) {
  let entries = await invoke('list_food_blacklist').catch(() => []);
  const catalog = await loadCatalog();
  let addLevel = 'hard';

  // Searchable index of everything blockable: categories, subgroups, products.
  const index = [];
  for (const c of CAT_ORDER) {
    if (catalog.some(p => p.category === c)) index.push({ kind: 'category', value: c, label: CAT_LABELS[c] || c });
  }
  for (const sg of [...new Set(catalog.map(p => (p.subgroup || '').trim()).filter(Boolean))].sort()) {
    index.push({ kind: 'subgroup', value: sg, label: sg });
  }
  for (const p of catalog) index.push({ kind: 'product', value: p.name, label: p.name, catalogId: p.id });

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:520px">
    <div class="modal-title">🚫 Блэклист еды</div>
    <p style="color:var(--text-secondary);font-size:13px;margin:0 0 14px;">
      «Не ем» — скрывается из рецептов и каталога. «Не люблю» — остаётся, но тусклее и ниже.</p>
    <div class="bl-sections"></div>
    <div class="bl-add">
      <div class="bl-pick bl-pick-level"></div>
      <div class="bl-search">
        <input class="form-input bl-value" placeholder="Найти продукт, подгруппу или категорию…" autocomplete="off">
        <div class="bl-ac" style="display:none"></div>
      </div>
    </div>
    <div class="modal-actions"><button class="btn-secondary" data-close>Закрыть</button></div>
  </div>`;

  const q = (s) => overlay.querySelector(s);
  const valueInp = q('.bl-value');
  const acEl = q('.bl-ac');

  function renderLevelPick() {
    q('.bl-pick-level').innerHTML = BL_LEVELS.map(l =>
      `<button class="bl-tab${l.level === addLevel ? ' active' : ''}" data-lvl="${l.level}">${l.icon} ${l.label}</button>`).join('');
    overlay.querySelectorAll('[data-lvl]').forEach(b =>
      b.onclick = () => { addLevel = b.dataset.lvl; renderLevelPick(); });
  }

  function renderSections() {
    q('.bl-sections').innerHTML = BL_LEVELS.map(l => {
      const items = entries.filter(e => (e.level || 'hard') === l.level);
      const chips = items.length ? items.map(e => {
        const disp = e.type === 'category' ? (CAT_LABELS[e.value] || e.value) : e.value;
        return `<span class="bl-chip bl-chip--${l.level}">${chipIcon(e.type)} ${escapeHtml(disp)}
          <button data-del="${e.id}" title="Убрать">×</button></span>`;
      }).join('') : '<span style="color:var(--text-muted);font-size:12px">пусто</span>';
      return `<div class="bl-section"><div class="bl-section-head">${l.icon} ${l.label}</div>
        <div class="bl-chips">${chips}</div></div>`;
    }).join('');
    overlay.querySelectorAll('[data-del]').forEach(b =>
      b.onclick = () => removeEntry(parseInt(b.dataset.del)));
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
      row.onmousedown = (e) => { e.preventDefault(); pick(hits[parseInt(row.dataset.i)]); });
  }

  async function pick(hit) {
    const type = hit.kind === 'category' ? 'category' : hit.kind === 'subgroup' ? 'tag' : 'product';
    const ok = await applyBlacklist(type, hit.value, addLevel, hit.catalogId || null, async () => {
      await refresh(); if (onChange) onChange();
    });
    if (ok) { valueInp.value = ''; acEl.style.display = 'none'; valueInp.focus(); }
  }

  async function refresh() {
    entries = await invoke('list_food_blacklist').catch(() => []);
    renderSections();
  }

  async function removeEntry(id) {
    await invoke('remove_food_blacklist', { id }).catch(() => {});
    invalidateBlacklistCache();
    await refresh();
    if (onChange) onChange();
  }

  valueInp.oninput = renderAC;
  valueInp.onblur = () => setTimeout(() => { acEl.style.display = 'none'; }, 150);
  q('[data-close]').onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  renderLevelPick();
  renderSections();
  document.body.appendChild(overlay);
  valueInp.focus();
}
