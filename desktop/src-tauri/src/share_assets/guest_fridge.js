// guest_fridge.js — Thin guest wrapper around fridge-shared.js (view-only).
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;
  const { api } = u;

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.fridge = {
    mount(el) {
      if (!window.HanniFridge) { el.innerHTML = '<div class="err">fridge-shared.js не загружен</div>'; return; }
      window.HanniFridge.mountInventory({
        el,
        // View-only: no add/update/remove → fridge-shared hides those controls.
        backend: {
          list: async () => (await api('/fridge')).items || [],
        },
      });
    },
  };
})();
