// ══════════════════════════════════════════════
//  Hanni — Entry Point (ES Module)
//  Imports all modules, registers tabLoaders, initializes the app.
// ══════════════════════════════════════════════

// ── Foundation ──
import { S, invoke, listen, chat, input, sendBtn, tabLoaders, TAB_REGISTRY } from './js/state.js';
import { renderPageHeader, setupPageHeaderControls } from './js/utils.js';

// ── Core modules ──
import { renderTabBar, renderSubSidebar, openTab, closeTab, switchTab, activateView, ensureViewDiv, loadSubTabContent, showChatSettingsMode, hideChatSettingsMode } from './js/tabs.js';
import { loadConversationsList, loadConversation, autoSaveConversation } from './js/conversations.js';
import {
  addMsg, scrollDown, send, newChat,
  renderChatWelcomeCard, removeChatWelcomeCard,
  loadChatSettings, addFeedbackButtons, addProactiveFeedbackButtons,
  showStub, streamChat, toggleTTS, stopAllTTS, showAgentIndicator,
} from './js/chat.js';
import { parseAndExecuteActions, executeAction } from './js/actions.js';
import { checkVoiceServer, startRecording, stopRecordingAndSend, cancelRecording, toggleCallMode, startCallMode, endCallMode, startWakeWordSSE, stopWakeWordSSE } from './js/voice.js';

// ── Tab modules ──
import { loadCalendar } from './js/tab-calendar.js';
import { loadFocus, createFocusWidget, updateFocusWidget, updateFocusWidgetVisibility, toggleFocusWidgetPopover, startPomodoro, bindFocusWidgetEvents } from './js/tab-focus.js';
import { loadNotes, renderDatabaseView, renderNoteEditor, renderLinkedNotes, createAndOpenNote, createAndOpenTask } from './js/tab-notes.js';
import {
  loadHome, loadMindset, loadFood, loadMoney, loadPeople,
  loadMemoryTab, loadAbout, loadWork, loadDevelopment,
  loadHobbies, loadSports, loadHealth, loadCustomPage,
} from './js/tab-data.js';

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
tabLoaders.loadWork = loadWork;
tabLoaders.loadDevelopment = loadDevelopment;
tabLoaders.loadHome = loadHome;
tabLoaders.loadHobbies = loadHobbies;
tabLoaders.loadSports = loadSports;
tabLoaders.loadHealth = loadHealth;
tabLoaders.loadMindset = loadMindset;
tabLoaders.loadFood = loadFood;
tabLoaders.loadMoney = loadMoney;
tabLoaders.loadPeople = loadPeople;
tabLoaders.loadCustomPage = loadCustomPage;

// Focus widget
tabLoaders.createFocusWidget = createFocusWidget;
tabLoaders.updateFocusWidget = updateFocusWidget;
tabLoaders.updateFocusWidgetVisibility = updateFocusWidgetVisibility;
tabLoaders.toggleFocusWidgetPopover = toggleFocusWidgetPopover;
tabLoaders.startPomodoro = startPomodoro;

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
//  Initialization
// ══════════════════════════════════════════════

(async () => {
  // Load custom pages into TAB_REGISTRY before rendering
  try {
    const customPages = await invoke('get_custom_pages');
    for (const page of customPages) {
      const tabId = `page_${page.id}`;
      TAB_REGISTRY[tabId] = {
        label: page.title,
        icon: page.icon,
        closable: true,
        subTabs: JSON.parse(page.sub_tabs || '[]'),
        custom: true,
        pageId: page.id,
      };
    }
  } catch (_) {}

  // Re-filter openTabs now that custom pages are registered
  S.openTabs = S.openTabs.filter(id => TAB_REGISTRY[id]);
  if (!S.openTabs.includes('chat')) S.openTabs.unshift('chat');

  // Render tab bar
  renderTabBar();
  activateView();

  // Focus floating widget
  createFocusWidget();
  updateFocusWidget();
  S.focusWidgetPollInterval = setInterval(() => updateFocusWidget(), 3000);

  // Auto-restore last conversation
  try {
    const convs = await invoke('get_conversations', { limit: 1 });
    if (convs.length > 0) {
      const latest = convs[0];
      const conv = await invoke('get_conversation', { id: latest.id });
      if (conv.messages && conv.messages.length > 0) {
        S.currentConversationId = latest.id;
        S.history = conv.messages;
        for (const [role, content] of S.history) {
          if (role === 'user' && content.startsWith('[Action result:')) {
            const div = document.createElement('div');
            div.className = 'action-result success';
            div.textContent = content;
            chat.appendChild(div);
          } else {
            addMsg(role === 'assistant' ? 'bot' : role, content);
          }
        }
        scrollDown();
      }
    }
  } catch (_) {}
  if (chat.children.length === 0) renderChatWelcomeCard();
  loadConversationsList();
})();
