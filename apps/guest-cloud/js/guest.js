// guest.js — skeleton: shared utils + tab routing for share-link guest UI.
(function () {
  const ctx = window.__SHARE__ || {};
  // Static-hosted (Firebase Hosting) → ctx.tunnel_url is the absolute axum
  // URL the host published to Firestore. Server-rendered (axum landing) →
  // tunnel_url is undefined and we fall back to relative /s/<token>.
  const tunnel = (ctx.tunnel_url || '').replace(/\/+$/, '');
  const base = (tunnel || '') + `/s/${encodeURIComponent(ctx.token)}`;
  const app = document.getElementById('app');
  const perms = ctx.permissions || [];
  const can = (p) => perms.includes(p);
  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  async function api(path, opts = {}) {
    // When hosted on Firebase (https://*.web.app) without a known tunnel URL,
    // the host is offline — surface a clean error so view-modules can hide
    // write controls instead of throwing CORS spam.
    const isStaticOrigin = /\.web\.app$|\.firebaseapp\.com$/.test(location.host);
    if (isStaticOrigin && !tunnel) {
      const err = new Error('Hanni офлайн — изменения недоступны');
      err.code = 'HOST_OFFLINE';
      throw err;
    }
    const r = await fetch(base + path, { credentials: 'omit', ...opts });
    if (!r.ok) {
      // 410 Gone == revoked/expired link. Axum sends a plain-text body
      // like "Link revoked" or "Link expired". Surface a typed error so
      // the bootstrap can show "Ссылка отозвана" instead of a generic
      // Firestore-fallback failure.
      const body = (await r.text()) || r.statusText;
      const err = new Error(body);
      if (r.status === 410) err.code = body.toLowerCase().includes('expired')
        ? 'LINK_EXPIRED' : 'LINK_REVOKED';
      else if (r.status === 403) err.code = 'LINK_FORBIDDEN';
      err.status = r.status;
      throw err;
    }
    // After a successful write, drop the matching Firestore cache so the next
    // read fetches fresh data (writes go through axum; Firestore mirror lags).
    const m = (opts.method || 'GET').toUpperCase();
    if (m !== 'GET') {
      const coll = (path.split('/')[1] || '').replace(/[?#].*/, '');
      if (coll) window.HanniGuest?.firestore?.invalidate?.(coll);
    }
    return r.json();
  }

  function rememberAuthor(name) {
    if (!name) return;
    try { localStorage.setItem('hanni-share-author', name); } catch {}
  }
  function recallAuthor() {
    try { return localStorage.getItem('hanni-share-author') || ''; } catch { return ''; }
  }

  // Available views by tab.scope:
  //   food + recipes  → recipes only
  //   food + products → products only
  //   food + meal_plan → meal plan only
  //   food + all      → tab bar with all three
  function viewsFor() {
    if (ctx.tab !== 'food') return [];
    if (ctx.scope === 'recipes') return [{ id: 'recipes', label: '🍔 Рецепты' }];
    if (ctx.scope === 'products') return [{ id: 'products', label: '🛒 Продукты' }];
    if (ctx.scope === 'meal_plan') return [{ id: 'meal_plan', label: '🍽 План' }];
    if (ctx.scope === 'memory') return [{ id: 'memory', label: '🧠 Предпочтения' }];
    if (ctx.scope === 'fridge') return [{ id: 'fridge', label: '🥶 Холодильник' }];
    return [
      { id: 'recipes', label: '🍔 Рецепты' },
      { id: 'products', label: '🛒 Продукты' },
      { id: 'fridge', label: '🥶 Холодильник' },
      { id: 'meal_plan', label: '🍽 План' },
      { id: 'memory', label: '🧠 Предпочтения' },
    ];
  }

  function mountView(viewId, mountEl) {
    const mod = (window.HanniGuest || {})[viewId];
    if (!mod || typeof mod.mount !== 'function') {
      mountEl.innerHTML = `<div class="err">Внутренняя ошибка: модуль "${esc(viewId)}" не загружен.</div>`;
      return;
    }
    mod.mount(mountEl);
  }

  function renderShell() {
    const views = viewsFor();
    if (!views.length) {
      app.innerHTML = '<div class="empty">Этот таб пока не поддерживает шаринг.</div>';
      return;
    }
    if (views.length === 1) {
      app.innerHTML = '<div id="view-mount"></div>';
      mountView(views[0].id, app.querySelector('#view-mount'));
      return;
    }
    const tabsHtml = views.map(v =>
      `<button class="guest-tab" data-view="${v.id}">${esc(v.label)}</button>`
    ).join('');
    app.innerHTML = `
      <div class="guest-tabs">${tabsHtml}</div>
      <div id="view-mount"></div>`;
    const mount = app.querySelector('#view-mount');
    const tabs = Array.from(app.querySelectorAll('.guest-tab'));
    function activate(id) {
      tabs.forEach(t => t.classList.toggle('active', t.dataset.view === id));
      mountView(id, mount);
    }
    tabs.forEach(t => t.addEventListener('click', () => activate(t.dataset.view)));
    activate(views[0].id);
  }

  // Global "/" → focus search (when not in input/textarea, no modal open).
  document.addEventListener('keydown', (e) => {
    if (e.key !== '/' || e.metaKey || e.ctrlKey || e.altKey) return;
    const tag = (document.activeElement?.tagName || '').toLowerCase();
    if (tag === 'input' || tag === 'textarea') return;
    if (document.querySelector('.modal-overlay')) return;
    const search = document.querySelector('.recipe-search');
    if (search) { e.preventDefault(); search.focus(); search.select(); }
  });

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.utils = { ctx, base, api, esc, can, rememberAuthor, recallAuthor };
  // Exposed so the Firebase landing (which appends view-modules dynamically
  // AFTER DOMContentLoaded) can trigger the initial render at the right time.
  window.HanniGuest.renderShell = renderShell;

  // Don't render until view-modules (recipes/products/etc) are registered.
  // The Firebase landing script appends them dynamically AFTER guest.js, then
  // explicitly calls window.HanniGuest.renderShell(). Auto-rendering here
  // would race: at first call HanniGuest.recipes is still undefined and
  // mountView() would flash "Внутренняя ошибка: модуль 'recipes' не загружен".
})();
