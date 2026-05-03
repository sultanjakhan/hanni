// guest_meal_plan.js — meal plan: meal-plan-block 1:1 with food-meal-plan.js.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can, rememberAuthor, recallAuthor } = u;

  const MEAL_LABELS = { breakfast: 'Завтрак', lunch: 'Обед', dinner: 'Ужин', snack: 'Перекус' };
  const MEAL_COLORS = {
    breakfast: 'var(--color-yellow)',
    lunch: 'var(--color-green)',
    dinner: 'var(--color-purple)',
    snack: 'var(--text-muted)',
  };

  const state = { mount: null, date: todayISO(), meals: [], recipes: [], showAdd: false, search: '', pickedType: 'breakfast' };

  function todayISO() {
    const d = new Date();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${m}-${day}`;
  }

  // Stage C-1: pull meal_plan + recipes from Firestore mirror so the plan view
  // works while Hanni is offline. Writes still go through axum.
  const fs = (window.HanniGuest || {}).firestore;

  async function fetchPlan(dateIso) {
    if (fs) {
      try {
        const [plan, recipes] = await Promise.all([
          fs.list('meal_plan'),
          fs.list('recipes'),
        ]);
        const recipeById = new Map();
        (recipes || []).forEach(r => recipeById.set(Number(r.id), r));
        const meals = (plan || [])
          .filter(p => p.date === dateIso)
          .map(p => {
            const r = recipeById.get(Number(p.recipe_id));
            return {
              id: p.id, date: p.date, meal_type: p.meal_type,
              recipe_id: p.recipe_id, notes: p.notes || '',
              recipe_name: r ? r.name : `Рецепт ${p.recipe_id}`,
              calories: r ? (r.calories || 0) : 0,
            };
          });
        const recipes_index = (recipes || [])
          .map(r => ({ id: r.id, name: r.name }))
          .sort((a, b) => String(a.name).localeCompare(String(b.name)));
        return { meals, recipes_index };
      } catch (e) {
        console.warn('[guest_meal_plan] firestore failed, falling back:', e?.message || e);
      }
    }
    return await api(`/meal-plan?date=${encodeURIComponent(dateIso)}`);
  }

  async function load() {
    if (!can('view') && !can('add')) {
      state.mount.innerHTML = '<div class="empty">Ссылка активна, но без прав просмотра плана.</div>';
      return;
    }
    try {
      const data = await fetchPlan(state.date);
      state.meals = data.meals || [];
      state.recipes = data.recipes_index || [];
      render();
    } catch (e) {
      state.mount.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function blockHtml() {
    if (!state.meals.length) return '<div class="empty">На эту дату планов нет.</div>';
    const totalCal = state.meals.reduce((s, m) => s + (m.calories || 0), 0);
    const items = state.meals.map(m => {
      const label = MEAL_LABELS[m.meal_type] || m.meal_type;
      const color = MEAL_COLORS[m.meal_type] || 'var(--text-secondary)';
      const delBtn = can('delete') ? `<button class="meal-plan-del" data-del-id="${m.id}" title="Убрать">×</button>` : '';
      return `<div class="meal-plan-item" data-meal-id="${m.id}">
        <span class="meal-plan-type" style="color:${color};">${esc(label)}</span>
        <span class="meal-plan-name">${esc(m.recipe_name)}</span>
        <span class="meal-plan-cal">${m.calories || '—'} kcal</span>
        ${delBtn}
      </div>`;
    }).join('');
    return `<div class="meal-plan-block">
      <div class="meal-plan-header">
        <span>🍽 План питания</span>
        <span class="meal-plan-total">${totalCal} kcal</span>
      </div>
      ${items}
    </div>`;
  }

  function render() {
    const addBlock = state.showAdd ? addPanelHtml() : '';
    const addBtn = can('add') && !state.showAdd
      ? `<button class="btn-primary" id="mp-open-add" style="margin-top:8px">+ Приём пищи</button>` : '';
    state.mount.innerHTML = `
      <h2>План питания</h2>
      <div class="form-group" style="max-width:240px">
        <label class="form-label">Дата</label>
        <input class="form-input" id="mp-date" type="date" value="${esc(state.date)}">
      </div>
      ${blockHtml()}
      ${addBtn}
      ${addBlock}`;

    state.mount.querySelector('#mp-date').addEventListener('change', e => {
      state.date = e.target.value || todayISO(); load();
    });
    state.mount.querySelectorAll('.meal-plan-del').forEach(b =>
      b.addEventListener('click', () => removeMeal(parseInt(b.dataset.delId))));
    state.mount.querySelector('#mp-open-add')?.addEventListener('click', () => { state.showAdd = true; render(); });
    if (state.showAdd) bindAdd();
  }

  function typeChipsHtml() {
    return Object.entries(MEAL_LABELS).map(([v, l]) =>
      `<button class="rf-chip ${v === state.pickedType ? 'active' : ''}" data-mt="${v}">${esc(l)}</button>`
    ).join('');
  }
  function recipeListHtml() {
    if (!state.recipes.length) return '<div class="muted" style="padding:8px">Нет рецептов в каталоге.</div>';
    const q = state.search.toLowerCase().trim();
    const filtered = q ? state.recipes.filter(r => r.name.toLowerCase().includes(q)) : state.recipes;
    if (!filtered.length) return '<div class="muted" style="padding:8px">Не найдено.</div>';
    return filtered.map(r =>
      `<div class="mp-recipe-option" data-rid="${r.id}">
        <span>${esc(r.name)}</span>
        <span class="muted" style="font-size:12px">→</span>
      </div>`).join('');
  }

  function addPanelHtml() {
    return `<div style="margin-top:14px;padding:14px;border:1px solid var(--border-default);border-radius:var(--radius-lg);background:var(--bg-card)">
      <h4 style="margin:0 0 10px">Добавить приём пищи — ${esc(state.date)}</h4>
      <div class="form-group">
        <label class="form-label">Тип</label>
        <div class="recipe-filter-bar" data-chips="type" style="margin:0">${typeChipsHtml()}</div>
      </div>
      <div class="form-group">
        <label class="form-label">Рецепт</label>
        <input class="form-input" id="mp-search" placeholder="Поиск..." value="${esc(state.search)}">
        <div class="mp-recipe-list" id="mp-list" style="margin-top:6px">${recipeListHtml()}</div>
      </div>
      <div id="mp-msg"></div>
      <div style="display:flex;justify-content:flex-end;gap:6px;margin-top:8px">
        <button class="btn-secondary" id="mp-cancel">Отмена</button>
      </div>
    </div>`;
  }

  function bindAdd() {
    const m = state.mount;
    const typeGroup = m.querySelector('[data-chips="type"]');
    typeGroup?.addEventListener('click', (e) => {
      const btn = e.target.closest('.rf-chip'); if (!btn) return;
      state.pickedType = btn.dataset.mt;
      typeGroup.querySelectorAll('.rf-chip').forEach(c => c.classList.toggle('active', c === btn));
    });
    const search = m.querySelector('#mp-search');
    search?.addEventListener('input', (e) => {
      state.search = e.target.value;
      m.querySelector('#mp-list').innerHTML = recipeListHtml();
      bindRows();
      search.focus();
    });
    bindRows();
    m.querySelector('#mp-cancel').addEventListener('click', () => { state.showAdd = false; state.search = ''; render(); });
  }

  function bindRows() {
    state.mount.querySelectorAll('.mp-recipe-option').forEach(row =>
      row.addEventListener('click', () => addMeal(parseInt(row.dataset.rid))));
  }

  async function addMeal(recipe_id) {
    const m = state.mount, msg = m.querySelector('#mp-msg');
    const author = recallAuthor() || 'guest';
    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      await api('/meal-plan', { method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ date: state.date, meal_type: state.pickedType, recipe_id, notes: '', author }) });
      state.showAdd = false; state.search = '';
      load();
    } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
  }

  async function removeMeal(id) {
    const item = state.meals.find(x => x.id === id);
    if (!confirm(`Убрать "${item ? item.recipe_name : 'приём'}"?`)) return;
    try {
      await api(`/meal-plan/${id}`, { method: 'DELETE' });
      state.meals = state.meals.filter(x => x.id !== id);
      render();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.meal_plan = { mount(el) { state.mount = el; load(); } };
})();
