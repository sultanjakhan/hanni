const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;


// ── Markdown rendering setup ──
const markedInstance = new marked.Marked({
  breaks: true,
  gfm: true,
  highlight: (code, lang) => {
    if (lang && hljs.getLanguage(lang)) {
      return hljs.highlight(code, { language: lang }).value;
    }
    return hljs.highlightAuto(code).value;
  },
});
markedInstance.use({
  renderer: {
    code({ text, lang }) {
      const highlighted = lang && hljs.getLanguage(lang)
        ? hljs.highlight(text, { language: lang }).value
        : hljs.highlightAuto(text).value;
      const langLabel = lang || 'code';
      return `<div class="code-block"><div class="code-header"><span>${langLabel}</span><button class="code-copy-btn" onclick="navigator.clipboard.writeText(this.closest('.code-block').querySelector('code').textContent)">Копировать</button></div><pre><code class="hljs">${highlighted}</code></pre></div>`;
    },
    link({ href, text }) {
      return `<a href="#" onclick="event.preventDefault();window.__TAURI__.core.invoke('open_url',{url:'${href.replace(/'/g, "\\'")}'});return false;">${text}</a>`;
    },
  },
});
function renderMarkdown(text) {
  return markedInstance.parse(text);
}

const chat = document.getElementById('chat');
const input = document.getElementById('input');
const sendBtn = document.getElementById('send');
const attachBtn = document.getElementById('attach');
const fileInput = document.getElementById('file-input');
const attachPreview = document.getElementById('attach-preview');
let APP_VERSION = '?';
// Fetch real version from Tauri at startup
(async () => {
  try {
    APP_VERSION = await invoke('get_app_version');
    // Update version labels that rendered before this resolved
    document.querySelectorAll('.version-label').forEach(el => {
      el.textContent = `v${APP_VERSION}`;
    });
  } catch (_) {}
})();

let busy = false;
let history = [];
let attachedFile = null;

// Normalize history message: supports both old [role, content] tuples and new {role, content, ...} objects
function normalizeHistoryMessage(msg) {
  if (Array.isArray(msg)) {
    return { role: msg[0], content: msg[1] };
  }
  // Strip proactive flag before sending to backend (it's JS-only metadata)
  if (msg.proactive) {
    const { proactive, ...rest } = msg;
    return rest;
  }
  return msg;
}
function getRole(msg) {
  if (Array.isArray(msg)) return msg[0];
  return msg.role;
}
function getContent(msg) {
  if (Array.isArray(msg)) return msg[1];
  return msg.content || '';
}
let isRecording = false;
const VOICE_SERVER = 'http://127.0.0.1:8237';
let voiceServerAvailable = null; // null = unknown, true/false after check

async function checkVoiceServer(retries = 1) {
  for (let i = 0; i < retries; i++) {
    try {
      const r = await fetch(`${VOICE_SERVER}/health`, { signal: AbortSignal.timeout(2000) });
      if (r.ok) { voiceServerAvailable = true; return true; }
    } catch (_) {}
    if (i < retries - 1) await new Promise(r => setTimeout(r, 2000));
  }
  voiceServerAvailable = false;
  return false;
}
// Check on startup with retries (voice server may take time to start)
setTimeout(() => checkVoiceServer(5), 3000);
let currentConversationId = null;
let isSpeaking = false;
let convSearchTimeout = null;
let focusTimerInterval = null;
let focusWidgetActivity = null;
let focusWidgetOpen = false;
let focusWidgetTimerInterval = null;
let focusWidgetPollInterval = null;
let currentNoteId = null;
let noteAutoSaveTimeout = null;
let tagColorMap = {};
let noteTagFilter = null;
let notesView = localStorage.getItem('hanni_notes_view') || 'all';
let notesFilters = new Set();
let notesSearchQuery = '';
let notesTableSort = { col: 'updated_at', dir: 'desc' };
let calendarYear = new Date().getFullYear();
let calendarMonth = new Date().getMonth();
let selectedCalendarDate = null;
let calWeekOffset = 0;
let currentProjectId = null;
let devFilter = 'all';
let mediaStatusFilter = 'all';

// ── Proactive style definitions ──
const PROACTIVE_STYLE_DEFINITIONS = [
  { id: 'observation', icon: '👀', name: 'Наблюдение', desc: 'Комментарий к экрану, музыке, браузеру' },
  { id: 'calendar', icon: '📅', name: 'Календарь', desc: 'Упоминание предстоящих событий' },
  { id: 'nudge', icon: '💪', name: 'Подталкивание', desc: 'Мягкие напоминания о продуктивности/здоровье' },
  { id: 'curiosity', icon: '🤔', name: 'Любопытство', desc: 'Вопросы о дне, проекте, настроении' },
  { id: 'humor', icon: '😄', name: 'Юмор', desc: 'Лёгкие шутки о привычках' },
  { id: 'care', icon: '💙', name: 'Забота', desc: 'Проверка настроения, предложение отдыха' },
  { id: 'memory', icon: '🧠', name: 'Память', desc: 'Ссылки на сохранённые факты' },
  { id: 'food', icon: '🍽️', name: 'Еда', desc: 'Предупреждения о сроках годности' },
  { id: 'goals', icon: '🎯', name: 'Цели', desc: 'Прогресс и дедлайны по целям' },
  { id: 'journal', icon: '📝', name: 'Дневник', desc: 'Напоминание написать рефлексию' },
  { id: 'digest', icon: '☀️', name: 'Дайджест', desc: 'Утренняя сводка (8–10 утра)' },
  { id: 'accountability', icon: '⏰', name: 'Контроль', desc: 'Мягкое указание на отвлечение 30+ мин' },
  { id: 'schedule', icon: '🔔', name: 'Расписание', desc: 'Напоминание за 30 мин до события' },
  { id: 'continuity', icon: '💬', name: 'Продолжение', desc: 'Продолжение темы из чата' },
];

// ── SVG Icon set (Lucide-style, 16x16, stroke 1.5) ──
const _s = (d) => `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
const TAB_ICONS = {
  chat:        _s('<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>'),
  dashboard:   _s('<rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/>'),
  calendar:    _s('<rect x="3" y="4" width="18" height="18" rx="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/>'),
  focus:       _s('<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2"/>'),
  notes:       _s('<path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>'),
  work:        _s('<rect x="2" y="7" width="20" height="14" rx="2"/><path d="M16 7V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v2"/>'),
  development: _s('<path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/>'),
  home:        _s('<path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/>'),
  hobbies:     _s('<line x1="6" y1="11" x2="10" y2="11"/><line x1="8" y1="9" x2="8" y2="13"/><line x1="15" y1="12" x2="15.01" y2="12"/><line x1="18" y1="10" x2="18.01" y2="10"/><path d="M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z"/>'),
  sports:      _s('<path d="M14.4 14.4 9.6 9.6"/><path d="M18.657 21.485a2 2 0 1 1-2.829-2.828l-1.767 1.768a2 2 0 1 1-2.829-2.829l6.364-6.364a2 2 0 1 1 2.829 2.829l-1.768 1.767a2 2 0 1 1 2.828 2.829z"/><path d="m2.515 18.657a2 2 0 1 1 2.828 2.829l1.768-1.768a2 2 0 1 1 2.828 2.829l-6.364-6.364a2 2 0 1 1-2.828-2.829l1.767-1.768a2 2 0 1 1-2.829-2.828z"/>'),
  health:      _s('<path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/>'),
  mindset:     _s('<path d="M9.5 2A2.5 2.5 0 0 1 12 4.5v15a2.5 2.5 0 0 1-4.96.44A2.5 2.5 0 0 1 4.5 17.5a2.5 2.5 0 0 1-.44-4.96A2.5 2.5 0 0 1 4.5 9.5a2.5 2.5 0 0 1 .44-4.96A2.5 2.5 0 0 1 9.5 2z"/><path d="M14.5 2A2.5 2.5 0 0 0 12 4.5v15a2.5 2.5 0 0 0 4.96.44 2.5 2.5 0 0 0 2.54-2.94 2.5 2.5 0 0 0 .44-4.96A2.5 2.5 0 0 0 19.5 9.5a2.5 2.5 0 0 0-.44-4.96A2.5 2.5 0 0 0 14.5 2z"/>'),
  food:        _s('<path d="M3 2v7c0 1.1.9 2 2 2h4a2 2 0 0 0 2-2V2"/><path d="M7 2v20"/><path d="M21 15V2v0a5 5 0 0 0-5 5v6c0 1.1.9 2 2 2h3Zm0 0v7"/>'),
  money:       _s('<rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/>'),
  people:      _s('<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>'),
  settings:    _s('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>'),
};

// ── Tab Registry ──
const TAB_REGISTRY = {
  chat:        { label: 'Chat',        icon: TAB_ICONS.chat, closable: false, subTabs: [], subIcons: {} },
  dashboard:   { label: 'Dashboard',   icon: TAB_ICONS.dashboard, closable: true,  subTabs: ['Overview'] },
  calendar:    { label: 'Calendar',    icon: TAB_ICONS.calendar, closable: true,  subTabs: ['Месяц', 'Неделя', 'День', 'Список', 'Интеграции'] },
  focus:       { label: 'Focus',       icon: TAB_ICONS.focus, closable: true,  subTabs: ['Current', 'History'] },
  notes:       { label: 'Notes',       icon: TAB_ICONS.notes, closable: true,  subTabs: [] },
  work:        { label: 'Work',        icon: TAB_ICONS.work, closable: true,  subTabs: ['Projects'] },
  development: { label: 'Development', icon: TAB_ICONS.development, closable: true,  subTabs: ['Courses', 'Skills', 'Articles'] },
  home:        { label: 'Home',        icon: TAB_ICONS.home, closable: true,  subTabs: ['Supplies', 'Shopping List'] },
  hobbies:     { label: 'Hobbies',     icon: TAB_ICONS.hobbies, closable: true,  subTabs: ['Overview','Music','Anime','Manga','Movies','Series','Cartoons','Games','Books','Podcasts'] },
  sports:      { label: 'Sports',      icon: TAB_ICONS.sports, closable: true,  subTabs: ['Workouts', 'Martial Arts', 'Stats'] },
  health:      { label: 'Health',      icon: TAB_ICONS.health,  closable: true,  subTabs: ['Today', 'Habits'] },
  mindset:     { label: 'Mindset',     icon: TAB_ICONS.mindset, closable: true,  subTabs: ['Journal', 'Mood', 'Principles'] },
  food:        { label: 'Food',        icon: TAB_ICONS.food, closable: true,  subTabs: ['Food Log', 'Recipes', 'Products'] },
  money:       { label: 'Money',       icon: TAB_ICONS.money, closable: true,  subTabs: ['Expenses', 'Income', 'Budget', 'Savings', 'Subscriptions', 'Debts'] },
  people:      { label: 'People',      icon: TAB_ICONS.people, closable: true,  subTabs: ['All', 'Blocked', 'Favorites'] },
};

const TAB_DESCRIPTIONS = {
  dashboard: 'Overview of your day, activities, and quick actions',
  calendar: 'Events, schedules, and calendar integrations',
  focus: 'Deep work sessions and activity tracking',
  notes: 'Quick notes, ideas, and thoughts',
  work: 'Projects and tasks management',
  development: 'Courses, skills, and learning resources',
  home: 'Household supplies and shopping lists',
  hobbies: 'Media collections — track what you watch, read, play',
  sports: 'Workouts, martial arts, and fitness stats',
  health: 'Daily health metrics and habit tracking',
  mindset: 'Journal, mood tracking, and personal principles',
  food: 'Food log, recipes, and product inventory',
  money: 'Expenses, income, budgets, and savings',
  people: 'Contacts and relationship management',
};

// ── Page customization (Notion-like editable icon/description) ──
const PAGE_EMOJIS = ['📄','📝','📋','📌','📎','📁','💡','🎯','🔥','⭐','🏠','💼','🎨','🎮','📚','🎵','💰','🏋️','❤️','🧠','🍔','📅','🔧','🚀','🌟','✅','📊','🗂️','💬','🔔','🧪','🔬','📸','🎬','🎭','🎪','🏆','🗺️','🌍','🧩'];
let tabCustomizations = {};
try { tabCustomizations = JSON.parse(localStorage.getItem('hanni_tab_custom') || '{}'); } catch(_) {}

function saveTabCustom() {
  localStorage.setItem('hanni_tab_custom', JSON.stringify(tabCustomizations));
}

function getTabIcon(tabId) {
  const custom = tabCustomizations[tabId];
  if (custom?.icon) return custom.icon;
  return TAB_REGISTRY[tabId]?.icon || '';
}

function getTabDesc(tabId) {
  const custom = tabCustomizations[tabId];
  if (custom?.desc !== undefined) return custom.desc;
  return TAB_DESCRIPTIONS[tabId] || '';
}

function renderPageHeader(tabId, extra) {
  const reg = TAB_REGISTRY[tabId];
  if (!reg) return '';
  const customIcon = tabCustomizations[tabId]?.icon;
  const iconHtml = customIcon
    ? `<button class="page-header-icon-btn" data-tab-id="${tabId}" title="Сменить иконку">${customIcon}</button>`
    : `<button class="page-header-icon-btn page-header-icon-svg" data-tab-id="${tabId}" title="Сменить иконку">${reg.icon || ''}</button>`;
  const desc = extra?.description || getTabDesc(tabId);
  const props = extra?.properties || [];
  return `<div class="page-header" data-tab-id="${tabId}">
    ${iconHtml}
    <div class="page-header-title">${extra?.title || reg.label}</div>
    <input class="page-header-desc-input" data-tab-id="${tabId}" value="${escapeHtml(desc)}" placeholder="Добавить описание...">
    ${props.length ? `<div class="page-header-properties">${props.map(p =>
      `<span class="page-property"><span class="page-property-label">${p.label}</span><span class="page-property-value ${p.class || ''}">${p.value}</span></span>`
    ).join('')}</div>` : ''}
    <div class="page-emoji-picker hidden" id="page-emoji-picker-${tabId}">
      ${PAGE_EMOJIS.map(e => `<button class="emoji-pick-btn" data-emoji="${e}">${e}</button>`).join('')}
    </div>
  </div>`;
}

function setupPageHeaderControls(tabId) {
  // Icon click → toggle emoji picker
  const iconBtn = document.querySelector(`.page-header-icon-btn[data-tab-id="${tabId}"]`);
  const picker = document.getElementById(`page-emoji-picker-${tabId}`);
  if (iconBtn && picker) {
    iconBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      picker.classList.toggle('hidden');
    });
    picker.querySelectorAll('.emoji-pick-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const emoji = btn.dataset.emoji;
        if (!tabCustomizations[tabId]) tabCustomizations[tabId] = {};
        tabCustomizations[tabId].icon = emoji;
        saveTabCustom();
        iconBtn.textContent = emoji;
        iconBtn.classList.remove('page-header-icon-svg');
        picker.classList.add('hidden');
        renderTabBar();
      });
    });
    const closePicker = (e) => {
      if (!picker.contains(e.target) && e.target !== iconBtn) picker.classList.add('hidden');
    };
    document.addEventListener('click', closePicker, { once: false });
  }

  // Description edit
  const descInput = document.querySelector(`.page-header-desc-input[data-tab-id="${tabId}"]`);
  if (descInput) {
    descInput.addEventListener('input', () => {
      if (!tabCustomizations[tabId]) tabCustomizations[tabId] = {};
      tabCustomizations[tabId].desc = descInput.value;
      saveTabCustom();
    });
  }
}

let openTabs = ['chat', 'dashboard'];
let activeTab = 'chat';
let activeSubTab = {};
let chatSidebarCollapsed = !!localStorage.getItem('hanni_chat_sidebar_collapsed');

// Init default sub-tabs
for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
  if (reg.subTabs?.length) activeSubTab[id] = reg.subTabs[0];
}
activeSubTab.chat = null; // Chat view shows chat by default, not settings

// Restore from localStorage
try {
  const saved = JSON.parse(localStorage.getItem('hanni_tabs'));
  if (saved) {
    openTabs = (saved.open || ['chat', 'dashboard']).filter(id => TAB_REGISTRY[id]);
    if (!openTabs.includes('chat')) openTabs.unshift('chat');
    activeTab = TAB_REGISTRY[saved.active] ? saved.active : 'chat';
    if (saved.sub) {
      Object.assign(activeSubTab, saved.sub);
      // Clean up old 'Чат' sub-tab value
      if (activeSubTab.chat && activeSubTab.chat !== 'Настройки') activeSubTab.chat = null;
    }
  }
} catch (_) {}

function saveTabs() {
  localStorage.setItem('hanni_tabs', JSON.stringify({ open: openTabs, active: activeTab, sub: activeSubTab }));
}

// ── Auto-update notification ──
listen('update-available', (event) => {
  const version = event.payload;
  const banner = document.createElement('div');
  banner.style.cssText = 'padding:8px 16px;background:var(--bg-card);color:var(--text-secondary);font-size:12px;text-align:center;border-bottom:1px solid var(--border-default);';
  banner.textContent = `Обновление до v${version}...`;
  document.getElementById('content-area')?.prepend(banner);
});

// ── Proactive message listener ──
let lastProactiveTime = 0; // timestamp of last proactive message for engagement tracking
listen('proactive-message', async (event) => {
  // Prevent race condition: don't mutate history while chat is streaming
  if (busy) return;
  // v0.22: payload is now {text, id} JSON
  const payload = typeof event.payload === 'object' ? event.payload : { text: event.payload, id: 0 };
  const text = payload.text;
  const proactiveId = payload.id || 0;
  lastProactiveTime = Date.now();

  // Use addMsg to get proper wrapper with TTS button
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
  const histIdx = history.length;
  history.push({ role: 'assistant', content: text, proactive: true });
  scrollDown();

  // P1: Execute any action blocks from proactive messages
  const proactiveActions = parseAndExecuteActions(text);
  if (proactiveActions.length > 0) {
    for (const actionJson of proactiveActions) {
      const { success, result: actionResult } = await executeAction(actionJson);
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
    }
    history.push({ role: 'user', content: `[Action result: ${proactiveActions.map(() => 'ok').join('; ')}]` });
  }

  await autoSaveConversation();

  // Add proactive feedback buttons (copy + 👍/👎 — no regen)
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
let typingTimeout = null;
input.addEventListener('input', () => {
  invoke('set_user_typing', { typing: true }).catch(() => {});
  clearTimeout(typingTimeout);
  typingTimeout = setTimeout(() => {
    invoke('set_user_typing', { typing: false }).catch(() => {});
  }, 10000);
});

// ── Reminder notifications ──
listen('reminder-fired', (event) => {
  const title = event.payload;
  addMsg('bot', `⏰ Напоминание: ${title}`);
  scrollDown();
  if (!document.hasFocus()) {
    new Notification('Напоминание', { body: title });
  }
});

listen('note-reminder-fired', (event) => {
  const { id, title } = event.payload;
  addMsg('bot', `📝 Заметка-напоминание: **${title}**`);
  scrollDown();
  if (!document.hasFocus()) {
    new Notification('Заметка', { body: title });
  }
});

// ── Voice recording ──

const recordBtn = document.getElementById('record');

// Telegram-style: press-and-hold to record, release to send
let recordPending = null; // holds the pending /transcribe fetch promise
let lastMessageWasVoice = false;
let lastSttTimeMs = 0;
let voiceRecordStartTime = 0;

async function startRecording() {
  if (isRecording || busy) return;
  // Block proactive messages while recording
  invoke('set_recording_state', { recording: true }).catch(() => {});
  invoke('set_user_typing', { typing: true }).catch(() => {});
  await checkVoiceServer();

  if (voiceServerAvailable) {
    isRecording = true;
    voiceRecordStartTime = performance.now();
    recordBtn.classList.add('recording');
    recordBtn.title = 'Отпустите для отправки';
    // Start recording (blocks until silence or /finish)
    recordPending = fetch(`${VOICE_SERVER}/transcribe`, { method: 'POST' })
      .then(r => r.json())
      .catch(() => null);
  } else {
    // Fallback: Rust cpal + whisper-rs
    try {
      const hasModel = await invoke('check_whisper_model');
      if (!hasModel) {
        if (await confirmModal('Модель Whisper не найдена (~1.5GB). Скачать?')) {
          addMsg('bot', 'Скачиваю модель Whisper...');
          const unlisten = await listen('whisper-download-progress', (event) => {
            const msgs = chat.querySelectorAll('.msg.bot');
            const last = msgs[msgs.length - 1];
            if (last) last.textContent = `Скачиваю Whisper... ${event.payload}%`;
          });
          try { await invoke('download_whisper_model'); addMsg('bot', 'Whisper загружен!'); } catch (e) { addMsg('bot', 'Ошибка: ' + e); }
          unlisten();
        }
        return;
      }
      await invoke('start_recording');
      isRecording = true;
      recordBtn.classList.add('recording');
      recordBtn.title = 'Отпустите для отправки';
    } catch (e) { addMsg('bot', 'Ошибка записи: ' + e); }
  }
}

async function stopRecordingAndSend() {
  if (!isRecording) return;
  isRecording = false;
  recordBtn.classList.remove('recording');
  recordBtn.classList.add('transcribing');
  recordBtn.title = 'Распознаю...';
  recordBtn.disabled = true;

  if (voiceServerAvailable && recordPending) {
    // Signal voice server to finish recording (triggers transcription of collected audio)
    try { await fetch(`${VOICE_SERVER}/finish`, { method: 'POST' }); } catch (_) {}
    const data = await recordPending;
    recordPending = null;
    if (data && data.text && data.text.trim()) {
      lastMessageWasVoice = true;
      lastSttTimeMs = performance.now() - voiceRecordStartTime;
      input.value = (input.value ? input.value + ' ' : '') + data.text.trim();
      sendBtn.click();
    }
  } else {
    // Fallback: Rust whisper-rs
    try {
      const text = await invoke('stop_recording');
      if (text && text.trim()) {
        lastMessageWasVoice = true;
        lastSttTimeMs = performance.now() - voiceRecordStartTime;
        input.value = (input.value ? input.value + ' ' : '') + text.trim();
        sendBtn.click();
      }
    } catch (e) {
      if (!String(e).includes('No audio')) addMsg('bot', 'Ошибка: ' + e);
    }
  }
  recordBtn.classList.remove('transcribing');
  recordBtn.disabled = false;
  recordBtn.title = 'Удерживайте для записи';
  // Release recording lock immediately
  invoke('set_recording_state', { recording: false }).catch(() => {});
  // Release typing lock (delayed — give send() time to acquire busy flag)
  setTimeout(() => invoke('set_user_typing', { typing: false }).catch(() => {}), 3000);
}

function cancelRecording() {
  if (!isRecording) return;
  isRecording = false;
  recordPending = null;
  recordBtn.classList.remove('recording');
  recordBtn.title = 'Удерживайте для записи';
  invoke('set_recording_state', { recording: false }).catch(() => {});
  invoke('set_user_typing', { typing: false }).catch(() => {});
  if (voiceServerAvailable) {
    fetch(`${VOICE_SERVER}/stop`, { method: 'POST' }).catch(() => {});
  } else {
    invoke('stop_recording').catch(() => {});
  }
}

// Press-and-hold handlers
recordBtn.addEventListener('mousedown', (e) => { e.preventDefault(); startRecording(); });
recordBtn.addEventListener('mouseup', () => stopRecordingAndSend());
recordBtn.addEventListener('mouseleave', () => { if (isRecording) cancelRecording(); });
recordBtn.addEventListener('touchstart', (e) => { e.preventDefault(); startRecording(); }, { passive: false });
recordBtn.addEventListener('touchend', (e) => { e.preventDefault(); stopRecordingAndSend(); });

// Cancel recording with Escape
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && isRecording) {
    e.preventDefault();
    cancelRecording();
  }
});

// ── Focus mode listener ──

listen('focus-ended', () => {
  addMsg('bot', 'Фокус-режим завершён!');
  if (focusTimerInterval) {
    clearInterval(focusTimerInterval);
    focusTimerInterval = null;
  }
  updateFocusWidget();
});

// ── Conversation sidebar ──

async function loadConversationsList(searchQuery) {
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
    // Normalize messages: handle both old [role, content] and new {role, content} formats
    history = (conv.messages || []).map(normalizeHistoryMessage);
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
    for (let i = 0; i < history.length; i++) {
      const role = getRole(history[i]);
      const content = getContent(history[i]);
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
        addMsg(role === 'assistant' ? 'bot' : role, content);
        const lastWrapper = chat.querySelector('.msg-wrapper:last-of-type');
        if (lastWrapper) lastWrapper.dataset.historyIdx = String(i);
        // Add feedback buttons to bot messages
        if (role === 'assistant') {
          if (lastWrapper) {
            const { thumbUp, thumbDown } = addFeedbackButtons(lastWrapper, id, i, content);
            if (ratingsMap[i] === 1) thumbUp.classList.add('active');
            if (ratingsMap[i] === -1) thumbDown.classList.add('active');
          }
        }
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
  return div.innerHTML.replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

// Custom confirm modal (replaces window.confirm which may not work in Tauri WebView)
function confirmModal(msg = 'Удалить?', confirmLabel = 'Да') {
  return new Promise(resolve => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal modal-compact" style="max-width:320px;text-align:center;">
      <div class="modal-title">${escapeHtml(msg)}</div>
      <div class="modal-actions">
        <button class="btn-secondary confirm-no">Отмена</button>
        <button class="btn-primary confirm-yes" style="background:var(--color-red)">${escapeHtml(confirmLabel)}</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    overlay.querySelector('.confirm-no').onclick = () => { overlay.remove(); resolve(false); };
    overlay.querySelector('.confirm-yes').onclick = () => { overlay.remove(); resolve(true); };
    overlay.addEventListener('click', e => { if (e.target === overlay) { overlay.remove(); resolve(false); } });
  });
}

function skeletonSettings(rows = 3) {
  let html = '<div class="skeleton-card">';
  html += '<div class="skeleton skeleton-header"></div>';
  for (let i = 0; i < rows; i++) {
    html += `<div class="skeleton-row"><div class="skeleton skeleton-line w-1-4"></div><div class="skeleton skeleton-line w-1-4"></div></div>`;
  }
  html += '</div>';
  return html;
}

function skeletonGrid(cols = 4) {
  let html = '<div style="display:grid;grid-template-columns:repeat(' + cols + ',1fr);gap:10px;margin-bottom:20px;">';
  for (let i = 0; i < cols; i++) {
    html += '<div class="skeleton-stat"><div class="skeleton skeleton-line w-1-2" style="margin:0 auto 6px;height:20px;"></div><div class="skeleton skeleton-line w-3-4" style="margin:0 auto;height:10px;"></div></div>';
  }
  html += '</div>';
  return html;
}

function skeletonList(items = 5) {
  let html = '';
  for (let i = 0; i < items; i++) {
    const w = i % 3 === 0 ? 'w-3-4' : i % 3 === 1 ? 'w-full' : 'w-1-2';
    html += `<div class="skeleton skeleton-line ${w}"></div>`;
  }
  return html;
}

function skeletonPage() {
  return skeletonGrid(4) + skeletonSettings(3) + skeletonSettings(2);
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
    item.dataset.tabId = tabId;
    item.title = reg.label;
    const customIcon = tabCustomizations[tabId]?.icon;
    item.innerHTML = customIcon
      ? `<span class="tab-item-icon tab-item-icon-emoji">${customIcon}</span>`
      : `<span class="tab-item-icon">${reg.icon || ''}</span>`;
    if (tabId === 'focus' && focusWidgetActivity) {
      const dot = document.createElement('span');
      dot.className = 'tab-focus-dot';
      item.appendChild(dot);
    }
    item.addEventListener('click', () => switchTab(tabId));
    tabList.appendChild(item);
  }

  // Bottom area: settings gear + version
  const bottom = document.getElementById('tab-bar-bottom');
  if (bottom) {
    bottom.innerHTML = '';
    const gear = document.createElement('div');
    gear.className = 'tab-item' + (activeTab === 'chat' && activeSubTab.chat === 'Настройки' ? ' active' : '');
    gear.title = 'Настройки';
    gear.innerHTML = `<span class="tab-item-icon">${TAB_ICONS.settings}</span>`;
    gear.addEventListener('click', () => {
      activeSubTab.chat = 'Настройки';
      switchTab('chat');
    });
    bottom.appendChild(gear);
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

  // Chat tab: conversations live in the sub-sidebar
  if (activeTab === 'chat') {
    sidebar.classList.remove('hidden');
    sidebar.classList.toggle('collapsed', !!chatSidebarCollapsed);
    const convPanel = document.getElementById('conversations-panel');
    if (convPanel) convPanel.style.display = 'none';

    items.innerHTML = '';

    // Collapse toggle
    const toggleRow = document.createElement('div');
    toggleRow.className = 'sub-sidebar-toggle-row';
    toggleRow.innerHTML = chatSidebarCollapsed
      ? `<button class="sub-sidebar-collapse-btn" title="Развернуть">${TAB_ICONS.chat}</button>`
      : `<button id="new-chat-sidebar-btn" class="sub-sidebar-new-chat">+ Новый чат</button>
         <button class="sub-sidebar-collapse-btn" title="Свернуть">
           <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="11 17 6 12 11 7"/><polyline points="18 17 13 12 18 7"/></svg>
         </button>`;
    toggleRow.querySelector('.sub-sidebar-collapse-btn').addEventListener('click', () => {
      chatSidebarCollapsed = !chatSidebarCollapsed;
      localStorage.setItem('hanni_chat_sidebar_collapsed', chatSidebarCollapsed ? '1' : '');
      renderSubSidebar();
      if (!chatSidebarCollapsed) loadConversationsList();
    });
    toggleRow.querySelector('#new-chat-sidebar-btn')?.addEventListener('click', () => {
      document.getElementById('new-chat-btn')?.click();
    });
    items.appendChild(toggleRow);

    if (!chatSidebarCollapsed) {
      // Search
      const searchBox = document.createElement('div');
      searchBox.className = 'sub-sidebar-search';
      searchBox.innerHTML = `<input class="form-input sub-sidebar-conv-search" placeholder="Поиск..." autocomplete="off">`;
      searchBox.querySelector('input').addEventListener('input', (e) => {
        clearTimeout(convSearchTimeout);
        convSearchTimeout = setTimeout(() => loadConversationsList(e.target.value), 300);
      });
      items.appendChild(searchBox);

      // Conv list container
      const convListEl = document.createElement('div');
      convListEl.id = 'sidebar-conv-list';
      convListEl.className = 'sub-sidebar-conv-list';
      items.appendChild(convListEl);
    }
  } else {
    sidebar.classList.add('hidden');
    // Restore conversations panel visibility when leaving chat
    const convPanel = document.getElementById('conversations-panel');
    if (convPanel) convPanel.style.display = '';

    // Render horizontal sub-tab pills in content area
    renderSubTabBar(activeTab, reg);
  }

  // Bottom: gear + version
  const settingsBottom = document.getElementById('sub-sidebar-settings');
  if (settingsBottom) {
    settingsBottom.innerHTML = '';
    const gear = document.createElement('div');
    gear.className = 'sub-sidebar-item' + (activeTab === 'chat' && activeSubTab.chat === 'Настройки' ? ' active' : '');
    gear.innerHTML = `<span class="tab-item-icon">${TAB_ICONS.settings}</span>${chatSidebarCollapsed && activeTab === 'chat' ? '' : ' Настройки'}`;
    gear.addEventListener('click', () => {
      if (activeTab === 'chat' && activeSubTab.chat === 'Настройки') {
        activeSubTab.chat = null;
        loadSubTabContent('chat', null);
      } else {
        activeSubTab.chat = 'Настройки';
        switchTab('chat');
      }
    });
    settingsBottom.appendChild(gear);
    if (!(chatSidebarCollapsed && activeTab === 'chat')) {
      const ver = document.createElement('div');
      ver.className = 'version-label';
      ver.textContent = `v${APP_VERSION}`;
      settingsBottom.appendChild(ver);
    }
  }
  loadGoalsWidget();
}

function renderSubTabBar(tabId, reg) {
  if (!reg?.subTabs?.length) return;
  const viewEl = document.getElementById(`view-${tabId}`);
  if (!viewEl) return;

  // Remove existing sub-tab bar if any
  viewEl.querySelector('.sub-tab-bar')?.remove();

  const currentSub = activeSubTab[tabId] ?? reg.subTabs[0];
  const bar = document.createElement('div');
  bar.className = 'sub-tab-bar';
  for (const sub of reg.subTabs) {
    const pill = document.createElement('button');
    pill.className = 'sub-tab-pill' + (sub === currentSub ? ' active' : '');
    pill.textContent = sub;
    pill.addEventListener('click', () => {
      activeSubTab[tabId] = sub;
      saveTabs();
      renderSubTabBar(tabId, reg);
      loadSubTabContent(tabId, sub);
    });
    bar.appendChild(pill);
  }
  viewEl.insertBefore(bar, viewEl.firstChild);
}

async function loadGoalsWidget() {
  // Sub-sidebar goals (keep hidden — moved to content area)
  const section = document.getElementById('sub-sidebar-goals');
  if (section) section.classList.add('hidden');

  // Inject goals into content area
  if (activeTab === 'chat') return;
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
        <button class="btn-smallall" id="add-goal-btn">+ Goal</button>
      </div>
      ${goals.length > 0 ? goals.map(g => {
        const pct = g.target_value > 0 ? Math.min(100, Math.round(g.current_value / g.target_value * 100)) : 0;
        return `<div class="goal-inline-item">
          <div class="goal-inline-info"><span>${escapeHtml(g.title)}</span><span class="goal-inline-pct">${pct}%</span></div>
          <div class="goal-progress"><div class="goal-progress-bar" style="width:${pct}%"></div></div>
        </div>`;
      }).join('') : '<div style="color:var(--text-faint);font-size:12px;">No goals yet</div>'}`;
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
  if (!openTabs.includes(tabId)) {
    const idx = openTabs.indexOf(activeTab);
    openTabs.splice(idx + 1, 0, tabId);
  }
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
  if (!openTabs.includes(tabId)) {
    const idx = openTabs.indexOf(activeTab);
    openTabs.splice(idx + 1, 0, tabId);
  }
  activeTab = tabId;
  saveTabs();
  renderTabBar();
  activateView();
  updateFocusWidgetVisibility();
}

function ensureViewDiv(tabId) {
  let view = document.getElementById(`view-${tabId}`);
  if (!view) {
    view = document.createElement('div');
    view.id = `view-${tabId}`;
    view.className = 'view';
    view.innerHTML = `<div id="${tabId}-content" class="tab-content"></div>`;
    document.getElementById('content-area').appendChild(view);
  }
  return view;
}

function activateView() {
  document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
  // Ensure view div exists for custom pages
  if (activeTab.startsWith('page_')) ensureViewDiv(activeTab);
  const view = document.getElementById(`view-${activeTab}`);
  if (view) view.classList.add('active');
  renderSubSidebar();
  const reg = TAB_REGISTRY[activeTab];
  const sub = reg?.subTabs ? (activeTab === 'chat' ? activeSubTab[activeTab] : (activeSubTab[activeTab] ?? reg.subTabs[0])) : null;
  loadSubTabContent(activeTab, sub);
}

function loadSubTabContent(tabId, subTab) {
  switch (tabId) {
    case 'chat':
      if (subTab === 'Настройки') { showChatSettingsMode(); loadChatSettings(); }
      else { hideChatSettingsMode(); renderSubSidebar(); loadConversationsList(); input.focus(); }
      break;
    case 'dashboard': loadDashboard(); break;
    case 'calendar': loadCalendar(subTab); break;
    case 'focus': loadFocus(subTab); break;
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
    default:
      if (tabId.startsWith('page_')) loadCustomPage(tabId);
      break;
  }
  // Wire up editable page header controls (icon picker, description) after content renders
  if (tabId !== 'chat') setTimeout(() => setupPageHeaderControls(tabId), 50);
}

// Tab add dropdown
document.getElementById('tab-add')?.addEventListener('click', (e) => {
  e.stopPropagation();
  const dropdown = document.getElementById('tab-dropdown');
  const list = document.getElementById('tab-dropdown-list');
  const btn = document.getElementById('tab-add');
  list.innerHTML = '';

  // "New Page" option — always first
  const newPageItem = document.createElement('div');
  newPageItem.className = 'tab-dropdown-item tab-dropdown-new-page';
  newPageItem.innerHTML = `<span class="tab-item-icon">➕</span> Новая страница`;
  newPageItem.addEventListener('click', async () => {
    dropdown.classList.add('hidden');
    try {
      const page = await invoke('create_custom_page');
      const tabId = `page_${page.id}`;
      TAB_REGISTRY[tabId] = {
        label: page.title,
        icon: page.icon,
        closable: true,
        subTabs: [],
        custom: true,
        pageId: page.id,
      };
      ensureViewDiv(tabId);
      openTab(tabId);
    } catch (err) { console.error('Page create error:', err); }
  });
  list.appendChild(newPageItem);

  // Separator
  const sep = document.createElement('div');
  sep.className = 'tab-dropdown-separator';
  list.appendChild(sep);

  // Existing closed tabs
  for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
    if (openTabs.includes(id)) continue;
    const item = document.createElement('div');
    item.className = 'tab-dropdown-item';
    item.innerHTML = `<span class="tab-item-icon">${reg.icon || ''}</span> ${reg.label}`;
    item.addEventListener('click', () => { dropdown.classList.add('hidden'); openTab(id); });
    list.appendChild(item);
  }
  // Position dropdown near + button (vertical tab bar)
  const rect = btn.getBoundingClientRect();
  dropdown.style.left = (rect.right + 4) + 'px';
  dropdown.style.top = Math.max(8, rect.top) + 'px';
  dropdown.classList.toggle('hidden');
});

document.addEventListener('click', () => {
  document.getElementById('tab-dropdown')?.classList.add('hidden');
});

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
  if (!e.metaKey && !e.ctrlKey) return;
  if (e.shiftKey && (e.key === 'f' || e.key === 'F')) { e.preventDefault(); toggleFocusWidgetPopover(); return; }
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
  el.innerHTML = skeletonPage();
  try {
    const [proactive, ttsVoices, ttsServerUrl, memories, voiceCloneEnabled, voiceCloneSample, voiceSamples, trainStats, trainFlywheel, trainHistory] = await Promise.all([
      invoke('get_proactive_settings').catch(() => ({ enabled: false, interval_minutes: 15, active_hours_start: 9, active_hours_end: 23, reply_window_sec: 120, styles: [] })),
      invoke('get_tts_voices').catch(() => []),
      invoke('get_app_setting', { key: 'tts_server_url' }).catch(() => null),
      invoke('get_all_memories', { search: null }).catch(() => []),
      invoke('get_app_setting', { key: 'voice_clone_enabled' }).catch(() => null),
      invoke('get_app_setting', { key: 'voice_clone_sample' }).catch(() => null),
      invoke('list_voice_samples').catch(() => []),
      invoke('get_training_stats').catch(() => ({ conversations: 0, total_messages: 0 })),
      invoke('get_flywheel_status').catch(() => ({ thumbs_up_total: 0, new_pairs: 0, total_cycles: 0, ready_to_train: false })),
      invoke('get_flywheel_history').catch(() => []),
    ]);
    const voicesByLang = {};
    for (const v of ttsVoices) {
      const lang = v.lang || 'other';
      if (!voicesByLang[lang]) voicesByLang[lang] = [];
      voicesByLang[lang].push(v);
    }
    const langOrder = ['ru-RU', 'kk-KZ', 'en-US'];
    const sortedLangs = [...langOrder.filter(l => voicesByLang[l]), ...Object.keys(voicesByLang).filter(l => !langOrder.includes(l)).sort()];

    const enabledStyles = proactive.enabled_styles || PROACTIVE_STYLE_DEFINITIONS.map(s => s.id);
    const quietStart = proactive.quiet_start_time || `${String(proactive.quiet_hours_start ?? 23).padStart(2,'0')}:00`;
    const quietEnd = proactive.quiet_end_time || `${String(proactive.quiet_hours_end ?? 8).padStart(2,'0')}:00`;

    el.innerHTML = `
      <div class="chat-settings-tabs">
        <button class="chat-settings-tab active" data-panel="memory">Память</button>
        <button class="chat-settings-tab" data-panel="general">Автономный</button>
        <button class="chat-settings-tab" data-panel="voice">Голос</button>
        <button class="chat-settings-tab" data-panel="styles">Стили</button>
        <button class="chat-settings-tab" data-panel="data">Данные</button>
        <button class="chat-settings-tab" data-panel="about">О Hanni</button>
      </div>

      <div class="chat-settings-panel active" id="cs-panel-memory">
        <div class="memory-header">
          <div class="memory-search-box" style="flex:1;">
            <input class="form-input" id="cs-mem-search" placeholder="Поиск по памяти..." autocomplete="off">
          </div>
          <button class="btn-primary" id="cs-mem-add-btn">+ Добавить</button>
        </div>
        <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;" id="cs-mem-count">${memories.length} фактов</div>
        <div class="memory-browser" id="cs-mem-list"></div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-general">
        <div class="settings-section">
          <div class="settings-section-title">Автономный режим <span class="proactive-status-badge" id="proactive-status-badge">Активен</span></div>
          <div class="settings-row">
            <span class="settings-label">Включён</span>
            <label class="toggle">
              <input type="checkbox" id="chat-proactive-enabled" ${proactive.enabled ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row">
            <span class="settings-label">Интервал</span>
            <div class="proactive-interval-row">
              <input type="range" id="chat-proactive-slider" class="proactive-slider" min="1" max="60" value="${proactive.interval_minutes}">
              <input type="number" id="chat-proactive-number" class="proactive-interval-number" min="1" max="120" value="${proactive.interval_minutes}">
              <span class="proactive-interval-unit">мин</span>
            </div>
          </div>
          <div class="settings-row">
            <span class="settings-label">Тихие часы</span>
            <div class="quiet-hours-row">
              <span class="quiet-hours-label">С</span>
              <input type="time" id="chat-quiet-start" class="form-input" style="width:90px;text-align:center" value="${quietStart}">
              <span class="quiet-hours-separator">—</span>
              <span class="quiet-hours-label">До</span>
              <input type="time" id="chat-quiet-end" class="form-input" style="width:90px;text-align:center" value="${quietEnd}">
            </div>
          </div>
          <div class="settings-row">
            <span class="settings-label">Лимит в день</span>
            <div class="proactive-interval-row">
              <input type="number" id="chat-daily-limit" class="proactive-interval-number" min="0" max="100" value="${proactive.daily_limit ?? 20}">
              <span class="proactive-interval-unit">сообщ. (0 = безлимит)</span>
            </div>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-voice">
        <div class="settings-section">
          <div class="settings-section-title">Озвучка</div>
          <div class="settings-row">
            <span class="settings-label">Включена</span>
            <label class="toggle">
              <input type="checkbox" id="chat-voice-enabled" ${proactive.voice_enabled ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row">
            <span class="settings-label">Голос</span>
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
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Клонирование голоса (PC)</div>
          <div class="settings-row">
            <span class="settings-label">Использовать клон</span>
            <label class="toggle">
              <input type="checkbox" id="chat-voice-clone-enabled" ${voiceCloneEnabled === 'true' ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row">
            <span class="settings-label">Образец голоса</span>
            <select class="form-select" id="chat-voice-clone-sample" style="width:220px">
              <option value="">— нет —</option>
              ${voiceSamples.map(s => `<option value="${s.name}" ${voiceCloneSample === s.name ? 'selected' : ''}>${s.name}</option>`).join('')}
            </select>
          </div>
          <div class="settings-row">
            <span class="settings-label"></span>
            <button class="settings-btn" id="chat-record-voice-sample">Записать образец</button>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-styles">
        <div class="settings-section">
          <div class="settings-section-title">Стили проактивных сообщений</div>
          <div class="proactive-styles-section">
            <div class="proactive-styles-desc">Какие стили Hanni может использовать в автономном режиме.</div>
            <div class="proactive-styles-actions">
              <button class="btn-smallall" id="proactive-select-all">Все</button>
              <button class="btn-smallall" id="proactive-select-none">Снять все</button>
            </div>
            <div class="proactive-styles-grid" id="proactive-styles-grid">
              ${PROACTIVE_STYLE_DEFINITIONS.map(s => {
                const isEnabled = enabledStyles.includes(s.id);
                return `<div class="proactive-style-card${isEnabled ? ' enabled' : ''}" data-style-id="${s.id}">
                  <span class="proactive-style-icon">${s.icon}</span>
                  <div class="proactive-style-info">
                    <div class="proactive-style-name">${s.name}</div>
                    <div class="proactive-style-desc">${s.desc}</div>
                  </div>
                  <label class="mini-toggle proactive-style-toggle">
                    <input type="checkbox" ${isEnabled ? 'checked' : ''} data-style="${s.id}">
                    <span class="mini-toggle-slider"></span>
                  </label>
                </div>`;
              }).join('')}
            </div>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-data">
        <div class="settings-section">
          <div class="settings-section-title">Данные для обучения</div>
          <div class="settings-row"><span class="settings-label">Диалогов (4+ сообщений)</span><span class="settings-value">${trainStats.conversations}</span></div>
          <div class="settings-row"><span class="settings-label">Всего сообщений</span><span class="settings-value">${trainStats.total_messages}</span></div>
          <div class="settings-row"><span class="settings-label">Thumbs-up пар</span><span class="settings-value" id="train-pairs-count">...</span></div>
          <div class="settings-row"><span class="settings-label">Период</span><span class="settings-value">${trainStats.earliest ? trainStats.earliest.substring(0,10) + ' — ' + trainStats.latest.substring(0,10) : '—'}</span></div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Экспорт</div>
          <div class="settings-row"><span class="settings-label">JSONL</span><button class="settings-btn" id="train-export-btn">Экспорт данных</button></div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Data Flywheel</div>
          <div class="settings-row"><span class="settings-label">Всего thumbs-up</span><span class="settings-value">${trainFlywheel.thumbs_up_total}</span></div>
          <div class="settings-row"><span class="settings-label">Новых (не экспортировано)</span><span class="settings-value" style="${trainFlywheel.new_pairs >= 20 ? 'color:var(--accent)' : ''}">${trainFlywheel.new_pairs}</span></div>
          <div class="settings-row"><span class="settings-label">Циклов обучения</span><span class="settings-value">${trainFlywheel.total_cycles}</span></div>
          <div class="settings-row"><span class="settings-label">Последний цикл</span><span class="settings-value">${trainFlywheel.last_cycle ? trainFlywheel.last_cycle.date.substring(0,10) + ' — ' + trainFlywheel.last_cycle.status : '—'}</span></div>
          <div class="settings-row">
            <span class="settings-label">Полный цикл</span>
            <button class="settings-btn ${trainFlywheel.ready_to_train ? 'btn-primary' : ''}" id="flywheel-run-btn" ${trainFlywheel.ready_to_train ? '' : 'title="Нужно минимум 20 новых пар"'}>
              ${trainFlywheel.ready_to_train ? 'Запустить цикл' : 'Мало данных (' + trainFlywheel.new_pairs + '/20)'}
            </button>
          </div>
          <div id="flywheel-progress" class="hidden" style="margin-top:8px;">
            <div class="train-progress-bar"><div class="train-progress-fill" id="flywheel-fill"></div></div>
            <div id="flywheel-status" style="font-size:12px;color:var(--text-secondary);margin-top:4px;"></div>
          </div>
          ${trainHistory.length > 0 ? `
          <div style="margin-top:12px;">
            <div style="font-size:12px;color:var(--text-secondary);margin-bottom:6px;">История циклов:</div>
            ${trainHistory.slice(0, 5).map(c => `
              <div style="font-size:12px;color:var(--text-secondary);padding:3px 0;border-bottom:1px solid var(--border-color);">
                #${c.id} ${(c.started_at || '').substring(0,16)} — ${c.status} (${c.train_pairs} пар)${c.eval_score != null ? ' score:' + c.eval_score.toFixed(2) : ''}
              </div>
            `).join('')}
          </div>` : ''}
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-about">
        <div class="about-wrapper">
          <div class="about-card">
            <div class="about-header">
              <div class="about-logo">🤖</div>
              <div class="about-name">Hanni</div>
              <span class="about-version">v${APP_VERSION}</span>
            </div>
            <hr class="about-divider">
            <div class="about-info-list">
              <div class="about-info-row"><span class="about-info-label">Модель</span><span class="about-info-value" id="cs-about-model">Загрузка...</span></div>
              <div class="about-info-row"><span class="about-info-label">MLX сервер</span><span class="about-info-value" id="cs-about-mlx">Загрузка...</span></div>
              <div class="about-info-row"><span class="about-info-label">HTTP API</span><span class="about-info-value" id="cs-about-api">Проверяю...</span></div>
            </div>
            <hr class="about-divider">
            <div class="about-actions">
              <button class="settings-btn" id="cs-about-check-update">Проверить обновления</button>
            </div>
          </div>
        </div>
      </div>`;

    // ── Memory panel ──
    _csMemRenderList(memories);
    _chatSettingsSetupMemory(memories);

    // ── Tab switching ──
    document.querySelectorAll('.chat-settings-tab').forEach(tab => {
      tab.addEventListener('click', () => {
        document.querySelectorAll('.chat-settings-tab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.chat-settings-panel').forEach(p => p.classList.remove('active'));
        tab.classList.add('active');
        document.getElementById(`cs-panel-${tab.dataset.panel}`)?.classList.add('active');
      });
    });

    // ── Save handlers ──
    const getEnabledStyles = () => {
      const checks = document.querySelectorAll('#proactive-styles-grid input[data-style]');
      return Array.from(checks).filter(c => c.checked).map(c => c.dataset.style);
    };
    const getChatProactiveValues = () => {
      const qStart = document.getElementById('chat-quiet-start')?.value || '23:00';
      const qEnd = document.getElementById('chat-quiet-end')?.value || '08:00';
      const [sh, sm] = qStart.split(':').map(Number);
      const [eh, em] = qEnd.split(':').map(Number);
      return {
        enabled: document.getElementById('chat-proactive-enabled').checked,
        voice_enabled: document.getElementById('chat-voice-enabled').checked,
        voice_name: document.getElementById('chat-voice-name')?.value || 'xenia',
        interval_minutes: parseInt(document.getElementById('chat-proactive-number')?.value || '10'),
        quiet_hours_start: sh,
        quiet_hours_end: eh,
        quiet_start_time: qStart,
        quiet_end_time: qEnd,
        enabled_styles: getEnabledStyles(),
        daily_limit: parseInt(document.getElementById('chat-daily-limit')?.value || '20'),
      };
    };
    const saveChatSettings = () => invoke('set_proactive_settings', { settings: getChatProactiveValues() }).catch(() => {});

    document.getElementById('chat-proactive-enabled')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-voice-enabled')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-voice-name')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-quiet-start')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-quiet-end')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-daily-limit')?.addEventListener('change', saveChatSettings);

    // Interval slider <-> number sync
    const slider = document.getElementById('chat-proactive-slider');
    const numInput = document.getElementById('chat-proactive-number');
    slider?.addEventListener('input', () => {
      numInput.value = slider.value;
    });
    slider?.addEventListener('change', saveChatSettings);
    numInput?.addEventListener('input', () => {
      const v = Math.max(1, Math.min(120, parseInt(numInput.value) || 1));
      if (v <= 60) slider.value = v;
      saveChatSettings();
    });

    // Style toggles
    document.querySelectorAll('#proactive-styles-grid .proactive-style-card').forEach(card => {
      const checkbox = card.querySelector('input[data-style]');
      card.addEventListener('click', (e) => {
        if (e.target.closest('.mini-toggle')) return;
        checkbox.checked = !checkbox.checked;
        card.classList.toggle('enabled', checkbox.checked);
        saveChatSettings();
      });
      checkbox?.addEventListener('change', () => {
        card.classList.toggle('enabled', checkbox.checked);
        saveChatSettings();
      });
    });

    // Select all / none
    document.getElementById('proactive-select-all')?.addEventListener('click', () => {
      document.querySelectorAll('#proactive-styles-grid input[data-style]').forEach(c => { c.checked = true; });
      document.querySelectorAll('#proactive-styles-grid .proactive-style-card').forEach(c => c.classList.add('enabled'));
      saveChatSettings();
    });
    document.getElementById('proactive-select-none')?.addEventListener('click', () => {
      document.querySelectorAll('#proactive-styles-grid input[data-style]').forEach(c => { c.checked = false; });
      document.querySelectorAll('#proactive-styles-grid .proactive-style-card').forEach(c => c.classList.remove('enabled'));
      saveChatSettings();
    });

    document.getElementById('chat-test-voice')?.addEventListener('click', async () => {
      const voice = document.getElementById('chat-voice-name')?.value || 'xenia';
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

    // ── Voice Clone handlers ──
    document.getElementById('chat-voice-clone-enabled')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'voice_clone_enabled', value: String(e.target.checked) });
    });
    document.getElementById('chat-voice-clone-sample')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'voice_clone_sample', value: e.target.value });
    });
    document.getElementById('chat-record-voice-sample')?.addEventListener('click', async () => {
      const btn = document.getElementById('chat-record-voice-sample');
      if (!btn) return;
      const name = prompt('Имя для образца голоса:', 'my_voice');
      if (!name) return;
      btn.textContent = 'Записываю (5 сек)...';
      btn.disabled = true;
      try {
        // Record directly via cpal for 5 seconds → save as WAV
        const path = await invoke('record_voice_sample', { name, durationSecs: 5 });
        btn.textContent = 'Сохранено!';
        // Refresh sample list
        const samples = await invoke('list_voice_samples').catch(() => []);
        const sel = document.getElementById('chat-voice-clone-sample');
        if (sel) {
          sel.innerHTML = '<option value="">— нет —</option>' +
            samples.map(s => `<option value="${s.name}">${s.name}</option>`).join('');
          sel.value = name;
        }
        await invoke('set_app_setting', { key: 'voice_clone_sample', value: name });
      } catch (err) {
        btn.textContent = 'Ошибка: ' + String(err).substring(0, 30);
      }
      setTimeout(() => { btn.textContent = 'Записать образец'; btn.disabled = false; }, 3000);
    });

    // ── Training panel handlers ──
    try {
      const pairsPath = '~/Library/Application Support/Hanni/training_pairs.jsonl';
      const content = await invoke('read_file', { path: pairsPath }).catch(() => '');
      const count = content ? content.trim().split('\\n').filter(l => l.trim()).length : 0;
      const pairsEl = document.getElementById('train-pairs-count');
      if (pairsEl) pairsEl.textContent = String(count);
    } catch (_) {
      const pairsEl = document.getElementById('train-pairs-count');
      if (pairsEl) pairsEl.textContent = '0';
    }
    document.getElementById('train-export-btn')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Экспорт...'; btn.disabled = true;
      try { const r = await invoke('export_training_data'); btn.textContent = r.train_count + ' train + ' + r.valid_count + ' valid'; }
      catch (err) { btn.textContent = String(err).substring(0, 30); }
      setTimeout(() => { btn.textContent = 'Экспорт данных'; btn.disabled = false; }, 4000);
    });
    document.getElementById('flywheel-run-btn')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.disabled = true;
      const progress = document.getElementById('flywheel-progress');
      const fill = document.getElementById('flywheel-fill');
      const status = document.getElementById('flywheel-status');
      progress?.classList.remove('hidden');
      btn.textContent = 'Экспорт...';
      if (status) status.textContent = 'Шаг 1/3: Экспорт данных...';
      if (fill) fill.style.width = '15%';
      try {
        await invoke('export_training_data');
        btn.textContent = 'Обучение...';
        if (status) status.textContent = 'Шаг 2/3: Fine-tuning (может занять 10-30 минут)...';
        if (fill) fill.style.width = '40%';
        const r = await invoke('run_flywheel_cycle');
        if (fill) fill.style.width = '100%';
        if (status) status.textContent = `Цикл #${r.cycle_id} завершён: ${r.status} (${r.train_pairs} пар)`;
        btn.textContent = 'Готово!';
      } catch (err) {
        if (fill) fill.style.width = '100%';
        if (status) status.textContent = 'Ошибка: ' + String(err).substring(0, 80);
        btn.textContent = 'Ошибка';
      }
      setTimeout(() => { btn.textContent = 'Запустить цикл'; btn.disabled = false; }, 5000);
    });

    // ── About panel ──
    invoke('get_model_info').catch(() => ({})).then(info => {
      const modelEl = document.getElementById('cs-about-model');
      if (modelEl) modelEl.textContent = info.model_name || '?';
      const mlxEl = document.getElementById('cs-about-mlx');
      if (mlxEl) { mlxEl.textContent = info.server_online ? 'Онлайн' : 'Офлайн'; mlxEl.className = 'about-info-value ' + (info.server_online ? 'online' : 'offline'); }
    });
    fetch('http://127.0.0.1:8235/api/status').then(resp => {
      const apiEl = document.getElementById('cs-about-api');
      if (apiEl) { apiEl.textContent = resp.ok ? 'Активен' : 'Недоступен'; apiEl.className = 'about-info-value ' + (resp.ok ? 'online' : 'offline'); }
    }).catch(() => {
      const apiEl = document.getElementById('cs-about-api');
      if (apiEl) { apiEl.textContent = 'Недоступен'; apiEl.className = 'about-info-value offline'; }
    });
    document.getElementById('cs-about-check-update')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Проверяю...'; btn.disabled = true;
      try { const r = await invoke('check_update'); btn.textContent = r; }
      catch (err) { btn.textContent = 'Ошибка'; }
      setTimeout(() => { btn.textContent = 'Проверить обновления'; btn.disabled = false; }, 4000);
    });

  } catch (e) {
    el.innerHTML = `<div style="color:var(--color-red);font-size:14px;">Ошибка: ${e}</div>`;
  }
}

// Helper: render memory items (just the list — no listener re-registration)
function _csMemRenderList(memories) {
  const list = document.getElementById('cs-mem-list');
  if (!list) return;
  const countEl = document.getElementById('cs-mem-count');
  if (countEl) countEl.textContent = `${memories.length} фактов`;
  list.innerHTML = memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
    <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
    <span class="memory-item-key">${escapeHtml(m.key)}</span>
    <span class="memory-item-value">${escapeHtml(m.value)}</span>
    <div class="memory-item-actions">
      <button class="memory-item-btn memory-edit-btn" data-csedit="${m.id}" title="Редактировать">&#9998;</button>
      <button class="memory-item-btn" data-csdel="${m.id}" title="Удалить">&times;</button>
    </div>
  </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';
}

// Setup memory panel: event delegation + search + add (called ONCE)
function _chatSettingsSetupMemory(memories) {
  // Reload helper
  const reloadMem = async () => {
    const q = document.getElementById('cs-mem-search')?.value || null;
    const updated = await invoke('get_all_memories', { search: q }).catch(() => []);
    _csMemRenderList(updated);
  };

  // Event delegation on the list container (handles delete + edit for all items)
  const list = document.getElementById('cs-mem-list');
  if (list) {
    list.addEventListener('click', async (e) => {
      const delBtn = e.target.closest('[data-csdel]');
      if (delBtn) {
        if (await confirmModal()) {
          await invoke('delete_memory', { id: parseInt(delBtn.dataset.csdel) }).catch(e => console.error('delete_memory error:', e));
          reloadMem();
        }
        return;
      }
      const editBtn = e.target.closest('[data-csedit]');
      if (editBtn) {
        const id = parseInt(editBtn.dataset.csedit);
        // Fetch current value from DB to avoid stale data
        const allMem = await invoke('get_all_memories', { search: null }).catch(() => []);
        const m = allMem.find(x => x.id === id);
        if (!m) return;
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.innerHTML = `<div class="modal modal-compact">
          <div class="modal-title">Редактировать факт</div>
          <div class="form-group"><label class="form-label">Категория</label>
            <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
          </div>
          <div class="form-group"><label class="form-label">Ключ</label>
            <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
          </div>
          <div class="form-group"><label class="form-label">Значение</label>
            <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
          </div>
          <div class="modal-actions">
            <button class="btn-secondary mem-cancel">Отмена</button>
            <button class="btn-primary mem-save">Сохранить</button>
          </div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
        overlay.addEventListener('click', ev => { if (ev.target === overlay) overlay.remove(); });
        overlay.querySelector('.mem-save').onclick = async () => {
          const cat = overlay.querySelector('.memory-edit-cat').value;
          const key = overlay.querySelector('.memory-edit-key').value.trim();
          const val = overlay.querySelector('.memory-edit-val').value.trim();
          if (!key || !val) return;
          try { await invoke('update_memory', { id, category: cat, key, value: val }); } catch (err) { console.error(err); }
          overlay.remove();
          reloadMem();
        };
      }
    });
  }

  // Add button (once)
  document.getElementById('cs-mem-add-btn')?.addEventListener('click', () => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal modal-compact">
      <div class="modal-title">Новый факт</div>
      <div class="form-group"><label class="form-label">Категория</label>
        <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
      </div>
      <div class="form-group"><label class="form-label">Ключ</label>
        <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
      </div>
      <div class="form-group"><label class="form-label">Значение</label>
        <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
      </div>
      <div class="modal-actions">
        <button class="btn-secondary mem-cancel">Отмена</button>
        <button class="btn-primary mem-save">Добавить</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
    overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
    overlay.querySelector('.mem-save').onclick = async () => {
      const cat = overlay.querySelector('.memory-add-cat').value;
      const key = overlay.querySelector('.memory-add-key').value.trim();
      const val = overlay.querySelector('.memory-add-val').value.trim();
      if (!key || key.length < 2 || !val || val.length < 2) { overlay.querySelector('.memory-add-key').style.borderColor = !key || key.length < 2 ? 'var(--accent)' : ''; return; }
      try { await invoke('memory_remember', { category: cat, key, value: val }); } catch (err) { console.error(err); }
      overlay.remove();
      reloadMem();
    };
  });

  // Search (once, with debounce)
  let searchTimeout;
  document.getElementById('cs-mem-search')?.addEventListener('input', (e) => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(async () => {
      const q = e.target.value;
      const results = await invoke('get_all_memories', { search: q || null }).catch(() => []);
      _csMemRenderList(results);
    }, 300);
  });
}

// ── Chat helpers ──

let _scrollRAF = null;
function scrollDown() {
  if (_scrollRAF) return;
  _scrollRAF = requestAnimationFrame(() => {
    chat.scrollTop = chat.scrollHeight;
    _scrollRAF = null;
  });
}

// Scroll-to-bottom floating button
const scrollBottomBtn = document.getElementById('scroll-bottom-btn');
chat.addEventListener('scroll', () => {
  const distFromBottom = chat.scrollHeight - chat.scrollTop - chat.clientHeight;
  scrollBottomBtn?.classList.toggle('visible', distFromBottom > 200);
});
scrollBottomBtn?.addEventListener('click', () => {
  chat.scrollTo({ top: chat.scrollHeight, behavior: 'smooth' });
});

function addMsg(role, text, isVoice = false) {
  if (role === 'bot') {
    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const div = document.createElement('div');
    div.className = 'msg bot markdown-body';
    div.innerHTML = renderMarkdown(text);
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
  const wrapper = document.createElement('div');
  wrapper.className = 'msg-wrapper user-wrapper';
  const div = document.createElement('div');
  div.className = `msg ${role}`;
  if (isVoice && role === 'user') {
    const mic = document.createElement('span');
    mic.className = 'voice-indicator';
    mic.textContent = '\u{1F3A4} ';
    div.appendChild(mic);
    div.appendChild(document.createTextNode(text));
  } else {
    div.textContent = text;
  }
  wrapper.appendChild(div);

  // CH4: Edit button for user messages
  if (role === 'user') {
    const editBtn = document.createElement('button');
    editBtn.className = 'feedback-btn edit-msg-btn';
    editBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 000-1.41l-2.34-2.34a1 1 0 00-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/></svg>';
    editBtn.title = 'Редактировать';
    editBtn.addEventListener('click', () => {
      const input = document.getElementById('input');
      input.value = text;
      input.focus();
      // Find this wrapper's history index and remove everything after it
      const allWrappers = [...chat.querySelectorAll('.msg-wrapper, .msg.user, .action-result, .memory-toast')];
      const idx = allWrappers.indexOf(wrapper);
      if (idx >= 0) {
        for (let i = allWrappers.length - 1; i > idx; i--) allWrappers[i].remove();
      }
      wrapper.remove();
      // Find and truncate history to this user message
      const wrapperIdx = parseInt(wrapper.dataset.historyIdx || '-1', 10);
      if (wrapperIdx >= 0) {
        history.length = wrapperIdx;
      } else {
        // Fallback: remove from the last user message matching this text
        for (let i = history.length - 1; i >= 0; i--) {
          if (history[i].role === 'user' && history[i].content === text) {
            history.length = i;
            break;
          }
        }
      }
    });
    wrapper.appendChild(editBtn);
  }

  chat.appendChild(wrapper);
  scrollDown();
  return div;
}

function addFeedbackButtons(wrapper, conversationId, messageIndex, botText) {
  const actions = document.createElement('div');
  actions.className = 'msg-actions';

  // Copy button
  const copyBtn = document.createElement('button');
  copyBtn.className = 'feedback-btn copy-btn';
  copyBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/></svg>';
  copyBtn.title = 'Копировать';
  copyBtn.addEventListener('click', () => {
    navigator.clipboard.writeText(botText || '');
    copyBtn.classList.add('copied');
    copyBtn.title = 'Скопировано!';
    setTimeout(() => { copyBtn.classList.remove('copied'); copyBtn.title = 'Копировать'; }, 1500);
  });

  // Regenerate button
  const regenBtn = document.createElement('button');
  regenBtn.className = 'feedback-btn regen-btn';
  regenBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M17.65 6.35A7.96 7.96 0 0012 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>';
  regenBtn.title = 'Перегенерировать';
  regenBtn.addEventListener('click', async () => {
    if (busy) return;
    // Remove last bot message from history and re-send
    while (history.length > 0 && history[history.length - 1].role === 'assistant') {
      history.pop();
    }
    // Remove wrapper from DOM
    wrapper.remove();
    // Re-send
    send();
  });

  const thumbUp = document.createElement('button');
  thumbUp.className = 'feedback-btn thumb-up';
  thumbUp.innerHTML = '<svg viewBox="0 0 24 24"><path d="M2 20h2V10H2v10zm20-9a2 2 0 0 0-2-2h-6.31l.95-4.57.03-.32a1.5 1.5 0 0 0-.44-1.06L13.17 2 7.59 7.59A2 2 0 0 0 7 9v10a2 2 0 0 0 2 2h9c.83 0 1.54-.5 1.84-1.22l3.02-7.05c.09-.23.14-.47.14-.73v-2z"/></svg>';
  thumbUp.title = 'Хороший ответ';

  const thumbDown = document.createElement('button');
  thumbDown.className = 'feedback-btn thumb-down';
  thumbDown.innerHTML = '<svg viewBox="0 0 24 24"><path d="M22 4h-2v10h2V4zM2 13a2 2 0 0 0 2 2h6.31l-.95 4.57-.03.32c0 .4.17.77.44 1.06L10.83 22l5.58-5.59A2 2 0 0 0 17 15V5a2 2 0 0 0-2-2H6c-.83 0-1.54.5-1.84 1.22L1.14 11.27c-.09.23-.14.47-.14.73v2z"/></svg>';
  thumbDown.title = 'Плохой ответ';

  const handleClick = async (btn, rating) => {
    const isActive = btn.classList.contains('active');
    thumbUp.classList.remove('active');
    thumbDown.classList.remove('active');
    if (!isActive) {
      btn.classList.add('active');
      try {
        await invoke('rate_message', { conversationId, messageIndex, rating });
      } catch (e) {
        console.error('Rate error:', e);
      }
    } else {
      try {
        await invoke('rate_message', { conversationId, messageIndex, rating: 0 });
      } catch (e) {
        console.error('Rate error:', e);
      }
    }
  };

  thumbUp.addEventListener('click', () => handleClick(thumbUp, 1));
  thumbDown.addEventListener('click', () => handleClick(thumbDown, -1));

  actions.appendChild(copyBtn);
  actions.appendChild(regenBtn);
  actions.appendChild(thumbUp);
  actions.appendChild(thumbDown);
  wrapper.appendChild(actions);
  return { thumbUp, thumbDown };
}

function addProactiveFeedbackButtons(wrapper, proactiveId, botText) {
  const actions = document.createElement('div');
  actions.className = 'msg-actions';

  // Copy button
  const copyBtn = document.createElement('button');
  copyBtn.className = 'feedback-btn copy-btn';
  copyBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/></svg>';
  copyBtn.title = 'Копировать';
  copyBtn.addEventListener('click', () => {
    navigator.clipboard.writeText(botText || '');
    copyBtn.classList.add('copied');
    copyBtn.title = 'Скопировано!';
    setTimeout(() => { copyBtn.classList.remove('copied'); copyBtn.title = 'Копировать'; }, 1500);
  });

  const thumbUp = document.createElement('button');
  thumbUp.className = 'feedback-btn thumb-up';
  thumbUp.innerHTML = '<svg viewBox="0 0 24 24"><path d="M2 20h2V10H2v10zm20-9a2 2 0 0 0-2-2h-6.31l.95-4.57.03-.32a1.5 1.5 0 0 0-.44-1.06L13.17 2 7.59 7.59A2 2 0 0 0 7 9v10a2 2 0 0 0 2 2h9c.83 0 1.54-.5 1.84-1.22l3.02-7.05c.09-.23.14-.47.14-.73v-2z"/></svg>';
  thumbUp.title = 'Хороший ответ';

  const thumbDown = document.createElement('button');
  thumbDown.className = 'feedback-btn thumb-down';
  thumbDown.innerHTML = '<svg viewBox="0 0 24 24"><path d="M22 4h-2v10h2V4zM2 13a2 2 0 0 0 2 2h6.31l-.95 4.57-.03.32c0 .4.17.77.44 1.06L10.83 22l5.58-5.59A2 2 0 0 0 17 15V5a2 2 0 0 0-2-2H6c-.83 0-1.54.5-1.84 1.22L1.14 11.27c-.09.23-.14.47-.14.73v2z"/></svg>';
  thumbDown.title = 'Плохой ответ';

  const handleClick = async (btn, rating) => {
    const isActive = btn.classList.contains('active');
    thumbUp.classList.remove('active');
    thumbDown.classList.remove('active');
    if (!isActive) {
      btn.classList.add('active');
      try {
        await invoke('rate_proactive', { proactiveId, rating });
      } catch (e) {
        console.error('Rate proactive error:', e);
      }
    } else {
      try {
        await invoke('rate_proactive', { proactiveId, rating: 0 });
      } catch (e) {
        console.error('Rate proactive error:', e);
      }
    }
  };

  thumbUp.addEventListener('click', () => handleClick(thumbUp, 1));
  thumbDown.addEventListener('click', () => handleClick(thumbDown, -1));

  actions.appendChild(copyBtn);
  actions.appendChild(thumbUp);
  actions.appendChild(thumbDown);
  wrapper.appendChild(actions);
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

    // S5: Confirmation for dangerous actions
    const DANGEROUS_ACTIONS = ['run_shell', 'close_app', 'quit_app', 'open_app', 'start_focus'];
    if (DANGEROUS_ACTIONS.includes(actionType)) {
      const desc = actionType === 'run_shell' ? `Команда: ${action.command || action.cmd || '?'}`
        : actionType === 'start_focus' ? `Фокус: ${action.duration || '?'} мин`
        : actionType === 'open_app' ? `Открыть: ${action.name || action.app || '?'}`
        : `Закрыть: ${action.name || action.app || '?'}`;
      const confirmed = await new Promise(resolve => {
        const overlay = document.createElement('div');
        overlay.className = 'confirm-overlay';
        const modal = document.createElement('div');
        modal.className = 'confirm-modal';
        const title = document.createElement('div');
        title.className = 'confirm-title';
        title.textContent = 'Подтверждение действия';
        const descEl = document.createElement('div');
        descEl.className = 'confirm-desc';
        descEl.textContent = desc;
        const btns = document.createElement('div');
        btns.className = 'confirm-buttons';
        btns.innerHTML = '<button class="confirm-cancel">Отмена</button><button class="confirm-ok">Выполнить</button>';
        modal.append(title, descEl, btns);
        overlay.appendChild(modal);
        document.body.appendChild(overlay);
        overlay.querySelector('.confirm-cancel').onclick = () => { overlay.remove(); resolve(false); };
        overlay.querySelector('.confirm-ok').onclick = () => { overlay.remove(); resolve(true); };
      });
      if (!confirmed) return { success: false, result: 'Действие отменено пользователем' };
    }

    switch (actionType) {
      case 'add_purchase':
        result = await invoke('add_transaction', {
          transactionType: 'expense',
          amount: action.amount,
          category: action.category || 'other',
          description: action.description || '',
          currency: 'KZT'
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
      case 'create_note':
        result = await invoke('create_note', {
          title: action.title || '',
          content: action.content || '',
          tags: action.tags || '',
          tabName: action.tab || null,
          status: action.status || 'note',
          dueDate: action.due_date || null,
          reminderAt: action.remind_at || null,
        });
        if (callModeActive && action.save_audio) {
          try {
            const wavPath = await invoke('save_voice_note', { title: action.title || 'note' });
            result = (result || '') + ' (audio: ' + wavPath + ')';
          } catch (_) {}
        }
        break;
      case 'search_notes': {
        const filter = action.tab ? `tab:${action.tab}` : null;
        const notes = await invoke('get_notes', { filter, search: action.query || null });
        result = JSON.stringify((notes || []).slice(0, 5).map(n => ({ id: n.id, title: n.title, tags: n.tags, status: n.status })));
        break;
      }
      case 'complete_task':
        await invoke('update_note_status', { id: action.id, status: 'done' });
        result = 'Задача отмечена как выполненная';
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
        // ME2: Show memory notification
        { const tag = document.createElement('div');
          tag.className = 'memory-toast';
          tag.textContent = `Запомнила: ${action.key || action.value || ''}`;
          chat.appendChild(tag);
          setTimeout(() => tag.classList.add('fade-out'), 3000);
          setTimeout(() => tag.remove(), 3500); }
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
      case 'web_search':
      case 'search_web':
        result = await invoke('web_search', { query: action.query || '' });
        break;
      case 'read_url':
      case 'fetch_url':
        result = await invoke('read_url', { url: action.url || '' });
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
          date: action.date || null, transactionType: action.transaction_type || action.tx_type || 'expense',
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
          category: action.category || 'general', color: action.color || '#9B9B9B',
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
          title: action.name || action.title || action.activity || '',
          category: action.category || 'other',
          focusMode: action.focus_mode || false,
          duration: action.duration || null,
          apps: null, sites: null,
        });
        setTimeout(() => updateFocusWidget(), 100);
        break;
      case 'stop_activity':
        result = await invoke('stop_activity');
        setTimeout(() => updateFocusWidget(), 100);
        break;
      case 'get_current_activity':
        result = await invoke('get_current_activity');
        break;
      case 'start_pomodoro': {
        const title = action.title || action.name || 'Помодоро';
        const cat = action.category || 'work';
        startPomodoro(title, cat, action.focus_mode || false);
        result = `Помодоро запущен: ${title} (25 мин)`;
        setTimeout(() => updateFocusWidget(), 100);
        break;
      }
      case 'stop_pomodoro':
        pomodoroState.active = false;
        await invoke('stop_activity').catch(() => {});
        result = 'Помодоро остановлен';
        setTimeout(() => updateFocusWidget(), 100);
        break;
      // Navigation
      case 'open_tab':
        activateTab(action.tab || 'chat');
        result = `Открыта вкладка: ${action.tab}`;
        break;
      case 'open_note': {
        const noteId = action.id || action.note_id;
        if (noteId) {
          activateTab('notes');
          setTimeout(() => {
            currentNoteId = noteId;
            notesViewMode = 'edit';
            const notesEl = document.getElementById('notes-content');
            if (notesEl) renderNoteEditor(notesEl, noteId);
          }, 100);
          result = `Заметка #${noteId} открыта`;
        } else { result = 'Не указан id заметки'; }
        break;
      }
      case 'get_tasks': {
        const taskFilter = action.status || 'tasks';
        const tasks = await invoke('get_notes', { filter: taskFilter, search: action.query || null });
        result = JSON.stringify((tasks || []).slice(0, 10).map(n => ({
          id: n.id, title: n.title, status: n.status, due_date: n.due_date, tags: n.tags
        })));
        break;
      }
      case 'create_task': {
        const taskId = await invoke('create_note', {
          title: action.title || '', content: action.content || '',
          tags: action.tags || '', status: 'task',
          tabName: action.tab || null,
          dueDate: action.due_date || null,
          reminderAt: action.remind_at || null,
        });
        result = `Задача создана (id: ${taskId})`;
        { const tag = document.createElement('div');
          tag.className = 'memory-toast';
          tag.textContent = `✅ Задача: ${action.title || 'Новая задача'}${action.due_date ? ' → ' + action.due_date : ''}`;
          tag.style.cursor = 'pointer';
          tag.addEventListener('click', () => { activateTab('notes'); setTimeout(() => { notesFilters = new Set(['task']); loadNotes(); }, 100); });
          document.getElementById('chat')?.appendChild(tag);
          setTimeout(() => tag.classList.add('fade-out'), 4000);
          setTimeout(() => tag.remove(), 4500); }
        break;
      }
      // Projects & Tasks (legacy — work tab)
      case 'create_project_task':
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
      case 'log_health': {
        // Rust log_health takes one type at a time — call for each provided field
        const fields = {sleep: action.sleep, water: action.water, steps: action.steps, weight: action.weight};
        const logged = [];
        for (const [type, val] of Object.entries(fields)) {
          if (val != null && val !== undefined) {
            await invoke('log_health', { healthType: type, value: Number(val), notes: action.notes || null });
            logged.push(`${type}=${val}`);
          }
        }
        result = logged.length ? `Записано: ${logged.join(', ')}` : 'Нет данных для записи';
        break;
      }
      case 'add_workout':
      case 'create_workout':
      case 'log_workout':
        result = await invoke('create_workout', {
          workoutType: action.type || action.workout_type || 'general',
          title: action.title || action.name || 'Тренировка',
          durationMinutes: action.duration || action.duration_minutes || 60,
          calories: action.calories || null,
          notes: action.notes || '',
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
      // Reminders
      case 'set_reminder':
      case 'remind':
      case 'set_timer':
        result = await invoke('set_reminder', {
          title: action.title || action.text || '',
          remindAt: action.remind_at || action.time || '',
          repeat: action.repeat || null,
        });
        break;
      // App & Music control
      case 'open_app':
      case 'launch_app':
        result = await invoke('open_app', { name: action.name || action.app || '' });
        break;
      case 'close_app':
      case 'quit_app':
        result = await invoke('close_app', { name: action.name || action.app || '' });
        break;
      case 'music':
      case 'music_control':
        result = await invoke('music_control', { action: action.command || action.action_type || 'toggle' });
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
      summary.textContent = '🤔 Думает...';
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
      reasoningDetails.querySelector('summary').textContent = '🤔 Рассуждения';
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
      reasoningDetails.querySelector('summary').textContent = '🤔 Рассуждения';
    }
  });

  try {
    const msgs = history.slice(-20).map(normalizeHistoryMessage);
    const resultJson = await invoke('chat', { messages: msgs, callMode });
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
    await stopAllTTS();
    return;
  }
  isSpeaking = true;
  btn.classList.add('speaking');
  btn.innerHTML = '&#9632;';
  document.getElementById('stop-tts')?.classList.remove('hidden');
  try {
    let voice = 'xenia';
    try {
      const ps = await invoke('get_proactive_settings');
      voice = ps.voice_name || 'xenia';
    } catch (_) {}
    // V8: Check if voice clone is enabled
    const cloneEnabled = await invoke('get_app_setting', { key: 'voice_clone_enabled' }).catch(() => null);
    const cloneSample = await invoke('get_app_setting', { key: 'voice_clone_sample' }).catch(() => null);
    if (cloneEnabled === 'true' && cloneSample) {
      await invoke('speak_clone_blocking', { text, sampleName: cloneSample });
    } else {
      await invoke('speak_text_blocking', { text, voice });
    }
    btn.classList.remove('speaking');
    btn.innerHTML = '&#9654;';
    isSpeaking = false;
    document.getElementById('stop-tts')?.classList.add('hidden');
  } catch (_) {
    btn.classList.remove('speaking');
    btn.innerHTML = '&#9654;';
    isSpeaking = false;
    document.getElementById('stop-tts')?.classList.add('hidden');
  }
}

async function stopAllTTS() {
  await invoke('stop_speaking').catch(() => {});
  document.querySelectorAll('.tts-btn.speaking').forEach(b => {
    b.classList.remove('speaking');
    b.innerHTML = '&#9654;';
  });
  isSpeaking = false;
  document.getElementById('stop-tts')?.classList.add('hidden');
}

// Stop TTS button
document.getElementById('stop-tts')?.addEventListener('click', stopAllTTS);

// ── Send message ──

async function send() {
  const text = input.value.trim();
  if (!text || busy) return;

  busy = true;
  sendBtn.disabled = true;
  input.value = '';
  input.style.height = 'auto';

  // Report user chat activity for adaptive timing
  invoke('report_user_chat_activity').catch(() => {});
  // If user replies within 10 min of a proactive message, report engagement
  if (lastProactiveTime && (Date.now() - lastProactiveTime) < 600000) {
    invoke('report_proactive_engagement').catch(() => {});
    lastProactiveTime = 0;
  }

  // Build message with optional file
  const isVoice = lastMessageWasVoice;
  const sttTime = lastSttTimeMs;
  lastMessageWasVoice = false;
  lastSttTimeMs = 0;

  let userContent = text;
  if (attachedFile) {
    userContent += `\n\n📎 Файл: ${attachedFile.name}\n\`\`\`\n${attachedFile.content}\n\`\`\``;
    addMsg('user', `${text}\n📎 ${attachedFile.name}`);
    attachedFile = null;
    attachPreview.style.display = 'none';
  } else {
    addMsg('user', text, isVoice);
  }

  // If previous message was a proactive/autonomous message, add context hint
  // so the model focuses on the user's reply, not on echoing itself
  const prevMsg = history[history.length - 1];
  if (prevMsg && prevMsg.proactive) {
    // Rewrite the proactive message to include a marker for the model
    prevMsg.content = `[Автономное сообщение Ханни]: ${prevMsg.content}`;
  }

  history.push({ role: 'user', content: userContent });
  // Set history index on user wrapper for edit support
  { const lastUserWrapper = chat.querySelector('.user-wrapper:last-of-type');
    if (lastUserWrapper) lastUserWrapper.dataset.historyIdx = String(history.length - 1); }

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
      history.push(assistantMsg);
      wrapper.dataset.historyIdx = String(history.length - 1);

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
        const { success, result: actionResult } = await executeAction(actionJson);
        indicatorDiv.remove();
        const actionDiv = document.createElement('div');
        actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
        actionDiv.textContent = actionResult;
        chat.appendChild(actionDiv);
        scrollDown();

        // Push tool result into history
        history.push({
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

    history.push({ role: 'assistant', content: result.fullReply });
    wrapper.dataset.historyIdx = String(history.length - 1);

    // Fallback path: parse ```action blocks from text (backward compat)
    const actions = parseAndExecuteActions(result.fullReply);

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
      const { success, result: actionResult } = await executeAction(actionJson);
      indicatorDiv.remove();
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
      scrollDown();
      results.push(actionResult);
    }

    // Feed results back into history so the model sees them
    history.push({ role: 'user', content: `[Action result: ${results.join('; ')}]` });
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
  const sttInfo = isVoice && sttTime ? `STT ${(sttTime / 1000).toFixed(1)}s · ` : '';
  timing.textContent = `${sttInfo}${ttft}s first token · ${total}s total · ${totalTokens} tokens${stepInfo}`;
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
      // Add feedback buttons to bot messages that have a history index
      if (currentConversationId) {
        chat.querySelectorAll('.msg-wrapper[data-history-idx]').forEach(w => {
          if (w.querySelector('.feedback-btn')) return;
          const idx = parseInt(w.dataset.historyIdx, 10);
          if (!isNaN(idx) && getRole(history[idx]) === 'assistant') {
            addFeedbackButtons(w, currentConversationId, idx, history[idx]?.content || '');
          }
        });
      }
      if (history.length >= 2) {
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

// ── Home ──
async function loadHome(subTab) {
  const el = document.getElementById('home-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('home') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Shopping List') loadShoppingList(pc);
  else loadSupplies(pc);
}

async function loadSupplies(el) {
  try {
    const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
    const categories = { cleaning: 'Cleaning', hygiene: 'Hygiene', household: 'Household', electronics: 'Electronics', tools: 'Tools', other: 'Other' };
    el.innerHTML = `
      <div class="module-header"><h2>Supplies</h2><button class="btn-primary" id="home-add-btn">+ Add Item</button></div>
      <div id="home-items-list">
        ${items.map(i => `<div class="focus-log-item" style="${i.needed ? 'border-left:2px solid var(--text-secondary);' : ''}">
          <span class="focus-log-title">${escapeHtml(i.name)}</span>
          <span class="badge badge-gray">${categories[i.category] || i.category}</span>
          ${i.quantity != null ? `<span style="color:var(--text-secondary);font-size:12px;">${i.quantity} ${i.unit||''}</span>` : ''}
          <span style="color:var(--text-faint);font-size:11px;">${i.location||''}</span>
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
          ${i.quantity != null ? `<span style="color:var(--text-secondary);font-size:12px;margin-left:auto;">${i.quantity} ${i.unit||''}</span>` : ''}
        </div>`).join('')}
      </div>` : '<div style="color:var(--text-faint);font-size:14px;padding:20px;text-align:center;">All stocked up!</div>'}`;
    el.querySelectorAll('[data-bought]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('toggle_home_item_needed', { id: parseInt(btn.dataset.bought) }).catch(()=>{});
        loadShoppingList(el);
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── Mindset ──
async function loadMindset(subTab) {
  const el = document.getElementById('mindset-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('mindset') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Journal') loadJournal(pc);
  else if (subTab === 'Mood') loadMoodLog(pc);
  else if (subTab === 'Principles') loadPrinciples(pc);
  else loadJournal(pc);
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
          <span style="color:var(--text-faint);font-size:11px;">${p.category||''}</span>
          <button class="memory-item-btn" data-del="${p.id}" style="margin-left:auto;">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-del]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (await confirmModal('Удалить?')) { await invoke('delete_principle', { id: parseInt(btn.dataset.del) }).catch(()=>{}); loadPrinciples(el); }
      });
    });
    document.getElementById('add-principle-btn')?.addEventListener('click', () => {
      const title = prompt('Principle:');
      if (title) invoke('create_principle', { title, description: '', category: 'discipline' }).then(() => loadPrinciples(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── Food ──
async function loadFood(subTab) {
  const el = document.getElementById('food-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('food') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Food Log') loadFoodLog(pc);
  else if (subTab === 'Recipes') loadRecipes(pc);
  else if (subTab === 'Products') loadProducts(pc);
  else loadFoodLog(pc);
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
    const fixedColumns = [
      { key: 'name', label: 'Name', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'prep_time', label: 'Prep', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.prep_time||0}+${r.cook_time||0} min</span>` },
      { key: 'calories', label: 'Calories', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories||'—'}</span>` },
      { key: 'tags', label: 'Tags', render: r => r.tags ? r.tags.split(',').map(t => `<span class="badge badge-gray">${t.trim()}</span>`).join(' ') : '' },
    ];
    el.innerHTML = '<div id="recipes-dbv"></div>';
    const dbvEl = document.getElementById('recipes-dbv');
    await renderDatabaseView(dbvEl, 'food', 'recipes', recipes, {
      fixedColumns, idField: 'id',
      addButton: '+ Add Recipe',
      onAdd: () => showAddRecipeModal(el),
      reloadFn: () => loadRecipes(el),
      _tabId: 'food', _recordTable: 'recipes',
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddRecipeModal(el) {
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
}

async function loadProducts(el) {
  try {
    const products = await invoke('get_products', { location: null, expiringSoon: false }).catch(() => []);
    const fixedColumns = [
      { key: 'name', label: 'Name', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'location', label: 'Location', render: r => `<span class="badge badge-gray">${r.location||''}</span>` },
      { key: 'quantity', label: 'Qty', render: r => r.quantity ? `<span style="font-size:12px;color:var(--text-secondary);">${r.quantity} ${r.unit||''}</span>` : '' },
      { key: 'expiry_date', label: 'Expiry', render: r => {
        if (!r.expiry_date) return '';
        const exp = new Date(r.expiry_date);
        const isExpiring = (exp - Date.now()) < 3 * 86400000;
        return `<span style="color:${isExpiring?'var(--color-red)':'var(--text-secondary)'};font-size:12px;">${r.expiry_date}</span>`;
      }},
    ];
    el.innerHTML = '<div id="products-dbv"></div>';
    const dbvEl = document.getElementById('products-dbv');
    await renderDatabaseView(dbvEl, 'food', 'products', products, {
      fixedColumns, idField: 'id',
      addButton: '+ Add Product',
      onAdd: () => showAddProductModal(el),
      reloadFn: () => loadProducts(el),
      _tabId: 'food', _recordTable: 'products',
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddProductModal(el) {
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
}

// ── Money ──
async function loadMoney(subTab) {
  const el = document.getElementById('money-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('money') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Expenses' || subTab === 'Income') loadTransactions(pc, subTab === 'Income' ? 'income' : 'expense');
  else if (subTab === 'Budget') loadBudgets(pc);
  else if (subTab === 'Savings') loadSavings(pc);
  else if (subTab === 'Subscriptions') loadSubscriptions(pc);
  else if (subTab === 'Debts') loadDebts(pc);
  else loadTransactions(pc, 'expense');
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
          <span class="focus-log-duration" style="color:${isExpense?'var(--text-muted)':'var(--text-primary)'}">${isExpense?'-':'+'} ${t.amount} ${t.currency||'KZT'}</span>
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
              <span style="color:var(--text-primary);font-size:14px;">${escapeHtml(b.category)}</span>
              <span style="color:${warn?'var(--color-yellow)':'var(--text-secondary)'};font-size:12px;">${b.spent||0} / ${b.amount} (${b.period})</span>
            </div>
            <div class="dev-progress" style="margin-top:6px;"><div class="dev-progress-bar" style="width:${pct}%;background:${warn?'var(--color-yellow)':'var(--accent-blue)'}"></div></div>
          </div>`;
        }).join('')}
      </div>`;
    document.getElementById('budget-add-btn')?.addEventListener('click', () => {
      const cat = prompt('Category:');
      const amt = prompt('Amount:');
      if (cat && amt) invoke('create_budget', { category: cat, amount: parseFloat(amt), period: 'monthly' }).then(() => loadBudgets(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
              <span style="color:var(--text-primary);font-size:14px;">${escapeHtml(g.name)}</span>
              <span style="color:var(--text-secondary);font-size:12px;">${g.current_amount} / ${g.target_amount}</span>
            </div>
            <div class="dev-progress" style="margin-top:6px;"><div class="dev-progress-bar" style="width:${pct}%"></div></div>
            ${g.deadline ? `<div style="font-size:11px;color:var(--text-faint);margin-top:4px;">Deadline: ${g.deadline}</div>` : ''}
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
      if (name && target) invoke('create_savings_goal', { name, targetAmount: parseFloat(target), currentAmount: 0, deadline: null, color: '#9B9B9B' }).then(() => loadSavings(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
          ${s.next_payment ? `<span style="color:var(--text-faint);font-size:11px;">Next: ${s.next_payment}</span>` : ''}
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
              <span style="color:var(--text-primary);font-size:14px;">${escapeHtml(d.name)}</span>
              <span class="badge ${d.type==='owe'?'badge-purple':'badge-green'}">${d.type==='owe'?'I owe':'Owed to me'}</span>
            </div>
            <div style="color:var(--text-secondary);font-size:12px;margin-top:4px;">Remaining: ${d.remaining} / ${d.amount}</div>
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── People Tab ──
async function loadPeople(subTab) {
  const el = document.getElementById('people-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('people') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  const filter = subTab === 'Blocked' ? { blocked: true } : subTab === 'Favorites' ? {} : {};
  try {
    const items = await invoke('get_contacts', filter);
    let contacts = Array.isArray(items) ? items : [];
    if (subTab === 'Favorites') contacts = contacts.filter(c => c.favorite);
    // Load blocks for each contact
    for (const c of contacts) {
      try { c._blocks = await invoke('get_contact_blocks', { contactId: c.id }); } catch { c._blocks = []; }
    }
    pc.innerHTML = `
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
                ${c.block_reason ? '<div class="contact-detail" style="color:var(--text-muted)">' + c.block_reason + '</div>' : ''}
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
    pc.innerHTML = `<div class="tab-stub"><div class="tab-stub-icon">⚠️</div>${e}</div>`;
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
  if (await confirmModal('Удалить контакт?')) {
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
        <input type="checkbox" id="mc-blocked"><label for="mc-blocked" style="color:var(--text-secondary);font-size:14px">Block this contact</label>
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
      <div class="memory-header">
        <div class="module-header" style="margin:0;flex:1;"><h2>Память</h2></div>
        <button class="btn-primary" id="mem-tab-add-btn">+ Добавить</button>
      </div>
      <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;">${memories.length} фактов</div>
      <div class="memory-browser" id="memory-all-list">
        ${memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
          <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
          <div class="memory-item-actions">
            <button class="memory-item-btn memory-edit-btn" data-medit="${m.id}" title="Редактировать">&#9998;</button>
            <button class="memory-item-btn" data-mdel="${m.id}" title="Удалить">&times;</button>
          </div>
        </div>`).join('')}
      </div>`;

    // Delete handlers
    el.querySelectorAll('[data-mdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (await confirmModal()) { await invoke('delete_memory', { id: parseInt(btn.dataset.mdel) }).catch(e => console.error('delete_memory error:', e)); loadAllFacts(el); }
      });
    });

    // Edit handlers
    el.querySelectorAll('[data-medit]').forEach(btn => {
      btn.addEventListener('click', () => {
        const id = parseInt(btn.dataset.medit);
        const m = memories.find(x => x.id === id);
        if (!m) return;
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.innerHTML = `<div class="modal modal-compact">
          <div class="modal-title">Редактировать факт</div>
          <div class="form-group"><label class="form-label">Категория</label>
            <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
          </div>
          <div class="form-group"><label class="form-label">Ключ</label>
            <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
          </div>
          <div class="form-group"><label class="form-label">Значение</label>
            <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
          </div>
          <div class="modal-actions">
            <button class="btn-secondary mem-cancel">Отмена</button>
            <button class="btn-primary mem-save">Сохранить</button>
          </div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
        overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
        overlay.querySelector('.mem-save').onclick = async () => {
          const cat = overlay.querySelector('.memory-edit-cat').value;
          const key = overlay.querySelector('.memory-edit-key').value.trim();
          const val = overlay.querySelector('.memory-edit-val').value.trim();
          if (!key || !val) return;
          try {
            await invoke('update_memory', { id, category: cat, key, value: val });
          } catch (err) { console.error('Memory edit error:', err); }
          overlay.remove();
          loadAllFacts(el);
        };
      });
    });

    // Add button
    document.getElementById('mem-tab-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Новый факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Добавить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-add-cat').value;
        const key = overlay.querySelector('.memory-add-key').value.trim();
        const val = overlay.querySelector('.memory-add-val').value.trim();
        if (!key || key.length < 2 || !val || val.length < 2) return;
        try { await invoke('memory_remember', { category: cat, key, value: val }); } catch (err) { console.error('Memory add error:', err); }
        overlay.remove();
        loadAllFacts(el);
      };
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

const MEMORY_CATEGORIES = ['user', 'preferences', 'people', 'habits', 'work', 'health', 'observation', 'other'];

function renderMemoryList(memories, el) {
  const list = document.getElementById('settings-mem-list');
  if (!list) return;
  const countEl = document.getElementById('settings-mem-count');
  if (countEl) countEl.textContent = `${memories.length} фактов`;
  list.innerHTML = memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
    <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
    <span class="memory-item-key">${escapeHtml(m.key)}</span>
    <span class="memory-item-value">${escapeHtml(m.value)}</span>
    <div class="memory-item-actions">
      <button class="memory-item-btn memory-edit-btn" data-edit="${m.id}" title="Редактировать">&#9998;</button>
      <button class="memory-item-btn" data-del="${m.id}" title="Удалить">&times;</button>
    </div>
  </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';

  list.querySelectorAll('[data-del]').forEach(btn => {
    btn.addEventListener('click', async () => {
      if (await confirmModal()) { await invoke('delete_memory', { id: parseInt(btn.dataset.del) }).catch(e => console.error('delete_memory error:', e)); loadMemoryInSettings(el); }
    });
  });

  list.querySelectorAll('[data-edit]').forEach(btn => {
    btn.addEventListener('click', () => {
      const id = parseInt(btn.dataset.edit);
      const m = memories.find(x => x.id === id);
      if (!m) return;
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Редактировать факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Сохранить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-edit-cat').value;
        const key = overlay.querySelector('.memory-edit-key').value.trim();
        const val = overlay.querySelector('.memory-edit-val').value.trim();
        if (!key || !val) return;
        try {
          await invoke('update_memory', { id, category: cat, key, value: val });
        } catch (err) { console.error('Failed to update memory:', err); }
        overlay.remove();
        loadMemoryInSettings(el);
      };
    });
  });
}

async function loadMemoryInSettings(el) {
  el.innerHTML = skeletonPage();
  try {
    const memories = await invoke('get_all_memories', { search: null }).catch(() => []);
    el.innerHTML = `
      <div class="memory-header">
        <div class="memory-search-box" style="flex:1;">
          <input class="form-input" id="settings-mem-search" placeholder="Поиск по памяти..." autocomplete="off">
        </div>
        <button class="btn-primary" id="mem-add-btn">+ Добавить</button>
      </div>
      <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;" id="settings-mem-count">${memories.length} фактов</div>
      <div class="memory-browser" id="settings-mem-list"></div>`;

    renderMemoryList(memories, el);

    document.getElementById('mem-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Новый факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Добавить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-add-cat').value;
        const key = overlay.querySelector('.memory-add-key').value.trim();
        const val = overlay.querySelector('.memory-add-val').value.trim();
        if (!key || !val) return;
        try {
          await invoke('memory_remember', { category: cat, key, value: val });
        } catch (err) { console.error('Failed to add memory:', err); }
        overlay.remove();
        loadMemoryInSettings(el);
      };
    });

    let searchTimeout;
    document.getElementById('settings-mem-search')?.addEventListener('input', async (e) => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(async () => {
        const q = e.target.value;
        const results = await invoke('get_all_memories', { search: q || null }).catch(() => []);
        renderMemoryList(results, el);
      }, 300);
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

async function loadMemorySearch(el) {
  el.innerHTML = `
    <div class="module-header"><h2>Поиск по памяти</h2></div>
    <div class="memory-search-box" style="margin-bottom:16px;">
      <input class="form-input" id="mem-search-input" placeholder="Поиск..." autocomplete="off">
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
          <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
        </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';
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

// ── About (Settings page) ──
async function loadAbout(el) {
  try {
    const info = await invoke('get_model_info').catch(() => ({}));
    el.innerHTML = `
      <div class="about-wrapper">
        <div class="about-card">
          <div class="about-header">
            <div class="about-logo">🤖</div>
            <div class="about-name">Hanni</div>
            <span class="about-version">v${APP_VERSION}</span>
          </div>
          <hr class="about-divider">
          <div class="about-info-list">
            <div class="about-info-row"><span class="about-info-label">Модель</span><span class="about-info-value">${info.model_name||'?'}</span></div>
            <div class="about-info-row"><span class="about-info-label">MLX сервер</span><span class="about-info-value ${info.server_online?'online':'offline'}">${info.server_online?'Онлайн':'Офлайн'}</span></div>
            <div class="about-info-row"><span class="about-info-label">HTTP API</span><span class="about-info-value" id="about-api-status">Проверяю...</span></div>
          </div>
          <hr class="about-divider">
          <div class="about-actions">
            <button class="settings-btn" id="about-check-update">Проверить обновления</button>
          </div>
        </div>
      </div>`;
    document.getElementById('about-check-update')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Проверяю...'; btn.disabled = true;
      try { const r = await invoke('check_update'); btn.textContent = r; }
      catch (err) { btn.textContent = 'Ошибка'; }
      setTimeout(() => { btn.textContent = 'Проверить обновления'; btn.disabled = false; }, 4000);
    });
    try {
      const resp = await fetch('http://127.0.0.1:8235/api/status');
      const apiEl = document.getElementById('about-api-status');
      if (apiEl) { apiEl.textContent = resp.ok ? 'Активен' : 'Недоступен'; apiEl.className = 'about-info-value ' + (resp.ok ? 'online' : 'offline'); }
    } catch (_) {
      const apiEl = document.getElementById('about-api-status');
      if (apiEl) { apiEl.textContent = 'Недоступен'; apiEl.className = 'about-info-value offline'; }
    }
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

// ── Tab loaders (stubs) ──
function showStub(containerId, icon, label, desc) {
  const el = document.getElementById(containerId);
  if (el) el.innerHTML = `<div class="tab-stub">
    <div class="tab-stub-icon">${icon}</div>
    <div class="tab-stub-title">${label}</div>
    ${desc ? `<div class="tab-stub-desc">${desc}</div>` : ''}
    <span class="tab-stub-badge">Скоро</span>
  </div>`;
}

// ── Dashboard ──
async function loadDashboard() {
  const el = document.getElementById('dashboard-content');
  if (!el) return;
  el.innerHTML = skeletonPage();
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

    el.innerHTML = renderPageHeader('dashboard') + `<div class="page-content">
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
      </div></div>`;
  } catch (e) {
    // Fallback for when backend command doesn't exist yet
    const now = new Date();
    const dateStr = now.toLocaleDateString('ru-RU', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
    const greeting = now.getHours() < 12 ? 'Доброе утро' : now.getHours() < 18 ? 'Добрый день' : 'Добрый вечер';
    el.innerHTML = renderPageHeader('dashboard') + `<div class="page-content">
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
      </div></div>`;
  }
}

// ── Focus ──
// Pomodoro state
let pomodoroState = { active: false, mode: 'work', workMin: 25, breakMin: 5, startedAt: 0, totalSec: 0 };

async function loadFocus(subTab) {
  const el = document.getElementById('focus-content');
  if (!el) return;
  if (subTab === 'History') { renderFocusHistory(el); return; }
  renderFocusCurrent(el);
}

async function renderFocusCurrent(el) {
  el.innerHTML = skeletonPage();
  try {
    const current = await invoke('get_current_activity').catch(() => null);
    const log = await invoke('get_activity_log', { date: null }).catch(() => []);
    const focusStatus = await invoke('get_focus_status').catch(() => ({ active: false }));

    let timerHtml = '';
    if (current) {
      // Active session — show ring timer
      timerHtml = `<div class="focus-current">
        <div class="focus-ring-container">
          <svg class="focus-ring" viewBox="0 0 120 120">
            <circle class="focus-ring-bg" cx="60" cy="60" r="54" />
            <circle class="focus-ring-progress" id="focus-ring-progress" cx="60" cy="60" r="54" />
          </svg>
          <div class="focus-ring-inner">
            <div class="focus-current-timer" id="focus-timer">00:00</div>
            <div class="focus-current-activity">${escapeHtml(current.title)}</div>
          </div>
        </div>
        ${focusStatus.active ? '<div class="focus-blocking-badge">🛡 Блокировка активна</div>' : ''}
        <div class="focus-actions">
          <button class="btn-danger" id="stop-activity-btn">Завершить</button>
        </div>
      </div>`;
    } else if (pomodoroState.active) {
      // Pomodoro break/work countdown
      const modeLabel = pomodoroState.mode === 'work' ? 'Работа' : 'Перерыв';
      timerHtml = `<div class="focus-current">
        <div class="focus-ring-container">
          <svg class="focus-ring" viewBox="0 0 120 120">
            <circle class="focus-ring-bg" cx="60" cy="60" r="54" />
            <circle class="focus-ring-progress ${pomodoroState.mode === 'break' ? 'break' : ''}" id="focus-ring-progress" cx="60" cy="60" r="54" />
          </svg>
          <div class="focus-ring-inner">
            <div class="focus-current-timer" id="focus-timer">00:00</div>
            <div class="focus-current-activity">${modeLabel}</div>
          </div>
        </div>
        <div class="focus-actions">
          <button class="btn-secondary" id="pomo-skip-btn">Пропустить</button>
          <button class="btn-danger" id="pomo-stop-btn">Стоп</button>
        </div>
      </div>`;
    } else {
      // Idle — show start form + pomodoro
      timerHtml = `<div class="focus-current">
        <div class="focus-ring-container idle">
          <svg class="focus-ring" viewBox="0 0 120 120">
            <circle class="focus-ring-bg" cx="60" cy="60" r="54" />
          </svg>
          <div class="focus-ring-inner">
            <div class="focus-current-timer" style="color:var(--text-muted);">00:00</div>
            <div class="focus-current-activity" style="color:var(--text-faint);">Готов</div>
          </div>
        </div>
        <input id="activity-title" class="form-input focus-title-input" placeholder="Название активности..." autocomplete="off">
        <div class="focus-presets" id="activity-presets">
          <button class="focus-preset" data-category="work">Работа</button>
          <button class="focus-preset" data-category="study">Учёба</button>
          <button class="focus-preset" data-category="sport">Спорт</button>
          <button class="focus-preset" data-category="rest">Отдых</button>
          <button class="focus-preset" data-category="hobby">Хобби</button>
          <button class="focus-preset" data-category="other">Другое</button>
        </div>
        <div class="focus-start-row">
          <label class="focus-check-label"><input type="checkbox" id="focus-block-check"> Блокировать отвлечения</label>
        </div>
        <div class="focus-actions">
          <button class="btn-primary" id="start-activity-btn">Начать</button>
          <button class="btn-secondary" id="start-pomo-btn" title="Pomodoro 25/5">🍅 Помодоро</button>
        </div>
      </div>`;
    }

    // Today's stats summary
    const totalMin = log.reduce((sum, item) => {
      const match = (item.duration || '').match(/(\d+)ч\s*(\d+)м|(\d+)м/);
      if (match) return sum + (parseInt(match[1] || 0) * 60) + parseInt(match[2] || match[3] || 0);
      return sum;
    }, 0);
    const categories = {};
    log.forEach(item => { categories[item.category || 'other'] = (categories[item.category || 'other'] || 0) + 1; });
    const catLabels = { work: 'Работа', study: 'Учёба', sport: 'Спорт', rest: 'Отдых', hobby: 'Хобби', other: 'Другое' };

    const statsHtml = `<div class="focus-today-stats">
      <div class="focus-stat"><div class="focus-stat-value">${Math.floor(totalMin / 60)}ч ${totalMin % 60}м</div><div class="focus-stat-label">Всего сегодня</div></div>
      <div class="focus-stat"><div class="focus-stat-value">${log.length}</div><div class="focus-stat-label">Сессий</div></div>
      ${Object.entries(categories).slice(0, 3).map(([cat, count]) =>
        `<div class="focus-stat"><div class="focus-stat-value">${count}</div><div class="focus-stat-label">${catLabels[cat] || cat}</div></div>`
      ).join('')}
    </div>`;

    const logHtml = log.length > 0 ? `
      <div class="focus-log-header">Сегодня</div>
      <div class="focus-log">
        ${log.map(item => `<div class="focus-log-item">
          <span class="focus-log-time">${item.time || ''}</span>
          <span class="focus-log-title">${escapeHtml(item.title)}</span>
          <span class="focus-log-category">${item.category || ''}</span>
          <span class="focus-log-duration">${item.duration || ''}</span>
        </div>`).join('')}
      </div>` : '';

    el.innerHTML = renderPageHeader('focus') + `<div class="page-content">${timerHtml}${statsHtml}${logHtml}</div>`;

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
        loadFocus('Current');
      } catch (err) { alert('Ошибка: ' + err); }
    });

    document.getElementById('stop-activity-btn')?.addEventListener('click', async () => {
      try { await invoke('stop_activity'); loadFocus('Current'); }
      catch (err) { alert('Ошибка: ' + err); }
    });

    // Pomodoro buttons
    document.getElementById('start-pomo-btn')?.addEventListener('click', () => {
      const title = document.getElementById('activity-title')?.value?.trim() || 'Помодоро';
      const focusMode = document.getElementById('focus-block-check')?.checked || false;
      startPomodoro(title, selectedCategory, focusMode);
    });

    document.getElementById('pomo-skip-btn')?.addEventListener('click', () => {
      if (pomodoroState.mode === 'work') {
        invoke('stop_activity').catch(() => {});
        pomodoroState.mode = 'break';
        pomodoroState.startedAt = Date.now();
        pomodoroState.totalSec = pomodoroState.breakMin * 60;
      } else {
        pomodoroState.mode = 'work';
        pomodoroState.startedAt = Date.now();
        pomodoroState.totalSec = pomodoroState.workMin * 60;
        invoke('start_activity', { title: 'Помодоро', category: 'work', focusMode: false, duration: null, apps: null, sites: null }).catch(() => {});
      }
      loadFocus('Current');
    });

    document.getElementById('pomo-stop-btn')?.addEventListener('click', () => {
      pomodoroState.active = false;
      invoke('stop_activity').catch(() => {});
      loadFocus('Current');
    });

    // Update timer
    startFocusTimer(current);

  } catch (e) {
    showStub('focus-content', '🎯', 'Фокус', 'Глубокая работа и трекинг активности');
  }
}

function startFocusTimer(current) {
  if (focusTimerInterval) clearInterval(focusTimerInterval);
  const circumference = 2 * Math.PI * 54;

  if (current && current.started_at) {
    const startedAt = new Date(current.started_at).getTime();
    const updateTimer = () => {
      const timerEl = document.getElementById('focus-timer');
      const ringEl = document.getElementById('focus-ring-progress');
      if (!timerEl || activeTab !== 'focus') { clearInterval(focusTimerInterval); return; }
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      const h = Math.floor(elapsed / 3600);
      const m = Math.floor((elapsed % 3600) / 60);
      const s = elapsed % 60;
      timerEl.textContent = h > 0 ? `${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}` : `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
      // Ring: animate based on minutes (full circle = 60 min)
      if (ringEl) {
        const progress = Math.min(1, elapsed / 3600);
        ringEl.style.strokeDasharray = circumference;
        ringEl.style.strokeDashoffset = circumference * (1 - progress);
      }
    };
    updateTimer();
    focusTimerInterval = setInterval(updateTimer, 1000);
  } else if (pomodoroState.active) {
    const updatePomo = () => {
      const timerEl = document.getElementById('focus-timer');
      const ringEl = document.getElementById('focus-ring-progress');
      if (!timerEl || activeTab !== 'focus') { clearInterval(focusTimerInterval); return; }
      const elapsed = Math.floor((Date.now() - pomodoroState.startedAt) / 1000);
      const remaining = Math.max(0, pomodoroState.totalSec - elapsed);
      const m = Math.floor(remaining / 60);
      const s = remaining % 60;
      timerEl.textContent = `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
      if (ringEl) {
        const progress = 1 - (remaining / pomodoroState.totalSec);
        ringEl.style.strokeDasharray = circumference;
        ringEl.style.strokeDashoffset = circumference * (1 - progress);
      }
      if (remaining <= 0) {
        clearInterval(focusTimerInterval);
        if (pomodoroState.mode === 'work') {
          invoke('stop_activity').catch(() => {});
          invoke('send_notification', { title: 'Помодоро', body: 'Время перерыва! 5 минут.' }).catch(() => {});
          pomodoroState.mode = 'break';
          pomodoroState.startedAt = Date.now();
          pomodoroState.totalSec = pomodoroState.breakMin * 60;
        } else {
          invoke('send_notification', { title: 'Помодоро', body: 'Перерыв окончен! Поехали.' }).catch(() => {});
          pomodoroState.mode = 'work';
          pomodoroState.startedAt = Date.now();
          pomodoroState.totalSec = pomodoroState.workMin * 60;
          invoke('start_activity', { title: 'Помодоро', category: 'work', focusMode: false, duration: null, apps: null, sites: null }).catch(() => {});
        }
        loadFocus('Current');
      }
    };
    updatePomo();
    focusTimerInterval = setInterval(updatePomo, 1000);
  }
}

async function startPomodoro(title, category, focusMode) {
  pomodoroState = { active: true, mode: 'work', workMin: 25, breakMin: 5, startedAt: Date.now(), totalSec: 25 * 60 };
  try {
    await invoke('start_activity', { title: title || 'Помодоро', category: category || 'work', focusMode, duration: 25, apps: null, sites: null });
  } catch (_) {}
  loadFocus('Current');
}

async function renderFocusHistory(el) {
  el.innerHTML = skeletonPage();
  try {
    // Load last 7 days
    const days = [];
    const today = new Date();
    for (let i = 0; i < 7; i++) {
      const d = new Date(today);
      d.setDate(today.getDate() - i);
      const dateStr = d.toISOString().slice(0, 10);
      const log = await invoke('get_activity_log', { date: dateStr }).catch(() => []);
      const totalMin = log.reduce((sum, item) => {
        const match = (item.duration || '').match(/(\d+)ч\s*(\d+)м|(\d+)м/);
        if (match) return sum + (parseInt(match[1] || 0) * 60) + parseInt(match[2] || match[3] || 0);
        return sum;
      }, 0);
      days.push({ date: dateStr, log, totalMin });
    }

    const maxMin = Math.max(1, ...days.map(d => d.totalMin));
    const dayNames = ['Вс', 'Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб'];
    const todayStr = today.toISOString().slice(0, 10);

    // Weekly bar chart
    const chartHtml = `<div class="focus-chart">
      <div class="focus-chart-title">Последние 7 дней</div>
      <div class="focus-chart-bars">
        ${days.slice().reverse().map(d => {
          const pct = Math.round((d.totalMin / maxMin) * 100);
          const dayDate = new Date(d.date + 'T00:00:00');
          const label = d.date === todayStr ? 'Сегодня' : dayNames[dayDate.getDay()];
          const hours = Math.floor(d.totalMin / 60);
          const mins = d.totalMin % 60;
          const timeLabel = hours > 0 ? `${hours}ч${mins > 0 ? ` ${mins}м` : ''}` : `${mins}м`;
          return `<div class="focus-chart-bar-col">
            <div class="focus-chart-bar-value">${d.totalMin > 0 ? timeLabel : ''}</div>
            <div class="focus-chart-bar-track"><div class="focus-chart-bar" style="height:${Math.max(2, pct)}%"></div></div>
            <div class="focus-chart-bar-label">${label}</div>
          </div>`;
        }).join('')}
      </div>
    </div>`;

    // Category breakdown for this week
    const allLogs = days.flatMap(d => d.log);
    const catTotals = {};
    const catLabels = { work: 'Работа', study: 'Учёба', sport: 'Спорт', rest: 'Отдых', hobby: 'Хобби', other: 'Другое' };
    const catColors = { work: 'var(--color-blue)', study: 'var(--color-purple)', sport: 'var(--color-green)', rest: 'var(--color-yellow)', hobby: 'var(--color-pink)', other: 'var(--text-muted)' };
    allLogs.forEach(item => {
      const cat = item.category || 'other';
      const match = (item.duration || '').match(/(\d+)ч\s*(\d+)м|(\d+)м/);
      const min = match ? (parseInt(match[1] || 0) * 60) + parseInt(match[2] || match[3] || 0) : 0;
      catTotals[cat] = (catTotals[cat] || 0) + min;
    });
    const totalWeekMin = Object.values(catTotals).reduce((a, b) => a + b, 0);

    const breakdownHtml = totalWeekMin > 0 ? `<div class="focus-breakdown">
      <div class="focus-chart-title">По категориям</div>
      ${Object.entries(catTotals).sort((a, b) => b[1] - a[1]).map(([cat, min]) => {
        const pct = Math.round((min / totalWeekMin) * 100);
        const hours = Math.floor(min / 60);
        const mins = min % 60;
        return `<div class="focus-breakdown-row">
          <span class="focus-breakdown-label">${catLabels[cat] || cat}</span>
          <div class="focus-breakdown-bar-track"><div class="focus-breakdown-bar" style="width:${pct}%;background:${catColors[cat] || 'var(--text-muted)'}"></div></div>
          <span class="focus-breakdown-value">${hours > 0 ? `${hours}ч ${mins}м` : `${mins}м`}</span>
        </div>`;
      }).join('')}
    </div>` : '';

    // Daily logs
    const logsHtml = days.filter(d => d.log.length > 0).map(d => {
      const dayDate = new Date(d.date + 'T00:00:00');
      const label = d.date === todayStr ? 'Сегодня' : `${dayDate.getDate()} ${['янв','фев','мар','апр','мая','июн','июл','авг','сен','окт','ноя','дек'][dayDate.getMonth()]}, ${dayNames[dayDate.getDay()]}`;
      return `<div class="focus-day-group">
        <div class="focus-log-header">${label}</div>
        <div class="focus-log">
          ${d.log.map(item => `<div class="focus-log-item">
            <span class="focus-log-time">${item.time || ''}</span>
            <span class="focus-log-title">${escapeHtml(item.title)}</span>
            <span class="focus-log-category">${item.category || ''}</span>
            <span class="focus-log-duration">${item.duration || ''}</span>
          </div>`).join('')}
        </div>
      </div>`;
    }).join('');

    el.innerHTML = renderPageHeader('focus') + `<div class="page-content">${chartHtml}${breakdownHtml}${logsHtml || '<div class="empty-state"><div class="empty-state-icon">📊</div><div class="empty-state-text">Нет данных за неделю</div></div>'}</div>`;

  } catch (e) {
    showStub('focus-content', '📊', 'История', 'История активностей');
  }
}

// ── Custom Pages ──
const COMMON_EMOJIS = ['📄','📝','📋','📌','📎','📁','💡','🎯','🔥','⭐','🏠','💼','🎨','🎮','📚','🎵','💰','🏋️','❤️','🧠','🍔','📅','🔧','🚀','🌟','✅','📊','🗂️','💬','🔔'];
let customPageAutoSave = null;

async function loadCustomPage(tabId) {
  const reg = TAB_REGISTRY[tabId];
  if (!reg?.custom || !reg.pageId) return;
  const el = document.getElementById(`${tabId}-content`);
  if (!el) return;

  try {
    const page = await invoke('get_custom_page', { id: reg.pageId });

    el.innerHTML = `
      <div class="custom-page-header">
        <div class="custom-page-icon-row">
          <button class="custom-page-icon-btn" id="cp-icon-btn" title="Сменить иконку">${escapeHtml(page.icon || '📄')}</button>
          <button class="btn-danger btn-small custom-page-delete-btn" id="cp-delete-btn">Удалить</button>
        </div>
        <input class="page-title-input" id="cp-title" value="${escapeHtml(page.title || '')}" placeholder="Без названия">
        <input class="page-description-input" id="cp-desc" value="${escapeHtml(page.description || '')}" placeholder="Добавить описание...">
      </div>
      <div class="custom-page-content">
        <textarea class="custom-page-body" id="cp-body" placeholder="Начните писать...">${escapeHtml(page.content || '')}</textarea>
      </div>
      <div class="custom-page-emoji-picker hidden" id="cp-emoji-picker">
        ${COMMON_EMOJIS.map(e => `<button class="emoji-pick-btn">${e}</button>`).join('')}
      </div>`;

    // Auto-save helper
    const autoSave = (field, value) => {
      clearTimeout(customPageAutoSave);
      customPageAutoSave = setTimeout(async () => {
        const args = { id: reg.pageId };
        args[field] = value;
        await invoke('update_custom_page', args).catch(() => {});
        // Sync title/icon to tab bar
        if (field === 'title') { reg.label = value || 'Без названия'; renderTabBar(); }
        if (field === 'icon') { reg.icon = value; renderTabBar(); }
      }, 500);
    };

    document.getElementById('cp-title')?.addEventListener('input', (e) => autoSave('title', e.target.value));
    document.getElementById('cp-desc')?.addEventListener('input', (e) => autoSave('description', e.target.value));
    document.getElementById('cp-body')?.addEventListener('input', (e) => autoSave('content', e.target.value));

    // Auto-resize textarea
    const body = document.getElementById('cp-body');
    if (body) {
      const resize = () => { body.style.height = 'auto'; body.style.height = body.scrollHeight + 'px'; };
      body.addEventListener('input', resize);
      setTimeout(resize, 0);
    }

    // Emoji picker
    const emojiPicker = document.getElementById('cp-emoji-picker');
    document.getElementById('cp-icon-btn')?.addEventListener('click', (e) => {
      e.stopPropagation();
      emojiPicker?.classList.toggle('hidden');
    });
    document.querySelectorAll('.emoji-pick-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const emoji = btn.textContent;
        document.getElementById('cp-icon-btn').textContent = emoji;
        emojiPicker?.classList.add('hidden');
        autoSave('icon', emoji);
      });
    });
    // Close emoji picker on outside click
    const closeEmojiPicker = (e) => {
      if (!emojiPicker?.contains(e.target) && e.target.id !== 'cp-icon-btn') {
        emojiPicker?.classList.add('hidden');
      }
    };
    document.addEventListener('click', closeEmojiPicker);

    // Delete page
    document.getElementById('cp-delete-btn')?.addEventListener('click', async () => {
      if (!(await confirmModal('Удалить страницу?'))) return;
      await invoke('delete_custom_page', { id: reg.pageId }).catch(() => {});
      closeTab(tabId);
      delete TAB_REGISTRY[tabId];
      const viewDiv = document.getElementById(`view-${tabId}`);
      if (viewDiv) viewDiv.remove();
    });

  } catch (e) {
    el.innerHTML = `<div class="empty-state"><div class="empty-state-icon">📄</div><div class="empty-state-text">Страница не найдена</div></div>`;
  }
}

// ── Notes ──
let notesViewMode = 'list'; // 'list' or 'edit'
let notePreviewMode = false;

function formatNoteDate(dateStr) {
  const date = new Date(dateStr);
  const now = new Date();
  const diff = now - date;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'только что';
  if (mins < 60) return `${mins} мин назад`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} ч назад`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days} дн назад`;
  return date.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' });
}

function formatDueDate(dateStr) {
  if (!dateStr) return '';
  const due = new Date(dateStr + (dateStr.includes('T') ? '' : 'T23:59:59'));
  const now = new Date();
  const diffMs = due - now;
  const days = Math.ceil(diffMs / 86400000);
  if (days < 0) return `<span class="due-overdue">просрочено ${Math.abs(days)} дн</span>`;
  if (days === 0) return '<span class="due-today">сегодня</span>';
  if (days <= 3) return `<span class="due-soon">через ${days} дн</span>`;
  return `<span class="due-later">через ${days} дн</span>`;
}

function renderNoteTags(tagsStr) {
  if (!tagsStr) return '';
  return tagsStr.split(',').map(t => t.trim()).filter(Boolean).map(t => {
    const color = tagColorMap[t] || 'blue';
    return `<span class="note-tag badge-${color}">${escapeHtml(t)}</span>`;
  }).join('');
}

async function loadTagColorMap() {
  try {
    const tags = await invoke('get_note_tags');
    tagColorMap = {};
    for (const t of tags) tagColorMap[t.name] = t.color;
  } catch (_) {}
}

async function loadNotes(subTab) {
  await renderNotesPage();
}

// ── Notes Page: Notion-like views + filter chips ──

function applyNotesFilters(notes) {
  let result = notes;

  // Status filters (OR logic)
  if (notesFilters.size > 0) {
    const todayStr = new Date().toISOString().slice(0, 10);
    result = result.filter(n => {
      const conditions = [];
      if (notesFilters.has('pin')) conditions.push(!!n.pinned);
      if (notesFilters.has('archive')) conditions.push(!!n.archived);
      if (notesFilters.has('task')) conditions.push(n.status === 'task');
      if (notesFilters.has('done')) conditions.push(n.status === 'done');
      if (notesFilters.has('overdue')) conditions.push(n.status === 'task' && n.due_date && n.due_date < todayStr);
      return conditions.some(c => c);
    });
  } else {
    result = result.filter(n => !n.archived);
  }

  // Tag filter (AND)
  if (noteTagFilter) {
    result = result.filter(n => (n.tags || '').split(',').map(t => t.trim()).includes(noteTagFilter));
  }

  // Search filter
  if (notesSearchQuery) {
    const q = notesSearchQuery.toLowerCase();
    result = result.filter(n =>
      (n.title || '').toLowerCase().includes(q) ||
      (n.content || '').toLowerCase().includes(q) ||
      (n.tags || '').toLowerCase().includes(q)
    );
  }

  return result;
}

async function renderNotesPage() {
  const el = document.getElementById('notes-content');
  if (!el) return;

  if (notesViewMode === 'edit' && currentNoteId) {
    renderNoteEditor(el, currentNoteId);
    return;
  }

  notesViewMode = 'list';
  await loadTagColorMap();

  let allNotes;
  try {
    allNotes = await invoke('get_notes', { filter: null, search: null }) || [];
  } catch (e) {
    showStub('notes-content', '📝', 'Заметки', 'Быстрые заметки и мысли');
    return;
  }

  const filtered = applyNotesFilters(allNotes);

  // Collect all tags
  const allTags = new Set();
  allNotes.forEach(n => (n.tags || '').split(',').map(t => t.trim()).filter(Boolean).forEach(t => allTags.add(t)));

  el.innerHTML = renderNotesHeader() + renderNotesViewBar() + renderNotesFilterBar(allTags) + `<div id="notes-view-content" class="page-content"></div>`;

  const content = document.getElementById('notes-view-content');
  if (!content) return;

  switch (notesView) {
    case 'kanban':   renderKanbanContent(content, filtered); break;
    case 'timeline': renderTimelineContent(content, filtered); break;
    case 'table':    renderTableView(content, filtered); break;
    case 'gallery':  renderGalleryView(content, filtered); break;
    default:         renderListContent(content, filtered, allNotes); break;
  }

  setupNotesControls();
}

function renderNotesHeader() {
  const customIcon = tabCustomizations.notes?.icon;
  const iconHtml = customIcon
    ? `<button class="page-header-icon-btn" data-tab-id="notes" title="Сменить иконку">${customIcon}</button>`
    : `<button class="page-header-icon-btn page-header-icon-svg" data-tab-id="notes" title="Сменить иконку">${TAB_ICONS.notes}</button>`;
  const desc = getTabDesc('notes');
  return `<div class="notes-header" data-tab-id="notes">
    <div class="notes-header-top">
      <div class="notes-header-left">
        ${iconHtml}
        <div class="page-header-title">Notes</div>
      </div>
      <div class="notes-header-actions">
        <button class="btn-primary" id="new-note-btn">+ Новая</button>
        <button class="btn-secondary" id="new-task-btn">+ Задача</button>
        <div class="notes-search-wrap">
          <input class="form-input" id="notes-search" placeholder="Поиск..." autocomplete="off" value="${escapeHtml(notesSearchQuery)}">
        </div>
      </div>
    </div>
    ${desc ? `<input class="page-header-desc-input" data-tab-id="notes" value="${escapeHtml(desc)}" placeholder="Добавить описание...">` : `<input class="page-header-desc-input" data-tab-id="notes" value="" placeholder="Добавить описание...">`}
    <div class="page-emoji-picker hidden" id="page-emoji-picker-notes">
      ${PAGE_EMOJIS.map(e => `<button class="emoji-pick-btn" data-emoji="${e}">${e}</button>`).join('')}
    </div>
  </div>`;
}

function renderNotesViewBar() {
  const views = [
    { id: 'all', label: 'All' },
    { id: 'kanban', label: 'Kanban' },
    { id: 'timeline', label: 'Timeline' },
    { id: 'table', label: 'Table' },
    { id: 'gallery', label: 'Gallery' },
  ];
  return `<div class="notes-view-bar">
    ${views.map(v => `<button class="notes-view-btn${notesView === v.id ? ' active' : ''}" data-view="${v.id}">${v.label}</button>`).join('')}
  </div>`;
}

function renderNotesFilterBar(allTags) {
  const filters = [
    { id: 'pin', label: '📌 Pin' },
    { id: 'archive', label: '📦 Архив' },
    { id: 'task', label: '☐ Задачи' },
    { id: 'done', label: '✅ Готово' },
    { id: 'overdue', label: '🔴 Просрочено' },
  ];
  const tagChips = allTags.size > 0 ? `<span class="notes-filter-divider"></span>` +
    [...allTags].map(t => `<button class="notes-filter-chip tag badge-${tagColorMap[t] || 'blue'}${noteTagFilter === t ? ' active' : ''}" data-tag="${escapeHtml(t)}">${escapeHtml(t)}</button>`).join('') : '';

  return `<div class="notes-filter-bar">
    ${filters.map(f => `<button class="notes-filter-chip${notesFilters.has(f.id) ? ' active' : ''}" data-filter="${f.id}">${f.label}</button>`).join('')}
    ${tagChips}
  </div>`;
}

function setupNotesControls() {
  // Page header controls (icon picker, description)
  setupPageHeaderControls('notes');

  // View bar clicks
  document.querySelectorAll('.notes-view-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      notesView = btn.dataset.view;
      localStorage.setItem('hanni_notes_view', notesView);
      renderNotesPage();
    });
  });

  // Filter chip clicks
  document.querySelectorAll('.notes-filter-chip[data-filter]').forEach(chip => {
    chip.addEventListener('click', () => {
      const f = chip.dataset.filter;
      if (notesFilters.has(f)) notesFilters.delete(f);
      else notesFilters.add(f);
      renderNotesPage();
    });
  });

  // Tag chip clicks
  document.querySelectorAll('.notes-filter-chip[data-tag]').forEach(chip => {
    chip.addEventListener('click', () => {
      const t = chip.dataset.tag;
      noteTagFilter = noteTagFilter === t ? null : t;
      renderNotesPage();
    });
  });

  // New note / task
  document.getElementById('new-note-btn')?.addEventListener('click', createAndOpenNote);
  document.getElementById('new-task-btn')?.addEventListener('click', createAndOpenTask);

  // Search
  document.getElementById('notes-search')?.addEventListener('input', (e) => {
    clearTimeout(noteAutoSaveTimeout);
    noteAutoSaveTimeout = setTimeout(() => {
      notesSearchQuery = e.target.value || '';
      renderNotesPage();
    }, 300);
  });
}

// ── List View (default) ──
function renderListContent(container, notes, allNotes) {
  const pinned = notes.filter(n => n.pinned && !n.archived);
  const regular = notes.filter(n => !n.pinned);

  if (notes.length === 0) {
    container.innerHTML = `<div class="empty-state">
      <div class="empty-state-icon">📝</div>
      <div class="empty-state-text">Нет заметок</div>
      <button class="btn-primary" id="empty-new-note-btn">Создать первую</button>
    </div>`;
    document.getElementById('empty-new-note-btn')?.addEventListener('click', createAndOpenNote);
    return;
  }

  const list = document.createElement('div');
  list.className = 'notes-card-list';
  list.id = 'notes-card-list';
  const refresh = () => renderNotesPage();

  if (pinned.length > 0) {
    const section = document.createElement('div');
    section.className = 'notes-section-label';
    section.textContent = '📌 Закреплённые';
    list.appendChild(section);
    for (const note of pinned) list.appendChild(createNoteCard(note, refresh));

    if (regular.length > 0) {
      const sep = document.createElement('div');
      sep.className = 'notes-section-label';
      sep.textContent = 'Все заметки';
      list.appendChild(sep);
    }
  }
  for (const note of regular) list.appendChild(createNoteCard(note, refresh));

  container.appendChild(list);
  setupNotesDnD(list);
}

// ── Kanban View ──
function renderKanbanContent(container, notes) {
  const columns = [
    { status: 'note', label: 'Заметки', icon: '📝', items: notes.filter(n => !n.status || n.status === 'note') },
    { status: 'task', label: 'Задачи', icon: '☐', items: notes.filter(n => n.status === 'task') },
    { status: 'done', label: 'Готово', icon: '✅', items: notes.filter(n => n.status === 'done') },
  ];

  container.innerHTML = `<div class="kanban-board" id="kanban-board">
    ${columns.map(col => `
      <div class="kanban-column" data-status="${col.status}">
        <div class="kanban-column-header">
          <span>${col.icon} ${col.label}</span>
          <span class="kanban-column-count">${col.items.length}</span>
          <button class="kanban-add-btn" data-status="${col.status}">+</button>
        </div>
        <div class="kanban-column-cards" data-status="${col.status}"></div>
      </div>
    `).join('')}
  </div>`;

  const refresh = () => renderNotesPage();
  for (const col of columns) {
    const colEl = container.querySelector(`.kanban-column-cards[data-status="${col.status}"]`);
    if (!colEl) continue;
    for (const note of col.items) colEl.appendChild(createNoteCard(note, refresh));
  }

  setupKanbanDnD(container.querySelector('#kanban-board'), refresh);

  container.querySelectorAll('.kanban-add-btn').forEach(btn => {
    btn.addEventListener('click', async () => {
      const status = btn.dataset.status === 'note' ? null : btn.dataset.status;
      try {
        const id = await invoke('create_note', { title: '', content: '', tags: '', status, tabName: null, dueDate: null, reminderAt: null });
        currentNoteId = id;
        notesViewMode = 'edit';
        const el = document.getElementById('notes-content');
        if (el) renderNoteEditor(el, id);
      } catch (err) { console.error('kanban create:', err); }
    });
  });
}

function setupKanbanDnD(board, refresh) {
  if (!board) return;
  board.querySelectorAll('.kanban-column-cards').forEach(col => {
    col.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      col.classList.add('drag-over');
    });
    col.addEventListener('dragleave', (e) => {
      if (!col.contains(e.relatedTarget)) col.classList.remove('drag-over');
    });
    col.addEventListener('drop', async (e) => {
      e.preventDefault();
      col.classList.remove('drag-over');
      const noteId = parseInt(e.dataTransfer.getData('text/plain'));
      if (!noteId) return;
      const targetStatus = col.dataset.status;
      try {
        await invoke('update_note_status', { id: noteId, status: targetStatus });
        refresh();
      } catch (err) { console.error('kanban drop:', err); }
    });
  });
}

// ── Timeline View ──
function renderTimelineContent(container, notes) {
  const now = new Date();
  const todayStr = now.toISOString().slice(0, 10);

  const dayOfWeek = now.getDay() || 7;
  const thisMonday = new Date(now);
  thisMonday.setDate(now.getDate() - dayOfWeek + 1);
  thisMonday.setHours(0, 0, 0, 0);
  const thisSunday = new Date(thisMonday);
  thisSunday.setDate(thisMonday.getDate() + 6);
  const nextMonday = new Date(thisMonday);
  nextMonday.setDate(thisMonday.getDate() + 7);
  const nextSunday = new Date(nextMonday);
  nextSunday.setDate(nextMonday.getDate() + 6);

  const toDateStr = (d) => `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
  const thisMondayStr = toDateStr(thisMonday);
  const thisSundayStr = toDateStr(thisSunday);
  const nextMondayStr = toDateStr(nextMonday);
  const nextSundayStr = toDateStr(nextSunday);

  const overdue = notes.filter(n => n.status === 'task' && n.due_date && n.due_date < todayStr);
  const thisWeek = notes.filter(n => n.due_date && n.due_date >= thisMondayStr && n.due_date <= thisSundayStr && !(n.status === 'task' && n.due_date < todayStr));
  const nextWeek = notes.filter(n => n.due_date && n.due_date >= nextMondayStr && n.due_date <= nextSundayStr);
  const later = notes.filter(n => n.due_date && n.due_date > nextSundayStr);
  const noDate = notes.filter(n => !n.due_date);

  const formatWeekRange = (mon, sun) => {
    const months = ['янв', 'фев', 'мар', 'апр', 'мая', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
    return `${mon.getDate()} ${months[mon.getMonth()]} – ${sun.getDate()} ${months[sun.getMonth()]}`;
  };

  const groups = [
    { label: 'Просроченные', items: overdue, cls: 'overdue' },
    { label: `Эта неделя (${formatWeekRange(thisMonday, thisSunday)})`, items: thisWeek, cls: '' },
    { label: `Следующая неделя (${formatWeekRange(nextMonday, nextSunday)})`, items: nextWeek, cls: '' },
    { label: 'Позже', items: later, cls: '' },
    { label: 'Без даты', items: noDate, cls: 'no-date' },
  ];

  const months = ['янв', 'фев', 'мар', 'апр', 'мая', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
  const formatDateShort = (dateStr) => {
    if (!dateStr) return '';
    const d = new Date(dateStr + 'T00:00:00');
    return `${d.getDate()} ${months[d.getMonth()]}`;
  };

  const renderItem = (note) => {
    const isDone = note.status === 'done';
    const isOverdue = note.status === 'task' && note.due_date && note.due_date < todayStr;
    const isToday = note.due_date === todayStr;
    const tagsHtml = renderNoteTags(note.tags);
    const dotCls = isDone ? 'done' : isOverdue ? 'overdue' : '';
    const todayBadge = isToday ? '<span class="timeline-today-badge">сегодня</span>' : '';
    const overdueBadge = isOverdue ? `<span class="timeline-overdue-badge">просрочено</span>` : '';
    return `<div class="timeline-item${isDone ? ' task-done' : ''}" data-note-id="${note.id}">
      <div class="timeline-date">${formatDateShort(note.due_date)}</div>
      <div class="timeline-dot ${dotCls}"></div>
      <div class="timeline-content">
        <div class="timeline-title">${escapeHtml(note.title || 'Без названия')}</div>
        ${tagsHtml ? `<div class="timeline-tags">${tagsHtml}</div>` : ''}
      </div>
      ${todayBadge}${overdueBadge}
    </div>`;
  };

  const groupsHtml = groups
    .filter(g => g.items.length > 0)
    .map(g => `<div class="timeline-group ${g.cls}">
      <div class="timeline-group-header">${g.label}</div>
      ${g.items.map(renderItem).join('')}
    </div>`).join('');

  container.innerHTML = `<div class="timeline-view">${groupsHtml || '<div class="empty-state"><div class="empty-state-icon">📅</div><div class="empty-state-text">Нет задач</div></div>'}</div>`;

  container.querySelectorAll('.timeline-item').forEach(item => {
    item.addEventListener('click', () => {
      const noteId = parseInt(item.dataset.noteId);
      if (!noteId) return;
      currentNoteId = noteId;
      notesViewMode = 'edit';
      const el = document.getElementById('notes-content');
      if (el) renderNoteEditor(el, noteId);
    });
  });
}

// ── Table View ──
function renderTableView(container, notes) {
  const sortedNotes = [...notes].sort((a, b) => {
    const col = notesTableSort.col;
    const dir = notesTableSort.dir === 'asc' ? 1 : -1;
    const av = a[col] || '';
    const bv = b[col] || '';
    if (av < bv) return -1 * dir;
    if (av > bv) return 1 * dir;
    return 0;
  });

  const sortIcon = (col) => {
    if (notesTableSort.col !== col) return '';
    return notesTableSort.dir === 'asc' ? ' &uarr;' : ' &darr;';
  };

  const statusPill = (status) => {
    if (status === 'done') return '<span class="table-status-pill table-status-done">Готово</span>';
    if (status === 'task') return '<span class="table-status-pill table-status-task">Задача</span>';
    return '<span class="table-status-pill table-status-note">Заметка</span>';
  };

  const rows = sortedNotes.map(n => {
    const tagsHtml = (n.tags || '').split(',').map(t => t.trim()).filter(Boolean)
      .map(t => `<span class="note-tag badge-${tagColorMap[t] || 'blue'}">${escapeHtml(t)}</span>`).join('');
    const dueHtml = n.due_date ? formatDueDate(n.due_date) : '<span class="text-faint">—</span>';
    return `<tr class="notes-table-row" data-note-id="${n.id}">
      <td class="notes-table-title">${n.pinned ? '📌 ' : ''}${escapeHtml(n.title || 'Без названия')}</td>
      <td>${statusPill(n.status)}</td>
      <td>${dueHtml}</td>
      <td class="notes-table-tags">${tagsHtml || '<span class="text-faint">—</span>'}</td>
      <td class="text-faint">${formatNoteDate(n.updated_at || n.created_at)}</td>
    </tr>`;
  }).join('');

  container.innerHTML = `<div class="notes-table-wrap">
    <table class="notes-table">
      <thead>
        <tr>
          <th class="notes-table-sortable" data-sort="title">Название${sortIcon('title')}</th>
          <th class="notes-table-sortable" data-sort="status">Статус${sortIcon('status')}</th>
          <th class="notes-table-sortable" data-sort="due_date">Дата${sortIcon('due_date')}</th>
          <th>Теги</th>
          <th class="notes-table-sortable" data-sort="updated_at">Обновлено${sortIcon('updated_at')}</th>
        </tr>
      </thead>
      <tbody>${rows || '<tr><td colspan="5"><div class="empty-state"><div class="empty-state-icon">📋</div><div class="empty-state-text">Нет заметок</div></div></td></tr>'}</tbody>
    </table>
  </div>`;

  // Sort clicks
  container.querySelectorAll('.notes-table-sortable').forEach(th => {
    th.addEventListener('click', () => {
      const col = th.dataset.sort;
      if (notesTableSort.col === col) {
        notesTableSort.dir = notesTableSort.dir === 'asc' ? 'desc' : 'asc';
      } else {
        notesTableSort.col = col;
        notesTableSort.dir = 'asc';
      }
      renderNotesPage();
    });
  });

  // Row clicks
  container.querySelectorAll('.notes-table-row').forEach(row => {
    row.addEventListener('click', () => {
      const noteId = parseInt(row.dataset.noteId);
      if (!noteId) return;
      currentNoteId = noteId;
      notesViewMode = 'edit';
      const el = document.getElementById('notes-content');
      if (el) renderNoteEditor(el, noteId);
    });
  });
}

// ── Gallery View ──
function renderGalleryView(container, notes) {
  if (notes.length === 0) {
    container.innerHTML = `<div class="empty-state">
      <div class="empty-state-icon">🖼</div>
      <div class="empty-state-text">Нет заметок</div>
      <button class="btn-primary" id="empty-new-note-btn">Создать первую</button>
    </div>`;
    document.getElementById('empty-new-note-btn')?.addEventListener('click', createAndOpenNote);
    return;
  }

  const cards = notes.map(n => {
    const preview = (n.content || '').substring(0, 200).replace(/\n/g, ' ');
    const statusIcon = n.status === 'done' ? '☑ ' : n.status === 'task' ? '☐ ' : '';
    const tagsHtml = (n.tags || '').split(',').map(t => t.trim()).filter(Boolean)
      .map(t => `<span class="note-tag badge-${tagColorMap[t] || 'blue'}">${escapeHtml(t)}</span>`).join('');
    const dueHtml = (n.status === 'task' || n.status === 'done') && n.due_date ? `<span class="gallery-card-due">${formatDueDate(n.due_date)}</span>` : '';
    return `<div class="gallery-card card${n.status === 'done' ? ' task-done' : ''}" data-note-id="${n.id}">
      <div class="gallery-card-header">
        <span class="gallery-card-title">${statusIcon}${n.pinned ? '📌 ' : ''}${escapeHtml(n.title || 'Без названия')}</span>
      </div>
      ${preview ? `<div class="gallery-card-preview">${escapeHtml(preview)}</div>` : ''}
      <div class="gallery-card-footer">
        ${tagsHtml ? `<div class="gallery-card-tags">${tagsHtml}</div>` : ''}
        <div class="gallery-card-meta">
          <span class="text-faint">${formatNoteDate(n.updated_at || n.created_at)}</span>
          ${dueHtml}
        </div>
      </div>
    </div>`;
  }).join('');

  container.innerHTML = `<div class="gallery-grid">${cards}</div>`;

  container.querySelectorAll('.gallery-card').forEach(card => {
    card.addEventListener('click', () => {
      const noteId = parseInt(card.dataset.noteId);
      if (!noteId) return;
      currentNoteId = noteId;
      notesViewMode = 'edit';
      const el = document.getElementById('notes-content');
      if (el) renderNoteEditor(el, noteId);
    });
  });
}

async function createAndOpenTask() {
  try {
    const id = await invoke('create_note', { title: '', content: '', tags: '', status: 'task', tabName: null, dueDate: null, reminderAt: null });
    currentNoteId = id;
    notesViewMode = 'edit';
    const el = document.getElementById('notes-content');
    if (el) renderNoteEditor(el, id);
  } catch (err) { console.error('create_task error:', err); }
}

function createNoteCard(note, onRefresh) {
  const card = document.createElement('div');
  const isTask = note.status === 'task' || note.status === 'done';
  card.className = `note-card card${note.status === 'done' ? ' task-done' : ''}`;
  card.draggable = true;
  card.dataset.noteId = note.id;
  const preview = (note.content || '').substring(0, 120).replace(/\n/g, ' ');
  const tagsHtml = renderNoteTags(note.tags);
  const dueHtml = isTask ? formatDueDate(note.due_date) : '';
  const tabBadge = note.tab_name ? `<span class="note-tab-badge">${escapeHtml(note.tab_name)}</span>` : '';
  const statusIcon = note.status === 'done' ? '☑' : note.status === 'task' ? '☐' : '';

  card.innerHTML = `
    <div class="note-card-body">
      <div class="note-card-title">${statusIcon ? `<span class="note-status-icon" data-action="toggle-status">${statusIcon}</span> ` : ''}${note.pinned ? '<span class="note-pinned-icon">📌</span> ' : ''}${escapeHtml(note.title || 'Без названия')}</div>
      ${preview ? `<div class="note-card-preview">${escapeHtml(preview)}</div>` : ''}
      <div class="note-card-meta">
        <span>${formatNoteDate(note.updated_at || note.created_at)}</span>
        ${dueHtml ? `<span class="note-card-due">${dueHtml}</span>` : ''}
        ${tabBadge}
      </div>
      ${tagsHtml ? `<div class="note-card-tags">${tagsHtml}</div>` : ''}
    </div>
    <div class="note-card-actions">
      <button class="note-card-action-btn" data-action="pin" title="${note.pinned ? 'Открепить' : 'Закрепить'}">📌</button>
      <button class="note-card-action-btn" data-action="archive" title="${note.archived ? 'Разархивировать' : 'В архив'}">📦</button>
      <button class="note-card-action-btn note-card-action-danger" data-action="delete" title="Удалить">🗑</button>
    </div>`;

  // Click card body to open editor
  card.querySelector('.note-card-body').addEventListener('click', (e) => {
    if (e.target.closest('[data-action="toggle-status"]')) return;
    currentNoteId = note.id;
    notesViewMode = 'edit';
    const el = document.getElementById('notes-content');
    if (el) renderNoteEditor(el, note.id);
  });

  // Toggle task status
  card.querySelector('[data-action="toggle-status"]')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const nextStatus = note.status === 'done' ? 'task' : note.status === 'task' ? 'done' : 'task';
    await invoke('update_note_status', { id: note.id, status: nextStatus }).catch(err => console.error('status:', err));
    if (onRefresh) onRefresh();
  });

  // Pin
  card.querySelector('[data-action="pin"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    await invoke('toggle_note_pin', { id: note.id }).catch(err => console.error('pin:', err));
    if (onRefresh) onRefresh();
  });
  // Archive
  card.querySelector('[data-action="archive"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    await invoke('toggle_note_archive', { id: note.id }).catch(err => console.error('archive:', err));
    if (onRefresh) onRefresh();
  });
  // Delete
  card.querySelector('[data-action="delete"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    if (!(await confirmModal('Удалить заметку?'))) return;
    await invoke('delete_note', { id: note.id }).catch(err => console.error('delete:', err));
    if (onRefresh) onRefresh();
  });

  // DnD events
  card.addEventListener('dragstart', (e) => {
    card.classList.add('dragging');
    e.dataTransfer.setData('text/plain', note.id);
    e.dataTransfer.effectAllowed = 'move';
  });
  card.addEventListener('dragend', () => card.classList.remove('dragging'));

  return card;
}

function setupNotesDnD(list) {
  let dragOverCard = null;
  list.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    const card = e.target.closest('.note-card');
    if (card && card !== dragOverCard) {
      dragOverCard?.classList.remove('drag-over');
      card.classList.add('drag-over');
      dragOverCard = card;
    }
  });
  list.addEventListener('dragleave', (e) => {
    if (!list.contains(e.relatedTarget)) {
      dragOverCard?.classList.remove('drag-over');
      dragOverCard = null;
    }
  });
  list.addEventListener('drop', async (e) => {
    e.preventDefault();
    dragOverCard?.classList.remove('drag-over');
    const draggedId = parseInt(e.dataTransfer.getData('text/plain'));
    const targetCard = e.target.closest('.note-card');
    if (!targetCard) return;
    const targetId = parseInt(targetCard.dataset.noteId);
    if (draggedId === targetId) return;

    // Reorder DOM
    const cards = [...list.querySelectorAll('.note-card')];
    const ids = cards.map(c => parseInt(c.dataset.noteId));
    const fromIdx = ids.indexOf(draggedId);
    const toIdx = ids.indexOf(targetId);
    if (fromIdx < 0 || toIdx < 0) return;
    ids.splice(fromIdx, 1);
    ids.splice(toIdx, 0, draggedId);
    await invoke('reorder_notes', { ids }).catch(err => console.error('reorder:', err));
    loadNotes();
  });
}

async function createAndOpenNote() {
  try {
    const id = await invoke('create_note', { title: '', content: '', tags: '', tabName: null, status: null, dueDate: null, reminderAt: null });
    currentNoteId = id;
    notesViewMode = 'edit';
    const el = document.getElementById('notes-content');
    if (el) renderNoteEditor(el, id);
  } catch (err) { console.error('create_note error:', err); }
}

function saveCurrentNote(id) {
  clearTimeout(noteAutoSaveTimeout);
  const title = document.getElementById('note-title')?.value || '';
  const content = document.getElementById('note-body')?.value || '';
  const tags = document.getElementById('note-tags-input')?.value || '';
  const tabName = document.getElementById('note-tab-select')?.value || null;
  const status = document.getElementById('note-status-select')?.value || null;
  const dueDate = document.getElementById('note-due-date')?.value || null;
  const reminderAt = document.getElementById('note-reminder')?.value || null;
  return invoke('update_note', { id, title, content, tags, pinned: null, archived: null, tabName, status, dueDate, reminderAt });
}

async function renderNoteEditor(el, id) {
  try {
    const note = await invoke('get_note', { id });
    notePreviewMode = false;

    // Build tab options
    const tabKeys = Object.keys(TAB_REGISTRY).filter(k => k !== 'chat' && !k.startsWith('page_'));
    const tabOptions = tabKeys.map(k => `<option value="${k}" ${note.tab_name === k ? 'selected' : ''}>${TAB_REGISTRY[k].label}</option>`).join('');

    // Parse tags for pills
    const tags = (note.tags || '').split(',').map(t => t.trim()).filter(Boolean);
    const tagPillsHtml = tags.map(t => {
      const color = tagColorMap[t] || 'blue';
      return `<span class="note-tag-pill badge-${color}" data-tag="${escapeHtml(t)}">${escapeHtml(t)} <span class="note-tag-remove">×</span></span>`;
    }).join('');

    el.innerHTML = `<div class="page-content note-edit-view">
      <div class="note-edit-topbar">
        <div class="note-breadcrumb" id="note-back-btn">← Notes</div>
        <div class="note-edit-actions">
          <button class="note-action-btn" id="note-preview-btn" title="Markdown">👁</button>
          <button class="note-action-btn ${note.pinned ? 'active' : ''}" id="note-pin-btn" title="${note.pinned ? 'Открепить' : 'Закрепить'}">📌</button>
          <button class="note-action-btn" id="note-archive-btn" title="${note.archived ? 'Разархивировать' : 'В архив'}">📦</button>
          <button class="note-action-btn note-action-btn-danger" id="note-delete-btn" title="Удалить">🗑</button>
        </div>
      </div>

      <div class="note-task-bar">
        <select class="form-select note-status-select" id="note-status-select">
          <option value="note" ${note.status === 'note' ? 'selected' : ''}>Заметка</option>
          <option value="task" ${note.status === 'task' ? 'selected' : ''}>Задача</option>
          <option value="done" ${note.status === 'done' ? 'selected' : ''}>Выполнено</option>
        </select>
        <input type="date" class="form-input note-due-input" id="note-due-date" value="${note.due_date || ''}" placeholder="Дедлайн">
        <input type="datetime-local" class="form-input note-reminder-input" id="note-reminder" value="${note.reminder_at || ''}" placeholder="Напомнить">
        <select class="form-select note-tab-select" id="note-tab-select">
          <option value="">— Без таба —</option>
          ${tabOptions}
        </select>
      </div>

      <input class="page-title-input" id="note-title" value="${escapeHtml(note.title || '')}" placeholder="Без названия">

      <div class="note-tags-row" id="note-tags-row">
        ${tagPillsHtml}
        <input class="note-tag-input" id="note-tag-add" placeholder="+ тег" autocomplete="off">
      </div>

      <textarea class="custom-page-body" id="note-body" placeholder="Начните писать...">${escapeHtml(note.content || '')}</textarea>
      <div class="note-preview markdown-body" id="note-preview" style="display:none"></div>
      <input type="hidden" id="note-tags-input" value="${escapeHtml(note.tags || '')}">
    </div>`;

    // Auto-resize textarea
    const body = document.getElementById('note-body');
    if (body) {
      const resize = () => { body.style.height = 'auto'; body.style.height = Math.max(200, body.scrollHeight) + 'px'; };
      body.addEventListener('input', resize);
      setTimeout(resize, 0);
    }

    if (!note.title) document.getElementById('note-title')?.focus();
    else document.getElementById('note-body')?.focus();

    // Auto-save on typing
    const autoSave = () => {
      clearTimeout(noteAutoSaveTimeout);
      noteAutoSaveTimeout = setTimeout(() => {
        saveCurrentNote(id).catch(e => console.error('note autosave error:', e));
      }, 800);
    };
    document.getElementById('note-title')?.addEventListener('input', autoSave);
    document.getElementById('note-body')?.addEventListener('input', autoSave);
    document.getElementById('note-status-select')?.addEventListener('change', autoSave);
    document.getElementById('note-due-date')?.addEventListener('change', autoSave);
    document.getElementById('note-reminder')?.addEventListener('change', autoSave);
    document.getElementById('note-tab-select')?.addEventListener('change', autoSave);

    // Markdown preview toggle
    document.getElementById('note-preview-btn')?.addEventListener('click', () => {
      notePreviewMode = !notePreviewMode;
      const ta = document.getElementById('note-body');
      const pv = document.getElementById('note-preview');
      const btn = document.getElementById('note-preview-btn');
      if (notePreviewMode) {
        pv.innerHTML = renderMarkdown(ta.value || '');
        ta.style.display = 'none';
        pv.style.display = 'block';
        btn.classList.add('active');
      } else {
        ta.style.display = '';
        pv.style.display = 'none';
        btn.classList.remove('active');
      }
    });

    // Tag input
    const tagInput = document.getElementById('note-tag-add');
    const tagsRow = document.getElementById('note-tags-row');
    tagInput?.addEventListener('keydown', async (e) => {
      if (e.key === 'Enter' || e.key === ',') {
        e.preventDefault();
        const val = tagInput.value.trim().replace(/,/g, '');
        if (!val) return;
        const hidden = document.getElementById('note-tags-input');
        const curTags = (hidden.value || '').split(',').map(t => t.trim()).filter(Boolean);
        if (!curTags.includes(val)) {
          curTags.push(val);
          hidden.value = curTags.join(', ');
          // Ensure tag has a color
          if (!tagColorMap[val]) {
            const colors = ['blue','green','purple','orange','yellow','red','pink'];
            tagColorMap[val] = colors[curTags.length % colors.length];
            invoke('set_note_tag_color', { name: val, color: tagColorMap[val] }).catch(() => {});
          }
          const pill = document.createElement('span');
          pill.className = `note-tag-pill badge-${tagColorMap[val]}`;
          pill.dataset.tag = val;
          pill.innerHTML = `${escapeHtml(val)} <span class="note-tag-remove">×</span>`;
          tagsRow.insertBefore(pill, tagInput);
        }
        tagInput.value = '';
        autoSave();
      }
    });

    // Remove tag on click ×
    tagsRow?.addEventListener('click', (e) => {
      const rm = e.target.closest('.note-tag-remove');
      if (!rm) return;
      const pill = rm.closest('.note-tag-pill');
      const tag = pill?.dataset.tag;
      if (!tag) return;
      pill.remove();
      const hidden = document.getElementById('note-tags-input');
      hidden.value = (hidden.value || '').split(',').map(t => t.trim()).filter(t => t !== tag).join(', ');
      autoSave();
    });

    // Back
    document.getElementById('note-back-btn')?.addEventListener('click', () => {
      saveCurrentNote(id).catch(() => {});
      currentNoteId = null;
      notesViewMode = 'list';
      loadNotes();
    });

    // Pin
    document.getElementById('note-pin-btn')?.addEventListener('click', async () => {
      try {
        await saveCurrentNote(id);
        await invoke('toggle_note_pin', { id });
        renderNoteEditor(el, id);
      } catch (err) { console.error('pin error:', err); }
    });

    // Archive
    document.getElementById('note-archive-btn')?.addEventListener('click', async () => {
      try {
        await saveCurrentNote(id);
        await invoke('toggle_note_archive', { id });
        currentNoteId = null;
        notesViewMode = 'list';
        loadNotes();
      } catch (err) { console.error('archive error:', err); }
    });

    // Delete
    document.getElementById('note-delete-btn')?.addEventListener('click', async () => {
      if (!(await confirmModal('Удалить заметку?'))) return;
      try {
        await invoke('delete_note', { id });
        currentNoteId = null;
        notesViewMode = 'list';
        loadNotes();
      } catch (err) { console.error('delete error:', err); }
    });

  } catch (e) {
    console.error('renderNoteEditor error:', e);
    el.innerHTML = `<div class="empty-state"><div class="empty-state-icon">📝</div><div class="empty-state-text">Заметка не найдена</div></div>`;
  }
}

async function renderLinkedNotes(container, tabName) {
  try {
    const notes = await invoke('get_notes_for_tab', { tabName });
    if (!notes || notes.length === 0) return;
    const section = document.createElement('div');
    section.className = 'linked-notes-section';
    section.innerHTML = `<div class="linked-notes-header"><span>📝 Связанные заметки</span><span class="linked-notes-count">${notes.length}</span></div>`;
    const list = document.createElement('div');
    list.className = 'linked-notes-list';
    for (const n of notes.slice(0, 10)) {
      const item = document.createElement('div');
      item.className = `linked-note-item${n.status === 'done' ? ' task-done' : ''}`;
      const statusIcon = n.status === 'done' ? '☑ ' : n.status === 'task' ? '☐ ' : '';
      item.innerHTML = `<span class="linked-note-title">${statusIcon}${escapeHtml(n.title || 'Без названия')}</span>
        <span class="linked-note-date">${formatNoteDate(n.updated_at)}</span>`;
      item.addEventListener('click', () => {
        activateTab('notes');
        setTimeout(() => {
          currentNoteId = n.id;
          notesViewMode = 'edit';
          const el = document.getElementById('notes-content');
          if (el) renderNoteEditor(el, n.id);
        }, 100);
      });
      list.appendChild(item);
    }
    section.appendChild(list);
    container.appendChild(section);
  } catch (_) {}
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
    const tasks = await invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []);
    if (subTab === 'Неделя') {
      renderWeekCalendar(el, events || []);
    } else if (subTab === 'День') {
      renderDayCalendar(el, events || []);
    } else {
      renderCalendar(el, events || [], tasks || []);
    }
  } catch (e) {
    if (subTab === 'Неделя') renderWeekCalendar(el, []);
    else if (subTab === 'День') renderDayCalendar(el, []);
    else renderCalendar(el, []);
  }
}

function renderCalendar(el, events, tasks) {
  tasks = tasks || [];
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

  // Group tasks by due_date
  const tasksByDate = {};
  for (const t of tasks) {
    if (!t.due_date) continue;
    if (!tasksByDate[t.due_date]) tasksByDate[t.due_date] = [];
    tasksByDate[t.due_date].push(t);
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
    const dayTasks = tasksByDate[dateStr] || [];
    const dots = dayEvents.slice(0, 3).map(e => `<span class="calendar-event-dot" style="background:${e.color || 'var(--accent-blue)'}"></span>`).join('');
    const taskDots = dayTasks.slice(0, 2).map(t => `<span class="calendar-task-dot${t.status === 'done' ? ' done' : ''}"></span>`).join('');
    daysHtml += `<div class="calendar-day${isToday ? ' today' : ''}${isSelected ? ' selected' : ''}" data-date="${dateStr}">
      <span class="calendar-day-number">${d}</span>
      <div class="calendar-day-dots">${dots}${taskDots}</div>
    </div>`;
  }
  // Next month days
  const totalCells = startDay + lastDay.getDate();
  const remaining = totalCells % 7 === 0 ? 0 : 7 - (totalCells % 7);
  for (let i = 1; i <= remaining; i++) {
    daysHtml += `<div class="calendar-day other-month"><span class="calendar-day-number">${i}</span></div>`;
  }

  let dayPanelHtml = '';
  if (selectedCalendarDate) {
    const dayEvts = eventsByDate[selectedCalendarDate] || [];
    const dayTasks = tasksByDate[selectedCalendarDate] || [];
    const hasContent = dayEvts.length > 0 || dayTasks.length > 0;

    const eventsSection = dayEvts.length > 0 ? `
      <div class="calendar-day-panel-section">События</div>
      ${dayEvts.map(e => `<div class="calendar-event-item">
        <span class="calendar-event-time">${e.time || ''}</span>
        <span class="calendar-event-title">${escapeHtml(e.title)}</span>
        ${e.source && e.source !== 'manual' ? `<span class="badge badge-gray">${e.source === 'apple' ? '🍎' : '📅'}</span>` : ''}
      </div>`).join('')}` : '';

    const tasksSection = dayTasks.length > 0 ? `
      <div class="calendar-day-panel-section">Задачи</div>
      ${dayTasks.map(t => {
        const statusIcon = t.status === 'done' ? '☑' : '☐';
        const tagsHtml = (t.tags || '').split(',').map(tg => tg.trim()).filter(Boolean)
          .map(tg => `<span class="note-tag badge-${tagColorMap[tg] || 'blue'}">${escapeHtml(tg)}</span>`).join('');
        return `<div class="calendar-task-item" data-id="${t.id}">
          <span class="note-status-icon" data-action="toggle-task">${statusIcon}</span>
          <span class="calendar-task-title" data-action="open-task">${escapeHtml(t.title || 'Без названия')}</span>
          ${tagsHtml}
        </div>`;
      }).join('')}` : '';

    dayPanelHtml = `<div class="calendar-day-panel">
      <div class="calendar-day-panel-header">
        <div class="calendar-day-panel-title">${selectedCalendarDate}</div>
        <button class="btn-sm btn-secondary" id="cal-add-task-btn">+ Задача</button>
      </div>
      ${eventsSection}${tasksSection}
      ${!hasContent ? '<div class="calendar-day-panel-empty">Нет событий</div>' : ''}
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

  // Calendar task interactions
  document.querySelectorAll('.calendar-task-item').forEach(item => {
    item.querySelector('[data-action="toggle-task"]')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = parseInt(item.dataset.id);
      const task = tasks.find(t => t.id === id);
      if (!task) return;
      const nextStatus = task.status === 'done' ? 'task' : 'done';
      await invoke('update_note_status', { id, status: nextStatus }).catch(err => console.error('cal task toggle:', err));
      loadCalendar();
    });
    item.querySelector('[data-action="open-task"]')?.addEventListener('click', () => {
      const id = parseInt(item.dataset.id);
      activateTab('notes');
      setTimeout(() => {
        currentNoteId = id;
        notesViewMode = 'edit';
        const notesEl = document.getElementById('notes-content');
        if (notesEl) renderNoteEditor(notesEl, id);
      }, 100);
    });
  });

  document.getElementById('cal-add-task-btn')?.addEventListener('click', async () => {
    try {
      const id = await invoke('create_note', { title: '', content: '', tags: '', status: 'task', tabName: null, dueDate: selectedCalendarDate, reminderAt: null });
      activateTab('notes');
      setTimeout(() => {
        currentNoteId = id;
        notesViewMode = 'edit';
        const notesEl = document.getElementById('notes-content');
        if (notesEl) renderNoteEditor(notesEl, id);
      }, 100);
    } catch (err) { console.error('cal add task:', err); }
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
        color: '#9B9B9B',
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
  const dayEvents = events.filter(e => e.date === calDayDate).map(e => {
    // Normalize time to HH:MM (pad single-digit hour)
    if (e.time && /^\d:\d{2}$/.test(e.time)) e.time = '0' + e.time;
    return e;
  }).sort((a, b) => (a.time || '').localeCompare(b.time || ''));
  const d = new Date(calDayDate + 'T00:00:00');
  const dayNames = ['Воскресенье', 'Понедельник', 'Вторник', 'Среда', 'Четверг', 'Пятница', 'Суббота'];
  const monthNames = ['Января', 'Февраля', 'Марта', 'Апреля', 'Мая', 'Июня', 'Июля', 'Августа', 'Сентября', 'Октября', 'Ноября', 'Декабря'];

  const hours = Array.from({length: 17}, (_, i) => i + 6); // 6:00 - 22:00
  let timelineHtml = hours.map(h => {
    const timeStr = `${String(h).padStart(2,'0')}:`;
    const hourEvents = dayEvents.filter(e => e.time && e.time.startsWith(timeStr.slice(0,2)));
    const evtHtml = hourEvents.map(e => {
      const srcBadge = e.source && e.source !== 'manual' ? `<span class="badge badge-gray" style="margin-left:6px;">${e.source === 'apple' ? '🍎' : '📅'}</span>` : '';
      const endMin = (() => { const [hh,mm] = (e.time||'00:00').split(':').map(Number); const t = hh*60+mm+(e.duration_minutes||60); return `${String(Math.floor(t/60)%24).padStart(2,'0')}:${String(t%60).padStart(2,'0')}`; })();
      return `<div class="day-event" style="border-left:3px solid ${e.color || 'var(--text-secondary)'};">
        <span class="day-event-time">${e.time} – ${endMin}</span>
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
    <div class="day-hour-content">${allDay.map(e => `<div class="day-event" style="border-left:3px solid ${e.color || 'var(--text-secondary)'};"><span class="day-event-title">${escapeHtml(e.title)}</span></div>`).join('')}</div>
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
  const sourceColor = (s) => s === 'apple' ? '#4F9768' : s === 'google' ? '#447ACB' : 'var(--text-secondary)';

  let rowsHtml = '';
  if (events.length === 0) {
    rowsHtml = '<tr><td colspan="6" style="text-align:center;color:var(--text-muted);padding:24px;">Нет событий</td></tr>';
  } else {
    for (const ev of events) {
      const endTime = ev.time && ev.duration_minutes ? (() => {
        const [h, m] = ev.time.split(':').map(Number);
        const total = h * 60 + m + ev.duration_minutes;
        return `${String(Math.floor(total / 60) % 24).padStart(2,'0')}:${String(total % 60).padStart(2,'0')}`;
      })() : '';
      const timeRange = ev.time ? (endTime ? `${ev.time} – ${endTime}` : ev.time) : 'Весь день';
      rowsHtml += `<tr class="cal-list-row" data-id="${ev.id}">
        <td style="color:var(--text-primary);font-weight:500;">${escapeHtml(ev.title)}</td>
        <td>${ev.date}</td>
        <td>${timeRange}</td>
        <td>${ev.duration_minutes ? ev.duration_minutes + ' мин' : '—'}</td>
        <td><span style="color:${sourceColor(ev.source)};font-size:12px;">${sourceLabel(ev.source)}</span></td>
        <td style="color:var(--text-muted);font-size:12px;">${escapeHtml(ev.category || '')}</td>
      </tr>`;
    }
  }

  el.innerHTML = `
    <div class="calendar-nav">
      <button class="calendar-nav-btn" id="list-prev">&lt;</button>
      <div class="calendar-month-label">${monthNames[calendarMonth]} ${calendarYear}</div>
      <button class="calendar-nav-btn" id="list-next">&gt;</button>
      <button class="btn-primary" id="list-add-event" style="margin-left:16px;">+ Событие</button>
      <span style="color:var(--text-muted);font-size:12px;margin-left:auto;">${events.length} событий</span>
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
  el.innerHTML = skeletonPage();
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
          <span class="settings-label" style="color:var(--text-faint);font-size:12px;">Включает все календари добавленные в macOS (iCloud, Google, Exchange и др.)</span>
        </div>
        <div class="settings-row">
          <button class="btn-primary" id="calint-sync-apple">Синхронизировать сейчас</button>
          <span class="settings-value" id="calint-apple-status">—</span>
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-section-title">Google Calendar (ICS)</div>
        <div class="settings-row">
          <span class="settings-label" style="color:var(--text-faint);font-size:12px;">Приватный ICS URL: Google Calendar → Настройки → Настройки календаря → Секретный адрес в формате iCal</span>
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
          if (status) { status.textContent = '✗ ' + r.error; status.style.color = 'var(--color-red)'; }
        } else {
          if (status) { status.textContent = `✓ ${r.synced} событий`; status.style.color = ''; }
        }
      } catch (e) { if (status) { status.textContent = '✗ ' + e; status.style.color = 'var(--color-red)'; } }
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
    el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`;
  }
}

// ── Work ──
async function loadWork() {
  const el = document.getElementById('work-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('work') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  try {
    const projects = await invoke('get_projects').catch(() => []);
    renderWork(pc, projects || []);
  } catch (e) {
    pc.innerHTML = '<div class="tab-stub"><div class="tab-stub-icon">💼</div><div class="tab-stub-title">Работа</div><div class="tab-stub-desc">Проекты и задачи</div><span class="tab-stub-badge">Скоро</span></div>';
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
        <h2 style="font-size:16px;color:var(--text-primary);">${currentProjectId ? escapeHtml(projects.find(p => p.id === currentProjectId)?.name || '') : 'Выберите проект'}</h2>
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
    item.innerHTML = `<span class="work-project-dot" style="background:${p.color || 'var(--accent-blue)'}"></span>
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
    if (name) invoke('create_project', { name, description: '', color: '#9B9B9B' }).then(() => loadWork()).catch(e => alert(e));
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
  el.innerHTML = renderPageHeader('development') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  try {
    const items = await invoke('get_learning_items', { typeFilter: devFilter === 'all' ? null : devFilter }).catch(() => []);
    renderDevelopment(pc, items || []);
  } catch (e) {
    pc.innerHTML = '<div class="tab-stub"><div class="tab-stub-icon">🚀</div><div class="tab-stub-title">Развитие</div><div class="tab-stub-desc">Обучение и саморазвитие</div><span class="tab-stub-badge">Скоро</span></div>';
  }
}

function renderDevelopment(el, items) {
  const filters = ['all', 'course', 'book', 'skill', 'article'];
  const filterLabels = { all: 'Все', course: 'Курсы', book: 'Книги', skill: 'Навыки', article: 'Статьи' };
  const statusLabels = { planned: 'Запланировано', in_progress: 'В процессе', completed: 'Завершено' };
  const statusColors = { planned: 'badge-gray', in_progress: 'badge-blue', completed: 'badge-green' };

  const filterBar = `<div class="dev-filters">
    ${filters.map(f => `<button class="pill${devFilter === f ? ' active' : ''}" data-filter="${f}">${filterLabels[f]}</button>`).join('')}
  </div>`;

  const fixedColumns = [
    { key: 'title', label: 'Title', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
    { key: 'type', label: 'Type', render: r => `<span class="badge badge-purple">${filterLabels[r.type] || r.type}</span>` },
    { key: 'status', label: 'Status', render: r => `<span class="badge ${statusColors[r.status] || 'badge-gray'}">${statusLabels[r.status] || r.status}</span>` },
    { key: 'progress', label: 'Progress', render: r => `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${r.progress || 0}%"></div></div> <span style="font-size:11px;color:var(--text-faint);">${r.progress || 0}%</span>` },
  ];

  el.innerHTML = filterBar + '<div id="dev-dbv"></div>';
  const dbvEl = document.getElementById('dev-dbv');

  renderDatabaseView(dbvEl, 'development', 'learning_items', items, {
    fixedColumns,
    idField: 'id',
    addButton: '+ Добавить',
    onAdd: () => showAddLearningModal(),
    reloadFn: () => loadDevelopment(),
    _tabId: 'development',
    _recordTable: 'learning_items',
  });

  el.querySelectorAll('.dev-filters .pill').forEach(btn => {
    btn.addEventListener('click', () => { devFilter = btn.dataset.filter; loadDevelopment(); });
  });
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

// ── Database View System (Notion-style) ──

async function renderDatabaseView(el, tabId, recordTable, records, options = {}) {
  const { fixedColumns = [], idField = 'id', onRowClick, addButton, reloadFn } = options;

  // Load custom property definitions
  let customProps = [];
  try { customProps = await invoke('get_property_definitions', { tabId }); } catch {}

  // Load property values for all records
  const recordIds = records.map(r => r[idField]);
  let allValues = [];
  if (recordIds.length > 0 && customProps.length > 0) {
    try { allValues = await invoke('get_property_values', { recordTable, recordIds }); } catch {}
  }

  // Build values map: { recordId: { propertyId: value } }
  const valuesMap = {};
  for (const v of allValues) {
    if (!valuesMap[v.record_id]) valuesMap[v.record_id] = {};
    valuesMap[v.record_id][v.property_id] = v.value;
  }

  // Load and apply filters
  if (!dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filteredRecords = applyFilters(records, valuesMap, dbvFilters[tabId], idField);

  const visibleProps = customProps.filter(p => p.visible !== false);

  // Render header
  const headerHtml = `<div class="database-view-header">
    ${addButton ? `<button class="btn-primary" id="dbv-add-btn">${addButton}</button>` : ''}
  </div>`;

  // Render table
  const thFixed = fixedColumns.map(c => `<th class="sortable-header" data-sort="${c.key}">${c.label}</th>`).join('');
  const thCustom = visibleProps.map(p =>
    `<th class="sortable-header prop-header" data-sort="prop_${p.id}" data-prop-id="${p.id}"><span class="col-type-icon">${getTypeIcon(p.type)}</span>${escapeHtml(p.name)}</th>`
  ).join('');
  const thAddCol = `<th class="add-prop-col" id="dbv-add-prop-col" title="Добавить свойство">+</th>`;

  let tbodyHtml = '';
  for (const record of filteredRecords) {
    const rid = record[idField];
    const tdFixed = fixedColumns.map(c => {
      const val = c.render ? c.render(record) : escapeHtml(String(record[c.key] ?? ''));
      return `<td>${val}</td>`;
    }).join('');

    const tdCustom = visibleProps.map(p => {
      const rawVal = valuesMap[rid]?.[p.id] ?? '';
      const displayVal = formatPropValue(rawVal, p);
      return `<td class="cell-editable" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}'>${displayVal}</td>`;
    }).join('');

    tbodyHtml += `<tr class="data-table-row" data-id="${rid}">${tdFixed}${tdCustom}<td></td></tr>`;
  }

  if (filteredRecords.length === 0) {
    const colspan = fixedColumns.length + visibleProps.length + 1;
    tbodyHtml = `<tr><td colspan="${colspan}" style="text-align:center;color:var(--text-faint);padding:24px;">Пока пусто</td></tr>`;
  }

  el.innerHTML = headerHtml + `
    <table class="data-table database-view">
      <thead><tr>${thFixed}${thCustom}${thAddCol}</tr></thead>
      <tbody>${tbodyHtml}</tbody>
    </table>`;

  // Render filter bar if custom props exist
  if (customProps.length > 0) {
    renderFilterBar(el, tabId, customProps, reloadFn || (() => {}));
  }

  // Bind row click
  if (onRowClick) {
    el.querySelectorAll('.data-table-row').forEach(row => {
      row.style.cursor = 'pointer';
      row.addEventListener('click', (e) => {
        if (e.target.closest('.cell-editable') && e.target.closest('.inline-editor')) return;
        const id = parseInt(row.dataset.id);
        const record = filteredRecords.find(r => r[idField] === id);
        if (record) onRowClick(record);
      });
    });
  }

  // Bind inline editing
  el.querySelectorAll('.cell-editable').forEach(cell => {
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      startInlineEdit(cell, recordTable, reloadFn);
    });
  });

  // Bind + column header to add property
  el.querySelector('#dbv-add-prop-col')?.addEventListener('click', () => {
    showAddPropertyModal(tabId, reloadFn);
  });

  // Bind custom property header clicks to context menu
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      e.stopPropagation();
      const propId = parseInt(th.dataset.propId);
      const prop = customProps.find(p => p.id === propId);
      if (!prop) return;
      const rect = th.getBoundingClientRect();
      showColumnMenu(prop, rect, tabId, recordTable, reloadFn, el, records, allValues, fixedColumns, visibleProps, valuesMap, idField, options);
    });
  });

  // Bind add button
  if (addButton && options.onAdd) {
    document.getElementById('dbv-add-btn')?.addEventListener('click', options.onAdd);
  }

  // Bind sortable headers (fixed columns only — custom props use context menu)
  el.querySelectorAll('.sortable-header:not(.prop-header)').forEach(th => {
    th.addEventListener('click', () => {
      const sortKey = th.dataset.sort;
      const currentDir = th.dataset.dir || 'none';
      const newDir = currentDir === 'asc' ? 'desc' : 'asc';
      el.querySelectorAll('.sortable-header').forEach(h => { h.dataset.dir = 'none'; h.classList.remove('sort-asc', 'sort-desc'); });
      th.dataset.dir = newDir;
      th.classList.add(`sort-${newDir}`);
      sortDatabaseView(el, records, allValues, sortKey, newDir, fixedColumns, visibleProps, valuesMap, idField, options);
    });
  });
}

function formatPropValue(val, prop) {
  if (!val && val !== 0) return '<span class="text-faint">—</span>';
  switch (prop.type) {
    case 'checkbox': return val === 'true' ? '✓' : '—';
    case 'select': return `<span class="badge badge-blue">${escapeHtml(val)}</span>`;
    case 'multi_select': {
      try {
        const items = JSON.parse(val);
        return items.map(i => `<span class="badge badge-purple">${escapeHtml(i)}</span>`).join(' ');
      } catch { return escapeHtml(val); }
    }
    case 'url': return `<a href="${escapeHtml(val)}" target="_blank" style="color:var(--accent-blue);text-decoration:none;">${escapeHtml(val.substring(0, 30))}</a>`;
    case 'number': return escapeHtml(val);
    case 'date': return escapeHtml(val);
    default: return escapeHtml(val);
  }
}

function startInlineEdit(cell, recordTable, reloadFn) {
  if (cell.querySelector('.inline-editor')) return;
  const recordId = parseInt(cell.dataset.recordId);
  const propId = parseInt(cell.dataset.propId);
  const propType = cell.dataset.propType;
  let options = [];
  try { options = JSON.parse(cell.dataset.propOptions || '[]'); } catch {}

  const currentVal = cell.textContent.trim();
  const originalHtml = cell.innerHTML;

  let editorHtml = '';
  switch (propType) {
    case 'select':
      editorHtml = `<select class="inline-editor inline-select">
        <option value="">—</option>
        ${options.map(o => `<option value="${escapeHtml(o)}"${o === currentVal ? ' selected' : ''}>${escapeHtml(o)}</option>`).join('')}
      </select>`;
      break;
    case 'multi_select': {
      let selected = [];
      try { selected = JSON.parse(cell.dataset.currentValue || '[]'); } catch {}
      editorHtml = `<div class="inline-editor inline-multi-select">
        ${options.map(o => `<label class="inline-ms-option"><input type="checkbox" value="${escapeHtml(o)}"${selected.includes(o) ? ' checked' : ''}> ${escapeHtml(o)}</label>`).join('')}
        <button class="btn-primary inline-ms-done" style="font-size:11px;padding:2px 8px;margin-top:4px;">OK</button>
      </div>`;
      break;
    }
    case 'checkbox':
      // Toggle immediately
      const newVal = currentVal === '✓' ? 'false' : 'true';
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: newVal })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
      return;
    case 'date':
      editorHtml = `<input type="date" class="inline-editor inline-date" value="${currentVal === '—' ? '' : currentVal}">`;
      break;
    case 'number':
      editorHtml = `<input type="number" class="inline-editor inline-number" value="${currentVal === '—' ? '' : currentVal}">`;
      break;
    default:
      editorHtml = `<input type="text" class="inline-editor inline-text" value="${currentVal === '—' ? '' : escapeHtml(currentVal)}">`;
  }

  cell.innerHTML = editorHtml;
  const editor = cell.querySelector('.inline-editor');
  if (editor.tagName === 'INPUT' || editor.tagName === 'SELECT') {
    editor.focus();
    const saveAndClose = () => {
      const val = editor.value || null;
      cell.innerHTML = originalHtml;
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    };
    editor.addEventListener('blur', saveAndClose);
    editor.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); editor.blur(); }
      if (e.key === 'Escape') { cell.innerHTML = originalHtml; }
    });
  } else if (propType === 'multi_select') {
    cell.querySelector('.inline-ms-done')?.addEventListener('click', () => {
      const checked = [...cell.querySelectorAll('input:checked')].map(cb => cb.value);
      const val = checked.length > 0 ? JSON.stringify(checked) : null;
      cell.innerHTML = originalHtml;
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    });
  }
}

const PROPERTY_TYPE_DEFS = [
  { id: 'text', icon: 'Aa', name: 'Текст' },
  { id: 'number', icon: '#', name: 'Число' },
  { id: 'select', icon: '◉', name: 'Выбор' },
  { id: 'multi_select', icon: '☰', name: 'Мульти-выбор' },
  { id: 'date', icon: '◫', name: 'Дата' },
  { id: 'checkbox', icon: '☑', name: 'Чекбокс' },
  { id: 'url', icon: '↗', name: 'Ссылка' },
];

function getTypeIcon(typeId) {
  const t = PROPERTY_TYPE_DEFS.find(d => d.id === typeId);
  return t ? t.icon : 'Aa';
}

function getTypeName(typeId) {
  const t = PROPERTY_TYPE_DEFS.find(d => d.id === typeId);
  return t ? t.name : typeId;
}

function showAddPropertyModal(tabId, reloadFn) {
  let selectedType = 'text';
  let optionsList = [];

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';

  function renderModal() {
    const needsOptions = ['select', 'multi_select'].includes(selectedType);
    const typeGrid = PROPERTY_TYPE_DEFS.map(t =>
      `<div class="prop-type-card${t.id === selectedType ? ' selected' : ''}" data-type="${t.id}">
        <div class="prop-type-icon">${t.icon}</div>
        <div class="prop-type-name">${t.name}</div>
      </div>`
    ).join('');

    const optionsHtml = needsOptions ? `
      <div class="prop-config-section">
        <div class="prop-section-label">Варианты</div>
        <div class="prop-options-container">
          <div class="prop-options-tags" id="prop-tags">
            ${optionsList.map((o, i) => `<span class="prop-option-tag">${escapeHtml(o)}<span class="prop-option-tag-remove" data-idx="${i}">&times;</span></span>`).join('')}
          </div>
          <div class="prop-option-add">
            <input id="prop-option-input" type="text" placeholder="Новый вариант..." autocomplete="off">
            <button id="prop-option-add-btn">+</button>
          </div>
        </div>
      </div>` : '';

    overlay.innerHTML = `<div class="modal modal-property">
      <div class="modal-title">Новое свойство</div>
      <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="prop-name" placeholder="Без названия" autocomplete="off"></div>
      <div class="form-group">
        <label class="form-label">Тип</label>
        <div class="prop-type-grid">${typeGrid}</div>
      </div>
      ${optionsHtml}
      <div class="modal-actions">
        <button class="btn-secondary" id="prop-cancel">Отмена</button>
        <button class="btn-primary" id="prop-save">Добавить</button>
      </div>
    </div>`;

    // Bind type selection
    overlay.querySelectorAll('.prop-type-card').forEach(card => {
      card.addEventListener('click', () => {
        selectedType = card.dataset.type;
        const nameVal = document.getElementById('prop-name')?.value || '';
        renderModal();
        const nameInput = document.getElementById('prop-name');
        if (nameInput) nameInput.value = nameVal;
      });
    });

    // Bind option tag removal
    overlay.querySelectorAll('.prop-option-tag-remove').forEach(btn => {
      btn.addEventListener('click', () => {
        const idx = parseInt(btn.dataset.idx);
        optionsList.splice(idx, 1);
        const nameVal = document.getElementById('prop-name')?.value || '';
        renderModal();
        document.getElementById('prop-name').value = nameVal;
      });
    });

    // Bind add option
    const addOptBtn = overlay.querySelector('#prop-option-add-btn');
    const addOptInput = overlay.querySelector('#prop-option-input');
    const addOption = () => {
      const val = addOptInput?.value?.trim();
      if (val && !optionsList.includes(val)) {
        optionsList.push(val);
        const nameVal = document.getElementById('prop-name')?.value || '';
        renderModal();
        document.getElementById('prop-name').value = nameVal;
        document.getElementById('prop-option-input')?.focus();
      }
    };
    addOptBtn?.addEventListener('click', addOption);
    addOptInput?.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addOption(); } });

    // Bind cancel
    overlay.querySelector('#prop-cancel')?.addEventListener('click', () => overlay.remove());

    // Bind save
    overlay.querySelector('#prop-save')?.addEventListener('click', async () => {
      const name = document.getElementById('prop-name')?.value?.trim() || 'Без названия';
      let options = null;
      if (['select', 'multi_select'].includes(selectedType) && optionsList.length > 0) {
        options = JSON.stringify(optionsList);
      }
      try {
        await invoke('create_property_definition', { tabId, name, propType: selectedType, position: null, color: null, options, defaultValue: null });
        overlay.remove();
        if (reloadFn) reloadFn();
      } catch (err) { alert('Error: ' + err); }
    });
  }

  renderModal();
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  setTimeout(() => document.getElementById('prop-name')?.focus(), 50);
}

function showColumnMenu(propDef, anchorRect, tabId, recordTable, reloadFn, el, records, allValues, fixedColumns, visibleProps, valuesMap, idField, options) {
  // Remove any existing menu
  document.querySelectorAll('.col-context-menu').forEach(m => m.remove());

  const menu = document.createElement('div');
  menu.className = 'col-context-menu';

  const needsOptions = ['select', 'multi_select'].includes(propDef.type);

  menu.innerHTML = `
    <div class="col-menu-section">
      <input class="col-menu-name-input" value="${escapeHtml(propDef.name)}" id="col-rename-input" autocomplete="off">
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="type">
        <span class="col-menu-icon">${getTypeIcon(propDef.type)}</span>
        <span>${getTypeName(propDef.type)}</span>
      </div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="sort-asc">
        <span class="col-menu-icon">↑</span>
        <span>Сортировка А→Я</span>
      </div>
      <div class="col-menu-item" data-action="sort-desc">
        <span class="col-menu-icon">↓</span>
        <span>Сортировка Я→А</span>
      </div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="hide">
        <span class="col-menu-icon">◻</span>
        <span>Скрыть</span>
      </div>
      <div class="col-menu-item col-menu-item danger" data-action="delete">
        <span class="col-menu-icon">✕</span>
        <span>Удалить</span>
      </div>
    </div>
  `;

  // Position menu below the header
  menu.style.left = Math.min(anchorRect.left, window.innerWidth - 220) + 'px';
  menu.style.top = anchorRect.bottom + 4 + 'px';

  document.body.appendChild(menu);

  // Rename on Enter/blur
  const renameInput = menu.querySelector('#col-rename-input');
  const doRename = async () => {
    const newName = renameInput.value.trim();
    if (newName && newName !== propDef.name) {
      try {
        await invoke('update_property_definition', { id: propDef.id, name: newName, propType: null, position: null, color: null, options: null, visible: null });
        if (reloadFn) reloadFn();
      } catch {}
    }
  };
  renameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); doRename(); menu.remove(); }
    if (e.key === 'Escape') { menu.remove(); }
    e.stopPropagation();
  });
  renameInput.addEventListener('blur', () => { doRename(); });

  // Menu item clicks
  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      const action = item.dataset.action;
      switch (action) {
        case 'sort-asc':
        case 'sort-desc': {
          const dir = action === 'sort-asc' ? 'asc' : 'desc';
          const sortKey = `prop_${propDef.id}`;
          el.querySelectorAll('.sortable-header').forEach(h => { h.dataset.dir = 'none'; h.classList.remove('sort-asc', 'sort-desc'); });
          sortDatabaseView(el, records, allValues, sortKey, dir, fixedColumns, visibleProps, valuesMap, idField, options);
          menu.remove();
          break;
        }
        case 'hide':
          try {
            await invoke('update_property_definition', { id: propDef.id, name: null, propType: null, position: null, color: null, options: null, visible: false });
            if (reloadFn) reloadFn();
          } catch {}
          menu.remove();
          break;
        case 'delete':
          if (await confirmModal(`Удалить свойство "${propDef.name}"?`)) {
            try {
              await invoke('delete_property_definition', { id: propDef.id });
              if (reloadFn) reloadFn();
            } catch {}
          }
          menu.remove();
          break;
      }
    });
  });

  // Close on outside click
  const closeMenu = (e) => {
    if (!menu.contains(e.target)) {
      doRename();
      menu.remove();
      document.removeEventListener('mousedown', closeMenu);
    }
  };
  setTimeout(() => document.addEventListener('mousedown', closeMenu), 10);
}

function sortDatabaseView(el, records, allValues, sortKey, dir, fixedColumns, visibleProps, valuesMap, idField, options) {
  const sorted = [...records].sort((a, b) => {
    let va, vb;
    if (sortKey.startsWith('prop_')) {
      const pid = parseInt(sortKey.substring(5));
      va = valuesMap[a[idField]]?.[pid] ?? '';
      vb = valuesMap[b[idField]]?.[pid] ?? '';
    } else {
      va = a[sortKey] ?? '';
      vb = b[sortKey] ?? '';
    }
    if (typeof va === 'number' && typeof vb === 'number') return dir === 'asc' ? va - vb : vb - va;
    return dir === 'asc' ? String(va).localeCompare(String(vb)) : String(vb).localeCompare(String(va));
  });
  renderDatabaseView(el, options._tabId || '', options._recordTable || '', sorted, options);
}

// ── Filter System ──

// Active filters per tab: { tabId: [{ propId, condition, value }] }
const dbvFilters = {};

function renderFilterBar(el, tabId, customProps, onApply) {
  const filters = dbvFilters[tabId] || [];
  const chips = filters.map((f, idx) => {
    const prop = customProps.find(p => p.id === f.propId);
    const label = prop ? prop.name : '?';
    const condLabels = { eq: '=', neq: '\u2260', contains: '\u2248', empty: 'empty', not_empty: 'not empty' };
    return `<span class="filter-chip" data-idx="${idx}">
      ${escapeHtml(label)} ${condLabels[f.condition] || f.condition} ${f.value ? escapeHtml(f.value) : ''}
      <span class="filter-chip-remove" data-remove="${idx}">\u00d7</span>
    </span>`;
  }).join('');

  const bar = document.createElement('div');
  bar.className = 'filter-bar';
  bar.innerHTML = `<button class="btn-secondary" id="dbv-add-filter" style="font-size:11px;padding:4px 10px;">+ Filter</button>${chips}`;
  el.prepend(bar);

  bar.querySelector('#dbv-add-filter')?.addEventListener('click', () => {
    showFilterBuilderModal(tabId, customProps, onApply);
  });

  bar.querySelectorAll('.filter-chip-remove').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const idx = parseInt(btn.dataset.remove);
      if (dbvFilters[tabId]) dbvFilters[tabId].splice(idx, 1);
      saveFiltersToViewConfig(tabId);
      onApply();
    });
  });
}

function showFilterBuilderModal(tabId, customProps, onApply) {
  if (customProps.length === 0) { alert('Add custom properties first to filter by them.'); return; }
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Add Filter</div>
    <div class="form-group"><label class="form-label">Property</label>
      <select class="form-select" id="filter-prop" style="width:100%;">
        ${customProps.map(p => `<option value="${p.id}" data-type="${p.type}" data-options='${escapeHtml(p.options||"[]")}'>${escapeHtml(p.name)}</option>`).join('')}
      </select>
    </div>
    <div class="form-group"><label class="form-label">Condition</label>
      <select class="form-select" id="filter-cond" style="width:100%;">
        <option value="eq">Equals</option><option value="neq">Not equals</option>
        <option value="contains">Contains</option>
        <option value="empty">Is empty</option><option value="not_empty">Is not empty</option>
      </select>
    </div>
    <div class="form-group" id="filter-val-group"><label class="form-label">Value</label>
      <input class="form-input" id="filter-val" placeholder="Value">
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="filter-apply">Apply</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  // Update value input based on property type
  const updateValueInput = () => {
    const sel = document.getElementById('filter-prop');
    const opt = sel?.selectedOptions[0];
    const type = opt?.dataset.type;
    const cond = document.getElementById('filter-cond')?.value;
    const valGroup = document.getElementById('filter-val-group');

    if (cond === 'empty' || cond === 'not_empty') {
      valGroup.style.display = 'none';
      return;
    }
    valGroup.style.display = 'block';

    if (type === 'select' || type === 'multi_select') {
      let options = [];
      try { options = JSON.parse(opt?.dataset.options || '[]'); } catch {}
      valGroup.innerHTML = `<label class="form-label">Value</label>
        <select class="form-select" id="filter-val" style="width:100%;">
          ${options.map(o => `<option value="${escapeHtml(o)}">${escapeHtml(o)}</option>`).join('')}
        </select>`;
    } else {
      valGroup.innerHTML = `<label class="form-label">Value</label><input class="form-input" id="filter-val" placeholder="Value">`;
    }
  };

  document.getElementById('filter-prop')?.addEventListener('change', updateValueInput);
  document.getElementById('filter-cond')?.addEventListener('change', updateValueInput);

  document.getElementById('filter-apply')?.addEventListener('click', () => {
    const propId = parseInt(document.getElementById('filter-prop')?.value);
    const condition = document.getElementById('filter-cond')?.value || 'eq';
    const value = document.getElementById('filter-val')?.value || '';
    if (!dbvFilters[tabId]) dbvFilters[tabId] = [];
    dbvFilters[tabId].push({ propId, condition, value });
    overlay.remove();
    saveFiltersToViewConfig(tabId);
    onApply();
  });
}

function applyFilters(records, valuesMap, filters, idField) {
  if (!filters || filters.length === 0) return records;
  return records.filter(r => {
    const rid = r[idField];
    return filters.every(f => {
      const val = valuesMap[rid]?.[f.propId] ?? '';
      switch (f.condition) {
        case 'eq': return val === f.value;
        case 'neq': return val !== f.value;
        case 'contains': return String(val).toLowerCase().includes(f.value.toLowerCase());
        case 'empty': return !val;
        case 'not_empty': return !!val;
        default: return true;
      }
    });
  });
}

async function saveFiltersToViewConfig(tabId) {
  const filters = dbvFilters[tabId] || [];
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs.length > 0) {
      await invoke('update_view_config', { id: configs[0].id, filterJson: JSON.stringify(filters), sortJson: null, visibleColumns: null });
    } else {
      const id = await invoke('create_view_config', { tabId, name: 'Default', viewType: 'table' });
      await invoke('update_view_config', { id, filterJson: JSON.stringify(filters), sortJson: null, visibleColumns: null });
    }
  } catch {}
}

async function loadFiltersFromViewConfig(tabId) {
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs.length > 0 && configs[0].filter_json) {
      dbvFilters[tabId] = JSON.parse(configs[0].filter_json);
    }
  } catch {}
}

// ── Hobbies (Media Collections) ──
const MEDIA_TYPES = ['music','anime','manga','movie','series','cartoon','game','book','podcast'];
const MEDIA_LABELS = { music:'Music',anime:'Anime',manga:'Manga',movie:'Movies',series:'Series',cartoon:'Cartoons',game:'Games',book:'Books',podcast:'Podcasts' };
const STATUS_LABELS = { planned:'Planned',in_progress:'In Progress',completed:'Completed',on_hold:'On Hold',dropped:'Dropped' };

async function loadHobbies(subTab) {
  const el = document.getElementById('hobbies-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('hobbies') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (!subTab || subTab === 'Overview') {
    loadHobbiesOverview(pc);
  } else {
    const mediaType = Object.entries(MEDIA_LABELS).find(([k,v]) => v === subTab)?.[0];
    if (mediaType) loadMediaList(pc, mediaType);
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
      if (name) invoke('create_user_list', { name, description: '', color: '#9B9B9B' }).then(() => loadHobbies('Overview')).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadMediaList(el, mediaType) {
  try {
    const items = await invoke('get_media_items', { mediaType, status: mediaStatusFilter === 'all' ? null : mediaStatusFilter, hidden: false });
    const label = MEDIA_LABELS[mediaType];
    const hasEp = ['anime','series','cartoon','manga','podcast'].includes(mediaType);

    // Status filter bar
    const filterBar = `<div class="dev-filters">
      ${['all','planned','in_progress','completed','on_hold','dropped'].map(s =>
        `<button class="pill${mediaStatusFilter === s ? ' active' : ''}" data-filter="${s}">${s === 'all' ? 'All' : STATUS_LABELS[s]}</button>`
      ).join('')}
    </div>`;

    // Fixed columns from existing schema
    const fixedColumns = [
      { key: 'title', label: 'Title', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
      { key: 'status', label: 'Status', render: r => `<span class="badge ${r.status === 'completed' ? 'badge-green' : r.status === 'in_progress' ? 'badge-blue' : 'badge-gray'}">${STATUS_LABELS[r.status] || r.status}</span>` },
      { key: 'rating', label: 'Rating', render: r => {
        const stars = r.rating ? '\u2605'.repeat(Math.round(r.rating / 2)) + '\u2606'.repeat(5 - Math.round(r.rating / 2)) : '\u2014';
        return `<span style="color:var(--text-secondary);font-size:12px;">${stars}</span>`;
      }},
      ...(hasEp ? [{ key: 'progress', label: 'Progress', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.total_episodes ? `${r.progress || 0}/${r.total_episodes}` : ''}</span>` }] : []),
      { key: 'year', label: 'Year', render: r => `<span style="color:var(--text-muted);font-size:12px;">${r.year || '\u2014'}</span>` },
    ];

    // Render filter bar + database view container
    el.innerHTML = filterBar + '<div id="media-dbv"></div>';
    const dbvEl = document.getElementById('media-dbv');

    await renderDatabaseView(dbvEl, 'hobbies', 'media_items', items, {
      fixedColumns,
      idField: 'id',
      addButton: '+ Add',
      onAdd: () => showAddMediaModal(mediaType),
      onRowClick: (record) => showMediaDetail(record, mediaType),
      reloadFn: () => loadMediaList(el, mediaType),
      _tabId: 'hobbies',
      _recordTable: 'media_items',
    });

    el.querySelectorAll('.dev-filters .pill').forEach(btn => {
      btn.addEventListener('click', () => { mediaStatusFilter = btn.dataset.filter; loadMediaList(el, mediaType); });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
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
      loadHobbies(MEDIA_LABELS[mediaType]);
    } catch (err) { alert('Error: ' + err); }
  });
}

function showMediaDetail(item, mediaType) {
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">${escapeHtml(item.title)}</div>
    ${item.year ? `<div style="color:var(--text-secondary);font-size:12px;margin-bottom:8px;">${item.year}</div>` : ''}
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
      loadHobbies(MEDIA_LABELS[mediaType]);
    } catch (err) { alert('Error: ' + err); }
  });
  document.getElementById('md-delete')?.addEventListener('click', async () => {
    if (!(await confirmModal('Удалить?'))) return;
    await invoke('delete_media_item', { id: item.id }).catch(e => alert(e));
    overlay.remove();
    loadHobbies(MEDIA_LABELS[mediaType]);
  });
}

// ── Sports ──
async function loadSports(subTab) {
  const el = document.getElementById('sports-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('sports') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Martial Arts') {
    loadMartialArts(pc);
    return;
  }
  if (subTab === 'Stats') {
    loadSportsStats(pc);
    return;
  }
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
    renderSports(pc, workouts || [], stats);
  } catch (e) {
    showStub('sports-content', '💪', 'Спорт', 'Тренировки и физическая активность');
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
      tbody.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--text-faint);padding:24px;">Нет тренировок</td></tr>';
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
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);">Error: ${e}</div>`; }
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
        <h3 style="font-size:14px;color:var(--text-muted);margin-bottom:8px;">По типам</h3>
        ${Object.entries(byType).map(([t, c]) => `<div style="display:flex;justify-content:space-between;padding:4px 0;font-size:14px;color:var(--text-secondary);border-bottom:1px solid var(--bg-hover);">
          <span>${typeLabels[t] || t}</span><span style="color:var(--text-muted);">${c}</span>
        </div>`).join('') || '<div style="color:var(--text-faint);font-size:14px;">No data yet</div>'}
      </div>`;
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);">Error: ${e}</div>`; }
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
  el.innerHTML = renderPageHeader('health') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  try {
    const today = await invoke('get_health_today').catch(() => ({}));
    const habits = await invoke('get_habits_today').catch(() => []);
    renderHealth(pc, today, habits);
  } catch (e) {
    pc.innerHTML = '<div class="tab-stub"><div class="tab-stub-icon">❤️</div><div class="tab-stub-title">Здоровье</div><div class="tab-stub-desc">Метрики здоровья и привычки</div><span class="tab-stub-badge">Скоро</span></div>';
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

// ── Initialization ──
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
  openTabs = openTabs.filter(id => TAB_REGISTRY[id]);
  if (!openTabs.includes('chat')) openTabs.unshift('chat');

  // Render tab bar
  renderTabBar();
  activateView();

  // Focus floating widget
  createFocusWidget();
  updateFocusWidget();
  focusWidgetPollInterval = setInterval(() => updateFocusWidget(), 3000);

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

// ── Wake Word SSE Management ──

let _wakeWordSSE = null;

function startWakeWordSSE(keyword) {
  stopWakeWordSSE();
  if (!voiceServerAvailable) return;
  try {
    _wakeWordSSE = new EventSource(`${VOICE_SERVER}/wakeword/events`);
    _wakeWordSSE.onmessage = async (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.detected && !callModeActive) {
          console.log('[WakeWord] Detected! Starting call mode...');
          // Stop wake word (call mode will use mic)
          stopWakeWordSSE();
          try { await fetch(`${VOICE_SERVER}/wakeword/stop`, { method: 'POST' }); } catch (_) {}
          // Focus window and start call mode
          try {
            const { getCurrentWindow } = window.__TAURI__.window;
            const win = getCurrentWindow();
            await win.unminimize();
            await win.show();
            await win.setFocus();
          } catch (_) {}
          toggleCallMode();
        }
      } catch (_) {}
    };
    _wakeWordSSE.onerror = () => {
      // SSE disconnected — will reconnect or stop
      stopWakeWordSSE();
    };
  } catch (_) {}
}

function stopWakeWordSSE() {
  if (_wakeWordSSE) {
    _wakeWordSSE.close();
    _wakeWordSSE = null;
  }
}

// Auto-start wake word on load if enabled
(async () => {
  try {
    const enabled = await invoke('get_app_setting', { key: 'wakeword_enabled' });
    if (enabled === 'true' && voiceServerAvailable !== false) {
      const keyword = await invoke('get_app_setting', { key: 'wakeword_keyword' }).catch(() => 'ханни');
      // Wait for voice server to be ready
      setTimeout(async () => {
        if (!voiceServerAvailable) return;
        try {
          await fetch(`${VOICE_SERVER}/wakeword/start`, {
            method: 'POST', headers: {'Content-Type':'application/json'},
            body: JSON.stringify({ keyword: keyword || 'ханни' }),
          });
          startWakeWordSSE(keyword || 'ханни');
        } catch (_) {}
      }, 8000); // Wait for voice server boot
    }
  } catch (_) {}
})();

// ── Call Mode ──

let callModeActive = false;
let callInitializing = false;  // guard: prevent race between init and end
let callBusy = false;          // guard: prevent overlapping LLM calls in call mode
let callPendingTranscript = null;  // queued transcript while callBusy
let callDurationInterval = null;
let callStartTime = 0;

const callBtn = document.getElementById('call-btn');
const callOverlay = document.getElementById('call-overlay');
const callPhaseText = document.getElementById('call-phase-text');
const callTranscriptArea = document.getElementById('call-transcript-area');
const callEndBtn = document.getElementById('call-end-btn');
const callWaveform = document.getElementById('call-waveform');
const callStatusHint = document.getElementById('call-status-hint');
const callDurationEl = document.getElementById('call-duration');
const callWaveBars = callWaveform ? callWaveform.querySelectorAll('.call-wave-bar') : [];

const PHASE_LABELS = {
  idle: '',
  listening: 'Слушаю...',
  recording: 'Записываю...',
  processing: 'Думаю...',
  speaking: 'Говорю...',
};

async function toggleCallMode() {
  if (callModeActive) {
    await endCallMode();
  } else {
    await startCallMode();
  }
}

async function startCallMode() {
  if (callInitializing) return;  // prevent double-init
  callInitializing = true;
  try {
  callModeActive = true;
  callBtn.classList.add('active');
  callOverlay.classList.remove('hidden');
  callOverlay.setAttribute('data-phase', 'listening');
  callPhaseText.textContent = PHASE_LABELS.listening;
  callTranscriptArea.innerHTML = '';
  if (callStatusHint) callStatusHint.textContent = '';

  // Start wave observer + call duration timer
  ensureWaveObserver();
  callStartTime = Date.now();
  if (callDurationEl) callDurationEl.textContent = '0:00';
  callDurationInterval = setInterval(() => {
    const total = Math.floor((Date.now() - callStartTime) / 1000);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    if (callDurationEl) callDurationEl.textContent = h > 0
      ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
      : `${m}:${s.toString().padStart(2, '0')}`;
  }, 1000);

  // Start a fresh chat for this call
  await autoSaveConversation();
  currentConversationId = null;
  history = [];
  chat.innerHTML = '';
  addMsg('bot', 'Звонок начат... Говорите!');

  // Disable normal input
  input.disabled = true;
  sendBtn.disabled = true;
  recordBtn.disabled = true;

  await checkVoiceServer();

  if (voiceServerAvailable) {
    // Use Python voice server for call mode (SSE stream)
    // Python voice server has its own MLX Whisper — no Rust model needed
    try {
      // Close any stale EventSource from a previous session
      if (window._callEventSource) {
        window._callEventSource.close();
        window._callEventSource = null;
      }

      const eventSource = new EventSource(`${VOICE_SERVER}/listen`);
      window._callEventSource = eventSource;

      eventSource.onmessage = (event) => {
        if (!callModeActive) { eventSource.close(); window._callEventSource = null; return; }
        try {
          const data = JSON.parse(event.data);
          if (data.error) {
            addMsg('bot', 'Ошибка голоса: ' + data.error);
            endCallMode();
            return;
          }
          if (data.phase === 'listening') {
            callOverlay.setAttribute('data-phase', 'listening');
            callPhaseText.textContent = PHASE_LABELS.listening;
            return;
          }
          if (data.text) {
            const customEvent = new CustomEvent('voice-server-transcript', {
              detail: { text: data.text, sttMs: data.stt_ms || 0 }
            });
            window.dispatchEvent(customEvent);
          }
        } catch (_) {}
      };

      let sseRetryCount = 0;
      eventSource.onerror = () => {
        if (!callModeActive) { eventSource.close(); window._callEventSource = null; return; }
        sseRetryCount++;
        if (sseRetryCount >= 3) {
          eventSource.close();
          window._callEventSource = null;
          addMsg('bot', 'Голосовой сервер недоступен — переключаюсь на Rust режим');
          // Fallback to Rust call mode
          (async () => {
            if (!callModeActive) return;  // abort if call already ended
            try {
              const hasModel = await invoke('check_whisper_model');
              if (!callModeActive) return;
              if (hasModel) {
                await invoke('start_call_mode');
              } else {
                addMsg('bot', 'Rust Whisper модель не установлена');
                await endCallMode();
              }
            } catch (e) {
              if (!callModeActive) return;
              addMsg('bot', 'Ошибка запуска Rust режима: ' + e);
              await endCallMode();
            }
          })();
        }
      };
    } catch (e) {
      window._callEventSource = null;
      addMsg('bot', 'Ошибка голосового сервера: ' + e);
      await endCallMode();
    }
  } else {
    // Fallback: Rust call mode (needs Rust Whisper ggml model)
    try {
      const hasModel = await invoke('check_whisper_model');
      if (!hasModel) {
        if (await confirmModal('Модель Whisper не найдена (~1.5GB). Скачать для голосового ввода?')) {
          addMsg('bot', 'Скачиваю модель Whisper...');
          const unlisten = await listen('whisper-download-progress', (event) => {
            const msgs = chat.querySelectorAll('.msg.bot');
            const last = msgs[msgs.length - 1];
            if (last) last.textContent = `Скачиваю Whisper... ${event.payload}%`;
          });
          try {
            await invoke('download_whisper_model');
            addMsg('bot', 'Whisper загружен!');
          } catch (e) {
            addMsg('bot', 'Ошибка загрузки Whisper: ' + e);
            unlisten();
            await endCallMode();
            return;
          }
          unlisten();
        } else {
          await endCallMode();
          return;
        }
      }
    } catch (e) {
      addMsg('bot', 'Ошибка Whisper: ' + e);
      await endCallMode();
      return;
    }
    try {
      await invoke('start_call_mode');
    } catch (e) {
      addMsg('bot', 'Ошибка запуска звонка: ' + e);
      await endCallMode();
    }
  }
  } finally {
    callInitializing = false;
  }
}

async function endCallMode() {
  callModeActive = false;
  callInitializing = false;
  callBusy = false;
  callPendingTranscript = null;
  callBtn.classList.remove('active');
  callOverlay.classList.add('hidden');

  // Stop call duration timer + wave observer
  if (callDurationInterval) { clearInterval(callDurationInterval); callDurationInterval = null; }
  if (callWaveObserver) { callWaveObserver.disconnect(); callWaveObserver = null; }
  if (ambientWaveFrame) { cancelAnimationFrame(ambientWaveFrame); ambientWaveFrame = null; }

  // Reset waveform
  for (const bar of callWaveBars) bar.style.height = '6px';
  if (callStatusHint) callStatusHint.textContent = '';

  // Close voice server SSE if active
  if (window._callEventSource) {
    window._callEventSource.close();
    window._callEventSource = null;
    try { await fetch(`${VOICE_SERVER}/listen/stop`, { method: 'POST' }); } catch (_) {}
    // Cooldown: let Python server release _listen_lock before allowing restart
    await new Promise(r => setTimeout(r, 150));
  }

  // Re-enable input
  input.disabled = false;
  sendBtn.disabled = false;
  recordBtn.disabled = false;

  try {
    await invoke('stop_call_mode');
  } catch (e) {
    console.error('[call] stop_call_mode failed:', e);
  }

  // Resume wake word if it was enabled
  (async () => {
    try {
      const wkEnabled = await invoke('get_app_setting', { key: 'wakeword_enabled' });
      if (wkEnabled === 'true') {
        const wkKeyword = await invoke('get_app_setting', { key: 'wakeword_keyword' }).catch(() => 'ханни');
        await fetch(`${VOICE_SERVER}/wakeword/start`, {
          method: 'POST', headers: {'Content-Type':'application/json'},
          body: JSON.stringify({ keyword: wkKeyword || 'ханни' }),
        });
        startWakeWordSSE(wkKeyword || 'ханни');
      }
    } catch (_) {}
  })();

  input.focus();
}

// Listen for phase changes
listen('call-phase-changed', (event) => {
  const phase = event.payload;
  if (!callModeActive && phase !== 'idle') return;
  callOverlay.setAttribute('data-phase', phase);
  callPhaseText.textContent = PHASE_LABELS[phase] || phase;
});

// Audio level visualization (waveform bars)
listen('call-audio-level', (event) => {
  if (!callModeActive || !callWaveBars.length) return;
  const level = event.payload; // 0-100
  const barCount = callWaveBars.length;
  const center = Math.floor(barCount / 2);
  for (let i = 0; i < barCount; i++) {
    const dist = Math.abs(i - center);
    const scale = Math.max(0, 1 - dist * 0.15);
    const jitter = 0.7 + Math.random() * 0.6;
    const h = Math.max(6, (level / 100) * 34 * scale * jitter);
    callWaveBars[i].style.height = h + 'px';
  }
});

// Ambient waveform animation for processing/speaking phases (no real audio level)
let ambientWaveFrame = null;
function animateAmbientWave() {
  if (!callModeActive || !callWaveBars.length) { ambientWaveFrame = null; return; }
  const phase = callOverlay.getAttribute('data-phase');
  if (phase !== 'processing' && phase !== 'speaking') { ambientWaveFrame = null; return; }
  const t = Date.now() / 1000;
  const amplitude = phase === 'speaking' ? 20 : 12;
  const speed = phase === 'speaking' ? 3 : 2;
  const barCount = callWaveBars.length;
  for (let i = 0; i < barCount; i++) {
    const h = Math.max(6, amplitude * (0.5 + 0.5 * Math.sin(t * speed + i * 0.8)) + Math.random() * 4);
    callWaveBars[i].style.height = h + 'px';
  }
  ambientWaveFrame = requestAnimationFrame(animateAmbientWave);
}

// Start ambient wave when phase changes to processing/speaking
let callWaveObserver = null;
function ensureWaveObserver() {
  if (callWaveObserver) return;
  callWaveObserver = new MutationObserver(() => {
    const phase = callOverlay.getAttribute('data-phase');
    if ((phase === 'processing' || phase === 'speaking') && !ambientWaveFrame) {
      animateAmbientWave();
    }
  });
  callWaveObserver.observe(callOverlay, { attributes: true, attributeFilter: ['data-phase'] });
}

// Limit transcript DOM to last 40 elements to prevent memory growth on long calls
const MAX_TRANSCRIPT_CHILDREN = 40;
function trimTranscript() {
  while (callTranscriptArea.children.length > MAX_TRANSCRIPT_CHILDREN) {
    callTranscriptArea.removeChild(callTranscriptArea.firstChild);
  }
}

// Not-heard feedback
listen('call-not-heard', (event) => {
  if (!callModeActive || !callStatusHint) return;
  callStatusHint.textContent = 'Не расслышала, повторите...';
  callStatusHint.classList.remove('flash');
  void callStatusHint.offsetWidth; // force reflow
  callStatusHint.classList.add('flash');
});

// Barge-in visual feedback
listen('call-barge-in', () => {
  if (!callModeActive) return;
  // Flash the overlay border
  callOverlay.classList.remove('barged-in');
  void callOverlay.offsetWidth;
  callOverlay.classList.add('barged-in');
  setTimeout(() => callOverlay.classList.remove('barged-in'), 600);
  if (callStatusHint) {
    callStatusHint.textContent = 'Перебили — слушаю!';
    callStatusHint.classList.remove('flash');
    void callStatusHint.offsetWidth;
    callStatusHint.classList.add('flash');
  }
});

// Audio error — auto end call
listen('call-error', async (event) => {
  if (!callModeActive) return;
  addMsg('bot', event.payload || 'Ошибка аудио');
  await endCallMode();
});

// Handle transcripts from Python voice server
window.addEventListener('voice-server-transcript', (event) => {
  if (!callModeActive || !event.detail) return;
  const { text, sttMs } = event.detail;
  handleCallTranscript(text, sttMs || 0);
});

// Listen for transcripts from Rust call mode
listen('call-transcript', async (event) => {
  if (!callModeActive || !event.payload) return;
  handleCallTranscript(event.payload, 0);
});

async function handleCallTranscript(userText, sttMs = 0) {
  if (!callModeActive || !userText) return;

  // Guard: if LLM is already processing, queue the latest transcript
  if (callBusy) {
    callPendingTranscript = { text: userText, sttMs };
    return;
  }
  callBusy = true;

  // Pause voice server mic during LLM processing to avoid new transcripts
  const useVoiceServer = !!window._callEventSource;
  if (useVoiceServer) {
    try { await fetch(`${VOICE_SERVER}/listen/pause`, { method: 'POST' }); } catch (_) {}
  }

  // Show user bubble in overlay
  const userBubble = document.createElement('div');
  userBubble.className = 'call-transcript-user';
  userBubble.textContent = userText;
  callTranscriptArea.appendChild(userBubble);
  trimTranscript();
  callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;

  // Also add to actual chat history (voice)
  addMsg('user', userText, true);
  history.push({ role: 'user', content: userText });
  { const lastUserWrapper = chat.querySelector('.user-wrapper:last-of-type');
    if (lastUserWrapper) lastUserWrapper.dataset.historyIdx = String(history.length - 1); }

  // Update UI phase to "processing" while LLM thinks
  callOverlay.setAttribute('data-phase', 'processing');
  callPhaseText.textContent = PHASE_LABELS.processing;

  // Run LLM — same agentic loop as send()
  try {
  const t0 = performance.now();
  let iteration = 0;
  const MAX_ITERATIONS = 5;
  let lastReply = '';

  while (iteration < MAX_ITERATIONS) {
    iteration++;
    if (iteration > 1) showAgentIndicator(iteration);

    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const botDiv = document.createElement('div');
    botDiv.className = 'msg bot';
    wrapper.appendChild(botDiv);
    chat.appendChild(wrapper);
    scrollDown();

    const result = await streamChat(botDiv, t0, true);

    // Primary path: native tool calls
    if (result.toolCalls && result.toolCalls.length > 0) {
      history.push({ role: 'assistant', content: result.fullReply || null, tool_calls: result.toolCalls });
      wrapper.dataset.historyIdx = String(history.length - 1);
      lastReply = result.fullReply || '';
      botDiv.classList.add('intermediate');

      for (const tc of result.toolCalls) {
        let args;
        try { args = JSON.parse(tc.function.arguments); } catch (_) { args = {}; }
        args.action = tc.function.name;
        const actionJson = JSON.stringify(args);
        const { success, result: actionResult } = await executeAction(actionJson);
        const actionDiv = document.createElement('div');
        actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
        actionDiv.textContent = actionResult;
        chat.appendChild(actionDiv);
        scrollDown();
        // Show action result in call overlay
        if (callModeActive) {
          const callAction = document.createElement('div');
          callAction.className = `call-action-result ${success ? 'success' : 'error'}`;
          callAction.textContent = `${success ? '\u2713' : '\u2717'} ${actionResult}`;
          callTranscriptArea.appendChild(callAction);
          callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;
        }
        history.push({ role: 'tool', tool_call_id: tc.id, name: tc.function.name, content: String(actionResult) });
      }
      continue;
    }

    if (!result.fullReply) break;

    history.push({ role: 'assistant', content: result.fullReply });
    wrapper.dataset.historyIdx = String(history.length - 1);
    lastReply = result.fullReply;

    // Fallback: parse ```action blocks
    const actions = parseAndExecuteActions(result.fullReply);
    if (actions.length === 0) break;

    botDiv.classList.add('intermediate');
    const results = [];
    for (const actionJson of actions) {
      const { success, result: actionResult } = await executeAction(actionJson);
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
      scrollDown();
      // Show action result in call overlay
      if (callModeActive) {
        const callAction = document.createElement('div');
        callAction.className = `call-action-result ${success ? 'success' : 'error'}`;
        callAction.textContent = `${success ? '\u2713' : '\u2717'} ${actionResult}`;
        callTranscriptArea.appendChild(callAction);
        callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;
      }
      results.push(actionResult);
    }
    history.push({ role: 'user', content: `[Action result: ${results.join('; ')}]` });
  }

  if (!callModeActive) return;

  const llmMs = performance.now() - t0;

  // Show bot reply + timing in overlay
  if (lastReply) {
    const displayText = lastReply
      .replace(/```action[\s\S]*?```/g, '')
      .replace(/<think>[\s\S]*?<\/think>/g, '')
      .trim();
    if (displayText) {
      const botBubble = document.createElement('div');
      botBubble.className = 'call-transcript-bot';
      botBubble.textContent = displayText;
      callTranscriptArea.appendChild(botBubble);

      // Show latency breakdown in call overlay
      const parts = [];
      if (sttMs > 0) parts.push(`STT ${(sttMs / 1000).toFixed(1)}s`);
      parts.push(`LLM ${(llmMs / 1000).toFixed(1)}s`);
      const callTiming = document.createElement('div');
      callTiming.className = 'call-timing';
      callTiming.textContent = parts.join(' · ');
      callTranscriptArea.appendChild(callTiming);
      trimTranscript();
      callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;
    }
  }

  // Save conversation (non-blocking) + add feedback buttons
  (async () => {
    try {
      if (currentConversationId) {
        await invoke('update_conversation', { id: currentConversationId, messages: history });
      } else {
        currentConversationId = await invoke('save_conversation', { messages: history });
      }
      if (currentConversationId) {
        chat.querySelectorAll('.msg-wrapper[data-history-idx]').forEach(w => {
          if (w.querySelector('.feedback-btn')) return;
          const idx = parseInt(w.dataset.historyIdx, 10);
          if (!isNaN(idx) && getRole(history[idx]) === 'assistant') {
            addFeedbackButtons(w, currentConversationId, idx, history[idx]?.content || '');
          }
        });
      }
      if (history.length >= 2) {
        await invoke('process_conversation_end', { messages: history, conversationId: currentConversationId });
      }
      loadConversationsList();
    } catch (_) {}
  })();

  // Speak the reply sentence-by-sentence, then resume listening
  if (lastReply && callModeActive) {
    const ttsT0 = performance.now();
    await speakAndListen(lastReply);
    // Update timing with TTS duration
    const ttsMs = performance.now() - ttsT0;
    const timingEl = callTranscriptArea.querySelector('.call-timing:last-of-type');
    if (timingEl && ttsMs > 500) {
      timingEl.textContent += ` · TTS ${(ttsMs / 1000).toFixed(1)}s`;
    }
  } else if (callModeActive) {
    // Resume voice server mic (was paused at start of handleCallTranscript)
    if (useVoiceServer) {
      try { await fetch(`${VOICE_SERVER}/listen/resume`, { method: 'POST' }); } catch (_) {}
    }
    await invoke('call_mode_resume_listening').catch(() => {});
  }

  } finally {
    callBusy = false;
    // Process queued transcript (only keep the latest one)
    if (callPendingTranscript && callModeActive) {
      const pending = callPendingTranscript;
      callPendingTranscript = null;
      handleCallTranscript(pending.text, pending.sttMs);
    }
  }
}

/// Split text into sentences for streaming TTS
function splitIntoSentences(text) {
  // Split on sentence-ending punctuation, keeping the punctuation attached
  const parts = text.match(/[^.!?…]+[.!?…]+|[^.!?…]+$/g);
  if (!parts) return [text];
  return parts.map(s => s.trim()).filter(s => s.length > 0);
}

async function speakAndListen(text) {
  if (!callModeActive) return;

  const useVoiceServer = !!window._callEventSource;

  // Pause voice server mic to prevent echo (TTS audio picked up by mic)
  if (useVoiceServer) {
    try { await fetch(`${VOICE_SERVER}/listen/pause`, { method: 'POST' }); } catch (_) {}
  }

  // Set phase to speaking
  callOverlay.setAttribute('data-phase', 'speaking');
  callPhaseText.textContent = PHASE_LABELS.speaking;
  if (callStatusHint) callStatusHint.textContent = 'Можешь перебить';
  if (!useVoiceServer) await invoke('call_mode_set_speaking').catch(() => {});

  // Get voice
  let voice = 'xenia';
  try {
    const ps = await invoke('get_proactive_settings');
    voice = ps.voice_name || voice;
  } catch (_) {}

  // Strip action blocks and think blocks for TTS
  const ttsText = text
    .replace(/```action[\s\S]*?```/g, '')
    .replace(/<think>[\s\S]*?<\/think>/g, '')
    .trim();
  if (!ttsText) {
    if (useVoiceServer) {
      try { await fetch(`${VOICE_SERVER}/listen/resume`, { method: 'POST' }); } catch (_) {}
    } else {
      await invoke('call_mode_resume_listening').catch(() => {});
    }
    if (callModeActive) {
      callOverlay.setAttribute('data-phase', 'listening');
      callPhaseText.textContent = PHASE_LABELS.listening;
    }
    return;
  }

  // Split into sentences for streaming TTS
  const sentences = splitIntoSentences(ttsText);

  // Barge-in polling only for Rust call mode (Rust audio loop detects speech during TTS)
  // Voice server mode: mic is paused during TTS, so no barge-in detection possible
  let bargedIn = false;
  let bargeInterval;
  if (!useVoiceServer) {
    bargeInterval = setInterval(async () => {
      if (!callModeActive) { clearInterval(bargeInterval); return; }
      try {
        const b = await invoke('call_mode_check_bargein');
        if (b) {
          bargedIn = true;
          clearInterval(bargeInterval);
          await invoke('stop_speaking').catch(() => {});
          if (callModeActive) {
            await invoke('call_mode_resume_listening').catch(() => {});
          }
        }
      } catch (_) {}
    }, 250);
  }

  // Speak each sentence sequentially
  try {
    for (const sentence of sentences) {
      if (bargedIn || !callModeActive) break;
      try {
        await invoke('speak_sentence_blocking', { sentence, voice });
      } catch (_) {}
      // Check barge-in between sentences (Rust mode only)
      if (!useVoiceServer && !bargedIn && callModeActive) {
        try {
          const b = await invoke('call_mode_check_bargein');
          if (b) {
            bargedIn = true;
            await invoke('stop_speaking').catch(() => {});
            if (callModeActive) {
              await invoke('call_mode_resume_listening').catch(() => {});
            }
          }
        } catch (_) {}
      }
    }
  } finally {
    if (bargeInterval) clearInterval(bargeInterval);
  }

  // Resume voice server mic after TTS finishes
  if (useVoiceServer) {
    try { await fetch(`${VOICE_SERVER}/listen/resume`, { method: 'POST' }); } catch (_) {}
  }

  // Resume listening
  if (callStatusHint) callStatusHint.textContent = '';
  if (!bargedIn && callModeActive) {
    callOverlay.setAttribute('data-phase', 'listening');
    callPhaseText.textContent = PHASE_LABELS.listening;
    if (!useVoiceServer) await invoke('call_mode_resume_listening').catch(() => {});
  }
}

callBtn.addEventListener('click', toggleCallMode);
callEndBtn.addEventListener('click', endCallMode);

// Global shortcut: Cmd+Shift+H toggles call mode (works even when app is minimized)
listen('global-toggle-call', async () => {
  // Show window if hidden/minimized when starting call
  if (!callModeActive) {
    try {
      const { getCurrentWindow } = window.__TAURI__.window;
      const win = getCurrentWindow();
      await win.unminimize();
      await win.show();
      await win.setFocus();
    } catch (_) {}
  }
  toggleCallMode();
});

// Keyboard shortcut: Escape ends call
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && callModeActive) {
    e.preventDefault();
    endCallMode();
  }
});

// ── Focus Floating Widget ──

function createFocusWidget() {
  const existing = document.getElementById('focus-widget');
  if (existing) existing.remove();
  const widget = document.createElement('div');
  widget.id = 'focus-widget';
  widget.className = 'focus-widget';
  widget.innerHTML = `
    <div class="focus-widget-popover hidden" id="fw-popover">
      <h4>Быстрый старт</h4>
      <input class="fw-input" id="fw-input" placeholder="Название..." />
      <input class="fw-input" id="fw-duration" type="number" min="1" max="480" placeholder="Длительность (мин)..." />
      <div class="fw-presets">
        <span class="fw-preset" data-cat="work">Работа</span>
        <span class="fw-preset" data-cat="study">Учёба</span>
        <span class="fw-preset" data-cat="sport">Спорт</span>
        <span class="fw-preset" data-cat="rest">Отдых</span>
        <span class="fw-preset" data-cat="hobby">Хобби</span>
        <span class="fw-preset" data-cat="other">Другое</span>
      </div>
      <button class="fw-start-btn" id="fw-start-btn">Начать</button>
    </div>
    <div class="focus-widget-active hidden" id="fw-active">
      <span class="fw-pulse-dot"></span>
      <span class="fw-activity-name" id="fw-activity-name"></span>
      <span class="fw-timer" id="fw-timer">00:00</span>
      <span class="fw-stop-btn" id="fw-stop-btn">■</span>
    </div>
    <div class="focus-widget-btn" id="fw-idle-btn">◎</div>
  `;
  document.getElementById('content-area').appendChild(widget);
  bindFocusWidgetEvents();
}

function bindFocusWidgetEvents() {
  const idleBtn = document.getElementById('fw-idle-btn');
  const popover = document.getElementById('fw-popover');
  const startBtn = document.getElementById('fw-start-btn');
  const stopBtn = document.getElementById('fw-stop-btn');
  const presets = document.querySelectorAll('.fw-preset');
  let selectedCat = 'other';

  idleBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleFocusWidgetPopover();
  });

  presets.forEach(p => {
    p.addEventListener('click', (e) => {
      e.stopPropagation();
      presets.forEach(x => x.classList.remove('selected'));
      p.classList.add('selected');
      selectedCat = p.dataset.cat;
      const fwInput = document.getElementById('fw-input');
      if (!fwInput.value.trim()) fwInput.value = p.textContent;
    });
  });

  startBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const fwInput = document.getElementById('fw-input');
    const durationInput = document.getElementById('fw-duration');
    const title = fwInput.value.trim() || 'Без названия';
    const durMin = parseInt(durationInput.value);
    const duration = durMin > 0 ? durMin : null;
    try {
      await invoke('start_activity', {
        title, category: selectedCat, focusMode: false, duration, apps: null, sites: null,
      });
    } catch (_) {}
    fwInput.value = '';
    durationInput.value = '';
    presets.forEach(x => x.classList.remove('selected'));
    selectedCat = 'other';
    popover.classList.add('hidden');
    focusWidgetOpen = false;
    await updateFocusWidget();
  });

  stopBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try { await invoke('stop_activity'); } catch (_) {}
    await updateFocusWidget();
  });

  document.getElementById('fw-active').addEventListener('click', (e) => {
    if (e.target.closest('.fw-stop-btn')) return;
    switchTab('focus');
  });

  document.addEventListener('click', (e) => {
    const widget = document.getElementById('focus-widget');
    if (focusWidgetOpen && widget && !widget.contains(e.target)) {
      popover.classList.add('hidden');
      focusWidgetOpen = false;
    }
  });
}

function toggleFocusWidgetPopover() {
  const popover = document.getElementById('fw-popover');
  if (!popover) return;
  if (focusWidgetActivity) return;
  focusWidgetOpen = !focusWidgetOpen;
  popover.classList.toggle('hidden', !focusWidgetOpen);
  if (focusWidgetOpen) {
    const fwInput = document.getElementById('fw-input');
    if (fwInput) setTimeout(() => fwInput.focus(), 50);
  }
}

async function updateFocusWidget() {
  const widget = document.getElementById('focus-widget');
  if (!widget) return;

  let activity = null;
  try {
    activity = await invoke('get_current_activity');
  } catch (_) {}

  const idleBtn = document.getElementById('fw-idle-btn');
  const activeBar = document.getElementById('fw-active');
  const popover = document.getElementById('fw-popover');

  if (activity && activity.id) {
    const changed = !focusWidgetActivity || focusWidgetActivity.id !== activity.id;
    focusWidgetActivity = activity;
    idleBtn.classList.add('hidden');
    popover.classList.add('hidden');
    focusWidgetOpen = false;
    activeBar.classList.remove('hidden');
    document.getElementById('fw-activity-name').textContent = activity.title || 'Активность';
    if (changed) startFocusWidgetTimer(activity.started_at);
    updateSidebarFocusIndicator(true);
  } else {
    focusWidgetActivity = null;
    stopFocusWidgetTimer();
    activeBar.classList.add('hidden');
    idleBtn.classList.remove('hidden');
    updateSidebarFocusIndicator(false);
  }

  updateFocusWidgetVisibility();
}

function startFocusWidgetTimer(startedAt) {
  stopFocusWidgetTimer();
  const start = new Date(startedAt).getTime();
  const timerEl = document.getElementById('fw-timer');
  function tick() {
    const elapsed = Math.floor((Date.now() - start) / 1000);
    const h = Math.floor(elapsed / 3600);
    const m = Math.floor((elapsed % 3600) / 60);
    const s = elapsed % 60;
    if (timerEl) timerEl.textContent = h > 0
      ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
      : `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
  tick();
  focusWidgetTimerInterval = setInterval(tick, 1000);
}

function stopFocusWidgetTimer() {
  if (focusWidgetTimerInterval) {
    clearInterval(focusWidgetTimerInterval);
    focusWidgetTimerInterval = null;
  }
}

function updateSidebarFocusIndicator(active) {
  const focusTab = document.querySelector('.tab-item[data-tab-id="focus"]');
  if (!focusTab) return;
  const existing = focusTab.querySelector('.tab-focus-dot');
  if (active && !existing) {
    const dot = document.createElement('span');
    dot.className = 'tab-focus-dot';
    focusTab.appendChild(dot);
  } else if (!active && existing) {
    existing.remove();
  }
}

function updateFocusWidgetVisibility() {
  const widget = document.getElementById('focus-widget');
  if (widget) widget.classList.toggle('hidden', activeTab === 'focus');
}

input.focus();
