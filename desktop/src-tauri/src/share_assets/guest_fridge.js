// guest_fridge.js — Guest wrapper around fridge-shared.js. Honours add/edit/delete permissions.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, can, recallAuthor } = u;

  async function loadCatalog() {
    return (await api('/recipes')).catalog || [];
  }

  async function loadFridgeItems() {
    return (await api('/fridge')).items || [];
  }

  const backend = {
    list: loadFridgeItems,
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
