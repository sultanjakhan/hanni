// guest_fridge.js — Guest wrapper around fridge-shared.js. Honours add/edit/delete permissions.
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api, can, recallAuthor } = u;

  // Tunnel-first read; Firestore only when host is offline.
  const fs = (window.HanniGuest || {}).firestore;
  const haveTunnel = !!((window.__SHARE__ || {}).tunnel_url);

  async function loadCatalog() {
    if (haveTunnel) {
      try { return (await api('/recipes')).catalog || []; }
      catch (e) {
        console.warn('[guest_fridge] tunnel catalog failed, falling back to Firestore:', e?.message || e);
      }
    }
    if (!fs) return [];
    try {
      const cat = await fs.list('ingredient_catalog');
      return (cat || []).map(c => ({
        name: c.name || '', category: c.category || 'other', tags: c.tags || '',
      }));
    } catch { return []; }
  }

  async function loadFridgeItems() {
    if (haveTunnel) {
      try { return (await api('/fridge')).items || []; }
      catch (e) {
        console.warn('[guest_fridge] tunnel fridge failed, falling back to Firestore:', e?.message || e);
      }
    }
    if (!fs) throw new Error('Firestore не настроен');
    const all = await fs.list('products');
    return (all || []).filter(p => (p.location || 'fridge') === 'fridge');
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
