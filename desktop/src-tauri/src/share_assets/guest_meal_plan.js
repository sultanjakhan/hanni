// guest_meal_plan.js — meal plan view: per-day list with add/delete.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can, rememberAuthor, recallAuthor } = u;

  const MEAL_LABELS = {
    breakfast: '🌅 Завтрак', lunch: '☀️ Обед',
    dinner: '🌙 Ужин', snack: '🍎 Перекус',
  };

  const state = { mount: null, date: todayISO(), meals: [], recipes: [] };

  function todayISO() {
    const d = new Date();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${m}-${day}`;
  }

  async function load() {
    if (!can('view') && !can('add')) {
      state.mount.innerHTML = '<div class="empty">Ссылка активна, но без прав просмотра плана.</div>';
      return;
    }
    try {
      const data = await api(`/meal-plan?date=${encodeURIComponent(state.date)}`);
      state.meals = data.meals || [];
      state.recipes = data.recipes_index || [];
      render();
    } catch (e) {
      state.mount.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function render() {
    const list = state.meals.length
      ? state.meals.map(m => {
          const label = MEAL_LABELS[m.meal_type] || m.meal_type;
          const delBtn = can('delete') ? `<button class="link-btn link-btn-danger" data-del="${m.id}">×</button>` : '';
          return `<div class="card meal-card meal-${esc(m.meal_type)}">
            <div class="prod-top">
              <div><span class="meal-label">${esc(label)}</span> · <b>${esc(m.recipe_name)}</b></div>
              <div class="prod-actions">${delBtn}</div>
            </div>
            <div class="card-meta">
              ${m.calories ? `<span>🔥 ${m.calories} ккал</span>` : ''}
              ${m.notes ? `<span>${esc(m.notes)}</span>` : ''}
            </div>
          </div>`;
        }).join('')
      : '<div class="empty">На эту дату планов нет.</div>';

    const addBlock = can('add') ? renderAddForm() : '';
    state.mount.innerHTML = `
      <h2 style="margin:0 0 14px">План питания</h2>
      <label style="display:block;margin-bottom:12px">Дата
        <input id="mp-date" type="date" value="${esc(state.date)}">
      </label>
      ${list}${addBlock}`;

    state.mount.querySelector('#mp-date').addEventListener('change', e => {
      state.date = e.target.value || todayISO();
      load();
    });
    state.mount.querySelectorAll('[data-del]').forEach(b =>
      b.addEventListener('click', () => removeMeal(parseInt(b.dataset.del))));
    const save = state.mount.querySelector('#mp-save');
    if (save) save.addEventListener('click', submitNew);
  }

  function mealOptions() {
    return Object.entries(MEAL_LABELS).map(([v, l]) =>
      `<option value="${v}">${esc(l)}</option>`).join('');
  }
  function recipeOptions() {
    if (!state.recipes.length) return '<option value="">— нет рецептов —</option>';
    return state.recipes.map(r =>
      `<option value="${r.id}">${esc(r.name)}</option>`).join('');
  }

  function renderAddForm() {
    return `<div class="card" style="margin-top:20px">
      <div class="card-title">Добавить приём пищи</div>
      <label>Тип</label><select id="mp-type">${mealOptions()}</select>
      <label>Рецепт</label><select id="mp-recipe">${recipeOptions()}</select>
      <label>Заметка</label><input id="mp-notes">
      <label>Ваше имя (опционально)</label><input id="mp-author" value="${esc(recallAuthor())}">
      <div id="mp-msg"></div>
      <div class="row-actions"><button class="btn" id="mp-save">Добавить</button></div>
    </div>`;
  }

  async function submitNew() {
    const m = state.mount;
    const msg = m.querySelector('#mp-msg');
    const recipe_id = parseInt(m.querySelector('#mp-recipe').value);
    if (!recipe_id) { msg.innerHTML = '<div class="err">Выберите рецепт</div>'; return; }
    const author = m.querySelector('#mp-author').value.trim() || 'guest';
    rememberAuthor(author);
    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      await api('/meal-plan', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          date: state.date,
          meal_type: m.querySelector('#mp-type').value,
          recipe_id,
          notes: m.querySelector('#mp-notes').value.trim(),
          author,
        }),
      });
      msg.innerHTML = '<div class="ok">Добавлено</div>';
      setTimeout(load, 400);
    } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
  }

  async function removeMeal(id) {
    const item = state.meals.find(x => x.id === id);
    if (!confirm(`Удалить "${item ? item.recipe_name : 'приём'}"?`)) return;
    try {
      await api(`/meal-plan/${id}`, { method: 'DELETE' });
      state.meals = state.meals.filter(x => x.id !== id);
      render();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.meal_plan = {
    mount(el) { state.mount = el; load(); },
  };
})();
