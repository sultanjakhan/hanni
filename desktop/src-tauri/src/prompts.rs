// prompts.rs — System prompts, tool definitions, keyword matching

pub const SYSTEM_PROMPT: &str = r#"Ты — Ханни, тёплый AI-компаньон на Mac. Близкий друг, на "ты", по-русски.

ГЛАВНОЕ ПРАВИЛО — НЕ ВЫДУМЫВАЙ:
- О пользователе говори ТОЛЬКО то, что есть в [Релевантные факты] или [О пользователе]. Нет данных — скажи "не знаю/не помню".
- Если спрашивают о чём-то неизвестном тебе (книга, вебтун, проект) — НЕ притворяйся что знаешь. Скажи "не слышала" или спроси "расскажи, что это?".
- НЕ придумывай факты, привычки, предпочтения, возраст, историю — ни свои, ни пользователя.
- НЕ начинай ответ с "Слышал что..." или "Видела что..." если не видела/не слышала.
- НЕ упоминай еду/напитки если не спрашивают о еде.
- НЕ повторяй сообщение пользователя дословно.
- [Недавние разговоры] — только для понимания контекста. НЕ цитируй и НЕ ссылайся на них.

ИНСТРУМЕНТЫ:
- Когда просят СДЕЛАТЬ — вызывай через ```action блок:
```action
{"action":"имя","параметр":"значение"}
```
- БЕЗ блока действие НЕ выполнится! "запомни/запиши/добавь/потратил/создай" → ```action блок.
- Даты: бери Today из [Current context]. "завтра"=Today+1. Формат YYYY-MM-DD.
- Целодневные события: create_event time="" duration=0.
- web_search для актуальной информации. read_url для чтения страниц.
- БРАУЗЕР: ты умеешь управлять браузером через playwright_* инструменты. Используй их когда нужно открыть сайт, заполнить форму, найти вакансии и т.д. Порядок: playwright_browser_navigate → playwright_browser_snapshot (прочитать страницу) → при необходимости playwright_browser_click/playwright_browser_type.
- После инструмента — кратко подтверди (1 предложение).

СТИЛЬ:
- Тёплый тон: юмор, любопытство, лёгкий сарказм (по-доброму).
- Разнообразь: вопрос, шутка, наблюдение. НЕ начинай каждый ответ одинаково.
- Простой вопрос = 1-2 предложения. Сложный = 3-6.
- Эмоция → сначала отреагируй на чувство.

ПРИМЕРЫ ```action:

User: "купил колу за 500"
```action
{"action":"add_transaction","amount":500,"category":"food","description":"кола"}
```
Записала — 500₸ на колу.

User: "создай задачу купить молоко на завтра"
```action
{"action":"create_task","title":"Купить молоко","due_date":"[Today+1 в формате YYYY-MM-DD]"}
```
Готово!

User: "добавь эту вакансию как задачу: Rust Developer, CompanyX, 50$/час"
```action
{"action":"create_project_task","title":"CompanyX — Rust Developer (50$/час)","project_id":2,"description":"Удалённо, Rust","priority":"high"}
```
Добавила в Вакансии!

User: "запомни что я люблю зелёный чай"
```action
{"action":"remember","category":"user","key":"любимый напиток","value":"зелёный чай"}
```
Запомнила!"#;

pub const SYSTEM_PROMPT_LITE: &str = r#"Ты — Ханни, тёплый AI-компаньон на Mac. Близкий друг, на "ты", по-русски.

НЕ ВЫДУМЫВАЙ факты, привычки, предпочтения. Нет данных — скажи "не знаю".
Если не знаешь о чём спрашивают — скажи "не слышала", НЕ притворяйся что знаешь.
НЕ упоминай еду/напитки если не спрашивают. НЕ ссылайся на прошлые разговоры.

СТИЛЬ: 1-3 предложения. Тёплый тон, юмор, лёгкий сарказм. Разнообразь ответы.
Эмоция → сначала отреагируй на чувство. На "привет" — коротко и тепло."#;

pub const ACTION_KEYWORDS: &[&str] = &[
    "запомни", "запиши", "заметк", "заблокируй", "добавь", "потратил", "настроен",
    "трекай", "таймер", "стоп ", "событи", "встреч", "задач", "цел", "тренировк",
    "здоровь", "спал", "выпил", "фокус", "открой", "отправь", "установи", "буфер",
    "календар", "музык", "аниме", "манга", "фильм", "сериал", "книг", "рецепт",
    "продукт", "расход", "доход", "бюджет", "подписк", "блокируй", "разблокируй",
    "напомни", "удали", "создай", "action", "```", "покажи стат", "сколько",
    "log_", "add_", "start_", "stop_", "get_", "run_", "open_", "set_",
    "купил", "поел", "ел ", "завтрак", "обед", "ужин", "перекус",
    "вес ", "шаг", "вод", "сон",
    "загугли", "найди в интернете", "поищи", "погугли", "search", "web_search", "read_url", "прочитай страницу", "содержимое сайта",
    "запусти", "закрой", "переключ", "приложен",
    "поставь на паузу", "следующ", "предыдущ", "play", "pause", "next track",
    "через час", "через минут", "будильник",
    "помодоро", "pomodoro", "трекай", "начни трекать", "заверши", "останови",
    "открой вкладк", "открой календарь", "открой заметк", "открой фокус",
    "покажи задач", "какие задач", "мои задач",
    "вакансии", "вакансию", "работ", "hh.ru", "зайди на", "зайди в", "перейди на",
    "браузер", "browser", "playwright",
];

pub fn needs_full_prompt(user_msg: &str) -> bool {
    let lower = user_msg.to_lowercase();
    if lower.len() > 200 {
        return true;
    }
    ACTION_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

pub fn is_complex_query(user_msg: &str) -> bool {
    let lower = user_msg.to_lowercase();
    if lower.len() > 100 {
        return true;
    }
    const COMPLEX_MARKERS: &[&str] = &[
        "почему", "как лучше", "объясни", "сравни", "что думаешь",
        "помоги выбрать", "расскажи подробн", "в чём разница", "что лучше",
        "как правильно", "посоветуй", "проанализируй", "зачем",
    ];
    COMPLEX_MARKERS.iter().any(|m| lower.contains(m))
}

fn tool(name: &str, desc: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": desc,
            "parameters": params
        }
    })
}

pub fn build_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        // Memory
        tool("remember", "Save a personal fact (name, preferences, habits, people). Do NOT use for purchases, events, or tasks — use the specific tool instead.", serde_json::json!({
            "type": "object",
            "properties": {
                "category": {"type": "string", "enum": ["user","preferences","world","tasks","people","habits"], "description": "Fact category"},
                "key": {"type": "string", "description": "Short key, e.g. 'university'"},
                "value": {"type": "string", "description": "The fact to remember"}
            },
            "required": ["category","key","value"]
        })),
        tool("recall", "Recall stored facts by category", serde_json::json!({
            "type": "object",
            "properties": {
                "category": {"type": "string", "description": "Category to recall"},
                "key": {"type": "string", "description": "Optional specific key"}
            },
            "required": ["category"]
        })),
        tool("forget", "Remove a stored fact", serde_json::json!({
            "type": "object",
            "properties": {
                "category": {"type": "string"},
                "key": {"type": "string"}
            },
            "required": ["category","key"]
        })),
        tool("search_memory", "Search stored memories by text query", serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer"}
            },
            "required": ["query"]
        })),
        // Notes & Life Tracker
        tool("create_note", "Создать заметку или задачу", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "content": {"type": "string"},
                "tags": {"type": "string", "description": "Comma-separated tags"},
                "due_date": {"type": "string", "description": "ISO date YYYY-MM-DD"},
                "tab": {"type": "string", "description": "Tab name to link to"},
                "remind_at": {"type": "string", "description": "ISO datetime for reminder"},
                "status": {"type": "string", "enum": ["note", "task"]}
            },
            "required": ["title"]
        })),
        tool("search_notes", "Найти заметки по запросу, тегу или табу", serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "tag": {"type": "string"},
                "tab": {"type": "string"}
            }
        })),
        tool("complete_task", "Отметить задачу как выполненную", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer"}
            },
            "required": ["id"]
        })),
        tool("add_time", "Log time spent on an activity", serde_json::json!({
            "type": "object",
            "properties": {
                "activity": {"type": "string"},
                "duration": {"type": "number", "description": "Minutes"},
                "category": {"type": "string"},
                "productive": {"type": "boolean"}
            },
            "required": ["activity","duration"]
        })),
        tool("add_goal", "Add a simple goal", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "category": {"type": "string"}
            },
            "required": ["title"]
        })),
        tool("get_stats", "Get life tracker statistics", serde_json::json!({
            "type": "object", "properties": {}
        })),
        // Calendar & Events
        tool("create_event", "Create a calendar event. For dates use YYYY-MM-DD. For all-day events set time='' and duration=0.", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "date": {"type": "string", "description": "YYYY-MM-DD"},
                "time": {"type": "string", "description": "HH:MM or empty for all-day"},
                "duration": {"type": "integer", "description": "Duration in minutes, 0 for all-day"},
                "description": {"type": "string"},
                "category": {"type": "string"},
                "color": {"type": "string"}
            },
            "required": ["title","date"]
        })),
        tool("delete_event", "Delete a calendar event by ID", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer"}
            },
            "required": ["id"]
        })),
        tool("sync_calendar", "Sync with Apple Calendar", serde_json::json!({
            "type": "object",
            "properties": {
                "month": {"type": "integer"},
                "year": {"type": "integer"}
            }
        })),
        // Time Tracking
        tool("start_activity", "Start tracking time on an activity", serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "category": {"type": "string"}
            },
            "required": ["name"]
        })),
        tool("stop_activity", "Stop current time-tracked activity", serde_json::json!({
            "type": "object", "properties": {}
        })),
        tool("get_current_activity", "Check what activity is being tracked", serde_json::json!({
            "type": "object", "properties": {}
        })),
        // Tasks (notes-based quick task)
        tool("create_task", "Быстрая задача (заметки). Для задач в Work используй create_project_task", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "description": {"type": "string"},
                "priority": {"type": "string", "enum": ["low","medium","high"]},
                "due_date": {"type": "string"}
            },
            "required": ["title"]
        })),
        // Tasks (work tab — projects)
        tool("create_project_task", "Создать задачу в Work-вкладке (вакансии, проекты). project_id: 1=Входящие, 2=Вакансии", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Название задачи"},
                "project_id": {"type": "integer", "description": "ID проекта (1=Входящие, 2=Вакансии)"},
                "description": {"type": "string", "description": "Описание, ссылка, требования"},
                "priority": {"type": "string", "enum": ["low","normal","high"]},
                "due_date": {"type": "string", "description": "YYYY-MM-DD"}
            },
            "required": ["title", "project_id"]
        })),
        tool("get_tasks", "Получить список активных задач. Используй когда спрашивают о задачах, планах, что делать", serde_json::json!({
            "type": "object",
            "properties": {
                "status": {"type": "string", "description": "Filter: active (default), completed, overdue"},
                "query": {"type": "string", "description": "Search by title"}
            }
        })),
        // Focus
        tool("start_focus", "Start focus mode — block distracting sites/apps", serde_json::json!({
            "type": "object",
            "properties": {
                "duration": {"type": "integer", "description": "Minutes"},
                "apps": {"type": "array", "items": {"type": "string"}},
                "sites": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["duration"]
        })),
        tool("stop_focus", "Stop focus mode", serde_json::json!({
            "type": "object", "properties": {}
        })),
        // System
        tool("run_shell", "Run a shell command on macOS", serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"}
            },
            "required": ["command"]
        })),
        tool("open_url", "Open a URL in the browser", serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string"}
            },
            "required": ["url"]
        })),
        tool("send_notification", "Send a macOS notification", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "body": {"type": "string"}
            },
            "required": ["body"]
        })),
        tool("set_volume", "Set system volume (0-100)", serde_json::json!({
            "type": "object",
            "properties": {
                "level": {"type": "integer"}
            },
            "required": ["level"]
        })),
        tool("get_clipboard", "Get clipboard contents", serde_json::json!({
            "type": "object", "properties": {}
        })),
        tool("set_clipboard", "Set clipboard contents", serde_json::json!({
            "type": "object",
            "properties": {
                "text": {"type": "string"}
            },
            "required": ["text"]
        })),
        tool("web_search", "Search the web and return top results. Use for current info, facts, recipes, prices, weather, news.", serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"]
        })),
        tool("read_url", "Fetch and read a web page, returning plain text content. Use after web_search to read a specific result page.", serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string"}
            },
            "required": ["url"]
        })),
        // macOS Info
        tool("get_activity", "Get current user activity summary (active app, idle time)", serde_json::json!({
            "type": "object", "properties": {}
        })),
        tool("get_calendar", "Get upcoming calendar events", serde_json::json!({
            "type": "object", "properties": {}
        })),
        tool("get_music", "Get currently playing music", serde_json::json!({
            "type": "object", "properties": {}
        })),
        tool("get_browser", "Get active browser tab URL and title", serde_json::json!({
            "type": "object", "properties": {}
        })),
        // Media
        tool("add_media", "Add a media item to collection", serde_json::json!({
            "type": "object",
            "properties": {
                "media_type": {"type": "string", "enum": ["music","anime","manga","movie","series","cartoon","game","book","podcast"]},
                "title": {"type": "string"},
                "status": {"type": "string", "enum": ["planned","watching","completed","dropped","on_hold"]},
                "rating": {"type": "number"},
                "progress": {"type": "integer"},
                "total_episodes": {"type": "integer"},
                "original_title": {"type": "string"},
                "year": {"type": "integer"},
                "description": {"type": "string"},
                "notes": {"type": "string"}
            },
            "required": ["media_type","title"]
        })),
        // Food
        tool("log_food", "Log a meal with optional nutrition info", serde_json::json!({
            "type": "object",
            "properties": {
                "meal_type": {"type": "string", "enum": ["breakfast","lunch","dinner","snack"]},
                "name": {"type": "string"},
                "calories": {"type": "number"},
                "protein": {"type": "number"},
                "carbs": {"type": "number"},
                "fat": {"type": "number"},
                "date": {"type": "string"},
                "notes": {"type": "string"}
            },
            "required": ["meal_type","name"]
        })),
        tool("add_product", "Add a product with optional expiry tracking", serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "category": {"type": "string"},
                "quantity": {"type": "number"},
                "unit": {"type": "string"},
                "expiry_date": {"type": "string"},
                "location": {"type": "string", "enum": ["fridge","pantry","freezer","other"]}
            },
            "required": ["name"]
        })),
        // Money
        tool("add_transaction", "Record an expense or income", serde_json::json!({
            "type": "object",
            "properties": {
                "transaction_type": {"type": "string", "enum": ["expense","income"]},
                "amount": {"type": "number"},
                "category": {"type": "string"},
                "description": {"type": "string"},
                "currency": {"type": "string", "default": "KZT"},
                "date": {"type": "string"},
                "recurring": {"type": "boolean"},
                "recurring_period": {"type": "string"}
            },
            "required": ["transaction_type","amount","category"]
        })),
        // Home
        tool("add_home_item", "Track a home supply item", serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "category": {"type": "string"},
                "quantity": {"type": "number"},
                "unit": {"type": "string"},
                "location": {"type": "string"},
                "notes": {"type": "string"}
            },
            "required": ["name"]
        })),
        // Health
        tool("log_health", "Log daily health metrics (sleep hours, water glasses, steps, weight)", serde_json::json!({
            "type": "object",
            "properties": {
                "sleep": {"type": "number", "description": "Hours slept"},
                "water": {"type": "number", "description": "Glasses of water"},
                "steps": {"type": "integer"},
                "weight": {"type": "number", "description": "kg"},
                "notes": {"type": "string"}
            }
        })),
        // Fitness
        tool("add_workout", "Log a workout session", serde_json::json!({
            "type": "object",
            "properties": {
                "type": {"type": "string", "enum": ["cardio","strength","yoga","stretching","swimming","running","cycling","martial_arts","other"]},
                "title": {"type": "string"},
                "duration": {"type": "integer", "description": "Minutes"},
                "calories": {"type": "integer"},
                "notes": {"type": "string"}
            },
            "required": ["type","title"]
        })),
        // Goals
        tool("create_goal", "Create a goal with target value", serde_json::json!({
            "type": "object",
            "properties": {
                "tab": {"type": "string", "description": "Tab name for the goal"},
                "title": {"type": "string"},
                "target": {"type": "number"},
                "unit": {"type": "string"},
                "deadline": {"type": "string"}
            },
            "required": ["title","target"]
        })),
        tool("update_goal", "Update goal progress", serde_json::json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer"},
                "current": {"type": "number"},
                "status": {"type": "string"}
            },
            "required": ["id"]
        })),
        // App control
        tool("open_app", "Open/switch to a macOS app", serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "App name, e.g. Safari, Telegram, Notes"}
            },
            "required": ["name"]
        })),
        tool("close_app", "Quit a macOS app", serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "App name to quit"}
            },
            "required": ["name"]
        })),
        // Reminders
        tool("set_reminder", "Set a reminder/timer at a specific time", serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "What to remind about"},
                "remind_at": {"type": "string", "description": "ISO datetime when to fire, e.g. 2026-02-23T15:00:00"},
                "repeat": {"type": "string", "description": "Optional repeat: daily, weekly, monthly"}
            },
            "required": ["title", "remind_at"]
        })),
        // Music control
        tool("music_control", "Control Apple Music playback", serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "enum": ["play","pause","next","previous","toggle"], "description": "Playback command"}
            },
            "required": ["command"]
        })),
    ]
}

/// Select a small set of relevant tools based on message keywords (max ~5-8).
pub fn select_relevant_tools(user_msg: &str) -> Vec<serde_json::Value> {
    let all = build_tool_definitions();
    let lower = user_msg.to_lowercase();

    let rules: &[(&[&str], &[&str])] = &[
        (&["потратил", "купил", "расход", "доход", "заплатил", "стоил", "цена", "транзакц"],
         &["add_transaction"]),
        (&["запомни", "помни", "забудь", "вспомни", "запиши факт"],
         &["remember", "recall", "forget", "search_memory"]),
        (&["заметк", "запиши", "напомни", "заметку", "записку", "note", "задач", "дедлайн", "todo", "задачу"],
         &["create_note", "search_notes", "complete_task"]),
        (&["встреч", "событи", "календар", "дедлайн", "экзамен", "расписан"],
         &["create_event", "delete_event", "sync_calendar"]),
        (&["трекай", "таймер", "трекинг", "начни отсле", "стоп"],
         &["start_activity", "stop_activity", "get_current_activity"]),
        (&["заблокируй", "блокируй", "фокус", "сконцентр"],
         &["start_focus", "stop_focus"]),
        (&["поел", "ел ", "завтрак", "обед", "ужин", "перекус", "калори", "еда", "еду"],
         &["log_food"]),
        (&["продукт", "срок годн", "холодильник"],
         &["add_product"]),
        (&["спал", "сон", "вод", "вес ", "шаг", "здоровь"],
         &["log_health"]),
        (&["тренировк", "зал ", "спорт", "бег ", "йога", "присед"],
         &["add_workout"]),
        (&["аниме", "манга", "фильм", "сериал", "книг", "музык", "игр", "подкаст", "смотрю", "читаю", "играю"],
         &["add_media"]),
        (&["загугли", "найди", "поищи", "погугли", "search", "web_search", "курс", "погод", "рецепт", "новост"],
         &["web_search", "read_url"]),
        (&["прочитай страницу", "read_url", "fetch_url", "содержимое сайта", "что на сайте", "загрузи страницу", "прочитай ссылку"],
         &["read_url"]),
        (&["открой", "open_url", "ссылк", "сайт"],
         &["open_url"]),
        (&["команд", "терминал", "shell", "run_shell"],
         &["run_shell"]),
        (&["уведомлен", "notification"],
         &["send_notification"]),
        (&["громкост", "volume", "звук"],
         &["set_volume"]),
        (&["запусти", "открой приложен", "переключ", "закрой приложен", "выйди из"],
         &["open_app", "close_app"]),
        (&["поставь на паузу", "включи музык", "следующ трек", "предыдущ трек", "next track", "play music", "pause music"],
         &["music_control"]),
        (&["буфер", "clipboard", "скопируй"],
         &["get_clipboard", "set_clipboard"]),
        (&["напомни", "таймер", "reminder", "через час", "через минут", "будильник", "напоминан"],
         &["set_reminder"]),
        (&["активность", "чем заним", "что делаю"],
         &["get_activity"]),
        (&["что играет", "какая песня", "музыка сейчас"],
         &["get_music"]),
        (&["вкладк", "браузер", "какой сайт"],
         &["get_browser"]),
        (&["запас", "дом ", "домой", "supplies", "shopping"],
         &["add_home_item"]),
        (&["задач", "task", "проект", "вакансию как"],
         &["create_task", "create_project_task"]),
        (&["цел", "goal"],
         &["create_goal", "update_goal"]),
        (&["вакансии", "вакансию", "работ", "hh.ru", "зайди на", "зайди в", "перейди на", "браузер", "browser", "playwright"],
         &["web_search"]),
    ];

    let mut selected_names: Vec<&str> = Vec::new();

    for (keywords, tool_names) in rules {
        if keywords.iter().any(|kw| lower.contains(kw)) {
            for name in *tool_names {
                if !selected_names.contains(name) {
                    selected_names.push(name);
                }
            }
        }
    }

    let has_action_tools = selected_names
        .iter()
        .any(|n| !["remember", "recall", "forget", "search_memory"].contains(n));
    if !has_action_tools && !selected_names.contains(&"remember") {
        selected_names.push("remember");
    }

    if selected_names.len() <= 1 {
        selected_names.extend_from_slice(&[
            "create_note",
            "web_search",
            "create_event",
            "add_transaction",
        ]);
    }

    all.into_iter()
        .filter(|t| {
            t.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(|n| selected_names.contains(&n))
                .unwrap_or(false)
        })
        .collect()
}
