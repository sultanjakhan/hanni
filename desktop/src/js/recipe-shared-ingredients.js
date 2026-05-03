// recipe-shared-ingredients.js — Ingredient row + single-autocomplete picker.
// Loaded as a plain <script> BEFORE recipe-shared.js. Registers helpers under
// window.HanniRecipe.ingredients so the main module can call them.
(function () {
  const UNITS = ['г', 'кг', 'мл', 'л', 'шт', 'ст.л.', 'ч.л.', 'стакан'];
  const CAT_LABELS = {
    meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты', grain: 'Крупы',
    dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи', spice: 'Специи',
    oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', other: 'Другое',
  };
  const CAT_ORDER = ['meat', 'fish', 'veg', 'fruit', 'grain', 'dairy', 'legumes',
    'nuts', 'spice', 'oil', 'bakery', 'drinks', 'other'];

  // ── Blacklist helpers (no-op when blacklist is empty / missing) ──
  const isIngrBlocked = (name, bl) => (bl || []).some(e =>
    e.type === 'ingredient' && (e.value || '').toLowerCase() === name.toLowerCase());
  const isTagBlocked = (tag, bl) => tag && (bl || []).some(e =>
    e.type === 'tag' && (e.value || '').toLowerCase() === tag.toLowerCase());
  const isCatBlocked = (cat, bl) => (bl || []).some(e =>
    e.type === 'category' && e.value === cat);
  const keywordBlocked = (typed, bl) => (bl || []).some(e =>
    e.type === 'keyword' && typed.toLowerCase().includes((e.value || '').toLowerCase()));
  const productBlocked = (c, bl) => isIngrBlocked(c.name, bl)
    || isCatBlocked(c.category, bl)
    || (c.tags || '').split(',').some(t => isTagBlocked(t.trim(), bl));

  // ── Ingredient rows ──
  function renderIngredientRows(ct, catalog, blacklist, backend) {
    ct.innerHTML = '';
    ct.appendChild(createRow(catalog, blacklist, backend, ct));
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

  function createRow(catalog, blacklist, backend, container) {
    const row = document.createElement('div');
    row.className = 'ingr-row';
    const selUnit = UNITS[0];
    row.innerHTML = `<div class="ingr-row-main">
      <input class="form-input ingr-name-input" placeholder="Найти ингредиент..." autocomplete="off">
      <input class="form-input ingr-amount-input" type="number" placeholder="100">
      <div class="ingr-unit-acc"><button type="button" class="ingr-unit-btn">${selUnit} ▾</button>
        <div class="ingr-unit-dropdown" style="display:none">${UNITS.map(un =>
          `<div class="ingr-unit-opt${un === selUnit ? ' active' : ''}" data-val="${un}">${un}</div>`).join('')}</div>
      </div>
      <button type="button" class="ingr-del-btn">&times;</button></div>`;
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
    return row;
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
      .filter(c => !productBlocked(c, blacklist))
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
        const opt = document.createElement('div');
        opt.className = `ingr-autocomplete-item ingr-product-row`;
        const ns = document.createElement('span');
        ns.textContent = item.name;
        const cl = document.createElement('span');
        cl.className = `ingr-cat-label ingr-cat-${cat}`;
        cl.textContent = CAT_LABELS[cat] || cat;
        opt.append(ns, cl);
        opt.onmousedown = (e) => { e.preventDefault(); selectItem(input, item.name, row, item.id); };
        dd.appendChild(opt);
        shown += 1;
      }
    }

    // Offer "+ Создать" when typed value has no exact match.
    if (q && !visible.some(i => i.name.toLowerCase() === q)) {
      const typed = input.value.trim();
      const blocked = isIngrBlocked(typed, blacklist) || keywordBlocked(typed, blacklist);
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
      items.push(item);
    });
    return items;
  }

  window.HanniRecipe = window.HanniRecipe || {};
  window.HanniRecipe.ingredients = { renderIngredientRows, collectIngredientItems };
})();
