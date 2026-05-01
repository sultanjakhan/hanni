// guest_fridge.js — Guest wrapper around fridge-shared.js. Honours add/edit/delete permissions.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, can, recallAuthor } = u;

  // Catalog autocomplete piggybacks on /recipes (which already returns inline catalog).
  // If the share scope doesn't allow recipes we silently degrade to a free-text input.
  async function loadCatalog() {
    try { return (await api('/recipes')).catalog || []; }
    catch { return []; }
  }

  const backend = {
    list: async () => (await api('/fridge')).items || [],
    getCatalog: loadCatalog,
  };
  if (can('add')) {
    backend.add = (p) => api('/products', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...p, author: recallAuthor() || 'guest' }),
    });
  }
  if (can('edit')) {
    backend.update = (id, p) => api(`/products/${id}`, {
      method: 'PATCH', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...p, author: recallAuthor() || 'guest' }),
    });
  }
  if (can('delete')) {
    backend.remove = (id) => api(`/products/${id}`, { method: 'DELETE' });
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.fridge = {
    mount(el) {
      if (!window.HanniFridge) { el.innerHTML = '<div class="err">fridge-shared.js не загружен</div>'; return; }
      window.HanniFridge.mountInventory({ el, backend });
    },
  };
})();
