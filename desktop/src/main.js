const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const chat = document.getElementById('chat');
const input = document.getElementById('input');
const sendBtn = document.getElementById('send');
const attachBtn = document.getElementById('attach');
const fileInput = document.getElementById('file-input');
const attachPreview = document.getElementById('attach-preview');
const APP_VERSION = '0.8.9';

let busy = false;
let history = [];
let attachedFile = null;
let isRecording = false;
let currentConversationId = null;
let isSpeaking = false;
let convSearchTimeout = null;
let focusTimerInterval = null;
let currentNoteId = null;
let noteAutoSaveTimeout = null;
let calendarYear = new Date().getFullYear();
let calendarMonth = new Date().getMonth();
let selectedCalendarDate = null;
let calWeekOffset = 0;
let currentProjectId = null;
let devFilter = 'all';
let mediaStatusFilter = 'all';

// ── Tab Registry ──
const TAB_REGISTRY = {
  chat:        { label: 'Chat',        icon: '\u{1F4AC}', closable: false, subTabs: ['Чат', 'Настройки'], subIcons: { 'Чат': '\u{1F4AC}', 'Настройки': '\u{2699}' } },
  dashboard:   { label: 'Dashboard',   icon: '\u{1F3E0}', closable: true,  subTabs: ['Overview'] },
  calendar:    { label: 'Calendar',    icon: '\u{1F4C5}', closable: true,  subTabs: ['Месяц', 'Неделя', 'День', 'Список', 'Интеграции'] },
  focus:       { label: 'Focus',       icon: '\u{1F3AF}', closable: true,  subTabs: ['Current', 'History'] },
  notes:       { label: 'Notes',       icon: '\u{1F4DD}', closable: true,  subTabs: ['All', 'Pinned', 'Archived'] },
  work:        { label: 'Work',        icon: '\u{1F4BC}', closable: true,  subTabs: ['Projects'] },
  development: { label: 'Development', icon: '\u{1F4DA}', closable: true,  subTabs: ['Courses', 'Skills', 'Articles'] },
  home:        { label: 'Home',        icon: '\u{1F3E1}', closable: true,  subTabs: ['Supplies', 'Shopping List'] },
  hobbies:     { label: 'Hobbies',     icon: '\u{1F3AE}', closable: true,  subTabs: ['Overview','Music','Anime','Manga','Movies','Series','Cartoons','Games','Books','Podcasts'] },
  sports:      { label: 'Sports',      icon: '\u{1F3CB}', closable: true,  subTabs: ['Workouts', 'Martial Arts', 'Stats'] },
  health:      { label: 'Health',      icon: '\u{2764}',  closable: true,  subTabs: ['Today', 'Habits'] },
  mindset:     { label: 'Mindset',     icon: '\u{1F9E0}', closable: true,  subTabs: ['Journal', 'Mood', 'Principles'] },
  food:        { label: 'Food',        icon: '\u{1F355}', closable: true,  subTabs: ['Food Log', 'Recipes', 'Products'] },
  money:       { label: 'Money',       icon: '\u{1F4B0}', closable: true,  subTabs: ['Expenses', 'Income', 'Budget', 'Savings', 'Subscriptions', 'Debts'] },
  people:      { label: 'People',      icon: '\u{1F465}', closable: true,  subTabs: ['All', 'Blocked', 'Favorites'] },
  memory:      { label: 'Memory',      icon: '\u{1F4BE}', closable: true,  subTabs: ['All Facts', 'Search'] },
  settings:    { label: 'Settings',    icon: '\u{2699}',  closable: true,  subTabs: ['General', 'Blocklist', 'Integrations', 'About'] },
};

let openTabs = ['chat', 'dashboard'];
let activeTab = 'chat';
let activeSubTab = {};

// Init default sub-tabs
for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
  if (reg.subTabs?.length) activeSubTab[id] = reg.subTabs[0];
}

// Restore from localStorage
try {
  const saved = JSON.parse(localStorage.getItem('hanni_tabs'));
  if (saved) {
    openTabs = saved.open || ['chat', 'dashboard'];
    activeTab = saved.active || 'chat';
    if (saved.sub) Object.assign(activeSubTab, saved.sub);
  }
} catch (_) {}

function saveTabs() {
  localStorage.setItem('hanni_tabs', JSON.stringify({ open: openTabs, active: activeTab, sub: activeSubTab }));
}

// ── Auto-update notification ──
listen('update-available', (event) => {
  const version = event.payload;
  const banner = document.createElement('div');
  banner.style.cssText = 'padding:8px 16px;background:#111113;color:#a1a1a6;font-size:12px;text-align:center;border-bottom:1px solid #222224;';
  banner.textContent = `Обновление до v${version}...`;
  document.getElementById('content-area')?.prepend(banner);
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
        item.className = 'conv-item' + (c.id === currentConversationId ? ' active' : '');
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
          if (currentConversationId === id) {
            currentConversationId = null;
            history = [];
            chat.innerHTML = '';
          }
          loadConversationsList();
        });
        convList.appendChild(item);
      }
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

// New chat button
document.getElementById('new-chat-btn')?.addEventListener('click', async () => {
  await autoSaveConversation();
  currentConversationId = null;
  history = [];
  chat.innerHTML = '';
  input.focus();
  loadConversationsList();
});

// Conversation search
document.getElementById('conv-search')?.addEventListener('input', (e) => {
  clearTimeout(convSearchTimeout);
  convSearchTimeout = setTimeout(() => {
    loadConversationsList(e.target.value);
  }, 300);
});

// ── Tab navigation ──

function renderTabBar() {
  const tabList = document.getElementById('tab-list');
  tabList.innerHTML = '';
  for (const tabId of openTabs) {
    const reg = TAB_REGISTRY[tabId];
    if (!reg) continue;
    const item = document.createElement('div');
    item.className = 'tab-item' + (tabId === activeTab ? ' active' : '');
    item.innerHTML = `<span class="tab-item-icon">${reg.icon || ''}</span><span class="tab-item-label">${reg.label}</span>` +
      (reg.closable ? `<button class="tab-item-close" data-tab="${tabId}">&times;</button>` : '');
    item.addEventListener('click', (e) => {
      if (e.target.classList.contains('tab-item-close')) return;
      switchTab(tabId);
    });
    if (reg.closable) {
      item.querySelector('.tab-item-close').addEventListener('click', (e) => {
        e.stopPropagation();
        closeTab(tabId);
      });
    }
    tabList.appendChild(item);
  }
}

function renderSubSidebar() {
  const sidebar = document.getElementById('sub-sidebar');
  const items = document.getElementById('sub-sidebar-items');
  const reg = TAB_REGISTRY[activeTab];

  if (!reg || !reg.subTabs) {
    sidebar.classList.add('hidden');
    return;
  }

  sidebar.classList.remove('hidden');
  items.innerHTML = '';
  // Don't show sub-tabs when settings is the active tab (settings uses its own sub-tabs)
  const subTabs = (activeTab === 'settings') ? reg.subTabs : reg.subTabs;
  const currentSub = activeSubTab[activeTab] || subTabs[0];
  for (const sub of subTabs) {
    const item = document.createElement('div');
    item.className = 'sub-sidebar-item' + (sub === currentSub ? ' active' : '');
    const subIcon = reg.subIcons?.[sub];
    item.textContent = subIcon ? `${subIcon} ${sub}` : sub;
    item.addEventListener('click', () => {
      activeSubTab[activeTab] = sub;
      saveTabs();
      renderSubSidebar();
      loadSubTabContent(activeTab, sub);
    });
    items.appendChild(item);
  }
  // Settings shortcut at bottom
  const settingsBtn = document.getElementById('settings-shortcut');
  if (settingsBtn) {
    settingsBtn.className = 'sub-sidebar-item' + (activeTab === 'settings' ? ' active' : '');
    settingsBtn.onclick = () => switchTab('settings');
  }
  loadGoalsWidget();
}

async function loadGoalsWidget() {
  // Sub-sidebar goals (keep hidden — moved to content area)
  const section = document.getElementById('sub-sidebar-goals');
  if (section) section.classList.add('hidden');

  // Inject goals into content area
  if (activeTab === 'chat' || activeTab === 'settings') return;
  const contentEl = document.getElementById(`${activeTab}-content`);
  if (!contentEl) return;

  let existing = contentEl.querySelector('.goals-inline');
  if (existing) existing.remove();

  try {
    const goals = await invoke('get_goals', { tabName: activeTab });
    const wrapper = document.createElement('div');
    wrapper.className = 'goals-inline';
    wrapper.innerHTML = `
      <div class="goals-inline-header">
        <span class="goals-inline-title">Goals</span>
        <button class="btn-small" id="add-goal-btn">+ Goal</button>
      </div>
      ${goals.length > 0 ? goals.map(g => {
        const pct = g.target_value > 0 ? Math.min(100, Math.round(g.current_value / g.target_value * 100)) : 0;
        return `<div class="goal-inline-item">
          <div class="goal-inline-info"><span>${escapeHtml(g.title)}</span><span class="goal-inline-pct">${pct}%</span></div>
          <div class="goal-progress"><div class="goal-progress-bar" style="width:${pct}%"></div></div>
        </div>`;
      }).join('') : '<div style="color:#3f3f44;font-size:12px;">No goals yet</div>'}`;
    contentEl.insertBefore(wrapper, contentEl.firstChild);
    wrapper.querySelector('#add-goal-btn')?.addEventListener('click', () => showAddGoalModal());
  } catch (_) {}
}

function showAddGoalModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">New Goal</div>
    <div class="form-row"><input class="form-input" id="goal-title" placeholder="Goal title"></div>
    <div class="form-row">
      <input class="form-input" id="goal-target" type="number" placeholder="Target" style="max-width:100px;">
      <input class="form-input" id="goal-unit" placeholder="Unit (e.g. km, books)" style="max-width:120px;">
      <input class="form-input" id="goal-deadline" type="date" style="max-width:150px;">
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="goal-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('goal-save')?.addEventListener('click', async () => {
    const title = document.getElementById('goal-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_goal', {
        tabName: activeTab,
        title,
        targetValue: parseInt(document.getElementById('goal-target')?.value || '1') || 1,
        currentValue: 0,
        unit: document.getElementById('goal-unit')?.value || '',
        deadline: document.getElementById('goal-deadline')?.value || null,
      });
      overlay.remove();
      loadGoalsWidget();
    } catch (err) { alert('Error: ' + err); }
  });
}

function openTab(tabId) {
  if (!TAB_REGISTRY[tabId]) return;
  if (!openTabs.includes(tabId)) openTabs.push(tabId);
  switchTab(tabId);
}

function closeTab(tabId) {
  if (!TAB_REGISTRY[tabId]?.closable) return;
  const idx = openTabs.indexOf(tabId);
  if (idx === -1) return;
  openTabs.splice(idx, 1);
  if (activeTab === tabId) activeTab = openTabs[Math.min(idx, openTabs.length - 1)] || 'chat';
  saveTabs();
  renderTabBar();
  activateView();
}

function switchTab(tabId) {
  if (!TAB_REGISTRY[tabId]) return;
  if (!openTabs.includes(tabId)) openTabs.push(tabId);
  activeTab = tabId;
  saveTabs();
  renderTabBar();
  activateView();
}

function activateView() {
  document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
  const view = document.getElementById(`view-${activeTab}`);
  if (view) view.classList.add('active');
  renderSubSidebar();
  const reg = TAB_REGISTRY[activeTab];
  const sub = reg?.subTabs ? (activeSubTab[activeTab] || reg.subTabs[0]) : null;
  loadSubTabContent(activeTab, sub);
}

function loadSubTabContent(tabId, subTab) {
  switch (tabId) {
    case 'chat':
      if (subTab === 'Настройки') { showChatSettingsMode(); loadChatSettings(); }
      else { hideChatSettingsMode(); loadConversationsList(); input.focus(); }
      break;
    case 'dashboard': loadDashboard(); break;
    case 'calendar': loadCalendar(subTab); break;
    case 'focus': loadFocus(); break;
    case 'notes': loadNotes(subTab); break;
    case 'work': loadWork(); break;
    case 'development': loadDevelopment(); break;
    case 'home': loadHome(subTab); break;
    case 'hobbies': loadHobbies(subTab); break;
    case 'sports': loadSports(subTab); break;
    case 'health': loadHealth(); break;
    case 'mindset': loadMindset(subTab); break;
    case 'food': loadFood(subTab); break;
    case 'money': loadMoney(subTab); break;
    case 'people': loadPeople(subTab); break;
    case 'memory': loadMemoryTab(subTab); break;
    case 'settings': loadSettings(subTab); break;
  }
}

// Tab add dropdown
document.getElementById('tab-add')?.addEventListener('click', (e) => {
  e.stopPropagation();
  const dropdown = document.getElementById('tab-dropdown');
  const list = document.getElementById('tab-dropdown-list');
  list.innerHTML = '';
  for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
    if (openTabs.includes(id)) continue;
    const item = document.createElement('div');
    item.className = 'tab-dropdown-item';
    item.textContent = `${reg.icon || ''} ${reg.label}`;
    item.addEventListener('click', () => { dropdown.classList.add('hidden'); openTab(id); });
    list.appendChild(item);
  }
  dropdown.classList.toggle('hidden');
});

document.addEventListener('click', () => {
  document.getElementById('tab-dropdown')?.classList.add('hidden');
});

// Settings button in tab bar (always visible)
document.getElementById('tab-settings-btn')?.addEventListener('click', () => openTab('settings'));

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
  if (!e.metaKey && !e.ctrlKey) return;
  if (e.key === 'w') { e.preventDefault(); if (TAB_REGISTRY[activeTab]?.closable) closeTab(activeTab); return; }
  if (e.key === 't') { e.preventDefault(); document.getElementById('tab-add')?.click(); return; }
  const num = parseInt(e.key);
  if (num >= 1 && num <= 9 && num <= openTabs.length) { e.preventDefault(); switchTab(openTabs[num - 1]); }
});

// ── Chat settings mode ──

function showChatSettingsMode() {
  const view = document.getElementById('view-chat');
  view.classList.add('chat-settings-mode');
}

function hideChatSettingsMode() {
  const view = document.getElementById('view-chat');
  view.classList.remove('chat-settings-mode');
}

async function loadChatSettings() {
  const el = document.getElementById('chat-settings-content');
  if (!el) return;
  el.innerHTML = '<div style="color:#3f3f44;font-size:13px;">Загрузка...</div>';
  try {
    const [proactive, ttsVoices, ttsServerUrl] = await Promise.all([
      invoke('get_proactive_settings'),
      invoke('get_tts_voices').catch(() => []),
      invoke('get_app_setting', { key: 'tts_server_url' }).catch(() => null),
    ]);
    const voicesByLang = {};
    for (const v of ttsVoices) {
      const lang = v.lang || 'other';
      if (!voicesByLang[lang]) voicesByLang[lang] = [];
      voicesByLang[lang].push(v);
    }
    const langOrder = ['ru-RU', 'kk-KZ', 'en-US'];
    const sortedLangs = [...langOrder.filter(l => voicesByLang[l]), ...Object.keys(voicesByLang).filter(l => !langOrder.includes(l)).sort()];

    el.innerHTML = `
      <div class="settings-section">
        <div class="settings-section-title">Автономный режим</div>
        <div class="settings-row">
          <span class="settings-label">Включён</span>
          <label class="toggle">
            <input type="checkbox" id="chat-proactive-enabled" ${proactive.enabled ? 'checked' : ''}>
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="settings-row">
          <span class="settings-label">Интервал</span>
          <div class="pill-group" id="chat-proactive-interval">
            ${[5, 10, 15, 30].map(v =>
              `<button class="pill${proactive.interval_minutes === v ? ' active' : ''}" data-value="${v}">${v}м</button>`
            ).join('')}
          </div>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Голос</div>
        <div class="settings-row">
          <span class="settings-label">Включён</span>
          <label class="toggle">
            <input type="checkbox" id="chat-voice-enabled" ${proactive.voice_enabled ? 'checked' : ''}>
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="settings-row">
          <span class="settings-label">Голос TTS</span>
          <select class="form-select" id="chat-voice-name" style="width:260px">
            ${sortedLangs.map(lang => `
              <optgroup label="${lang}">
                ${(voicesByLang[lang] || []).map(v =>
                  `<option value="${v.name}" ${proactive.voice_name === v.name ? 'selected' : ''}>${v.name} (${v.gender})</option>`
                ).join('')}
              </optgroup>
            `).join('')}
          </select>
        </div>
        <div class="settings-row">
          <span class="settings-label"></span>
          <button class="settings-btn" id="chat-test-voice">Прослушать</button>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">TTS Сервер (PC)</div>
        <div class="settings-row">
          <span class="settings-label">URL сервера</span>
          <input class="form-input" id="chat-tts-server-url" placeholder="http://192.168.x.x:8236" value="${ttsServerUrl || ''}" style="width:220px">
        </div>
        <div class="settings-row">
          <span class="settings-label">Статус</span>
          <span class="settings-value" id="chat-tts-server-status">—</span>
        </div>
        <div class="settings-row">
          <span class="settings-label"></span>
          <button class="settings-btn" id="chat-tts-server-save">Сохранить</button>
        </div>
      </div>`;

    // Save handlers
    const getChatProactiveValues = () => ({
      enabled: document.getElementById('chat-proactive-enabled').checked,
      voice_enabled: document.getElementById('chat-voice-enabled').checked,
      voice_name: document.getElementById('chat-voice-name')?.value || 'ru-RU-SvetlanaNeural',
      interval_minutes: parseInt(document.querySelector('#chat-proactive-interval .pill.active')?.dataset.value || '10'),
      quiet_hours_start: proactive.quiet_hours_start || 23,
      quiet_hours_end: proactive.quiet_hours_end || 8,
    });
    const saveChatSettings = () => invoke('set_proactive_settings', { settings: getChatProactiveValues() }).catch(() => {});

    document.getElementById('chat-proactive-enabled')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-voice-enabled')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-voice-name')?.addEventListener('change', saveChatSettings);

    document.querySelectorAll('#chat-proactive-interval .pill').forEach(pill => {
      pill.addEventListener('click', () => {
        document.querySelectorAll('#chat-proactive-interval .pill').forEach(p => p.classList.remove('active'));
        pill.classList.add('active');
        saveChatSettings();
      });
    });

    document.getElementById('chat-test-voice')?.addEventListener('click', async () => {
      const voice = document.getElementById('chat-voice-name')?.value || 'ru-RU-SvetlanaNeural';
      const btn = document.getElementById('chat-test-voice');
      btn.textContent = 'Говорю...';
      btn.disabled = true;
      try {
        await invoke('speak_text', { text: 'Привет! Я Ханни, твой персональный ассистент.', voice });
      } catch (e) { console.error(e); }
      setTimeout(() => { btn.textContent = 'Прослушать'; btn.disabled = false; }, 3000);
    });

    // TTS server
    document.getElementById('chat-tts-server-save')?.addEventListener('click', async () => {
      const url = document.getElementById('chat-tts-server-url')?.value.trim() || '';
      await invoke('set_app_setting', { key: 'tts_server_url', value: url });
      const statusEl = document.getElementById('chat-tts-server-status');
      if (!url) { if (statusEl) statusEl.textContent = 'Отключён (edge-tts)'; return; }
      try {
        const resp = await fetch(url.replace(/\/$/, '') + '/health');
        const data = await resp.json();
        if (statusEl) statusEl.textContent = `${data.model} | ${data.gpu || 'CPU'}`;
      } catch (e) {
        if (statusEl) statusEl.textContent = 'Недоступен';
      }
    });

    // Auto-check TTS server
    if (ttsServerUrl) {
      try {
        const resp = await fetch(ttsServerUrl.replace(/\/$/, '') + '/health');
        const data = await resp.json();
        const s = document.getElementById('chat-tts-server-status');
        if (s) s.textContent = `${data.model} | ${data.gpu || 'CPU'}`;
      } catch { const s = document.getElementById('chat-tts-server-status'); if (s) s.textContent = 'Недоступен'; }
    }
  } catch (e) {
    el.innerHTML = `<div style="color:#ef4444;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

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
    const ttsBtn = document.createElement('button');
    ttsBtn.className = 'tts-btn';
    ttsBtn.innerHTML = '&#9654;';
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
    // Normalize common model variations
    if (action.meal_type) {
      const mealMap = {'завтрак':'breakfast','обед':'lunch','ужин':'dinner','перекус':'snack',
        'breakfast':'breakfast','lunch':'lunch','dinner':'dinner','snack':'snack'};
      action.meal_type = mealMap[action.meal_type.toLowerCase()] || action.meal_type.toLowerCase();
    }
    let actionType = action.action || action.type;
    let result;
    // If log_health has only mood, redirect to log_mood
    if (actionType === 'log_health' && action.mood && !action.sleep && !action.water && !action.steps && !action.weight) {
      actionType = 'log_mood';
    }

    switch (actionType) {
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
      // Media
      case 'add_media_item':
      case 'add_media':
        result = await invoke('add_media_item', {
          mediaType: action.media_type || 'movie', title: action.title || '',
          originalTitle: action.original_title || null, year: action.year || null,
          description: action.description || null, coverUrl: action.cover_url || null,
          status: action.status || 'planned', rating: action.rating || null,
          progress: action.progress || null, totalEpisodes: action.total_episodes || null,
          notes: action.notes || null,
        });
        break;
      // Food
      case 'log_food':
        result = await invoke('log_food', {
          date: action.date || null, mealType: action.meal_type || 'snack',
          name: action.name || '', calories: action.calories || null,
          protein: action.protein || null, carbs: action.carbs || null,
          fat: action.fat || null, notes: action.notes || null,
        });
        break;
      case 'add_product':
        result = await invoke('add_product', {
          name: action.name || '', category: action.category || null,
          quantity: action.quantity || null, unit: action.unit || null,
          expiryDate: action.expiry_date || null, location: action.location || 'fridge',
          barcode: action.barcode || null, notes: action.notes || null,
        });
        break;
      // Money
      case 'add_transaction':
        result = await invoke('add_transaction', {
          date: action.date || null, txType: action.transaction_type || action.tx_type || 'expense',
          amount: action.amount || 0, currency: action.currency || 'KZT',
          category: action.category || 'other', description: action.description || '',
          recurring: action.recurring || false, recurringPeriod: action.recurring_period || null,
        });
        break;
      // Mindset
      case 'log_mood':
        result = await invoke('log_mood', {
          mood: action.mood || 3, note: action.note || null,
          trigger: action.trigger || null,
        });
        break;
      case 'save_journal':
        result = await invoke('save_journal_entry', {
          mood: action.mood || 3, energy: action.energy || 3, stress: action.stress || 3,
          gratitude: action.gratitude || null, reflection: action.reflection || null,
          wins: action.wins || null, struggles: action.struggles || null,
        });
        break;
      // Calendar events
      case 'create_event':
        result = await invoke('create_event', {
          title: action.title || '', description: action.description || '',
          date: action.date || new Date().toISOString().slice(0, 10),
          time: action.time || '', durationMinutes: action.duration || action.duration_minutes || 60,
          category: action.category || 'general', color: action.color || '#a1a1a6',
        });
        break;
      case 'delete_event':
        result = await invoke('delete_event', { id: action.id });
        break;
      case 'sync_calendar':
        try {
          const m = action.month || (new Date().getMonth() + 1);
          const y = action.year || new Date().getFullYear();
          result = await invoke('sync_apple_calendar', { month: m, year: y });
        } catch (e) { result = 'Sync error: ' + e; }
        break;
      // Activities (time tracking)
      case 'start_activity':
        result = await invoke('start_activity', {
          name: action.name || action.activity || '',
          category: action.category || 'other',
        });
        break;
      case 'stop_activity':
        result = await invoke('stop_activity');
        break;
      case 'get_current_activity':
        result = await invoke('get_current_activity');
        break;
      // Projects & Tasks
      case 'create_task':
        result = await invoke('create_task', {
          projectId: action.project_id || 1, title: action.title || '',
          description: action.description || '', priority: action.priority || 'medium',
          dueDate: action.due_date || null,
        });
        break;
      // Home items
      case 'add_home_item':
        result = await invoke('add_home_item', {
          name: action.name || '', category: action.category || 'other',
          quantity: action.quantity || null, unit: action.unit || null,
          location: action.location || 'other', notes: action.notes || null,
        });
        break;
      // Health
      case 'log_health':
        result = await invoke('log_health', {
          sleep: action.sleep || null, water: action.water || null,
          steps: action.steps || null, weight: action.weight || null,
          notes: action.notes || null,
        });
        break;
      // Goals
      case 'create_goal':
        result = await invoke('create_goal', {
          tabName: action.tab || 'general', title: action.title || '',
          targetValue: action.target || 1, currentValue: action.current || 0,
          unit: action.unit || '', deadline: action.deadline || null,
        });
        break;
      case 'update_goal':
        result = await invoke('update_goal', {
          id: action.id, currentValue: action.current || null,
          status: action.status || null,
        });
        break;
      default:
        console.warn('Unknown action:', actionType, action);
        result = 'Unknown action: ' + actionType;
    }

    return { success: true, result };
  } catch (e) {
    return { success: false, result: String(e) };
  }
}

function parseAndExecuteActions(text) {
  // Lenient regex: optional newlines, handle ```action\n...\n``` and ```action{...}```
  const actionRegex = /```action\s*\n?([\s\S]*?)```/g;
  let match;
  const actions = [];

  while ((match = actionRegex.exec(text)) !== null) {
    let json = match[1].trim();
    if (json) actions.push(repairJson(json));
  }

  // Fallback: if no ```action found, try to find bare JSON with "action" key
  if (actions.length === 0) {
    const bareJson = text.match(/\{[^{}]*"action"\s*:\s*"[^"]+?"[^{}]*\}/g);
    if (bareJson) {
      for (const j of bareJson) actions.push(repairJson(j));
    }
  }

  return actions;
}

function repairJson(str) {
  // Fix common model JSON mistakes
  let s = str.trim();
  // Remove trailing commas before } or ]
  s = s.replace(/,\s*([}\]])/g, '$1');
  // Replace single quotes with double quotes (but not inside strings)
  if (!s.includes('"') && s.includes("'")) {
    s = s.replace(/'/g, '"');
  }
  // Fix unquoted keys: {action: "foo"} → {"action": "foo"}
  s = s.replace(/([{,])\s*([a-zA-Z_]\w*)\s*:/g, '$1"$2":');
  return s;
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
    document.querySelectorAll('.tts-btn.speaking').forEach(b => {
      b.classList.remove('speaking');
      b.innerHTML = '&#9654;';
    });
    isSpeaking = false;
    return;
  }
  isSpeaking = true;
  btn.classList.add('speaking');
  btn.innerHTML = '&#9632;';
  try {
    let voice = 'ru-RU-SvetlanaNeural';
    try {
      const ps = await invoke('get_proactive_settings');
      voice = ps.voice_name || 'ru-RU-SvetlanaNeural';
    } catch (_) {}
    await invoke('speak_text', { text, voice });
    const wordCount = text.split(/\s+/).length;
    const durationMs = Math.max(2000, wordCount * 300);
    setTimeout(() => {
      btn.classList.remove('speaking');
      btn.innerHTML = '&#9654;';
      isSpeaking = false;
    }, durationMs);
  } catch (_) {
    btn.classList.remove('speaking');
    btn.innerHTML = '&#9654;';
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

// ── Home ──
async function loadHome(subTab) {
  const el = document.getElementById('home-content');
  if (!el) return;
  if (subTab === 'Shopping List') loadShoppingList(el);
  else loadSupplies(el);
}

async function loadSupplies(el) {
  try {
    const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
    const categories = { cleaning: 'Cleaning', hygiene: 'Hygiene', household: 'Household', electronics: 'Electronics', tools: 'Tools', other: 'Other' };
    el.innerHTML = `
      <div class="module-header"><h2>Supplies</h2><button class="btn-primary" id="home-add-btn">+ Add Item</button></div>
      <div id="home-items-list">
        ${items.map(i => `<div class="focus-log-item" style="${i.needed ? 'border-left:2px solid #a1a1a6;' : ''}">
          <span class="focus-log-title">${escapeHtml(i.name)}</span>
          <span class="badge badge-gray">${categories[i.category] || i.category}</span>
          ${i.quantity != null ? `<span style="color:#a1a1a6;font-size:12px;">${i.quantity} ${i.unit||''}</span>` : ''}
          <span style="color:#3f3f44;font-size:11px;">${i.location||''}</span>
          <button class="btn-secondary" style="padding:2px 8px;font-size:10px;margin-left:4px;" data-need="${i.id}">${i.needed ? 'In stock' : 'Need'}</button>
          <button class="memory-item-btn" data-hdel="${i.id}">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-need]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('toggle_home_item_needed', { id: parseInt(btn.dataset.need) }).catch(()=>{});
        loadSupplies(el);
      });
    });
    el.querySelectorAll('[data-hdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_home_item', { id: parseInt(btn.dataset.hdel) }).catch(()=>{});
        loadSupplies(el);
      });
    });
    document.getElementById('home-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add Supply</div>
        <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="hi-name"></div>
        <div class="form-group"><label class="form-label">Category</label>
          <select class="form-select" id="hi-cat" style="width:100%;">
            ${Object.entries(categories).map(([k,v]) => `<option value="${k}">${v}</option>`).join('')}
          </select></div>
        <div class="form-group"><label class="form-label">Quantity</label><input class="form-input" id="hi-qty" type="number"></div>
        <div class="form-group"><label class="form-label">Unit</label><input class="form-input" id="hi-unit" placeholder="pcs, kg, L..."></div>
        <div class="form-group"><label class="form-label">Location</label>
          <select class="form-select" id="hi-loc" style="width:100%;">
            <option value="kitchen">Kitchen</option><option value="bathroom">Bathroom</option>
            <option value="bedroom">Bedroom</option><option value="living_room">Living Room</option>
            <option value="storage">Storage</option><option value="other">Other</option>
          </select></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="hi-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('hi-save')?.addEventListener('click', async () => {
        const name = document.getElementById('hi-name')?.value?.trim();
        if (!name) return;
        try {
          await invoke('add_home_item', {
            name, category: document.getElementById('hi-cat')?.value || 'other',
            quantity: parseFloat(document.getElementById('hi-qty')?.value) || null,
            unit: document.getElementById('hi-unit')?.value || null,
            location: document.getElementById('hi-loc')?.value || 'other',
            notes: null,
          });
          overlay.remove();
          loadSupplies(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadShoppingList(el) {
  try {
    const items = await invoke('get_home_items', { category: null, neededOnly: true }).catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Shopping List</h2></div>
      ${items.length > 0 ? `<div id="shopping-list">
        ${items.map(i => `<div class="habit-item">
          <div class="habit-check" data-bought="${i.id}"></div>
          <span class="habit-name">${escapeHtml(i.name)}</span>
          ${i.quantity != null ? `<span style="color:#a1a1a6;font-size:12px;margin-left:auto;">${i.quantity} ${i.unit||''}</span>` : ''}
        </div>`).join('')}
      </div>` : '<div style="color:#3f3f44;font-size:13px;padding:20px;text-align:center;">All stocked up!</div>'}`;
    el.querySelectorAll('[data-bought]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('toggle_home_item_needed', { id: parseInt(btn.dataset.bought) }).catch(()=>{});
        loadShoppingList(el);
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

// ── Mindset ──
async function loadMindset(subTab) {
  const el = document.getElementById('mindset-content');
  if (!el) return;
  if (subTab === 'Journal') loadJournal(el);
  else if (subTab === 'Mood') loadMoodLog(el);
  else if (subTab === 'Principles') loadPrinciples(el);
  else loadJournal(el);
}

async function loadJournal(el) {
  try {
    const today = await invoke('get_journal_entry', { date: null }).catch(() => null);
    const entries = await invoke('get_journal_entries', { days: 7 }).catch(() => []);
    const mood = today?.mood || 3, energy = today?.energy || 3, stress = today?.stress || 3;
    el.innerHTML = `
      <div class="module-header"><h2>Journal</h2></div>
      <div class="settings-section">
        <div class="settings-section-title">Today</div>
        <div class="settings-row"><span class="settings-label">Mood (1-5)</span><input class="form-input" id="j-mood" type="number" min="1" max="5" value="${mood}" style="width:60px;"></div>
        <div class="settings-row"><span class="settings-label">Energy (1-5)</span><input class="form-input" id="j-energy" type="number" min="1" max="5" value="${energy}" style="width:60px;"></div>
        <div class="settings-row"><span class="settings-label">Stress (1-5)</span><input class="form-input" id="j-stress" type="number" min="1" max="5" value="${stress}" style="width:60px;"></div>
        <div class="form-group"><label class="form-label">Gratitude</label><textarea class="form-textarea" id="j-gratitude" rows="2">${escapeHtml(today?.gratitude||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Wins</label><textarea class="form-textarea" id="j-wins" rows="2">${escapeHtml(today?.wins||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Struggles</label><textarea class="form-textarea" id="j-struggles" rows="2">${escapeHtml(today?.struggles||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Reflection</label><textarea class="form-textarea" id="j-reflection" rows="3">${escapeHtml(today?.reflection||'')}</textarea></div>
        <button class="btn-primary" id="j-save" style="margin-top:8px;">Save</button>
      </div>
      ${entries.length > 0 ? `<div class="module-card-title" style="margin-top:16px;">Recent Entries</div>
        ${entries.map(e => `<div class="focus-log-item">
          <span class="focus-log-time">${e.date}</span>
          <span class="focus-log-title">Mood:${e.mood} Energy:${e.energy} Stress:${e.stress}</span>
        </div>`).join('')}` : ''}`;
    document.getElementById('j-save')?.addEventListener('click', async () => {
      try {
        await invoke('save_journal_entry', {
          mood: parseInt(document.getElementById('j-mood')?.value)||3,
          energy: parseInt(document.getElementById('j-energy')?.value)||3,
          stress: parseInt(document.getElementById('j-stress')?.value)||3,
          gratitude: document.getElementById('j-gratitude')?.value||null,
          reflection: document.getElementById('j-reflection')?.value||null,
          wins: document.getElementById('j-wins')?.value||null,
          struggles: document.getElementById('j-struggles')?.value||null,
        });
        loadJournal(el);
      } catch (err) { alert('Error: ' + err); }
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadMoodLog(el) {
  try {
    const history = await invoke('get_mood_history', { days: 14 }).catch(() => []);
    const moods = ['😤','😕','😐','🙂','😊'];
    el.innerHTML = `
      <div class="module-header"><h2>Mood Log</h2></div>
      <div style="display:flex;gap:12px;justify-content:center;margin:20px 0;">
        ${moods.map((m,i) => `<button class="mood-btn" data-mood="${i+1}" style="font-size:32px;background:none;border:none;cursor:pointer;opacity:0.5;transition:opacity 0.1s;" title="Mood ${i+1}">${m}</button>`).join('')}
      </div>
      <input class="form-input" id="mood-note" placeholder="Note (optional)..." style="max-width:400px;margin:0 auto 16px;display:block;">
      <div class="module-card-title">Recent</div>
      <div id="mood-history">
        ${history.map(m => `<div class="focus-log-item">
          <span class="focus-log-time">${m.date} ${m.time||''}</span>
          <span style="font-size:18px;">${moods[(m.mood||3)-1]}</span>
          <span class="focus-log-title">${escapeHtml(m.note||'')}</span>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('.mood-btn').forEach(btn => {
      btn.addEventListener('mouseenter', () => btn.style.opacity = '1');
      btn.addEventListener('mouseleave', () => btn.style.opacity = '0.5');
      btn.addEventListener('click', async () => {
        try {
          await invoke('log_mood', { mood: parseInt(btn.dataset.mood), note: document.getElementById('mood-note')?.value||null, trigger: null });
          loadMoodLog(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadPrinciples(el) {
  try {
    const principles = await invoke('get_principles').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Principles</h2><button class="btn-primary" id="add-principle-btn">+ Add</button></div>
      <div id="principles-list">
        ${principles.map(p => `<div class="habit-item">
          <div class="habit-check${p.active ? ' checked' : ''}" data-id="${p.id}">${p.active ? '&#10003;' : ''}</div>
          <span class="habit-name">${escapeHtml(p.title)}</span>
          <span style="color:#3f3f44;font-size:11px;">${p.category||''}</span>
          <button class="memory-item-btn" data-del="${p.id}" style="margin-left:auto;">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-del]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (confirm('Delete?')) { await invoke('delete_principle', { id: parseInt(btn.dataset.del) }).catch(()=>{}); loadPrinciples(el); }
      });
    });
    document.getElementById('add-principle-btn')?.addEventListener('click', () => {
      const title = prompt('Principle:');
      if (title) invoke('create_principle', { title, description: '', category: 'discipline' }).then(() => loadPrinciples(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

// ── Food ──
async function loadFood(subTab) {
  const el = document.getElementById('food-content');
  if (!el) return;
  if (subTab === 'Food Log') loadFoodLog(el);
  else if (subTab === 'Recipes') loadRecipes(el);
  else if (subTab === 'Products') loadProducts(el);
  else loadFoodLog(el);
}

async function loadFoodLog(el) {
  try {
    const today = new Date().toISOString().split('T')[0];
    const log = await invoke('get_food_log', { date: today }).catch(() => []);
    const stats = await invoke('get_food_stats', { days: 1 }).catch(() => ({}));
    const mealLabels = { breakfast:'Breakfast', lunch:'Lunch', dinner:'Dinner', snack:'Snack' };
    el.innerHTML = `
      <div class="module-header"><h2>Food Log</h2><button class="btn-primary" id="food-add-btn">+ Log Food</button></div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_calories||0}</div><div class="dashboard-stat-label">Calories</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_protein||0}g</div><div class="dashboard-stat-label">Protein</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_carbs||0}g</div><div class="dashboard-stat-label">Carbs</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_fat||0}g</div><div class="dashboard-stat-label">Fat</div></div>
      </div>
      <div id="food-log-list">
        ${['breakfast','lunch','dinner','snack'].map(meal => {
          const items = log.filter(l => l.meal_type === meal);
          return items.length > 0 ? `<div class="module-card-title">${mealLabels[meal]}</div>
            ${items.map(i => `<div class="focus-log-item">
              <span class="focus-log-title">${escapeHtml(i.name)}</span>
              <span class="focus-log-duration">${i.calories||0} kcal</span>
              <button class="memory-item-btn" data-fdel="${i.id}">&times;</button>
            </div>`).join('')}` : '';
        }).join('')}
      </div>`;
    el.querySelectorAll('[data-fdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_food_entry', { id: parseInt(btn.dataset.fdel) }).catch(()=>{});
        loadFoodLog(el);
      });
    });
    document.getElementById('food-add-btn')?.addEventListener('click', () => showAddFoodModal(el));
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

function showAddFoodModal(el) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Log Food</div>
    <div class="form-group"><label class="form-label">Meal</label>
      <select class="form-select" id="food-meal" style="width:100%;">
        <option value="breakfast">Breakfast</option><option value="lunch">Lunch</option>
        <option value="dinner">Dinner</option><option value="snack">Snack</option>
      </select></div>
    <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="food-name"></div>
    <div class="form-group"><label class="form-label">Calories</label><input class="form-input" id="food-cal" type="number"></div>
    <div class="form-group"><label class="form-label">Protein (g)</label><input class="form-input" id="food-protein" type="number"></div>
    <div class="form-group"><label class="form-label">Carbs (g)</label><input class="form-input" id="food-carbs" type="number"></div>
    <div class="form-group"><label class="form-label">Fat (g)</label><input class="form-input" id="food-fat" type="number"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="food-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('food-save')?.addEventListener('click', async () => {
    const name = document.getElementById('food-name')?.value?.trim();
    if (!name) return;
    try {
      await invoke('log_food', {
        date: null, mealType: document.getElementById('food-meal')?.value || 'snack', name,
        calories: parseInt(document.getElementById('food-cal')?.value)||null,
        protein: parseFloat(document.getElementById('food-protein')?.value)||null,
        carbs: parseFloat(document.getElementById('food-carbs')?.value)||null,
        fat: parseFloat(document.getElementById('food-fat')?.value)||null,
        notes: null,
      });
      overlay.remove();
      loadFoodLog(el);
    } catch (err) { alert('Error: ' + err); }
  });
}

async function loadRecipes(el) {
  try {
    const recipes = await invoke('get_recipes', { search: null, tags: null }).catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Recipes</h2><button class="btn-primary" id="recipe-add-btn">+ Add Recipe</button></div>
      <div class="dev-grid" id="recipes-grid">
        ${recipes.map(r => `<div class="dev-card">
          <div class="dev-card-title">${escapeHtml(r.name)}</div>
          <div style="font-size:12px;color:#63636a;">${r.prep_time||0}+${r.cook_time||0} min · ${r.calories||'?'} kcal</div>
          ${r.tags ? `<div class="dev-card-meta">${r.tags.split(',').map(t => `<span class="badge badge-gray">${t.trim()}</span>`).join('')}</div>` : ''}
        </div>`).join('')}
      </div>`;
    document.getElementById('recipe-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add Recipe</div>
        <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="rcp-name"></div>
        <div class="form-group"><label class="form-label">Ingredients</label><textarea class="form-textarea" id="rcp-ing" rows="3"></textarea></div>
        <div class="form-group"><label class="form-label">Instructions</label><textarea class="form-textarea" id="rcp-inst" rows="3"></textarea></div>
        <div class="form-group"><label class="form-label">Prep time (min)</label><input class="form-input" id="rcp-prep" type="number"></div>
        <div class="form-group"><label class="form-label">Calories</label><input class="form-input" id="rcp-cal" type="number"></div>
        <div class="form-group"><label class="form-label">Tags (comma-separated)</label><input class="form-input" id="rcp-tags"></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="rcp-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('rcp-save')?.addEventListener('click', async () => {
        const name = document.getElementById('rcp-name')?.value?.trim();
        if (!name) return;
        try {
          await invoke('create_recipe', {
            name, description: null,
            ingredients: document.getElementById('rcp-ing')?.value||'',
            instructions: document.getElementById('rcp-inst')?.value||'',
            prepTime: parseInt(document.getElementById('rcp-prep')?.value)||null,
            cookTime: null, servings: null,
            calories: parseInt(document.getElementById('rcp-cal')?.value)||null,
            tags: document.getElementById('rcp-tags')?.value||null,
          });
          overlay.remove();
          loadRecipes(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadProducts(el) {
  try {
    const products = await invoke('get_products', { location: null, expiringSoon: false }).catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Products</h2><button class="btn-primary" id="product-add-btn">+ Add Product</button></div>
      <div id="products-list">
        ${products.map(p => {
          const exp = p.expiry_date ? new Date(p.expiry_date) : null;
          const isExpiring = exp && (exp - Date.now()) < 3 * 86400000;
          return `<div class="focus-log-item" style="${isExpiring ? 'border-left:2px solid #63636a;' : ''}">
            <span class="focus-log-title">${escapeHtml(p.name)}</span>
            <span class="badge badge-gray">${p.location||''}</span>
            ${p.quantity ? `<span style="color:#a1a1a6;font-size:12px;">${p.quantity} ${p.unit||''}</span>` : ''}
            ${exp ? `<span style="color:${isExpiring?'#63636a':'#a1a1a6'};font-size:11px;">${p.expiry_date}</span>` : ''}
            <button class="memory-item-btn" data-pdel="${p.id}">&times;</button>
          </div>`;
        }).join('')}
      </div>`;
    el.querySelectorAll('[data-pdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_product', { id: parseInt(btn.dataset.pdel) }).catch(()=>{});
        loadProducts(el);
      });
    });
    document.getElementById('product-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add Product</div>
        <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="prod-name"></div>
        <div class="form-group"><label class="form-label">Location</label>
          <select class="form-select" id="prod-loc" style="width:100%;">
            <option value="fridge">Fridge</option><option value="freezer">Freezer</option>
            <option value="pantry">Pantry</option><option value="other">Other</option>
          </select></div>
        <div class="form-group"><label class="form-label">Quantity</label><input class="form-input" id="prod-qty" type="number"></div>
        <div class="form-group"><label class="form-label">Unit</label><input class="form-input" id="prod-unit" placeholder="pcs, kg, L..."></div>
        <div class="form-group"><label class="form-label">Expiry Date</label><input class="form-input" id="prod-exp" type="date"></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="prod-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('prod-save')?.addEventListener('click', async () => {
        const name = document.getElementById('prod-name')?.value?.trim();
        if (!name) return;
        try {
          await invoke('add_product', {
            name, category: null,
            quantity: parseFloat(document.getElementById('prod-qty')?.value)||null,
            unit: document.getElementById('prod-unit')?.value||null,
            expiryDate: document.getElementById('prod-exp')?.value||null,
            location: document.getElementById('prod-loc')?.value||'fridge',
            barcode: null, notes: null,
          });
          overlay.remove();
          loadProducts(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

// ── Money ──
async function loadMoney(subTab) {
  const el = document.getElementById('money-content');
  if (!el) return;
  if (subTab === 'Expenses' || subTab === 'Income') loadTransactions(el, subTab === 'Income' ? 'income' : 'expense');
  else if (subTab === 'Budget') loadBudgets(el);
  else if (subTab === 'Savings') loadSavings(el);
  else if (subTab === 'Subscriptions') loadSubscriptions(el);
  else if (subTab === 'Debts') loadDebts(el);
  else loadTransactions(el, 'expense');
}

async function loadTransactions(el, txType) {
  try {
    const items = await invoke('get_transactions', { txType, category: null, days: 30 }).catch(() => []);
    const stats = await invoke('get_transaction_stats', { days: 30 }).catch(() => ({}));
    const isExpense = txType === 'expense';
    el.innerHTML = `
      <div class="module-header"><h2>${isExpense ? 'Expenses' : 'Income'}</h2><button class="btn-primary" id="tx-add-btn">+ Add</button></div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${isExpense ? (stats.total_expenses||0) : (stats.total_income||0)}</div><div class="dashboard-stat-label">Total (30d)</div></div>
      </div>
      <div id="tx-list">
        ${items.map(t => `<div class="focus-log-item">
          <span class="focus-log-time">${t.date}</span>
          <span class="focus-log-title">${escapeHtml(t.description||t.category)}</span>
          <span class="badge badge-gray">${t.category}</span>
          <span class="focus-log-duration" style="color:${isExpense?'#63636a':'#fafafa'}">${isExpense?'-':'+'} ${t.amount} ${t.currency||'KZT'}</span>
          <button class="memory-item-btn" data-txdel="${t.id}">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-txdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_transaction', { id: parseInt(btn.dataset.txdel) }).catch(()=>{});
        loadTransactions(el, txType);
      });
    });
    document.getElementById('tx-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add ${isExpense ? 'Expense' : 'Income'}</div>
        <div class="form-group"><label class="form-label">Amount</label><input class="form-input" id="tx-amount" type="number" step="0.01"></div>
        <div class="form-group"><label class="form-label">Category</label><input class="form-input" id="tx-category" placeholder="food, transport, salary..."></div>
        <div class="form-group"><label class="form-label">Description</label><input class="form-input" id="tx-desc"></div>
        <div class="form-group"><label class="form-label">Currency</label>
          <select class="form-select" id="tx-currency" style="width:100%;">
            <option value="KZT">KZT</option><option value="USD">USD</option><option value="RUB">RUB</option>
          </select></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="tx-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('tx-save')?.addEventListener('click', async () => {
        const amount = parseFloat(document.getElementById('tx-amount')?.value);
        if (!amount) return;
        try {
          await invoke('add_transaction', {
            date: null, txType, amount,
            currency: document.getElementById('tx-currency')?.value||'KZT',
            category: document.getElementById('tx-category')?.value||'other',
            description: document.getElementById('tx-desc')?.value||'',
            recurring: false, recurringPeriod: null,
          });
          overlay.remove();
          loadTransactions(el, txType);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadBudgets(el) {
  try {
    const budgets = await invoke('get_budgets').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Budgets</h2><button class="btn-primary" id="budget-add-btn">+ Add Budget</button></div>
      <div id="budgets-list">
        ${budgets.map(b => {
          const pct = b.amount > 0 ? Math.min(100, Math.round((b.spent||0) / b.amount * 100)) : 0;
          const warn = pct > 80;
          return `<div class="settings-section" style="margin-bottom:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="color:#fafafa;font-size:14px;">${escapeHtml(b.category)}</span>
              <span style="color:${warn?'#63636a':'#a1a1a6'};font-size:12px;">${b.spent||0} / ${b.amount} (${b.period})</span>
            </div>
            <div class="dev-progress" style="margin-top:6px;"><div class="dev-progress-bar" style="width:${pct}%;background:${warn?'#63636a':'#fafafa'}"></div></div>
          </div>`;
        }).join('')}
      </div>`;
    document.getElementById('budget-add-btn')?.addEventListener('click', () => {
      const cat = prompt('Category:');
      const amt = prompt('Amount:');
      if (cat && amt) invoke('create_budget', { category: cat, amount: parseFloat(amt), period: 'monthly' }).then(() => loadBudgets(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadSavings(el) {
  try {
    const goals = await invoke('get_savings_goals').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Savings Goals</h2><button class="btn-primary" id="savings-add-btn">+ Add Goal</button></div>
      <div id="savings-list">
        ${goals.map(g => {
          const pct = g.target_amount > 0 ? Math.min(100, Math.round(g.current_amount / g.target_amount * 100)) : 0;
          return `<div class="settings-section" style="margin-bottom:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="color:#fafafa;font-size:14px;">${escapeHtml(g.name)}</span>
              <span style="color:#a1a1a6;font-size:12px;">${g.current_amount} / ${g.target_amount}</span>
            </div>
            <div class="dev-progress" style="margin-top:6px;"><div class="dev-progress-bar" style="width:${pct}%"></div></div>
            ${g.deadline ? `<div style="font-size:11px;color:#3f3f44;margin-top:4px;">Deadline: ${g.deadline}</div>` : ''}
            <button class="btn-secondary" style="margin-top:6px;font-size:11px;padding:4px 10px;" data-sadd="${g.id}">+ Add funds</button>
          </div>`;
        }).join('')}
      </div>`;
    el.querySelectorAll('[data-sadd]').forEach(btn => {
      btn.addEventListener('click', async () => {
        const amount = prompt('Amount to add:');
        if (amount) {
          const goal = goals.find(g => g.id === parseInt(btn.dataset.sadd));
          if (goal) {
            await invoke('update_savings_goal', { id: goal.id, currentAmount: (goal.current_amount||0) + parseFloat(amount), name: null, targetAmount: null, deadline: null }).catch(e => alert(e));
            loadSavings(el);
          }
        }
      });
    });
    document.getElementById('savings-add-btn')?.addEventListener('click', () => {
      const name = prompt('Goal name:');
      const target = prompt('Target amount:');
      if (name && target) invoke('create_savings_goal', { name, targetAmount: parseFloat(target), currentAmount: 0, deadline: null, color: '#e0e0e0' }).then(() => loadSavings(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadSubscriptions(el) {
  try {
    const subs = await invoke('get_subscriptions').catch(() => []);
    const monthly = subs.filter(s => s.active).reduce((sum, s) => sum + (s.period === 'yearly' ? s.amount/12 : s.amount), 0);
    el.innerHTML = `
      <div class="module-header"><h2>Subscriptions</h2><button class="btn-primary" id="sub-add-btn">+ Add</button></div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${Math.round(monthly)}</div><div class="dashboard-stat-label">/month</div></div>
      </div>
      <div id="subs-list">
        ${subs.map(s => `<div class="focus-log-item">
          <span class="focus-log-title">${escapeHtml(s.name)}</span>
          <span class="badge ${s.active?'badge-green':'badge-gray'}">${s.active?'Active':'Paused'}</span>
          <span class="focus-log-duration">${s.amount} ${s.currency||'KZT'}/${s.period}</span>
          ${s.next_payment ? `<span style="color:#3f3f44;font-size:11px;">Next: ${s.next_payment}</span>` : ''}
        </div>`).join('')}
      </div>`;
    document.getElementById('sub-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add Subscription</div>
        <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="sub-name"></div>
        <div class="form-group"><label class="form-label">Amount</label><input class="form-input" id="sub-amount" type="number"></div>
        <div class="form-group"><label class="form-label">Period</label>
          <select class="form-select" id="sub-period" style="width:100%;"><option value="monthly">Monthly</option><option value="yearly">Yearly</option></select></div>
        <div class="form-group"><label class="form-label">Category</label><input class="form-input" id="sub-cat" placeholder="entertainment, tools..."></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="sub-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('sub-save')?.addEventListener('click', async () => {
        const name = document.getElementById('sub-name')?.value?.trim();
        if (!name) return;
        try {
          await invoke('add_subscription', {
            name, amount: parseFloat(document.getElementById('sub-amount')?.value)||0,
            currency: 'KZT', period: document.getElementById('sub-period')?.value||'monthly',
            nextPayment: null, category: document.getElementById('sub-cat')?.value||'other', active: true,
          });
          overlay.remove();
          loadSubscriptions(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadDebts(el) {
  try {
    const debts = await invoke('get_debts').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Debts</h2><button class="btn-primary" id="debt-add-btn">+ Add</button></div>
      <div id="debts-list">
        ${debts.map(d => {
          const pct = d.amount > 0 ? Math.min(100, Math.round((d.amount - d.remaining) / d.amount * 100)) : 0;
          return `<div class="settings-section" style="margin-bottom:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="color:#fafafa;font-size:14px;">${escapeHtml(d.name)}</span>
              <span class="badge ${d.type==='owe'?'badge-purple':'badge-green'}">${d.type==='owe'?'I owe':'Owed to me'}</span>
            </div>
            <div style="color:#a1a1a6;font-size:12px;margin-top:4px;">Remaining: ${d.remaining} / ${d.amount}</div>
            <div class="dev-progress" style="margin-top:4px;"><div class="dev-progress-bar" style="width:${pct}%"></div></div>
          </div>`;
        }).join('')}
      </div>`;
    document.getElementById('debt-add-btn')?.addEventListener('click', () => {
      const name = prompt('Name:');
      const type = prompt('Type (owe/owed):') || 'owe';
      const amount = prompt('Amount:');
      if (name && amount) invoke('add_debt', { name, debtType: type, amount: parseFloat(amount), remaining: parseFloat(amount), interestRate: null, dueDate: null, description: '' }).then(() => loadDebts(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

// ── People Tab ──
async function loadPeople(subTab) {
  const el = document.getElementById('people-content');
  if (!el) return;
  const filter = subTab === 'Blocked' ? { blocked: true } : subTab === 'Favorites' ? {} : {};
  try {
    const items = await invoke('get_contacts', filter);
    let contacts = Array.isArray(items) ? items : [];
    if (subTab === 'Favorites') contacts = contacts.filter(c => c.favorite);
    // Load blocks for each contact
    for (const c of contacts) {
      try { c._blocks = await invoke('get_contact_blocks', { contactId: c.id }); } catch { c._blocks = []; }
    }
    el.innerHTML = `
      <div class="module-header">
        <h2>${subTab === 'Blocked' ? 'Blocked' : subTab === 'Favorites' ? 'Favorites' : 'All Contacts'}</h2>
        <button class="btn-primary" id="add-contact-btn">+ Add</button>
      </div>
      <div class="contacts-list">
        ${contacts.length === 0 ? '<div class="tab-stub"><div class="tab-stub-icon">👤</div>No contacts yet</div>' :
          contacts.map(c => `
            <div class="contact-item${c.blocked ? ' blocked' : ''}${c.favorite ? ' favorite' : ''}">
              <div class="contact-avatar">${c.name.charAt(0).toUpperCase()}</div>
              <div class="contact-info">
                <div class="contact-name">${c.name}${c.favorite ? ' ★' : ''}</div>
                <div class="contact-detail">${c.relationship || c.category || ''}${c.phone ? ' · ' + c.phone : ''}${c.email ? ' · ' + c.email : ''}</div>
                ${c.blocked ? '<span class="badge badge-red">Blocked</span>' : ''}
                ${c.block_reason ? '<div class="contact-detail" style="color:#63636a">' + c.block_reason + '</div>' : ''}
                ${c.notes ? '<div class="contact-detail">' + c.notes + '</div>' : ''}
                ${c._blocks && c._blocks.length > 0 ? `
                  <div class="contact-blocks-list">
                    ${c._blocks.map(b => `
                      <div class="contact-block-item">
                        <span class="contact-block-type">${b.block_type === 'app' ? 'App' : 'Site'}</span>
                        <span class="contact-block-value">${b.value}</span>
                        ${b.reason ? '<span class="contact-block-reason">' + b.reason + '</span>' : ''}
                        <button class="contact-block-del" onclick="deleteContactBlock(${b.id})">✕</button>
                      </div>
                    `).join('')}
                  </div>` : ''}
              </div>
              <div class="contact-actions">
                <button class="btn-secondary" onclick="showContactBlockModal(${c.id}, '${c.name.replace(/'/g, "\\'")}')" title="Block sites/apps">🔗</button>
                <button class="btn-secondary" onclick="toggleContactFav(${c.id})" title="${c.favorite ? 'Unfavorite' : 'Favorite'}">${c.favorite ? '★' : '☆'}</button>
                <button class="btn-secondary" onclick="toggleContactBlock(${c.id})" title="${c.blocked ? 'Unblock' : 'Block'}">${c.blocked ? '🔓' : '🚫'}</button>
                <button class="btn-danger" onclick="deleteContact(${c.id})" style="padding:8px 12px">✕</button>
              </div>
            </div>
          `).join('')}
      </div>`;
    document.getElementById('add-contact-btn')?.addEventListener('click', showAddContactModal);
  } catch (e) {
    el.innerHTML = `<div class="tab-stub"><div class="tab-stub-icon">⚠️</div>${e}</div>`;
  }
}

window.toggleContactFav = async (id) => {
  await invoke('toggle_contact_favorite', { id });
  loadPeople(activeSubTab.people || 'All');
};
window.toggleContactBlock = async (id) => {
  await invoke('toggle_contact_blocked', { id });
  loadPeople(activeSubTab.people || 'All');
};
window.deleteContact = async (id) => {
  if (confirm('Delete this contact?')) {
    await invoke('delete_contact', { id });
    loadPeople(activeSubTab.people || 'All');
  }
};
window.deleteContactBlock = async (id) => {
  await invoke('delete_contact_block', { id });
  loadPeople(activeSubTab.people || 'All');
};
window.showContactBlockModal = (contactId, contactName) => {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div class="modal-title">Block site/app for ${contactName}</div>
      <div class="form-group"><label class="form-label">Type</label>
        <select class="form-select" id="cb-type" style="width:100%">
          <option value="site">Site</option><option value="app">App</option>
        </select>
      </div>
      <div class="form-group"><label class="form-label">Value *</label><input class="form-input" id="cb-value" placeholder="e.g. instagram.com or Instagram"></div>
      <div class="form-group"><label class="form-label">Reason</label><input class="form-input" id="cb-reason" placeholder="Why block?"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="cb-cancel">Cancel</button>
        <button class="btn-primary" id="cb-save">Add</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#cb-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#cb-save').onclick = async () => {
    const value = document.getElementById('cb-value').value.trim();
    if (!value) return;
    await invoke('add_contact_block', {
      contactId,
      blockType: document.getElementById('cb-type').value,
      value,
      reason: document.getElementById('cb-reason').value.trim() || null,
    });
    overlay.remove();
    loadPeople(activeSubTab.people || 'All');
  };
};

function showAddContactModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div class="modal-title">Add Contact</div>
      <div class="form-group"><label class="form-label">Name *</label><input class="form-input" id="mc-name" placeholder="Name"></div>
      <div class="form-group"><label class="form-label">Phone</label><input class="form-input" id="mc-phone" placeholder="Phone"></div>
      <div class="form-group"><label class="form-label">Email</label><input class="form-input" id="mc-email" placeholder="Email"></div>
      <div class="form-group"><label class="form-label">Category</label>
        <select class="form-select" id="mc-category" style="width:100%">
          <option value="friend">Friend</option><option value="family">Family</option><option value="work">Work</option>
          <option value="spammer">Spammer</option><option value="other">Other</option>
        </select>
      </div>
      <div class="form-group"><label class="form-label">Relationship</label><input class="form-input" id="mc-rel" placeholder="e.g. College friend"></div>
      <div class="form-group"><label class="form-label">Notes</label><textarea class="form-textarea" id="mc-notes" placeholder="Notes"></textarea></div>
      <div class="form-group" style="display:flex;align-items:center;gap:8px">
        <input type="checkbox" id="mc-blocked"><label for="mc-blocked" style="color:#a1a1a6;font-size:13px">Block this contact</label>
      </div>
      <div class="form-group"><label class="form-label">Block reason</label><input class="form-input" id="mc-reason" placeholder="Why blocked?"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="mc-cancel">Cancel</button>
        <button class="btn-primary" id="mc-save">Save</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#mc-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#mc-save').onclick = async () => {
    const name = document.getElementById('mc-name').value.trim();
    if (!name) return;
    await invoke('add_contact', {
      name,
      phone: document.getElementById('mc-phone').value.trim() || null,
      email: document.getElementById('mc-email').value.trim() || null,
      category: document.getElementById('mc-category').value,
      relationship: document.getElementById('mc-rel').value.trim() || null,
      notes: document.getElementById('mc-notes').value.trim() || null,
      blocked: document.getElementById('mc-blocked').checked,
      blockReason: document.getElementById('mc-reason').value.trim() || null,
    });
    overlay.remove();
    loadPeople(activeSubTab.people || 'All');
  };
}

// ── Memory Tab ──
async function loadMemoryTab(subTab) {
  const el = document.getElementById('memory-content');
  if (!el) return;
  if (subTab === 'Search') loadMemorySearch(el);
  else loadAllFacts(el);
}

async function loadAllFacts(el) {
  try {
    const memories = await invoke('get_all_memories', { search: null }).catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Memory</h2></div>
      <div class="memory-browser" id="memory-all-list">
        ${memories.map(m => `<div class="memory-item">
          <span class="memory-item-category">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
          <div class="memory-item-actions"><button class="memory-item-btn" data-mid="${m.id}">&times;</button></div>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-mid]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (confirm('Delete?')) { await invoke('delete_memory', { id: parseInt(btn.dataset.mid) }).catch(()=>{}); loadAllFacts(el); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadMemorySearch(el) {
  el.innerHTML = `
    <div class="module-header"><h2>Memory Search</h2></div>
    <div class="memory-search-box" style="margin-bottom:16px;">
      <input class="form-input" id="mem-search-input" placeholder="Search memories..." autocomplete="off">
    </div>
    <div class="memory-browser" id="mem-search-results"></div>`;
  document.getElementById('mem-search-input')?.addEventListener('input', async (e) => {
    clearTimeout(convSearchTimeout);
    convSearchTimeout = setTimeout(async () => {
      const q = e.target.value;
      if (!q || q.length < 2) return;
      try {
        const results = await invoke('get_all_memories', { search: q });
        const list = document.getElementById('mem-search-results');
        if (list) list.innerHTML = results.map(m => `<div class="memory-item">
          <span class="memory-item-category">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
        </div>`).join('') || '<div style="color:#3f3f44;font-size:12px;padding:8px;">No results</div>';
      } catch (_) {}
    }, 300);
  });
}

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

let integrationsLoaded = false;
async function loadIntegrations(force) {
  const integrationsContent = document.getElementById('settings-content');
  if (!integrationsContent) return;
  if (!force) integrationsContent.innerHTML = '<div style="color:#3f3f44;font-size:13px;">Загрузка...</div>';
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

    const appleCalEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
    const googleIcsUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');

    integrationsContent.innerHTML = `
      <div class="integrations-grid">
        <div class="integration-card macos-card">
          <div class="integration-card-title">Календари</div>
          <div class="settings-row">
            <span class="settings-label">Apple Calendar</span>
            <label class="toggle"><input type="checkbox" id="int-apple-cal" ${appleCalEnabled !== 'false' ? 'checked' : ''}><span class="toggle-track"></span></label>
          </div>
          <div class="panel-item-detail" style="margin-bottom:8px;">Синхронизирует события из Calendar.app (включая Google, если добавлен в macOS)</div>
          <div class="settings-row">
            <span class="settings-label">Google Calendar ICS</span>
          </div>
          <div style="display:flex;gap:8px;margin-bottom:8px;">
            <input class="form-input" id="int-google-ics" placeholder="https://calendar.google.com/...basic.ics" value="${escapeHtml(googleIcsUrl)}" style="flex:1">
            <button class="btn-secondary" id="int-cal-save">Сохранить</button>
          </div>
          <div class="panel-item-detail">Вставьте приватный ICS URL из Google Calendar (Настройки → Настройки календаря → Секретный адрес)</div>
        </div>
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

    // Calendar integration handlers
    document.getElementById('int-apple-cal')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'apple_calendar_enabled', value: e.target.checked ? 'true' : 'false' });
    });
    document.getElementById('int-cal-save')?.addEventListener('click', async () => {
      const url = document.getElementById('int-google-ics')?.value.trim() || '';
      await invoke('set_app_setting', { key: 'google_calendar_ics_url', value: url });
      const btn = document.getElementById('int-cal-save');
      if (btn) { btn.textContent = '✓'; setTimeout(() => btn.textContent = 'Сохранить', 1500); }
    });

    // Load memory browser
    loadMemoryBrowser();

    integrationsLoaded = true;
  } catch (e) {
    integrationsContent.innerHTML = `<div style="color:#63636a;font-size:13px;">Ошибка: ${e}</div>`;
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
      list.innerHTML = '<div style="color:#3f3f44;font-size:12px;padding:8px;">Нет фактов</div>';
    }
  } catch (_) {}

  // Bind search
  document.getElementById('memory-search')?.addEventListener('input', (e) => {
    clearTimeout(convSearchTimeout);
    convSearchTimeout = setTimeout(() => loadMemoryBrowser(e.target.value), 300);
  });
}

// ── Settings page ──

async function loadSettings(subTab) {
  const settingsContent = document.getElementById('settings-content');
  if (!settingsContent) return;
  if (subTab === 'Blocklist') { loadBlocklist(settingsContent); return; }
  if (subTab === 'Integrations') { loadIntegrations(); return; }
  if (subTab === 'About') { loadAbout(settingsContent); return; }
  settingsContent.innerHTML = '<div style="color:#3f3f44;font-size:13px;">Загрузка...</div>';
  try {
    const [info, trainingStats] = await Promise.all([
      invoke('get_model_info'),
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
    settingsContent.innerHTML = `<div style="color:#63636a;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

// ── Blocklist (Settings sub-tab) ──
async function loadBlocklist(el) {
  try {
    const items = await invoke('get_blocklist').catch(() => []);
    const sites = items.filter(i => i.type === 'site');
    const apps = items.filter(i => i.type === 'app');
    el.innerHTML = `
      <div class="module-header"><h2>Blocklist</h2><button class="btn-primary" id="bl-add-btn">+ Add</button></div>
      <div class="module-card-title">Sites</div>
      <div id="bl-sites">${sites.map(s => `<div class="focus-log-item">
        <span class="focus-log-title">${escapeHtml(s.value)}</span>
        <label class="toggle"><input type="checkbox" data-toggle="${s.id}" ${s.active?'checked':''}><span class="toggle-slider"></span></label>
        ${s.schedule ? `<span style="color:#3f3f44;font-size:11px;">${s.schedule}</span>` : ''}
        <button class="memory-item-btn" data-bldel="${s.id}">&times;</button>
      </div>`).join('') || '<div style="color:#3f3f44;font-size:12px;padding:4px 0;">None</div>'}</div>
      <div class="module-card-title" style="margin-top:16px;">Apps</div>
      <div id="bl-apps">${apps.map(a => `<div class="focus-log-item">
        <span class="focus-log-title">${escapeHtml(a.value)}</span>
        <label class="toggle"><input type="checkbox" data-toggle="${a.id}" ${a.active?'checked':''}><span class="toggle-slider"></span></label>
        <button class="memory-item-btn" data-bldel="${a.id}">&times;</button>
      </div>`).join('') || '<div style="color:#3f3f44;font-size:12px;padding:4px 0;">None</div>'}</div>`;
    el.querySelectorAll('[data-toggle]').forEach(cb => {
      cb.addEventListener('change', async () => {
        await invoke('toggle_blocklist_item', { id: parseInt(cb.dataset.toggle) }).catch(()=>{});
      });
    });
    el.querySelectorAll('[data-bldel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('remove_from_blocklist', { id: parseInt(btn.dataset.bldel) }).catch(()=>{});
        loadBlocklist(el);
      });
    });
    document.getElementById('bl-add-btn')?.addEventListener('click', () => {
      const type = prompt('Type (site/app):') || 'site';
      const value = prompt(type === 'site' ? 'Domain (e.g. youtube.com):' : 'App name (e.g. Discord):');
      if (value) invoke('add_to_blocklist', { blockType: type, value, schedule: null }).then(() => loadBlocklist(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

// ── About (Settings sub-tab) ──
async function loadAbout(el) {
  try {
    const info = await invoke('get_model_info').catch(() => ({}));
    const trainingStats = await invoke('get_training_stats').catch(() => ({ conversations: 0, total_messages: 0 }));
    el.innerHTML = `
      <div class="settings-section">
        <div class="settings-section-title">Hanni v${APP_VERSION}</div>
        <div class="settings-row"><span class="settings-label">Model</span><span class="settings-value">${info.model_name||'?'}</span></div>
        <div class="settings-row"><span class="settings-label">Server</span><span class="settings-value ${info.server_online?'online':'offline'}">${info.server_online?'Online':'Offline'}</span></div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Training Data</div>
        <div class="settings-row"><span class="settings-label">Conversations</span><span class="settings-value">${trainingStats.conversations}</span></div>
        <div class="settings-row"><span class="settings-label">Messages</span><span class="settings-value">${trainingStats.total_messages}</span></div>
        <div class="settings-row"><span class="settings-label">Export</span><button class="settings-btn" id="about-export-btn">Export JSONL</button></div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">HTTP API</div>
        <div class="settings-row"><span class="settings-label">Address</span><span class="settings-value">127.0.0.1:8235</span></div>
        <div class="settings-row"><span class="settings-label">Status</span><span class="settings-value" id="about-api-status">Checking...</span></div>
      </div>`;
    document.getElementById('about-export-btn')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Exporting...'; btn.disabled = true;
      try { const r = await invoke('export_training_data'); btn.textContent = `${r.train_count} train + ${r.valid_count} valid`; }
      catch (err) { btn.textContent = String(err).substring(0, 30); }
      setTimeout(() => { btn.textContent = 'Export JSONL'; btn.disabled = false; }, 4000);
    });
    try {
      const resp = await fetch('http://127.0.0.1:8235/api/status');
      const apiEl = document.getElementById('about-api-status');
      if (apiEl) { apiEl.textContent = resp.ok ? 'Active' : 'Unavailable'; apiEl.className = 'settings-value ' + (resp.ok ? 'online' : 'offline'); }
    } catch (_) {
      const apiEl = document.getElementById('about-api-status');
      if (apiEl) { apiEl.textContent = 'Unavailable'; apiEl.className = 'settings-value offline'; }
    }
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

// ── Tab loaders (stubs) ──
function showStub(containerId, icon, label) {
  const el = document.getElementById(containerId);
  if (el) el.innerHTML = `<div class="tab-stub"><div class="tab-stub-icon">${icon}</div>${label}</div>`;
}

// ── Dashboard ──
async function loadDashboard() {
  const el = document.getElementById('dashboard-content');
  if (!el) return;
  el.innerHTML = '<div style="color:#3f3f44;font-size:13px;">Загрузка...</div>';
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
async function loadFocus() {
  const el = document.getElementById('focus-content');
  if (!el) return;
  el.innerHTML = '<div style="color:#3f3f44;font-size:13px;">Загрузка...</div>';
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
            <label style="font-size:12px;color:#a1a1a6;display:flex;align-items:center;gap:6px;">
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
        if (!timerEl || activeTab !== 'focus') { clearInterval(focusTimerInterval); return; }
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
async function loadNotes(subTab) {
  const el = document.getElementById('notes-content');
  if (!el) return;
  try {
    const filter = subTab === 'Pinned' ? 'pinned' : subTab === 'Archived' ? 'archived' : null;
    const notes = await invoke('get_notes', { filter, search: null });
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
let syncedMonths = new Set();
async function loadCalendar(subTab) {
  const el = document.getElementById('calendar-content');
  if (!el) return;
  if (subTab === 'Интеграции') { renderCalendarIntegrations(el); return; }
  if (subTab === 'Список') { renderCalendarList(el); return; }

  // Auto-sync when navigating to a month not yet synced
  const monthKey = `${calendarYear}-${calendarMonth + 1}`;
  if (!syncedMonths.has(monthKey)) {
    syncedMonths.add(monthKey);
    const autoSync = await invoke('get_app_setting', { key: 'calendar_autosync' }).catch(() => 'false');
    if (autoSync === 'true') {
      const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
      const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');
      const syncAndRefresh = async () => {
        try {
          if (appleEnabled !== 'false') {
            const r = await invoke('sync_apple_calendar', { month: calendarMonth + 1, year: calendarYear });
            if (r.error) console.warn('Apple Calendar:', r.error);
          }
          if (googleUrl) await invoke('sync_google_ics', { url: googleUrl, month: calendarMonth + 1, year: calendarYear });
          // Refresh view after background sync completes
          const freshEvents = await invoke('get_events', { month: calendarMonth + 1, year: calendarYear }).catch(() => []);
          const calEl = document.getElementById('calendar-content');
          if (calEl && subTab === 'Список') renderCalendarList(calEl);
          else if (calEl && !subTab || subTab === 'Месяц') renderCalendar(calEl, freshEvents || []);
          else if (calEl && subTab === 'Неделя') renderWeekCalendar(calEl, freshEvents || []);
          else if (calEl && subTab === 'День') renderDayCalendar(calEl, freshEvents || []);
        } catch (e) { console.error('Auto-sync error:', e); }
      };
      syncAndRefresh(); // fire and forget — non-blocking
    }
  }

  try {
    const events = await invoke('get_events', { month: calendarMonth + 1, year: calendarYear }).catch(() => []);
    if (subTab === 'Неделя') {
      renderWeekCalendar(el, events || []);
    } else if (subTab === 'День') {
      renderDayCalendar(el, events || []);
    } else {
      renderCalendar(el, events || []);
    }
  } catch (e) {
    if (subTab === 'Неделя') renderWeekCalendar(el, []);
    else if (subTab === 'День') renderDayCalendar(el, []);
    else renderCalendar(el, []);
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
    const dots = dayEvents.slice(0, 3).map(e => `<span class="calendar-event-dot" style="background:${e.color || '#fafafa'}"></span>`).join('');
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
        ${e.source && e.source !== 'manual' ? `<span class="badge badge-gray">${e.source === 'apple' ? '🍎' : '📅'}</span>` : ''}
      </div>`).join('')}
    </div>`;
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="cal-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[calendarMonth]} ${calendarYear}</div>
      <button class="calendar-nav-btn" id="cal-next">&gt;</button>
      <button class="btn-primary" id="cal-add-event" style="margin-left:16px;">+ Событие</button>
      <button class="btn-secondary" id="cal-sync" style="margin-left:8px;">&#x21BB; Синхр.</button>
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
  document.getElementById('cal-sync')?.addEventListener('click', async () => {
    const btn = document.getElementById('cal-sync');
    if (btn) { btn.textContent = '...'; btn.disabled = true; }
    try {
      const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
      const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');
      let total = 0;
      let syncError = null;
      if (appleEnabled !== 'false') {
        const r = await invoke('sync_apple_calendar', { month: calendarMonth + 1, year: calendarYear });
        if (r.error) syncError = r.error;
        else total += r.synced || 0;
      }
      if (googleUrl) {
        const r = await invoke('sync_google_ics', { url: googleUrl, month: calendarMonth + 1, year: calendarYear });
        total += r.synced || 0;
      }
      if (syncError) {
        if (btn) { btn.textContent = '✗'; btn.title = syncError; }
        console.error('Calendar sync:', syncError);
      } else {
        if (btn) btn.textContent = `✓ ${total}`;
      }
      loadCalendar();
    } catch (e) {
      if (btn) btn.textContent = '✗';
      console.error('Calendar sync error:', e);
    }
    setTimeout(() => { if (btn) { btn.textContent = '↻ Синхр.'; btn.disabled = false; } }, 2000);
  });
}

function renderWeekCalendar(el, events) {
  const weekdays = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
  const today = new Date();
  // Get start of current week (Monday)
  const dayOfWeek = today.getDay() || 7;
  const weekStart = new Date(today);
  weekStart.setDate(today.getDate() - dayOfWeek + 1 + (calWeekOffset || 0) * 7);

  const eventsByDate = {};
  for (const ev of events) {
    if (!eventsByDate[ev.date]) eventsByDate[ev.date] = [];
    eventsByDate[ev.date].push(ev);
  }

  const todayStr = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
  const hours = Array.from({length: 16}, (_, i) => i + 7); // 7:00 - 22:00

  let daysHeader = '';
  let dayDates = [];
  for (let i = 0; i < 7; i++) {
    const d = new Date(weekStart);
    d.setDate(weekStart.getDate() + i);
    const dateStr = `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
    dayDates.push(dateStr);
    const isToday = dateStr === todayStr;
    daysHeader += `<div class="week-header-day${isToday ? ' today' : ''}">
      <div class="week-header-weekday">${weekdays[i]}</div>
      <div class="week-header-date">${d.getDate()}</div>
    </div>`;
  }

  let gridHtml = '';
  for (const h of hours) {
    gridHtml += `<div class="week-time-label">${String(h).padStart(2,'0')}:00</div>`;
    for (let i = 0; i < 7; i++) {
      const dayEvts = (eventsByDate[dayDates[i]] || []).filter(e => {
        if (!e.time) return h === 9; // No time = show at 9
        const hour = parseInt(e.time.split(':')[0]);
        return hour === h;
      });
      gridHtml += `<div class="week-cell" data-date="${dayDates[i]}" data-hour="${h}">
        ${dayEvts.map(e => `<div class="week-event">${escapeHtml(e.title)}</div>`).join('')}
      </div>`;
    }
  }

  const startLabel = `${weekStart.getDate()} ${['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'][weekStart.getMonth()]}`;
  const endDate = new Date(weekStart);
  endDate.setDate(weekStart.getDate() + 6);
  const endLabel = `${endDate.getDate()} ${['янв','фев','мар','апр','май','июн','июл','авг','сен','окт','ноя','дек'][endDate.getMonth()]}`;

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="week-prev">&lt;</button>
      <div class="calendar-month-label">${startLabel} \u2014 ${endLabel} ${weekStart.getFullYear()}</div>
      <button class="calendar-nav-btn" id="week-next">&gt;</button>
      <button class="btn-secondary" id="week-today" style="margin-left:8px;">Сегодня</button>
      <button class="btn-primary" id="week-add-event" style="margin-left:8px;">+ Событие</button>
    </div>
    <div class="week-grid">
      <div class="week-time-label"></div>
      ${daysHeader}
      ${gridHtml}
    </div>`;

  document.getElementById('week-prev')?.addEventListener('click', () => { calWeekOffset = (calWeekOffset || 0) - 1; loadCalendar('Неделя'); });
  document.getElementById('week-next')?.addEventListener('click', () => { calWeekOffset = (calWeekOffset || 0) + 1; loadCalendar('Неделя'); });
  document.getElementById('week-today')?.addEventListener('click', () => { calWeekOffset = 0; loadCalendar('Неделя'); });
  document.getElementById('week-add-event')?.addEventListener('click', () => showAddEventModal());
  el.querySelectorAll('.week-cell').forEach(cell => {
    cell.addEventListener('click', () => {
      selectedCalendarDate = cell.dataset.date;
      showAddEventModal();
      setTimeout(() => {
        const timeInput = document.getElementById('event-time');
        if (timeInput) timeInput.value = `${String(cell.dataset.hour).padStart(2,'0')}:00`;
      }, 50);
    });
  });
}

function showAddEventModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новое событие</div>
    <div class="form-row"><input class="form-input" id="event-title" placeholder="Название"></div>
    <div class="form-row">
      <input class="form-input" id="event-date" type="date" value="${selectedCalendarDate || new Date().toISOString().split('T')[0]}">
      <input class="form-input" id="event-time" type="time" style="max-width:120px;">
    </div>
    <textarea class="form-textarea" id="event-desc" placeholder="Описание (необязательно)" rows="2"></textarea>
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
        color: '#a1a1a6',
      });
      overlay.remove();
      loadCalendar();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Day View ──
let calDayDate = null;
function renderDayCalendar(el, events) {
  const today = new Date();
  if (!calDayDate) calDayDate = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
  const dayEvents = events.filter(e => e.date === calDayDate).sort((a, b) => (a.time || '').localeCompare(b.time || ''));
  const d = new Date(calDayDate + 'T00:00:00');
  const dayNames = ['Воскресенье', 'Понедельник', 'Вторник', 'Среда', 'Четверг', 'Пятница', 'Суббота'];
  const monthNames = ['Января', 'Февраля', 'Марта', 'Апреля', 'Мая', 'Июня', 'Июля', 'Августа', 'Сентября', 'Октября', 'Ноября', 'Декабря'];

  const hours = Array.from({length: 17}, (_, i) => i + 6); // 6:00 - 22:00
  let timelineHtml = hours.map(h => {
    const timeStr = `${String(h).padStart(2,'0')}:`;
    const hourEvents = dayEvents.filter(e => e.time && e.time.startsWith(timeStr.slice(0,2)));
    const evtHtml = hourEvents.map(e => {
      const srcBadge = e.source && e.source !== 'manual' ? `<span class="badge badge-gray" style="margin-left:6px;">${e.source === 'apple' ? '🍎' : '📅'}</span>` : '';
      return `<div class="day-event" style="border-left:3px solid ${e.color || '#a1a1a6'};">
        <span class="day-event-title">${escapeHtml(e.title)}</span>${srcBadge}
        <span class="day-event-dur">${e.duration_minutes || 60} мин</span>
      </div>`;
    }).join('');
    return `<div class="day-hour-row">
      <div class="day-hour-label">${String(h).padStart(2,'0')}:00</div>
      <div class="day-hour-content" data-date="${calDayDate}" data-hour="${h}">${evtHtml}</div>
    </div>`;
  }).join('');

  // All-day events (no time)
  const allDay = dayEvents.filter(e => !e.time);
  const allDayHtml = allDay.length ? `<div class="day-allday">
    <div class="day-hour-label">Весь день</div>
    <div class="day-hour-content">${allDay.map(e => `<div class="day-event" style="border-left:3px solid ${e.color || '#a1a1a6'};"><span class="day-event-title">${escapeHtml(e.title)}</span></div>`).join('')}</div>
  </div>` : '';

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="day-prev">&lt;</button>
      <div class="calendar-month-label">${d.getDate()} ${monthNames[d.getMonth()]} · ${dayNames[d.getDay()]}</div>
      <button class="calendar-nav-btn" id="day-next">&gt;</button>
      <button class="btn-secondary" id="day-today" style="margin-left:8px;">Сегодня</button>
      <button class="btn-primary" id="day-add-event" style="margin-left:8px;">+ Событие</button>
    </div>
    ${allDayHtml}
    <div class="day-timeline">${timelineHtml}</div>`;

  document.getElementById('day-prev')?.addEventListener('click', () => {
    const dd = new Date(calDayDate + 'T00:00:00'); dd.setDate(dd.getDate() - 1);
    calDayDate = dd.toISOString().slice(0, 10);
    calendarMonth = dd.getMonth(); calendarYear = dd.getFullYear();
    loadCalendar('День');
  });
  document.getElementById('day-next')?.addEventListener('click', () => {
    const dd = new Date(calDayDate + 'T00:00:00'); dd.setDate(dd.getDate() + 1);
    calDayDate = dd.toISOString().slice(0, 10);
    calendarMonth = dd.getMonth(); calendarYear = dd.getFullYear();
    loadCalendar('День');
  });
  document.getElementById('day-today')?.addEventListener('click', () => {
    calDayDate = null; calendarMonth = today.getMonth(); calendarYear = today.getFullYear();
    loadCalendar('День');
  });
  document.getElementById('day-add-event')?.addEventListener('click', () => {
    selectedCalendarDate = calDayDate;
    showAddEventModal();
  });
  el.querySelectorAll('.day-hour-content').forEach(cell => {
    cell.addEventListener('click', (e) => {
      if (e.target.closest('.day-event')) return;
      selectedCalendarDate = cell.dataset.date;
      showAddEventModal();
      setTimeout(() => {
        const ti = document.getElementById('event-time');
        if (ti) ti.value = `${String(cell.dataset.hour).padStart(2,'0')}:00`;
      }, 50);
    });
  });
}

// ── Calendar List view (Notion-style table) ──
async function renderCalendarList(el) {
  const monthNames = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
  const events = await invoke('get_events', { month: calendarMonth + 1, year: calendarYear }).catch(() => []) || [];

  const sourceLabel = (s) => s === 'apple' ? '🍎 Apple' : s === 'google' ? '📅 Google' : '✏️ Вручную';
  const sourceColor = (s) => s === 'apple' ? '#34d399' : s === 'google' ? '#60a5fa' : '#fafafa';

  let rowsHtml = '';
  if (events.length === 0) {
    rowsHtml = '<tr><td colspan="6" style="text-align:center;color:#63636a;padding:24px;">Нет событий</td></tr>';
  } else {
    for (const ev of events) {
      const endTime = ev.time && ev.duration_minutes ? (() => {
        const [h, m] = ev.time.split(':').map(Number);
        const total = h * 60 + m + ev.duration_minutes;
        return `${String(Math.floor(total / 60) % 24).padStart(2,'0')}:${String(total % 60).padStart(2,'0')}`;
      })() : '';
      const timeRange = ev.time ? (endTime ? `${ev.time} – ${endTime}` : ev.time) : 'Весь день';
      rowsHtml += `<tr class="cal-list-row" data-id="${ev.id}">
        <td style="color:#fafafa;font-weight:500;">${escapeHtml(ev.title)}</td>
        <td>${ev.date}</td>
        <td>${timeRange}</td>
        <td>${ev.duration_minutes ? ev.duration_minutes + ' мин' : '—'}</td>
        <td><span style="color:${sourceColor(ev.source)};font-size:12px;">${sourceLabel(ev.source)}</span></td>
        <td style="color:#63636a;font-size:12px;">${escapeHtml(ev.category || '')}</td>
      </tr>`;
    }
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="list-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[calendarMonth]} ${calendarYear}</div>
      <button class="calendar-nav-btn" id="list-next">&gt;</button>
      <button class="btn-primary" id="list-add-event" style="margin-left:16px;">+ Событие</button>
      <span style="color:#63636a;font-size:12px;margin-left:auto;">${events.length} событий</span>
    </div>
    <div style="overflow-x:auto;">
      <table class="cal-list-table">
        <thead>
          <tr>
            <th>Название</th>
            <th>Дата</th>
            <th>Время</th>
            <th>Длит.</th>
            <th>Источник</th>
            <th>Категория</th>
          </tr>
        </thead>
        <tbody>${rowsHtml}</tbody>
      </table>
    </div>`;

  document.getElementById('list-prev')?.addEventListener('click', () => {
    calendarMonth--;
    if (calendarMonth < 0) { calendarMonth = 11; calendarYear--; }
    loadCalendar('Список');
  });
  document.getElementById('list-next')?.addEventListener('click', () => {
    calendarMonth++;
    if (calendarMonth > 11) { calendarMonth = 0; calendarYear++; }
    loadCalendar('Список');
  });
  document.getElementById('list-add-event')?.addEventListener('click', () => showAddEventModal());
  el.querySelectorAll('.cal-list-row').forEach(row => {
    row.addEventListener('click', () => {
      const ev = events.find(e => e.id === Number(row.dataset.id));
      if (ev) { selectedCalendarDate = ev.date; calDayDate = ev.date; const dd = new Date(ev.date); calendarMonth = dd.getMonth(); calendarYear = dd.getFullYear(); loadCalendar('День'); }
    });
  });
}

// ── Calendar Integrations sub-tab ──
async function renderCalendarIntegrations(el) {
  el.innerHTML = '<div style="color:#3f3f44;font-size:13px;">Загрузка...</div>';
  try {
    const appleEnabled = await invoke('get_app_setting', { key: 'apple_calendar_enabled' }).catch(() => 'true');
    const googleUrl = await invoke('get_app_setting', { key: 'google_calendar_ics_url' }).catch(() => '');

    el.innerHTML = `
      <div class="settings-section">
        <div class="settings-section-title">Apple Calendar</div>
        <div class="settings-row">
          <span class="settings-label">Синхронизация с Calendar.app</span>
          <label class="toggle"><input type="checkbox" id="calint-apple" ${appleEnabled !== 'false' ? 'checked' : ''}><span class="toggle-track"></span></label>
        </div>
        <div class="settings-row">
          <span class="settings-label" style="color:#3f3f44;font-size:12px;">Включает все календари добавленные в macOS (iCloud, Google, Exchange и др.)</span>
        </div>
        <div class="settings-row">
          <button class="btn-primary" id="calint-sync-apple">Синхронизировать сейчас</button>
          <span class="settings-value" id="calint-apple-status">—</span>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Google Calendar (ICS)</div>
        <div class="settings-row">
          <span class="settings-label" style="color:#3f3f44;font-size:12px;">Приватный ICS URL: Google Calendar → Настройки → Настройки календаря → Секретный адрес в формате iCal</span>
        </div>
        <div style="display:flex;gap:8px;padding:8px 0;">
          <input class="form-input" id="calint-google-url" placeholder="https://calendar.google.com/...basic.ics" value="${escapeHtml(googleUrl)}" style="flex:1">
        </div>
        <div class="settings-row">
          <button class="btn-primary" id="calint-save-google">Сохранить и синхронизировать</button>
          <span class="settings-value" id="calint-google-status">—</span>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Автосинхронизация</div>
        <div class="settings-row">
          <span class="settings-label">Синхронизировать при открытии календаря</span>
          <label class="toggle"><input type="checkbox" id="calint-autosync" ${(await invoke('get_app_setting', { key: 'calendar_autosync' }).catch(() => 'false')) === 'true' ? 'checked' : ''}><span class="toggle-track"></span></label>
        </div>
      </div>`;

    document.getElementById('calint-apple')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'apple_calendar_enabled', value: e.target.checked ? 'true' : 'false' });
    });
    document.getElementById('calint-autosync')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'calendar_autosync', value: e.target.checked ? 'true' : 'false' });
    });
    document.getElementById('calint-sync-apple')?.addEventListener('click', async () => {
      const btn = document.getElementById('calint-sync-apple');
      const status = document.getElementById('calint-apple-status');
      if (btn) { btn.textContent = 'Синхронизация...'; btn.disabled = true; }
      try {
        const r = await invoke('sync_apple_calendar', { month: new Date().getMonth() + 1, year: new Date().getFullYear() });
        if (r.error) {
          if (status) { status.textContent = '✗ ' + r.error; status.style.color = '#ef4444'; }
        } else {
          if (status) { status.textContent = `✓ ${r.synced} событий`; status.style.color = ''; }
        }
      } catch (e) { if (status) { status.textContent = '✗ ' + e; status.style.color = '#ef4444'; } }
      setTimeout(() => { if (btn) { btn.textContent = 'Синхронизировать сейчас'; btn.disabled = false; } }, 2000);
    });
    document.getElementById('calint-save-google')?.addEventListener('click', async () => {
      const url = document.getElementById('calint-google-url')?.value.trim() || '';
      const btn = document.getElementById('calint-save-google');
      const status = document.getElementById('calint-google-status');
      await invoke('set_app_setting', { key: 'google_calendar_ics_url', value: url });
      if (!url) { if (status) status.textContent = 'URL удалён'; return; }
      if (btn) { btn.textContent = 'Синхронизация...'; btn.disabled = true; }
      try {
        const r = await invoke('sync_google_ics', { url, month: new Date().getMonth() + 1, year: new Date().getFullYear() });
        if (status) status.textContent = `✓ ${r.synced} событий`;
      } catch (e) { if (status) status.textContent = '✗ ' + e; }
      setTimeout(() => { if (btn) { btn.textContent = 'Сохранить и синхронизировать'; btn.disabled = false; } }, 2000);
    });
  } catch (e) {
    el.innerHTML = `<div style="color:#63636a;font-size:13px;">Ошибка: ${e}</div>`;
  }
}

// ── Work ──
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
        <h2 style="font-size:16px;color:#fafafa;">${currentProjectId ? escapeHtml(projects.find(p => p.id === currentProjectId)?.name || '') : 'Выберите проект'}</h2>
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
    item.innerHTML = `<span class="work-project-dot" style="background:${p.color || '#fafafa'}"></span>
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
    if (name) invoke('create_project', { name, description: '', color: '#e0e0e0' }).then(() => loadWork()).catch(e => alert(e));
  });

  document.getElementById('new-task-btn')?.addEventListener('click', () => {
    const title = prompt('Задача:');
    if (title) invoke('create_task', { projectId: currentProjectId, title, description: '', priority: 'normal', dueDate: null }).then(() => loadWork()).catch(e => alert(e));
  });
}

// ── Development ──
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
      ${item.description ? `<div style="font-size:12px;color:#63636a;margin-bottom:6px;">${escapeHtml(item.description.substring(0, 100))}</div>` : ''}
      <div class="dev-progress"><div class="dev-progress-bar" style="width:${item.progress || 0}%"></div></div>
      <div style="font-size:11px;color:#3f3f44;margin-top:4px;">${item.progress || 0}%</div>`;
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

// ── Hobbies (Media Collections) ──
const MEDIA_TYPES = ['music','anime','manga','movie','series','cartoon','game','book','podcast'];
const MEDIA_LABELS = { music:'Music',anime:'Anime',manga:'Manga',movie:'Movies',series:'Series',cartoon:'Cartoons',game:'Games',book:'Books',podcast:'Podcasts' };
const STATUS_LABELS = { planned:'Planned',in_progress:'In Progress',completed:'Completed',on_hold:'On Hold',dropped:'Dropped' };

async function loadHobbies(subTab) {
  const el = document.getElementById('hobbies-content');
  if (!el) return;
  if (!subTab || subTab === 'Overview') {
    loadHobbiesOverview(el);
  } else {
    const mediaType = Object.entries(MEDIA_LABELS).find(([k,v]) => v === subTab)?.[0];
    if (mediaType) loadMediaList(el, mediaType);
  }
}

async function loadHobbiesOverview(el) {
  try {
    const stats = await invoke('get_media_stats', { mediaType: null }).catch(() => ({}));
    const lists = await invoke('get_user_lists').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Collections</h2></div>
      <div class="dashboard-stats">
        ${MEDIA_TYPES.map(t => `<div class="dashboard-stat"><div class="dashboard-stat-value">${stats[t] || 0}</div><div class="dashboard-stat-label">${MEDIA_LABELS[t]}</div></div>`).join('')}
      </div>
      ${lists.length > 0 ? `<div class="module-card-title" style="margin-top:16px;">Lists</div>
        <div class="hobby-grid">${lists.map(l => `<div class="hobby-card" data-list="${l.id}">
          <div class="hobby-card-name">${escapeHtml(l.name)}</div>
          <div class="hobby-card-label">${l.item_count || 0} items</div>
        </div>`).join('')}</div>` : ''}
      <div style="margin-top:16px;">
        <button class="btn-primary" id="create-list-btn">+ New List</button>
      </div>`;
    document.getElementById('create-list-btn')?.addEventListener('click', () => {
      const name = prompt('List name:');
      if (name) invoke('create_user_list', { name, description: '', color: '#e0e0e0' }).then(() => loadHobbies('Overview')).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

async function loadMediaList(el, mediaType) {
  try {
    const items = await invoke('get_media_items', { mediaType, status: mediaStatusFilter === 'all' ? null : mediaStatusFilter, hidden: false });
    const label = MEDIA_LABELS[mediaType];
    const hasEp = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
    el.innerHTML = `
      <div class="module-header"><h2>${label}</h2><button class="btn-primary" id="media-add-btn">+ Add</button></div>
      <div class="dev-filters">
        ${['all','planned','in_progress','completed','on_hold','dropped'].map(s =>
          `<button class="pill${mediaStatusFilter === s ? ' active' : ''}" data-filter="${s}">${s === 'all' ? 'All' : STATUS_LABELS[s]}</button>`
        ).join('')}
      </div>
      <table class="data-table" id="media-table">
        <thead><tr>
          <th>Title</th><th>Status</th><th>Rating</th>${hasEp ? '<th>Progress</th>' : ''}<th>Year</th>
        </tr></thead>
        <tbody id="media-tbody"></tbody>
      </table>`;
    const tbody = document.getElementById('media-tbody');
    for (const item of items) {
      const row = document.createElement('tr');
      row.className = 'data-table-row';
      row.style.cursor = 'pointer';
      const stars = item.rating ? '\u2605'.repeat(Math.round(item.rating / 2)) + '\u2606'.repeat(5 - Math.round(item.rating / 2)) : '\u2014';
      const progress = hasEp && item.total_episodes ? `${item.progress || 0}/${item.total_episodes}` : '';
      row.innerHTML = `
        <td class="data-table-title">${escapeHtml(item.title)}</td>
        <td><span class="badge ${item.status === 'completed' ? 'badge-green' : item.status === 'in_progress' ? 'badge-blue' : 'badge-gray'}">${STATUS_LABELS[item.status] || item.status}</span></td>
        <td style="color:#a1a1a6;font-size:12px;">${stars}</td>
        ${hasEp ? `<td style="font-size:12px;color:#63636a;">${progress}</td>` : ''}
        <td style="color:#63636a;font-size:12px;">${item.year || '\u2014'}</td>`;
      row.addEventListener('click', () => showMediaDetail(item, mediaType));
      tbody.appendChild(row);
    }
    if (items.length === 0) {
      tbody.innerHTML = '<tr><td colspan="5" style="text-align:center;color:#3f3f44;padding:24px;">No items yet</td></tr>';
    }
    el.querySelectorAll('.dev-filters .pill').forEach(btn => {
      btn.addEventListener('click', () => { mediaStatusFilter = btn.dataset.filter; loadMediaList(el, mediaType); });
    });
    document.getElementById('media-add-btn')?.addEventListener('click', () => showAddMediaModal(mediaType));
  } catch (e) { el.innerHTML = `<div style="color:#63636a;font-size:13px;">Error: ${e}</div>`; }
}

function showAddMediaModal(mediaType) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Add ${MEDIA_LABELS[mediaType]}</div>
    <div class="form-row"><input class="form-input" id="media-title" placeholder="Title"></div>
    <div class="form-row">
      <select class="form-select" id="media-status">
        <option value="planned">Planned</option><option value="in_progress">In Progress</option>
        <option value="completed">Completed</option><option value="on_hold">On Hold</option>
      </select>
      <input class="form-input" id="media-year" type="number" placeholder="Year" style="max-width:80px;">
      <input class="form-input" id="media-rating" type="number" min="0" max="10" placeholder="Rating" style="max-width:80px;">
    </div>
    ${hasEpisodes ? `<div class="form-row">
      <input class="form-input" id="media-progress" type="number" min="0" placeholder="Episode" style="max-width:80px;">
      <span class="form-hint">/</span>
      <input class="form-input" id="media-total" type="number" min="0" placeholder="Total" style="max-width:80px;">
    </div>` : ''}
    <textarea class="form-textarea" id="media-notes" placeholder="Notes" rows="2"></textarea>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="media-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('media-save')?.addEventListener('click', async () => {
    const title = document.getElementById('media-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('add_media_item', {
        mediaType, title,
        originalTitle: null, year: parseInt(document.getElementById('media-year')?.value) || null,
        description: null, coverUrl: null,
        status: document.getElementById('media-status')?.value || 'planned',
        rating: parseInt(document.getElementById('media-rating')?.value) || null,
        progress: hasEpisodes ? (parseInt(document.getElementById('media-progress')?.value) || null) : null,
        totalEpisodes: hasEpisodes ? (parseInt(document.getElementById('media-total')?.value) || null) : null,
        notes: document.getElementById('media-notes')?.value || null,
      });
      overlay.remove();
      loadMediaList(document.getElementById('hobbies-content'), mediaType);
    } catch (err) { alert('Error: ' + err); }
  });
}

function showMediaDetail(item, mediaType) {
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">${escapeHtml(item.title)}</div>
    ${item.year ? `<div style="color:#a1a1a6;font-size:12px;margin-bottom:8px;">${item.year}</div>` : ''}
    <div class="form-group"><label class="form-label">Status</label>
      <select class="form-select" id="md-status" style="width:100%;">
        ${Object.entries(STATUS_LABELS).map(([k,v]) => `<option value="${k}"${item.status===k?' selected':''}>${v}</option>`).join('')}
      </select></div>
    <div class="form-group"><label class="form-label">Rating (0-10)</label><input class="form-input" id="md-rating" type="number" min="0" max="10" value="${item.rating||''}"></div>
    ${hasEpisodes ? `<div class="form-group"><label class="form-label">Progress</label><input class="form-input" id="md-progress" type="number" value="${item.progress||0}"></div>` : ''}
    <div class="form-group"><label class="form-label">Notes</label><textarea class="form-textarea" id="md-notes">${escapeHtml(item.notes||'')}</textarea></div>
    <div class="modal-actions">
      <button class="btn-danger" id="md-delete">Delete</button>
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="md-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('md-save')?.addEventListener('click', async () => {
    try {
      await invoke('update_media_item', {
        id: item.id,
        status: document.getElementById('md-status')?.value || null,
        rating: parseInt(document.getElementById('md-rating')?.value) || null,
        progress: hasEpisodes ? (parseInt(document.getElementById('md-progress')?.value) || null) : null,
        notes: document.getElementById('md-notes')?.value || null,
        title: null, description: null, coverUrl: null, totalEpisodes: null,
      });
      overlay.remove();
      loadMediaList(document.getElementById('hobbies-content'), mediaType);
    } catch (err) { alert('Error: ' + err); }
  });
  document.getElementById('md-delete')?.addEventListener('click', async () => {
    if (!confirm('Delete this item?')) return;
    await invoke('delete_media_item', { id: item.id }).catch(e => alert(e));
    overlay.remove();
    loadMediaList(document.getElementById('hobbies-content'), mediaType);
  });
}

// ── Sports ──
async function loadSports(subTab) {
  const el = document.getElementById('sports-content');
  if (!el) return;
  if (subTab === 'Martial Arts') {
    loadMartialArts(el);
    return;
  }
  if (subTab === 'Stats') {
    loadSportsStats(el);
    return;
  }
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
    renderSports(el, workouts || [], stats);
  } catch (e) {
    showStub('sports-content', '&#9829;', 'Спорт — скоро');
  }
}

async function loadMartialArts(el) {
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const ma = (workouts || []).filter(w => w.type === 'martial_arts');
    el.innerHTML = `
      <div class="module-header"><h2>Единоборства</h2><button class="btn-primary" id="new-ma-btn">+ Тренировка</button></div>
      <table class="data-table">
        <thead><tr><th>Дата</th><th>Название</th><th>Время</th><th>Калории</th></tr></thead>
        <tbody id="ma-tbody"></tbody>
      </table>`;
    const tbody = document.getElementById('ma-tbody');
    if (ma.length === 0) {
      tbody.innerHTML = '<tr><td colspan="4" style="text-align:center;color:#3f3f44;padding:24px;">Нет тренировок</td></tr>';
    } else {
      for (const w of ma) {
        const row = document.createElement('tr');
        row.className = 'data-table-row';
        row.innerHTML = `<td>${w.date || '\u2014'}</td><td class="data-table-title">${escapeHtml(w.title || 'Единоборства')}</td><td>${w.duration_minutes || 0} мин</td><td>${w.calories || '\u2014'}</td>`;
        tbody.appendChild(row);
      }
    }
    document.getElementById('new-ma-btn')?.addEventListener('click', () => {
      showAddWorkoutModal();
      setTimeout(() => { const sel = document.getElementById('workout-type'); if (sel) sel.value = 'martial_arts'; }, 50);
    });
  } catch (e) { el.innerHTML = `<div style="color:#63636a;">Error: ${e}</div>`; }
}

async function loadSportsStats(el) {
  try {
    const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };
    const byType = {};
    for (const w of (workouts || [])) {
      byType[w.type] = (byType[w.type] || 0) + 1;
    }
    el.innerHTML = `
      <div class="module-header"><h2>Статистика</h2></div>
      <div class="sports-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.count || 0}</div><div class="dashboard-stat-label">Тренировок</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_minutes || 0}м</div><div class="dashboard-stat-label">Общее время</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_calories || 0}</div><div class="dashboard-stat-label">Калории</div></div>
      </div>
      <div style="margin-top:16px;">
        <h3 style="font-size:14px;color:#63636a;margin-bottom:8px;">По типам</h3>
        ${Object.entries(byType).map(([t, c]) => `<div style="display:flex;justify-content:space-between;padding:4px 0;font-size:13px;color:#a1a1a6;border-bottom:1px solid #19191b;">
          <span>${typeLabels[t] || t}</span><span style="color:#63636a;">${c}</span>
        </div>`).join('') || '<div style="color:#3f3f44;font-size:13px;">No data yet</div>'}
      </div>`;
  } catch (e) { el.innerHTML = `<div style="color:#63636a;">Error: ${e}</div>`; }
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
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новая тренировка</div>
    <div class="form-row">
      <select class="form-select" id="workout-type">
        <option value="gym">Зал</option><option value="cardio">Кардио</option>
        <option value="yoga">Йога</option><option value="swimming">Плавание</option>
        <option value="martial_arts">Единоборства</option><option value="other">Другое</option>
      </select>
      <input class="form-input" id="workout-title" placeholder="Название">
    </div>
    <div class="form-row">
      <input class="form-input" id="workout-duration" type="number" value="60" placeholder="Минуты" style="max-width:100px;">
      <span class="form-hint">мин</span>
      <input class="form-input" id="workout-calories" type="number" placeholder="Калории" style="max-width:100px;">
      <span class="form-hint">ккал</span>
    </div>
    <textarea class="form-textarea" id="workout-notes" placeholder="Заметки (необязательно)" rows="2"></textarea>
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

// ── Initialization ──
(async () => {
  // Render tab bar
  renderTabBar();
  activateView();

  // Auto-restore last conversation
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
