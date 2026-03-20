// ── js/chat-overlay.js — Floating chat overlay panel ──

import { S, invoke, chat, input, listen, tabLoaders } from './state.js';
import { renderMarkdown, getRole, getContent } from './utils.js';
import { scrollDown, addMsg, removeChatWelcomeCard, renderChatWelcomeCard } from './chat-render.js';
import { send, newChat } from './chat-input.js';

let coPanel = null;
let coMessages = null;
let coInput = null;
let coTyping = null;
let coFab = null;

function createChatOverlay() {
  if (document.getElementById('chat-overlay')) return;

  // FAB button
  const fab = document.createElement('button');
  fab.id = 'chat-overlay-fab';
  fab.className = 'chat-overlay-fab hidden';
  fab.title = 'Открыть чат (\u2318\u21E7C)';
  fab.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>';
  coFab = fab;

  // Panel
  const panel = document.createElement('div');
  panel.id = 'chat-overlay';
  panel.className = 'chat-overlay hidden';
  panel.innerHTML = `
    <div class="co-header">
      <div class="co-header-left">
        <button class="co-conv-selector" id="co-conv-selector" title="Выбрать чат">
          <span class="co-conv-name" id="co-conv-name">Новый чат</span>
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
        <button class="co-new-chat-btn" id="co-new-chat" title="Новый чат">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </button>
      </div>
      <div class="co-header-actions">
        <button id="co-goto-chat" title="Открыть полный чат">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>
        </button>
        <button id="co-close" title="Закрыть">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
    </div>
    <div class="co-conv-dropdown hidden" id="co-conv-dropdown"></div>
    <div class="co-messages" id="co-messages"></div>
    <div class="co-typing hidden" id="co-typing">Печатает...</div>
    <div class="co-input-area">
      <div class="co-input-wrapper">
        <textarea class="co-input" id="co-input" placeholder="Сообщение..." rows="1"></textarea>
        <button class="co-send" id="co-send">
          <svg viewBox="0 0 16 16" fill="none"><path d="M3 13V3l11 5-11 5z" fill="currentColor"/></svg>
        </button>
      </div>
    </div>
  `;

  const contentArea = document.getElementById('content-area');
  contentArea.appendChild(fab);
  contentArea.appendChild(panel);

  coPanel = panel;
  coMessages = panel.querySelector('#co-messages');
  coInput = panel.querySelector('#co-input');
  coTyping = panel.querySelector('#co-typing');

  // Events
  fab.addEventListener('click', toggleChatOverlay);

  panel.querySelector('#co-close').addEventListener('click', () => {
    coPanel.classList.add('hidden');
    coFab.classList.remove('hidden');
  });

  panel.querySelector('#co-goto-chat').addEventListener('click', () => {
    coPanel.classList.add('hidden');
    coFab.classList.add('hidden');
    tabLoaders.switchTab('chat');
  });

  // New chat button
  panel.querySelector('#co-new-chat').addEventListener('click', async () => {
    await newChat();
    updateOverlayConvName();
    refreshOverlayMessages();
  });

  // Conversation selector dropdown
  const convSelector = panel.querySelector('#co-conv-selector');
  const convDropdown = panel.querySelector('#co-conv-dropdown');
  convSelector.addEventListener('click', async (e) => {
    e.stopPropagation();
    const isOpen = !convDropdown.classList.contains('hidden');
    if (isOpen) {
      convDropdown.classList.add('hidden');
      return;
    }
    // Load conversations
    try {
      const convs = await invoke('get_conversations', { limit: 15 });
      convDropdown.innerHTML = '';
      if (!convs || convs.length === 0) {
        convDropdown.innerHTML = '<div class="co-conv-item co-conv-empty">Нет чатов</div>';
      } else {
        for (const c of convs) {
          const item = document.createElement('div');
          item.className = 'co-conv-item';
          if (c.id === S.currentConversationId) item.classList.add('active');
          item.textContent = c.title || c.summary || `Чат #${c.id}`;
          item.addEventListener('click', async () => {
            convDropdown.classList.add('hidden');
            await tabLoaders.loadConversation(c.id);
            updateOverlayConvName();
            refreshOverlayMessages();
          });
          convDropdown.appendChild(item);
        }
      }
      convDropdown.classList.remove('hidden');
    } catch (_) {}
  });

  const coSendBtn = panel.querySelector('#co-send');
  coSendBtn.addEventListener('click', sendFromOverlay);

  coInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendFromOverlay();
    }
  });

  // Auto-resize textarea
  coInput.addEventListener('input', () => {
    coInput.style.height = 'auto';
    coInput.style.height = Math.min(coInput.scrollHeight, 80) + 'px';
  });

  // Close on outside click (also close dropdown)
  document.addEventListener('click', (e) => {
    if (!coPanel.classList.contains('hidden') && !coPanel.contains(e.target) && !coFab.contains(e.target)) {
      coPanel.classList.add('hidden');
      coFab.classList.remove('hidden');
    }
    // Close dropdown if clicking outside it
    if (convDropdown && !convDropdown.classList.contains('hidden') && !convSelector.contains(e.target) && !convDropdown.contains(e.target)) {
      convDropdown.classList.add('hidden');
    }
  });
}

function updateOverlayConvName() {
  const nameEl = document.getElementById('co-conv-name');
  if (!nameEl) return;
  if (!S.currentConversationId) {
    nameEl.textContent = 'Новый чат';
  } else {
    // Try to get title from first user message
    const firstUser = S.history.find(m => getRole(m) === 'user');
    const content = firstUser ? getContent(firstUser) : '';
    nameEl.textContent = content ? (content.length > 30 ? content.substring(0, 30) + '...' : content) : `Чат #${S.currentConversationId}`;
  }
}

function toggleChatOverlay() {
  if (!coPanel) return;
  const isHidden = coPanel.classList.contains('hidden');
  if (isHidden) {
    updateOverlayConvName();
    refreshOverlayMessages();
    coPanel.classList.remove('hidden');
    coFab.classList.add('hidden');
    coInput.focus();
  } else {
    coPanel.classList.add('hidden');
    coFab.classList.remove('hidden');
  }
}

function refreshOverlayMessages() {
  if (!coMessages) return;
  coMessages.innerHTML = '';

  // Show last 20 messages from history
  const msgs = S.history.slice(-20);
  if (msgs.length === 0) {
    coMessages.innerHTML = '<div class="co-empty">Начните разговор</div>';
    return;
  }

  for (const m of msgs) {
    const role = getRole(m);
    if (role === 'tool' || role === 'system') continue;
    const content = getContent(m);
    if (!content) continue;

    const div = document.createElement('div');
    if (role === 'user') {
      div.className = 'co-msg user';
      div.textContent = content;
    } else {
      div.className = 'co-msg bot markdown-body';
      div.innerHTML = renderMarkdown(content);
    }
    coMessages.appendChild(div);
  }

  coMessages.scrollTop = coMessages.scrollHeight;
}

async function sendFromOverlay() {
  const text = coInput.value.trim();
  if (!text || S.busy) return;

  // Add user message to overlay immediately
  const userDiv = document.createElement('div');
  userDiv.className = 'co-msg user';
  userDiv.textContent = text;
  coMessages.querySelector('.co-empty')?.remove();
  coMessages.appendChild(userDiv);
  coMessages.scrollTop = coMessages.scrollHeight;

  // Show typing indicator
  coTyping.classList.remove('hidden');

  // Send via main chat
  coInput.value = '';
  coInput.style.height = 'auto';
  input.value = text;
  await send();

  // Refresh overlay with actual response
  coTyping.classList.add('hidden');
  refreshOverlayMessages();
}

function updateChatOverlayVisibility() {
  if (!coFab) return;
  const onChatTab = S.activeTab === 'chat';
  if (onChatTab) {
    coFab.classList.add('hidden');
    coPanel?.classList.add('hidden');
  } else {
    // Show FAB only when overlay panel is closed
    if (coPanel?.classList.contains('hidden')) {
      coFab.classList.remove('hidden');
    }
  }
}

// Listen for new proactive messages to refresh overlay
listen('proactive-message', () => {
  if (coPanel && !coPanel.classList.contains('hidden')) {
    setTimeout(refreshOverlayMessages, 100);
  }
});

export {
  createChatOverlay,
  toggleChatOverlay,
  updateChatOverlayVisibility,
  refreshOverlayMessages,
};
