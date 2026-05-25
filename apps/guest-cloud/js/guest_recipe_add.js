// guest_recipe_add.js — Thin guest wrapper around shared recipe-shared.js.
// Same window.HanniRecipe.showAddRecipeModal({ backend, onSaved, recipe? }) entry
// as Hanni — passing `recipe` puts the modal in edit-mode.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, ctx } = u;

  // Embedded fallback cuisines — match the default rows Hanni seeds at init
  // (desktop/src-tauri/src/db.rs). Used only when both axum /cuisines and
  // the per-token localStorage cache are unavailable, so the dropdown
  // still has something usable when the host tunnel is dead and the guest
  // is on their first visit (no cache).
  const FALLBACK_CUISINES = [
    { code: 'kz',    name: 'Казахская',    emoji: '🇰🇿' },
    { code: 'ru',    name: 'Русская',      emoji: '🇷🇺' },
    { code: 'it',    name: 'Итальянская',  emoji: '🇮🇹' },
    { code: 'jp',    name: 'Японская',     emoji: '🇯🇵' },
    { code: 'ge',    name: 'Грузинская',   emoji: '🇬🇪' },
    { code: 'tr',    name: 'Турецкая',     emoji: '🇹🇷' },
    { code: 'uz',    name: 'Узбекская',    emoji: '🇺🇿' },
    { code: 'kr',    name: 'Корейская',    emoji: '🇰🇷' },
    { code: 'us',    name: 'Американская', emoji: '🇺🇸' },
    { code: 'mx',    name: 'Мексиканская', emoji: '🇲🇽' },
    { code: 'other', name: 'Другая',       emoji: '🌍' },
  ];
  const CUISINES_CACHE_KEY = 'hg:cuisines:' + (ctx?.token || '_');

  async function fetchCuisinesResilient() {
    // 1. Try axum — when the host is online this gives the freshest list
    //    including custom cuisines the host added.
    try {
      const resp = await api('/cuisines');
      const list = resp?.cuisines || [];
      if (list.length) {
        try { localStorage.setItem(CUISINES_CACHE_KEY, JSON.stringify(list)); } catch {}
        return list;
      }
    } catch {}
    // 2. Last-known-good list from localStorage. Survives across tunnel
    //    restarts and even Hanni quits, as long as the guest visited once
    //    while the host was up.
    try {
      const cached = JSON.parse(localStorage.getItem(CUISINES_CACHE_KEY) || 'null');
      if (Array.isArray(cached) && cached.length) return cached;
    } catch {}
    // 3. Embedded defaults — covers the recipes the host is most likely to
    //    have, so the cuisine chip resolves correctly even on a cold visit.
    return FALLBACK_CUISINES;
  }

  function makeBackend(catalog) {
    return {
      // Catalog comes pre-loaded by guest_recipes.js (from /recipes response).
      // Convert the {name: category} map back into the array shape recipe-
      // shared-ingredients.js expects.
      getCatalog: async () => {
        if (Array.isArray(catalog)) return catalog;
        if (catalog && typeof catalog === 'object') {
          return Object.entries(catalog).map(([name, category]) => ({ name, category }));
        }
        return [];
      },
      getCuisines: fetchCuisinesResilient,
      getBlacklist: async () => [],
      addCatalogItem: async ({ name, category }) => {
        const resp = await api('/catalog', {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ name, category }),
        });
        return resp && typeof resp.id === 'number' ? resp.id : null;
      },
      addCuisine: ({ code, name, emoji }) => api('/cuisines', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code, name, emoji }),
      }),
      createRecipe: (payload) => api('/recipes', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...payload, author: u.recallAuthor() || 'guest' }),
      }),
      updateRecipe: (id, payload) => api(`/recipes/${id}`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...payload, author: u.recallAuthor() || 'guest' }),
      }),
    };
  }

  async function showAddRecipeModal(catalog, reloadFn) {
    if (!window.HanniRecipe) { alert('recipe-shared.js не загружен'); return; }
    return window.HanniRecipe.showAddRecipeModal({
      backend: makeBackend(catalog),
      onSaved: reloadFn,
    });
  }

  async function showEditRecipeModal(recipe, catalog, reloadFn) {
    if (!window.HanniRecipe) { alert('recipe-shared.js не загружен'); return; }
    return window.HanniRecipe.showAddRecipeModal({
      backend: makeBackend(catalog),
      onSaved: reloadFn,
      recipe,
    });
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.recipeAdd = { showAddRecipeModal, showEditRecipeModal };
})();
