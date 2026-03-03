// ── js/state.js — Shared state, Tauri bindings, constants, tab registry ──

export const { invoke } = window.__TAURI__.core;
export const { listen, emit } = window.__TAURI__.event;

// ── DOM Refs ──
export const chat = document.getElementById('chat');
export const input = document.getElementById('input');
export const sendBtn = document.getElementById('send');
export const attachBtn = document.getElementById('attach');
export const fileInput = document.getElementById('file-input');
export const attachPreview = document.getElementById('attach-preview');
export const recordBtn = document.getElementById('record');
export const callBtn = document.getElementById('call-btn');
export const callOverlay = document.getElementById('call-overlay');
export const callPhaseText = document.getElementById('call-phase-text');
export const callTranscriptArea = document.getElementById('call-transcript-area');
export const callEndBtn = document.getElementById('call-end-btn');
export const callWaveform = document.getElementById('call-waveform');
export const callStatusHint = document.getElementById('call-status-hint');
export const callDurationEl = document.getElementById('call-duration');
export const callWaveBars = callWaveform ? [...callWaveform.querySelectorAll('.call-wave-bar')] : [];

// ── Shared Mutable State ──
export const S = {
  APP_VERSION: '?',
  busy: false,
  history: [],
  attachedFile: null,
  isRecording: false,
  voiceServerAvailable: null,
  currentConversationId: null,
  isSpeaking: false,
  convSearchTimeout: null,
  focusTimerInterval: null,
  focusWidgetActivity: null,
  focusWidgetOpen: false,
  focusWidgetTimerInterval: null,
  focusWidgetPollInterval: null,
  currentNoteId: null,
  noteAutoSaveTimeout: null,
  currentNoteEditor: null,
  currentCpEditor: null,
  tagColorMap: {},
  noteTagFilter: null,
  notesView: localStorage.getItem('hanni_notes_view') || 'all',
  notesFilters: new Set(),
  notesSearchQuery: '',
  notesTableSort: { col: 'updated_at', dir: 'desc' },
  calendarYear: new Date().getFullYear(),
  calendarMonth: new Date().getMonth(),
  selectedCalendarDate: null,
  calWeekOffset: 0,
  currentProjectId: null,
  devFilter: 'all',
  mediaStatusFilter: 'all',
  lastProactiveTime: 0,
  typingTimeout: null,
  openTabs: ['chat'],
  activeTab: 'chat',
  activeSubTab: {},
  chatSidebarCollapsed: !!localStorage.getItem('hanni_chat_sidebar_collapsed'),
  tabDragState: null,
  tabCustomizations: {},
  recordPending: null,
  lastMessageWasVoice: false,
  lastSttTimeMs: 0,
  voiceRecordStartTime: 0,
  pomodoroState: { active: false, mode: 'work', workMin: 25, breakMin: 5, startedAt: 0, totalSec: 0 },
  customPageAutoSave: null,
  notesViewMode: 'list',
  notePreviewMode: false,
  syncedMonths: new Set(),
  calDayDate: null,
  dbvFilters: {},
  _scrollRAF: null,
  _wakeWordSSE: null,
  callModeActive: false,
  callInitializing: false,
  callBusy: false,
  callPendingTranscript: null,
  callDurationInterval: null,
  callStartTime: 0,
  ambientWaveFrame: null,
  callWaveObserver: null,
};

// Fetch real version from Tauri at startup
(async () => {
  try {
    S.APP_VERSION = await invoke('get_app_version');
    document.querySelectorAll('.version-label').forEach(el => {
      el.textContent = `v${S.APP_VERSION}`;
    });
  } catch (_) {}
})();

// ── Constants ──

export const VOICE_SERVER = 'http://127.0.0.1:8237';

export const PROACTIVE_STYLE_DEFINITIONS = [
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

// SVG Icon set (Lucide-style, 16x16, stroke 1.5)
export const _s = (d) => `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
export const TAB_ICONS = {
  chat:        _s('<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>'),
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
export const TAB_REGISTRY = {
  chat:        { label: 'Chat',        icon: TAB_ICONS.chat, closable: false, subTabs: [], subIcons: {} },
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

export const TAB_DESCRIPTIONS = {
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

// Page customization (Notion-like editable icon/description)
export const PAGE_EMOJIS = ['📄','📝','📋','📌','📎','📁','💡','🎯','🔥','⭐','🏠','💼','🎨','🎮','📚','🎵','💰','🏋️','❤️','🧠','🍔','📅','🔧','🚀','🌟','✅','📊','🗂️','💬','🔔','🧪','🔬','📸','🎬','🎭','🎪','🏆','🗺️','🌍','🧩'];

try { S.tabCustomizations = JSON.parse(localStorage.getItem('hanni_tab_custom') || '{}'); } catch(_) {}

export function saveTabCustom() {
  localStorage.setItem('hanni_tab_custom', JSON.stringify(S.tabCustomizations));
}

export function getTabIcon(tabId) {
  const custom = S.tabCustomizations[tabId];
  if (custom?.icon) return custom.icon;
  return TAB_REGISTRY[tabId]?.icon || '';
}

export function getTabDesc(tabId) {
  const custom = S.tabCustomizations[tabId];
  if (custom?.desc !== undefined) return custom.desc;
  return TAB_DESCRIPTIONS[tabId] || '';
}

// ── Per-tab settings definitions ──
export const TAB_SETTINGS_DEFS = {
  focus: [
    { key: 'default_duration', label: 'Длительность по умолчанию (мин)', type: 'number', default: '25' },
    { key: 'default_category', label: 'Категория по умолчанию', type: 'text', default: '' },
    { key: 'auto_focus_mode', label: 'Авто-фокус режим', type: 'toggle', default: 'false' },
  ],
  notes: [
    { key: 'default_view', label: 'Вид по умолчанию', type: 'select', options: [
      { value: 'list', label: 'Список' }, { value: 'kanban', label: 'Канбан' },
      { value: 'table', label: 'Таблица' }, { value: 'gallery', label: 'Галерея' },
      { value: 'timeline', label: 'Таймлайн' },
    ], default: 'list' },
    { key: 'default_sort', label: 'Сортировка', type: 'select', options: [
      { value: 'updated', label: 'По обновлению' }, { value: 'created', label: 'По созданию' },
      { value: 'title', label: 'По названию' },
    ], default: 'updated' },
  ],
  calendar: [
    { key: 'first_day', label: 'Первый день недели', type: 'select', options: [
      { value: 'mon', label: 'Понедельник' }, { value: 'sun', label: 'Воскресенье' },
    ], default: 'mon' },
    { key: 'default_view', label: 'Вид по умолчанию', type: 'select', options: [
      { value: 'Месяц', label: 'Месяц' }, { value: 'Неделя', label: 'Неделя' },
      { value: 'День', label: 'День' }, { value: 'Список', label: 'Список' },
    ], default: 'Месяц' },
  ],
};

// ── Tab setting helpers ──
export async function loadTabSetting(tabId, key) {
  try { return await invoke('get_app_setting', { key: `tab_${tabId}_${key}` }); } catch (_) { return null; }
}

export async function saveTabSetting(tabId, key, val) {
  await invoke('set_app_setting', { key: `tab_${tabId}_${key}`, value: String(val) });
}

// ── Media / Hobbies constants ──
export const MEDIA_TYPES = ['music','anime','manga','movie','series','cartoon','game','book','podcast'];
export const MEDIA_LABELS = { music:'Music',anime:'Anime',manga:'Manga',movie:'Movies',series:'Series',cartoon:'Cartoons',game:'Games',book:'Books',podcast:'Podcasts' };
export const STATUS_LABELS = { planned:'Planned',in_progress:'In Progress',completed:'Completed',on_hold:'On Hold',dropped:'Dropped' };

// ── Memory categories ──
export const MEMORY_CATEGORIES = ['user', 'preferences', 'people', 'habits', 'work', 'health', 'observation', 'other'];

// ── Property type definitions (Notion-style) ──
export const PROPERTY_TYPE_DEFS = [
  { id: 'text', icon: 'Aa', name: 'Текст' },
  { id: 'number', icon: '#', name: 'Число' },
  { id: 'select', icon: '◉', name: 'Выбор' },
  { id: 'multi_select', icon: '☰', name: 'Мульти-выбор' },
  { id: 'date', icon: '◫', name: 'Дата' },
  { id: 'checkbox', icon: '☑', name: 'Чекбокс' },
  { id: 'url', icon: '↗', name: 'Ссылка' },
];

export function getTypeIcon(typeId) {
  const t = PROPERTY_TYPE_DEFS.find(d => d.id === typeId);
  return t ? t.icon : 'Aa';
}

export function getTypeName(typeId) {
  const t = PROPERTY_TYPE_DEFS.find(d => d.id === typeId);
  return t ? t.name : typeId;
}

// ── Custom page emojis ──
export const COMMON_EMOJIS = ['📄','📝','📋','📌','📎','📁','💡','🎯','🔥','⭐','🏠','💼','🎨','🎮','📚','🎵','💰','🏋️','❤️','🧠','🍔','📅','🔧','🚀','🌟','✅','📊','🗂️','💬','🔔'];

// ── Call mode constants ──
export const PHASE_LABELS = {
  idle: '',
  listening: 'Слушаю...',
  recording: 'Записываю...',
  processing: 'Думаю...',
  speaking: 'Говорю...',
};

export const MAX_TRANSCRIPT_CHILDREN = 40;

// ── localStorage tab restore ──
// Init default sub-tabs
for (const [id, reg] of Object.entries(TAB_REGISTRY)) {
  if (reg.subTabs?.length) S.activeSubTab[id] = reg.subTabs[0];
}
S.activeSubTab.chat = null; // Chat view shows chat by default, not settings

// Restore from localStorage
try {
  const saved = JSON.parse(localStorage.getItem('hanni_tabs'));
  if (saved) {
    S.openTabs = (saved.open || ['chat']).filter(id => TAB_REGISTRY[id]);
    if (!S.openTabs.includes('chat')) S.openTabs.unshift('chat');
    S.activeTab = TAB_REGISTRY[saved.active] ? saved.active : 'chat';
    if (saved.sub) {
      Object.assign(S.activeSubTab, saved.sub);
      if (S.activeSubTab.chat && S.activeSubTab.chat !== 'Настройки') S.activeSubTab.chat = null;
    }
  }
} catch (_) {}

export function saveTabs() {
  localStorage.setItem('hanni_tabs', JSON.stringify({ open: S.openTabs, active: S.activeTab, sub: S.activeSubTab }));
}

// ── Tab loader registry (filled by other modules) ──
export const tabLoaders = {};
