// ── food-recipe-add.js — Add recipe modal ──
import { invoke } from './state.js';
import { loadCuisines, invalidateCuisineCache } from './food-recipe-filters.js';
import { renderIngredientRows, collectIngredientItems } from './food-recipe-ingredients.js';
import { renderStepsRows, collectSteps } from './food-recipe-steps.js';

function acc(title, field, content, open) {
  return `<div class="rf-acc" data-field="${field}">
    <div class="rf-acc-header">${title}<span class="rf-acc-arrow">${open ? '▾' : '▸'}</span></div>
    <div class="rf-acc-body" style="display:${open ? '' : 'none'}">${content}</div></div>`;
}

export async function showAddRecipeModal(reloadFn) {
  const [catalog, cuisines] = await Promise.all([
    invoke('get_ingredient_catalog').catch(() => []), loadCuisines(),
  ]);
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
      <div style="position:relative;"><input class="form-input" id="r-cuisine-input" placeholder="Поиск кухни..." value="${defCuisine ? `${defCuisine.emoji} ${defCuisine.name}` : ''}" autocomplete="off">
        <div class="ingr-autocomplete" id="r-cuisine-dd" style="display:none;width:100%;"></div>
      </div><div id="new-cuisine-form" style="display:none;margin-top:6px;"></div></div>
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
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#r-cancel').onclick = () => overlay.remove();
  renderIngredientRows(overlay.querySelector('#r-ingr-rows'), catalog);
  renderStepsRows(overlay.querySelector('#r-steps'), () => collectIngredientItems(overlay.querySelector('#r-ingr-rows')).map(i => i.name));
  overlay.querySelectorAll('.rf-acc-header').forEach(hdr => hdr.onclick = () => {
    const body = hdr.nextElementSibling, open = body.style.display !== 'none';
    body.style.display = open ? 'none' : '';
    hdr.querySelector('.rf-acc-arrow').textContent = open ? '▸' : '▾';
  });
  bindMultiChips(overlay, 'tags', state);
  bindChips(overlay, 'diff', state);
  bindCuisineInput(overlay, state, cuisines);

  overlay.querySelector('#r-save').addEventListener('click', async () => {
    const nameEl = overlay.querySelector('#r-name');
    const name = nameEl?.value?.trim();
    if (!name) { nameEl.classList.add('input-error'); nameEl.focus(); return; }
    const ingredientItems = collectIngredientItems(overlay.querySelector('#r-ingr-rows'));
    const ingrFlat = ingredientItems.map(i => `${i.name}: ${i.amount}${i.unit}`).join(', ');
    const v = id => parseInt(overlay.querySelector(`#${id}`)?.value) || 0;
    try {
      await invoke('create_recipe', {
        name, description: '', ingredients: ingrFlat,
        instructions: collectSteps(overlay.querySelector('#r-steps')),
        prepTime: v('r-prep'), cookTime: v('r-cook'), servings: v('r-serv') || 1, calories: v('r-cal'),
        tags: [...state.tags].join(','), difficulty: state.diff, cuisine: state.cuisine,
        healthScore: v('r-health') || 5, priceScore: v('r-price') || 5,
        protein: v('r-protein'), fat: v('r-fat'), carbs: v('r-carbs'), ingredientItems,
      });
      overlay.remove(); if (reloadFn) await reloadFn();
    } catch (e) { alert('Ошибка: ' + e); }
  });
}

function chip(id, label, active) {
  return `<button type="button" class="rf-chip${active ? ' active' : ''}" data-val="${id}">${label}</button>`;
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
    dd.innerHTML = matches.map(c => `<div class="ingr-autocomplete-item" data-id="${c.id}">${c.emoji} ${c.name}</div>`).join('')
      + `<div class="ingr-autocomplete-item ingr-autocomplete-create" data-id="__new__">+ Новая кухня</div>`;
    dd.style.display = '';
    dd.querySelectorAll('.ingr-autocomplete-item').forEach(opt => { opt.onmousedown = (e) => {
      e.preventDefault(); dd.style.display = 'none';
      if (opt.dataset.id === '__new__') return showNewCuisineForm();
      const c = cuisines.find(x => x.id === opt.dataset.id);
      if (c) { state.cuisine = c.id; inp.value = `${c.emoji} ${c.name}`; }
    }; });
  }
  const show = () => showDD(inp.value.trim());
  inp.addEventListener('focus', show); inp.addEventListener('input', show);
  inp.addEventListener('blur', () => setTimeout(() => { dd.style.display = 'none'; }, 150));
  function showNewCuisineForm() {
    form.style.display = '';
    form.innerHTML = `<div style="display:flex;gap:6px;align-items:center;"><input class="form-input" id="nc-name" placeholder="Название" style="flex:1;"><input class="form-input" id="nc-emoji" placeholder="🌍" style="width:48px;text-align:center;"><button class="btn-primary" id="nc-save" style="padding:4px 10px;font-size:12px;">OK</button><button class="btn-secondary" id="nc-cancel" style="padding:4px 8px;font-size:12px;">✕</button></div>`;
    form.querySelector('#nc-cancel').onclick = () => { form.style.display = 'none'; };
    form.querySelector('#nc-save').onclick = async () => {
      const n = form.querySelector('#nc-name')?.value?.trim(); if (!n) return;
      const em = form.querySelector('#nc-emoji')?.value?.trim() || '🌍', code = n.toLowerCase().replace(/\s+/g, '_').slice(0, 20);
      try { await invoke('add_cuisine', { code, name: n, emoji: em }); invalidateCuisineCache();
        cuisines.push({ id: code, name: n, emoji: em }); state.cuisine = code; form.style.display = 'none'; inp.value = `${em} ${n}`;
      } catch (e) { alert('Ошибка: ' + e); }
    };
  }
}
