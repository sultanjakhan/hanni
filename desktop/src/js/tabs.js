// ── js/tabs.js — Tab navigation, sub-sidebar, sub-tab bar, goals, dropdown, shortcuts, router ──

import { S, invoke, listen, TAB_REGISTRY, TAB_ICONS, TAB_SETTINGS_DEFS, saveTabs, tabLoaders, loadTabSetting, saveTabSetting, IS_MOBILE } from './state.js';
import { escapeHtml, confirmModal, renderTabSettingsPage, setupPageHeaderControls, renderPageHeader } from './utils.js';

// ── Settings sections per tab ──
const SETTINGS_SECTIONS = {
  chat: [
    { id: 'memory', label: 'Память' },
    { id: 'general', label: 'Автономный' },
    { id: 'voice', label: 'Голос' },
    { id: 'styles', label: 'Стили' },
    { id: 'tools', label: 'Инструменты' },
    { id: 'data', label: 'Данные' },
    { id: 'sync', label: 'Синхронизация' },
    { id: 'appearance', label: 'Оформление' },
    { id: 'about', label: 'О Hanni' },
  ],
  _default: [
    { id: 'general', label: 'Основные' },
    { id: 'mode', label: 'Режим работы' },
    { id: 'skills', label: 'Скиллы' },
    { id: 'integrations', label: 'Интеграции' },
    { id: 'blocklist', label: 'Блок-лист' },
    { id: 'mcp', label: 'MCP серверы' },
    { id: 'manage', label: 'Управление' },
  ],
};

// ── renderTabBar ──

function renderTabBar() {
  const tabList = document.getElementById('tab-list');
  tabList.innerHTML = '';

  const MOBILE_MAX_TABS = 5;
  const visibleTabs = IS_MOBILE ? S.openTabs.slice(0, MOBILE_MAX_TABS) : S.openTabs;

  for (const tabId of visibleTabs) {
    const reg = TAB_REGISTRY[tabId];
    if (!reg) continue;
    const item = document.createElement('div');
    item.className = 'tab-item' + (tabId === S.activeTab ? ' active' : '');
    item.dataset.tabId = tabId;
    item.title = reg.label;
    const icon = S.tabCustomizations[tabId]?.icon || reg.icon || '';
    const isEmoji = icon && !icon.startsWith('<');
    item.innerHTML = `<span class="tab-item-icon${isEmoji ? ' tab-item-icon-emoji' : ''}">${icon}</span>`;
    if (tabId === 'focus' && S.focusWidgetActivity) {
      const dot = document.createElement('span');
      dot.className = 'tab-focus-dot';
      item.appendChild(dot);
    }
    item.addEventListener('click', (e) => {
      if (item.dataset.wasDragged) { delete item.dataset.wasDragged; return; }
      switchTab(tabId);
    });

    if (!IS_MOBILE) {
      // Drag-to-reorder (desktop only)
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

      // Context menu (desktop only)
      item.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        showTabContextMenu(tabId, e.clientX, e.clientY);
      });
    }

    tabList.appendChild(item);
  }

  // Mobile: "More" button to show all tabs
  if (IS_MOBILE) {
    const more = document.createElement('div');
    const isMoreActive = S.openTabs.indexOf(S.activeTab) >= MOBILE_MAX_TABS;
    more.className = 'tab-item' + (isMoreActive ? ' active' : '');
    more.title = 'Ещё';
    more.innerHTML = `<span class="tab-item-icon">${TAB_ICONS.more || '⋯'}</span>`;
    more.addEventListener('click', () => showMobileTabPicker());
    tabList.appendChild(more);
  }

  // Bottom area: settings gear (desktop only, hidden on mobile via CSS)
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
      if (S.activeTab === 'chat' && S.activeSubTab.chat !== 'Настройки') {
        tabLoaders.loadConversationsList?.();
      }
    });
    bottom.appendChild(gear);
  }
}

// ── Mobile tab picker (full-screen grid of all tabs) ──

function showMobileTabPicker() {
  const dropdown = document.getElementById('tab-dropdown');
  const list = document.getElementById('tab-dropdown-list');
  list.innerHTML = '';
  for (const tabId of S.openTabs) {
    const reg = TAB_REGISTRY[tabId];
    if (!reg) continue;
    const icon = S.tabCustomizations[tabId]?.icon || reg.icon || '';
    const isEmoji = icon && !icon.startsWith('<');
    const item = document.createElement('div');
    item.className = 'tab-dropdown-item' + (tabId === S.activeTab ? ' active' : '');
    item.innerHTML = `<span class="tab-item-icon${isEmoji ? ' tab-item-icon-emoji' : ''}">${icon}</span> ${reg.label}`;
    item.addEventListener('click', () => {
      dropdown.classList.add('hidden');
      switchTab(tabId);
    });
    list.appendChild(item);
  }
  dropdown.classList.remove('hidden');
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

  // ── Settings mode: render settings in sub-sidebar ──
  const isSettingsMode = (S.activeTab === 'chat' && S.activeSubTab.chat === 'Настройки') ||
                         (S.activeTab !== 'chat' && S.activeSubTab[S.activeTab] === 'Настройки');

  if (isSettingsMode) {
    if (S.activeTab === 'chat') {
      // Chat: hide sidebar, use horizontal tabs in content (same as other tabs)
      sidebar.classList.remove('settings-mode');
      sidebar.classList.add('hidden');
      const convPanel = document.getElementById('conversations-panel');
      if (convPanel) convPanel.style.display = 'none';
      const goalsSection = document.getElementById('sub-sidebar-goals');
      if (goalsSection) goalsSection.classList.add('hidden');
    } else {
      // Non-chat: no sidebar sections, horizontal tabs are in content area
      sidebar.classList.remove('settings-mode');
      const sections = SETTINGS_SECTIONS[S.activeTab] || SETTINGS_SECTIONS._default;
      if (!S.settingsSection || !sections.find(s => s.id === S.settingsSection)) {
        S.settingsSection = sections[0]?.id || 'general';
      }
      // Keep sidebar visible with normal sub-tab nav if tab has subTabs
      const reg = TAB_REGISTRY[S.activeTab];
      if (reg?.subTabs?.length > 0) {
        sidebar.classList.remove('hidden', 'collapsed');
        items.innerHTML = '';
        const currentSub = S.activeSubTab[S.activeTab];
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
      } else {
        sidebar.classList.add('hidden');
      }
    }
    return;
  }

  sidebar.classList.remove('settings-mode');

  if (!reg || !reg.subTabs) {
    sidebar.classList.add('hidden');
    return;
  }

  // Chat tab: conversations live in the sub-sidebar
  if (S.activeTab === 'chat') {
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
  // Settings page — render in content area with sidebar nav
  if (subTab === 'Настройки') {
    if (tabId === 'chat') {
      showChatSettingsMode();
      tabLoaders.loadChatSettings?.();
    } else {
      const sec = S.settingsSection || 'general';
      renderSettingsPage(tabId, sec);
    }
    return;
  }

  // Exiting settings — remove overlay, restore original content
  const contentEl = document.getElementById(`${tabId}-content`);
  if (contentEl) {
    contentEl.querySelector('.settings-page')?.remove();
    Array.from(contentEl.children).forEach(c => c.style.display = '');
  }

  switch (tabId) {
    case 'chat':
      hideChatSettingsMode(); tabLoaders.focusInput?.();
      if (!S.chatSidebarCollapsed) tabLoaders.loadConversationsList?.();
      break;
    case 'calendar': tabLoaders.loadCalendar?.(subTab); break;
    case 'focus': tabLoaders.loadFocus?.(subTab); break;
    case 'notes': tabLoaders.loadNotes?.(subTab); break;
    case 'jobs': tabLoaders.loadJobs?.(subTab); break;
    case 'projects': tabLoaders.loadProjects?.(subTab); break;
    case 'development': tabLoaders.loadDevelopment?.(subTab); break;
    case 'home': tabLoaders.loadHome?.(subTab); break;
    case 'hobbies': tabLoaders.loadHobbies?.(subTab); break;
    case 'sports': tabLoaders.loadSports?.(subTab); break;
    case 'health': tabLoaders.loadHealth?.(subTab); break;
    case 'mindset': tabLoaders.loadMindset?.(subTab); break;
    case 'food': tabLoaders.loadFood?.(subTab); break;
    case 'money': tabLoaders.loadMoney?.(subTab); break;
    case 'people': tabLoaders.loadPeople?.(subTab); break;
    case 'schedule': tabLoaders.loadSchedule?.(subTab); break;
    case 'dankoe': tabLoaders.loadDanKoe?.(subTab); break;
    case 'timeline': tabLoaders.loadTimeline?.(subTab); break;
    default:
      if (tabId.startsWith('page_')) tabLoaders.loadCustomPage?.(tabId, subTab);
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

  // "New Project" option
  const newProjItem = document.createElement('div');
  newProjItem.className = 'tab-dropdown-item tab-dropdown-new-page';
  newProjItem.innerHTML = `<span class="tab-item-icon">📁</span> Новый проект`;
  newProjItem.addEventListener('click', async () => {
    dropdown.classList.add('hidden');
    try {
      const page = await invoke('create_custom_page', { pageType: 'project' });
      const tabId = `page_${page.id}`;
      TAB_REGISTRY[tabId] = {
        label: page.title,
        icon: page.icon,
        closable: true,
        subTabs: [],
        custom: true,
        pageId: page.id,
        pageType: 'project',
      };
      ensureViewDiv(tabId);
      openTab(tabId);
    } catch (err) { console.error('Project create error:', err); }
  });
  list.appendChild(newProjItem);

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

// ── Settings page renderer (full page in content area) ──

async function renderSettingsPage(tabId, sectionId) {
  const el = document.getElementById(`${tabId}-content`);
  if (!el) return;

  const reg = TAB_REGISTRY[tabId];
  const sections = SETTINGS_SECTIONS[tabId] || SETTINGS_SECTIONS._default;
  const sec = sections.find(s => s.id === sectionId) || sections[0];
  if (!sec) return;

  // For chat — use existing panel system, just switch active panel
  if (tabId === 'chat') {
    document.querySelectorAll('.chat-settings-panel').forEach(p => p.classList.remove('active'));
    document.getElementById(`cs-panel-${sectionId}`)?.classList.add('active');
    document.querySelectorAll('.chat-settings-tab').forEach(t => t.classList.remove('active'));
    document.querySelector(`.chat-settings-tab[data-panel="${sectionId}"]`)?.classList.add('active');
    return;
  }

  // For non-chat — render settings with horizontal tabs in content area
  const tabLabel = S.tabCustomizations[tabId]?.label || reg?.label || tabId;

  // Hide original content, show settings overlay
  Array.from(el.children).forEach(c => {
    if (!c.classList.contains('settings-page')) c.style.display = 'none';
  });
  el.querySelector('.settings-page')?.remove();

  // Build horizontal tabs
  const tabsHtml = sections.map(s =>
    `<button class="tab-settings-tab${s.id === sec.id ? ' active' : ''}" data-section="${s.id}">${s.label}</button>`
  ).join('');

  // Build section content
  const contentHtml = await renderSettingsSectionContent(tabId, sec.id);

  el.insertAdjacentHTML('beforeend', `<div class="settings-page">
    <div class="settings-page-header">
      <span class="settings-page-icon">${TAB_ICONS.settings}</span>
      <span class="settings-page-title">Настройки — ${tabLabel}</span>
    </div>
    <div class="tab-settings-tabs">${tabsHtml}</div>
    <div class="settings-page-content">${contentHtml}</div>
  </div>`);

  // Wire horizontal tab clicks
  el.querySelectorAll('.tab-settings-tab').forEach(btn => {
    btn.addEventListener('click', () => {
      S.settingsSection = btn.dataset.section;
      renderSubSidebar();
      renderSettingsPage(tabId, btn.dataset.section);
    });
  });

  // Wire up controls
  wireSettingsControls(el, tabId);
  wireSyncControls(el);
  wireManageControls(el, tabId);
}

async function renderSettingsSectionContent(tabId, sectionId) {
  if (sectionId === 'general') {
    const defs = TAB_SETTINGS_DEFS[tabId] || [];
    if (defs.length) {
      let rowsHtml = '';
      for (const def of defs) {
        const val = await loadTabSetting(tabId, def.key) ?? def.default;
        let controlHtml = '';
        if (def.type === 'toggle') {
          controlHtml = `<label class="toggle"><input type="checkbox" data-tab-id="${tabId}" data-setting-key="${def.key}" ${val === 'true' ? 'checked' : ''}><span class="toggle-track"></span></label>`;
        } else if (def.type === 'select') {
          controlHtml = `<div class="setting-pills" data-tab-id="${tabId}" data-setting-key="${def.key}">` +
            def.options.map(o => `<button class="setting-pill${val === o.value ? ' active' : ''}" data-value="${o.value}">${o.label}</button>`).join('') + `</div>`;
        } else if (def.type === 'number') {
          controlHtml = `<input class="form-input" type="number" min="${def.min || 1}" max="${def.max || 480}" data-tab-id="${tabId}" data-setting-key="${def.key}" value="${escapeHtml(val)}" style="width:100px;">`;
        } else {
          controlHtml = `<input class="form-input" type="text" data-tab-id="${tabId}" data-setting-key="${def.key}" value="${escapeHtml(val || '')}">`;
        }
        rowsHtml += `<div class="settings-row"><span class="settings-label">${def.label}</span><span class="settings-value">${controlHtml}</span></div>`;
      }
      return `<div class="settings-section"><div class="settings-section-title">Основные</div>${rowsHtml}</div>`;
    }
    return `<div class="settings-section"><div class="settings-section-title">Основные</div>
      <div class="settings-empty-hint">Нет настроек для этой секции</div></div>`;
  } else if (sectionId === 'mode') {
    return `<div class="settings-section"><div class="settings-section-title">Режим работы</div>
      <div class="settings-row"><span class="settings-label">Автопилот</span><span class="settings-value"><label class="toggle"><input type="checkbox" id="setting-autopilot"><span class="toggle-track"></span></label></span></div>
      <div class="settings-row"><span class="settings-hint">Ханни автоматически ищет, заполняет и обновляет данные</span></div>
    </div>`;
  } else if (sectionId === 'skills') {
    return `<div class="settings-section"><div class="settings-section-title">Скиллы (роль Ханни)</div>
      <div class="settings-empty-hint">Скиллы пока не настроены</div>
      <button class="btn-smallall" style="margin-top:12px;">+ Добавить скилл</button></div>`;
  } else if (sectionId === 'integrations') {
    return `<div class="settings-section"><div class="settings-section-title">Интеграции</div>
      <div class="settings-empty-hint">Нет подключённых интеграций</div>
      <button class="btn-smallall" style="margin-top:12px;">+ Подключить</button></div>`;
  } else if (sectionId === 'blocklist') {
    return `<div class="settings-section"><div class="settings-section-title">Блок-лист</div>
      <div class="settings-empty-hint">Блок-лист пуст</div>
      <button class="btn-smallall" style="margin-top:12px;">+ Добавить в блок-лист</button></div>`;
  } else if (sectionId === 'mcp') {
    return `<div class="settings-section"><div class="settings-section-title">MCP серверы</div>
      <div class="settings-row"><span class="settings-hint">Подключение внешних инструментов через MCP</span></div>
      <div class="settings-empty-hint">Нет подключённых MCP серверов</div>
      <button class="btn-smallall" style="margin-top:12px;">+ Подключить MCP</button></div>`;
  } else if (sectionId === 'sync') {
    return renderSyncSection();
  } else if (sectionId === 'manage') {
    return renderManageSection(tabId);
  }
  return '';
}

function wireSettingsControls(el, tabId) {
  el.querySelectorAll('input[data-setting-key], select[data-setting-key]').forEach(ctrl => {
    ctrl.addEventListener('change', () => {
      const v = ctrl.type === 'checkbox' ? ctrl.checked : ctrl.value;
      saveTabSetting(ctrl.dataset.tabId, ctrl.dataset.settingKey, v);
    });
  });
  el.querySelectorAll('.setting-pills').forEach(group => {
    group.querySelectorAll('.setting-pill').forEach(pill => {
      pill.addEventListener('click', () => {
        group.querySelectorAll('.setting-pill').forEach(p => p.classList.remove('active'));
        pill.classList.add('active');
        saveTabSetting(group.dataset.tabId, group.dataset.settingKey, pill.dataset.value);
      });
    });
  });
}

// ── Sync Settings Section ──

async function renderSyncSection() {
  let status = { enabled: false, last_sync: null, pending_changes: 0, site_id: '', device_name: '' };
  try { status = await invoke('get_sync_status'); } catch(_) {}
  return `
    <div class="settings-section">
      <div class="settings-section-title">Синхронизация</div>
      <div class="settings-row"><span class="settings-label">Включить синхронизацию</span>
        <span class="settings-value"><label class="toggle"><input type="checkbox" id="sync-enabled" ${status.enabled ? 'checked' : ''}><span class="toggle-track"></span></label></span></div>
      <div class="settings-row"><span class="settings-label">Relay URL</span>
        <span class="settings-value"><input class="form-input" id="sync-relay-url" type="text" placeholder="https://hanni-sync.workers.dev" style="width:260px;"></span></div>
      <div class="settings-row"><span class="settings-label">Device Token</span>
        <span class="settings-value"><input class="form-input" id="sync-device-token" type="password" placeholder="secret" style="width:200px;"></span></div>
      <div class="settings-row"><span class="settings-label">Имя устройства</span>
        <span class="settings-value"><input class="form-input" id="sync-device-name" type="text" value="${escapeHtml(status.device_name)}" placeholder="MacBook" style="width:200px;"></span></div>
      <div class="settings-row"><span class="settings-label">Device ID</span>
        <span class="settings-value"><span class="settings-hint">${status.site_id || '—'}</span></span></div>
      <div class="settings-row"><span class="settings-label">Последняя синхронизация</span>
        <span class="settings-value"><span class="settings-hint">${status.last_sync || 'никогда'}</span></span></div>
      <div class="settings-row"><span class="settings-label">Ожидающие изменения</span>
        <span class="settings-value"><span class="settings-hint">${status.pending_changes}</span></span></div>
      <div class="settings-row" style="gap:var(--space-2);justify-content:flex-end;">
        <button class="btn-smallall" id="sync-save-btn">Сохранить</button>
        <button class="btn-primary" id="sync-now-btn">Синхронизировать</button>
      </div>
    </div>`;
}

function wireSyncControls(el) {
  el.querySelector('#sync-save-btn')?.addEventListener('click', async () => {
    const enabled = el.querySelector('#sync-enabled')?.checked || false;
    const relayUrl = el.querySelector('#sync-relay-url')?.value || '';
    const deviceToken = el.querySelector('#sync-device-token')?.value || '';
    const deviceName = el.querySelector('#sync-device-name')?.value || '';
    try {
      await invoke('set_sync_config', { enabled, relayUrl, deviceToken, deviceName });
    } catch(e) { console.error('sync config save:', e); }
  });
  el.querySelector('#sync-now-btn')?.addEventListener('click', async () => {
    const btn = el.querySelector('#sync-now-btn');
    btn.textContent = 'Синхронизация…';
    btn.disabled = true;
    try {
      const result = await invoke('sync_now');
      btn.textContent = 'Готово!';
      setTimeout(() => { btn.textContent = 'Синхронизировать'; btn.disabled = false; }, 2000);
    } catch(e) {
      btn.textContent = 'Ошибка';
      console.error('sync:', e);
      setTimeout(() => { btn.textContent = 'Синхронизировать'; btn.disabled = false; }, 3000);
    }
  });
}

// ── Tab Management (Danger Zone) ──

function renderManageSection(tabId) {
  const reg = TAB_REGISTRY[tabId];
  const tabName = S.tabCustomizations[tabId]?.label || reg?.label || tabId;
  return `
    <div class="settings-section" style="border:1px solid var(--color-red-bg);">
      <div class="settings-section-title" style="color:var(--color-red);">Danger Zone</div>
      <div class="settings-row"><span class="settings-label">Удалить проект «${escapeHtml(tabName)}»</span>
        <span class="settings-value"><button class="btn-danger" id="manage-delete-btn">Удалить</button></span></div>
      <div class="settings-row"><span class="settings-hint">Проект будет закрыт и убран из панели. Для пользовательских страниц — данные удаляются навсегда.</span></div>
    </div>`;
}

function wireManageControls(el, tabId) {
  const deleteBtn = el.querySelector('#manage-delete-btn');
  if (!deleteBtn) return;
  deleteBtn.addEventListener('click', async () => {
    const reg = TAB_REGISTRY[tabId];
    const tabName = S.tabCustomizations[tabId]?.label || reg?.label || tabId;
    const ok = await confirmModal(`Удалить «${tabName}»?`, 'Удалить');
    if (!ok) return;
    // For custom pages — also delete from DB
    if (reg?.custom && reg?.pageId) {
      await invoke('delete_custom_page', { id: reg.pageId }).catch(() => {});
      delete TAB_REGISTRY[tabId];
    }
    closeTab(tabId);
    switchTab('chat');
  });
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
