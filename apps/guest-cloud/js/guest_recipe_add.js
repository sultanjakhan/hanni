// guest_recipe_add.js — Thin guest wrapper around shared recipe-shared.js.
// Same window.HanniRecipe.showAddRecipeModal({ backend, onSaved, recipe? }) entry
// as Hanni — passing `recipe` puts the modal in edit-mode.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api } = u;

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
