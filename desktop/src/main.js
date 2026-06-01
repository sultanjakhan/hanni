// ══════════════════════════════════════════════
//  Hanni — Entry Point (ES Module)
//  Imports all modules, registers tabLoaders, initializes the app.
// ══════════════════════════════════════════════

// ── Foundation ──
import { S, invoke, listen, chat, input, sendBtn, tabLoaders, TAB_REGISTRY, TAB_DESCRIPTIONS, setTheme, getTabIcon, saveTabCustom, IS_MOBILE } from './js/state.js';
import { renderPageHeader, setupPageHeaderControls, loadTabBlockEditor, escapeHtml } from './js/utils.js';

// ── Core modules ──
import { renderTabBar, renderSubSidebar, openTab, closeTab, switchTab, activateView, ensureViewDiv, loadSubTabContent, showChatSettingsMode, hideChatSettingsMode, closeDrawer, updateMobileTitle } from './js/tabs.js';
import { loadConversationsList, loadConversation, autoSaveConversation } from './js/conversations.js';
import {
  addMsg, scrollDown, send, newChat,
  renderChatWelcomeCard, removeChatWelcomeCard,
  loadChatSettings, addFeedbackButtons, addProactiveFeedbackButtons,
  showStub, streamChat, toggleTTS, stopAllTTS, showAgentIndicator,
  createChatOverlay, toggleChatOverlay, updateChatOverlayVisibility, refreshOverlayMessages,
} from './js/chat.js';
import { parseAndExecuteActions, executeAction } from './js/actions.js';
import { checkVoiceServer, startRecording, stopRecordingAndSend, cancelRecording, toggleCallMode, startCallMode, endCallMode, startWakeWordSSE, stopWakeWordSSE } from './js/voice.js';

// ── Tab modules ──
import { loadCalendar } from './js/tab-calendar.js';
import { loadFocus, createFocusWidget, updateFocusWidget, updateFocusWidgetVisibility, toggleFocusWidgetPopover, startPomodoro, bindFocusWidgetEvents } from './js/tab-focus.js';
import { initNotificationWidget } from './js/notification-widget.js';
import { initMusicWidget } from './js/music-widget.js';
import { initQuotesWidget } from './js/quotes-widget.js';
import { initTaskControlWidget } from './js/task-control-widget.js';
import { loadNotes, renderDatabaseView, renderNoteEditor, renderLinkedNotes, createAndOpenNote, createAndOpenTask } from './js/tab-notes.js';
import {
  loadHome, loadFood, loadMoney, loadPeople,
  loadMemoryTab, loadAbout, loadJobs, loadProjects, loadDevelopment,
  loadHobbies, loadSports, loadHealth, loadCustomPage,
  loadSchedule, loadDanKoe,
} from './js/tab-data.js';
import { autoImportHealth, startHealthPolling, maybeRequestHealthBackground } from './js/health-auto-sync.js';
import { checkAndroidUpdate, checkWebUpdate, confirmWebBoot, desktopWebOTA } from './js/android-update.js';

// ── One-time migration: work → jobs tab rename ──
(() => {
  ['dbv_view_', 'dbv_colwidths_', 'dbv_removed_', 'dbv_hidden_fixed_', 'dbv_deleted_fixed_', 'dbv_fixed_names_', 'dbv_col_order_', 'dbv_wrap_', 'dbv_frozen_'].forEach(prefix => {
    const old = localStorage.getItem(prefix + 'work');
    if (old && !localStorage.getItem(prefix + 'jobs')) {
      localStorage.setItem(prefix + 'jobs', old);
      localStorage.removeItem(prefix + 'work');
    }
  });
  try {
    const panes = JSON.parse(localStorage.getItem('hanni_panes') || '{}');
    if (panes.work && !panes.jobs) { panes.jobs = panes.work; delete panes.work; localStorage.setItem('hanni_panes', JSON.stringify(panes)); }
  } catch {}
  try {
    const tabs = JSON.parse(localStorage.getItem('hanni_tabs') || '{}');
    let changed = false;
    if (tabs.open?.includes('work')) { tabs.open = tabs.open.map(t => t === 'work' ? 'jobs' : t); changed = true; }
    if (tabs.active === 'work') { tabs.active = 'jobs'; changed = true; }
    if (tabs.sub?.work) { tabs.sub.jobs = tabs.sub.work; delete tabs.sub.work; changed = true; }
    if (changed) localStorage.setItem('hanni_tabs', JSON.stringify(tabs));
  } catch {}
})();

// ══════════════════════════════════════════════
//  Register tabLoaders (late-binding registry)
//  Modules use tabLoaders.fn() to call cross-module functions
//  without circular imports.
// ══════════════════════════════════════════════

// Tabs
tabLoaders.renderTabBar = renderTabBar;
tabLoaders._renderTabBar = renderTabBar; // used by utils.js setupPageHeaderControls
tabLoaders.renderSubSidebar = renderSubSidebar;
tabLoaders.switchTab = switchTab;
tabLoaders.openTab = openTab;
tabLoaders.closeTab = closeTab;
tabLoaders.activateView = activateView;
tabLoaders.ensureViewDiv = ensureViewDiv;
tabLoaders.closeDrawer = closeDrawer;
tabLoaders.updateMobileTitle = updateMobileTitle;
tabLoaders.loadSubTabContent = loadSubTabContent;

// Chat
tabLoaders.addMsg = addMsg;
tabLoaders.scrollDown = scrollDown;
tabLoaders.send = send;
tabLoaders.newChat = newChat;
tabLoaders.renderChatWelcomeCard = renderChatWelcomeCard;
tabLoaders.removeChatWelcomeCard = removeChatWelcomeCard;
tabLoaders.loadChatSettings = loadChatSettings;
tabLoaders.addFeedbackButtons = addFeedbackButtons;
tabLoaders.addProactiveFeedbackButtons = addProactiveFeedbackButtons;
tabLoaders.showStub = showStub;
tabLoaders.streamChat = streamChat;
tabLoaders.toggleTTS = toggleTTS;
tabLoaders.stopAllTTS = stopAllTTS;
tabLoaders.showAgentIndicator = showAgentIndicator;
tabLoaders.showChatSettingsMode = showChatSettingsMode;
tabLoaders.hideChatSettingsMode = hideChatSettingsMode;
tabLoaders.focusInput = () => input.focus();
tabLoaders.setTheme = setTheme;

// Conversations
tabLoaders.loadConversationsList = loadConversationsList;
tabLoaders.loadConversation = loadConversation;
tabLoaders.autoSaveConversation = autoSaveConversation;

// Actions
tabLoaders.parseAndExecuteActions = parseAndExecuteActions;
tabLoaders.executeAction = executeAction;

// Voice
tabLoaders.toggleCallMode = toggleCallMode;
tabLoaders.startCallMode = startCallMode;
tabLoaders.endCallMode = endCallMode;
tabLoaders.startWakeWordSSE = startWakeWordSSE;
tabLoaders.stopWakeWordSSE = stopWakeWordSSE;

// Tab loaders (for the router in tabs.js loadSubTabContent)
tabLoaders.loadCalendar = loadCalendar;
tabLoaders.loadFocus = loadFocus;
tabLoaders.loadNotes = loadNotes;
tabLoaders.loadJobs = loadJobs;
tabLoaders.loadProjects = loadProjects;
tabLoaders.loadDevelopment = loadDevelopment;
tabLoaders.loadHome = loadHome;
tabLoaders.loadHobbies = loadHobbies;
tabLoaders.loadSports = loadSports;
tabLoaders.loadHealth = loadHealth;
tabLoaders.loadFood = loadFood;
tabLoaders.loadMoney = loadMoney;
tabLoaders.loadPeople = loadPeople;
tabLoaders.loadSchedule = loadSchedule;
tabLoaders.loadDanKoe = loadDanKoe;
tabLoaders.loadCustomPage = loadCustomPage;

// Focus widget
tabLoaders.createFocusWidget = createFocusWidget;
tabLoaders.updateFocusWidget = updateFocusWidget;
tabLoaders.updateFocusWidgetVisibility = updateFocusWidgetVisibility;
tabLoaders.toggleFocusWidgetPopover = toggleFocusWidgetPopover;
tabLoaders.startPomodoro = startPomodoro;

// Chat overlay
tabLoaders.createChatOverlay = createChatOverlay;
tabLoaders.toggleChatOverlay = toggleChatOverlay;
tabLoaders.updateChatOverlayVisibility = updateChatOverlayVisibility;
tabLoaders.refreshOverlayMessages = refreshOverlayMessages;

// Block editor
tabLoaders.loadTabBlockEditor = loadTabBlockEditor;

// Notes
tabLoaders.renderDatabaseView = renderDatabaseView;
tabLoaders.renderNoteEditor = renderNoteEditor;
tabLoaders.renderLinkedNotes = renderLinkedNotes;
tabLoaders.createAndOpenNote = createAndOpenNote;
tabLoaders.createAndOpenTask = createAndOpenTask;
tabLoaders.loadNotes = loadNotes;

// ══════════════════════════════════════════════
//  Window globals (for onclick in HTML templates)
// ══════════════════════════════════════════════

window.switchTab = switchTab;

// ══════════════════════════════════════════════
//  Cmd+K Command Palette
// ══════════════════════════════════════════════

function showCommandPalette() {
  if (document.getElementById('cmd-palette')) return;
  const overlay = document.createElement('div');
  overlay.id = 'cmd-palette';
  overlay.className = 'cmd-palette-overlay';
  overlay.innerHTML = `
    <div class="cmd-palette">
      <input class="cmd-palette-input" placeholder="Поиск по вкладкам..." autofocus>
      <div class="cmd-palette-results"></div>
    </div>`;
  document.body.appendChild(overlay);

  const inp = overlay.querySelector('.cmd-palette-input');
  const results = overlay.querySelector('.cmd-palette-results');
  let selectedIdx = 0;

  function getItems(query) {
    const q = (query || '').toLowerCase();
    const items = [];
    for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
      const label = reg.label || id;
      const desc = TAB_DESCRIPTIONS[id] || '';
      if (q && !label.toLowerCase().includes(q) && !desc.toLowerCase().includes(q) && !id.includes(q)) continue;
      items.push({ id, label, icon: getTabIcon(id), desc });
    }
    return items;
  }

  function render(query) {
    const items = getItems(query);
    selectedIdx = Math.min(selectedIdx, Math.max(0, items.length - 1));
    results.innerHTML = items.length === 0
      ? '<div class="cmd-palette-empty">Ничего не найдено</div>'
      : items.map((it, i) => `
        <div class="cmd-palette-item${i === selectedIdx ? ' selected' : ''}" data-id="${it.id}">
          <span class="cmd-palette-icon">${it.icon}</span>
          <div class="cmd-palette-item-text">
            <span class="cmd-palette-item-label">${escapeHtml(it.label)}</span>
            ${it.desc ? `<span class="cmd-palette-item-desc">${escapeHtml(it.desc)}</span>` : ''}
          </div>
        </div>`).join('');
    results.querySelectorAll('.cmd-palette-item').forEach(el => {
      el.addEventListener('click', () => { selectItem(el.dataset.id); });
    });
  }

  function selectItem(tabId) {
    overlay.remove();
    if (tabId && TAB_REGISTRY[tabId]) {
      if (!S.openTabs.includes(tabId)) S.openTabs.push(tabId);
      switchTab(tabId);
    }
  }

  inp.addEventListener('input', () => { selectedIdx = 0; render(inp.value); });
  inp.addEventListener('keydown', (e) => {
    const items = getItems(inp.value);
    if (e.key === 'ArrowDown') { e.preventDefault(); selectedIdx = Math.min(selectedIdx + 1, items.length - 1); render(inp.value); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); selectedIdx = Math.max(selectedIdx - 1, 0); render(inp.value); }
    else if (e.key === 'Enter') { e.preventDefault(); if (items[selectedIdx]) selectItem(items[selectedIdx].id); }
    else if (e.key === 'Escape') { overlay.remove(); }
  });
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  render('');
  inp.focus();
}

document.addEventListener('keydown', (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault();
    showCommandPalette();
  }
});

// ══════════════════════════════════════════════
//  Initialization
// ══════════════════════════════════════════════

(async () => {
  // Paint the drawer + the active view container immediately from
  // localStorage so the user doesn't see a blank screen for the 1-5s the
  // DB takes to come up. activateView() also adds the `.active` class to
  // the right view div — without it CSS hides them all and the app body
  // is literally empty until the poll finishes. The data-fetches inside
  // each tab's loader race the Rust setup() hook and may come back empty;
  // the post-poll activateView() below + the visibilitychange handler
  // both re-render so the user sees fresh content quickly.
  try {
    renderTabBar();
    activateView();
    if (IS_MOBILE) updateMobileTitle();
  } catch (e) { console.warn('[hanni] early paint skipped', e); }

  // Right-side floating widgets — create them NOW, before the DB-ready await
  // below. On Android init_database() blocks invoke() for ~1.3s; if these run
  // after `await dbReady(...)` the +/music/bell buttons don't exist for the
  // first ~2s and early taps are lost. Each widget builds its DOM + click
  // handler synchronously and fetches its data fire-and-forget (.catch), so
  // it's interactive immediately and fills in once the DB is up.
  try {
    initQuotesWidget();
    initTaskControlWidget();
    initMusicWidget();
    initNotificationWidget();
  } catch (e) { console.warn('[hanni] widget init skipped', e); }

  // First paint is up — fade out the boot splash. Done in a microtask so
  // the browser has a chance to lay out renderTabBar's nodes first.
  queueMicrotask(() => {
    const splash = document.getElementById('boot-splash');
    if (splash) {
      splash.style.opacity = '0';
      setTimeout(() => splash.remove(), 200);
    }
  });

  // Load custom pages into TAB_REGISTRY before rendering.
  // Android: the webview boots in parallel with the Rust setup() hook, where
  // the DB is managed only after init_database() finishes. While migrations
  // run, invoke() can hang indefinitely (no resolve, no reject), so a bare
  // `await invoke()` deadlocks main.js. Wrap each attempt in a Promise.race
  // with a per-attempt timeout, retry until total deadline, then fall through
  // with empty customPages — the UI still renders, custom pages just won't
  // appear until next launch. On desktop attempt #0 wins immediately.
  const dbReady = async (cmd, args, perAttemptMs = 800, totalMs = 30000) => {
    const end = Date.now() + totalMs;
    for (;;) {
      try {
        return await Promise.race([
          invoke(cmd, args),
          new Promise((_, rej) => setTimeout(() => rej(new Error('attempt-timeout')), perAttemptMs)),
        ]);
      } catch (e) {
        if (Date.now() >= end) throw e;
        await new Promise(r => setTimeout(r, 200));
      }
    }
  };
  let customPages = [];
  try {
    customPages = await dbReady('get_custom_pages');
  } catch (e) {
    console.warn('[hanni] get_custom_pages not ready in 30s, continuing without custom pages', e);
  }
  for (const page of customPages) {
    const tabId = `page_${page.id}`;
    const isProject = page.page_type === 'project';
    TAB_REGISTRY[tabId] = {
      label: page.title,
      icon: page.icon,
      closable: true,
      subTabs: JSON.parse(page.sub_tabs || '[]'),
      custom: true,
      pageId: page.id,
      pageType: isProject ? 'project' : 'page',
    };
  }

  // Re-filter openTabs now that custom pages are registered
  S.openTabs = S.openTabs.filter(id => TAB_REGISTRY[id]);
  if (!S.openTabs.includes('chat')) S.openTabs.unshift('chat');

  // Mobile: open every registered tab on fresh install. Earlier we limited
  // mobile to ~6 default tabs, which made the drawer feel empty next to the
  // desktop install (and there's no UX path to discover the rest before
  // opening drawer + tapping `+`). Tabs are cheap, drawer is scrollable.
  if (IS_MOBILE && S.openTabs.length <= 1) {
    for (const id of Object.keys(TAB_REGISTRY)) {
      if (!S.openTabs.includes(id)) S.openTabs.push(id);
    }
  }

  // Sync tab_meta icons → tabCustomizations so sidebar shows custom emojis
  for (const tabId of S.openTabs) {
    try {
      const raw = await Promise.race([
        invoke('get_ui_state', { key: `tab_meta_${tabId}` }),
        new Promise((_, rej) => setTimeout(() => rej(new Error('tab_meta-timeout')), 1500)),
      ]);
      if (raw) {
        const meta = JSON.parse(raw);
        if (meta.icon) {
          if (!S.tabCustomizations[tabId]) S.tabCustomizations[tabId] = {};
          S.tabCustomizations[tabId].icon = meta.icon;
        }
      }
    } catch (_) {}
  }
  saveTabCustom();

  // Render tab bar (re-render now that custom pages are registered).
  // Widgets were already initialised up top, before the DB-ready await.
  renderTabBar();
  activateView();
  if (IS_MOBILE) updateMobileTitle();

  // Focus floating widget — disabled while MLX is offline
  // TODO: re-enable when MLX is back
  // createFocusWidget();
  // updateFocusWidget();
  // S.focusWidgetPollInterval = setInterval(() => updateFocusWidget(), 3000);

  // Chat floating overlay — disabled while MLX is offline
  // TODO: re-enable when MLX is back
  // createChatOverlay();
  // updateChatOverlayVisibility();

  // Auto-restore last conversation (reuse loadConversation to get feedback buttons + ratings)
  try {
    const convs = await Promise.race([
      invoke('get_conversations', { limit: 1 }),
      new Promise((_, rej) => setTimeout(() => rej(new Error('get_conversations-timeout')), 3000)),
    ]);
    if (convs.length > 0 && convs[0].id) {
      await loadConversation(convs[0].id);
    }
  } catch (_) {}
  if (chat.children.length === 0) renderChatWelcomeCard();
  loadConversationsList();

  // Auto-sync health data from Health Connect (Android only)
  // Triggered on cold start and whenever the app returns to foreground —
  // this is the "wake up, open Hanni" path: Watch → Health Connect → Hanni
  // → Firestore CRDT → Mac.
  if (IS_MOBILE) {
    autoImportHealth();
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState !== 'visible') return;
      autoImportHealth();
      // Returning to foreground: pull the latest from whichever transport is
      // active so we see what the Mac changed while we were backgrounded
      // (e.g. a task it started). lan_sync_now is bidirectional (one POST =
      // full exchange); cloud_owner_pull covers the async backend. Re-paint
      // (and refresh the task widget) after each settles so fresh rows show
      // without waiting for a poll.
      const repaint = () => {
        try { activateView(); } catch (_) {}
        try { window.dispatchEvent(new Event('task-state-changed')); } catch (_) {}
      };
      repaint();
      invoke('lan_sync_now').then(repaint, () => {});
      invoke('cloud_owner_pull').then(repaint, () => {});
    });
    startHealthPolling(); // 3-min poll while foregrounded
    // Schedule WorkManager-driven periodic HC pull so the watch → Hanni →
    // Mac path keeps flowing even when this WebView/Rust process is dead.
    // Android caps the interval at 15 min; we ask for the minimum.
    invoke('bg_sync_enable', { intervalMinutes: 15 }).catch(() => {});
    // One-time: ensure background HC read permission so the worker above can
    // actually read in the background (else HC auto-revokes sleep access).
    maybeRequestHealthBackground();
    confirmWebBoot(); // current OTA bundle booted OK → keep it (no-op on desktop)
    checkAndroidUpdate(); // GitHub Releases → APK update banner (no-op on desktop)
    checkWebUpdate(); // OTA web-asset bundle → applied for next launch (no-op on desktop)
  }

  // macOS desktop OTA web-assets: localStorage migration across the origin
  // switch + bundle check. No-op on dev (:1430) and non-macOS desktop.
  if (!IS_MOBILE) {
    desktopWebOTA();
  }

  // Android back button: close overlays, then go to previous tab
  if (IS_MOBILE) {
    history.pushState(null, '', '');
    window.addEventListener('popstate', () => {
      history.pushState(null, '', '');
      // Close drawer sidebar
      const tabBar = document.getElementById('tab-bar');
      if (tabBar?.classList.contains('drawer-open')) { tabLoaders.closeDrawer?.(); return; }
      // Close tab dropdown
      const dd = document.getElementById('tab-dropdown');
      if (dd && !dd.classList.contains('hidden')) { dd.classList.add('hidden'); return; }
      // Close sub-sidebar drawer
      const sb = document.getElementById('sub-sidebar');
      if (sb?.classList.contains('mobile-open')) { sb.classList.remove('mobile-open'); document.querySelector('.sub-sidebar-backdrop')?.remove(); return; }
      // Close any modal
      const modal = document.querySelector('.modal-overlay');
      if (modal) { modal.remove(); return; }
      // Switch to calendar (home) — chat tab is disabled while MLX is offline
      if (S.activeTab !== 'calendar') { switchTab('calendar'); return; }
    });
  }
})();
