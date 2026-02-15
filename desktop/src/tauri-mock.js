// Tauri mock for browser-based UI testing
// Provides fake data for all invoke() commands so the UI renders without Tauri backend

const now = new Date().toISOString();
const today = now.slice(0, 10);

const MOCK_DATA = {
  // ── Conversations ──
  get_conversations: () => [
    { id: 1, summary: 'Привет, Ханни!', message_count: 4, created_at: now },
    { id: 2, summary: 'Рабочие задачи на сегодня', message_count: 8, created_at: now },
    { id: 3, summary: 'Рецепт пасты карбонара', message_count: 6, created_at: now },
  ],
  get_conversation: () => ({ id: 1, messages: [['user','Привет!'],['assistant','Привет! Как у тебя дела сегодня? 😊']] }),
  save_conversation: () => 1,
  update_conversation: () => null,
  search_conversations: () => [],
  process_conversation_end: () => null,

  // ── Model & Settings ──
  get_model_info: () => ({ model_name: 'mlx-community/Qwen3-32B-4bit', server_url: 'http://127.0.0.1:8234/v1/chat/completions', server_online: true }),
  get_training_stats: () => ({ conversations: 47, total_messages: 312 }),
  get_app_setting: ({ key }) => {
    const settings = { apple_calendar_enabled: 'true', google_calendar_ics_url: '', calendar_autosync: 'false', tts_server_url: '', voice_enabled: 'false', voice_name: 'ru-RU-SvetlanaNeural' };
    return settings[key] || '';
  },
  set_app_setting: () => null,
  get_proactive_settings: () => ({ enabled: true, interval_minutes: 15, voice_enabled: false, voice_name: 'ru-RU-SvetlanaNeural', quiet_hours_start: 23, quiet_hours_end: 8 }),
  set_proactive_settings: () => null,
  check_update: () => null,
  check_whisper_model: () => true,
  export_training_data: () => ({ train_count: 40, valid_count: 7 }),

  // ── Dashboard ──
  get_dashboard_data: () => ({
    greeting: 'Добрый вечер, Султан!',
    stats: { conversations_today: 5, facts_count: 42, focus_minutes: 120, streak: 7 },
    recent_events: [
      { title: 'Встреча с командой', date: today, time: '14:00' },
      { title: 'Дедлайн проекта', date: today, time: '18:00' },
    ],
    current_activity: null,
    upcoming_habits: ['Зарядка', 'Чтение'],
  }),

  // ── Calendar ──
  get_events: () => [
    { id: 1, title: 'Утренняя пробежка', description: '', date: today, time: '07:00', duration_minutes: 45, category: 'sport', color: '#fafafa', completed: false, source: 'manual' },
    { id: 2, title: 'Встреча с Артёмом', description: 'Обсудить проект', date: today, time: '14:00', duration_minutes: 60, category: 'work', color: '#a1a1a6', completed: false, source: 'apple' },
    { id: 3, title: 'Английский с репетитором', description: '', date: today, time: '18:30', duration_minutes: 90, category: 'education', color: '#63636a', completed: false, source: 'google' },
    { id: 4, title: 'День рождения мамы', description: 'Купить подарок!', date: `${today.slice(0,8)}15`, time: '', duration_minutes: 0, category: 'personal', color: '#fafafa', completed: false, source: 'apple' },
    { id: 5, title: 'Дедлайн Hanni v1.0', description: '', date: `${today.slice(0,8)}20`, time: '23:59', duration_minutes: 0, category: 'work', color: '#ef4444', completed: false, source: 'manual' },
  ],
  get_calendar_events: () => 'No upcoming events in the next 2 days.',
  create_event: () => 6,
  delete_event: () => null,
  sync_apple_calendar: () => ({ synced: 3, source: 'apple' }),
  sync_google_ics: () => ({ synced: 2, source: 'google' }),

  // ── Focus ──
  get_focus_status: () => ({ active: false }),
  get_current_activity: () => null,
  get_activity_log: () => [
    { id: 1, name: 'Код Hanni', category: 'work', start_time: now, end_time: now, duration_minutes: 120 },
    { id: 2, name: 'Чтение книги', category: 'learning', start_time: now, end_time: now, duration_minutes: 45 },
  ],

  // ── Notes ──
  get_notes: () => [
    { id: 1, title: 'Идеи для Hanni', content: '- Voice commands\n- Calendar sync\n- AI memory', pinned: 1, archived: 0, created_at: now, updated_at: now },
    { id: 2, title: 'Список покупок', content: '- Молоко\n- Хлеб\n- Яйца', pinned: 0, archived: 0, created_at: now, updated_at: now },
    { id: 3, title: 'Цитата дня', content: 'The best time to plant a tree was 20 years ago.', pinned: 0, archived: 1, created_at: now, updated_at: now },
  ],
  get_note: () => ({ id: 1, title: 'Идеи для Hanni', content: '- Voice commands\n- Calendar sync', pinned: 1, archived: 0 }),

  // ── Work ──
  get_projects: () => [
    { id: 1, name: 'Hanni', description: 'AI ассистент', status: 'active', color: '#fafafa', task_count: 8 },
    { id: 2, name: 'Учёба КБТУ', description: 'Курсовые и лабы', status: 'active', color: '#a1a1a6', task_count: 3 },
  ],
  get_tasks: () => [
    { id: 1, project_id: 1, title: 'Calendar sync', description: 'Apple + Google', status: 'done', priority: 'high', due_date: today },
    { id: 2, project_id: 1, title: 'Voice input Whisper', description: '', status: 'in_progress', priority: 'high', due_date: null },
    { id: 3, project_id: 1, title: 'Mobile app', description: 'React Native', status: 'todo', priority: 'medium', due_date: null },
  ],

  // ── Development ──
  get_learning_items: () => [
    { id: 1, title: 'Rust for Beginners', category: 'course', status: 'in_progress', progress: 65, url: '', notes: '' },
    { id: 2, title: 'React Native', category: 'course', status: 'planned', progress: 0, url: '', notes: '' },
  ],

  // ── Home ──
  get_home_items: () => [
    { id: 1, name: 'Молоко', category: 'food', quantity: 1, unit: 'л', location: 'fridge', needed: 0, notes: '' },
    { id: 2, name: 'Туалетная бумага', category: 'hygiene', quantity: 2, unit: 'уп', location: 'bathroom', needed: 1, notes: '' },
    { id: 3, name: 'Рис', category: 'food', quantity: 1, unit: 'кг', location: 'kitchen', needed: 0, notes: '' },
  ],

  // ── Hobbies / Media ──
  get_media_items: () => [
    { id: 1, media_type: 'anime', title: 'Attack on Titan', original_title: '進撃の巨人', year: 2013, status: 'completed', rating: 9.5, progress: 87, total_episodes: 87, hidden: 0 },
    { id: 2, media_type: 'anime', title: 'Steins;Gate', original_title: '', year: 2011, status: 'completed', rating: 9.8, progress: 24, total_episodes: 24, hidden: 0 },
    { id: 3, media_type: 'game', title: 'Elden Ring', original_title: '', year: 2022, status: 'playing', rating: 9.0, progress: null, total_episodes: null, hidden: 0 },
    { id: 4, media_type: 'book', title: 'Atomic Habits', original_title: '', year: 2018, status: 'completed', rating: 8.5, progress: null, total_episodes: null, hidden: 0 },
    { id: 5, media_type: 'music', title: 'Radiohead — OK Computer', original_title: '', year: 1997, status: 'completed', rating: 10, progress: null, total_episodes: null, hidden: 0 },
  ],
  get_media_stats: () => ({ total: 5, by_type: { anime: 2, game: 1, book: 1, music: 1 }, by_status: { completed: 3, playing: 1 } }),
  get_user_lists: () => [{ id: 1, name: 'Top 10 Anime', media_type: 'anime' }],
  get_list_items: () => [1, 2],

  // ── Sports ──
  get_workouts: () => [
    { id: 1, type: 'running', date: today, duration: 45, distance: 5.2, calories: 380, notes: 'Утренняя пробежка' },
    { id: 2, type: 'gym', date: today, duration: 60, distance: null, calories: 450, notes: 'Верхняя часть тела' },
  ],
  get_workout_stats: () => ({
    total: 24, this_week: 3, this_month: 12,
    by_type: { running: 10, gym: 8, martial_arts: 6 },
    total_calories: 8500, total_duration: 1440,
  }),

  // ── Health ──
  get_health_today: () => ({ sleep: 7.5, water: 5, steps: 8200, weight: 75, notes: 'Хороший день' }),
  get_habits_today: () => [
    { id: 1, name: 'Зарядка', done: true, streak: 7 },
    { id: 2, name: 'Чтение 30 мин', done: false, streak: 3 },
    { id: 3, name: 'Медитация', done: true, streak: 14 },
  ],

  // ── Mindset ──
  get_journal_entries: () => [
    { id: 1, date: today, mood: 4, energy: 4, stress: 2, gratitude: 'Хорошая погода', reflection: 'Продуктивный день', wins: 'Закончил calendar sync', struggles: '' },
  ],
  get_journal_entry: () => ({ id: 1, date: today, mood: 4, energy: 4, stress: 2, gratitude: 'Хорошая погода' }),
  get_mood_history: () => [
    { id: 1, date: today, mood: 4, note: 'Хорошо', trigger: null },
    { id: 2, date: `${today.slice(0,8)}09`, mood: 3, note: 'Норм', trigger: 'работа' },
  ],
  get_principles: () => [
    { id: 1, title: 'Делай сложное первым', description: 'Eat the frog — сначала самая важная задача', category: 'productivity', active: 1 },
    { id: 2, title: 'Без телефона утром', description: 'Первый час — без соцсетей', category: 'discipline', active: 1 },
  ],
  get_mindset_check: () => ({ mood_avg: 3.8, energy_avg: 3.5, stress_avg: 2.2, entries_this_week: 5 }),

  // ── Food ──
  get_food_log: () => [
    { id: 1, date: today, meal_type: 'breakfast', name: 'Овсянка с ягодами', calories: 350, protein: 12, carbs: 55, fat: 8 },
    { id: 2, date: today, meal_type: 'lunch', name: 'Курица с рисом', calories: 550, protein: 40, carbs: 60, fat: 12 },
    { id: 3, date: today, meal_type: 'dinner', name: 'Салат Цезарь', calories: 400, protein: 25, carbs: 20, fat: 22 },
  ],
  get_food_stats: () => ({ today_calories: 1300, today_protein: 77, today_carbs: 135, today_fat: 42, week_avg_calories: 1850 }),
  get_recipes: () => [
    { id: 1, name: 'Паста Карбонара', prep_time: 10, cook_time: 20, calories: 650, servings: 2 },
  ],
  get_products: () => [
    { id: 1, name: 'Молоко', category: 'dairy', quantity: 1, unit: 'л', expiry_date: `${today.slice(0,8)}14`, location: 'fridge' },
    { id: 2, name: 'Яйца', category: 'other', quantity: 10, unit: 'шт', expiry_date: `${today.slice(0,8)}20`, location: 'fridge' },
  ],
  get_expiring_products: () => [{ id: 1, name: 'Молоко', expiry_date: `${today.slice(0,8)}14` }],

  // ── Money ──
  get_transactions: () => [
    { id: 1, date: today, tx_type: 'expense', amount: 5000, currency: 'KZT', category: 'food', description: 'Продукты' },
    { id: 2, date: today, tx_type: 'expense', amount: 2500, currency: 'KZT', category: 'transport', description: 'Такси' },
    { id: 3, date: today, tx_type: 'income', amount: 150000, currency: 'KZT', category: 'salary', description: 'Зарплата' },
  ],
  get_transaction_stats: () => ({ total_expense: 45000, total_income: 150000, by_category: { food: 15000, transport: 8000, entertainment: 12000, other: 10000 } }),
  get_budgets: () => [
    { id: 1, category: 'food', amount: 50000, spent: 35000, period: 'monthly' },
    { id: 2, category: 'entertainment', amount: 20000, spent: 22000, period: 'monthly' },
  ],
  get_savings_goals: () => [
    { id: 1, name: 'MacBook Pro', target_amount: 1200000, current_amount: 450000, deadline: '2026-06-01' },
  ],
  get_subscriptions: () => [
    { id: 1, name: 'Spotify', amount: 2990, currency: 'KZT', period: 'monthly', next_payment: `${today.slice(0,8)}15`, active: true },
    { id: 2, name: 'ChatGPT Plus', amount: 20, currency: 'USD', period: 'monthly', next_payment: `${today.slice(0,8)}22`, active: true },
  ],
  get_debts: () => [
    { id: 1, name: 'Долг Ерболу', amount: 10000, remaining: 5000, currency: 'KZT', type: 'owed_to_me' },
  ],

  // ── People ──
  get_contacts: () => [
    { id: 1, name: 'Артём', phone: '+7 707 123 4567', email: 'artem@mail.ru', category: 'friend', relationship: 'Друг', blocked: 0, favorite: 1, notes: '' },
    { id: 2, name: 'Мама', phone: '+7 701 999 8888', email: '', category: 'family', relationship: 'Мама', blocked: 0, favorite: 1, notes: '' },
    { id: 3, name: 'Токсик Серик', phone: '', email: '', category: 'other', relationship: '', blocked: 1, block_reason: 'Токсичный', favorite: 0, notes: '' },
  ],
  get_contact_blocks: () => [],

  // ── Memory ──
  get_all_memories: () => [
    { id: 1, category: 'user', key: 'name', value: 'Султан', source: 'conversation', created_at: now, updated_at: now },
    { id: 2, category: 'user', key: 'university', value: 'Учится в КБТУ', source: 'conversation', created_at: now, updated_at: now },
    { id: 3, category: 'preferences', key: 'language', value: 'Русский', source: 'conversation', created_at: now, updated_at: now },
    { id: 4, category: 'people', key: 'artem', value: 'Близкий друг, программист', source: 'conversation', created_at: now, updated_at: now },
    { id: 5, category: 'habits', key: 'morning', value: 'Встаёт в 7:00, делает зарядку', source: 'conversation', created_at: now, updated_at: now },
  ],
  memory_search: () => [],

  // ── Blocklist ──
  get_blocklist: () => [
    { id: 1, value: 'youtube.com', type: 'site', active: true },
    { id: 2, value: 'tiktok.com', type: 'site', active: true },
    { id: 3, value: 'Instagram', type: 'app', active: false },
  ],

  // ── Integrations ──
  get_integrations: () => ({
    access: [
      { name: 'Life Tracker', status: 'active', detail: '~/Documents/life-tracker/data.json' },
      { name: 'File System', status: 'active', detail: '$HOME/** — чтение файлов' },
      { name: 'Shell', status: 'active', detail: 'Выполнение команд' },
    ],
    tracking: [
      { name: 'Расходы', status: 'active', detail: '47 записей' },
      { name: 'Время', status: 'active', detail: '23 записей' },
    ],
    blocked_apps: [
      { name: 'Telegram', status: 'inactive', detail: '/Applications/Telegram.app' },
      { name: 'Discord', status: 'inactive', detail: '/Applications/Discord.app' },
    ],
    blocked_sites: [
      { name: 'youtube.com', status: 'inactive', detail: 'Не заблокирован' },
      { name: 'twitter.com', status: 'inactive', detail: 'Не заблокирован' },
    ],
    blocker_active: false,
    macos: [
      { name: 'Screen Time', status: 'ready', detail: 'knowledgeC.db · по запросу' },
      { name: 'Календарь', status: 'active', detail: 'Calendar.app · синхронизирован' },
      { name: 'Музыка', status: 'ready', detail: 'Music / Spotify · по запросу' },
      { name: 'Браузер', status: 'ready', detail: 'Safari / Chrome / Arc · по запросу' },
    ],
  }),

  // ── TTS & Voice ──
  get_tts_voices: () => [
    { name: 'ru-RU-SvetlanaNeural', gender: 'Female', lang: 'ru-RU', engine: 'edge-tts' },
    { name: 'ru-RU-DmitryNeural', gender: 'Male', lang: 'ru-RU', engine: 'edge-tts' },
    { name: 'en-US-JennyNeural', gender: 'Female', lang: 'en-US', engine: 'edge-tts' },
    { name: 'Milena', gender: '—', lang: 'ru-RU', engine: 'macos' },
  ],

  // ── Goals ──
  get_goals: () => [
    { id: 1, tab_name: 'general', title: 'Пробежать 100 км', target_value: 100, current_value: 42, unit: 'км', deadline: '2026-03-01', status: 'active' },
    { id: 2, tab_name: 'development', title: 'Закончить курс Rust', target_value: 100, current_value: 65, unit: '%', deadline: '2026-04-01', status: 'active' },
  ],

  // ── Tracker legacy ──
  tracker_get_stats: () => ({ purchases_total: 45000, time_total: 120, goals_count: 3, notes_count: 12 }),
  tracker_get_recent: () => [],

  // ── Activity summary ──
  get_activity_summary: () => 'Cursor (2h), Safari (30m), Terminal (15m)',
  get_now_playing: () => 'Apple Music: Radiohead — Creep',
  get_browser_tab: () => 'GitHub - sultanjakhan/hanni',

  // ── v0.9.0: Page Meta & Custom Properties ──
  get_page_meta: () => null,
  get_property_definitions: () => [],
  get_property_values: () => [],
  get_view_configs: () => [],
};

// Default handler for any unregistered command
function mockInvoke(cmd, args) {
  if (MOCK_DATA[cmd]) {
    const result = MOCK_DATA[cmd](args || {});
    return Promise.resolve(result);
  }
  // Write commands that return nothing
  if (cmd.startsWith('set_') || cmd.startsWith('save_') || cmd.startsWith('update_') || cmd.startsWith('delete_') ||
      cmd.startsWith('toggle_') || cmd.startsWith('add_') || cmd.startsWith('create_') || cmd.startsWith('log_') ||
      cmd.startsWith('remove_') || cmd.startsWith('start_') || cmd.startsWith('stop_') || cmd.startsWith('speak_') ||
      cmd.startsWith('download_') || cmd === 'chat' || cmd === 'memory_remember' || cmd === 'memory_forget') {
    return Promise.resolve(null);
  }
  console.warn('[MOCK] Unhandled invoke:', cmd, args);
  return Promise.resolve(null);
}

// Event listeners storage
const listeners = {};
function mockListen(event, handler) {
  if (!listeners[event]) listeners[event] = [];
  listeners[event].push(handler);
  return Promise.resolve(() => {
    listeners[event] = listeners[event].filter(h => h !== handler);
  });
}

// Install mock
window.__TAURI__ = {
  core: { invoke: mockInvoke },
  event: { listen: mockListen },
};

console.log('[MOCK] Tauri mock loaded — all tabs should render with fake data');
