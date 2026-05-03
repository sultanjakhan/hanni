// guest_fridge.js — Guest wrapper around fridge-shared.js. Honours add/edit/delete permissions.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, can, recallAuthor } = u;

  // Stage C-1: read fridge inventory (products with location=fridge) and the
  // ingredient catalog from Firestore mirror when configured, so the guest
  // works while Hanni is offline. Writes still go via axum.
  const fs = (window.HanniGuest || {}).firestore;

  async function loadCatalog() {
    if (fs) {
      try {
        const cat = await fs.list('ingredient_catalog');
        return (cat || []).map(c => ({
          name: c.name || '', category: c.category || 'other', tags: c.tags || '',
        }));
      } catch (e) {
        console.warn('[guest_fridge] firestore catalog failed, falling back:', e?.message || e);
      }
    }
    try { return (await api('/recipes')).catalog || []; }
    catch { return []; }
  }

  async function loadFridgeItems() {
    if (fs) {
      try {
        const all = await fs.list('products');
        return (all || []).filter(p => (p.location || 'fridge') === 'fridge');
      } catch (e) {
        console.warn('[guest_fridge] firestore products failed, falling back:', e?.message || e);
      }
    }
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
