// guest_memory.js — Memory view: shows host's food blacklist (what NOT to cook).
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can } = u;

  const TYPE_LABELS = {
    product: '🚫 Не есть продукты',
    category: '🚫 Не есть категории',
    tag: '🚫 Не есть теги',
    keyword: '🚫 Не есть слова в названии',
  };
  const TYPE_DESC = {
    product: 'Конкретные продукты, которые не подавать.',
    category: 'Целые категории (мясо, молочное и т.п.).',
    tag: 'Все рецепты или ингредиенты с этими тегами.',
    keyword: 'Любой рецепт, в названии которого встречается слово.',
  };

  // Categories nice-print (mirror food-recipe-filters.js CAT_LABELS).
  const CAT_LABELS = { meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты',
    grain: 'Крупы', dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи',
    spice: 'Специи', oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', other: 'Другое' };

  const state = { mount: null, items: [] };

  // Stage C-1: pull blacklist from Firestore mirror so the guest stays alive
  // when Hanni is offline.
  const fs = (window.HanniGuest || {}).firestore;

  async function fetchBlacklist() {
    if (fs) {
      try {
        const items = await fs.list('food_blacklist');
        return { blacklist: items || [] };
      } catch (e) {
        console.warn('[guest_memory] firestore failed, falling back:', e?.message || e);
      }
    }
    return await api('/blacklist');
  }

  async function load() {
    if (!can('view')) {
      state.mount.innerHTML = '<div class="empty">Без прав просмотра.</div>';
      return;
    }
    try {
      const data = await fetchBlacklist();
      state.items = data.blacklist || [];
      render();
    } catch (e) {
      state.mount.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function pretty(type, value) {
    if (type === 'category') return CAT_LABELS[value] || value;
    return value;
  }

  function render() {
    if (!state.items.length) {
      state.mount.innerHTML = `
        <h2>Память</h2>
        <p class="muted" style="margin:8px 0 16px">Что хозяин не ест — здесь будет список ограничений.</p>
        <div class="empty">Пока ограничений нет — можно готовить что угодно.</div>`;
      return;
    }

    const groups = {};
    for (const it of state.items) {
      (groups[it.type] = groups[it.type] || []).push(it);
    }

    const sections = ['product', 'category', 'tag', 'keyword']
      .filter(t => groups[t]?.length)
      .map(t => {
        const items = groups[t].map(it =>
          `<span class="badge badge-red" style="margin:2px 4px 2px 0;font-size:12px;padding:4px 10px">${esc(pretty(t, it.value))}</span>`
        ).join('');
        return `<div class="recipe-detail-section">
          <h4>${TYPE_LABELS[t] || t}</h4>
          <p class="muted" style="font-size:12px;margin-bottom:8px">${TYPE_DESC[t] || ''}</p>
          <div>${items}</div>
        </div>`;
      }).join('');

    state.mount.innerHTML = `
      <h2>Память (${state.items.length})</h2>
      <p class="muted" style="margin:0 0 16px">Что хозяин <b>не ест</b> — учитывайте при добавлении рецептов и планировании.</p>
      ${sections}`;
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.memory = { mount(el) { state.mount = el; load(); } };
})();
