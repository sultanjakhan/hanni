// guest.js — Guest-facing UI for share-links (view / add / edit / comment).
(function () {
  const ctx = window.__SHARE__ || {};
  const base = `/s/${encodeURIComponent(ctx.token)}`;
  const app = document.getElementById('app');
  const perms = ctx.permissions || [];
  const can = (p) => perms.includes(p);
  const esc = (s) => String(s || '').replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  let state = { view: 'list', recipes: [], current: null, comments: [] };

  async function api(path, opts = {}) {
    const r = await fetch(base + path, { credentials: 'omit', ...opts });
    if (!r.ok) throw new Error((await r.text()) || r.statusText);
    return r.json();
  }

  function render() {
    if (state.view === 'detail' && state.current) return renderDetail();
    return renderList();
  }

  async function loadList() {
    if (!can('view') && !can('add')) {
      app.innerHTML = '<div class="empty">Ссылка активна, но без прав просмотра.</div>';
      return;
    }
    try {
      if (can('view')) {
        const data = await api('/recipes');
        state.recipes = data.recipes || [];
      }
      state.view = 'list';
      render();
    } catch (e) {
      app.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function renderList() {
    const listHtml = can('view') ? (state.recipes.length
      ? state.recipes.map((r) => `
          <div class="card" data-id="${r.id}" style="cursor:pointer">
            <div class="card-title">${esc(r.name)}</div>
            <div class="card-meta">
              ${r.prep_time || r.cook_time ? `<span>⏱ ${(r.prep_time || 0) + (r.cook_time || 0)} мин</span>` : ''}
              ${r.servings ? `<span>🍽 ${r.servings} порц.</span>` : ''}
              ${r.calories ? `<span>🔥 ${r.calories} ккал</span>` : ''}
            </div>
          </div>`).join('')
      : '<div class="empty">Рецептов пока нет.</div>') : '';

    const addHtml = can('add') ? `
      <div class="card" style="margin-top:20px">
        <div class="card-title">Добавить рецепт</div>
        <label>Название</label><input id="f-name">
        <label>Ингредиенты</label><textarea id="f-ingr" rows="3"></textarea>
        <label>Инструкции</label><textarea id="f-inst" rows="4"></textarea>
        <label>Ваше имя (опционально)</label><input id="f-author">
        <div id="f-msg"></div>
        <div class="row-actions"><button class="btn" id="f-save">Отправить</button></div>
      </div>` : '';

    app.innerHTML = `
      <h2 style="margin:0 0 14px">Рецепты${can('view') ? ` (${state.recipes.length})` : ''}</h2>
      ${listHtml}${addHtml}`;
    app.querySelectorAll('.card[data-id]').forEach(el =>
      el.addEventListener('click', () => openDetail(parseInt(el.dataset.id))));
    const save = app.querySelector('#f-save');
    if (save) save.addEventListener('click', submitNew);
  }

  async function openDetail(id) {
    try {
      state.current = await api(`/recipes/${id}`);
      state.comments = (await api(`/recipes/${id}/comments`).catch(() => ({ comments: [] }))).comments || [];
      state.view = 'detail';
      render();
    } catch (e) {
      alert('Ошибка: ' + (e.message || e));
    }
  }

  function renderDetail() {
    const r = state.current;
    const editForm = can('edit') ? `
      <details style="margin-top:20px"><summary>✏️ Редактировать</summary>
        <div style="margin-top:10px">
          <label>Название</label><input id="e-name" value="${esc(r.name)}">
          <label>Описание</label><textarea id="e-desc" rows="2">${esc(r.description)}</textarea>
          <label>Ингредиенты</label><textarea id="e-ingr" rows="3">${esc(r.ingredients)}</textarea>
          <label>Инструкции</label><textarea id="e-inst" rows="4">${esc(r.instructions)}</textarea>
          <label>Ваше имя</label><input id="e-author">
          <div id="e-msg"></div>
          <div class="row-actions"><button class="btn" id="e-save">Сохранить</button></div>
        </div>
      </details>` : '';
    const commentsList = state.comments.length
      ? state.comments.map(c => `<div class="card">
          <div class="card-meta"><b>${esc(c.author)}</b> · <span>${esc((c.created_at || '').slice(0, 16))}</span></div>
          <div style="margin-top:4px">${esc(c.text)}</div>
        </div>`).join('')
      : '<div class="muted" style="font-size:13px">Пока нет заметок.</div>';
    const commentForm = can('comment') ? `
      <div class="card" style="margin-top:14px">
        <div class="card-title">Оставить заметку</div>
        <label>Ваше имя</label><input id="c-author">
        <label>Текст</label><textarea id="c-text" rows="3"></textarea>
        <div id="c-msg"></div>
        <div class="row-actions"><button class="btn" id="c-save">Отправить</button></div>
      </div>` : '';

    app.innerHTML = `
      <button class="btn btn-ghost" id="back" style="margin-bottom:14px">← Назад</button>
      <h2 style="margin:0 0 6px">${esc(r.name)}</h2>
      <div class="card-meta" style="margin-bottom:14px">
        ${r.prep_time || r.cook_time ? `<span>⏱ ${(r.prep_time || 0) + (r.cook_time || 0)} мин</span>` : ''}
        ${r.servings ? `<span>🍽 ${r.servings} порц.</span>` : ''}
        ${r.calories ? `<span>🔥 ${r.calories} ккал</span>` : ''}
      </div>
      ${r.description ? `<p>${esc(r.description)}</p>` : ''}
      ${r.ingredients ? `<h3>Ингредиенты</h3><pre style="white-space:pre-wrap;font-family:inherit;font-size:14px">${esc(r.ingredients)}</pre>` : ''}
      ${r.instructions ? `<h3>Инструкции</h3><pre style="white-space:pre-wrap;font-family:inherit;font-size:14px">${esc(r.instructions)}</pre>` : ''}
      ${editForm}
      <h3 style="margin-top:24px">Заметки гостей</h3>
      ${commentsList}
      ${commentForm}`;

    app.querySelector('#back').addEventListener('click', () => { state.view = 'list'; state.current = null; render(); });
    const eSave = app.querySelector('#e-save'); if (eSave) eSave.addEventListener('click', submitEdit);
    const cSave = app.querySelector('#c-save'); if (cSave) cSave.addEventListener('click', submitComment);
  }

  async function submitNew() {
    const msg = app.querySelector('#f-msg');
    const name = app.querySelector('#f-name').value.trim();
    if (!name) { msg.innerHTML = '<div class="err">Название обязательно</div>'; return; }
    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      await api('/recipes', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name,
          ingredients: app.querySelector('#f-ingr').value,
          instructions: app.querySelector('#f-inst').value,
          author: app.querySelector('#f-author').value.trim() || 'guest',
        }),
      });
      msg.innerHTML = '<div class="ok">Готово!</div>';
      setTimeout(loadList, 700);
    } catch (e) {
      msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`;
    }
  }

  async function submitEdit() {
    const msg = app.querySelector('#e-msg');
    const id = state.current.id;
    msg.innerHTML = '<div class="muted">Сохраняем…</div>';
    try {
      await api(`/recipes/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: app.querySelector('#e-name').value,
          description: app.querySelector('#e-desc').value,
          ingredients: app.querySelector('#e-ingr').value,
          instructions: app.querySelector('#e-inst').value,
          author: app.querySelector('#e-author').value.trim() || 'guest',
        }),
      });
      msg.innerHTML = '<div class="ok">Обновлено!</div>';
      setTimeout(() => openDetail(id), 600);
    } catch (e) {
      msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`;
    }
  }

  async function submitComment() {
    const msg = app.querySelector('#c-msg');
    const text = app.querySelector('#c-text').value.trim();
    if (!text) { msg.innerHTML = '<div class="err">Текст обязателен</div>'; return; }
    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      await api(`/recipes/${state.current.id}/comments`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          text,
          author: app.querySelector('#c-author').value.trim() || 'Guest',
        }),
      });
      msg.innerHTML = '<div class="ok">Заметка добавлена</div>';
      setTimeout(() => openDetail(state.current.id), 600);
    } catch (e) {
      msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`;
    }
  }

  loadList();
})();
