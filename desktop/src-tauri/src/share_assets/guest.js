// guest.js — skeleton: shared utils + tab routing for share-link guest UI.
(function () {
  const ctx = window.__SHARE__ || {};
  const base = `/s/${encodeURIComponent(ctx.token)}`;
  const app = document.getElementById('app');
  const perms = ctx.permissions || [];
  const can = (p) => perms.includes(p);
  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  async function api(path, opts = {}) {
    const r = await fetch(base + path, { credentials: 'omit', ...opts });
    if (!r.ok) throw new Error((await r.text()) || r.statusText);
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
    return [
      { id: 'recipes', label: '🍔 Рецепты' },
      { id: 'products', label: '🛒 Продукты' },
      { id: 'meal_plan', label: '🍽 План' },
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

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.utils = { ctx, base, api, esc, can, rememberAuthor, recallAuthor };

  // Defer mount so view-modules (loaded via separate <script> tags below) are registered.
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', renderShell);
  } else {
    renderShell();
  }
})();
