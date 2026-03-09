// ── db-view/db-toolbar.js — Toolbar: view switcher (Notion-style) ──

const VIEW_ICONS = {
  table: `<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1.5" y="1.5" width="13" height="13" rx="1.5"/><line x1="1.5" y1="5.5" x2="14.5" y2="5.5"/><line x1="1.5" y1="10" x2="14.5" y2="10"/><line x1="6" y1="5.5" x2="6" y2="14.5"/></svg>`,
  kanban: `<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1.5" width="4" height="13" rx="1"/><rect x="6" y="1.5" width="4" height="9" rx="1"/><rect x="11" y="1.5" width="4" height="11" rx="1"/></svg>`,
  list: `<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><line x1="4.5" y1="3" x2="14" y2="3"/><line x1="4.5" y1="8" x2="14" y2="8"/><line x1="4.5" y1="13" x2="14" y2="13"/><circle cx="2" cy="3" r="0.8" fill="currentColor" stroke="none"/><circle cx="2" cy="8" r="0.8" fill="currentColor" stroke="none"/><circle cx="2" cy="13" r="0.8" fill="currentColor" stroke="none"/></svg>`,
  gallery: `<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="6" height="6" rx="1.5"/><rect x="9" y="1" width="6" height="6" rx="1.5"/><rect x="1" y="9" width="6" height="6" rx="1.5"/><rect x="9" y="9" width="6" height="6" rx="1.5"/></svg>`,
};

const VIEW_LABELS = {
  table: 'Таблица',
  kanban: 'Канбан',
  list: 'Список',
  gallery: 'Галерея',
};

/**
 * Render a Notion-style view switcher toolbar.
 */
export function renderToolbar(container, availableViews, activeView, onViewChange) {
  if (availableViews.length <= 1) return null;

  const toolbar = document.createElement('div');
  toolbar.className = 'dbv-toolbar';

  const viewBar = document.createElement('div');
  viewBar.className = 'dbv-view-bar';

  for (const vt of availableViews) {
    const btn = document.createElement('button');
    btn.className = 'dbv-view-btn' + (vt === activeView ? ' active' : '');
    btn.innerHTML = `<span class="dbv-view-icon">${VIEW_ICONS[vt] || ''}</span>${VIEW_LABELS[vt] || vt}`;
    btn.addEventListener('click', () => {
      if (vt !== activeView) onViewChange(vt);
    });
    viewBar.appendChild(btn);
  }

  toolbar.appendChild(viewBar);
  container.prepend(toolbar);
  return toolbar;
}
