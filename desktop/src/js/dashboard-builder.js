// dashboard-builder.js — Load widgets from DB, render dashboard, handle edit mode
import { invoke } from './state.js';
import { extractPath, renderStatWidget, renderInteractiveWidget, renderProgressWidget, renderListWidget, renderTextWidget } from './dashboard-widgets.js';

const RENDERERS = {
  stat: renderStatWidget,
  interactive: renderInteractiveWidget,
  progress: renderProgressWidget,
  list: renderListWidget,
  text: renderTextWidget,
};

export async function renderDashboard(paneEl, tabId) {
  let widgets = await invoke('get_dashboard_widgets', { tabId }).catch(() => []);
  if (!widgets.length) {
    await invoke('seed_dashboard_defaults', { tabId }).catch(() => {});
    widgets = await invoke('get_dashboard_widgets', { tabId }).catch(() => []);
  }
  if (!widgets.length) {
    paneEl.innerHTML = '';
    paneEl.appendChild(buildDashHeader(tabId));
    paneEl.insertAdjacentHTML('beforeend', '<div class="empty-state">Нет виджетов. Нажмите ••• чтобы добавить.</div>');
    return;
  }

  // Batch data fetches — group widgets by command+args
  const dataCache = {};
  const fetchKeys = [];
  for (const w of widgets) {
    const c = w.config;
    if (!c.command) continue;
    const key = c.command + '|' + JSON.stringify(c.commandArgs || {});
    if (!dataCache[key]) {
      dataCache[key] = null;
      fetchKeys.push({ key, command: c.command, args: c.commandArgs || {} });
    }
  }
  await Promise.all(fetchKeys.map(async ({ key, command, args }) => {
    try { dataCache[key] = await invoke(command, args); }
    catch { dataCache[key] = {}; }
  }));

  // Render grid
  const grid = document.createElement('div');
  grid.className = 'uni-dash-grid';
  const reload = () => renderDashboard(paneEl, tabId);

  for (const w of widgets) {
    const card = document.createElement('div');
    const c = w.config;
    const key = c.command ? c.command + '|' + JSON.stringify(c.commandArgs || {}) : null;
    const data = key ? dataCache[key] : null;
    const renderer = RENDERERS[w.widget_type];
    if (renderer) {
      if (w.widget_type === 'interactive') renderer(card, w, data, reload);
      else renderer(card, w, data);
    } else {
      card.className = 'uni-dash-card';
      card.textContent = `Unknown: ${w.widget_type}`;
    }
    card.dataset.widgetId = w.id;
    grid.appendChild(card);
  }

  paneEl.innerHTML = '';
  paneEl.appendChild(buildDashHeader(tabId));
  paneEl.appendChild(grid);
}

function buildDashHeader(tabId) {
  const row = document.createElement('div');
  row.className = 'dash-header-row';
  const btn = document.createElement('button');
  btn.className = 'dash-edit-toggle';
  btn.innerHTML = '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="3" cy="8" r="1.2"/><circle cx="8" cy="8" r="1.2"/><circle cx="13" cy="8" r="1.2"/></svg>';
  btn.title = 'Настроить дашборд';
  btn.addEventListener('click', async () => {
    const { enterEditMode } = await import('./dashboard-editor.js');
    enterEditMode(row.closest('.pane-dash, [class*="pane"]') || row.parentElement, tabId);
  });
  row.appendChild(btn);
  return row;
}
