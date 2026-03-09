// ── js/tabs.js — Tab navigation, sub-sidebar, sub-tab bar, goals, dropdown, shortcuts, router ──

import { S, invoke, listen, TAB_REGISTRY, TAB_ICONS, TAB_SETTINGS_DEFS, saveTabs, tabLoaders } from './state.js';
import { escapeHtml, renderTabSettingsPage, setupPageHeaderControls, renderPageHeader } from './utils.js';

// ── renderTabBar ──

function renderTabBar() {
  const tabList = document.getElementById('tab-list');
  tabList.innerHTML = '';
  for (const tabId of S.openTabs) {
    const reg = TAB_REGISTRY[tabId];
    if (!reg) continue;
    const item = document.createElement('div');
    item.className = 'tab-item' + (tabId === S.activeTab ? ' active' : '');
    item.dataset.tabId = tabId;
    item.title = reg.label;
    const customIcon = S.tabCustomizations[tabId]?.icon;
    item.innerHTML = customIcon
      ? `<span class="tab-item-icon tab-item-icon-emoji">${customIcon}</span>`
      : `<span class="tab-item-icon">${reg.icon || ''}</span>`;
    if (tabId === 'focus' && S.focusWidgetActivity) {
      const dot = document.createElement('span');
      dot.className = 'tab-focus-dot';
      item.appendChild(dot);
    }
    item.addEventListener('click', (e) => {
      if (item.dataset.wasDragged) { delete item.dataset.wasDragged; return; }
      switchTab(tabId);
    });

    // Drag-to-reorder (mouse events — reliable in WebKit/Tauri)
    item.addEventListener('mousedown', (e) => {
      if (e.button !== 0) return;
      const startY = e.clientY;
      let dragging = false;

      const onMove = (ev) => {
        if (!dragging && Math.abs(ev.clientY - startY) > 5) {
          dragging = true;
          item.classList.add('dragging');
          S.tabDragState = { tabId, el: item };
        }
        if (!dragging) return;
        // Find drop target
        tabList.querySelectorAll('.tab-item').forEach(el => {
          el.classList.remove('drag-over-above', 'drag-over-below');
          if (el === item) return;
          const rect = el.getBoundingClientRect();
          if (ev.clientY >= rect.top && ev.clientY <= rect.bottom) {
            const mid = rect.top + rect.height / 2;
            el.classList.toggle('drag-over-above', ev.clientY < mid);
            el.classList.toggle('drag-over-below', ev.clientY >= mid);
          }
        });
      };

      const onUp = (ev) => {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        tabList.querySelectorAll('.tab-item').forEach(el => el.classList.remove('drag-over-above', 'drag-over-below', 'dragging'));
        S.tabDragState = null;
        if (!dragging) return;
        item.dataset.wasDragged = '1';
        // Find target
        const target = [...tabList.querySelectorAll('.tab-item')].find(el => {
          if (el === item) return false;
          const rect = el.getBoundingClientRect();
          return ev.clientY >= rect.top && ev.clientY <= rect.bottom;
        });
        if (!target) return;
        const targetId = target.dataset.tabId;
        const fromIdx = S.openTabs.indexOf(tabId);
        if (fromIdx === -1) return;
        S.openTabs.splice(fromIdx, 1);
        const targetRect = target.getBoundingClientRect();
        let toIdx = S.openTabs.indexOf(targetId);
        if (ev.clientY >= targetRect.top + targetRect.height / 2) toIdx++;
        S.openTabs.splice(toIdx, 0, tabId);
        saveTabs();
        renderTabBar();
      };

      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });

    // Context menu (right click)
    item.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      showTabContextMenu(tabId, e.clientX, e.clientY);
    });

    tabList.appendChild(item);
  }

  // Bottom area: settings gear (context-aware)
  const bottom = document.getElementById('tab-bar-bottom');
  if (bottom) {
    bottom.innerHTML = '';
    const gear = document.createElement('div');
    const isOnSettings = S.activeSubTab[S.activeTab] === 'Настройки' || (S.activeTab === 'chat' && S.activeSubTab.chat === 'Настройки');
    gear.className = 'tab-item' + (isOnSettings ? ' active' : '');
    gear.title = 'Настройки';
    gear.innerHTML = `<span class="tab-item-icon">${TAB_ICONS.settings}</span>`;
    gear.addEventListener('click', () => {
      if (S.activeTab === 'chat') {
        if (S.activeSubTab.chat === 'Настройки') {
          S.activeSubTab.chat = null;
        } else {
          S.activeSubTab.chat = 'Настройки';
        }
      } else {
        if (!TAB_SETTINGS_DEFS[S.activeTab]?.length) return; // no settings for this tab
        if (S.activeSubTab[S.activeTab] === 'Настройки') {
          S.activeSubTab[S.activeTab] = TAB_REGISTRY[S.activeTab]?.subTabs?.[0] || null;
        } else {
          S.activeSubTab[S.activeTab] = 'Настройки';
        }
      }
      saveTabs();
      loadSubTabContent(S.activeTab, S.activeSubTab[S.activeTab] ?? (S.activeTab === 'chat' ? S.activeSubTab.chat : null));
      renderTabBar();
      renderSubSidebar();
    });
    bottom.appendChild(gear);
  }
}

// ── showTabContextMenu ──

function showTabContextMenu(tabId, x, y) {
  let menu = document.getElementById('tab-context-menu');
  if (menu) menu.remove();
  menu = document.createElement('div');
  menu.id = 'tab-context-menu';

  const reg = TAB_REGISTRY[tabId];
  const idx = S.openTabs.indexOf(tabId);
  const items = [];

  // Close
  if (reg?.closable) {
    items.push({ label: 'Закрыть', action: () => closeTab(tabId), cls: 'danger' });
  }
  // Close others
  const closableOthers = S.openTabs.filter(id => id !== tabId && TAB_REGISTRY[id]?.closable);
  if (closableOthers.length) {
    items.push({ label: 'Закрыть другие', action: () => { closableOthers.forEach(id => closeTab(id)); } });
  }
  if (items.length && (idx > 0 || idx < S.openTabs.length - 1)) {
    items.push({ separator: true });
  }
  // Move up / down
  if (idx > 0) {
    items.push({ label: 'Переместить вверх', action: () => moveTab(tabId, -1) });
  }
  if (idx < S.openTabs.length - 1) {
    items.push({ label: 'Переместить вниз', action: () => moveTab(tabId, 1) });
  }

  if (!items.length) return;

  for (const it of items) {
    if (it.separator) {
      const sep = document.createElement('div');
      sep.className = 'tab-ctx-separator';
      menu.appendChild(sep);
    } else {
      const el = document.createElement('div');
      el.className = 'tab-ctx-item' + (it.cls ? ` ${it.cls}` : '');
      el.textContent = it.label;
      el.addEventListener('click', () => { menu.remove(); it.action(); });
      menu.appendChild(el);
    }
  }

  // Position: ensure within viewport
  document.body.appendChild(menu);
  const mr = menu.getBoundingClientRect();
  menu.style.left = Math.min(x, window.innerWidth - mr.width - 8) + 'px';
  menu.style.top = Math.min(y, window.innerHeight - mr.height - 8) + 'px';

  const closeMenu = (e) => { if (!menu.contains(e.target)) { menu.remove(); document.removeEventListener('click', closeMenu); } };
  setTimeout(() => document.addEventListener('click', closeMenu), 0);
}

// ── moveTab ──

function moveTab(tabId, direction) {
  const idx = S.openTabs.indexOf(tabId);
  if (idx === -1) return;
  const newIdx = idx + direction;
  if (newIdx < 0 || newIdx >= S.openTabs.length) return;
  S.openTabs.splice(idx, 1);
  S.openTabs.splice(newIdx, 0, tabId);
  saveTabs();
  renderTabBar();
}

// ── renderSubSidebar ──

function renderSubSidebar() {
  const sidebar = document.getElementById('sub-sidebar');
  const items = document.getElementById('sub-sidebar-items');
  const reg = TAB_REGISTRY[S.activeTab];
  document.title = `Hanni [${S.activeTab}] subs=${reg?.subTabs?.length || 0}`;

  if (!reg || !reg.subTabs) {
    sidebar.classList.add('hidden');
    return;
  }

  // Chat tab: conversations live in the sub-sidebar (hidden during settings)
  if (S.activeTab === 'chat' && S.activeSubTab.chat !== 'Настройки') {
    sidebar.classList.remove('hidden');
    sidebar.classList.toggle('collapsed', !!S.chatSidebarCollapsed);
    const convPanel = document.getElementById('conversations-panel');
    if (convPanel) convPanel.style.display = 'none';

    items.innerHTML = '';

    // Collapse toggle
    const toggleRow = document.createElement('div');
    toggleRow.className = 'sub-sidebar-toggle-row';
    toggleRow.innerHTML = S.chatSidebarCollapsed
      ? `<button class="sub-sidebar-collapse-btn" title="Развернуть">${TAB_ICONS.chat}</button>`
      : `<button id="new-chat-sidebar-btn" class="sub-sidebar-new-chat">+ Новый чат</button>
         <button class="sub-sidebar-collapse-btn" title="Свернуть">
           <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="11 17 6 12 11 7"/><polyline points="18 17 13 12 18 7"/></svg>
         </button>`;
    toggleRow.querySelector('.sub-sidebar-collapse-btn').addEventListener('click', () => {
      S.chatSidebarCollapsed = !S.chatSidebarCollapsed;
      localStorage.setItem('hanni_chat_sidebar_collapsed', S.chatSidebarCollapsed ? '1' : '');
      renderSubSidebar();
      if (!S.chatSidebarCollapsed) tabLoaders.loadConversationsList?.();
    });
    toggleRow.querySelector('#new-chat-sidebar-btn')?.addEventListener('click', () => {
      document.getElementById('new-chat-btn')?.click();
    });
    items.appendChild(toggleRow);

    if (!S.chatSidebarCollapsed) {
      // Search
      const searchBox = document.createElement('div');
      searchBox.className = 'sub-sidebar-search';
      searchBox.innerHTML = `<input class="form-input sub-sidebar-conv-search" placeholder="Поиск..." autocomplete="off">`;
      searchBox.querySelector('input').addEventListener('input', (e) => {
        clearTimeout(S.convSearchTimeout);
        S.convSearchTimeout = setTimeout(() => tabLoaders.loadConversationsList?.(e.target.value), 300);
      });
      items.appendChild(searchBox);

      // Conv list container
      const convListEl = document.createElement('div');
      convListEl.id = 'sidebar-conv-list';
      convListEl.className = 'sub-sidebar-conv-list';
      items.appendChild(convListEl);
    }
  } else if (reg.subTabs?.length > 0) {
    // Non-chat tabs with sub-tabs: show sub-sidebar with vertical navigation + goals
    sidebar.classList.remove('hidden');
    sidebar.classList.remove('collapsed');
    const convPanel = document.getElementById('conversations-panel');
    if (convPanel) convPanel.style.display = '';

    items.innerHTML = '';
    const currentSub = S.activeSubTab[S.activeTab] ?? reg.subTabs[0];
    for (const sub of reg.subTabs) {
      const item = document.createElement('div');
      item.className = 'sub-sidebar-item' + (sub === currentSub ? ' active' : '');
      item.innerHTML = `<span class="sub-sidebar-dot"></span>${escapeHtml(sub)}`;
      item.addEventListener('click', () => {
        S.activeSubTab[S.activeTab] = sub;
        saveTabs();
        renderSubSidebar();
        loadSubTabContent(S.activeTab, sub);
      });
      items.appendChild(item);
    }

    // Remove horizontal sub-tab bar if it exists
    const viewEl = document.getElementById(`view-${S.activeTab}`);
    viewEl?.querySelector('.sub-tab-bar')?.remove();
  } else {
    sidebar.classList.add('hidden');
    // Restore conversations panel visibility when leaving chat
    const convPanel = document.getElementById('conversations-panel');
    if (convPanel) convPanel.style.display = '';
  }

  // Bottom: version only (gear is in tab bar)
  const settingsBottom = document.getElementById('sub-sidebar-settings');
  if (settingsBottom) {
    settingsBottom.innerHTML = '';
    if (!(S.chatSidebarCollapsed && S.activeTab === 'chat')) {
      const ver = document.createElement('div');
      ver.className = 'version-label';
      ver.textContent = `v${S.APP_VERSION}`;
      settingsBottom.appendChild(ver);
    }
  }
  loadGoalsWidget();
}

// ── renderSubTabBar ──

function renderSubTabBar(tabId, reg) {
  if (!reg?.subTabs?.length) return;
  const viewEl = document.getElementById(`view-${tabId}`);
  if (!viewEl) return;

  // Remove existing sub-tab bar if any
  viewEl.querySelector('.sub-tab-bar')?.remove();

  const currentSub = S.activeSubTab[tabId] ?? reg.subTabs[0];
  const bar = document.createElement('div');
  bar.className = 'sub-tab-bar';
  for (const sub of reg.subTabs) {
    const pill = document.createElement('button');
    pill.className = 'sub-tab-pill' + (sub === currentSub ? ' active' : '');
    pill.textContent = sub;
    pill.addEventListener('click', () => {
      S.activeSubTab[tabId] = sub;
      saveTabs();
      renderSubTabBar(tabId, reg);
      loadSubTabContent(tabId, sub);
    });
    bar.appendChild(pill);
  }
  viewEl.insertBefore(bar, viewEl.firstChild);
}

// ── loadGoalsWidget / showAddGoalModal ──

async function loadGoalsWidget() {
  const section = document.getElementById('sub-sidebar-goals');
  const goalsList = document.getElementById('goals-list');
  if (!section || !goalsList) return;

  // Hide goals for chat tab or when sub-sidebar is hidden
  if (S.activeTab === 'chat') {
    section.classList.add('hidden');
    return;
  }

  // Remove old inline goals from content area (cleanup from previous version)
  const contentEl = document.getElementById(`${S.activeTab}-content`);
  if (contentEl) {
    const old = contentEl.querySelector('.goals-inline');
    if (old) old.remove();
  }

  try {
    const goals = await invoke('get_goals', { tabName: S.activeTab });
    section.classList.remove('hidden');

    goalsList.innerHTML = `
      ${goals.length > 0 ? goals.map(g => {
        const pct = g.target_value > 0 ? Math.min(100, Math.round(g.current_value / g.target_value * 100)) : 0;
        return `<div class="goal-item">
          <div class="goal-inline-info"><span>${escapeHtml(g.title)}</span><span class="goal-inline-pct">${pct}%</span></div>
          <div class="goal-progress"><div class="goal-progress-bar" style="width:${pct}%"></div></div>
        </div>`;
      }).join('') : '<div class="goal-item" style="color:var(--text-faint);">No goals yet</div>'}
      <button class="btn-smallall" id="add-goal-btn" style="margin:6px 16px;">+ Goal</button>`;

    // Toggle collapse
    const toggle = document.getElementById('goals-toggle');
    if (toggle) {
      toggle.onclick = () => goalsList.classList.toggle('hidden');
      // Show count
      toggle.textContent = `Goals${goals.length > 0 ? ` (${goals.length})` : ''}`;
    }

    // Default: expanded
    goalsList.classList.remove('hidden');

    goalsList.querySelector('#add-goal-btn')?.addEventListener('click', () => showAddGoalModal());
  } catch (_) {
    section.classList.add('hidden');
  }
}

function showAddGoalModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">New Goal</div>
    <div class="form-row"><input class="form-input" id="goal-title" placeholder="Goal title"></div>
    <div class="form-row">
      <input class="form-input" id="goal-target" type="number" placeholder="Target" style="max-width:100px;">
      <input class="form-input" id="goal-unit" placeholder="Unit (e.g. km, books)" style="max-width:120px;">
      <input class="form-input" id="goal-deadline" type="date" style="max-width:150px;">
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="goal-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('goal-save')?.addEventListener('click', async () => {
    const title = document.getElementById('goal-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_goal', {
        tabName: S.activeTab,
        title,
        targetValue: parseInt(document.getElementById('goal-target')?.value || '1') || 1,
        currentValue: 0,
        unit: document.getElementById('goal-unit')?.value || '',
        deadline: document.getElementById('goal-deadline')?.value || null,
      });
      overlay.remove();
      loadGoalsWidget();
    } catch (err) { alert('Error: ' + err); }
  });
}

// ── openTab / closeTab / switchTab / ensureViewDiv / activateView ──

function openTab(tabId) {
  if (!TAB_REGISTRY[tabId]) return;
  if (!S.openTabs.includes(tabId)) {
    const idx = S.openTabs.indexOf(S.activeTab);
    S.openTabs.splice(idx + 1, 0, tabId);
  }
  switchTab(tabId);
}

function closeTab(tabId) {
  if (!TAB_REGISTRY[tabId]?.closable) return;
  const idx = S.openTabs.indexOf(tabId);
  if (idx === -1) return;
  S.openTabs.splice(idx, 1);
  if (S.activeTab === tabId) S.activeTab = S.openTabs[Math.min(idx, S.openTabs.length - 1)] || 'chat';
  saveTabs();
  renderTabBar();
  activateView();
}

function switchTab(tabId) {
  if (!TAB_REGISTRY[tabId]) return;
  if (!S.openTabs.includes(tabId)) {
    const idx = S.openTabs.indexOf(S.activeTab);
    S.openTabs.splice(idx + 1, 0, tabId);
  }
  S.activeTab = tabId;
  saveTabs();
  renderTabBar();
  activateView();
  tabLoaders.updateFocusWidgetVisibility?.();
  tabLoaders.updateChatOverlayVisibility?.();
}

function ensureViewDiv(tabId) {
  let view = document.getElementById(`view-${tabId}`);
  if (!view) {
    view = document.createElement('div');
    view.id = `view-${tabId}`;
    view.className = 'view';
    view.innerHTML = `<div id="${tabId}-content" class="tab-content"></div>`;
    document.getElementById('content-area').appendChild(view);
  }
  return view;
}

function activateView() {
  document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
  // Ensure view div exists for custom pages
  if (S.activeTab.startsWith('page_')) ensureViewDiv(S.activeTab);
  const view = document.getElementById(`view-${S.activeTab}`);
  if (view) view.classList.add('active');
  renderSubSidebar();
  const reg = TAB_REGISTRY[S.activeTab];
  const sub = reg?.subTabs ? (S.activeTab === 'chat' ? S.activeSubTab[S.activeTab] : (S.activeSubTab[S.activeTab] ?? reg.subTabs[0])) : null;
  loadSubTabContent(S.activeTab, sub);
}

// ── loadSubTabContent (router) ──

function loadSubTabContent(tabId, subTab) {
  // Per-tab settings page (gear button)
  if (subTab === 'Настройки' && tabId !== 'chat') {
    renderTabSettingsPage(tabId);
    return;
  }
  switch (tabId) {
    case 'chat':
      if (subTab === 'Настройки') { showChatSettingsMode(); tabLoaders.loadChatSettings?.(); }
      else { hideChatSettingsMode(); renderSubSidebar(); tabLoaders.loadConversationsList?.(); tabLoaders.focusInput?.(); }
      break;
    case 'calendar': tabLoaders.loadCalendar?.(subTab); break;
    case 'focus': tabLoaders.loadFocus?.(subTab); break;
    case 'notes': tabLoaders.loadNotes?.(subTab); break;
    case 'work': tabLoaders.loadWork?.(subTab); break;
    case 'development': tabLoaders.loadDevelopment?.(subTab); break;
    case 'home': tabLoaders.loadHome?.(subTab); break;
    case 'hobbies': tabLoaders.loadHobbies?.(subTab); break;
    case 'sports': tabLoaders.loadSports?.(subTab); break;
    case 'health': tabLoaders.loadHealth?.(subTab); break;
    case 'mindset': tabLoaders.loadMindset?.(subTab); break;
    case 'food': tabLoaders.loadFood?.(subTab); break;
    case 'money': tabLoaders.loadMoney?.(subTab); break;
    case 'people': tabLoaders.loadPeople?.(subTab); break;
    default:
      if (tabId.startsWith('page_')) tabLoaders.loadCustomPage?.(tabId);
      break;
  }
  // Wire up editable page header controls (icon picker, description) after content renders
  // Notes handles its own via setupNotesControls()
  if (tabId !== 'chat' && tabId !== 'notes') setTimeout(() => setupPageHeaderControls(tabId), 50);
}

// ── Tab add dropdown handler ──

document.getElementById('tab-add')?.addEventListener('click', (e) => {
  e.stopPropagation();
  const dropdown = document.getElementById('tab-dropdown');
  const list = document.getElementById('tab-dropdown-list');
  const btn = document.getElementById('tab-add');
  list.innerHTML = '';

  // "New Page" option — always first
  const newPageItem = document.createElement('div');
  newPageItem.className = 'tab-dropdown-item tab-dropdown-new-page';
  newPageItem.innerHTML = `<span class="tab-item-icon">➕</span> Новая страница`;
  newPageItem.addEventListener('click', async () => {
    dropdown.classList.add('hidden');
    try {
      const page = await invoke('create_custom_page');
      const tabId = `page_${page.id}`;
      TAB_REGISTRY[tabId] = {
        label: page.title,
        icon: page.icon,
        closable: true,
        subTabs: [],
        custom: true,
        pageId: page.id,
      };
      ensureViewDiv(tabId);
      openTab(tabId);
    } catch (err) { console.error('Page create error:', err); }
  });
  list.appendChild(newPageItem);

  // Separator
  const sep = document.createElement('div');
  sep.className = 'tab-dropdown-separator';
  list.appendChild(sep);

  // Existing closed tabs
  for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
    if (S.openTabs.includes(id)) continue;
    const item = document.createElement('div');
    item.className = 'tab-dropdown-item';
    item.innerHTML = `<span class="tab-item-icon">${reg.icon || ''}</span> ${reg.label}`;
    item.addEventListener('click', () => { dropdown.classList.add('hidden'); openTab(id); });
    list.appendChild(item);
  }
  // Position dropdown near + button (vertical tab bar)
  const rect = btn.getBoundingClientRect();
  dropdown.style.left = (rect.right + 4) + 'px';
  dropdown.style.top = Math.max(8, rect.top) + 'px';
  dropdown.classList.toggle('hidden');
});

document.addEventListener('click', () => {
  document.getElementById('tab-dropdown')?.classList.add('hidden');
});

// ── Keyboard shortcuts ──

document.addEventListener('keydown', (e) => {
  if (!e.metaKey && !e.ctrlKey) return;
  if (e.shiftKey && (e.key === 'f' || e.key === 'F')) { e.preventDefault(); tabLoaders.toggleFocusWidgetPopover?.(); return; }
  if (e.shiftKey && (e.key === 'c' || e.key === 'C' || e.key === 'с' || e.key === 'С')) { e.preventDefault(); tabLoaders.toggleChatOverlay?.(); return; }
  if (e.key === 'w') { e.preventDefault(); if (TAB_REGISTRY[S.activeTab]?.closable) closeTab(S.activeTab); return; }
  if (e.key === 't') { e.preventDefault(); document.getElementById('tab-add')?.click(); return; }
  const num = parseInt(e.key);
  if (num >= 1 && num <= 9 && num <= S.openTabs.length) { e.preventDefault(); switchTab(S.openTabs[num - 1]); }
});

// ── Chat settings mode ──

function showChatSettingsMode() {
  const view = document.getElementById('view-chat');
  view.classList.add('chat-settings-mode');
}

function hideChatSettingsMode() {
  const view = document.getElementById('view-chat');
  view.classList.remove('chat-settings-mode');
}

// ── Exports ──

export {
  renderTabBar,
  renderSubSidebar,
  openTab,
  closeTab,
  switchTab,
  activateView,
  ensureViewDiv,
  loadSubTabContent,
  showChatSettingsMode,
  hideChatSettingsMode,
  renderSubTabBar,
  loadGoalsWidget,
};
