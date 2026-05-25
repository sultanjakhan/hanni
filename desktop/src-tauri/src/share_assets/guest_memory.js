// guest_memory.js — Memory view: host's food preferences split by level
// (Не ем / Не люблю / Люблю) and by type (продукты / теги / категории).
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, esc, can } = u;

  const LEVEL_META = {
    hard: { icon: '🚫', label: 'Не ем',     hint: 'Не подавать ни в каком виде.',       badge: 'badge-red' },
    soft: { icon: '💔', label: 'Не люблю',  hint: 'Лучше избегать, если есть выбор.',   badge: 'badge-orange' },
    love: { icon: '❤️', label: 'Люблю',     hint: 'Любимое — добавлять в приоритете.',  badge: 'badge-green' },
  };
  const TYPE_LABEL = {
    product:  'Продукты',
    tag:      'Теги',
    category: 'Категории',
    keyword:  'Слова в названии',
    recipe:   'Рецепты',
  };
  const CAT_LABELS = { meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты',
    grain: 'Крупы', dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи',
    spice: 'Специи', oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', other: 'Другое' };

  const state = { mount: null, items: [] };

  // Tunnel-first read; Firestore only when host is offline.
  const fs = (window.HanniGuest || {}).firestore;
  const haveTunnel = !!((window.__SHARE__ || {}).tunnel_url);

  async function fetchBlacklist() {
    if (haveTunnel) {
      try { return await api('/blacklist'); }
      catch (e) {
        console.warn('[guest_memory] tunnel failed, falling back to Firestore:', e?.message || e);
      }
    }
    if (!fs) throw new Error('Firestore не настроен');
    const items = await fs.list('food_blacklist');
    return { blacklist: items || [] };
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

  function renderLevelGroup(level, items) {
    const meta = LEVEL_META[level];
    if (!meta) return '';
    // Empty section: keep the header so users see the structure (and know
    // what categories Hanni supports) instead of silently hiding it.
    if (!items.length) {
      return `<div class="recipe-detail-section mem-level mem-level-${level}" style="opacity:0.55">
        <h4>${meta.icon} ${esc(meta.label)} <span class="muted" style="font-weight:400">· пусто</span></h4>
        <p class="muted" style="font-size:12px;margin-bottom:0">— пока ничего</p>
      </div>`;
    }
    const byType = {};
    for (const it of items) (byType[it.type] = byType[it.type] || []).push(it);
    const typeSections = Object.entries(byType).map(([t, list]) => {
      const chips = list.map(it =>
        `<span class="badge ${meta.badge}" style="margin:2px 4px 2px 0;font-size:12px;padding:4px 10px">${esc(pretty(t, it.value))}</span>`
      ).join('');
      return `<div class="mem-type-row">
        <div class="mem-type-label">${esc(TYPE_LABEL[t] || t)} · ${list.length}</div>
        <div>${chips}</div>
      </div>`;
    }).join('');
    return `<div class="recipe-detail-section mem-level mem-level-${level}">
      <h4>${meta.icon} ${esc(meta.label)} <span class="muted" style="font-weight:400">· ${items.length}</span></h4>
      <p class="muted" style="font-size:12px;margin-bottom:8px">${esc(meta.hint)}</p>
      ${typeSections}
    </div>`;
  }

  function render() {
    if (!state.items.length) {
      state.mount.innerHTML = `
        <h2>Предпочтения</h2>
        <p class="muted" style="margin:8px 0 16px">Султан ещё не отметил предпочтений — можно готовить что угодно.</p>`;
      return;
    }
    const byLevel = { hard: [], soft: [], love: [] };
    for (const it of state.items) {
      const lvl = byLevel[it.level] ? it.level : 'hard';  // legacy rows w/o level
      byLevel[lvl].push(it);
    }
    // Render all three levels, even when empty — gives guests an at-a-glance
    // overview of the structure (Не ем / Не люблю / Люблю) instead of
    // hiding categories the host hasn't filled yet.
    const sections = ['hard', 'soft', 'love']
      .map(lvl => renderLevelGroup(lvl, byLevel[lvl])).join('');
    state.mount.innerHTML = `
      <h2>Предпочтения (${state.items.length})</h2>
      <p class="muted" style="margin:0 0 16px">Что Султан ест, не ест и любит — используйте при выборе рецептов.</p>
      ${sections}`;
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.memory = { mount(el) { state.mount = el; load(); } };
})();
