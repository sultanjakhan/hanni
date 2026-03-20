// ── js/chat.js — Main entry, event listeners, re-exports ──

import { S, invoke, listen, chat, input, tabLoaders } from './state.js';
import { autoSaveConversation } from './conversations.js';
import { showChatSettingsMode, hideChatSettingsMode } from './tabs.js';

// ── Sub-modules ──
import { loadChatSettings } from './chat-settings.js';
import { addMsg, scrollDown, addFeedbackButtons, addProactiveFeedbackButtons, showStub, showAgentIndicator, toggleTTS, stopAllTTS, removeChatWelcomeCard, renderChatWelcomeCard } from './chat-render.js';
import { streamChat } from './chat-stream.js';
import { send, newChat } from './chat-input.js';
import { createChatOverlay, toggleChatOverlay, updateChatOverlayVisibility, refreshOverlayMessages } from './chat-overlay.js';

// ── Auto-update notification ──
listen('update-available', (event) => {
  const version = event.payload;
  const banner = document.createElement('div');
  banner.style.cssText = 'padding:8px 16px;background:var(--bg-card);color:var(--text-secondary);font-size:12px;text-align:center;border-bottom:1px solid var(--border-default);';
  banner.textContent = `Обновление до v${version}...`;
  document.getElementById('content-area')?.prepend(banner);
});

// ── Proactive message listener ──
listen('proactive-message', async (event) => {
  // Prevent race condition: don't mutate history while chat is streaming
  if (S.busy) return;
  // v0.22: payload is now {text, id} JSON
  const payload = typeof event.payload === 'object' ? event.payload : { text: event.payload, id: 0 };
  const text = payload.text;
  const proactiveId = payload.id || 0;
  S.lastProactiveTime = Date.now();

  // Use addMsg to get proper wrapper with TTS button
  removeChatWelcomeCard();
  const msgDiv = addMsg('bot', text);
  const wrapper = msgDiv.closest('.msg-wrapper');
  if (wrapper) {
    msgDiv.classList.add('proactive');
  }

  const ts = document.createElement('div');
  ts.className = 'proactive-time';
  ts.textContent = new Date().toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  chat.appendChild(ts);

  // Add to history so user can reply naturally (marked as proactive for context)
  const histIdx = S.history.length;
  S.history.push({ role: 'assistant', content: text, proactive: true });
  scrollDown();

  // P1: Execute any action blocks from proactive messages
  const proactiveActions = tabLoaders.parseAndExecuteActions(text);
  if (proactiveActions.length > 0) {
    for (const actionJson of proactiveActions) {
      const { success, result: actionResult } = await tabLoaders.executeAction(actionJson);
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
    }
    S.history.push({ role: 'user', content: `[Action result: ${proactiveActions.map(() => 'ok').join('; ')}]` });
  }

  await autoSaveConversation();

  // Add proactive feedback buttons (copy + thumbs — no regen)
  if (wrapper) {
    addProactiveFeedbackButtons(wrapper, proactiveId, text);
  }

  // Desktop notification if window not focused
  if (!document.hasFocus()) {
    new Notification('Hanni', { body: text });
  }
});

// ── Auto-quiet status listener ──
listen('proactive-auto-quiet', (event) => {
  const badge = document.getElementById('proactive-status-badge');
  if (badge) {
    if (event.payload) {
      badge.textContent = 'Тишина (авто)';
      badge.classList.add('quiet');
    } else {
      badge.textContent = 'Активен';
      badge.classList.remove('quiet');
    }
  }
});

// ── Typing signal ──
input.addEventListener('input', () => {
  invoke('set_user_typing', { typing: true }).catch(() => {});
  clearTimeout(S.typingTimeout);
  S.typingTimeout = setTimeout(() => {
    invoke('set_user_typing', { typing: false }).catch(() => {});
  }, 10000);
});

// ── Reminder notifications ──
listen('reminder-fired', (event) => {
  const title = event.payload;
  addMsg('bot', `\u23F0 Напоминание: ${title}`);
  scrollDown();
  if (!document.hasFocus()) {
    new Notification('Напоминание', { body: title });
  }
});

listen('note-reminder-fired', (event) => {
  const { id, title } = event.payload;
  addMsg('bot', `\u{1F4DD} Заметка-напоминание: **${title}**`);
  scrollDown();
  if (!document.hasFocus()) {
    new Notification('Заметка', { body: title });
  }
});

// ── Focus mode listener ──
listen('focus-ended', () => {
  addMsg('bot', 'Фокус-режим завершён!');
  if (S.focusTimerInterval) {
    clearInterval(S.focusTimerInterval);
    S.focusTimerInterval = null;
  }
  if (tabLoaders.updateFocusWidget) tabLoaders.updateFocusWidget();
});

// ── Exports ──

export {
  addMsg,
  scrollDown,
  send,
  newChat,
  renderChatWelcomeCard,
  removeChatWelcomeCard,
  showChatSettingsMode,
  hideChatSettingsMode,
  loadChatSettings,
  addFeedbackButtons,
  addProactiveFeedbackButtons,
  showStub,
  streamChat,
  toggleTTS,
  stopAllTTS,
  showAgentIndicator,
  createChatOverlay,
  toggleChatOverlay,
  updateChatOverlayVisibility,
  refreshOverlayMessages,
};
