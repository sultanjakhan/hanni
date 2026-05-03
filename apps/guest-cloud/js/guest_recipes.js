// guest_recipes.js — recipes view: list (Hanni recipe-card 1:1) + detail + filters.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can, rememberAuthor, recallAuthor } = u;

  const MEAL_LABELS = { breakfast: 'Завтрак', lunch: 'Обед', dinner: 'Ужин', universal: 'Универсал' };
  const MEAL_COLORS = { breakfast: 'green', lunch: 'yellow', dinner: 'red', universal: 'blue' };
  const DIFF_LABELS = { easy: 'Лёгкий', medium: 'Средний', hard: 'Сложный' };
  const CUISINES = { kz: '🇰🇿 Казахская', ru: '🇷🇺 Русская', it: '🇮🇹 Итальянская', jp: '🇯🇵 Японская', cn: '🇨🇳 Китайская', other: '🌍 Другое' };

  const state = {
    mount: null, view: 'list', recipes: [], catalog: {},
    current: null, comments: [],
    filters: { search: '', meal: 'all', diff: 'all' },
    showFilters: false,
  };

  function catCat(name) {
    const k = (name || '').toLowerCase().trim();
    return state.catalog[k] || null;
  }
  function ingrNames(r) {
    return (r.ingredients || '').split(',').map(s => {
      const i = s.indexOf(':'); return (i > -1 ? s.slice(0, i) : s).trim();
    }).filter(Boolean);
  }
  function tagList(r) {
    return (r.tags || '').split(/[,\s]+/).map(t => t.trim()).filter(t => MEAL_LABELS[t]);
  }

  // Stage C-1: Firestore is the source of truth for READ when the host
  // mirror is configured, so the guest stays alive even when Hanni is closed.
  // axum is used only as fallback (cloud not configured) and for WRITE.
  const fs = (window.HanniGuest || {}).firestore;

  async function fetchListAndCatalog() {
    if (fs) {
      try {
        const [recipes, catalog] = await Promise.all([
          fs.list('recipes'),
          fs.list('ingredient_catalog'),
        ]);
        return {
          recipes: (recipes || []).sort((a, b) =>
            String(b.updated_at || '').localeCompare(String(a.updated_at || ''))).slice(0, 200),
          catalog: (catalog || []).map(c => ({ name: c.name || '', category: c.category || 'other' })),
        };
      } catch (e) {
        console.warn('[guest_recipes] firestore failed, falling back to axum:', e?.message || e);
      }
    }
    return await api('/recipes');
  }

  async function fetchRecipe(id) {
    if (fs) {
      try {
        const recipe = state.recipes.find(r => Number(r.id) === Number(id))
                       || await fs.get('recipes', id);
        if (!recipe) throw new Error('Recipe not found');
        const allIngr = await fs.list('recipe_ingredients');
        recipe.ingredient_items = (allIngr || [])
          .filter(i => Number(i.recipe_id) === Number(id));
        return recipe;
      } catch (e) {
        console.warn('[guest_recipes] firestore detail failed, falling back to axum:', e?.message || e);
      }
    }
    return await api(`/recipes/${id}`);
  }

  async function load() {
    if (!can('view') && !can('add')) {
      state.mount.innerHTML = '<div class="empty">Ссылка активна, но без прав просмотра рецептов.</div>';
      return;
    }
    try {
      if (can('view')) {
        const data = await fetchListAndCatalog();
        state.recipes = data.recipes || [];
        state.catalog = {};
        (data.catalog || []).forEach(c => { state.catalog[(c.name || '').toLowerCase()] = c.category; });
      }
      state.view = 'list';
      render();
    } catch (e) {
      state.mount.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function applyFilters(list) {
    const f = state.filters, q = f.search.toLowerCase().trim();
    return list.filter(r => {
      if (q && !r.name.toLowerCase().includes(q)) return false;
      if (f.meal !== 'all' && !tagList(r).includes(f.meal)) return false;
      if (f.diff !== 'all' && (r.difficulty || 'easy') !== f.diff) return false;
      return true;
    });
  }

  function chipsRow(name, opts, current) {
    return Object.entries(opts).map(([v, l]) =>
      `<button class="rf-chip ${current === v ? 'active' : ''}" data-fname="${name}" data-fval="${v}">${esc(l)}</button>`
    ).join('');
  }

  function filterPanelHtml() {
    const meal = { all: 'Все', ...MEAL_LABELS };
    const diff = { all: 'Все', easy: 'Лёгкий', medium: 'Средний', hard: 'Сложный' };
    return `<div class="rf-panel">
      <div class="rf-section"><span class="rf-title">Приём</span>${chipsRow('meal', meal, state.filters.meal)}</div>
      <div class="rf-section"><span class="rf-title">Сложность</span>${chipsRow('diff', diff, state.filters.diff)}</div>
    </div>`;
  }

  function cardHtml(r) {
    const total = (r.prep_time || 0) + (r.cook_time || 0);
    const diff = r.difficulty || 'easy';
    const tags = tagList(r);
    const badges = tags.map(t => `<span class="badge badge-${MEAL_COLORS[t] || 'gray'}">${esc(MEAL_LABELS[t])}</span>`).join('');
    const names = ingrNames(r);
    const tagsHtml = names.slice(0, 5).map(n => {
      const cat = catCat(n);
      return `<span class="ingr-tag${cat ? ' ingr-cat-' + cat : ''}">${esc(n)}</span>`;
    }).join('') + (names.length > 5 ? `<span class="ingr-tag ingr-more">+${names.length - 5}</span>` : '');
    return `<div class="recipe-card${r.favorite ? ' recipe-fav' : ''}" data-id="${r.id}">
      <div class="recipe-card-header">
        <span class="recipe-card-name">${r.favorite ? '★ ' : ''}${esc(r.name)}</span>
        <span class="recipe-card-cal">${r.calories || '—'} kcal</span>
      </div>
      <div class="recipe-card-meta">
        ${total ? `<span>⏱ ${total} мин</span>` : ''}
        <span>👥 ${r.servings || 1}</span>
        <span class="recipe-diff recipe-diff-${diff}">${esc(DIFF_LABELS[diff] || diff)}</span>
        <span>❤${r.health_score || 5}</span><span>💰${r.price_score || 5}</span>
      </div>
      ${badges ? `<div class="recipe-card-tags">${badges}</div>` : ''}
      ${tagsHtml ? `<div class="recipe-card-ingr">${tagsHtml}</div>` : ''}
    </div>`;
  }

  function listHtml() {
    const filtered = applyFilters(state.recipes);
    const grid = filtered.length
      ? `<div class="recipe-grid">${filtered.map(cardHtml).join('')}</div>`
      : '<div class="empty">Ничего не найдено.</div>';
    const filterCount = (state.filters.meal !== 'all' ? 1 : 0) + (state.filters.diff !== 'all' ? 1 : 0);
    const addBtn = can('add') ? `<button class="btn-primary" id="rf-add">+ Рецепт</button>` : '';
    return `<div class="recipe-pane">
      <div class="recipe-filter-bar">
        <button class="rf-toggle ${state.showFilters || filterCount ? 'rf-active' : ''}" id="rf-tog">⚙ Фильтры${filterCount ? `<span class="rf-badge">${filterCount}</span>` : ''}</button>
        <input class="recipe-search" id="rf-search" placeholder="Поиск... (нажмите /)" value="${esc(state.filters.search)}">
        ${addBtn}
      </div>
      ${state.showFilters ? filterPanelHtml() : ''}
      <h2>Рецепты (${filtered.length})</h2>
      ${grid}
    </div>`;
  }

  function bindList() {
    state.mount.querySelectorAll('.recipe-card').forEach(c =>
      c.addEventListener('click', () => openDetail(parseInt(c.dataset.id))));
    state.mount.querySelector('#rf-tog')?.addEventListener('click', () => { state.showFilters = !state.showFilters; render(); });
    const search = state.mount.querySelector('#rf-search');
    if (search) search.addEventListener('input', (e) => { state.filters.search = e.target.value; render(); search.focus(); search.setSelectionRange(search.value.length, search.value.length); });
    state.mount.querySelectorAll('.rf-chip').forEach(c => c.addEventListener('click', () => {
      state.filters[c.dataset.fname] = c.dataset.fval; render();
    }));
    state.mount.querySelector('#rf-add')?.addEventListener('click', openAddModal);
  }

  function openAddModal() {
    const mod = (window.HanniGuest || {}).recipeAdd;
    if (!mod) { alert('Модуль добавления не загружен'); return; }
    // Pass catalog as array of { name, category } for autocomplete + "+ Создать" flow.
    const catalog = Object.entries(state.catalog).map(([name, category]) => ({ name, category, tags: '' }));
    mod.showAddRecipeModal(catalog, load);
  }

  async function openDetail(id) {
    try {
      state.current = await fetchRecipe(id);
      // Comments still flow through axum (no Firestore mirror in C-1) — soft-fail
      // so the recipe still opens when Hanni is offline.
      state.comments = (await api(`/recipes/${id}/comments`).catch(() => ({ comments: [] }))).comments || [];
      state.view = 'detail';
      render();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  }

  function detailHtml() {
    const r = state.current;
    const total = (r.prep_time || 0) + (r.cook_time || 0);
    const diff = r.difficulty || 'easy';
    const items = (r.ingredient_items || []).filter(i => i.name);
    const ingrHtml = items.length
      ? items.map(i => {
          const cat = catCat(i.name);
          return `<span class="ingr-item"><span class="ingr-tag${cat ? ' ingr-cat-' + cat : ''}">${esc(i.name)}</span><span class="ingr-amt">${i.amount}${esc(i.unit || '')}</span></span>`;
        }).join('')
      : ingrNames(r).map(n => {
          const cat = catCat(n);
          return `<span class="ingr-tag${cat ? ' ingr-cat-' + cat : ''}">${esc(n)}</span>`;
        }).join(' ');
    const steps = (r.instructions || '').split(/\n+/).map(s => s.trim()).filter(Boolean);
    const stepsHtml = steps.length ? `<ol class="recipe-instructions">${steps.map(s => `<li>${esc(s)}</li>`).join('')}</ol>` : '<div class="muted">Нет инструкций.</div>';
    const cuisine = CUISINES[r.cuisine] || `🌍 ${r.cuisine || ''}`;

    const editBtn = can('edit') ? '<button class="btn-secondary" id="r-edit" style="font-size:13px;padding:4px 10px">Изменить</button>' : '';
    const delBtn = can('delete') ? '<button class="btn-danger" id="r-del" style="font-size:13px;padding:4px 10px">Удалить</button>' : '';
    const actions = (editBtn || delBtn)
      ? `<div class="detail-actions" style="display:flex;gap:6px;justify-content:flex-end;margin-bottom:8px">${editBtn}${delBtn}</div>`
      : '';
    return `<button class="detail-back" id="back">← Назад к списку</button>
      ${actions}
      <div class="detail-title-row">
        <div class="detail-title">${r.favorite ? '★ ' : ''}${esc(r.name)}</div>
      </div>
      ${r.description ? `<p style="color:var(--text-secondary);font-size:14px;margin-bottom:12px">${esc(r.description)}</p>` : ''}
      <div class="recipe-detail-meta">
        ${total ? `<span class="badge badge-blue">⏱ ${total} мин</span>` : ''}
        <span class="badge badge-gray">👥 ${r.servings || 1} порц.</span>
        <span class="badge badge-green">${r.calories || 0} kcal</span>
        <span class="badge badge-gray">Б${r.protein} Ж${r.fat} У${r.carbs}</span>
        <span class="badge badge-purple">${esc(DIFF_LABELS[diff] || diff)}</span>
        <span class="badge badge-gray">${esc(cuisine)}</span>
        <span class="badge badge-green">❤ ${r.health_score || 5}/10</span>
        <span class="badge badge-yellow">💰 ${r.price_score || 5}/10</span>
      </div>
      <div class="recipe-detail-section"><h4>Ингредиенты</h4><div class="recipe-ingr-tags" style="flex-direction:row;flex-wrap:wrap;gap:6px">${ingrHtml || '<div class="muted">—</div>'}</div></div>
      <div class="recipe-detail-section"><h4>Приготовление</h4>${stepsHtml}</div>
      <div class="recipe-detail-section"><h4>Заметки гостей</h4>
        ${state.comments.length ? state.comments.map(c => `<div style="padding:8px 0;border-bottom:1px solid var(--border-subtle);font-size:13px"><b>${esc(c.author)}</b> <span class="muted">· ${esc((c.created_at || '').slice(0,16))}</span><div style="margin-top:2px">${esc(c.text)}</div></div>`).join('') : '<div class="muted">Пока нет.</div>'}
      </div>
      ${can('comment') ? `<div class="form-group" style="margin-top:12px"><label class="form-label">Ваше имя</label><input class="form-input" id="c-author" value="${esc(recallAuthor())}"></div>
        <div class="form-group"><label class="form-label">Заметка</label><textarea class="form-textarea" id="c-text" rows="2"></textarea></div>
        <div id="c-msg"></div>
        <div style="display:flex;justify-content:flex-end"><button class="btn-primary" id="c-save">Отправить</button></div>` : ''}`;
  }

  function bindDetail() {
    state.mount.querySelector('#back')?.addEventListener('click', () => { state.view = 'list'; state.current = null; render(); });
    state.mount.querySelector('#c-save')?.addEventListener('click', submitComment);
    state.mount.querySelector('#r-edit')?.addEventListener('click', openEditModal);
    state.mount.querySelector('#r-del')?.addEventListener('click', confirmDelete);
  }

  async function confirmDelete() {
    const r = state.current;
    if (!r || !confirm(`Удалить рецепт «${r.name}»? Это действие необратимо.`)) return;
    try {
      await api(`/recipes/${r.id}`, { method: 'DELETE' });
      state.recipes = state.recipes.filter(x => x.id !== r.id);
      state.current = null;
      state.view = 'list';
      render();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  }

  function openEditModal() {
    const r = state.current;
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal" style="max-width:480px;max-height:90vh;overflow-y:auto">
      <div class="modal-title">Изменить рецепт</div>
      <div class="form-group"><label class="form-label">Название</label>
        <input class="form-input" id="re-name" value="${esc(r.name)}"></div>
      <div class="form-group"><label class="form-label">Описание</label>
        <textarea class="form-textarea" id="re-desc" rows="2">${esc(r.description || '')}</textarea></div>
      <div class="form-group"><label class="form-label">Приготовление</label>
        <textarea class="form-textarea" id="re-instr" rows="4">${esc(r.instructions || '')}</textarea></div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">
        <div class="form-group"><label class="form-label">Подготовка (мин)</label>
          <input class="form-input" id="re-prep" type="number" value="${r.prep_time || 0}"></div>
        <div class="form-group"><label class="form-label">Готовка (мин)</label>
          <input class="form-input" id="re-cook" type="number" value="${r.cook_time || 0}"></div>
        <div class="form-group"><label class="form-label">Порций</label>
          <input class="form-input" id="re-serv" type="number" value="${r.servings || 1}"></div>
        <div class="form-group"><label class="form-label">Калории</label>
          <input class="form-input" id="re-kcal" type="number" value="${r.calories || 0}"></div>
      </div>
      <div id="re-msg"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="re-cancel">Отмена</button>
        <button class="btn-primary" id="re-save">Сохранить</button>
      </div></div>`;
    document.body.appendChild(overlay);
    const close = () => overlay.remove();
    overlay.addEventListener('click', e => { if (e.target === overlay) close(); });
    overlay.querySelector('#re-cancel').onclick = close;
    overlay.querySelector('#re-save').onclick = async () => {
      const msg = overlay.querySelector('#re-msg');
      const payload = {
        name: overlay.querySelector('#re-name').value.trim(),
        description: overlay.querySelector('#re-desc').value,
        instructions: overlay.querySelector('#re-instr').value,
        prep_time: parseInt(overlay.querySelector('#re-prep').value) || 0,
        cook_time: parseInt(overlay.querySelector('#re-cook').value) || 0,
        servings: parseInt(overlay.querySelector('#re-serv').value) || 1,
        calories: parseInt(overlay.querySelector('#re-kcal').value) || 0,
        author: recallAuthor() || 'guest',
      };
      if (!payload.name) { msg.innerHTML = '<div class="err">Название обязательно</div>'; return; }
      msg.innerHTML = '<div class="muted">Сохраняем…</div>';
      try {
        await api(`/recipes/${r.id}`, { method: 'PATCH',
          headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
        close();
        await openDetail(r.id);
      } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
    };
  }

  function render() {
    if (state.view === 'detail' && state.current) { state.mount.innerHTML = detailHtml(); bindDetail(); }
    else { state.mount.innerHTML = listHtml(); bindList(); }
  }

  async function submitComment() {
    const m = state.mount, msg = m.querySelector('#c-msg');
    const text = m.querySelector('#c-text').value.trim();
    if (!text) { msg.innerHTML = '<div class="err">Текст обязателен</div>'; return; }
    const author = m.querySelector('#c-author').value.trim() || 'Guest';
    rememberAuthor(author);
    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      await api(`/recipes/${state.current.id}/comments`, { method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text, author }) });
      msg.innerHTML = '<div class="ok">Заметка добавлена</div>';
      setTimeout(() => openDetail(state.current.id), 500);
    } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.recipes = { mount(el) { state.mount = el; load(); } };
})();
