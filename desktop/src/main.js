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

const APP_VERSION = '0.3.0';

let busy = false;
let history = [];
let attachedFile = null; // {name, content}
let currentTab = 'chat';
let integrationsTimer = null;

// ── Auto-update notification ──
listen('update-available', (event) => {
  const version = event.payload;
  const banner = document.createElement('div');
  banner.style.cssText = 'padding:8px 16px;background:#161616;color:#888;font-size:12px;text-align:center;border-bottom:1px solid #222;';
  banner.textContent = `Обновление до v${version}...`;
  document.querySelector('main').prepend(banner);
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

  // Clear integrations timer
  if (integrationsTimer) {
    clearInterval(integrationsTimer);
    integrationsTimer = null;
  }

  if (tab === 'integrations') {
    loadIntegrations();
    integrationsTimer = setInterval(loadIntegrations, 5000);
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

  // Create bot message with cursor
  const botDiv = document.createElement('div');
  botDiv.className = 'msg bot';
  const cursor = document.createElement('span');
  cursor.className = 'cursor';
  botDiv.appendChild(cursor);
  chat.appendChild(botDiv);
  scrollDown();

  const t0 = performance.now();
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
    const msgs = history.slice(-10);
    await invoke('chat', { messages: msgs });
  } catch (e) {
    if (!fullReply) {
      botDiv.textContent = 'MLX сервер недоступен';
    }
  }

  unlisten();
  unlistenDone();
  cursor.remove();

  if (fullReply) {
    history.push(['assistant', fullReply]);

    // Parse and execute any life-tracker actions
    const actions = parseAndExecuteActions(fullReply);
    for (const actionJson of actions) {
      const { success, result } = await executeAction(actionJson);
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = result;
      chat.appendChild(actionDiv);
      scrollDown();

      // Add result to history so model knows
      history.push(['user', `[Action result: ${result}]`]);
    }
  }

  // Show timing
  const total = ((performance.now() - t0) / 1000).toFixed(1);
  const ttft = firstToken ? (firstToken / 1000).toFixed(1) : '?';
  const timing = document.createElement('div');
  timing.className = 'timing';
  timing.textContent = `${ttft}s first token · ${total}s total · ${tokens} tokens`;
  chat.appendChild(timing);
  scrollDown();

  busy = false;
  sendBtn.disabled = false;
  input.focus();
}

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

async function loadIntegrations() {
  integrationsContent.innerHTML = '<div style="color:#555;font-size:13px;">Загрузка...</div>';
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
      </div>`;
  } catch (e) {
    integrationsContent.innerHTML = `<div style="color:#f87171;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

// ── Settings page ──

async function loadSettings() {
  settingsContent.innerHTML = '<div style="color:#555;font-size:13px;">Загрузка...</div>';
  try {
    const info = await invoke('get_model_info');
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
        <div class="settings-section-title">Приложение</div>
        <div class="settings-row">
          <span class="settings-label">Версия</span>
          <span class="settings-value">${APP_VERSION}</span>
        </div>
      </div>`;
  } catch (e) {
    settingsContent.innerHTML = `<div style="color:#f87171;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

// ── Activity update listener ──
listen('activity-update', () => {
  if (currentTab === 'integrations') {
    loadIntegrations();
  }
});

input.focus();
