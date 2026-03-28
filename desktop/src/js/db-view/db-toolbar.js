// ── db-view/db-toolbar.js — Toolbar: view switcher + actions (Notion-style) ──

const _s = (d) => `<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">${d}</svg>`;
const EYE_OFF = _s('<path d="M2 8s2.5-4 6-4 6 4 6 4-2.5 4-6 4-6-4-6-4z"/><circle cx="8" cy="8" r="2"/><line x1="2" y1="14" x2="14" y2="2"/>');
const EYE_ON = _s('<path d="M2 8s2.5-4 6-4 6 4 6 4-2.5 4-6 4-6-4-6-4z"/><circle cx="8" cy="8" r="2"/>');
const VIEW_ICONS = {
  table: _s('<rect x="1.5" y="1.5" width="13" height="13" rx="1.5"/><line x1="1.5" y1="5.5" x2="14.5" y2="5.5"/><line x1="1.5" y1="10" x2="14.5" y2="10"/><line x1="6" y1="5.5" x2="6" y2="14.5"/>'),
  kanban: _s('<rect x="1" y="1.5" width="4" height="13" rx="1"/><rect x="6" y="1.5" width="4" height="9" rx="1"/><rect x="11" y="1.5" width="4" height="11" rx="1"/>'),
  list: _s('<line x1="4.5" y1="3" x2="14" y2="3"/><line x1="4.5" y1="8" x2="14" y2="8"/><line x1="4.5" y1="13" x2="14" y2="13"/><circle cx="2" cy="3" r="0.8" fill="currentColor" stroke="none"/><circle cx="2" cy="8" r="0.8" fill="currentColor" stroke="none"/><circle cx="2" cy="13" r="0.8" fill="currentColor" stroke="none"/>'),
  gallery: _s('<rect x="1" y="1" width="6" height="6" rx="1.5"/><rect x="9" y="1" width="6" height="6" rx="1.5"/><rect x="1" y="9" width="6" height="6" rx="1.5"/><rect x="9" y="9" width="6" height="6" rx="1.5"/>'),
  timeline: _s('<line x1="1" y1="8" x2="15" y2="8"/><rect x="3" y="4" width="4" height="3" rx="1"/><rect x="8" y="9" width="5" height="3" rx="1"/>'),
  calendar: _s('<rect x="1.5" y="2.5" width="13" height="12" rx="1.5"/><path d="M1.5 6.5h13M5 1v3M11 1v3"/><circle cx="5" cy="10" r="1" fill="currentColor" stroke="none"/>'),
};
const VIEW_LABELS = { table: 'Таблица', kanban: 'Канбан', list: 'Список', gallery: 'Галерея', timeline: 'Таймлайн', calendar: 'Календарь' };
const ALL_VIEWS = ['table', 'kanban', 'list', 'gallery', 'timeline', 'calendar'];

export function renderToolbar(container, availableViews, activeView, onViewChange, actions = {}) {
  const toolbar = document.createElement('div');
  toolbar.className = 'dbv-toolbar';

  // ── View tabs (always visible) ──
  const viewBar = document.createElement('div');
  viewBar.className = 'dbv-view-bar';

  for (const vt of availableViews) {
    const btn = document.createElement('button');
    btn.className = 'dbv-view-btn' + (vt === activeView ? ' active' : '');
    btn.innerHTML = `<span class="dbv-view-icon">${VIEW_ICONS[vt] || ''}</span>${VIEW_LABELS[vt] || vt}`;
    if (vt !== 'table' && actions.onDeleteView) {
      const close = document.createElement('span');
      close.className = 'dbv-view-close';
      close.innerHTML = '×'; close.title = 'Убрать вид';
      close.addEventListener('click', (e) => { e.stopPropagation(); actions.onDeleteView(vt); });
      btn.appendChild(close);
    }
    btn.addEventListener('click', () => { if (vt !== activeView) onViewChange(vt); });
    viewBar.appendChild(btn);
  }

  // ── "+" button to add views ──
  const addable = ALL_VIEWS.filter(v => !availableViews.includes(v));
  if (addable.length > 0 && actions.onAddView) {
    const addBtn = document.createElement('button');
    addBtn.className = 'dbv-view-add';
    addBtn.innerHTML = '+'; addBtn.title = 'Добавить вид';
    addBtn.addEventListener('click', () => showAddViewDropdown(addBtn, addable, actions.onAddView));
    viewBar.appendChild(addBtn);
  }

  toolbar.appendChild(viewBar);

  // ── Actions bar (right side) ──
  const actionsBar = document.createElement('div');
  actionsBar.className = 'dbv-actions-bar';

  if (actions.onSearch) {
    const wrap = document.createElement('div'); wrap.className = 'dbv-search-wrap';
    wrap.innerHTML = `<button class="dbv-action-btn dbv-search-btn" title="Поиск">${_s('<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14"/>')}</button><input class="dbv-search-input" placeholder="Поиск..." style="display:none">`;
    const inp = wrap.querySelector('.dbv-search-input');
    wrap.querySelector('.dbv-search-btn').addEventListener('click', () => {
      inp.style.display = inp.style.display === 'none' ? '' : 'none';
      if (inp.style.display !== 'none') inp.focus(); else { inp.value = ''; actions.onSearch(''); }
    });
    inp.addEventListener('input', () => actions.onSearch(inp.value));
    inp.addEventListener('keydown', (e) => { if (e.key === 'Escape') { inp.style.display = 'none'; inp.value = ''; actions.onSearch(''); } e.stopPropagation(); });
    actionsBar.appendChild(wrap);
  }

  if (actions.onFilter) addActionBtn(actionsBar, _s('<path d="M2 4h12M4 8h8M6 12h4"/>') + 'Фильтр', () => actions.onFilter(actionsBar.lastChild));

  if (actions.onQuickFilter) {
    const qf = actions.quickFilter || null;
    const grp = document.createElement('div');
    grp.className = 'dbv-quick-filter-group';
    const mkBtn = (mode, label) => {
      const b = document.createElement('button');
      b.className = 'dbv-quick-filter-btn' + (qf === mode ? ' active' : '');
      b.textContent = label;
      b.addEventListener('click', () => actions.onQuickFilter(qf === mode ? null : mode));
      return b;
    };
    grp.appendChild(mkBtn('week', 'Н'));
    grp.appendChild(mkBtn('month', 'М'));
    actionsBar.appendChild(grp);
  }

  if (actions.onSort) addActionBtn(actionsBar, _s('<path d="M4 3v10M4 3L2 5M4 3l2 2M12 13V3M12 13l-2-2M12 13l2-2"/>') + 'Сорт', () => actions.onSort(actionsBar.lastChild));

  if (actions.hiddenColumns?.length > 0) {
    const hBtn = document.createElement('button'); hBtn.className = 'dbv-action-btn dbv-hidden-cols-btn';
    hBtn.innerHTML = `${_s('<path d="M2 8s2.5-4 6-4 6 4 6 4-2.5 4-6 4-6-4-6-4z"/><circle cx="8" cy="8" r="2"/><line x1="2" y1="14" x2="14" y2="2"/>')}<span class="dbv-hidden-count">${actions.hiddenColumns.length}</span>`;
    hBtn.title = 'Скрытые столбцы';
    hBtn.addEventListener('click', () => showHiddenColumnsMenu(hBtn, actions.hiddenColumns, actions.onShowColumn));
    actionsBar.appendChild(hBtn);
  }

  if (actions.onExport || actions.onImport) addActionBtn(actionsBar, '⋯', () => showMoreMenu(actionsBar.lastChild, actions), 'Ещё');


  toolbar.appendChild(actionsBar);
  container.prepend(toolbar);
  return toolbar;
}

function addActionBtn(bar, html, onClick, title) {
  const btn = document.createElement('button'); btn.className = 'dbv-action-btn'; btn.innerHTML = html;
  if (title) btn.title = title; btn.addEventListener('click', onClick); bar.appendChild(btn);
}

function autoClose(menu) {
  setTimeout(() => { const close = (e) => { if (!menu.contains(e.target)) { menu.remove(); document.removeEventListener('mousedown', close); } }; document.addEventListener('mousedown', close); }, 10);
}

function showAddViewDropdown(anchor, addable, onAddView) {
  document.querySelectorAll('.dbv-add-view-menu').forEach(m => m.remove());
  const rect = anchor.getBoundingClientRect();
  const menu = document.createElement('div');
  menu.className = 'inline-dropdown dbv-add-view-menu';
  menu.style.cssText = `top:${rect.bottom + 4}px;left:${rect.left}px`;
  menu.innerHTML = addable.map(v =>
    `<div class="inline-dd-option" data-view="${v}"><span class="dbv-view-icon">${VIEW_ICONS[v] || ''}</span>${VIEW_LABELS[v] || v}</div>`
  ).join('');
  document.body.appendChild(menu);
  menu.querySelectorAll('.inline-dd-option').forEach(item => {
    item.addEventListener('click', () => { menu.remove(); onAddView(item.dataset.view); });
  });
  autoClose(menu);
}

function showHiddenColumnsMenu(anchor, hiddenColumns, onShowColumn) {
  document.querySelectorAll('.dbv-hidden-cols-menu').forEach(m => m.remove());
  const rect = anchor.getBoundingClientRect(), menu = document.createElement('div');
  menu.className = 'inline-dropdown dbv-hidden-cols-menu';
  menu.style.cssText = `top:${rect.bottom + 4}px;right:${window.innerWidth - rect.right}px`;
  menu.innerHTML = `<div class="dbv-hidden-cols-title">Скрытые столбцы</div>` +
    hiddenColumns.map((col, i) => `<div class="inline-dd-option dbv-hidden-col-item" data-idx="${i}"><span class="dbv-hidden-col-eye">${EYE_OFF}</span><span class="dbv-hidden-col-name">${col.name}</span><span class="dbv-hidden-col-show">${EYE_ON}</span></div>`).join('') +
    (hiddenColumns.length > 1 ? `<div class="dbv-hidden-cols-divider"></div><div class="inline-dd-option dbv-hidden-col-all">Показать все</div>` : '');
  document.body.appendChild(menu);
  menu.querySelectorAll('.dbv-hidden-col-item').forEach(item => item.addEventListener('click', () => { onShowColumn?.(hiddenColumns[parseInt(item.dataset.idx)]); menu.remove(); }));
  menu.querySelector('.dbv-hidden-col-all')?.addEventListener('click', () => { hiddenColumns.forEach(col => onShowColumn?.(col)); menu.remove(); });
  autoClose(menu);
}

function showMoreMenu(anchor, actions) {
  document.querySelectorAll('.dbv-more-menu').forEach(m => m.remove());
  const rect = anchor.getBoundingClientRect();
  const menu = document.createElement('div');
  menu.className = 'inline-dropdown dbv-more-menu';
  menu.style.cssText = `top:${rect.bottom + 4}px;right:${window.innerWidth - rect.right}px`;
  const items = [];
  if (actions.onExport) items.push('<div class="inline-dd-option" data-action="export">⤓ Экспорт CSV</div>');
  if (actions.onImport) items.push('<div class="inline-dd-option" data-action="import">⤒ Импорт CSV</div>');
  items.push('<div class="inline-dd-option" data-action="copy-link">🔗 Копировать ссылку</div>');
  menu.innerHTML = items.join('');
  document.body.appendChild(menu);
  const handlers = { export: actions.onExport, import: actions.onImport, 'copy-link': () => navigator.clipboard.writeText(window.location.href).catch(() => {}) };
  menu.querySelectorAll('.inline-dd-option').forEach(el => el.addEventListener('click', () => { handlers[el.dataset.action]?.(); menu.remove(); }));
  autoClose(menu);
}
