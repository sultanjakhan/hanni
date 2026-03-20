// ── js/chat-input.js — Input handling, send, file attachment, drag-drop, new chat, input chips ──

import { S, invoke, chat, input, sendBtn, attachBtn, fileInput, attachPreview, tabLoaders } from './state.js';
import { getRole } from './utils.js';
import { autoSaveConversation, loadConversationsList } from './conversations.js';
import { addMsg, scrollDown, addFeedbackButtons, showAgentIndicator, toggleTTS, removeChatWelcomeCard, renderChatWelcomeCard } from './chat-render.js';
import { streamChat } from './chat-stream.js';

// ── File attachment ──

attachBtn.addEventListener('click', () => fileInput.click());

fileInput.addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (!file) return;

  if (file.size > 512000) {
    addMsg('bot', 'Файл слишком большой (макс 500KB)');
    fileInput.value = '';
    return;
  }

  const text = await file.text();
  S.attachedFile = { name: file.name, content: text };
  attachPreview.textContent = `\u{1F4CE} ${file.name}`;
  attachPreview.style.display = 'block';
  fileInput.value = '';
});

attachPreview.addEventListener('click', () => {
  S.attachedFile = null;
  attachPreview.style.display = 'none';
});

// ── Drag & drop ──

document.body.addEventListener('dragover', (e) => {
  e.preventDefault();
  document.body.classList.add('dragover');
});

document.body.addEventListener('dragleave', () => {
  document.body.classList.remove('dragover');
});

document.body.addEventListener('drop', async (e) => {
  e.preventDefault();
  document.body.classList.remove('dragover');
  const file = e.dataTransfer.files[0];
  if (!file) return;

  if (file.size > 512000) {
    addMsg('bot', 'Файл слишком большой (макс 500KB)');
    return;
  }

  const text = await file.text();
  S.attachedFile = { name: file.name, content: text };
  attachPreview.textContent = `\u{1F4CE} ${file.name}`;
  attachPreview.style.display = 'block';
});

// ── Send message ──

async function send() {
  const text = input.value.trim();
  if (!text || S.busy) return;

  S.busy = true;
  sendBtn.disabled = true;
  input.value = '';
  input.style.height = 'auto';

  try {

  // Report user chat activity for adaptive timing
  invoke('report_user_chat_activity').catch(() => {});
  // If user replies within 10 min of a proactive message, report engagement
  if (S.lastProactiveTime && (Date.now() - S.lastProactiveTime) < 600000) {
    invoke('report_proactive_engagement').catch(() => {});
    S.lastProactiveTime = 0;
  }

  // Build message with optional file
  const isVoice = S.lastMessageWasVoice;
  const sttTime = S.lastSttTimeMs;
  S.lastMessageWasVoice = false;
  S.lastSttTimeMs = 0;

  removeChatWelcomeCard();

  let userContent = text;
  if (S.attachedFile) {
    userContent += `\n\n\u{1F4CE} Файл: ${S.attachedFile.name}\n\`\`\`\n${S.attachedFile.content}\n\`\`\``;
    addMsg('user', `${text}\n\u{1F4CE} ${S.attachedFile.name}`);
    S.attachedFile = null;
    attachPreview.style.display = 'none';
  } else {
    addMsg('user', text, isVoice);
  }

  // If previous message was a proactive/autonomous message, add context hint
  // so the model focuses on the user's reply, not on echoing itself
  const prevMsg = S.history[S.history.length - 1];
  if (prevMsg && prevMsg.proactive) {
    // Rewrite the proactive message to include a marker for the model
    prevMsg.content = `[Автономное сообщение Ханни]: ${prevMsg.content}`;
  }

  S.history.push({ role: 'user', content: userContent });
  // Set history index on user wrapper for edit support
  { const lastUserWrapper = chat.querySelector('.user-wrapper:last-of-type');
    if (lastUserWrapper) lastUserWrapper.dataset.historyIdx = String(S.history.length - 1); }

  const MAX_ITERATIONS = 5;
  const t0 = performance.now();
  let totalTokens = 0;
  let firstToken = 0;
  let iteration = 0;

  while (iteration < MAX_ITERATIONS) {
    iteration++;

    // Show step indicator for iterations after the first
    if (iteration > 1) {
      showAgentIndicator(iteration);
    }

    // Create bot message div with TTS wrapper
    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const botDiv = document.createElement('div');
    botDiv.className = 'msg bot';
    wrapper.appendChild(botDiv);
    chat.appendChild(wrapper);
    scrollDown();

    // Stream model response
    const result = await streamChat(botDiv, t0);
    if (!firstToken && result.firstToken) firstToken = result.firstToken;
    totalTokens += result.tokens;

    // Primary path: native tool calls from the model
    if (result.toolCalls && result.toolCalls.length > 0) {
      // Push assistant message with tool_calls into history
      const assistantMsg = { role: 'assistant', content: result.fullReply || null, tool_calls: result.toolCalls };
      S.history.push(assistantMsg);
      wrapper.dataset.historyIdx = String(S.history.length - 1);

      // Mark as intermediate (tool calls, not final answer)
      botDiv.classList.add('intermediate');

      // Execute each tool call and push results
      for (const tc of result.toolCalls) {
        let args;
        try { args = JSON.parse(tc.function.arguments); } catch (_) { args = {}; }
        // CH7: Show streaming action indicator
        const indicatorDiv = document.createElement('div');
        indicatorDiv.className = 'action-indicator';
        indicatorDiv.textContent = `Выполняю: ${tc.function.name}...`;
        chat.appendChild(indicatorDiv);
        scrollDown();
        // Inject action type for executeAction compatibility
        args.action = tc.function.name;
        const actionJson = JSON.stringify(args);
        const { success, result: actionResult } = await tabLoaders.executeAction(actionJson);
        indicatorDiv.remove();
        const actionDiv = document.createElement('div');
        actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
        actionDiv.textContent = actionResult;
        chat.appendChild(actionDiv);
        scrollDown();

        // Push tool result into history
        S.history.push({
          role: 'tool',
          tool_call_id: tc.id,
          name: tc.function.name,
          content: String(actionResult)
        });
      }
      // Continue loop — model will respond with a summary
      continue;
    }

    if (!result.fullReply) break;

    S.history.push({ role: 'assistant', content: result.fullReply });
    wrapper.dataset.historyIdx = String(S.history.length - 1);

    // Fallback path: parse ```action blocks from text (backward compat)
    const actions = tabLoaders.parseAndExecuteActions(result.fullReply);

    // No actions — this is the final answer, stop the loop
    if (actions.length === 0) break;

    // Mark this response as intermediate (it contained actions, not a final answer)
    botDiv.classList.add('intermediate');

    // Execute actions and show results
    const results = [];
    for (const actionJson of actions) {
      // CH7: Show action indicator
      let actionName = 'action';
      try { const a = JSON.parse(actionJson); actionName = a.action || a.type || 'action'; } catch (_) {}
      const indicatorDiv = document.createElement('div');
      indicatorDiv.className = 'action-indicator';
      indicatorDiv.textContent = `Выполняю: ${actionName}...`;
      chat.appendChild(indicatorDiv);
      scrollDown();
      const { success, result: actionResult } = await tabLoaders.executeAction(actionJson);
      indicatorDiv.remove();
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
      scrollDown();
      results.push(actionResult);
    }

    // Feed results back into history so the model sees them
    S.history.push({ role: 'user', content: `[Action result: ${results.join('; ')}]` });
  }

  // Add TTS button to last bot wrapper
  const lastWrapper = chat.querySelector('.msg-wrapper:last-of-type');
  if (lastWrapper && !lastWrapper.querySelector('.tts-btn')) {
    const lastBotDiv = lastWrapper.querySelector('.msg.bot');
    if (lastBotDiv) {
      const ttsBtn = document.createElement('button');
      ttsBtn.className = 'tts-btn';
      ttsBtn.innerHTML = '&#9654;';
      ttsBtn.title = 'Озвучить';
      const ttsText = lastBotDiv.textContent;
      ttsBtn.addEventListener('click', () => toggleTTS(ttsBtn, ttsText));
      lastWrapper.appendChild(ttsBtn);
    }
  }

  // Show timing
  const total = ((performance.now() - t0) / 1000).toFixed(1);
  const ttft = firstToken ? (firstToken / 1000).toFixed(1) : '?';
  const timing = document.createElement('div');
  timing.className = 'timing';
  const stepInfo = iteration > 1 ? ` \u00B7 ${iteration} steps` : '';
  const sttInfo = isVoice && sttTime ? `STT ${(sttTime / 1000).toFixed(1)}s \u00B7 ` : '';
  timing.textContent = `${sttInfo}${ttft}s first token \u00B7 ${total}s total \u00B7 ${totalTokens} tokens${stepInfo}`;
  chat.appendChild(timing);
  scrollDown();

  // Post-chat: incremental save + extract facts in background
  (async () => {
    try {
      if (S.currentConversationId) {
        await invoke('update_conversation', { id: S.currentConversationId, messages: S.history });
      } else {
        S.currentConversationId = await invoke('save_conversation', { messages: S.history });
      }
      // Add feedback buttons to bot messages that have a history index
      if (S.currentConversationId) {
        chat.querySelectorAll('.msg-wrapper[data-history-idx]').forEach(w => {
          if (w.querySelector('.feedback-btn')) return;
          const idx = parseInt(w.dataset.historyIdx, 10);
          if (!isNaN(idx) && getRole(S.history[idx]) === 'assistant') {
            addFeedbackButtons(w, S.currentConversationId, idx, S.history[idx]?.content || '');
          }
        });
      }
      if (S.history.length >= 2) {
        await invoke('process_conversation_end', { messages: S.history, conversationId: S.currentConversationId });
      }
      loadConversationsList();
    } catch (_) {}
  })();

  } catch (err) {
    console.error('send() error:', err);
    addMsg('bot', 'Ошибка: ' + (err.message || err));
  } finally {
    S.busy = false;
    sendBtn.disabled = false;
    input.focus();
  }
}

// ── New Chat ──

async function newChat() {
  if (S.busy) S.busy = false;  // allow new chat even during active stream
  // Save current conversation before clearing
  await autoSaveConversation();
  S.currentConversationId = null;
  S.history = [];
  chat.innerHTML = '';
  renderChatWelcomeCard();
  loadConversationsList();
  input.focus();
}

document.getElementById('new-chat')?.addEventListener('click', newChat);

// ── Input toolbar chips (Thinking, Self-refine, Web Search) ──
const chipThinking = document.getElementById('chip-thinking');
const chipSelfRefine = document.getElementById('chip-selfrefine');
const chipWebSearch = document.getElementById('chip-websearch');

(async () => {
  const [thinkVal, refineVal, webVal] = await Promise.all([
    invoke('get_app_setting', { key: 'enable_thinking' }).catch(() => null),
    invoke('get_app_setting', { key: 'enable_self_refine' }).catch(() => null),
    invoke('get_app_setting', { key: 'enable_web_search' }).catch(() => null),
  ]);
  if (thinkVal === 'true') chipThinking?.classList.add('active');
  if (refineVal === 'true') chipSelfRefine?.classList.add('active');
  if (webVal === 'true') chipWebSearch?.classList.add('active');
})();

chipThinking?.addEventListener('click', async () => {
  chipThinking.classList.toggle('active');
  const on = chipThinking.classList.contains('active');
  await invoke('set_app_setting', { key: 'enable_thinking', value: on ? 'true' : 'false' }).catch(() => {});
});

chipSelfRefine?.addEventListener('click', async () => {
  chipSelfRefine.classList.toggle('active');
  const on = chipSelfRefine.classList.contains('active');
  await invoke('set_app_setting', { key: 'enable_self_refine', value: on ? 'true' : 'false' }).catch(() => {});
});

chipWebSearch?.addEventListener('click', async () => {
  chipWebSearch.classList.toggle('active');
  const on = chipWebSearch.classList.contains('active');
  await invoke('set_app_setting', { key: 'enable_web_search', value: on ? 'true' : 'false' }).catch(() => {});
});

sendBtn.addEventListener('click', send);
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    send();
  }
});
// Auto-resize textarea as user types
input.addEventListener('input', () => {
  input.style.height = 'auto';
  input.style.height = Math.min(input.scrollHeight, 150) + 'px';
});

export { send, newChat };
