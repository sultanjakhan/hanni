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

const APP_VERSION = '0.5.1';

let busy = false;
let history = [];
let attachedFile = null; // {name, content}
let currentTab = 'chat';
let isRecording = false;
let integrationsLoaded = false;

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
  if (focusTimer) {
    clearInterval(focusTimer);
    focusTimer = null;
  }
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
  document.getElementById(`view-${tab}`).classList.add('active');

  if (tab === 'integrations') {
    if (!integrationsLoaded) loadIntegrations();
  } else if (tab === 'settings') {
    loadSettings();
  } else if (tab === 'chat') {
    input.focus();
  }
}

document.querySelectorAll('.sidebar-btn').forEach(btn => {
  btn.addEventListener('click', () => switchTab(btn.dataset.tab));
});

// ── Chat helpers ──

function scrollDown() {
  chat.scrollTop = chat.scrollHeight;
}

function addMsg(role, text) {
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
    const msgs = history.slice(-16);
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

    // Create bot message div
    const botDiv = document.createElement('div');
    botDiv.className = 'msg bot';
    chat.appendChild(botDiv);
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

  // Show timing
  const total = ((performance.now() - t0) / 1000).toFixed(1);
  const ttft = firstToken ? (firstToken / 1000).toFixed(1) : '?';
  const timing = document.createElement('div');
  timing.className = 'timing';
  const stepInfo = iteration > 1 ? ` · ${iteration} steps` : '';
  timing.textContent = `${ttft}s first token · ${total}s total · ${totalTokens} tokens${stepInfo}`;
  chat.appendChild(timing);
  scrollDown();

  // Post-chat: save conversation and extract facts in background
  if (history.length >= 4) {
    (async () => {
      try {
        const convId = await invoke('save_conversation', { messages: history });
        await invoke('process_conversation_end', { messages: history, conversationId: convId });
      } catch (_) {}
    })();
  }

  busy = false;
  sendBtn.disabled = false;
  input.focus();
}

// ── New Chat ──

async function newChat() {
  if (busy) return;
  // Save current conversation before clearing
  if (history.length >= 4) {
    try {
      const convId = await invoke('save_conversation', { messages: history });
      await invoke('process_conversation_end', { messages: history, conversationId: convId });
    } catch (_) {}
  }
  history = [];
  chat.innerHTML = '';
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

    // Get focus status
    let focusStatus = { active: false, remaining_seconds: 0, blocked_apps: [], blocked_sites: [] };
    try { focusStatus = await invoke('get_focus_status'); } catch(_) {}

    const focusRemaining = focusStatus.active
      ? `${Math.floor(focusStatus.remaining_seconds / 60)}м ${focusStatus.remaining_seconds % 60}с`
      : '';

    const focusCard = `
      <div class="integration-card focus-card">
        <div class="integration-card-title">Фокус-режим</div>
        <span class="panel-status-badge ${focusStatus.active ? 'on' : 'off'}">${focusStatus.active ? 'Активен · ' + focusRemaining : 'Неактивен'}</span>
        <div class="focus-controls">
          <select id="focus-duration" class="settings-select" ${focusStatus.active ? 'disabled' : ''}>
            <option value="15">15 мин</option>
            <option value="30" selected>30 мин</option>
            <option value="60">60 мин</option>
            <option value="90">90 мин</option>
            <option value="120">120 мин</option>
          </select>
          <button class="settings-btn" id="focus-toggle-btn">${focusStatus.active ? 'Остановить' : 'Запустить'}</button>
        </div>
      </div>`;

    integrationsContent.innerHTML = `
      <div class="integrations-grid">
        ${focusCard}
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
      </div>`;

    // Focus toggle handler
    document.getElementById('focus-toggle-btn')?.addEventListener('click', async (e) => {
      const btn = e.target;
      btn.disabled = true;
      try {
        if (focusStatus.active) {
          await invoke('stop_focus');
          btn.textContent = 'Запустить';
        } else {
          const dur = parseInt(document.getElementById('focus-duration').value);
          await invoke('start_focus', { durationMinutes: dur, apps: null, sites: null });
          btn.textContent = 'Остановить';
        }
        loadIntegrations(true);
      } catch (err) {
        btn.textContent = 'Ошибка';
        btn.title = String(err);
      }
      btn.disabled = false;
    });

    integrationsLoaded = true;
  } catch (e) {
    integrationsContent.innerHTML = `<div style="color:#f87171;font-size:13px;">Ошибка: ${e}</div>`;
  }
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

input.focus();
