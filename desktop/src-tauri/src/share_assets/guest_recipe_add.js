// guest_recipe_add.js — Add recipe modal (copy of food-recipe-add.js).
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc } = u;

  function acc(title, field, content, open) {
    return `<div class="rf-acc" data-field="${field}">
      <div class="rf-acc-header">${title}<span class="rf-acc-arrow">${open ? '▾' : '▸'}</span></div>
      <div class="rf-acc-body" style="display:${open ? '' : 'none'}">${content}</div></div>`;
  }
  function chip(id, label, active) {
    return `<button type="button" class="rf-chip${active ? ' active' : ''}" data-val="${id}">${esc(label)}</button>`;
  }

  async function showAddRecipeModal(catalog, reloadFn) {
    const cuisines = (await api('/cuisines').catch(() => ({ cuisines: [] }))).cuisines || [];
    const state = { tags: new Set(['universal']), diff: 'easy', cuisine: 'kz' };
    const mealsHtml = ['breakfast:Завтрак', 'lunch:Обед', 'dinner:Ужин', 'universal:Универсал']
      .map(s => { const [id, l] = s.split(':'); return chip(id, l, state.tags.has(id)); }).join('');
    const diffsHtml = ['easy:Лёгкий', 'medium:Средний', 'hard:Сложный']
      .map(s => { const [id, l] = s.split(':'); return chip(id, l, state.diff === id); }).join('');
    const defCuisine = cuisines.find(c => c.id === state.cuisine);

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
          <input class="form-input" id="r-cuisine-input" placeholder="Поиск кухни..." value="${defCuisine ? `${defCuisine.emoji} ${esc(defCuisine.name)}` : ''}" autocomplete="off">
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
      <div class="form-group"><label class="form-label">Ваше имя (опц.)</label>
        <input class="form-input" id="r-author" value="${esc(u.recallAuthor())}">
      </div>
      <div id="r-msg"></div>
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

    const ing = window.HanniGuest.recipeIngredients;
    const stp = window.HanniGuest.recipeSteps;
    ing.renderIngredientRows(overlay.querySelector('#r-ingr-rows'), catalog);
    stp.renderStepsRows(overlay.querySelector('#r-steps'),
      () => ing.collectIngredientItems(overlay.querySelector('#r-ingr-rows')).map(i => i.name));

    overlay.querySelectorAll('.rf-acc-header').forEach(hdr => hdr.onclick = () => {
      const body = hdr.nextElementSibling, open = body.style.display !== 'none';
      body.style.display = open ? 'none' : '';
      hdr.querySelector('.rf-acc-arrow').textContent = open ? '▸' : '▾';
    });
    bindMultiChips(overlay, 'tags', state);
    bindChips(overlay, 'diff', state);
    bindCuisineInput(overlay, state, cuisines);

    overlay.querySelector('#r-save').onclick = async () => {
      const nameEl = overlay.querySelector('#r-name');
      const name = nameEl?.value?.trim();
      const msg = overlay.querySelector('#r-msg');
      if (!name) { nameEl.classList.add('input-error'); nameEl.focus(); return; }
      const ingredient_items = ing.collectIngredientItems(overlay.querySelector('#r-ingr-rows'));
      const v = id => parseInt(overlay.querySelector(`#${id}`)?.value) || 0;
      const author = overlay.querySelector('#r-author').value.trim() || 'guest';
      u.rememberAuthor(author);
      msg.innerHTML = '<div class="muted">Сохраняем…</div>';
      try {
        await api('/recipes', {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            name, description: '',
            instructions: stp.collectSteps(overlay.querySelector('#r-steps')),
            prep_time: v('r-prep'), cook_time: v('r-cook'), servings: v('r-serv') || 1, calories: v('r-cal'),
            tags: [...state.tags].join(','), difficulty: state.diff, cuisine: state.cuisine,
            health_score: v('r-health') || 5, price_score: v('r-price') || 5,
            protein: v('r-protein'), fat: v('r-fat'), carbs: v('r-carbs'),
            ingredient_items, author,
          }),
        });
        close(); if (reloadFn) await reloadFn();
      } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
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
  function bindCuisineInput(overlay, state, cuisines) {
    const inp = overlay.querySelector('#r-cuisine-input');
    const dd = overlay.querySelector('#r-cuisine-dd');
    const form = overlay.querySelector('#new-cuisine-form');
    function showDD(q) {
      const lc = q.toLowerCase();
      const matches = lc ? cuisines.filter(c => c.name.toLowerCase().includes(lc)) : cuisines;
      dd.innerHTML = matches.map(c => `<div class="ingr-autocomplete-item" data-id="${esc(c.id)}">${c.emoji} ${esc(c.name)}</div>`).join('')
        + `<div class="ingr-autocomplete-item ingr-autocomplete-create" data-id="__new__">+ Новая кухня</div>`;
      dd.style.display = '';
      dd.querySelectorAll('.ingr-autocomplete-item').forEach(opt => {
        opt.onmousedown = (e) => {
          e.preventDefault(); dd.style.display = 'none';
          if (opt.dataset.id === '__new__') return showNewCuisineForm();
          const c = cuisines.find(x => x.id === opt.dataset.id);
          if (c) { state.cuisine = c.id; inp.value = `${c.emoji} ${c.name}`; }
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
          await api('/cuisines', { method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code, name: n, emoji: em }) });
          cuisines.push({ id: code, code, name: n, emoji: em, is_default: 0 });
          state.cuisine = code; form.style.display = 'none'; inp.value = `${em} ${n}`;
        } catch (e) { alert('Ошибка: ' + (e.message || e)); }
      };
    }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.recipeAdd = { showAddRecipeModal };
})();
