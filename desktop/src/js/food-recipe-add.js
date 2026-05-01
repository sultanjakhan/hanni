// ── food-recipe-add.js — Thin Hanni wrapper around shared recipe-shared.js ──
// Shared logic: window.HanniRecipe.showAddRecipeModal({ backend, onSaved }).
// Loaded via <script> tag in index.html (sets up window.HanniRecipe).
import { invoke } from './state.js';
import { loadCuisines, invalidateCuisineCache, getBlacklist, invalidateCatalogCache } from './food-recipe-filters.js';

// Convert snake_case payload from shared module → camelCase for Tauri invoke.
function toCamel(obj) {
  const out = {};
  for (const [k, v] of Object.entries(obj)) {
    out[k.replace(/_([a-z])/g, (_, c) => c.toUpperCase())] = v;
  }
  return out;
}

export async function showAddRecipeModal(reloadFn) {
  if (!window.HanniRecipe) {
    alert('Модуль recipe-shared.js не загружен'); return;
  }
  return window.HanniRecipe.showAddRecipeModal({
    backend: {
      getCatalog: () => invoke('get_ingredient_catalog').catch(() => []),
      getCuisines: () => loadCuisines(),
      getBlacklist: () => getBlacklist(),
      addCatalogItem: async ({ name, category }) => {
        const id = await invoke('add_ingredient_to_catalog', { name, category });
        invalidateCatalogCache();
        return id;
      },
      addCuisine: ({ code, name, emoji }) => invoke('add_cuisine', { code, name, emoji })
        .then(() => invalidateCuisineCache()),
      createRecipe: (payload) => invoke('create_recipe', toCamel(payload)),
    },
    onSaved: reloadFn,
  });
}
