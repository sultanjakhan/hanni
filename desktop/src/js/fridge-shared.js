// fridge-shared.js — Backend-agnostic fridge (inventory) UI shared by Hanni & guest.
// Loaded as a plain <script>. Registers `window.HanniFridge.mountInventory({ el, backend })`.
//
// Backend interface (Promise-returning):
//   list()                        → [{ id, name, category, quantity, unit, expiry_date, location, notes }]
//   add?(payload)                 → void   // optional — hides add button if missing
//   update?(id, payload)          → void   // optional — hides edit
//   remove?(id)                   → void   // optional — hides delete
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
    const state = { items: [], loc: 'all' };
    const canAdd = !!backend.add, canEdit = !!backend.update, canDelete = !!backend.remove;

    async function load() {
      try { state.items = await backend.list(); render(); }
      catch (e) { el.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`; }
    }

    function locFilter(p) { return state.loc === 'all' || p.location === state.loc; }

    function cardHtml(p) {
      const cls = CAT_COLORS[p.category] || 'gray';
      const label = CAT_LABELS[p.category] || p.category;
      const qty = `${p.quantity ?? 1} ${p.unit || 'шт'}`.trim();
      const loc = LOC_LABELS[p.location] || p.location;
      const exp = expiryBadge(p.expiry_date);
      return `<div class="product-card" data-id="${p.id}">
        <div class="product-card-name">${esc(p.name)}</div>
        <div class="product-card-tags">
          <span class="product-card-cat product-cat-${cls}">${esc(label)}</span>
          <span class="product-card-tag">${esc(qty)}</span>
          <span class="product-card-tag">${esc(loc)}</span>
          ${exp}
        </div>
        ${p.notes ? `<div style="margin-top:6px;font-size:12px;color:var(--text-muted);font-style:italic">${esc(p.notes)}</div>` : ''}
      </div>`;
    }

    function locChips() {
      const opts = { all: 'Все', ...LOC_LABELS };
      return Object.entries(opts).map(([v, l]) =>
        `<button type="button" class="rf-chip ${state.loc === v ? 'active' : ''}" data-loc="${v}">${esc(l)}</button>`
      ).join('');
    }

    function render() {
      const filtered = state.items.filter(locFilter);
      const grid = filtered.length
        ? `<div class="product-grid">${filtered.map(cardHtml).join('')}</div>`
        : '<div class="empty">Холодильник пуст.</div>';
      const addBtn = canAdd ? `<button class="btn-primary" id="fr-add">+ Продукт</button>` : '';
      el.innerHTML = `
        <div class="recipe-filter-bar" style="position:static;padding:0;margin:0 0 12px">
          <div style="display:flex;gap:6px;flex-wrap:wrap;flex:1">${locChips()}</div>
          ${addBtn}
        </div>
        <h2>Холодильник (${filtered.length})</h2>
        ${grid}`;
      el.querySelectorAll('[data-loc]').forEach(b => b.onclick = () => { state.loc = b.dataset.loc; render(); });
      el.querySelectorAll('.product-card').forEach(c =>
        c.onclick = () => openEditModal(state.items.find(p => p.id === parseInt(c.dataset.id))));
      const a = el.querySelector('#fr-add'); if (a) a.onclick = () => openAddModal();
    }

    function openAddModal() { openModal(null); }
    function openEditModal(p) {
      if (!canEdit && !canDelete) return; // pure view → no popup
      openModal(p);
    }

    function openModal(p) {
      const isEdit = !!p;
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      const catChips = Object.entries(CAT_LABELS).map(([v, l]) =>
        `<button type="button" class="rf-chip ${(p ? p.category : 'other') === v ? 'active' : ''}" data-cat="${v}">${esc(l)}</button>`).join('');
      const locChipsHtml = Object.entries(LOC_LABELS).map(([v, l]) =>
        `<button type="button" class="rf-chip ${(p ? p.location : 'fridge') === v ? 'active' : ''}" data-loc-pick="${v}">${esc(l)}</button>`).join('');
      const unitChips = UNITS.map(un =>
        `<button type="button" class="rf-chip ${(p ? p.unit : 'шт') === un ? 'active' : ''}" data-unit="${un}">${esc(un)}</button>`).join('');
      const delBtn = isEdit && canDelete ? `<button class="btn-danger" id="fr-del">Удалить</button>` : '';
      overlay.innerHTML = `<div class="modal" style="max-width:480px;max-height:90vh;overflow-y:auto;">
        <div class="modal-title">${isEdit ? 'Изменить продукт' : 'Новый продукт'}</div>
        <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="fr-name" value="${esc(p ? p.name : '')}"></div>
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
      overlay.querySelector('#fr-save').onclick = async () => {
        const name = overlay.querySelector('#fr-name').value.trim();
        const msg = overlay.querySelector('#fr-msg');
        if (!name) { msg.innerHTML = '<div class="err">Название обязательно</div>'; return; }
        const payload = {
          name,
          category: overlay.querySelector('[data-group="cat"]').dataset.value || 'other',
          quantity: parseFloat(overlay.querySelector('#fr-qty').value) || 1,
          unit: overlay.querySelector('[data-group="unit"]').dataset.value || 'шт',
          expiry_date: overlay.querySelector('#fr-exp').value || null,
          location: overlay.querySelector('[data-group="loc"]').dataset.value || 'fridge',
          notes: overlay.querySelector('#fr-notes').value.trim(),
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
    }

    load();
  }

  window.HanniFridge = { mountInventory };
})();
