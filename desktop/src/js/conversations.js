// ── js/conversations.js — Conversation sidebar, loading, saving, search ──

import { S, invoke, chat, input, tabLoaders } from './state.js';
import { escapeHtml, normalizeHistoryMessage, getRole, getContent } from './utils.js';

// ── Load conversations list (grouped by date) ──

export async function loadConversationsList(searchQuery) {
  const convList = document.getElementById('sidebar-conv-list') || document.getElementById('conv-list');
  if (!convList) return;
  try {
    let convs;
    if (searchQuery && searchQuery.trim().length > 1) {
      convs = await invoke('search_conversations', { query: searchQuery, limit: 20 });
    } else {
      convs = await invoke('get_conversations', { limit: 50 });
    }
    convList.innerHTML = '';

    // Group by date (Today / Yesterday / This Week / Earlier)
    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterdayStart = new Date(todayStart); yesterdayStart.setDate(yesterdayStart.getDate() - 1);
    const weekStart = new Date(todayStart); weekStart.setDate(weekStart.getDate() - 7);

    const groups = { today: [], yesterday: [], week: [], earlier: [] };
    for (const c of convs) {
      const d = new Date(c.started_at);
      if (d >= todayStart) groups.today.push(c);
      else if (d >= yesterdayStart) groups.yesterday.push(c);
      else if (d >= weekStart) groups.week.push(c);
      else groups.earlier.push(c);
    }

    const labels = { today: 'Сегодня', yesterday: 'Вчера', week: 'На этой неделе', earlier: 'Ранее' };
    for (const [key, items] of Object.entries(groups)) {
      if (items.length === 0) continue;
      const header = document.createElement('div');
      header.className = 'conv-group-header';
      header.textContent = labels[key];
      convList.appendChild(header);
      for (const c of items) {
        const item = document.createElement('div');
        item.className = 'conv-item' + (c.id === S.currentConversationId ? ' active' : '');
        const summary = c.summary || `Диалог (${c.message_count} сообщ.)`;
        const isProactive = summary.startsWith('[Auto]') || summary.startsWith('[Proactive]');
        item.innerHTML = `
          ${isProactive ? '<span class="conv-auto-badge">auto</span>' : ''}
          <div class="conv-item-summary">${escapeHtml(summary.replace(/^\[(Auto|Proactive)\]\s*/, ''))}</div>
          <button class="conv-delete" data-id="${c.id}" title="Удалить">&times;</button>
        `;
        item.addEventListener('click', (e) => {
          if (e.target.classList.contains('conv-delete')) return;
          loadConversation(c.id);
        });
        item.querySelector('.conv-delete').addEventListener('click', async (e) => {
          e.stopPropagation();
          const id = parseInt(e.target.dataset.id);
          await invoke('delete_conversation', { id });
          if (S.currentConversationId === id) {
            S.currentConversationId = null;
            S.history = [];
            chat.innerHTML = '';
            tabLoaders.renderChatWelcomeCard();
          }
          loadConversationsList();
        });
        convList.appendChild(item);
      }
    }
  } catch (_) {}
}

// ── Load a single conversation ──

export async function loadConversation(id) {
  if (S.busy) return;
  try {
    // Save current conversation before switching
    await autoSaveConversation();
    const conv = await invoke('get_conversation', { id });
    S.currentConversationId = id;
    // Normalize messages: handle both old [role, content] and new {role, content} formats
    S.history = (conv.messages || []).map(normalizeHistoryMessage);
    chat.innerHTML = '';
    // Load existing ratings
    let ratingsMap = {};
    try {
      const ratings = await invoke('get_message_ratings', { conversationId: id });
      for (const [idx, rating] of ratings) {
        ratingsMap[idx] = rating;
      }
    } catch (_) {}
    // Render all messages
    for (let i = 0; i < S.history.length; i++) {
      const role = getRole(S.history[i]);
      const content = getContent(S.history[i]);
      // Skip tool result messages in UI
      if (role === 'tool') {
        const div = document.createElement('div');
        div.className = 'action-result success';
        div.textContent = content;
        chat.appendChild(div);
        continue;
      }
      if (role === 'user' && content.startsWith('[Action result:')) {
        const div = document.createElement('div');
        div.className = 'action-result success';
        div.textContent = content;
        chat.appendChild(div);
      } else if (role === 'user' || role === 'assistant') {
        tabLoaders.addMsg(role === 'assistant' ? 'bot' : role, content);
        const lastWrapper = chat.querySelector('.msg-wrapper:last-of-type');
        if (lastWrapper) lastWrapper.dataset.historyIdx = String(i);
        // Add feedback buttons to bot messages
        if (role === 'assistant') {
          if (lastWrapper) {
            const { thumbUp, thumbDown } = tabLoaders.addFeedbackButtons(lastWrapper, id, i, content);
            if (ratingsMap[i] === 1) thumbUp.classList.add('active');
            if (ratingsMap[i] === -1) thumbDown.classList.add('active');
          }
        }
      }
    }
    tabLoaders.scrollDown();
    loadConversationsList();
  } catch (e) {
    tabLoaders.addMsg('bot', 'Ошибка загрузки: ' + e);
  }
}

// ── Auto-save current conversation ──

export async function autoSaveConversation() {
  if (S.history.length < 2) return;
  try {
    if (S.currentConversationId) {
      await invoke('update_conversation', { id: S.currentConversationId, messages: S.history });
    } else {
      S.currentConversationId = await invoke('save_conversation', { messages: S.history });
    }
  } catch (_) {}
}

// ── New chat button ──

document.getElementById('new-chat-btn')?.addEventListener('click', async () => {
  await autoSaveConversation();
  S.currentConversationId = null;
  S.history = [];
  chat.innerHTML = '';
  tabLoaders.renderChatWelcomeCard();
  input.focus();
  loadConversationsList();
});

// ── Conversation search (debounced) ──

document.getElementById('conv-search')?.addEventListener('input', (e) => {
  clearTimeout(S.convSearchTimeout);
  S.convSearchTimeout = setTimeout(() => {
    loadConversationsList(e.target.value);
  }, 300);
});
