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

  async function load() {
    if (!can('view') && !can('add')) {
      state.mount.innerHTML = '<div class="empty">Ссылка активна, но без прав просмотра рецептов.</div>';
      return;
    }
    try {
      if (can('view')) {
        const data = await api('/recipes');
        state.recipes = data.recipes || [];
        state.catalog = {};
        (data.catalog || []).forEach(c => { state.catalog[c.name.toLowerCase()] = c.category; });
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
    return `<div class="recipe-pane">
      <div class="recipe-filter-bar">
        <button class="rf-toggle ${state.showFilters || filterCount ? 'rf-active' : ''}" id="rf-tog">⚙ Фильтры${filterCount ? `<span class="rf-badge">${filterCount}</span>` : ''}</button>
        <input class="recipe-search" id="rf-search" placeholder="Поиск..." value="${esc(state.filters.search)}">
      </div>
      ${state.showFilters ? filterPanelHtml() : ''}
      <h2>Рецепты (${filtered.length})</h2>
      ${grid}
      ${can('add') ? addFormHtml() : ''}
    </div>`;
  }

  function addFormHtml() {
    return `<div style="margin-top:24px;padding:14px;border:1px solid var(--border-default);border-radius:var(--radius-lg);background:var(--bg-card)">
      <h4 style="margin:0 0 10px">Добавить рецепт</h4>
      <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="f-name"></div>
      <div class="form-group"><label class="form-label">Ингредиенты (через запятую)</label><textarea class="form-textarea" id="f-ingr" rows="2"></textarea></div>
      <div class="form-group"><label class="form-label">Инструкции</label><textarea class="form-textarea" id="f-inst" rows="3"></textarea></div>
      <div class="form-group"><label class="form-label">Ваше имя (опц.)</label><input class="form-input" id="f-author" value="${esc(recallAuthor())}"></div>
      <div id="f-msg"></div>
      <div style="display:flex;justify-content:flex-end;margin-top:8px"><button class="btn-primary" id="f-save">Отправить</button></div>
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
    const save = state.mount.querySelector('#f-save'); if (save) save.addEventListener('click', submitNew);
  }

  async function openDetail(id) {
    try {
      state.current = await api(`/recipes/${id}`);
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

    return `<button class="detail-back" id="back">← Назад к списку</button>
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
  }

  function render() {
    if (state.view === 'detail' && state.current) { state.mount.innerHTML = detailHtml(); bindDetail(); }
    else { state.mount.innerHTML = listHtml(); bindList(); }
  }

  async function submitNew() {
    const m = state.mount, msg = m.querySelector('#f-msg');
    const name = m.querySelector('#f-name').value.trim();
    if (!name) { msg.innerHTML = '<div class="err">Название обязательно</div>'; return; }
    const author = m.querySelector('#f-author').value.trim() || 'guest';
    rememberAuthor(author);
    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      await api('/recipes', { method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, ingredients: m.querySelector('#f-ingr').value, instructions: m.querySelector('#f-inst').value, author }) });
      msg.innerHTML = '<div class="ok">Готово!</div>';
      setTimeout(load, 600);
    } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
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
