// recipe-shared.js — Backend-agnostic add-recipe modal shared between Hanni & guest.
// Loaded as a plain <script> in both. Registers `window.HanniRecipe.showAddRecipeModal({ backend, onSaved })`.
// Backend interface (Promise-returning):
//   getCatalog()  → [{name, category, tags?}]
//   getCuisines() → [{id, code, name, emoji}]   // id may equal code
//   getBlacklist? → []                          // optional
//   addCatalogItem({name, category}) → void
//   addCuisine({code, name, emoji}) → void
//   createRecipe(snake_case_payload) → void
(function () {
  const UNITS = ['г', 'кг', 'мл', 'л', 'шт', 'ст.л.', 'ч.л.', 'стакан'];
  const ACTIONS = ['Жарить', 'Варить', 'Тушить', 'Запекать', 'Нарезать', 'Смешать',
    'Обжарить', 'Залить', 'Добавить', 'Довести до кипения', 'Остудить',
    'Натереть', 'Замариновать', 'Отварить', 'Подать'];
  const CAT_LABELS = { meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты',
    grain: 'Крупы', dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи',
    spice: 'Специи', oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', other: 'Другое' };
  const CAT_ORDER = ['meat', 'fish', 'veg', 'fruit', 'grain', 'dairy', 'legumes', 'nuts',
    'spice', 'oil', 'bakery', 'drinks', 'other'];

  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  // ── Blacklist helpers (no-op when blacklist is empty / missing) ──
  const productBlocked = (c, bl) => isIngrBlocked(c.name, bl)
    || (c.tags || '').split(',').some(t => isTagBlocked(t.trim(), bl));
  function isIngrBlocked(name, bl) { return (bl || []).some(e => e.type === 'ingredient' && (e.value || '').toLowerCase() === name.toLowerCase()); }
  function isTagBlocked(tag, bl) { return tag && (bl || []).some(e => e.type === 'tag' && (e.value || '').toLowerCase() === tag.toLowerCase()); }
  function isCatBlocked(cat, bl) { return (bl || []).some(e => e.type === 'category' && e.value === cat); }
  const keywordBlocked = (typed, bl) => (bl || []).some(e => e.type === 'keyword' && typed.toLowerCase().includes((e.value || '').toLowerCase()));

  // ── Public entry ──
  async function showAddRecipeModal({ backend, onSaved }) {
    const [catalog, cuisines, blacklist] = await Promise.all([
      backend.getCatalog().catch(() => []),
      backend.getCuisines().catch(() => []),
      (backend.getBlacklist ? backend.getBlacklist() : Promise.resolve([])).catch(() => []),
    ]);
    buildModal({ backend, onSaved, catalog, cuisines, blacklist });
  }

  function chip(id, label, active) { return `<button type="button" class="rf-chip${active ? ' active' : ''}" data-val="${id}">${esc(label)}</button>`; }
  function acc(title, field, content, open) {
    return `<div class="rf-acc" data-field="${field}">
      <div class="rf-acc-header">${title}<span class="rf-acc-arrow">${open ? '▾' : '▸'}</span></div>
      <div class="rf-acc-body" style="display:${open ? '' : 'none'}">${content}</div></div>`;
  }

  function buildModal({ backend, onSaved, catalog, cuisines, blacklist }) {
    const state = { tags: new Set(['universal']), diff: 'easy', cuisine: 'kz' };
    const mealsHtml = ['breakfast:Завтрак', 'lunch:Обед', 'dinner:Ужин', 'universal:Универсал']
      .map(s => { const [id, l] = s.split(':'); return chip(id, l, state.tags.has(id)); }).join('');
    const diffsHtml = ['easy:Лёгкий', 'medium:Средний', 'hard:Сложный']
      .map(s => { const [id, l] = s.split(':'); return chip(id, l, state.diff === id); }).join('');
    const defC = cuisines.find(c => (c.id || c.code) === state.cuisine);

    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:90vh;overflow-y:auto;">
      <div class="modal-title">Новый рецепт</div>
      <div class="form-group"><label class="form-label">Название <span class="req">*</span></label>
        <input class="form-input" id="r-name" placeholder="Название рецепта"></div>
      <div class="form-group"><label class="form-label">Ингредиенты <span class="req">*</span></label>
        <div id="r-ingr-rows"></div></div>
      <div class="form-group"><label class="form-label">Приготовление <span class="req">*</span></label>
        <div id="r-steps"></div></div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
        <div class="form-group"><label class="form-label">Подготовка (мин)</label><input class="form-input" id="r-prep" type="number" value="10"></div>
        <div class="form-group"><label class="form-label">Готовка (мин)</label><input class="form-input" id="r-cook" type="number" value="20"></div>
        <div class="form-group"><label class="form-label">Порции</label><input class="form-input" id="r-serv" type="number" value="2"></div>
        <div class="form-group"><label class="form-label">Калории</label><input class="form-input" id="r-cal" type="number"></div>
      </div>
      ${acc('Тип блюда', 'tags', `<div class="add-chips" data-field="tags">${mealsHtml}</div>`, false)}
      ${acc('Сложность', 'diff', `<div class="add-chips" data-field="diff">${diffsHtml}</div>`, false)}
      <div class="form-group"><label class="form-label">Кухня</label>
        <div style="position:relative;">
          <input class="form-input" id="r-cuisine-input" placeholder="Поиск кухни..." value="${defC ? `${defC.emoji} ${esc(defC.name)}` : ''}" autocomplete="off">
          <div class="ingr-autocomplete" id="r-cuisine-dd" style="display:none;width:100%;"></div>
        </div>
        <div id="new-cuisine-form" style="display:none;margin-top:6px;"></div>
      </div>
      ${acc('БЖУ и оценки', 'extra', `<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
        <div class="form-group"><label class="form-label">Полезность (1-10)</label><input class="form-input" id="r-health" type="number" min="1" max="10" value="5"></div>
        <div class="form-group"><label class="form-label">Цена (1-10)</label><input class="form-input" id="r-price" type="number" min="1" max="10" value="5"></div></div>
        <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;">
        <div class="form-group"><label class="form-label">Белки (г)</label><input class="form-input" id="r-protein" type="number" value="0"></div>
        <div class="form-group"><label class="form-label">Жиры (г)</label><input class="form-input" id="r-fat" type="number" value="0"></div>
        <div class="form-group"><label class="form-label">Углеводы (г)</label><input class="form-input" id="r-carbs" type="number" value="0"></div></div>`, false)}
      <div class="modal-actions">
        <button class="btn-secondary" id="r-cancel">Отмена</button>
        <button class="btn-primary" id="r-save">Сохранить</button>
      </div></div>`;

    document.body.appendChild(overlay);
    const close = () => { overlay.remove(); document.removeEventListener('keydown', escH); };
    const escH = (e) => { if (e.key === 'Escape') close(); };
    document.addEventListener('keydown', escH);
    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
    overlay.querySelector('#r-cancel').onclick = close;

    renderIngredientRows(overlay.querySelector('#r-ingr-rows'), catalog, blacklist, backend);
    renderStepsRows(overlay.querySelector('#r-steps'),
      () => collectIngredientItems(overlay.querySelector('#r-ingr-rows')).map(i => i.name));
    overlay.querySelectorAll('.rf-acc-header').forEach(hdr => hdr.onclick = () => {
      const body = hdr.nextElementSibling, open = body.style.display !== 'none';
      body.style.display = open ? 'none' : '';
      hdr.querySelector('.rf-acc-arrow').textContent = open ? '▸' : '▾';
    });
    bindMultiChips(overlay, 'tags', state);
    bindChips(overlay, 'diff', state);
    bindCuisineInput(overlay, state, cuisines, backend);

    overlay.querySelector('#r-save').onclick = async () => {
      const nameEl = overlay.querySelector('#r-name');
      const name = nameEl?.value?.trim();
      if (!name) { nameEl.classList.add('input-error'); nameEl.focus(); return; }
      const ingredient_items = collectIngredientItems(overlay.querySelector('#r-ingr-rows'));
      const v = id => parseInt(overlay.querySelector(`#${id}`)?.value) || 0;
      try {
        await backend.createRecipe({
          name, description: '',
          instructions: collectSteps(overlay.querySelector('#r-steps')),
          prep_time: v('r-prep'), cook_time: v('r-cook'), servings: v('r-serv') || 1, calories: v('r-cal'),
          tags: [...state.tags].join(','), difficulty: state.diff, cuisine: state.cuisine,
          health_score: v('r-health') || 5, price_score: v('r-price') || 5,
          protein: v('r-protein'), fat: v('r-fat'), carbs: v('r-carbs'),
          ingredient_items,
        });
        close(); if (onSaved) await onSaved();
      } catch (e) { alert('Ошибка: ' + (e.message || e)); }
    };
    setTimeout(() => overlay.querySelector('#r-name')?.focus(), 50);
  }

  function bindChips(overlay, field, state) {
    overlay.querySelector(`[data-field="${field}"]`)?.querySelectorAll('.rf-chip').forEach(btn => {
      btn.onclick = (e) => {
        e.preventDefault();
        state[field] = btn.dataset.val;
        btn.closest('.add-chips').querySelectorAll('.rf-chip').forEach(b =>
          b.classList.toggle('active', b.dataset.val === state[field]));
      };
    });
  }
  function bindMultiChips(overlay, field, state) {
    overlay.querySelector(`[data-field="${field}"]`)?.querySelectorAll('.rf-chip').forEach(btn => {
      btn.onclick = (e) => {
        e.preventDefault();
        const v = btn.dataset.val;
        if (state[field].has(v)) { if (state[field].size > 1) state[field].delete(v); }
        else state[field].add(v);
        btn.closest('.add-chips').querySelectorAll('.rf-chip').forEach(b =>
          b.classList.toggle('active', state[field].has(b.dataset.val)));
      };
    });
  }
  function bindCuisineInput(overlay, state, cuisines, backend) {
    const inp = overlay.querySelector('#r-cuisine-input');
    const dd = overlay.querySelector('#r-cuisine-dd');
    const form = overlay.querySelector('#new-cuisine-form');
    function showDD(q) {
      const lc = q.toLowerCase();
      const matches = lc ? cuisines.filter(c => c.name.toLowerCase().includes(lc)) : cuisines;
      dd.innerHTML = matches.map(c => `<div class="ingr-autocomplete-item" data-id="${esc(c.id || c.code)}">${c.emoji} ${esc(c.name)}</div>`).join('')
        + `<div class="ingr-autocomplete-item ingr-autocomplete-create" data-id="__new__">+ Новая кухня</div>`;
      dd.style.display = '';
      dd.querySelectorAll('.ingr-autocomplete-item').forEach(opt => {
        opt.onmousedown = (e) => {
          e.preventDefault(); dd.style.display = 'none';
          if (opt.dataset.id === '__new__') return showNewCuisineForm();
          const c = cuisines.find(x => (x.id || x.code) === opt.dataset.id);
          if (c) { state.cuisine = c.id || c.code; inp.value = `${c.emoji} ${c.name}`; }
        };
      });
    }
    const show = () => showDD(inp.value.trim());
    inp.addEventListener('focus', show); inp.addEventListener('input', show);
    inp.addEventListener('blur', () => setTimeout(() => { dd.style.display = 'none'; }, 150));
    function showNewCuisineForm() {
      form.style.display = '';
      form.innerHTML = `<div style="display:flex;gap:6px;align-items:center;">
        <input class="form-input" id="nc-name" placeholder="Название" style="flex:1;">
        <input class="form-input" id="nc-emoji" placeholder="🌍" style="width:48px;text-align:center;">
        <button type="button" class="btn-primary" id="nc-save" style="padding:4px 10px;font-size:12px;">OK</button>
        <button type="button" class="btn-secondary" id="nc-cancel" style="padding:4px 8px;font-size:12px;">✕</button></div>`;
      form.querySelector('#nc-cancel').onclick = () => { form.style.display = 'none'; };
      form.querySelector('#nc-save').onclick = async () => {
        const n = form.querySelector('#nc-name')?.value?.trim(); if (!n) return;
        const em = form.querySelector('#nc-emoji')?.value?.trim() || '🌍';
        const code = n.toLowerCase().replace(/\s+/g, '_').slice(0, 20);
        try {
          await backend.addCuisine({ code, name: n, emoji: em });
          cuisines.push({ id: code, code, name: n, emoji: em });
          state.cuisine = code; form.style.display = 'none'; inp.value = `${em} ${n}`;
        } catch (e) { alert('Ошибка: ' + (e.message || e)); }
      };
    }
  }

  // ── Ingredient rows ──
  function renderIngredientRows(ct, catalog, blacklist, backend) {
    ct.innerHTML = '';
    ct.appendChild(createRow(catalog, blacklist, backend));
    const ab = document.createElement('button');
    ab.type = 'button';
    ab.className = 'btn-secondary'; ab.textContent = '+ Ингредиент';
    ab.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
    ab.onclick = () => { ct.insertBefore(createRow(catalog, blacklist, backend), ab); };
    ct.appendChild(ab);
  }
  function createRow(catalog, blacklist, backend) {
    const row = document.createElement('div');
    row.className = 'ingr-row';
    const selUnit = UNITS[0];
    row.innerHTML = `<div class="ingr-row-main">
      <input class="form-input ingr-name-input" placeholder="Категория..." autocomplete="off" readonly>
      <input class="form-input ingr-amount-input" type="number" placeholder="100">
      <div class="ingr-unit-acc"><button type="button" class="ingr-unit-btn">${selUnit} ▾</button>
        <div class="ingr-unit-dropdown" style="display:none">${UNITS.map(un => `<div class="ingr-unit-opt${un === selUnit ? ' active' : ''}" data-val="${un}">${un}</div>`).join('')}</div>
      </div>
      <button type="button" class="ingr-del-btn">&times;</button></div>`;
    setupUnitDD(row);
    row.querySelector('.ingr-del-btn').onclick = () => {
      if (row.parentElement.querySelectorAll('.ingr-row').length > 1) row.remove();
    };
    const nameInput = row.querySelector('.ingr-name-input');
    nameInput.addEventListener('focus', () => openPicker(row, nameInput, catalog, blacklist, backend));
    nameInput.addEventListener('blur', () => setTimeout(() => closeAC(row), 180));
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
  function openPicker(row, input, catalog, blacklist, backend) {
    if (row.dataset.cat) showProductList(row, input, catalog, row.dataset.cat, blacklist, backend);
    else showCategoryPicker(row, input, catalog, blacklist, backend);
  }
  function showCategoryPicker(row, input, catalog, blacklist, backend) {
    closeAC(row);
    const dd = document.createElement('div');
    dd.className = 'ingr-autocomplete ingr-cat-picker';
    for (const cat of CAT_ORDER) {
      if (isCatBlocked(cat, blacklist)) continue;
      const count = catalog.filter(c => c.category === cat && !productBlocked(c, blacklist)).length;
      if (!count) continue;
      const opt = document.createElement('div');
      opt.className = 'ingr-autocomplete-item ingr-cat-btn';
      opt.innerHTML = `${esc(CAT_LABELS[cat] || cat)} <span class="ingr-cat-count">(${count})</span>`;
      opt.onmousedown = (e) => {
        e.preventDefault(); row.dataset.cat = cat; input.readOnly = false;
        input.placeholder = `Поиск в ${CAT_LABELS[cat]}...`; input.value = '';
        showProductList(row, input, catalog, cat, blacklist, backend); input.focus();
      };
      dd.appendChild(opt);
    }
    row.querySelector('.ingr-row-main').appendChild(dd);
  }
  function showProductList(row, input, catalog, cat, blacklist, backend) {
    closeAC(row);
    const q = input.value.trim().toLowerCase();
    const items = catalog.filter(c => c.category === cat && !productBlocked(c, blacklist)
      && (!q || c.name.toLowerCase().includes(q)));
    const dd = document.createElement('div');
    dd.className = 'ingr-autocomplete';
    const back = document.createElement('div');
    back.className = 'ingr-autocomplete-item ingr-back-link';
    back.textContent = `← ${CAT_LABELS[cat]}`;
    back.onmousedown = (e) => {
      e.preventDefault(); delete row.dataset.cat; input.readOnly = true;
      input.placeholder = 'Категория...'; input.value = '';
      showCategoryPicker(row, input, catalog, blacklist, backend);
    };
    dd.appendChild(back);
    for (const item of items.slice(0, 15)) {
      const opt = document.createElement('div');
      opt.className = 'ingr-autocomplete-item ingr-product-row';
      const ns = document.createElement('span');
      ns.textContent = item.name;
      ns.onmousedown = (e) => { e.preventDefault(); selectItem(input, item.name, row); };
      opt.appendChild(ns);
      dd.appendChild(opt);
    }
    if (q && !items.some(i => i.name.toLowerCase() === q)) {
      const create = document.createElement('div');
      create.className = 'ingr-autocomplete-item ingr-autocomplete-create';
      const typed = input.value.trim();
      const blocked = isIngrBlocked(typed, blacklist) || keywordBlocked(typed, blacklist);
      create.textContent = blocked ? `🚫 «${typed}» в блэклисте` : `+ Создать «${typed}»`;
      if (blocked) create.style.color = 'var(--color-red)';
      create.onmousedown = async (e) => {
        e.preventDefault(); if (blocked) return;
        try { await backend.addCatalogItem({ name: typed, category: cat });
          catalog.push({ name: typed, category: cat, tags: '' }); } catch {}
        selectItem(input, typed, row);
      };
      dd.appendChild(create);
    }
    row.querySelector('.ingr-row-main').appendChild(dd);
    input.oninput = () => showProductList(row, input, catalog, cat, blacklist, backend);
  }
  function selectItem(input, name, row) {
    input.value = name; input.readOnly = true; input.oninput = null;
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
      items.push({ name, amount, unit });
    });
    return items;
  }

  // ── Step rows ──
  function renderStepsRows(container, ingredientsFn) {
    container.innerHTML = '';
    container.appendChild(createStepRow(ingredientsFn));
    const addBtn = document.createElement('button');
    addBtn.type = 'button';
    addBtn.className = 'btn-secondary';
    addBtn.textContent = '+ Шаг';
    addBtn.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
    addBtn.onclick = () => { container.insertBefore(createStepRow(ingredientsFn), addBtn); updateNumbers(container); };
    container.appendChild(addBtn);
  }
  function createStepRow(ingredientsFn) {
    const row = document.createElement('div');
    row.className = 'step-row';
    row.innerHTML = `
      <span class="step-num">1.</span>
      <div class="step-fields">
        <div class="step-line">
          <div class="step-acc-wrap">
            <button type="button" class="step-acc-btn step-prod-btn">Продукт ▾</button>
            <div class="step-dropdown" data-for="prod" style="display:none"></div>
          </div>
          <div class="step-acc-wrap">
            <button type="button" class="step-acc-btn step-action-btn">Действие ▾</button>
            <div class="step-dropdown" data-for="action" style="display:none">
              ${ACTIONS.map(a => `<div class="step-dd-opt" data-val="${a}">${a}</div>`).join('')}
            </div>
          </div>
          <input class="form-input step-time-input" type="number" placeholder="мин">
        </div>
        <input class="form-input step-note-input" placeholder="Доп. (вращать каждые N мин, помешивать...)">
      </div>
      <button type="button" class="ingr-del-btn step-del">&times;</button>`;
    setupStepDD(row, '.step-action-btn', '[data-for="action"]', (val) => {
      row.querySelector('.step-action-btn').textContent = `${val} ▾`;
    });
    const prodBtn = row.querySelector('.step-prod-btn');
    const prodDD = row.querySelector('[data-for="prod"]');
    prodBtn.onclick = (e) => {
      e.preventDefault();
      const open = prodDD.style.display !== 'none';
      if (open) { prodDD.style.display = 'none'; return; }
      const names = ingredientsFn();
      prodDD.innerHTML = names.length
        ? names.map(n => `<div class="step-dd-opt" data-val="${n}">${n}</div>`).join('')
        : '<div class="step-dd-opt" style="color:var(--text-muted)">Добавьте ингредиенты</div>';
      prodDD.style.display = '';
      prodDD.querySelectorAll('.step-dd-opt[data-val]').forEach(opt => {
        opt.onclick = () => { prodBtn.textContent = `${opt.dataset.val} ▾`; prodDD.style.display = 'none'; };
      });
    };
    row.addEventListener('focusout', () => setTimeout(() => { prodDD.style.display = 'none'; }, 150));
    row.querySelector('.step-del').onclick = () => {
      if (row.parentElement.querySelectorAll('.step-row').length > 1) {
        row.remove(); updateNumbers(row.parentElement || document);
      }
    };
    return row;
  }
  function setupStepDD(row, btnSel, ddSel, onSelect) {
    const btn = row.querySelector(btnSel), dd = row.querySelector(ddSel);
    btn.onclick = (e) => { e.preventDefault(); dd.style.display = dd.style.display === 'none' ? '' : 'none'; };
    dd.querySelectorAll('.step-dd-opt').forEach(opt => {
      opt.onclick = () => { onSelect(opt.dataset.val); dd.style.display = 'none'; };
    });
    row.addEventListener('focusout', () => setTimeout(() => { dd.style.display = 'none'; }, 150));
  }
  function updateNumbers(container) {
    container.querySelectorAll('.step-row').forEach((row, i) => {
      const num = row.querySelector('.step-num');
      if (num) num.textContent = `${i + 1}.`;
    });
  }
  function collectSteps(container) {
    const steps = [];
    container.querySelectorAll('.step-row').forEach(row => {
      const prod = row.querySelector('.step-prod-btn')?.textContent?.replace(' ▾', '').trim();
      const action = row.querySelector('.step-action-btn')?.textContent?.replace(' ▾', '').trim();
      const time = row.querySelector('.step-time-input')?.value?.trim();
      const note = row.querySelector('.step-note-input')?.value?.trim();
      const parts = [];
      if (prod && prod !== 'Продукт') parts.push(prod);
      if (action && action !== 'Действие') parts.push(action.toLowerCase());
      if (time) parts.push(`${time} мин`);
      if (note) parts.push(note);
      if (parts.length) steps.push(parts.join(', '));
    });
    return steps.join('\n');
  }

  window.HanniRecipe = { showAddRecipeModal };
})();
