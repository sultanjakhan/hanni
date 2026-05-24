// recipe-shared.js — Backend-agnostic add-recipe modal shared between Hanni & guest.
// Loaded as a plain <script> AFTER recipe-shared-ingredients.js. Registers
// `window.HanniRecipe.showAddRecipeModal({ backend, onSaved, recipe? })`.
// Backend (Promise-returning): getCatalog, getCuisines, getBlacklist?,
// addCatalogItem, addCuisine, createRecipe(payload), updateRecipe(id, payload).
(function () {
  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  async function showAddRecipeModal({ backend, onSaved, recipe }) {
    const [catalog, cuisines, blacklist] = await Promise.all([
      backend.getCatalog().catch(() => []),
      backend.getCuisines().catch(() => []),
      (backend.getBlacklist ? backend.getBlacklist() : Promise.resolve([])).catch(() => []),
    ]);
    buildModal({ backend, onSaved, catalog, cuisines, blacklist, recipe });
  }

  function chip(id, label, active) {
    return `<button type="button" class="rf-chip${active ? ' active' : ''}" data-val="${id}">${esc(label)}</button>`;
  }
  function acc(title, field, content, open) {
    return `<div class="rf-acc" data-field="${field}">
      <div class="rf-acc-header">${title}<span class="rf-acc-arrow">${open ? '▾' : '▸'}</span></div>
      <div class="rf-acc-body" style="display:${open ? '' : 'none'}">${content}</div></div>`;
  }

  function buildModal({ backend, onSaved, catalog, cuisines, blacklist, recipe }) {
    const ingrApi = (window.HanniRecipe && window.HanniRecipe.ingredients) || null;
    const stepsApi = (window.HanniRecipe && window.HanniRecipe.steps) || null;
    if (!ingrApi) { alert('recipe-shared-ingredients.js не загружен'); return; }

    const r = recipe || {};
    const imgApi = (window.HanniRecipe && window.HanniRecipe.image) || null;
    const state = { tags: new Set((r.tags || 'universal').split(',').map(x => x.trim()).filter(Boolean)), diff: r.difficulty || 'easy', cuisine: r.cuisine || 'kz', image: r.image || '' };
    const mealsHtml = ['breakfast:Завтрак', 'lunch:Обед', 'dinner:Ужин', 'universal:Универсал']
      .map(s => { const [id, l] = s.split(':'); return chip(id, l, state.tags.has(id)); }).join('');
    const diffsHtml = ['easy:Лёгкий', 'medium:Средний', 'hard:Сложный']
      .map(s => { const [id, l] = s.split(':'); return chip(id, l, state.diff === id); }).join('');
    const defC = cuisines.find(c => c.code === state.cuisine);
    const stepsPaneHtml = stepsApi ? `<div id="r-steps"></div>`
      : `<textarea class="form-textarea" id="r-instr" rows="6" placeholder="1. Сварить картофель 20 мин.\n2. Натереть на тёрке."></textarea>`;

    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal recipe-wizard" style="max-width:560px;max-height:90vh;overflow-y:auto;">
      <div class="modal-title">${recipe ? 'Изменить рецепт' : 'Новый рецепт'}</div>
      <div class="rw-nav">
        <div class="rw-step active" data-step="1">1 · Блюдо</div>
        <div class="rw-step" data-step="2">2 · Продукты</div>
        <div class="rw-step" data-step="3">3 · Приготовление</div>
      </div>
      <div class="rw-pane" data-pane="1">
        <div class="form-group"><label class="form-label">Название <span class="req">*</span></label>
          <input class="form-input" id="r-name" placeholder="Название рецепта" value="${esc(r.name || '')}"></div>
        ${imgApi ? imgApi.fieldHtml(r) : ''}
        <div class="form-group"><label class="form-label">Тип блюда</label><div class="add-chips" data-field="tags">${mealsHtml}</div></div>
        <div class="form-group"><label class="form-label">Сложность</label><div class="add-chips" data-field="diff">${diffsHtml}</div></div>
        <div class="form-group"><label class="form-label">Кухня</label>
          <div style="position:relative;">
            <input class="form-input" id="r-cuisine-input" placeholder="Поиск кухни..." value="${defC ? `${defC.emoji} ${esc(defC.name)}` : ''}" autocomplete="off">
            <div class="ingr-autocomplete" id="r-cuisine-dd" style="display:none;width:100%;"></div>
          </div>
          <div id="new-cuisine-form" style="display:none;margin-top:6px;"></div>
        </div>
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
          <div class="form-group"><label class="form-label">Подготовка (мин)</label><input class="form-input" id="r-prep" type="number" value="${r.prep_time ?? 10}"></div>
          <div class="form-group"><label class="form-label">Готовка (мин)</label><input class="form-input" id="r-cook" type="number" value="${r.cook_time ?? 20}"></div>
          <div class="form-group"><label class="form-label">Порции</label><input class="form-input" id="r-serv" type="number" value="${r.servings ?? 2}"></div>
          <div class="form-group"><label class="form-label">Ккал / 100 г</label><input class="form-input" id="r-cal" type="number" value="${r.calories ?? ''}"></div>
        </div>
        ${acc('Оценки и КБЖУ (на 100 г)', 'extra', `<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
          <div class="form-group"><label class="form-label">Полезность (1-10)</label><input class="form-input" id="r-health" type="number" min="1" max="10" value="${r.health_score ?? 5}"></div>
          <div class="form-group"><label class="form-label">Цена (1-10)</label><input class="form-input" id="r-price" type="number" min="1" max="10" value="${r.price_score ?? 5}"></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;">
          <div class="form-group"><label class="form-label">Белки (г/100г)</label><input class="form-input" id="r-protein" type="number" value="${r.protein ?? 0}"></div>
          <div class="form-group"><label class="form-label">Жиры (г/100г)</label><input class="form-input" id="r-fat" type="number" value="${r.fat ?? 0}"></div>
          <div class="form-group"><label class="form-label">Углеводы (г/100г)</label><input class="form-input" id="r-carbs" type="number" value="${r.carbs ?? 0}"></div></div>`, false)}
      </div>
      <div class="rw-pane" data-pane="2" style="display:none">
        <label class="form-label">Ингредиенты</label>
        <div id="r-ingr-rows"></div>
      </div>
      <div class="rw-pane" data-pane="3" style="display:none">
        <label class="form-label">Приготовление по шагам</label>
        ${stepsPaneHtml}
      </div>
      <div class="modal-actions">
        <button class="btn-secondary" id="r-cancel">Отмена</button>
        <button class="btn-secondary" id="r-back" style="display:none">← Назад</button>
        <button class="btn-primary" id="r-next">Далее →</button>
        <button class="btn-primary" id="r-save" style="display:none">Сохранить</button>
      </div></div>`;

    document.body.appendChild(overlay);
    const close = () => { overlay.remove(); document.removeEventListener('keydown', escH); };
    const escH = (e) => { if (e.key === 'Escape') close(); };
    document.addEventListener('keydown', escH);
    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
    overlay.querySelector('#r-cancel').onclick = close;

    const rowsEl = overlay.querySelector('#r-ingr-rows');
    ingrApi.renderIngredientRows(rowsEl, catalog, blacklist, backend, r.ingredient_items);
    if (stepsApi) stepsApi.renderStepRows(overlay.querySelector('#r-steps'),
      () => ingrApi.collectIngredientItems(rowsEl), r.instructions);
    overlay.querySelectorAll('.rf-acc-header').forEach(hdr => hdr.onclick = () => {
      const body = hdr.nextElementSibling, open = body.style.display !== 'none';
      body.style.display = open ? 'none' : '';
      hdr.querySelector('.rf-acc-arrow').textContent = open ? '▸' : '▾';
    });
    bindMultiChips(overlay, 'tags', state);
    bindChips(overlay, 'diff', state);
    bindCuisineInput(overlay, state, cuisines, backend);
    if (imgApi) imgApi.attach(overlay, state);
    let step = 1;
    const nameOk = () => {
      const nm = overlay.querySelector('#r-name');
      if (nm.value.trim()) return true;
      nm.classList.add('input-error'); showStep(1); nm.focus(); return false;
    };
    function showStep(n) {
      step = n;
      overlay.querySelectorAll('.rw-pane').forEach(p => { p.style.display = p.dataset.pane === String(n) ? '' : 'none'; });
      overlay.querySelectorAll('.rw-step').forEach(s => s.classList.toggle('active', s.dataset.step === String(n)));
      overlay.querySelector('#r-back').style.display = n > 1 ? '' : 'none';
      overlay.querySelector('#r-next').style.display = n < 3 ? '' : 'none';
      // Edit-mode: save always visible (user may want to fix one field without
      // walking through the wizard). Create-mode keeps the wizard flow.
      overlay.querySelector('#r-save').style.display = (recipe || n === 3) ? '' : 'none';
    }
    overlay.querySelector('#r-back').onclick = () => showStep(Math.max(1, step - 1));
    overlay.querySelector('#r-next').onclick = () => { if (step > 1 || nameOk()) showStep(Math.min(3, step + 1)); };
    overlay.querySelectorAll('.rw-step').forEach(s => s.onclick = () => {
      const t = parseInt(s.dataset.step);
      if (t === 1 || nameOk()) showStep(t);
    });
    // Edit-mode: collapse the 3-step wizard into one long scrollable form.
    // Users want to fix any single field (name, prep_time, an ingredient)
    // without walking through validation gates between panes. The wizard
    // flow stays for new recipes where step-by-step guidance helps.
    if (recipe) {
      overlay.querySelector('.rw-nav').style.display = 'none';
      overlay.querySelectorAll('.rw-pane').forEach(p => { p.style.display = ''; });
      overlay.querySelector('#r-back').style.display = 'none';
      overlay.querySelector('#r-next').style.display = 'none';
      overlay.querySelector('#r-save').style.display = '';
    }

    overlay.querySelector('#r-save').onclick = async () => {
      if (!nameOk()) return;
      const name = overlay.querySelector('#r-name').value.trim();
      const ingredient_items = ingrApi.collectIngredientItems(rowsEl);
      const instructions = stepsApi
        ? JSON.stringify(stepsApi.collectSteps(overlay.querySelector('#r-steps')))
        : (overlay.querySelector('#r-instr')?.value?.trim() || '');
      const v = id => parseInt(overlay.querySelector(`#${id}`)?.value) || 0;
      const payload = {
        name, description: '', instructions, ingredients: ingredient_items.flatMap(i => [i.name, ...String(i.alternatives || '').split(',').map(s => s.trim()).filter(Boolean)]).join(', '),
        prep_time: v('r-prep'), cook_time: v('r-cook'), servings: v('r-serv') || 1, calories: v('r-cal'),
        tags: [...state.tags].join(','), difficulty: state.diff, cuisine: state.cuisine,
        health_score: v('r-health') || 5, price_score: v('r-price') || 5,
        protein: v('r-protein'), fat: v('r-fat'), carbs: v('r-carbs'), image: state.image, ingredient_items,
      };
      try {
        if (recipe) await backend.updateRecipe(recipe.id, payload);
        else await backend.createRecipe(payload);
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
      dd.innerHTML = matches.map(c =>
        `<div class="ingr-autocomplete-item" data-id="${esc(c.code)}">${c.emoji} ${esc(c.name)}</div>`).join('')
        + `<div class="ingr-autocomplete-item ingr-autocomplete-create" data-id="__new__">+ Новая кухня</div>`;
      dd.style.display = '';
      dd.querySelectorAll('.ingr-autocomplete-item').forEach(opt => {
        opt.onmousedown = (e) => {
          e.preventDefault(); dd.style.display = 'none';
          if (opt.dataset.id === '__new__') return showNewCuisineForm();
          const c = cuisines.find(x => x.code === opt.dataset.id);
          if (c) { state.cuisine = c.code; inp.value = `${c.emoji} ${c.name}`; }
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

  window.HanniRecipe = window.HanniRecipe || {};
  window.HanniRecipe.showAddRecipeModal = showAddRecipeModal;
})();
