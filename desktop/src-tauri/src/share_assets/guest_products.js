// guest_products.js — products view: list + add/edit/delete inventory.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can, rememberAuthor, recallAuthor } = u;

  const CATEGORY_LABELS = {
    other: 'Прочее', meat: 'Мясо', fish: 'Рыба', dairy: 'Молочное',
    veg: 'Овощи', fruit: 'Фрукты', grain: 'Крупы', bakery: 'Выпечка',
    drink: 'Напитки', sweet: 'Сладости', frozen: 'Заморозка',
  };
  const LOCATION_LABELS = { fridge: '❄️ Холодильник', freezer: '🧊 Морозилка', pantry: '🥫 Полка' };

  const state = { mount: null, items: [], editing: null };

  function expiryBadge(date) {
    if (!date) return '';
    const today = new Date(); today.setHours(0, 0, 0, 0);
    const exp = new Date(date);
    if (isNaN(exp.getTime())) return '';
    const days = Math.round((exp - today) / 86400000);
    if (days < 0) return `<span class="badge badge-danger">Просрочено ${-days}д</span>`;
    if (days === 0) return `<span class="badge badge-warn">Сегодня</span>`;
    if (days <= 3) return `<span class="badge badge-warn">${days}д</span>`;
    return `<span class="badge">${days}д</span>`;
  }

  async function load() {
    if (!can('view') && !can('add')) {
      state.mount.innerHTML = '<div class="empty">Ссылка активна, но без прав просмотра продуктов.</div>';
      return;
    }
    try {
      if (can('view')) {
        const data = await api('/products');
        state.items = data.products || [];
      }
      render();
    } catch (e) {
      state.mount.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function render() {
    const list = can('view') ? (state.items.length
      ? state.items.map(p => {
          const cat = CATEGORY_LABELS[p.category] || p.category;
          const loc = LOCATION_LABELS[p.location] || p.location;
          const qty = `${p.quantity} ${p.unit || ''}`.trim();
          const editBtn = can('edit') ? `<button class="link-btn" data-edit="${p.id}">✏️</button>` : '';
          const delBtn = can('delete') ? `<button class="link-btn link-btn-danger" data-del="${p.id}">×</button>` : '';
          return `<div class="card prod-card prod-cat-${esc(p.category)}">
            <div class="prod-top">
              <div class="prod-name">${esc(p.name)}</div>
              <div class="prod-actions">${editBtn}${delBtn}</div>
            </div>
            <div class="card-meta">
              <span>${esc(cat)}</span>
              <span>${esc(qty)}</span>
              <span>${esc(loc)}</span>
              ${expiryBadge(p.expiry_date)}
            </div>
            ${p.notes ? `<div class="prod-notes">${esc(p.notes)}</div>` : ''}
          </div>`;
        }).join('')
      : '<div class="empty">Холодильник пуст.</div>') : '';

    const addBlock = can('add') ? renderAddForm() : '';
    state.mount.innerHTML = `
      <h2 style="margin:0 0 14px">Продукты${can('view') ? ` (${state.items.length})` : ''}</h2>
      ${list}${addBlock}`;

    state.mount.querySelectorAll('[data-del]').forEach(b =>
      b.addEventListener('click', () => removeProduct(parseInt(b.dataset.del))));
    state.mount.querySelectorAll('[data-edit]').forEach(b =>
      b.addEventListener('click', () => startEdit(parseInt(b.dataset.edit))));
    const save = state.mount.querySelector('#p-save');
    if (save) save.addEventListener('click', submitNew);
    const cancel = state.mount.querySelector('#p-cancel');
    if (cancel) cancel.addEventListener('click', () => { state.editing = null; render(); });
  }

  function categoryOptions(selected) {
    return Object.entries(CATEGORY_LABELS).map(([v, l]) =>
      `<option value="${v}" ${v === selected ? 'selected' : ''}>${esc(l)}</option>`).join('');
  }
  function locationOptions(selected) {
    return Object.entries(LOCATION_LABELS).map(([v, l]) =>
      `<option value="${v}" ${v === selected ? 'selected' : ''}>${esc(l)}</option>`).join('');
  }

  function renderAddForm() {
    const e = state.editing;
    const title = e ? 'Изменить продукт' : 'Добавить продукт';
    const btn = e ? 'Сохранить' : 'Отправить';
    const cancelBtn = e ? `<button class="btn btn-ghost" id="p-cancel">Отмена</button>` : '';
    return `<div class="card" style="margin-top:20px">
      <div class="card-title">${title}</div>
      <label>Название</label><input id="p-name" value="${esc(e ? e.name : '')}">
      <label>Категория</label><select id="p-cat">${categoryOptions(e ? e.category : 'other')}</select>
      <div style="display:flex;gap:10px">
        <div style="flex:1"><label>Кол-во</label><input id="p-qty" type="number" step="0.1" value="${e ? e.quantity : 1}"></div>
        <div style="flex:1"><label>Ед.</label><input id="p-unit" value="${esc(e ? e.unit : 'шт')}"></div>
      </div>
      <label>Срок годности</label><input id="p-exp" type="date" value="${esc(e && e.expiry_date ? e.expiry_date.slice(0,10) : '')}">
      <label>Хранение</label><select id="p-loc">${locationOptions(e ? e.location : 'fridge')}</select>
      <label>Заметка</label><input id="p-notes" value="${esc(e ? e.notes : '')}">
      <label>Ваше имя (опционально)</label><input id="p-author" value="${esc(recallAuthor())}">
      <div id="p-msg"></div>
      <div class="row-actions">
        <button class="btn" id="p-save">${btn}</button>
        ${cancelBtn}
      </div>
    </div>`;
  }

  function startEdit(id) {
    state.editing = state.items.find(p => p.id === id) || null;
    render();
    state.mount.querySelector('#p-name')?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  async function submitNew() {
    const m = state.mount;
    const msg = m.querySelector('#p-msg');
    const name = m.querySelector('#p-name').value.trim();
    if (!name) { msg.innerHTML = '<div class="err">Название обязательно</div>'; return; }
    const author = m.querySelector('#p-author').value.trim() || 'guest';
    rememberAuthor(author);

    const payload = {
      name,
      category: m.querySelector('#p-cat').value,
      quantity: parseFloat(m.querySelector('#p-qty').value) || 1,
      unit: m.querySelector('#p-unit').value.trim() || 'шт',
      expiry_date: m.querySelector('#p-exp').value || null,
      location: m.querySelector('#p-loc').value,
      notes: m.querySelector('#p-notes').value.trim(),
      author,
    };

    msg.innerHTML = '<div class="muted">Отправка…</div>';
    try {
      if (state.editing) {
        await api(`/products/${state.editing.id}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
      } else {
        await api('/products', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
      }
      state.editing = null;
      msg.innerHTML = '<div class="ok">Готово!</div>';
      setTimeout(load, 500);
    } catch (e) { msg.innerHTML = `<div class="err">${esc(e.message || e)}</div>`; }
  }

  async function removeProduct(id) {
    const item = state.items.find(p => p.id === id);
    if (!confirm(`Удалить "${item ? item.name : 'продукт'}"?`)) return;
    try {
      await api(`/products/${id}`, { method: 'DELETE' });
      state.items = state.items.filter(p => p.id !== id);
      render();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.products = {
    mount(el) { state.mount = el; load(); },
  };
})();
