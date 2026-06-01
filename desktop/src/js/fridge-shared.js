// fridge-shared.js — Backend-agnostic fridge (inventory) UI shared by Hanni & guest.
// Loaded as a plain <script>. Registers `window.HanniFridge.mountInventory({ el, backend })`.
//
// Backend interface (Promise-returning):
//   list()                        → [{ id, name, category, quantity, unit, expiry_date, location, notes, catalog_id?, catalog_name? }]
//   add?(payload)                 → void   // payload may include catalog_id
//   update?(id, payload)          → void   // payload may include catalog_id
//   remove?(id)                   → void
//   getCatalog?()                 → [{ id, name, category }]   // optional — enables name autocomplete
(function () {
  const CAT_LABELS = { meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты',
    grain: 'Крупы', dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи',
    spice: 'Специи', oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки',
    sweet: 'Сладости', frozen: 'Заморозка', other: 'Другое' };
  const CAT_COLORS = { meat: 'red', fish: 'blue', veg: 'green', fruit: 'orange',
    grain: 'yellow', dairy: 'purple', legumes: 'green', nuts: 'orange',
    spice: 'pink', oil: 'gray', bakery: 'orange', drinks: 'blue',
    sweet: 'pink', frozen: 'blue', other: 'gray' };
  const LOC_LABELS = { fridge: '❄️ Холод', freezer: '🧊 Морозилка', pantry: '🥫 Полка', other: '📦 Другое' };
  const UNITS = ['шт', 'г', 'кг', 'мл', 'л', 'упак.'];

  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  function expiryBadge(date) {
    if (!date) return '';
    const today = new Date(); today.setHours(0,0,0,0);
    const exp = new Date(date);
    if (isNaN(exp.getTime())) return '';
    const days = Math.round((exp - today) / 86400000);
    if (days < 0) return `<span class="badge badge-red">Просрочен ${-days}д</span>`;
    if (days === 0) return `<span class="badge badge-yellow">Сегодня</span>`;
    if (days <= 3) return `<span class="badge badge-yellow">${days}д</span>`;
    return `<span class="badge badge-gray">${days}д</span>`;
  }

  async function mountInventory({ el, backend }) {
    const state = { items: [], loc: 'all', catalog: null, q: '' };
    const canAdd = !!backend.add, canEdit = !!backend.update, canDelete = !!backend.remove;

    async function load() {
      try { state.items = await backend.list(); render(); }
      catch (e) { el.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`; }
    }

    async function ensureCatalog() {
      if (!backend.getCatalog) return [];
      // Retry on next call if the previous attempt failed (state.catalog stays null).
      if (state.catalog) return state.catalog;
      try { state.catalog = await backend.getCatalog() || []; }
      catch { state.catalog = null; }
      return state.catalog || [];
    }
    function lookupCatalog(name) {
      if (!state.catalog) return null;
      const norm = String(name || '').trim().toLowerCase();
      if (!norm) return null;
      return state.catalog.find(c => String(c.name).trim().toLowerCase() === norm) || null;
    }

    function locFilter(p) { return state.loc === 'all' || p.location === state.loc; }

    // Quantity pill: a −/＋ stepper when the backend allows edits (so you can
    // "use one up" without opening the modal), else a static tag.
    function qtyControl(p) {
      const txt = esc(`${p.quantity ?? 1} ${p.unit || 'шт'}`.trim());
      if (!canEdit) return `<span class="product-card-tag">${txt}</span>`;
      const btn = 'border:none;background:none;cursor:pointer;font-size:15px;line-height:1;color:var(--text-secondary);padding:0 2px';
      return `<span class="product-card-tag" style="display:inline-flex;align-items:center;gap:6px">
        <button type="button" data-dec="${p.id}" title="Меньше" style="${btn}">−</button>
        <span>${txt}</span>
        <button type="button" data-inc="${p.id}" title="Больше" style="${btn}">＋</button>
      </span>`;
    }

    function cardHtml(p) {
      const cls = CAT_COLORS[p.category] || 'gray';
      const label = CAT_LABELS[p.category] || p.category;
      const loc = LOC_LABELS[p.location] || p.location;
      const exp = expiryBadge(p.expiry_date);
      const done = canDelete
        ? `<button type="button" data-done="${p.id}" title="Закончилось — убрать" style="border:none;background:none;cursor:pointer;color:var(--text-muted);font-size:12px;margin-left:auto;padding:0 2px">✓ закончилось</button>`
        : '';
      return `<div class="product-card" data-id="${p.id}">
        <div class="product-card-name" style="display:flex;align-items:center;gap:6px">${esc(p.name)}${done}</div>
        <div class="product-card-tags">
          <span class="product-card-cat product-cat-${cls}">${esc(label)}</span>
          ${qtyControl(p)}
          <span class="product-card-tag">${esc(loc)}</span>
          ${exp}
        </div>
        ${p.notes ? `<div style="margin-top:6px;font-size:12px;color:var(--text-muted);font-style:italic">${esc(p.notes)}</div>` : ''}
      </div>`;
    }

    // Days-to-expiry summary across the whole inventory (not just the filtered
    // location) — the fridge's main job is to stop food rotting unseen.
    function expiryAlert() {
      const today = new Date(); today.setHours(0, 0, 0, 0);
      let overdue = 0, soon = 0;
      for (const p of state.items) {
        if (!p.expiry_date) continue;
        const d = Math.round((new Date(p.expiry_date) - today) / 86400000);
        if (isNaN(d)) continue;
        if (d < 0) overdue++; else if (d <= 3) soon++;
      }
      if (!overdue && !soon) return '';
      const parts = [];
      if (overdue) parts.push(`<b style="color:var(--color-red)">${overdue} просрочено</b>`);
      if (soon) parts.push(`<b style="color:var(--color-yellow)">${soon} скоро испортится</b>`);
      return `<div style="background:var(--bg-hover);border-radius:8px;padding:8px 12px;margin-bottom:12px;font-size:13px">⚠️ ${parts.join(' · ')}</div>`;
    }

    // Compact table of items expiring within 3 days (or overdue), pinned above
    // the grid so what needs eating is impossible to miss.
    function expiringTable() {
      const today = new Date(); today.setHours(0, 0, 0, 0);
      const rows = state.items.filter(p => {
        if (!p.expiry_date) return false;
        const d = Math.round((new Date(p.expiry_date) - today) / 86400000);
        return !isNaN(d) && d <= 3;
      }).slice().sort((a, b) => new Date(a.expiry_date) - new Date(b.expiry_date));
      if (!rows.length) return '';
      return `<div style="margin-bottom:14px">
        <div style="font-weight:600;font-size:13px;margin-bottom:6px">⏰ Скоро испортится (${rows.length})</div>
        <table style="width:100%;border-collapse:collapse;font-size:13px">
          ${rows.map(p => `<tr style="border-bottom:1px solid var(--border-subtle)">
            <td style="padding:5px 8px">${esc(p.name)}</td>
            <td style="padding:5px 8px;color:var(--text-muted)">${esc(`${p.quantity ?? 1} ${p.unit || 'шт'}`.trim())}</td>
            <td style="padding:5px 8px;text-align:right">${expiryBadge(p.expiry_date)}</td>
          </tr>`).join('')}
        </table>
      </div>`;
    }

    async function adjustQty(id, delta) {
      const p = state.items.find(x => x.id === id); if (!p || !canEdit) return;
      const next = Math.round(((parseFloat(p.quantity) || 0) + delta) * 100) / 100;
      if (next <= 0) return finishItem(id);
      try {
        await backend.update(id, { name: p.name, category: p.category, quantity: next, unit: p.unit,
          expiry_date: p.expiry_date, location: p.location, notes: p.notes, catalog_id: p.catalog_id });
        load();
      } catch (e) { alert('Ошибка: ' + (e.message || e)); }
    }
    async function finishItem(id) {
      if (!canDelete) return;
      try { await backend.remove(id); load(); }
      catch (e) { alert('Ошибка: ' + (e.message || e)); }
    }

    function locChips() {
      const opts = { all: 'Все', ...LOC_LABELS };
      return Object.entries(opts).map(([v, l]) =>
        `<button type="button" class="rf-chip ${state.loc === v ? 'active' : ''}" data-loc="${v}">${esc(l)}</button>`
      ).join('');
    }

    // Build the static shell (search + location chips + expiry banner) once;
    // the grid repaints on its own so typing in search keeps focus.
    function render() {
      const addBtn = canAdd ? `<button class="btn-primary" id="fr-add">+ Продукт</button>` : '';
      // "Что приготовить" uses Hanni's recipe/catalog commands → Tauri-only (hidden for guests).
      const cookBtn = window.__TAURI__?.core?.invoke ? `<button class="btn-secondary" id="fr-cook-what" style="white-space:nowrap">🍲 Что приготовить</button>` : '';
      // Bulk quick-fill — Hanni-only (loads fridge-multiadd.js on demand).
      const fastBtn = (canAdd && window.__TAURI__?.core?.invoke) ? `<button class="btn-secondary" id="fr-fast" style="white-space:nowrap">📋 Быстро</button>` : '';
      el.innerHTML = `
        <div class="recipe-filter-bar" style="position:static;padding:0;margin:0 0 12px;align-items:center;gap:8px">
          <input class="form-input" id="fr-search" placeholder="Поиск…" value="${esc(state.q)}" style="max-width:180px">
          <div style="display:flex;gap:6px;flex-wrap:wrap;flex:1">${locChips()}</div>
          ${cookBtn}${fastBtn}${addBtn}
        </div>
        ${expiryAlert()}
        ${expiringTable()}
        <div id="fr-grid-wrap"></div>`;
      el.querySelectorAll('[data-loc]').forEach(b => b.onclick = () => {
        state.loc = b.dataset.loc;
        el.querySelectorAll('[data-loc]').forEach(x => x.classList.toggle('active', x === b));
        paintGrid();
      });
      const s = el.querySelector('#fr-search'); if (s) s.oninput = () => { state.q = s.value; paintGrid(); };
      const a = el.querySelector('#fr-add'); if (a) a.onclick = () => openAddModal();
      const cw = el.querySelector('#fr-cook-what');
      if (cw) cw.onclick = async () => {
        const { showCookWhatModal } = await import('./food-cooking-log.js');
        showCookWhatModal(new Date().toISOString().slice(0, 10), () => load());
      };
      const fast = el.querySelector('#fr-fast');
      if (fast) fast.onclick = async () => {
        const { showMultiAddModal } = await import('./fridge-multiadd.js');
        showMultiAddModal({ backend, location: state.loc === 'all' ? 'fridge' : state.loc, onAdded: () => load() });
      };
      paintGrid();
    }

    function paintGrid() {
      const q = (state.q || '').trim().toLowerCase();
      // Soonest-to-expire first (items with no date sink to the bottom).
      const filtered = state.items.filter(locFilter)
        .filter(p => !q || String(p.name).toLowerCase().includes(q))
        .slice().sort((a, b) => {
          const ea = a.expiry_date ? new Date(a.expiry_date).getTime() : Infinity;
          const eb = b.expiry_date ? new Date(b.expiry_date).getTime() : Infinity;
          return ea - eb;
        });
      const empty = (q || state.loc !== 'all')
        ? 'Ничего не найдено.'
        : `Холодильник пуст.${canAdd ? ' Добавь первый продукт кнопкой «+ Продукт».' : ''}`;
      const grid = filtered.length
        ? `<div class="product-grid">${filtered.map(cardHtml).join('')}</div>`
        : `<div class="empty">${empty}</div>`;
      const wrap = el.querySelector('#fr-grid-wrap');
      if (!wrap) return;
      wrap.innerHTML = `<h2>Холодильник (${filtered.length})</h2>${grid}`;
      wrap.querySelectorAll('.product-card').forEach(c =>
        c.onclick = () => openEditModal(state.items.find(p => p.id === parseInt(c.dataset.id))));
      // Quick controls — stop propagation so they don't open the edit modal.
      wrap.querySelectorAll('[data-inc]').forEach(b => b.onclick = (e) => { e.stopPropagation(); adjustQty(parseInt(b.dataset.inc), 1); });
      wrap.querySelectorAll('[data-dec]').forEach(b => b.onclick = (e) => { e.stopPropagation(); adjustQty(parseInt(b.dataset.dec), -1); });
      wrap.querySelectorAll('[data-done]').forEach(b => b.onclick = (e) => { e.stopPropagation(); finishItem(parseInt(b.dataset.done)); });
    }

    function openAddModal() { openModal(null); }
    function openEditModal(p) {
      if (!canEdit && !canDelete) return; // pure view → no popup
      openModal(p);
    }

    async function openModal(p) {
      const isEdit = !!p;
      await ensureCatalog();
      const hasCatalog = !!state.catalog && state.catalog.length > 0;
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      const catChips = Object.entries(CAT_LABELS).map(([v, l]) =>
        `<button type="button" class="rf-chip ${(p ? p.category : 'other') === v ? 'active' : ''}" data-cat="${v}">${esc(l)}</button>`).join('');
      const locChipsHtml = Object.entries(LOC_LABELS).map(([v, l]) =>
        `<button type="button" class="rf-chip ${(p ? p.location : 'fridge') === v ? 'active' : ''}" data-loc-pick="${v}">${esc(l)}</button>`).join('');
      const unitChips = UNITS.map(un =>
        `<button type="button" class="rf-chip ${(p ? p.unit : 'шт') === un ? 'active' : ''}" data-unit="${un}">${esc(un)}</button>`).join('');
      const delBtn = isEdit && canDelete ? `<button class="btn-danger" id="fr-del">Удалить</button>` : '';
      // Shopping-list shortcut — only in Hanni (Tauri invoke); guest UI
      // has no SQLite access so the button is hidden there.
      const shopBtn = isEdit && window.__TAURI__?.core?.invoke ? `<button class="btn-secondary" id="fr-shop" title="Добавить в список покупок">🛒 В магазин</button>` : '';
      const datalistHtml = hasCatalog
        ? `<datalist id="fr-name-list">${state.catalog.map(c => `<option value="${esc(c.name)}">`).join('')}</datalist>`
        : '';
      const linkBadge = isEdit && p && p.catalog_id ? '<span class="badge badge-gray" id="fr-link-badge" title="Связан с каталогом">📚</span>' : '';
      overlay.innerHTML = `<div class="modal" style="max-width:480px;max-height:90vh;overflow-y:auto;">
        <div class="modal-title">${isEdit ? 'Изменить продукт' : 'Новый продукт'}</div>
        <div class="form-group"><label class="form-label">Название ${linkBadge}</label>
          <input class="form-input" id="fr-name" value="${esc(p ? p.name : '')}" ${hasCatalog ? 'list="fr-name-list" placeholder="Начните вводить — подскажу из каталога"' : ''}>
          ${datalistHtml}</div>
        <div class="form-group"><label class="form-label">Категория</label><div class="add-chips" data-group="cat">${catChips}</div></div>
        <div style="display:grid;grid-template-columns:1fr 2fr;gap:8px">
          <div class="form-group"><label class="form-label">Кол-во</label><input class="form-input" id="fr-qty" type="number" step="0.1" value="${p ? p.quantity ?? 1 : 1}"></div>
          <div class="form-group"><label class="form-label">Ед.</label><div class="add-chips" data-group="unit">${unitChips}</div></div>
        </div>
        <div class="form-group"><label class="form-label">Срок годности</label><input class="form-input" id="fr-exp" type="date" value="${esc(p && p.expiry_date ? p.expiry_date.slice(0,10) : '')}"></div>
        <div class="form-group"><label class="form-label">Хранение</label><div class="add-chips" data-group="loc">${locChipsHtml}</div></div>
        <div class="form-group"><label class="form-label">Заметка</label><input class="form-input" id="fr-notes" value="${esc(p ? p.notes : '')}"></div>
        <div id="fr-msg"></div>
        <div class="modal-actions">
          ${delBtn}
          ${shopBtn}
          <button class="btn-secondary" id="fr-cancel">Отмена</button>
          <button class="btn-primary" id="fr-save">${isEdit ? 'Сохранить' : 'Добавить'}</button>
        </div></div>`;
      document.body.appendChild(overlay);
      const close = () => { overlay.remove(); document.removeEventListener('keydown', escH); };
      const escH = (e) => { if (e.key === 'Escape') close(); };
      document.addEventListener('keydown', escH);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
      overlay.querySelector('#fr-cancel').onclick = close;
      // Single-select chip groups
      overlay.querySelectorAll('[data-group]').forEach(g => {
        g.dataset.value = (g.dataset.group === 'cat' ? (p ? p.category : 'other')
          : g.dataset.group === 'loc' ? (p ? p.location : 'fridge')
          : (p ? p.unit : 'шт'));
        g.addEventListener('click', (e) => {
          const btn = e.target.closest('.rf-chip'); if (!btn) return;
          g.dataset.value = btn.dataset.cat || btn.dataset.locPick || btn.dataset.unit;
          g.querySelectorAll('.rf-chip').forEach(c => c.classList.toggle('active', c === btn));
        });
      });
      // Auto-update category chip when typed name matches catalog (UX hint, server still resolves authoritative).
      const nameInput = overlay.querySelector('#fr-name');
      const catGroup = overlay.querySelector('[data-group="cat"]');
      function syncCategoryFromCatalog() {
        const match = lookupCatalog(nameInput.value);
        if (!match || !match.category) return;
        catGroup.dataset.value = match.category;
        catGroup.querySelectorAll('.rf-chip').forEach(c =>
          c.classList.toggle('active', c.dataset.cat === match.category));
      }
      if (hasCatalog) nameInput.addEventListener('change', syncCategoryFromCatalog);

      overlay.querySelector('#fr-save').onclick = async () => {
        const name = nameInput.value.trim();
        const msg = overlay.querySelector('#fr-msg');
        if (!name) { msg.innerHTML = '<div class="err">Название обязательно</div>'; return; }
        const match = lookupCatalog(name);
        const payload = {
          name,
          category: match?.category || catGroup.dataset.value || 'other',
          quantity: parseFloat(overlay.querySelector('#fr-qty').value) || 1,
          unit: overlay.querySelector('[data-group="unit"]').dataset.value || 'шт',
          expiry_date: overlay.querySelector('#fr-exp').value || null,
          location: overlay.querySelector('[data-group="loc"]').dataset.value || 'fridge',
          notes: overlay.querySelector('#fr-notes').value.trim(),
          catalog_id: match ? match.id : null,
        };
        msg.innerHTML = '<div class="muted">Сохраняем…</div>';
        try {
          if (isEdit) await backend.update(p.id, payload);
          else await backend.add(payload);
          close(); load();
        } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
      };
      const del = overlay.querySelector('#fr-del');
      if (del) del.onclick = async () => {
        if (!confirm(`Удалить "${p.name}"?`)) return;
        try { await backend.remove(p.id); close(); load(); }
        catch (e) { alert('Ошибка: ' + (e.message || e)); }
      };
      const shop = overlay.querySelector('#fr-shop');
      if (shop) shop.onclick = async () => {
        try {
          const { addShoppingItem } = await import('./shopping-list.js');
          const qty = `${overlay.querySelector('#fr-qty')?.value || ''} ${overlay.querySelector('[data-group="unit"]')?.dataset.value || ''}`.trim();
          await addShoppingItem(overlay.querySelector('#fr-name').value.trim(), qty, '');
          shop.textContent = '✓ Добавлено';
          shop.disabled = true;
        } catch (e) { alert('Ошибка: ' + (e.message || e)); }
      };
    }

    load();
  }

  window.HanniFridge = { mountInventory };
})();
