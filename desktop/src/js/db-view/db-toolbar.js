// ── db-view/db-toolbar.js — Toolbar: view switcher, search ──

const VIEW_ICONS = {
  table: '\u2630',   // ☰
  kanban: '\u25a8',  // ▨
  list: '\u2261',    // ≡
  gallery: '\u25a3', // ▣
};

const VIEW_LABELS = {
  table: 'Таблица',
  kanban: 'Канбан',
  list: 'Список',
  gallery: 'Галерея',
};

/**
 * Render a toolbar with view switcher tabs.
 *
 * @param {HTMLElement} container - Parent container
 * @param {string[]} availableViews - e.g. ['table', 'kanban', 'gallery']
 * @param {string} activeView - Current active view
 * @param {function} onViewChange - Callback (viewType) => void
 * @returns {HTMLElement} The toolbar element
 */
export function renderToolbar(container, availableViews, activeView, onViewChange) {
  // Only show toolbar if more than 1 view available
  if (availableViews.length <= 1) return null;

  const toolbar = document.createElement('div');
  toolbar.className = 'dbv-toolbar';

  const viewBar = document.createElement('div');
  viewBar.className = 'dbv-view-bar';

  for (const vt of availableViews) {
    const btn = document.createElement('button');
    btn.className = 'dbv-view-btn' + (vt === activeView ? ' active' : '');
    btn.innerHTML = `<span class="dbv-view-icon">${VIEW_ICONS[vt] || ''}</span> ${VIEW_LABELS[vt] || vt}`;
    btn.addEventListener('click', () => {
      if (vt !== activeView) onViewChange(vt);
    });
    viewBar.appendChild(btn);
  }

  toolbar.appendChild(viewBar);
  container.prepend(toolbar);
  return toolbar;
}
