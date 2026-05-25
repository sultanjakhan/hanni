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

  const state = { mount: null, items: [], activeLevel: 'hard' };

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

  function renderLevelBody(level, items) {
    const meta = LEVEL_META[level];
    if (!meta) return '';
    if (!items.length) {
      return `<p class="muted" style="font-size:13px;margin:8px 0">— пока ничего</p>`;
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
    return `<p class="muted" style="font-size:12px;margin-bottom:8px">${esc(meta.hint)}</p>${typeSections}`;
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
    // Three sub-tabs: one level at a time, so the page mirrors Hanni's own
    // food preferences screen instead of a long vertical stack.
    const subTabs = ['hard', 'soft', 'love'].map(lvl => {
      const meta = LEVEL_META[lvl];
      const cnt = byLevel[lvl].length;
      const active = lvl === state.activeLevel ? ' active' : '';
      return `<button class="mem-subtab mem-subtab-${lvl}${active}" data-lvl="${lvl}">
        ${meta.icon} ${esc(meta.label)} <span class="mem-subtab-count">${cnt || '•'}</span>
      </button>`;
    }).join('');
    state.mount.innerHTML = `
      <h2>Предпочтения (${state.items.length})</h2>
      <p class="muted" style="margin:0 0 12px">Что Султан ест, не ест и любит — используйте при выборе рецептов.</p>
      <div class="mem-subtabs">${subTabs}</div>
      <div class="mem-body">${renderLevelBody(state.activeLevel, byLevel[state.activeLevel])}</div>`;
    state.mount.querySelectorAll('.mem-subtab').forEach(b => {
      b.addEventListener('click', () => {
        state.activeLevel = b.dataset.lvl;
        render();
      });
    });
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.memory = { mount(el) { state.mount = el; load(); } };
})();
