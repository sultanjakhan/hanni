// guest_products.js — products catalog: drill-down (categories → subgroups → items).
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can } = u;

  const CAT_LABELS = {
    meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты',
    grain: 'Крупы', dairy: 'Молочное', legumes: 'Бобовые', nuts: 'Орехи',
    spice: 'Специи', oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки',
    other: 'Прочее', sweet: 'Сладости', frozen: 'Заморозка',
  };
  const CAT_EMOJI = {
    meat: '🥩', fish: '🐟', veg: '🥦', fruit: '🍎', grain: '🌾',
    dairy: '🥛', legumes: '🫘', nuts: '🥜', spice: '🌶️', oil: '🫒',
    bakery: '🍞', drinks: '🥤', sweet: '🍬', frozen: '🧊', other: '📦',
  };
  const CAT_COLORS = {
    meat: 'red', fish: 'blue', veg: 'green', fruit: 'orange', grain: 'yellow',
    dairy: 'purple', legumes: 'teal', nuts: 'brown', spice: 'pink',
    oil: 'amber', bakery: 'warm', drinks: 'cyan', sweet: 'pink', frozen: 'cyan', other: 'gray',
  };

  const state = { mount: null, items: [], path: [] };

  // Stage C-1: prefer Firestore mirror so the catalog stays browsable when
  // the host is offline. The "products" view shows ingredient_catalog rows.
  const fs = (window.HanniGuest || {}).firestore;

  async function fetchCatalog() {
    if (fs) {
      try {
        const items = await fs.list('ingredient_catalog');
        return { products: (items || []).sort((a, b) =>
          String(a.name || '').localeCompare(String(b.name || ''))) };
      } catch (e) {
        console.warn('[guest_products] firestore failed, falling back to axum:', e?.message || e);
      }
    }
    return await api('/products');
  }

  async function load() {
    if (!can('view')) {
      state.mount.innerHTML = '<div class="empty">Без прав просмотра.</div>';
      return;
    }
    try {
      const data = await fetchCatalog();
      state.items = data.products || [];
      render();
    } catch (e) {
      state.mount.innerHTML = `<div class="err">Ошибка: ${esc(e.message || e)}</div>`;
    }
  }

  function breadcrumbHtml() {
    const parts = [{ label: 'Все продукты', idx: -1 }];
    if (state.path[0]) parts.push({ label: CAT_LABELS[state.path[0]] || state.path[0], idx: 0 });
    if (state.path[1]) parts.push({ label: state.path[1], idx: 1 });
    return `<div class="pp-breadcrumb">${parts.map((p, i) =>
      `<span class="pp-crumb${i === parts.length - 1 ? ' pp-crumb-active' : ''}" data-crumb="${p.idx}">${esc(p.label)}</span>${i < parts.length - 1 ? '<span class="pp-crumb-sep">›</span>' : ''}`
    ).join('')}</div>`;
  }

  function categoriesHtml() {
    const groups = {};
    state.items.forEach(p => { groups[p.category] = (groups[p.category] || 0) + 1; });
    const keys = Object.keys(groups).sort((a, b) => (CAT_LABELS[a] || a).localeCompare(CAT_LABELS[b] || b));
    return `<div class="cat-grid">${keys.map(k =>
      `<div class="cat-tile" data-cat="${esc(k)}">
        <div class="cat-tile-emoji">${CAT_EMOJI[k] || '📦'}</div>
        <div class="cat-tile-name">${esc(CAT_LABELS[k] || k)}</div>
        <div class="cat-tile-count">${groups[k]}</div>
      </div>`).join('')}</div>`;
  }

  function subgroupsHtml(cat) {
    const filtered = state.items.filter(p => p.category === cat);
    const groups = {};
    filtered.forEach(p => {
      const sg = p.subgroup || '— без подгруппы —';
      groups[sg] = (groups[sg] || 0) + 1;
    });
    const keys = Object.keys(groups).sort();
    return `<div class="sg-grid">${keys.map(k =>
      `<div class="sg-tile" data-sg="${esc(k)}">
        <div class="sg-tile-name">${esc(k)}</div>
        <div class="sg-tile-count">${groups[k]}</div>
      </div>`).join('')}</div>`;
  }

  function productsHtml(cat, sg) {
    const filtered = state.items.filter(p =>
      p.category === cat && (sg === '— без подгруппы —' ? !p.subgroup : p.subgroup === sg)
    );
    if (!filtered.length) return '<div class="empty">Пусто.</div>';
    return `<div class="product-grid">${filtered.map(p => {
      const color = CAT_COLORS[p.category] || 'gray';
      const label = CAT_LABELS[p.category] || p.category;
      const tags = (p.tags || '').split(/[,\s]+/).filter(Boolean).slice(0, 4);
      const tagsHtml = tags.map(t => `<span class="product-card-tag">${esc(t)}</span>`).join('');
      return `<div class="product-card" data-id="${p.id}">
        <div class="product-card-name">${esc(p.name)}</div>
        <div class="product-card-tags">
          <span class="product-card-cat product-cat-${color}">${esc(label)}</span>
          ${tagsHtml}
        </div>
      </div>`;
    }).join('')}</div>`;
  }

  function render() {
    let body = '';
    if (!state.path.length) {
      body = `<h2>Каталог продуктов (${state.items.length})</h2>${categoriesHtml()}`;
    } else if (state.path.length === 1) {
      const cat = state.path[0];
      const cnt = state.items.filter(p => p.category === cat).length;
      body = `<h2>${esc(CAT_LABELS[cat] || cat)} (${cnt})</h2>${subgroupsHtml(cat)}`;
    } else {
      const cnt = state.items.filter(p =>
        p.category === state.path[0] &&
        (state.path[1] === '— без подгруппы —' ? !p.subgroup : p.subgroup === state.path[1])
      ).length;
      body = `<h2>${esc(state.path[1])} (${cnt})</h2>${productsHtml(state.path[0], state.path[1])}`;
    }
    state.mount.innerHTML = breadcrumbHtml() + body;

    state.mount.querySelectorAll('[data-crumb]').forEach(c => c.addEventListener('click', () => {
      const idx = parseInt(c.dataset.crumb);
      state.path = idx < 0 ? [] : state.path.slice(0, idx + 1);
      render();
    }));
    state.mount.querySelectorAll('[data-cat]').forEach(t =>
      t.addEventListener('click', () => { state.path = [t.dataset.cat]; render(); }));
    state.mount.querySelectorAll('[data-sg]').forEach(t =>
      t.addEventListener('click', () => { state.path = [state.path[0], t.dataset.sg]; render(); }));
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.products = { mount(el) { state.mount = el; load(); } };
})();
