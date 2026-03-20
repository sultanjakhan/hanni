// ── js/chat-stream.js — SSE streaming logic, token handling, chat-token/chat-done events ──

import { S, invoke, listen } from './state.js';
import { normalizeHistoryMessage, renderMarkdown } from './utils.js';
import { scrollDown } from './chat-render.js';

async function streamChat(botDiv, t0, callMode = false) {
  const cursor = document.createElement('span');
  cursor.className = 'cursor';
  botDiv.appendChild(cursor);

  let firstToken = 0;
  let tokens = 0;
  let fullReply = '';
  let reasoningText = '';
  let toolCalls = [];
  let finishReason = null;

  // Reasoning collapsible block (thinking mode)
  let reasoningDetails = null;
  let reasoningContent = null;

  let scrollRAF = null;
  const scrollDownThrottled = () => {
    if (!scrollRAF) {
      scrollRAF = requestAnimationFrame(() => { scrollDown(); scrollRAF = null; });
    }
  };

  const unlistenReasoning = await listen('chat-reasoning', (event) => {
    if (!firstToken) firstToken = performance.now() - t0;
    const token = event.payload.token;
    reasoningText += token;
    // Create collapsible block on first reasoning token
    if (!reasoningDetails) {
      reasoningDetails = document.createElement('details');
      reasoningDetails.className = 'thinking-block';
      reasoningDetails.open = true;
      const summary = document.createElement('summary');
      summary.textContent = '\u{1F914} Думает...';
      reasoningDetails.appendChild(summary);
      reasoningContent = document.createElement('div');
      reasoningContent.className = 'thinking-content';
      reasoningDetails.appendChild(reasoningContent);
      botDiv.insertBefore(reasoningDetails, cursor);
    }
    reasoningContent.appendChild(document.createTextNode(token));
    scrollDownThrottled();
  });

  const unlistenReasoningDone = await listen('chat-reasoning-done', () => {
    if (reasoningDetails) {
      reasoningDetails.open = false; // collapse when done
      reasoningDetails.querySelector('summary').textContent = '\u{1F914} Рассуждения';
    }
  });

  const unlisten = await listen('chat-token', (event) => {
    if (!firstToken) firstToken = performance.now() - t0;
    tokens++;
    const token = event.payload.token;
    fullReply += token;
    botDiv.insertBefore(document.createTextNode(token), cursor);
    scrollDownThrottled();
  });

  const unlistenDone = await listen('chat-done', () => {
    // Fallback: close reasoning block if model ran out of tokens before content
    if (reasoningDetails && reasoningDetails.open) {
      reasoningDetails.open = false;
      reasoningDetails.querySelector('summary').textContent = '\u{1F914} Рассуждения';
    }
  });

  try {
    const msgs = S.history.slice(-20).map(normalizeHistoryMessage);
    const resultJson = await invoke('chat', { messages: msgs, callMode, conversationId: S.currentConversationId });
    // Parse ChatResult JSON from Rust
    try {
      const chatResult = JSON.parse(resultJson);
      if (chatResult.tool_calls && chatResult.tool_calls.length > 0) {
        toolCalls = chatResult.tool_calls;
      }
      finishReason = chatResult.finish_reason || null;
      // fullReply was already built by streaming tokens; use chatResult.text as fallback
      if (!fullReply && chatResult.text) {
        fullReply = chatResult.text;
      }
    } catch (_) {
      // If not valid JSON, treat as plain text (backward compat)
      if (!fullReply && typeof resultJson === 'string') {
        fullReply = resultJson;
      }
    }
  } catch (e) {
    if (!fullReply) {
      const errMsg = String(e);
      if (errMsg.includes('connection') || errMsg.includes('Connection') || errMsg.includes('refused')) {
        botDiv.textContent = 'MLX сервер недоступен — модель загружается...';
      } else {
        botDiv.textContent = 'Ошибка: ' + errMsg;
      }
    }
  }

  unlisten();
  unlistenDone();
  unlistenReasoning();
  unlistenReasoningDone();
  cursor.remove();

  // Re-render as Markdown after streaming completes
  if (fullReply) {
    botDiv.classList.add('markdown-body');
    botDiv.innerHTML = renderMarkdown(fullReply);
  }

  return { fullReply, tokens, firstToken, toolCalls, finishReason };
}

export { streamChat };
