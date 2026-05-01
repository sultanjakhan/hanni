// guest_recipe_add.js — Thin guest wrapper around shared recipe-shared.js.
// Same window.HanniRecipe.showAddRecipeModal({ backend, onSaved }) entry as Hanni.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api } = u;

  async function showAddRecipeModal(catalog, reloadFn) {
    if (!window.HanniRecipe) { alert('recipe-shared.js не загружен'); return; }
    return window.HanniRecipe.showAddRecipeModal({
      backend: {
        // Catalog comes pre-loaded by guest_recipes.js (from /recipes response).
        // Fall back to a /catalog call if needed in the future.
        getCatalog: async () => catalog,
        getCuisines: async () => (await api('/cuisines').catch(() => ({ cuisines: [] }))).cuisines || [],
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
      },
      onSaved: reloadFn,
    });
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.recipeAdd = { showAddRecipeModal };
})();
