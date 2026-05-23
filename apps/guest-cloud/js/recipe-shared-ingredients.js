// recipe-shared-ingredients.js — Ingredient row with autocomplete + alternatives.
// Loaded BEFORE recipe-shared.js; exports window.HanniRecipe.ingredients.
(function () {
  const UNITS = ['г', 'кг', 'мл', 'л', 'шт', 'ст.л.', 'ч.л.', 'стакан'];
  const CAT_LABELS = {
    meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты', grain: 'Крупы',
    dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи', spice: 'Специи',
    oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', other: 'Другое',
  };
  const CAT_ORDER = ['meat', 'fish', 'veg', 'fruit', 'grain', 'dairy', 'legumes',
    'nuts', 'spice', 'oil', 'bakery', 'drinks', 'other'];

  // Blacklist level: '' | 'soft' | 'hard' (hard wins).
  const blMax = (a, b) => (a === 'hard' || b === 'hard') ? 'hard' : (a || b || '');
  function productBlockLevel(c, bl) {
    const nm = (c.name || '').toLowerCase();
    const tags = (c.tags || '').split(',').map(t => t.trim().toLowerCase()).filter(Boolean);
    let lvl = '';
    for (const e of (bl || [])) {
      const v = (e.value || '').toLowerCase();
      if (!v) continue;
      let hit = false;
      if (e.type === 'product') hit = nm === v;
      else if (e.type === 'keyword') hit = nm.includes(v);
      else if (e.type === 'category') hit = c.category === e.value;
      else if (e.type === 'tag') hit = tags.includes(v) || (c.subgroup || '').toLowerCase() === v;
      if (hit) { lvl = blMax(lvl, e.level || 'hard'); if (lvl === 'hard') return 'hard'; }
    }
    return lvl;
  }

  function renderIngredientRows(ct, catalog, blacklist, backend, initialItems) {
    ct.innerHTML = '';
    if (initialItems && initialItems.length) {
      for (const it of initialItems) ct.appendChild(createRow(catalog, blacklist, backend, ct, it));
    } else {
      ct.appendChild(createRow(catalog, blacklist, backend, ct));
    }
    const ab = document.createElement('button');
    ab.type = 'button';
    ab.className = 'btn-secondary';
    ab.textContent = '+ Ингредиент';
    ab.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
    ab.onclick = () => {
      const newRow = createRow(catalog, blacklist, backend, ct);
      ct.insertBefore(newRow, ab);
      newRow.querySelector('.ingr-name-input')?.focus();
    };
    ct.appendChild(ab);
  }

  function createRow(catalog, blacklist, backend, container, item) {
    const row = document.createElement('div');
    row.className = 'ingr-row';
    const selUnit = (item && UNITS.includes(item.unit)) ? item.unit : UNITS[0];
    row.innerHTML = `<div class="ingr-row-main">
      <input class="form-input ingr-name-input" placeholder="Найти ингредиент..." autocomplete="off">
      <input class="form-input ingr-amount-input" type="number" placeholder="100">
      <div class="ingr-unit-acc"><button type="button" class="ingr-unit-btn">${selUnit} ▾</button>
        <div class="ingr-unit-dropdown" style="display:none">${UNITS.map(un =>
          `<div class="ingr-unit-opt${un === selUnit ? ' active' : ''}" data-val="${un}">${un}</div>`).join('')}</div>
      </div>
      <button type="button" class="ingr-del-btn">&times;</button></div>
      <div class="ingr-alts"><button type="button" class="ingr-alt-add">+ ИЛИ</button></div>`;
    setupUnitDD(row);
    row.querySelector('.ingr-del-btn').onclick = () => {
      if (row.parentElement.querySelectorAll('.ingr-row').length > 1) row.remove();
    };
    const nameInput = row.querySelector('.ingr-name-input');
    const amountInput = row.querySelector('.ingr-amount-input');
    nameInput.addEventListener('focus', () => showProductPicker(row, nameInput, catalog, blacklist, backend));
    nameInput.addEventListener('input', () => showProductPicker(row, nameInput, catalog, blacklist, backend));
    nameInput.addEventListener('blur', () => setTimeout(() => closeAC(row), 180));
    // Enter in amount field on the LAST row → add new ingredient row.
    amountInput.addEventListener('keydown', (e) => {
      if (e.key !== 'Enter') return;
      const rows = container.querySelectorAll('.ingr-row');
      if (rows[rows.length - 1] !== row) return;
      e.preventDefault();
      const addBtn = container.querySelector('button.btn-secondary');
      if (addBtn) addBtn.click();
    });
    const altsEl = row.querySelector('.ingr-alts');
    const addAltBtn = altsEl.querySelector('.ingr-alt-add');
    addAltBtn.onclick = () => openAltInput(altsEl, addAltBtn);
    if (item) {
      nameInput.value = item.name || '';
      if (item.amount) amountInput.value = item.amount;
      const cid = item.catalog_id ?? item.catalogId;
      if (cid != null) row.dataset.catalogId = String(cid);
      for (const n of String(item.alternatives || '').split(',').map(s => s.trim()).filter(Boolean))
        addAltChip(altsEl, addAltBtn, n);
    }
    return row;
  }

  function openAltInput(altsEl, addBtn) {
    const inp = document.createElement('input');
    inp.className = 'form-input ingr-alt-input'; inp.placeholder = 'или…';
    addBtn.style.display = 'none'; altsEl.appendChild(inp); inp.focus();
    const finalize = () => { const v = inp.value.trim(); inp.remove(); addBtn.style.display = ''; if (v) addAltChip(altsEl, addBtn, v); };
    inp.onkeydown = e => { if (e.key === 'Enter') finalize(); else if (e.key === 'Escape') { inp.remove(); addBtn.style.display = ''; } };
    inp.onblur = finalize;
  }
  function addAltChip(altsEl, anchor, name) {
    const chip = document.createElement('span');
    chip.className = 'ingr-alt-chip'; chip.dataset.name = name; chip.textContent = name;
    const x = document.createElement('button');
    x.type = 'button'; x.className = 'ingr-alt-x'; x.textContent = '×';
    x.onclick = () => chip.remove(); chip.appendChild(x);
    altsEl.insertBefore(chip, anchor);
  }

  function setupUnitDD(row) {
    const btn = row.querySelector('.ingr-unit-btn'), dd = row.querySelector('.ingr-unit-dropdown');
    btn.onclick = (e) => { e.preventDefault(); dd.style.display = dd.style.display === 'none' ? '' : 'none'; };
    dd.querySelectorAll('.ingr-unit-opt').forEach(opt => {
      opt.onclick = () => {
        dd.querySelectorAll('.ingr-unit-opt').forEach(o => o.classList.remove('active'));
        opt.classList.add('active'); btn.textContent = `${opt.dataset.val} ▾`; dd.style.display = 'none';
      };
    });
    row.addEventListener('focusout', () => setTimeout(() => { dd.style.display = 'none'; }, 150));
  }

  function showProductPicker(row, input, catalog, blacklist, backend) {
    closeAC(row);
    const q = input.value.trim().toLowerCase();
    const visible = catalog
      .filter(c => !q || c.name.toLowerCase().includes(q));
    const dd = document.createElement('div');
    dd.className = 'ingr-autocomplete';

    // Group by CAT_ORDER for visual structure when query is empty or short.
    const byCat = {};
    for (const item of visible) {
      const cat = item.category || 'other';
      (byCat[cat] = byCat[cat] || []).push(item);
    }
    let shown = 0;
    const limit = q ? 30 : 20;
    for (const cat of CAT_ORDER) {
      if (shown >= limit) break;
      const items = byCat[cat]; if (!items?.length) continue;
      const head = document.createElement('div');
      head.className = 'ingr-ac-cat';
      head.textContent = CAT_LABELS[cat] || cat;
      dd.appendChild(head);
      for (const item of items) {
        if (shown >= limit) break;
        const lvl = productBlockLevel(item, blacklist);
        const opt = document.createElement('div');
        opt.className = `ingr-autocomplete-item ingr-product-row`;
        const ns = document.createElement('span');
        ns.textContent = lvl === 'hard' ? `🚫 ${item.name}` : lvl === 'soft' ? `👎 ${item.name}` : item.name;
        const cl = document.createElement('span');
        cl.className = `ingr-cat-label ingr-cat-${cat}`;
        cl.textContent = CAT_LABELS[cat] || cat;
        opt.append(ns, cl);
        if (lvl === 'hard') {
          opt.style.opacity = '0.5';
          opt.style.cursor = 'not-allowed';
          opt.title = 'Не ем';
        } else {
          if (lvl === 'soft') { opt.style.opacity = '0.65'; opt.title = 'Не люблю'; }
          opt.onmousedown = (e) => { e.preventDefault(); selectItem(input, item.name, row, item.id); };
        }
        dd.appendChild(opt);
        shown += 1;
      }
    }

    // Offer "+ Создать" when typed value has no exact match.
    if (q && !visible.some(i => i.name.toLowerCase() === q)) {
      const typed = input.value.trim();
      const blocked = productBlockLevel({ name: typed, tags: '', category: '' }, blacklist) === 'hard';
      const create = document.createElement('div');
      create.className = 'ingr-autocomplete-item ingr-autocomplete-create';
      create.textContent = blocked ? `🚫 «${typed}» в блэклисте` : `+ Создать «${typed}»`;
      if (blocked) create.style.color = 'var(--color-red)';
      else create.onmousedown = async (e) => {
        e.preventDefault();
        let newId;
        try {
          newId = await backend.addCatalogItem({ name: typed, category: 'other' });
          catalog.push({ id: newId, name: typed, category: 'other', tags: '' });
        } catch {}
        selectItem(input, typed, row, newId);
      };
      dd.appendChild(create);
    } else if (!shown) {
      const empty = document.createElement('div');
      empty.className = 'ingr-autocomplete-item';
      empty.style.color = 'var(--text-muted)';
      empty.textContent = 'Каталог пуст';
      dd.appendChild(empty);
    }

    row.querySelector('.ingr-row-main').appendChild(dd);
  }

  function selectItem(input, name, row, catalogId) {
    input.value = name;
    if (catalogId != null) row.dataset.catalogId = String(catalogId);
    else delete row.dataset.catalogId;
    closeAC(row); row.querySelector('.ingr-amount-input')?.focus();
  }

  function closeAC(row) { row.querySelector('.ingr-autocomplete')?.remove(); }

  function collectIngredientItems(container) {
    const items = [];
    container.querySelectorAll('.ingr-row').forEach(row => {
      const name = row.querySelector('.ingr-name-input')?.value?.trim();
      if (!name) return;
      const amount = parseFloat(row.querySelector('.ingr-amount-input')?.value) || 0;
      const unit = row.querySelector('.ingr-unit-opt.active')?.dataset.val || 'г';
      const item = { name, amount, unit };
      const cid = row.dataset.catalogId;
      if (cid) item.catalog_id = Number(cid);
      const alts = [...row.querySelectorAll('.ingr-alt-chip')].map(c => c.dataset.name).join(',');
      if (alts) item.alternatives = alts;
      items.push(item);
    });
    return items;
  }

  window.HanniRecipe = window.HanniRecipe || {};
  window.HanniRecipe.ingredients = { renderIngredientRows, collectIngredientItems };
})();
