const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const chat = document.getElementById('chat');
const input = document.getElementById('input');
const sendBtn = document.getElementById('send');
const attachBtn = document.getElementById('attach');
const fileInput = document.getElementById('file-input');
const attachPreview = document.getElementById('attach-preview');
const integrationsContent = document.getElementById('integrations-content');
const settingsContent = document.getElementById('settings-content');

const APP_VERSION = '0.7.0';

let busy = false;
let history = [];
let attachedFile = null; // {name, content}
let currentTab = 'chat';
let isRecording = false;
let integrationsLoaded = false;
let currentConversationId = null; // Active conversation ID in SQLite
let isSpeaking = false;
let convSearchTimeout = null;

// Track which tabs have been loaded
const tabLoaded = {};

// ── Auto-update notification ──
listen('update-available', (event) => {
  const version = event.payload;
  const banner = document.createElement('div');
  banner.style.cssText = 'padding:8px 16px;background:#161616;color:#888;font-size:12px;text-align:center;border-bottom:1px solid #222;';
  banner.textContent = `Обновление до v${version}...`;
  document.querySelector('main').prepend(banner);
});

// ── Proactive message listener ──
listen('proactive-message', (event) => {
  const text = event.payload;
  const div = document.createElement('div');
  div.className = 'msg bot proactive';
  div.textContent = text;
  chat.appendChild(div);

  const ts = document.createElement('div');
  ts.className = 'proactive-time';
  ts.textContent = new Date().toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  chat.appendChild(ts);

  // Add to history so user can reply naturally
  history.push(['assistant', text]);
  scrollDown();
  autoSaveConversation();

  // Desktop notification if window not focused
  if (!document.hasFocus()) {
    new Notification('Hanni', { body: text });
  }
});

// ── Typing signal ──
let typingTimeout = null;
input.addEventListener('input', () => {
  invoke('set_user_typing', { typing: true }).catch(() => {});
  clearTimeout(typingTimeout);
  typingTimeout = setTimeout(() => {
    invoke('set_user_typing', { typing: false }).catch(() => {});
  }, 5000);
});

// ── Voice recording ──

const recordBtn = document.getElementById('record');

recordBtn.addEventListener('click', async () => {
  if (isRecording) {
    // Stop recording
    isRecording = false;
    recordBtn.classList.remove('recording');
    recordBtn.title = 'Голосовой ввод';
    try {
      const text = await invoke('stop_recording');
      if (text) {
        input.value = (input.value ? input.value + ' ' : '') + text;
        input.focus();
      }
    } catch (e) {
      addMsg('bot', 'Ошибка транскрипции: ' + e);
    }
  } else {
    // Check if whisper model exists
    try {
      const hasModel = await invoke('check_whisper_model');
      if (!hasModel) {
        if (confirm('Модель Whisper не найдена (~1.5GB). Скачать?')) {
          addMsg('bot', 'Скачиваю модель Whisper...');
          const unlisten = await listen('whisper-download-progress', (event) => {
            // Update last bot message with progress
            const msgs = chat.querySelectorAll('.msg.bot');
            const last = msgs[msgs.length - 1];
            if (last) last.textContent = `Скачиваю Whisper... ${event.payload}%`;
          });
          try {
            await invoke('download_whisper_model');
            addMsg('bot', 'Whisper загружен! Можете записывать голос.');
          } catch (e) {
            addMsg('bot', 'Ошибка загрузки: ' + e);
          }
          unlisten();
        }
        return;
      }
    } catch (e) {
      addMsg('bot', 'Ошибка: ' + e);
      return;
    }

    // Start recording
    try {
      await invoke('start_recording');
      isRecording = true;
      recordBtn.classList.add('recording');
      recordBtn.title = 'Остановить запись';
    } catch (e) {
      addMsg('bot', 'Ошибка записи: ' + e);
    }
  }
});

// ── Focus mode listener ──

listen('focus-ended', () => {
  addMsg('bot', 'Фокус-режим завершён!');
  if (focusTimerInterval) {
    clearInterval(focusTimerInterval);
    focusTimerInterval = null;
  }
});

// ── Conversation sidebar ──

async function loadConversationsList(searchQuery) {
  const convList = document.getElementById('conv-list');
  if (!convList) return;
  try {
    let convs;
    if (searchQuery && searchQuery.trim().length > 1) {
      convs = await invoke('search_conversations', { query: searchQuery, limit: 20 });
    } else {
      convs = await invoke('get_conversations', { limit: 30 });
    }
    convList.innerHTML = '';
    for (const c of convs) {
      const item = document.createElement('div');
      item.className = 'conv-item' + (c.id === currentConversationId ? ' active' : '');
      const date = new Date(c.started_at);
      const dateStr = date.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' });
      const timeStr = date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
      const summary = c.summary || `Диалог (${c.message_count} сообщ.)`;
      item.innerHTML = `
        <div class="conv-item-summary">${escapeHtml(summary)}</div>
        <div class="conv-item-meta">${dateStr} ${timeStr} · ${c.message_count} сообщ.</div>
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
        if (currentConversationId === id) {
          currentConversationId = null;
          history = [];
          chat.innerHTML = '';
        }
        loadConversationsList();
      });
      convList.appendChild(item);
    }
  } catch (_) {}
}

async function loadConversation(id) {
  if (busy) return;
  try {
    // Save current conversation before switching
    await autoSaveConversation();
    const conv = await invoke('get_conversation', { id });
    currentConversationId = id;
    history = conv.messages || [];
    chat.innerHTML = '';
    // Render all messages
    for (const [role, content] of history) {
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
    loadConversationsList();
  } catch (e) {
    addMsg('bot', 'Ошибка загрузки: ' + e);
  }
}

async function autoSaveConversation() {
  if (history.length < 2) return;
  try {
    if (currentConversationId) {
      await invoke('update_conversation', { id: currentConversationId, messages: history });
    } else {
      currentConversationId = await invoke('save_conversation', { messages: history });
    }
  } catch (_) {}
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// Conversation search
document.getElementById('conv-search')?.addEventListener('input', (e) => {
  clearTimeout(convSearchTimeout);
  convSearchTimeout = setTimeout(() => {
    loadConversationsList(e.target.value);
  }, 300);
});

// ── Tab navigation ──

function switchTab(tab) {
  currentTab = tab;

  // Update sidebar buttons
  document.querySelectorAll('.sidebar-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.tab === tab);
  });

  // Update views
  document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
  const view = document.getElementById(`view-${tab}`);
  if (view) view.classList.add('active');

  // Show/hide chat-specific UI
  const newChatBtn = document.getElementById('new-chat');
  if (newChatBtn) newChatBtn.style.display = tab === 'chat' ? '' : 'none';

  // Lazy-load tab content
  switch (tab) {
    case 'chat':
      loadConversationsList();
      input.focus();
      break;
    case 'dashboard':
      loadDashboard();
      break;
    case 'calendar':
      loadCalendar();
      break;
    case 'focus':
      loadFocus();
      break;
    case 'notes':
      loadNotes();
      break;
    case 'work':
      loadWork();
      break;
    case 'development':
      loadDevelopment();
      break;
    case 'hobbies':
      loadHobbies();
      break;
    case 'sports':
      loadSports();
      break;
    case 'health':
      loadHealth();
      break;
    case 'integrations':
      if (!integrationsLoaded) loadIntegrations();
      break;
    case 'settings':
      loadSettings();
      break;
  }
}

// Keyboard shortcuts: Cmd+1..0 for tabs
document.addEventListener('keydown', (e) => {
  if (!e.metaKey && !e.ctrlKey) return;
  const tabMap = { '1': 'chat', '2': 'dashboard', '3': 'calendar', '4': 'focus', '5': 'notes',
                   '6': 'work', '7': 'development', '8': 'hobbies', '9': 'sports', '0': 'health' };
  if (tabMap[e.key]) {
    e.preventDefault();
    switchTab(tabMap[e.key]);
  }
});

document.querySelectorAll('.sidebar-btn').forEach(btn => {
  btn.addEventListener('click', () => switchTab(btn.dataset.tab));
});

// ── Chat helpers ──

function scrollDown() {
  chat.scrollTop = chat.scrollHeight;
}

function addMsg(role, text) {
  if (role === 'bot') {
    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const div = document.createElement('div');
    div.className = 'msg bot';
    div.textContent = text;
    wrapper.appendChild(div);
    // TTS button
    const ttsBtn = document.createElement('button');
    ttsBtn.className = 'tts-btn';
    ttsBtn.innerHTML = '&#9834;';
    ttsBtn.title = 'Озвучить';
    ttsBtn.addEventListener('click', () => toggleTTS(ttsBtn, text));
    wrapper.appendChild(ttsBtn);
    chat.appendChild(wrapper);
    scrollDown();
    return div;
  }
  const div = document.createElement('div');
  div.className = `msg ${role}`;
  div.textContent = text;
  chat.appendChild(div);
  scrollDown();
  return div;
}

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
  attachedFile = { name: file.name, content: text };
  attachPreview.textContent = `📎 ${file.name}`;
  attachPreview.style.display = 'block';
  fileInput.value = '';
});

attachPreview.addEventListener('click', () => {
  attachedFile = null;
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
  attachedFile = { name: file.name, content: text };
  attachPreview.textContent = `📎 ${file.name}`;
  attachPreview.style.display = 'block';
});

// ── Action parsing & execution ──

async function executeAction(actionJson) {
  try {
    const action = JSON.parse(actionJson);
    let result;

    switch (action.type) {
      case 'add_purchase':
        result = await invoke('tracker_add_purchase', {
          amount: action.amount,
          category: action.category || 'other',
          description: action.description || ''
        });
        break;
      case 'add_time':
        result = await invoke('tracker_add_time', {
          activity: action.activity || '',
          duration: action.duration || 0,
          category: action.category || 'other',
          productive: action.productive !== false
        });
        break;
      case 'add_goal':
        result = await invoke('tracker_add_goal', {
          title: action.title || '',
          category: action.category || 'other'
        });
        break;
      case 'add_note':
        result = await invoke('tracker_add_note', {
          title: action.title || '',
          content: action.content || ''
        });
        break;
      case 'get_stats':
        result = await invoke('tracker_get_stats');
        break;
      case 'get_activity':
        result = await invoke('get_activity_summary');
        break;
      case 'get_calendar':
        result = await invoke('get_calendar_events');
        break;
      case 'get_music':
        result = await invoke('get_now_playing');
        break;
      case 'get_browser':
        result = await invoke('get_browser_tab');
        break;
      case 'remember':
        result = await invoke('memory_remember', {
          category: action.category || 'user',
          key: action.key || '',
          value: action.value || ''
        });
        break;
      case 'recall':
        result = await invoke('memory_recall', {
          category: action.category || 'user',
          key: action.key || null
        });
        break;
      case 'forget':
        result = await invoke('memory_forget', {
          category: action.category || 'user',
          key: action.key || ''
        });
        break;
      case 'search_memory':
        result = await invoke('memory_search', {
          query: action.query || '',
          limit: action.limit || null
        });
        break;
      // Focus mode actions
      case 'start_focus':
        result = await invoke('start_focus', {
          durationMinutes: action.duration || 30,
          apps: action.apps || null,
          sites: action.sites || null,
        });
        break;
      case 'stop_focus':
        result = await invoke('stop_focus');
        break;
      // System actions
      case 'run_shell':
        result = await invoke('run_shell', { command: action.command || '' });
        break;
      case 'open_url':
        result = await invoke('open_url', { url: action.url || '' });
        break;
      case 'send_notification':
        result = await invoke('send_notification', {
          title: action.title || 'Hanni',
          body: action.body || ''
        });
        break;
      case 'set_volume':
        result = await invoke('set_volume', { level: action.level || 50 });
        break;
      case 'get_clipboard':
        result = await invoke('get_clipboard');
        break;
      case 'set_clipboard':
        result = await invoke('set_clipboard', { text: action.text || '' });
        break;
      default:
        result = 'Unknown action: ' + action.type;
    }

    return { success: true, result };
  } catch (e) {
    return { success: false, result: String(e) };
  }
}

function parseAndExecuteActions(text) {
  const actionRegex = /```action\s*\n([\s\S]*?)\n```/g;
  let match;
  const actions = [];

  while ((match = actionRegex.exec(text)) !== null) {
    actions.push(match[1].trim());
  }

  return actions;
}

// ── Stream chat helper ──

async function streamChat(botDiv, t0) {
  const cursor = document.createElement('span');
  cursor.className = 'cursor';
  botDiv.appendChild(cursor);

  let firstToken = 0;
  let tokens = 0;
  let fullReply = '';

  const unlisten = await listen('chat-token', (event) => {
    if (!firstToken) firstToken = performance.now() - t0;
    tokens++;
    const token = event.payload.token;
    fullReply += token;
    botDiv.insertBefore(document.createTextNode(token), cursor);
    scrollDown();
  });

  const unlistenDone = await listen('chat-done', () => {});

  try {
    const msgs = history.slice(-30);
    await invoke('chat', { messages: msgs });
  } catch (e) {
    if (!fullReply) {
      botDiv.textContent = 'MLX сервер недоступен';
    }
  }

  unlisten();
  unlistenDone();
  cursor.remove();

  return { fullReply, tokens, firstToken };
}

// ── Agent step indicator ──

function showAgentIndicator(step) {
  const div = document.createElement('div');
  div.className = 'agent-step';
  div.textContent = `шаг ${step}...`;
  chat.appendChild(div);
  scrollDown();
  return div;
}

// ── TTS ──

async function toggleTTS(btn, text) {
  if (isSpeaking) {
    await invoke('stop_speaking').catch(() => {});
    document.querySelectorAll('.tts-btn.speaking').forEach(b => b.classList.remove('speaking'));
    isSpeaking = false;
    return;
  }
  isSpeaking = true;
  btn.classList.add('speaking');
  try {
    // Get voice from proactive settings
    let voice = 'Milena';
    try {
      const ps = await invoke('get_proactive_settings');
      voice = ps.voice_name || 'Milena';
    } catch (_) {}
    await invoke('speak_text', { text, voice });
    // Auto-clear speaking state after estimated duration
    const wordCount = text.split(/\s+/).length;
    const durationMs = Math.max(2000, wordCount * 300);
    setTimeout(() => {
      btn.classList.remove('speaking');
      isSpeaking = false;
    }, durationMs);
  } catch (_) {
    btn.classList.remove('speaking');
    isSpeaking = false;
  }
}

// ── Send message ──

async function send() {
  const text = input.value.trim();
  if (!text || busy) return;

  busy = true;
  sendBtn.disabled = true;
  input.value = '';

  // Build message with optional file
  let userContent = text;
  if (attachedFile) {
    userContent += `\n\n📎 Файл: ${attachedFile.name}\n\`\`\`\n${attachedFile.content}\n\`\`\``;
    addMsg('user', `${text}\n📎 ${attachedFile.name}`);
    attachedFile = null;
    attachPreview.style.display = 'none';
  } else {
    addMsg('user', text);
  }

  history.push(['user', userContent]);

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

    if (!result.fullReply) break;

    history.push(['assistant', result.fullReply]);

    // Parse actions from response
    const actions = parseAndExecuteActions(result.fullReply);

    // No actions — this is the final answer, stop the loop
    if (actions.length === 0) break;

    // Mark this response as intermediate (it contained actions, not a final answer)
    botDiv.classList.add('intermediate');

    // Execute actions and show results
    const results = [];
    for (const actionJson of actions) {
      const { success, result: actionResult } = await executeAction(actionJson);
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
      scrollDown();
      results.push(actionResult);
    }

    // Feed results back into history so the model sees them
    history.push(['user', `[Action result: ${results.join('; ')}]`]);
  }

  // Add TTS button to last bot wrapper
  const lastWrapper = chat.querySelector('.msg-wrapper:last-of-type');
  if (lastWrapper && !lastWrapper.querySelector('.tts-btn')) {
    const lastBotDiv = lastWrapper.querySelector('.msg.bot');
    if (lastBotDiv) {
      const ttsBtn = document.createElement('button');
      ttsBtn.className = 'tts-btn';
      ttsBtn.innerHTML = '&#9834;';
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
  const stepInfo = iteration > 1 ? ` · ${iteration} steps` : '';
  timing.textContent = `${ttft}s first token · ${total}s total · ${totalTokens} tokens${stepInfo}`;
  chat.appendChild(timing);
  scrollDown();

  // Post-chat: incremental save + extract facts in background
  (async () => {
    try {
      if (currentConversationId) {
        await invoke('update_conversation', { id: currentConversationId, messages: history });
      } else {
        currentConversationId = await invoke('save_conversation', { messages: history });
      }
      if (history.length >= 4) {
        await invoke('process_conversation_end', { messages: history, conversationId: currentConversationId });
      }
      loadConversationsList();
    } catch (_) {}
  })();

  busy = false;
  sendBtn.disabled = false;
  input.focus();
}

// ── New Chat ──

async function newChat() {
  if (busy) return;
  // Save current conversation before clearing
  await autoSaveConversation();
  currentConversationId = null;
  history = [];
  chat.innerHTML = '';
  loadConversationsList();
  input.focus();
}

document.getElementById('new-chat')?.addEventListener('click', newChat);

sendBtn.addEventListener('click', send);
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    send();
  }
});

// ── Integrations page ──

function panelItem(item) {
  return `<div class="panel-item">
    <span class="panel-dot ${item.status}"></span>
    <div class="panel-item-info">
      <div class="panel-item-name">${item.name}</div>
      <div class="panel-item-detail">${item.detail}</div>
    </div>
  </div>`;
}

async function loadIntegrations(force) {
  if (!force) integrationsContent.innerHTML = '<div style="color:#555;font-size:13px;">Загрузка...</div>';
  try {
    const info = await invoke('get_integrations');

    let accessItems = '';
    for (const item of info.access) accessItems += panelItem(item);

    let trackingItems = '';
    for (const item of info.tracking) trackingItems += panelItem(item);

    let appsItems = '';
    for (const item of info.blocked_apps) appsItems += panelItem(item);

    let sitesItems = '';
    for (const item of info.blocked_sites) sitesItems += panelItem(item);

    const blockerBadge = `<span class="panel-status-badge ${info.blocker_active ? 'on' : 'off'}">${info.blocker_active ? 'Активна' : 'Неактивна'}</span>`;

    let macosItems = '';
    if (info.macos) {
      for (const item of info.macos) macosItems += panelItem(item);
    }

    const legend = `<div class="macos-legend">
      <span><span class="legend-dot active"></span> Активно</span>
      <span><span class="legend-dot ready"></span> По запросу</span>
      <span><span class="legend-dot inactive"></span> Ожидание</span>
    </div>`;

    integrationsContent.innerHTML = `
      <div class="integrations-grid">
        <div class="integration-card macos-card">
          <div class="integration-card-title">macOS интеграции</div>
          ${legend}
          ${macosItems}
        </div>
        <div class="integration-card">
          <div class="integration-card-title">Доступ</div>
          ${accessItems}
        </div>
        <div class="integration-card">
          <div class="integration-card-title">Трекинг</div>
          ${trackingItems}
        </div>
        <div class="integration-card">
          <div class="integration-card-title">Приложения</div>
          ${blockerBadge}
          ${appsItems}
        </div>
        <div class="integration-card">
          <div class="integration-card-title">Сайты</div>
          ${blockerBadge}
          ${sitesItems}
        </div>
        <div class="integration-card macos-card">
          <div class="integration-card-title">Память Hanni</div>
          <div class="memory-search-box">
            <input class="form-input" id="memory-search" placeholder="Поиск по памяти..." autocomplete="off">
          </div>
          <div class="memory-browser" id="memory-browser-list"></div>
        </div>
      </div>`;

    // Load memory browser
    loadMemoryBrowser();

    integrationsLoaded = true;
  } catch (e) {
    integrationsContent.innerHTML = `<div style="color:#f87171;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

// ── Memory browser ──

async function loadMemoryBrowser(search) {
  const list = document.getElementById('memory-browser-list');
  if (!list) return;
  try {
    const memories = await invoke('get_all_memories', { search: search || null });
    list.innerHTML = '';
    for (const m of memories) {
      const item = document.createElement('div');
      item.className = 'memory-item';
      item.innerHTML = `
        <span class="memory-item-category">${escapeHtml(m.category)}</span>
        <span class="memory-item-key">${escapeHtml(m.key)}</span>
        <span class="memory-item-value">${escapeHtml(m.value)}</span>
        <div class="memory-item-actions">
          <button class="memory-item-btn" data-id="${m.id}" title="Удалить">&times;</button>
        </div>`;
      item.querySelector('.memory-item-btn').addEventListener('click', async (e) => {
        if (confirm(`Удалить "${m.key}"?`)) {
          await invoke('delete_memory', { id: m.id }).catch(() => {});
          loadMemoryBrowser(search);
        }
      });
      list.appendChild(item);
    }
    if (memories.length === 0) {
      list.innerHTML = '<div style="color:#555;font-size:12px;padding:8px;">Нет фактов</div>';
    }
  } catch (_) {}

  // Bind search
  document.getElementById('memory-search')?.addEventListener('input', (e) => {
    clearTimeout(convSearchTimeout);
    convSearchTimeout = setTimeout(() => loadMemoryBrowser(e.target.value), 300);
  });
}

// ── Settings page ──

async function loadSettings() {
  settingsContent.innerHTML = '<div style="color:#555;font-size:13px;">Загрузка...</div>';
  try {
    const [info, proactive, trainingStats] = await Promise.all([
      invoke('get_model_info'),
      invoke('get_proactive_settings'),
      invoke('get_training_stats').catch(() => ({ conversations: 0, total_messages: 0 })),
    ]);
    settingsContent.innerHTML = `
      <div class="settings-section">
        <div class="settings-section-title">Модель</div>
        <div class="settings-row">
          <span class="settings-label">Название</span>
          <span class="settings-value">${info.model_name}</span>
        </div>
        <div class="settings-row">
          <span class="settings-label">Сервер</span>
          <span class="settings-value">${info.server_url}</span>
        </div>
        <div class="settings-row">
          <span class="settings-label">Статус</span>
          <span class="settings-value ${info.server_online ? 'online' : 'offline'}">${info.server_online ? 'Онлайн' : 'Офлайн'}</span>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Автономный режим</div>
        <div class="settings-row">
          <span class="settings-label">Включён</span>
          <label class="toggle">
            <input type="checkbox" id="proactive-enabled" ${proactive.enabled ? 'checked' : ''}>
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="settings-row">
          <span class="settings-label">Голос</span>
          <label class="toggle">
            <input type="checkbox" id="proactive-voice" ${proactive.voice_enabled ? 'checked' : ''}>
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="settings-row">
          <span class="settings-label">Голос TTS</span>
          <div class="pill-group" id="proactive-voice-name">
            ${['Milena', 'Yuri', 'Samantha', 'Daniel'].map(v =>
              `<button class="pill${proactive.voice_name === v ? ' active' : ''}" data-value="${v}">${v}</button>`
            ).join('')}
          </div>
        </div>
        <div class="settings-row">
          <span class="settings-label">Интервал</span>
          <div class="pill-group" id="proactive-interval">
            ${[5, 10, 15, 30].map(v =>
              `<button class="pill${proactive.interval_minutes === v ? ' active' : ''}" data-value="${v}">${v}м</button>`
            ).join('')}
          </div>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Тренировочные данные</div>
        <div class="settings-row">
          <span class="settings-label">Диалогов</span>
          <span class="settings-value">${trainingStats.conversations}</span>
        </div>
        <div class="settings-row">
          <span class="settings-label">Сообщений</span>
          <span class="settings-value">${trainingStats.total_messages}</span>
        </div>
        <div class="settings-row">
          <span class="settings-label">Экспорт</span>
          <button class="settings-btn" id="export-training-btn">Экспорт JSONL</button>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">HTTP API</div>
        <div class="settings-row">
          <span class="settings-label">Адрес</span>
          <span class="settings-value">127.0.0.1:8235</span>
        </div>
        <div class="settings-row">
          <span class="settings-label">Статус</span>
          <span class="settings-value" id="api-status">Проверяю...</span>
        </div>
      </div>`;

    // Proactive settings change handlers
    const getProactiveValues = () => ({
      enabled: document.getElementById('proactive-enabled').checked,
      voice_enabled: document.getElementById('proactive-voice').checked,
      voice_name: document.querySelector('#proactive-voice-name .pill.active')?.dataset.value || 'Milena',
      interval_minutes: parseInt(document.querySelector('#proactive-interval .pill.active')?.dataset.value || '10'),
      quiet_hours_start: 23,
      quiet_hours_end: 8,
    });
    const saveProactive = () => invoke('set_proactive_settings', { settings: getProactiveValues() }).catch(() => {});

    document.getElementById('proactive-enabled')?.addEventListener('change', saveProactive);
    document.getElementById('proactive-voice')?.addEventListener('change', saveProactive);

    // Pill group click handlers
    document.querySelectorAll('.pill-group').forEach(group => {
      group.addEventListener('click', (e) => {
        const pill = e.target.closest('.pill');
        if (!pill) return;
        group.querySelectorAll('.pill').forEach(p => p.classList.remove('active'));
        pill.classList.add('active');
        saveProactive();
      });
    });

    // Training data export
    document.getElementById('export-training-btn')?.addEventListener('click', async (e) => {
      const btn = e.target;
      btn.textContent = 'Экспорт...';
      btn.disabled = true;
      try {
        const result = await invoke('export_training_data');
        btn.textContent = `${result.train_count} train + ${result.valid_count} valid`;
      } catch (err) {
        btn.textContent = String(err).substring(0, 30);
      }
      setTimeout(() => { btn.textContent = 'Экспорт JSONL'; btn.disabled = false; }, 4000);
    });

    // Check API status
    try {
      const resp = await fetch('http://127.0.0.1:8235/api/status');
      const apiEl = document.getElementById('api-status');
      if (apiEl) apiEl.textContent = resp.ok ? 'Активен' : 'Недоступен';
      if (apiEl) apiEl.className = 'settings-value ' + (resp.ok ? 'online' : 'offline');
    } catch (_) {
      const apiEl = document.getElementById('api-status');
      if (apiEl) { apiEl.textContent = 'Недоступен'; apiEl.className = 'settings-value offline'; }
    }

  } catch (e) {
    settingsContent.innerHTML = `<div style="color:#f87171;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

// ── Tab loaders (stubs until implemented) ──

function showStub(containerId, icon, label) {
  const el = document.getElementById(containerId);
  if (el) el.innerHTML = `<div class="tab-stub"><div class="tab-stub-icon">${icon}</div>${label}</div>`;
}

// ── Dashboard ──
async function loadDashboard() {
  const el = document.getElementById('dashboard-content');
  if (!el) return;
  el.innerHTML = '<div style="color:#555;font-size:13px;">Загрузка...</div>';
  try {
    const data = await invoke('get_dashboard_data');
    const now = new Date();
    const dateStr = now.toLocaleDateString('ru-RU', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
    const greeting = now.getHours() < 12 ? 'Доброе утро' : now.getHours() < 18 ? 'Добрый день' : 'Добрый вечер';

    let focusBanner = '';
    if (data.current_activity) {
      focusBanner = `<div class="dashboard-focus-banner">
        <div class="dashboard-focus-indicator"></div>
        <div class="dashboard-focus-text">${escapeHtml(data.current_activity.title)}</div>
        <div class="dashboard-focus-time">${data.current_activity.elapsed || ''}</div>
      </div>`;
    }

    el.innerHTML = `
      <div class="dashboard-greeting">${greeting}!</div>
      <div class="dashboard-date">${dateStr}</div>
      ${focusBanner}
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${data.activities_today || 0}</div><div class="dashboard-stat-label">Активности</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${data.focus_minutes || 0}м</div><div class="dashboard-stat-label">Фокус</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${data.notes_count || 0}</div><div class="dashboard-stat-label">Заметки</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${data.events_today || 0}</div><div class="dashboard-stat-label">События</div></div>
      </div>
      ${data.events && data.events.length > 0 ? `
        <div class="dashboard-section-title">События сегодня</div>
        ${data.events.map(e => `<div class="calendar-event-item">
          <span class="calendar-event-time">${e.time || ''}</span>
          <span class="calendar-event-title">${escapeHtml(e.title)}</span>
        </div>`).join('')}` : ''}
      ${data.recent_notes && data.recent_notes.length > 0 ? `
        <div class="dashboard-section-title">Последние заметки</div>
        ${data.recent_notes.map(n => `<div class="calendar-event-item">
          <span class="calendar-event-title">${escapeHtml(n.title)}</span>
        </div>`).join('')}` : ''}
      <div class="dashboard-section-title">Быстрые действия</div>
      <div class="dashboard-quick-actions">
        <button class="btn-primary" onclick="switchTab('notes')">Новая заметка</button>
        <button class="btn-primary" onclick="switchTab('focus')">Начать активность</button>
        <button class="btn-primary" onclick="switchTab('health')">Залогировать</button>
      </div>`;
  } catch (e) {
    // Fallback for when backend command doesn't exist yet
    const now = new Date();
    const dateStr = now.toLocaleDateString('ru-RU', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
    const greeting = now.getHours() < 12 ? 'Доброе утро' : now.getHours() < 18 ? 'Добрый день' : 'Добрый вечер';
    el.innerHTML = `
      <div class="dashboard-greeting">${greeting}!</div>
      <div class="dashboard-date">${dateStr}</div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">0</div><div class="dashboard-stat-label">Активности</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">0м</div><div class="dashboard-stat-label">Фокус</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">0</div><div class="dashboard-stat-label">Заметки</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">0</div><div class="dashboard-stat-label">События</div></div>
      </div>
      <div class="dashboard-section-title">Быстрые действия</div>
      <div class="dashboard-quick-actions">
        <button class="btn-primary" onclick="switchTab('notes')">Новая заметка</button>
        <button class="btn-primary" onclick="switchTab('focus')">Начать активность</button>
        <button class="btn-primary" onclick="switchTab('health')">Залогировать</button>
      </div>`;
  }
}

// ── Focus ──
let focusTimerInterval = null;

async function loadFocus() {
  const el = document.getElementById('focus-content');
  if (!el) return;
  el.innerHTML = '<div style="color:#555;font-size:13px;">Загрузка...</div>';
  try {
    const current = await invoke('get_current_activity').catch(() => null);
    const log = await invoke('get_activity_log', { date: null }).catch(() => []);

    let currentHtml = '';
    if (current) {
      currentHtml = `<div class="focus-current">
        <div class="focus-current-title">Сейчас</div>
        <div class="focus-current-activity">${escapeHtml(current.title)}</div>
        <div class="focus-current-timer" id="focus-timer">${current.elapsed || '00:00'}</div>
        <div style="margin-top:12px;display:flex;gap:8px;justify-content:center;">
          <button class="btn-danger" id="stop-activity-btn">Завершить</button>
        </div>
      </div>`;
    } else {
      currentHtml = `<div class="focus-current">
        <div class="focus-current-title">Чем занимаешься?</div>
        <div style="margin-top:12px;">
          <input id="activity-title" class="form-input" placeholder="Название активности..." style="max-width:300px;margin:0 auto 12px;display:block;text-align:center;">
          <div class="focus-presets" id="activity-presets">
            <button class="focus-preset" data-category="work">Работа</button>
            <button class="focus-preset" data-category="study">Учёба</button>
            <button class="focus-preset" data-category="sport">Спорт</button>
            <button class="focus-preset" data-category="rest">Отдых</button>
            <button class="focus-preset" data-category="hobby">Хобби</button>
            <button class="focus-preset" data-category="other">Другое</button>
          </div>
          <div style="display:flex;gap:8px;justify-content:center;align-items:center;margin-top:8px;">
            <label style="font-size:12px;color:#888;display:flex;align-items:center;gap:6px;">
              <input type="checkbox" id="focus-block-check"> Блокировать отвлечения
            </label>
          </div>
          <button class="btn-primary" id="start-activity-btn" style="margin-top:12px;">Начать</button>
        </div>
      </div>`;
    }

    const logHtml = log.length > 0 ? `
      <div class="module-card-title" style="margin-top:8px;">Лог за сегодня</div>
      <div class="focus-log">
        ${log.map(item => `<div class="focus-log-item">
          <span class="focus-log-time">${item.time || ''}</span>
          <span class="focus-log-title">${escapeHtml(item.title)}</span>
          <span class="focus-log-category">${item.category || ''}</span>
          <span class="focus-log-duration">${item.duration || ''}</span>
        </div>`).join('')}
      </div>` : '';

    el.innerHTML = currentHtml + logHtml;

    // Bind events
    let selectedCategory = 'other';
    document.querySelectorAll('#activity-presets .focus-preset')?.forEach(btn => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('#activity-presets .focus-preset').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        selectedCategory = btn.dataset.category;
        const titleInput = document.getElementById('activity-title');
        if (titleInput && !titleInput.value) titleInput.value = btn.textContent;
      });
    });

    document.getElementById('start-activity-btn')?.addEventListener('click', async () => {
      const title = document.getElementById('activity-title')?.value?.trim() || selectedCategory;
      const focusMode = document.getElementById('focus-block-check')?.checked || false;
      try {
        await invoke('start_activity', { title, category: selectedCategory, focusMode, duration: null, apps: null, sites: null });
        loadFocus();
      } catch (err) {
        alert('Ошибка: ' + err);
      }
    });

    document.getElementById('stop-activity-btn')?.addEventListener('click', async () => {
      try {
        await invoke('stop_activity');
        loadFocus();
      } catch (err) {
        alert('Ошибка: ' + err);
      }
    });

    // Update timer
    if (current && current.started_at) {
      if (focusTimerInterval) clearInterval(focusTimerInterval);
      const startedAt = new Date(current.started_at).getTime();
      focusTimerInterval = setInterval(() => {
        const timerEl = document.getElementById('focus-timer');
        if (!timerEl || currentTab !== 'focus') { clearInterval(focusTimerInterval); return; }
        const elapsed = Math.floor((Date.now() - startedAt) / 1000);
        const h = Math.floor(elapsed / 3600);
        const m = Math.floor((elapsed % 3600) / 60);
        const s = elapsed % 60;
        timerEl.textContent = h > 0 ? `${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}` : `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
      }, 1000);
    }
  } catch (e) {
    showStub('focus-content', '&#9673;', 'Фокус — скоро');
  }
}

// ── Notes ──
let currentNoteId = null;
let noteAutoSaveTimeout = null;

async function loadNotes() {
  const el = document.getElementById('notes-content');
  if (!el) return;
  try {
    const notes = await invoke('get_notes', { filter: null, search: null });
    const notesList = notes || [];

    el.innerHTML = `<div class="notes-layout">
      <div class="notes-list">
        <div class="notes-list-header">
          <input class="form-input" id="notes-search" placeholder="Поиск заметок..." autocomplete="off">
          <button class="btn-primary" id="new-note-btn" style="width:100%;">+ Новая заметка</button>
        </div>
        <div class="notes-list-items" id="notes-list-items"></div>
      </div>
      <div class="notes-editor" id="notes-editor-panel">
        <div class="notes-empty">Выберите заметку или создайте новую</div>
      </div>
    </div>`;

    renderNotesList(notesList);

    document.getElementById('new-note-btn')?.addEventListener('click', async () => {
      try {
        const id = await invoke('create_note', { title: 'Без названия', content: '', tags: '' });
        currentNoteId = id;
        loadNotes();
      } catch (err) { alert('Ошибка: ' + err); }
    });

    document.getElementById('notes-search')?.addEventListener('input', async (e) => {
      clearTimeout(noteAutoSaveTimeout);
      noteAutoSaveTimeout = setTimeout(async () => {
        try {
          const results = await invoke('get_notes', { filter: null, search: e.target.value || null });
          renderNotesList(results || []);
        } catch (_) {}
      }, 300);
    });
  } catch (e) {
    showStub('notes-content', '&#9998;', 'Заметки — скоро');
  }
}

function renderNotesList(notes) {
  const list = document.getElementById('notes-list-items');
  if (!list) return;
  list.innerHTML = '';
  for (const note of notes) {
    const item = document.createElement('div');
    item.className = 'note-list-item' + (note.id === currentNoteId ? ' active' : '');
    const date = new Date(note.updated_at || note.created_at);
    const dateStr = date.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' });
    item.innerHTML = `
      <div class="note-list-item-title">${note.pinned ? '<span class="note-pinned-icon">&#x1F4CC;</span> ' : ''}${escapeHtml(note.title || 'Без названия')}</div>
      <div class="note-list-item-preview">${escapeHtml((note.content || '').substring(0, 60))}</div>
      <div class="note-list-item-meta">${dateStr}</div>`;
    item.addEventListener('click', () => openNote(note.id));
    list.appendChild(item);
  }
}

async function openNote(id) {
  currentNoteId = id;
  try {
    const note = await invoke('get_note', { id });
    const panel = document.getElementById('notes-editor-panel');
    if (!panel) return;
    panel.innerHTML = `
      <input class="notes-editor-title" id="note-title" value="${escapeHtml(note.title || '')}" placeholder="Заголовок...">
      <div class="notes-editor-tags">
        ${(note.tags || '').split(',').filter(t => t.trim()).map(t =>
          `<span class="note-tag">${escapeHtml(t.trim())}</span>`
        ).join('')}
        <input class="form-input" id="note-tags-input" placeholder="Теги через запятую..." value="${escapeHtml(note.tags || '')}" style="font-size:11px;padding:2px 8px;width:auto;min-width:120px;">
      </div>
      <textarea class="notes-editor-body" id="note-body" placeholder="Начните писать...">${escapeHtml(note.content || '')}</textarea>
      <div class="notes-editor-actions">
        <button class="btn-secondary" id="note-pin-btn">${note.pinned ? 'Открепить' : 'Закрепить'}</button>
        <button class="btn-secondary" id="note-archive-btn">${note.archived ? 'Разархивировать' : 'Архивировать'}</button>
        <button class="btn-danger" id="note-delete-btn">Удалить</button>
      </div>`;

    // Auto-save on typing
    const autoSave = () => {
      clearTimeout(noteAutoSaveTimeout);
      noteAutoSaveTimeout = setTimeout(async () => {
        const title = document.getElementById('note-title')?.value || '';
        const content = document.getElementById('note-body')?.value || '';
        const tags = document.getElementById('note-tags-input')?.value || '';
        await invoke('update_note', { id, title, content, tags }).catch(() => {});
      }, 1000);
    };
    document.getElementById('note-title')?.addEventListener('input', autoSave);
    document.getElementById('note-body')?.addEventListener('input', autoSave);
    document.getElementById('note-tags-input')?.addEventListener('input', autoSave);

    document.getElementById('note-pin-btn')?.addEventListener('click', async () => {
      await invoke('update_note', { id, title: note.title, content: note.content, tags: note.tags, pinned: !note.pinned }).catch(() => {});
      loadNotes();
    });
    document.getElementById('note-archive-btn')?.addEventListener('click', async () => {
      await invoke('update_note', { id, title: note.title, content: note.content, tags: note.tags, archived: !note.archived }).catch(() => {});
      loadNotes();
    });
    document.getElementById('note-delete-btn')?.addEventListener('click', async () => {
      if (confirm('Удалить заметку?')) {
        await invoke('delete_note', { id }).catch(() => {});
        currentNoteId = null;
        loadNotes();
      }
    });

    // Highlight active item in list
    document.querySelectorAll('.note-list-item').forEach(item => item.classList.remove('active'));
    document.querySelectorAll('.note-list-item').forEach((item, idx) => {
      // Re-select the clicked item
    });
  } catch (e) {
    alert('Ошибка: ' + e);
  }
}

// ── Calendar ──
let calendarYear = new Date().getFullYear();
let calendarMonth = new Date().getMonth();
let selectedCalendarDate = null;

async function loadCalendar() {
  const el = document.getElementById('calendar-content');
  if (!el) return;
  try {
    const events = await invoke('get_events', { month: calendarMonth + 1, year: calendarYear }).catch(() => []);
    renderCalendar(el, events || []);
  } catch (e) {
    renderCalendar(el, []);
  }
}

function renderCalendar(el, events) {
  const monthNames = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
  const weekdays = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];

  const firstDay = new Date(calendarYear, calendarMonth, 1);
  const lastDay = new Date(calendarYear, calendarMonth + 1, 0);
  let startDay = firstDay.getDay() - 1;
  if (startDay < 0) startDay = 6;

  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;

  // Group events by date
  const eventsByDate = {};
  for (const ev of events) {
    const d = ev.date;
    if (!eventsByDate[d]) eventsByDate[d] = [];
    eventsByDate[d].push(ev);
  }

  let daysHtml = '';
  // Prev month days
  const prevLast = new Date(calendarYear, calendarMonth, 0).getDate();
  for (let i = startDay - 1; i >= 0; i--) {
    daysHtml += `<div class="calendar-day other-month"><span class="calendar-day-number">${prevLast - i}</span></div>`;
  }
  // Current month days
  for (let d = 1; d <= lastDay.getDate(); d++) {
    const dateStr = `${calendarYear}-${String(calendarMonth+1).padStart(2,'0')}-${String(d).padStart(2,'0')}`;
    const isToday = dateStr === todayStr;
    const isSelected = dateStr === selectedCalendarDate;
    const dayEvents = eventsByDate[dateStr] || [];
    const dots = dayEvents.slice(0, 3).map(e => `<span class="calendar-event-dot" style="background:${e.color || '#818cf8'}"></span>`).join('');
    daysHtml += `<div class="calendar-day${isToday ? ' today' : ''}${isSelected ? ' selected' : ''}" data-date="${dateStr}">
      <span class="calendar-day-number">${d}</span>
      <div class="calendar-day-dots">${dots}</div>
    </div>`;
  }
  // Next month days
  const totalCells = startDay + lastDay.getDate();
  const remaining = totalCells % 7 === 0 ? 0 : 7 - (totalCells % 7);
  for (let i = 1; i <= remaining; i++) {
    daysHtml += `<div class="calendar-day other-month"><span class="calendar-day-number">${i}</span></div>`;
  }

  let dayPanelHtml = '';
  if (selectedCalendarDate && eventsByDate[selectedCalendarDate]) {
    const dayEvts = eventsByDate[selectedCalendarDate];
    dayPanelHtml = `<div class="calendar-day-panel">
      <div class="calendar-day-panel-title">${selectedCalendarDate}</div>
      ${dayEvts.map(e => `<div class="calendar-event-item">
        <span class="calendar-event-time">${e.time || ''}</span>
        <span class="calendar-event-title">${escapeHtml(e.title)}</span>
      </div>`).join('')}
    </div>`;
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="cal-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[calendarMonth]} ${calendarYear}</div>
      <button class="calendar-nav-btn" id="cal-next">&gt;</button>
      <button class="btn-primary" id="cal-add-event" style="margin-left:16px;">+ Событие</button>
    </div>
    <div class="calendar-weekdays">${weekdays.map(d => `<div class="calendar-weekday">${d}</div>`).join('')}</div>
    <div class="calendar-grid">${daysHtml}</div>
    ${dayPanelHtml}`;

  document.getElementById('cal-prev')?.addEventListener('click', () => {
    calendarMonth--;
    if (calendarMonth < 0) { calendarMonth = 11; calendarYear--; }
    loadCalendar();
  });
  document.getElementById('cal-next')?.addEventListener('click', () => {
    calendarMonth++;
    if (calendarMonth > 11) { calendarMonth = 0; calendarYear++; }
    loadCalendar();
  });
  document.querySelectorAll('.calendar-day:not(.other-month)').forEach(day => {
    day.addEventListener('click', () => {
      selectedCalendarDate = day.dataset.date;
      loadCalendar();
    });
  });
  document.getElementById('cal-add-event')?.addEventListener('click', () => showAddEventModal());
}

function showAddEventModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Новое событие</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="event-title"></div>
    <div class="form-group"><label class="form-label">Дата</label><input class="form-input" id="event-date" type="date" value="${selectedCalendarDate || new Date().toISOString().split('T')[0]}"></div>
    <div class="form-group"><label class="form-label">Время</label><input class="form-input" id="event-time" type="time"></div>
    <div class="form-group"><label class="form-label">Описание</label><textarea class="form-textarea" id="event-desc"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="event-cancel">Отмена</button>
      <button class="btn-primary" id="event-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('event-cancel')?.addEventListener('click', () => overlay.remove());
  document.getElementById('event-save')?.addEventListener('click', async () => {
    const title = document.getElementById('event-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_event', {
        title,
        description: document.getElementById('event-desc')?.value || '',
        date: document.getElementById('event-date')?.value || '',
        time: document.getElementById('event-time')?.value || '',
        durationMinutes: 60,
        category: 'general',
        color: '#818cf8',
      });
      overlay.remove();
      loadCalendar();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Work ──
let currentProjectId = null;

async function loadWork() {
  const el = document.getElementById('work-content');
  if (!el) return;
  try {
    const projects = await invoke('get_projects').catch(() => []);
    renderWork(el, projects || []);
  } catch (e) {
    showStub('work-content', '&#9642;', 'Работа — скоро');
  }
}

async function renderWork(el, projects) {
  if (!currentProjectId && projects.length > 0) currentProjectId = projects[0].id;
  const tasks = currentProjectId ? await invoke('get_tasks', { projectId: currentProjectId }).catch(() => []) : [];

  el.innerHTML = `<div class="work-layout">
    <div class="work-projects">
      <div class="work-projects-header">
        <button class="btn-primary" id="new-project-btn" style="width:100%;">+ Проект</button>
      </div>
      <div class="work-projects-list" id="work-projects-list"></div>
    </div>
    <div class="work-tasks">
      <div class="work-tasks-header">
        <h2 style="font-size:16px;color:#fff;">${currentProjectId ? escapeHtml(projects.find(p => p.id === currentProjectId)?.name || '') : 'Выберите проект'}</h2>
        ${currentProjectId ? '<button class="btn-primary" id="new-task-btn">+ Задача</button>' : ''}
      </div>
      <div id="work-tasks-list"></div>
    </div>
  </div>`;

  const projectList = document.getElementById('work-projects-list');
  for (const p of projects) {
    const item = document.createElement('div');
    item.className = 'work-project-item' + (p.id === currentProjectId ? ' active' : '');
    const taskCount = (p.task_count || 0);
    item.innerHTML = `<span class="work-project-dot" style="background:${p.color || '#818cf8'}"></span>
      <span class="work-project-name">${escapeHtml(p.name)}</span>
      <span class="work-project-count">${taskCount}</span>`;
    item.addEventListener('click', () => { currentProjectId = p.id; loadWork(); });
    projectList.appendChild(item);
  }

  const taskList = document.getElementById('work-tasks-list');
  for (const t of (tasks || [])) {
    const item = document.createElement('div');
    item.className = 'work-task-item';
    const isDone = t.status === 'done';
    const priorityClass = `priority-${t.priority || 'normal'}`;
    item.innerHTML = `
      <div class="work-task-check${isDone ? ' done' : ''}" data-id="${t.id}"></div>
      <span class="work-task-title${isDone ? ' done' : ''}">${escapeHtml(t.title)}</span>
      <span class="work-task-priority ${priorityClass}">${t.priority || 'normal'}</span>`;
    item.querySelector('.work-task-check').addEventListener('click', async () => {
      const newStatus = isDone ? 'todo' : 'done';
      await invoke('update_task_status', { id: t.id, status: newStatus }).catch(() => {});
      loadWork();
    });
    taskList.appendChild(item);
  }

  document.getElementById('new-project-btn')?.addEventListener('click', () => {
    const name = prompt('Название проекта:');
    if (name) invoke('create_project', { name, description: '', color: '#818cf8' }).then(() => loadWork()).catch(e => alert(e));
  });

  document.getElementById('new-task-btn')?.addEventListener('click', () => {
    const title = prompt('Задача:');
    if (title) invoke('create_task', { projectId: currentProjectId, title, description: '', priority: 'normal', dueDate: null }).then(() => loadWork()).catch(e => alert(e));
  });
}

// ── Development ──
let devFilter = 'all';

async function loadDevelopment() {
  const el = document.getElementById('development-content');
  if (!el) return;
  try {
    const items = await invoke('get_learning_items', { typeFilter: devFilter === 'all' ? null : devFilter }).catch(() => []);
    renderDevelopment(el, items || []);
  } catch (e) {
    showStub('development-content', '&#9636;', 'Развитие — скоро');
  }
}

function renderDevelopment(el, items) {
  const filters = ['all', 'course', 'book', 'skill', 'article'];
  const filterLabels = { all: 'Все', course: 'Курсы', book: 'Книги', skill: 'Навыки', article: 'Статьи' };
  const statusLabels = { planned: 'Запланировано', in_progress: 'В процессе', completed: 'Завершено' };
  const statusColors = { planned: 'badge-gray', in_progress: 'badge-blue', completed: 'badge-green' };

  el.innerHTML = `
    <div class="module-header"><h2>Развитие</h2><button class="btn-primary" id="dev-add-btn">+ Добавить</button></div>
    <div class="dev-filters">
      ${filters.map(f => `<button class="pill${devFilter === f ? ' active' : ''}" data-filter="${f}">${filterLabels[f]}</button>`).join('')}
    </div>
    <div class="dev-grid" id="dev-grid"></div>`;

  const grid = document.getElementById('dev-grid');
  for (const item of items) {
    const card = document.createElement('div');
    card.className = 'dev-card';
    card.innerHTML = `
      <div class="dev-card-title">${escapeHtml(item.title)}</div>
      <div class="dev-card-meta">
        <span class="badge ${statusColors[item.status] || 'badge-gray'}">${statusLabels[item.status] || item.status}</span>
        <span class="badge badge-purple">${filterLabels[item.type] || item.type}</span>
      </div>
      ${item.description ? `<div style="font-size:12px;color:#666;margin-bottom:6px;">${escapeHtml(item.description.substring(0, 100))}</div>` : ''}
      <div class="dev-progress"><div class="dev-progress-bar" style="width:${item.progress || 0}%"></div></div>
      <div style="font-size:11px;color:#555;margin-top:4px;">${item.progress || 0}%</div>`;
    grid.appendChild(card);
  }

  el.querySelectorAll('.dev-filters .pill').forEach(btn => {
    btn.addEventListener('click', () => { devFilter = btn.dataset.filter; loadDevelopment(); });
  });

  document.getElementById('dev-add-btn')?.addEventListener('click', () => showAddLearningModal());
}

function showAddLearningModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить</div>
    <div class="form-group"><label class="form-label">Тип</label>
      <select class="form-select" id="learn-type" style="width:100%;">
        <option value="course">Курс</option><option value="book">Книга</option>
        <option value="skill">Навык</option><option value="article">Статья</option>
      </select></div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="learn-title"></div>
    <div class="form-group"><label class="form-label">Описание</label><textarea class="form-textarea" id="learn-desc"></textarea></div>
    <div class="form-group"><label class="form-label">URL</label><input class="form-input" id="learn-url"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="learn-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('learn-save')?.addEventListener('click', async () => {
    const title = document.getElementById('learn-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_learning_item', {
        itemType: document.getElementById('learn-type')?.value || 'course',
        title,
        description: document.getElementById('learn-desc')?.value || '',
        url: document.getElementById('learn-url')?.value || '',
      });
      overlay.remove();
      loadDevelopment();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Hobbies ──
async function loadHobbies() {
  const el = document.getElementById('hobbies-content');
  if (!el) return;
  try {
    const hobbies = await invoke('get_hobbies').catch(() => []);
    renderHobbies(el, hobbies || []);
  } catch (e) {
    showStub('hobbies-content', '&#9670;', 'Хобби — скоро');
  }
}

function renderHobbies(el, hobbies) {
  el.innerHTML = `
    <div class="module-header"><h2>Хобби</h2><button class="btn-primary" id="hobby-add-btn">+ Добавить</button></div>
    <div class="hobby-grid" id="hobby-grid"></div>`;

  const grid = document.getElementById('hobby-grid');
  for (const h of hobbies) {
    const card = document.createElement('div');
    card.className = 'hobby-card';
    card.innerHTML = `
      <div class="hobby-card-icon">${h.icon || '&#9670;'}</div>
      <div class="hobby-card-name">${escapeHtml(h.name)}</div>
      <div class="hobby-card-hours">${h.total_hours || 0}</div>
      <div class="hobby-card-label">часов</div>`;
    card.addEventListener('click', () => showHobbyDetail(h));
    grid.appendChild(card);
  }

  document.getElementById('hobby-add-btn')?.addEventListener('click', () => {
    const name = prompt('Название хобби:');
    if (name) invoke('create_hobby', { name, category: 'general', icon: '&#9670;', color: '#818cf8' }).then(() => loadHobbies()).catch(e => alert(e));
  });
}

async function showHobbyDetail(hobby) {
  const entries = await invoke('get_hobby_entries', { hobbyId: hobby.id }).catch(() => []);
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">${escapeHtml(hobby.name)}</div>
    <div class="form-group">
      <label class="form-label">Длительность (мин)</label>
      <input class="form-input" id="hobby-duration" type="number" value="30">
    </div>
    <div class="form-group">
      <label class="form-label">Заметки</label>
      <input class="form-input" id="hobby-notes">
    </div>
    <button class="btn-primary" id="hobby-log-btn" style="width:100%;margin-bottom:16px;">Залогировать</button>
    <div class="module-card-title">История</div>
    ${(entries || []).map(e => `<div class="focus-log-item">
      <span class="focus-log-time">${e.date || ''}</span>
      <span class="focus-log-title">${e.notes || ''}</span>
      <span class="focus-log-duration">${e.duration_minutes || 0} мин</span>
    </div>`).join('')}
    <div class="modal-actions"><button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Закрыть</button></div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('hobby-log-btn')?.addEventListener('click', async () => {
    const dur = parseInt(document.getElementById('hobby-duration')?.value || '30');
    const notes = document.getElementById('hobby-notes')?.value || '';
    try {
      await invoke('log_hobby_entry', { hobbyId: hobby.id, durationMinutes: dur, notes });
      overlay.remove();
      loadHobbies();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Sports ──
async function loadSports() {
  const el = document.getElementById('sports-content');
  if (!el) return;
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
    renderSports(el, workouts || [], stats);
  } catch (e) {
    showStub('sports-content', '&#9829;', 'Спорт — скоро');
  }
}

function renderSports(el, workouts, stats) {
  const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };

  el.innerHTML = `
    <div class="module-header"><h2>Спорт</h2><button class="btn-primary" id="new-workout-btn">+ Тренировка</button></div>
    <div class="sports-stats">
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.count || 0}</div><div class="dashboard-stat-label">Тренировок</div></div>
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_minutes || 0}м</div><div class="dashboard-stat-label">Общее время</div></div>
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_calories || 0}</div><div class="dashboard-stat-label">Калории</div></div>
    </div>
    <div id="workouts-list"></div>`;

  const list = document.getElementById('workouts-list');
  for (const w of workouts) {
    const card = document.createElement('div');
    card.className = 'workout-card';
    card.innerHTML = `
      <div class="workout-card-header">
        <span class="workout-card-title">${escapeHtml(w.title || typeLabels[w.type] || w.type)}</span>
        <span class="workout-card-date">${w.date || ''}</span>
      </div>
      <div class="workout-card-meta">
        <span class="badge badge-purple">${typeLabels[w.type] || w.type}</span>
        <span>${w.duration_minutes || 0} мин</span>
        ${w.calories ? `<span>${w.calories} ккал</span>` : ''}
      </div>`;
    list.appendChild(card);
  }

  document.getElementById('new-workout-btn')?.addEventListener('click', () => showAddWorkoutModal());
}

function showAddWorkoutModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Новая тренировка</div>
    <div class="form-group"><label class="form-label">Тип</label>
      <select class="form-select" id="workout-type" style="width:100%;">
        <option value="gym">Зал</option><option value="cardio">Кардио</option>
        <option value="yoga">Йога</option><option value="swimming">Плавание</option>
        <option value="martial_arts">Единоборства</option><option value="other">Другое</option>
      </select></div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="workout-title"></div>
    <div class="form-group"><label class="form-label">Длительность (мин)</label><input class="form-input" id="workout-duration" type="number" value="60"></div>
    <div class="form-group"><label class="form-label">Калории</label><input class="form-input" id="workout-calories" type="number"></div>
    <div class="form-group"><label class="form-label">Заметки</label><textarea class="form-textarea" id="workout-notes"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="workout-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('workout-save')?.addEventListener('click', async () => {
    const title = document.getElementById('workout-title')?.value?.trim();
    try {
      await invoke('create_workout', {
        workoutType: document.getElementById('workout-type')?.value || 'other',
        title: title || '',
        durationMinutes: parseInt(document.getElementById('workout-duration')?.value || '60'),
        calories: parseInt(document.getElementById('workout-calories')?.value || '0') || null,
        notes: document.getElementById('workout-notes')?.value || '',
      });
      overlay.remove();
      loadSports();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Health ──
async function loadHealth() {
  const el = document.getElementById('health-content');
  if (!el) return;
  try {
    const today = await invoke('get_health_today').catch(() => ({}));
    const habits = await invoke('get_habits_today').catch(() => []);
    renderHealth(el, today, habits);
  } catch (e) {
    showStub('health-content', '&#10010;', 'Здоровье — скоро');
  }
}

function renderHealth(el, today, habits) {
  const sleep = today.sleep || null;
  const water = today.water || null;
  const mood = today.mood || null;
  const weight = today.weight || null;

  function metricClass(type, val) {
    if (val === null) return '';
    if (type === 'sleep') return val >= 7 ? 'good' : val >= 5 ? 'warning' : 'bad';
    if (type === 'water') return val >= 8 ? 'good' : val >= 4 ? 'warning' : 'bad';
    if (type === 'mood') return val >= 4 ? 'good' : val >= 3 ? 'warning' : 'bad';
    return '';
  }

  el.innerHTML = `
    <div class="module-header"><h2>Здоровье</h2><button class="btn-primary" id="health-log-btn">+ Записать</button></div>
    <div class="health-metrics">
      <div class="health-metric ${metricClass('sleep', sleep)}" data-type="sleep">
        <div class="health-metric-icon">&#x1F634;</div>
        <div class="health-metric-value">${sleep !== null ? sleep + 'ч' : '—'}</div>
        <div class="health-metric-label">Сон</div>
      </div>
      <div class="health-metric ${metricClass('water', water)}" data-type="water">
        <div class="health-metric-icon">&#x1F4A7;</div>
        <div class="health-metric-value">${water !== null ? water : '—'}</div>
        <div class="health-metric-label">Вода (стаканов)</div>
      </div>
      <div class="health-metric ${metricClass('mood', mood)}" data-type="mood">
        <div class="health-metric-icon">${mood >= 4 ? '&#x1F60A;' : mood >= 3 ? '&#x1F610;' : mood ? '&#x1F641;' : '&#x1F636;'}</div>
        <div class="health-metric-value">${mood !== null ? mood + '/5' : '—'}</div>
        <div class="health-metric-label">Настроение</div>
      </div>
      <div class="health-metric" data-type="weight">
        <div class="health-metric-icon">&#x2696;</div>
        <div class="health-metric-value">${weight !== null ? weight + 'кг' : '—'}</div>
        <div class="health-metric-label">Вес</div>
      </div>
    </div>
    <div class="habits-section">
      <div class="module-card-title" style="display:flex;justify-content:space-between;align-items:center;">
        Привычки
        <button class="btn-secondary" id="add-habit-btn" style="padding:4px 10px;font-size:11px;">+ Добавить</button>
      </div>
      <div id="habits-list"></div>
    </div>`;

  const habitsList = document.getElementById('habits-list');
  for (const h of habits) {
    const item = document.createElement('div');
    item.className = 'habit-item';
    item.innerHTML = `
      <div class="habit-check${h.completed ? ' checked' : ''}" data-id="${h.id}">${h.completed ? '&#10003;' : ''}</div>
      <span class="habit-name">${escapeHtml(h.name)}</span>
      ${h.streak > 0 ? `<span class="habit-streak">${h.streak} дн.</span>` : ''}`;
    item.querySelector('.habit-check').addEventListener('click', async () => {
      await invoke('check_habit', { habitId: h.id, date: null }).catch(() => {});
      loadHealth();
    });
    habitsList.appendChild(item);
  }

  // Click on metric to log
  el.querySelectorAll('.health-metric').forEach(m => {
    m.addEventListener('click', () => {
      const type = m.dataset.type;
      const labels = { sleep: 'Сон (часы)', water: 'Вода (стаканы)', mood: 'Настроение (1-5)', weight: 'Вес (кг)' };
      const val = prompt(labels[type] + ':');
      if (val) {
        invoke('log_health', { healthType: type, value: parseFloat(val), notes: null }).then(() => loadHealth()).catch(e => alert(e));
      }
    });
  });

  document.getElementById('health-log-btn')?.addEventListener('click', () => {
    const type = prompt('Тип (sleep/water/mood/weight):');
    if (!type) return;
    const val = prompt('Значение:');
    if (val) invoke('log_health', { healthType: type, value: parseFloat(val), notes: null }).then(() => loadHealth()).catch(e => alert(e));
  });

  document.getElementById('add-habit-btn')?.addEventListener('click', () => {
    const name = prompt('Название привычки:');
    if (name) invoke('create_habit', { name, icon: '', frequency: 'daily' }).then(() => loadHealth()).catch(e => alert(e));
  });
}

// ── Header version + update ──
const headerVersion = document.getElementById('header-version');
headerVersion.textContent = `v${APP_VERSION}`;
headerVersion.addEventListener('click', async () => {
  headerVersion.textContent = 'Проверяю...';
  try {
    const result = await invoke('check_update');
    headerVersion.textContent = result;
  } catch (err) {
    headerVersion.textContent = 'Ошибка';
  }
  setTimeout(() => { headerVersion.textContent = `v${APP_VERSION}`; }, 4000);
});

// ── Auto-restore last conversation on startup ──
(async () => {
  try {
    const convs = await invoke('get_conversations', { limit: 1 });
    if (convs.length > 0) {
      const latest = convs[0];
      const conv = await invoke('get_conversation', { id: latest.id });
      if (conv.messages && conv.messages.length > 0) {
        currentConversationId = latest.id;
        history = conv.messages;
        for (const [role, content] of history) {
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
  loadConversationsList();
})();

input.focus();
