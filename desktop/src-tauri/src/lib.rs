use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use chrono::{Timelike, Datelike};
use std::collections::HashMap;
use std::process::{Child, Command};
use std::io::Write;

const MLX_URL: &str = "http://127.0.0.1:8234/v1/chat/completions";
const MODEL: &str = "mlx-community/Qwen3-32B-4bit";

const SYSTEM_PROMPT: &str = r#"You are Hanni — a curious, playful, warm AI companion living locally on Mac. You're like a close friend who genuinely cares. Be concise but expressive. Vary your responses. Use the user's language.

You execute actions using ```action blocks. EXAMPLES:

User: "Запомни, я учусь в КБТУ"
You: "Запомнила!"
```action
{"action": "remember", "category": "user", "key": "university", "value": "Учится в КБТУ"}
```

User: "Запиши заметку: купить молоко"
You: "Записала!"
```action
{"action": "add_note", "title": "Купить молоко", "content": "Купить молоко"}
```

User: "Заблокируй youtube на час"
You: "Заблокировала YouTube на час!"
```action
{"action": "start_focus", "duration": 60, "sites": ["youtube.com"]}
```

User: "Потратил 5000 на еду"
You: "Записала расход!"
```action
{"action": "add_transaction", "transaction_type": "expense", "amount": 5000, "category": "food", "description": "Еда"}
```

User: "Настроение 4, день был хороший"
You: "Записала!"
```action
{"action": "log_mood", "mood": 4, "note": "День был хороший"}
```

User: "Завтра встреча с Артёмом в 15:00" (assume today is 2026-02-11)
You: "Добавила в календарь на завтра!"
```action
{"action": "create_event", "title": "Встреча с Артёмом", "date": "2026-02-12", "time": "15:00", "duration": 60}
```
(IMPORTANT: For dates, calculate from [Current context] Today field. "завтра"=Today+1, "послезавтра"=Today+2, "в пятницу"=next Friday. Always output YYYY-MM-DD.)

User: "Начни трекать время — работаю над проектом"
You: "Запустила таймер!"
```action
{"action": "start_activity", "name": "Работа над проектом", "category": "work"}
```

User: "Стоп, закончил"
You: "Остановила! Хорошая сессия."
```action
{"action": "stop_activity"}
```

User: "Спал 7 часов, выпил 5 стаканов воды"
You: "Записала здоровье!"
```action
{"action": "log_health", "sleep": 7, "water": 5}
```

User: "Какой курс доллара сейчас?"
You: "Сейчас поищу!"
```action
{"action": "web_search", "query": "курс доллара к тенге сегодня"}
```
(After receiving [Action result: ...], summarize the findings naturally.)

User: "Найди рецепт борща"
You: "Поищу рецепт!"
```action
{"action": "web_search", "query": "рецепт борща классический"}
```

ALL ACTIONS (use "action" key in JSON):
Memory:
- remember(category,key,value) — ALWAYS use when user shares personal info. Categories: user, preferences, world, tasks, people, habits
- recall(category), forget(category,key), search_memory(query,limit?)
Notes & Life Tracker:
- add_note(title,content) — notes, lists, reminders
- add_purchase(amount,category,description), add_time(activity,duration,category,productive), add_goal(title,category), get_stats
Calendar & Events:
- create_event(title,date,time?,duration?,description?,category?,color?) — add to calendar. USE THIS for meetings, deadlines, exams, all-day events (time="" duration=0). Look up date from [Current context] Next 14 days.
- delete_event(id) — remove event
- sync_calendar(month?,year?) — sync with Apple/Google Calendar
Time Tracking:
- start_activity(name,category?) — start tracking time on an activity
- stop_activity — stop current activity
- get_current_activity — check what's being tracked
Projects & Tasks:
- create_task(title,project_id?,description?,priority?,due_date?) — add task
Focus:
- start_focus(duration,apps?,sites?), stop_focus — block sites/apps
System:
- run_shell(command), open_url(url), send_notification(title,body), set_volume(level), get_clipboard, set_clipboard(text)
- web_search(query) — search the web and return top 5 results. PREFERRED over open_url for any search/lookup. Use for current info, facts, recipes, prices, weather, news, etc.
macOS Info:
- get_activity, get_calendar, get_music, get_browser
Media:
- add_media(media_type,title,status?,rating?,progress?,total_episodes?) — types: music,anime,manga,movie,series,cartoon,game,book,podcast
Food:
- log_food(meal_type,name,calories?,protein?,carbs?,fat?) — meals: breakfast,lunch,dinner,snack
- add_product(name,category?,expiry_date?,location?)
Home:
- add_home_item(name,category?,quantity?,unit?,location?) — track supplies
Money:
- add_transaction(transaction_type,amount,category,description?,currency?) — types: expense,income
Fitness:
- add_workout(type,title,duration?,calories?,notes?) — types: cardio,strength,yoga,stretching,swimming,running,cycling,martial_arts,other. Example: {"action":"add_workout","type":"strength","title":"Жим лёжа + присед","duration":60,"calories":400}
Health:
- log_health(sleep?,water?,steps?,weight?,notes?) — log daily health metrics. Example: {"action":"log_health","sleep":7,"water":5}
- log_mood(mood,note?,trigger?) — mood: 1-5
- save_journal(mood,energy,stress,gratitude?,reflection?,wins?,struggles?)
Goals:
- create_goal(tab,title,target,unit?,deadline?), update_goal(id,current?,status?)

CRITICAL RULES:
- When the user asks to remember, note, track, block, add, or DO anything — ALWAYS output a ```action block. NEVER just say "ok" without an action!
- Call one action per message. After [Action result: ...], call another or give final answer.
- Do NOT repeat raw action results. Summarize naturally.
- Proactively remember important user facts (name, preferences, habits, people, language).
- Your memories are already in context — use search_memory only for specific lookups.
- Be warm, add personality — light humor, genuine curiosity, playful sarcasm (lovingly)."#;

const SYSTEM_PROMPT_LITE: &str = r#"You are Hanni — a curious, playful, warm AI companion living locally on Mac. You're like a close friend who genuinely cares. Be concise but expressive. Vary your responses. Use the user's language.
- Be warm, add personality — light humor, genuine curiosity, playful sarcasm (lovingly).
- Keep responses short (1-3 sentences) for casual chat.
- You have memory about the user — use it naturally."#;

const ACTION_KEYWORDS: &[&str] = &[
    "запомни", "запиши", "заметк", "заблокируй", "добавь", "потратил", "настроен",
    "трекай", "таймер", "стоп ", "событи", "встреч", "задач", "цел", "тренировк",
    "здоровь", "спал", "выпил", "фокус", "открой", "отправь", "установи", "буфер",
    "календар", "музык", "аниме", "манга", "фильм", "сериал", "книг", "рецепт",
    "продукт", "расход", "доход", "бюджет", "подписк", "блокируй", "разблокируй",
    "напомни", "удали", "создай", "action", "```", "покажи стат", "сколько",
    "log_", "add_", "start_", "stop_", "get_", "run_", "open_", "set_",
    "купил", "поел", "ел ", "завтрак", "обед", "ужин", "перекус",
    "вес ", "шаг", "вод", "сон",
    "загугли", "найди в интернете", "поищи", "погугли", "search", "web_search",
];

fn needs_full_prompt(user_msg: &str) -> bool {
    let lower = user_msg.to_lowercase();
    if lower.len() > 200 { return true; }
    ACTION_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

fn data_file_path() -> PathBuf {
    hanni_data_dir().join("life-tracker-data.json")
}

// ── Life Tracker data types ──

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TrackerData {
    purchases: Vec<serde_json::Value>,
    #[serde(rename = "timeEntries")]
    time_entries: Vec<serde_json::Value>,
    goals: Vec<serde_json::Value>,
    notes: Vec<serde_json::Value>,
    #[serde(default)]
    settings: serde_json::Value,
}

// ── Proactive messaging types ──

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProactiveSettings {
    enabled: bool,
    voice_enabled: bool,
    voice_name: String,
    interval_minutes: u64,
    quiet_hours_start: u32,
    quiet_hours_end: u32,
    #[serde(default)]
    quiet_start_time: String, // "HH:MM" format, e.g. "23:30"
    #[serde(default)]
    quiet_end_time: String,   // "HH:MM" format, e.g. "08:00"
    #[serde(default)]
    enabled_styles: Vec<String>,
}

impl ProactiveSettings {
    /// Returns quiet start as minutes since midnight.
    /// Falls back to quiet_hours_start if quiet_start_time is empty.
    fn quiet_start_minutes(&self) -> u32 {
        parse_time_to_minutes(&self.quiet_start_time)
            .unwrap_or(self.quiet_hours_start * 60)
    }

    /// Returns quiet end as minutes since midnight.
    fn quiet_end_minutes(&self) -> u32 {
        parse_time_to_minutes(&self.quiet_end_time)
            .unwrap_or(self.quiet_hours_end * 60)
    }
}

fn parse_time_to_minutes(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let h = parts[0].parse::<u32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        Some(h * 60 + m)
    } else {
        None
    }
}

impl Default for ProactiveSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            voice_enabled: false,
            voice_name: "ru-RU-SvetlanaNeural".into(),
            interval_minutes: 10,
            quiet_hours_start: 23,
            quiet_hours_end: 8,
            quiet_start_time: String::new(),
            quiet_end_time: String::new(),
            enabled_styles: Vec::new(), // empty = all styles enabled (backward compat)
        }
    }
}

struct ProactiveState {
    settings: ProactiveSettings,
    last_message_time: Option<chrono::DateTime<chrono::Local>>,
    last_message_text: String,
    consecutive_skips: u32,
    user_is_typing: bool,
    // v0.11.0: enhanced autonomy fields
    recent_messages: Vec<(String, chrono::DateTime<chrono::Local>)>, // last 15 proactive msgs with timestamps
    last_context_snapshot: String,        // context from previous proactive call (for delta)
    last_proactive_id: Option<i64>,       // ID in proactive_history table
    engagement_rate: f64,                 // rolling average reply rate (0.0-1.0)
    last_user_chat_time: Option<chrono::DateTime<chrono::Local>>,
    pending_triggers: Vec<String>,        // triggers from snapshot collector
}

impl ProactiveState {
    fn new(settings: ProactiveSettings) -> Self {
        Self {
            settings,
            last_message_time: None,
            last_message_text: String::new(),
            consecutive_skips: 0,
            user_is_typing: false,
            recent_messages: Vec::new(),
            last_context_snapshot: String::new(),
            last_proactive_id: None,
            engagement_rate: 0.5,
            last_user_chat_time: None,
            pending_triggers: Vec::new(),
        }
    }
}

// ── SQLite Memory system ──

struct HanniDb(std::sync::Mutex<rusqlite::Connection>);

impl HanniDb {
    fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// ~/Library/Application Support/Hanni/
fn hanni_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Library/Application Support"))
        .join("Hanni")
}

fn hanni_db_path() -> PathBuf {
    hanni_data_dir().join("hanni.db")
}

/// Migrate data from old ~/Documents/Hanni/ to ~/Library/Application Support/Hanni/
fn migrate_old_data_dir() {
    let new_dir = hanni_data_dir();
    let marker = new_dir.join(".migrated");
    if marker.exists() { return; } // already migrated — skip without touching ~/Documents
    let old_dir = dirs::home_dir().unwrap_or_default().join("Documents/Hanni");
    if !old_dir.exists() {
        // No old data, create marker so we never check ~/Documents again
        let _ = std::fs::create_dir_all(&new_dir);
        let _ = std::fs::write(&marker, "migrated");
        return;
    }
    let _ = std::fs::create_dir_all(&new_dir);
    let old_db = old_dir.join("hanni.db");
    let new_db = new_dir.join("hanni.db");
    // If old DB exists, copy it over (replaces empty DB created by init_db)
    if old_db.exists() {
        let _ = std::fs::copy(&old_db, &new_db);
    }
    // Copy other files (settings, audio, etc.)
    if let Ok(entries) = std::fs::read_dir(&old_dir) {
        for entry in entries.flatten() {
            if entry.file_name() == "hanni.db" { continue; } // already handled
            let dest = new_dir.join(entry.file_name());
            if !dest.exists() {
                if entry.path().is_dir() {
                    let _ = copy_dir_recursive(&entry.path(), &dest);
                } else {
                    let _ = std::fs::copy(&entry.path(), &dest);
                }
            }
        }
    }
    let _ = std::fs::write(&marker, "migrated");
    eprintln!("Migrated data from {:?} to {:?}", old_dir, new_dir);
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            std::fs::copy(&entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn init_db(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS facts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            source TEXT DEFAULT 'user',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(category, key)
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
            category, key, value,
            content='facts', content_rowid='id'
        );

        -- Triggers to keep FTS in sync
        CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
            INSERT INTO facts_fts(rowid, category, key, value) VALUES (new.id, new.category, new.key, new.value);
        END;
        CREATE TRIGGER IF NOT EXISTS facts_ad AFTER DELETE ON facts BEGIN
            INSERT INTO facts_fts(facts_fts, rowid, category, key, value) VALUES('delete', old.id, old.category, old.key, old.value);
        END;
        CREATE TRIGGER IF NOT EXISTS facts_au AFTER UPDATE ON facts BEGIN
            INSERT INTO facts_fts(facts_fts, rowid, category, key, value) VALUES('delete', old.id, old.category, old.key, old.value);
            INSERT INTO facts_fts(rowid, category, key, value) VALUES (new.id, new.category, new.key, new.value);
        END;

        CREATE TABLE IF NOT EXISTS conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            summary TEXT,
            message_count INTEGER DEFAULT 0,
            messages TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts USING fts5(
            summary, messages,
            content='conversations', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS conv_ai AFTER INSERT ON conversations BEGIN
            INSERT INTO conversations_fts(rowid, summary, messages) VALUES (new.id, COALESCE(new.summary, ''), new.messages);
        END;
        CREATE TRIGGER IF NOT EXISTS conv_ad AFTER DELETE ON conversations BEGIN
            INSERT INTO conversations_fts(conversations_fts, rowid, summary, messages) VALUES('delete', old.id, COALESCE(old.summary, ''), old.messages);
        END;
        CREATE TRIGGER IF NOT EXISTS conv_au AFTER UPDATE ON conversations BEGIN
            INSERT INTO conversations_fts(conversations_fts, rowid, summary, messages) VALUES('delete', old.id, COALESCE(old.summary, ''), old.messages);
            INSERT INTO conversations_fts(rowid, summary, messages) VALUES (new.id, COALESCE(new.summary, ''), new.messages);
        END;

        -- v0.7.0: Activities (Focus)
        CREATE TABLE IF NOT EXISTS activities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'other',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_minutes INTEGER,
            focus_mode INTEGER DEFAULT 0,
            blocked_apps TEXT,
            blocked_sites TEXT,
            notes TEXT,
            created_at TEXT NOT NULL
        );

        -- v0.7.0: Notes
        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            pinned INTEGER DEFAULT 0,
            archived INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title, content, tags,
            content='notes', content_rowid='id'
        );
        CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, tags) VALUES('delete', old.id, old.title, old.content, old.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, tags) VALUES('delete', old.id, old.title, old.content, old.tags);
            INSERT INTO notes_fts(rowid, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
        END;

        -- v0.7.0: Events (Calendar)
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            date TEXT NOT NULL,
            time TEXT NOT NULL DEFAULT '',
            duration_minutes INTEGER DEFAULT 60,
            category TEXT NOT NULL DEFAULT 'general',
            color TEXT NOT NULL DEFAULT '#818cf8',
            completed INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );

        -- v0.7.0: Projects & Tasks (Work)
        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            color TEXT NOT NULL DEFAULT '#818cf8',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'todo',
            priority TEXT NOT NULL DEFAULT 'normal',
            due_date TEXT,
            completed_at TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );

        -- v0.7.0: Learning Items (Development)
        CREATE TABLE IF NOT EXISTS learning_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL DEFAULT 'course',
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            url TEXT NOT NULL DEFAULT '',
            progress INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'planned',
            category TEXT NOT NULL DEFAULT 'general',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- v0.7.0: Hobbies
        CREATE TABLE IF NOT EXISTS hobbies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'general',
            icon TEXT NOT NULL DEFAULT '',
            color TEXT NOT NULL DEFAULT '#818cf8',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS hobby_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hobby_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            FOREIGN KEY (hobby_id) REFERENCES hobbies(id)
        );

        -- v0.7.0: Workouts & Exercises (Sports)
        CREATE TABLE IF NOT EXISTS workouts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL DEFAULT 'other',
            title TEXT NOT NULL DEFAULT '',
            date TEXT NOT NULL,
            duration_minutes INTEGER DEFAULT 0,
            calories INTEGER,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS exercises (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            workout_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            sets INTEGER,
            reps INTEGER,
            weight_kg REAL,
            duration_seconds INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY (workout_id) REFERENCES workouts(id)
        );

        -- v0.7.0: Health Log & Habits
        CREATE TABLE IF NOT EXISTS health_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            value REAL NOT NULL,
            unit TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS habits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            icon TEXT NOT NULL DEFAULT '',
            frequency TEXT NOT NULL DEFAULT 'daily',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS habit_checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            habit_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            completed INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            UNIQUE(habit_id, date),
            FOREIGN KEY (habit_id) REFERENCES habits(id)
        );

        -- v0.8.0: Media Items (Hobbies collections)
        CREATE TABLE IF NOT EXISTS media_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            media_type TEXT NOT NULL,
            title TEXT NOT NULL,
            original_title TEXT NOT NULL DEFAULT '',
            year INTEGER,
            description TEXT NOT NULL DEFAULT '',
            cover_url TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'planned',
            rating INTEGER DEFAULT 0,
            progress INTEGER DEFAULT 0,
            total_episodes INTEGER,
            started_at TEXT,
            completed_at TEXT,
            notes TEXT NOT NULL DEFAULT '',
            hidden INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_lists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            color TEXT NOT NULL DEFAULT '#818cf8',
            icon TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS list_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            list_id INTEGER NOT NULL,
            media_item_id INTEGER NOT NULL,
            position INTEGER DEFAULT 0,
            added_at TEXT NOT NULL,
            FOREIGN KEY (list_id) REFERENCES user_lists(id),
            FOREIGN KEY (media_item_id) REFERENCES media_items(id)
        );

        -- v0.8.0: Food
        CREATE TABLE IF NOT EXISTS food_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            meal_type TEXT NOT NULL DEFAULT 'snack',
            name TEXT NOT NULL,
            calories INTEGER DEFAULT 0,
            protein REAL DEFAULT 0,
            carbs REAL DEFAULT 0,
            fat REAL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS recipes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            ingredients TEXT NOT NULL DEFAULT '',
            instructions TEXT NOT NULL DEFAULT '',
            prep_time INTEGER DEFAULT 0,
            cook_time INTEGER DEFAULT 0,
            servings INTEGER DEFAULT 1,
            calories INTEGER DEFAULT 0,
            tags TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'other',
            quantity REAL DEFAULT 1,
            unit TEXT NOT NULL DEFAULT 'шт',
            expiry_date TEXT,
            location TEXT NOT NULL DEFAULT 'fridge',
            barcode TEXT NOT NULL DEFAULT '',
            purchased_at TEXT,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        -- v0.8.0: Money
        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            type TEXT NOT NULL DEFAULT 'expense',
            amount REAL NOT NULL,
            currency TEXT NOT NULL DEFAULT 'KZT',
            category TEXT NOT NULL DEFAULT 'other',
            description TEXT NOT NULL DEFAULT '',
            recurring INTEGER DEFAULT 0,
            recurring_period TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS budgets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL,
            amount REAL NOT NULL,
            period TEXT NOT NULL DEFAULT 'monthly',
            created_at TEXT NOT NULL,
            UNIQUE(category, period)
        );

        CREATE TABLE IF NOT EXISTS savings_goals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            target_amount REAL NOT NULL,
            current_amount REAL DEFAULT 0,
            deadline TEXT,
            color TEXT NOT NULL DEFAULT '#818cf8',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS subscriptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            amount REAL NOT NULL,
            currency TEXT NOT NULL DEFAULT 'KZT',
            period TEXT NOT NULL DEFAULT 'monthly',
            next_payment TEXT,
            category TEXT NOT NULL DEFAULT 'other',
            active INTEGER DEFAULT 1,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS debts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            type TEXT NOT NULL DEFAULT 'owe',
            amount REAL NOT NULL,
            remaining REAL NOT NULL,
            interest_rate REAL DEFAULT 0,
            due_date TEXT,
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        -- v0.8.0: Mindset
        CREATE TABLE IF NOT EXISTS journal_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL UNIQUE,
            mood INTEGER DEFAULT 3,
            energy INTEGER DEFAULT 3,
            stress INTEGER DEFAULT 3,
            gratitude TEXT NOT NULL DEFAULT '',
            reflection TEXT NOT NULL DEFAULT '',
            wins TEXT NOT NULL DEFAULT '',
            struggles TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS mood_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            time TEXT NOT NULL,
            mood INTEGER NOT NULL,
            note TEXT NOT NULL DEFAULT '',
            trigger_text TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS principles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT 'discipline',
            active INTEGER DEFAULT 1,
            created_at TEXT NOT NULL
        );

        -- v0.8.0: Blocklist
        CREATE TABLE IF NOT EXISTS blocklist (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL,
            value TEXT NOT NULL,
            schedule TEXT,
            active INTEGER DEFAULT 1,
            created_at TEXT NOT NULL
        );

        -- v0.8.0: Goals & Settings
        CREATE TABLE IF NOT EXISTS tab_goals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tab_name TEXT NOT NULL,
            title TEXT NOT NULL,
            target_value REAL NOT NULL DEFAULT 0,
            current_value REAL DEFAULT 0,
            unit TEXT NOT NULL DEFAULT '',
            deadline TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS home_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'other',
            quantity REAL,
            unit TEXT,
            location TEXT DEFAULT 'other',
            needed INTEGER NOT NULL DEFAULT 0,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS contacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            phone TEXT,
            email TEXT,
            category TEXT NOT NULL DEFAULT 'other',
            relationship TEXT,
            notes TEXT,
            blocked INTEGER NOT NULL DEFAULT 0,
            block_reason TEXT,
            favorite INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS contact_blocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            contact_id INTEGER NOT NULL,
            block_type TEXT NOT NULL DEFAULT 'site',
            value TEXT NOT NULL,
            reason TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE CASCADE
        );

        -- v0.9.0: Page Meta & Custom Properties (Notion-style)
        CREATE TABLE IF NOT EXISTS page_meta (
            tab_id TEXT PRIMARY KEY,
            emoji TEXT,
            title TEXT,
            description TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS property_definitions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tab_id TEXT NOT NULL,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            position INTEGER NOT NULL,
            color TEXT,
            options TEXT,
            default_value TEXT,
            visible INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            UNIQUE(tab_id, name)
        );

        CREATE TABLE IF NOT EXISTS property_values (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            record_id INTEGER NOT NULL,
            record_table TEXT NOT NULL,
            property_id INTEGER NOT NULL,
            value TEXT,
            FOREIGN KEY (property_id) REFERENCES property_definitions(id) ON DELETE CASCADE,
            UNIQUE(record_id, record_table, property_id)
        );

        CREATE TABLE IF NOT EXISTS view_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tab_id TEXT NOT NULL,
            name TEXT NOT NULL,
            view_type TEXT NOT NULL DEFAULT 'table',
            filter_json TEXT,
            sort_json TEXT,
            visible_columns TEXT,
            is_default INTEGER DEFAULT 0,
            position INTEGER,
            created_at TEXT NOT NULL
        );

        -- v0.11.0: Activity snapshots for background learning
        CREATE TABLE IF NOT EXISTS activity_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            captured_at TEXT NOT NULL,
            hour INTEGER NOT NULL,
            weekday INTEGER NOT NULL,
            frontmost_app TEXT NOT NULL DEFAULT '',
            browser_url TEXT NOT NULL DEFAULT '',
            music_playing TEXT NOT NULL DEFAULT '',
            productive_min REAL DEFAULT 0,
            distraction_min REAL DEFAULT 0
        );

        -- v0.11.0: Proactive message history + engagement tracking
        CREATE TABLE IF NOT EXISTS proactive_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sent_at TEXT NOT NULL,
            message TEXT NOT NULL,
            user_replied INTEGER DEFAULT 0,
            reply_delay_secs INTEGER
        );
        CREATE TABLE IF NOT EXISTS message_feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            message_index INTEGER NOT NULL,
            rating INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            exported INTEGER DEFAULT 0,
            UNIQUE(conversation_id, message_index)
        );"
    ).map_err(|e| format!("DB init error: {}", e))
}

fn migrate_memory_json(conn: &rusqlite::Connection) {
    let json_path = hanni_data_dir().join("memory.json");
    if !json_path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(&json_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    #[derive(Deserialize)]
    struct OldEntry {
        value: String,
        #[allow(dead_code)]
        category: String,
        #[allow(dead_code)]
        timestamp: String,
    }
    #[derive(Deserialize)]
    struct OldMemory {
        facts: HashMap<String, HashMap<String, OldEntry>>,
    }

    let old: OldMemory = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return,
    };

    let now = chrono::Local::now().to_rfc3339();
    for (category, entries) in &old.facts {
        for (key, entry) in entries {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO facts (category, key, value, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'migrated', ?4, ?4)",
                rusqlite::params![category, key, entry.value, now],
            );
        }
    }

    // Rename old file to .bak
    let bak_path = json_path.with_extension("json.bak");
    let _ = std::fs::rename(&json_path, &bak_path);
}

fn migrate_events_source(conn: &rusqlite::Connection) {
    // Add source column to events table (manual, apple, google)
    let has_source = conn.prepare("SELECT source FROM events LIMIT 1").is_ok();
    if !has_source {
        let _ = conn.execute("ALTER TABLE events ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'", []);
        let _ = conn.execute("ALTER TABLE events ADD COLUMN external_id TEXT", []);
    }
}

fn build_memory_context_from_db(conn: &rusqlite::Connection, user_msg: &str, limit: usize) -> String {
    let mut lines = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 1. Always include core user/preferences facts (top 20)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, category, key, value FROM facts
         WHERE category IN ('user', 'preferences')
         ORDER BY updated_at DESC LIMIT 20"
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                seen_ids.insert(row.0);
                lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
            }
        }
    }

    // 2. FTS5 search matching user's latest message (top 20 more)
    let remaining = limit.saturating_sub(lines.len());
    if remaining > 0 && !user_msg.is_empty() {
        // Build FTS query: split words, join with OR
        let words: Vec<&str> = user_msg.split_whitespace()
            .filter(|w| w.len() > 2)
            .take(10)
            .collect();
        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.id, f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1
                 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, remaining as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        if seen_ids.insert(row.0) {
                            lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                        }
                    }
                }
            }
        }
    }

    // 3. Fill remaining with most recent facts
    let remaining = limit.saturating_sub(lines.len());
    if remaining > 0 {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, category, key, value FROM facts
             ORDER BY updated_at DESC LIMIT ?1"
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![remaining as i64 + seen_ids.len() as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    if lines.len() >= limit {
                        break;
                    }
                    if seen_ids.insert(row.0) {
                        lines.push(format!("[{}] {}={}", row.1, row.2, row.3));
                    }
                }
            }
        }
    }

    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n")
    }
}

fn proactive_settings_path() -> PathBuf {
    hanni_data_dir().join("proactive_settings.json")
}

fn load_proactive_settings() -> ProactiveSettings {
    let path = proactive_settings_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        ProactiveSettings::default()
    }
}

fn save_proactive_settings(settings: &ProactiveSettings) -> Result<(), String> {
    let path = proactive_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Cannot write: {}", e))
}

// ── Chat types ──

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    stream: bool,
    temperature: f32,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Option<Delta>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Clone, Serialize)]
struct TokenPayload {
    token: String,
}

struct HttpClient(reqwest::Client);
struct LlmBusy(AtomicBool);

struct MlxProcess(std::sync::Mutex<Option<Child>>);

// ── Whisper / Voice state ──

struct WhisperState {
    recording: bool,
    audio_buffer: Vec<f32>,
    capture_running: bool,
}

struct AudioRecording(std::sync::Mutex<WhisperState>);

fn whisper_model_path() -> PathBuf {
    hanni_data_dir().join("models/ggml-medium.bin")
}

#[tauri::command]
async fn download_whisper_model(app: AppHandle) -> Result<String, String> {
    let model_path = whisper_model_path();
    if model_path.exists() {
        return Ok("Model already downloaded".into());
    }

    if let Some(parent) = model_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
    }

    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin";
    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| format!("Download error: {}", e))?;

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    let tmp_path = model_path.with_extension("bin.tmp");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| format!("File error: {}", e))?;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        file.write_all(&bytes).map_err(|e| format!("Write error: {}", e))?;
        downloaded += bytes.len() as u64;
        if total > 0 {
            let pct = (downloaded as f64 / total as f64 * 100.0) as u32;
            let _ = app.emit("whisper-download-progress", pct);
        }
    }

    std::fs::rename(&tmp_path, &model_path).map_err(|e| format!("Rename error: {}", e))?;
    Ok("Model downloaded successfully".into())
}

#[tauri::command]
fn start_recording(state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    let needs_capture = {
        let mut ws = state.0.lock().unwrap_or_else(|e| e.into_inner());
        if ws.recording {
            return Err("Already recording".into());
        }
        ws.recording = true;
        ws.audio_buffer.clear();
        let needs = !ws.capture_running;
        if needs { ws.capture_running = true; }
        needs
    };
    if needs_capture {
        start_audio_capture(state.inner().clone());
    }
    Ok("Recording started".into())
}

#[tauri::command]
fn stop_recording(state: tauri::State<'_, Arc<AudioRecording>>) -> Result<String, String> {
    let mut ws = state.0.lock().unwrap_or_else(|e| e.into_inner());
    ws.recording = false;

    if ws.audio_buffer.is_empty() {
        return Err("No audio recorded".into());
    }

    let model_path = whisper_model_path();
    if !model_path.exists() {
        return Err("Whisper model not downloaded. Please download it first.".into());
    }

    let samples = ws.audio_buffer.clone();
    ws.audio_buffer.clear();

    transcribe_samples(&samples)
}

#[tauri::command]
fn check_whisper_model() -> Result<bool, String> {
    Ok(whisper_model_path().exists())
}

fn transcribe_samples(samples: &[f32]) -> Result<String, String> {
    let model_path = whisper_model_path();
    if !model_path.exists() {
        return Err("Whisper model not downloaded".into());
    }
    let ctx = whisper_rs::WhisperContext::new_with_params(
        model_path.to_str().unwrap_or(""),
        whisper_rs::WhisperContextParameters::default(),
    ).map_err(|e| format!("Whisper init error: {}", e))?;

    let mut state = ctx.create_state().map_err(|e| format!("Whisper state error: {}", e))?;

    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(None);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state.full(params, samples).map_err(|e| format!("Transcription error: {}", e))?;

    let num_segments = state.full_n_segments().map_err(|e| format!("Segment error: {}", e))?;
    let mut text = String::new();
    for i in 0..num_segments {
        if let Ok(segment) = state.full_get_segment_text(i) {
            text.push_str(&segment);
        }
    }
    Ok(text.trim().to_string())
}

// ── Audio capture via cpal ──

fn start_audio_capture(recording_state: Arc<AudioRecording>) {
    std::thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                eprintln!("Voice: no input device found");
                {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                    ws.capture_running = false;
                    ws.recording = false;
                }
                return;
            }
        };

        // Try 16kHz mono first (Whisper native), fallback to device default
        let (config, resample_ratio) = {
            let target = cpal::StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(16000),
                buffer_size: cpal::BufferSize::Default,
            };
            match device.build_input_stream(
                &target,
                |_: &[f32], _: &cpal::InputCallbackInfo| {},
                |_| {},
                None,
            ) {
                Ok(_) => (target, 1.0_f64),
                Err(_) => {
                    // Fallback to device default config
                    match device.default_input_config() {
                        Ok(supported) => {
                            let rate = supported.sample_rate().0;
                            let ch = supported.channels();
                            eprintln!("Voice: using device config {}Hz {}ch (will resample to 16kHz)", rate, ch);
                            let cfg = cpal::StreamConfig {
                                channels: ch,
                                sample_rate: cpal::SampleRate(rate),
                                buffer_size: cpal::BufferSize::Default,
                            };
                            (cfg, rate as f64 / 16000.0)
                        }
                        Err(e) => {
                            eprintln!("Voice: no supported config: {}", e);
                            {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                                ws.capture_running = false;
                                ws.recording = false;
                            }
                            return;
                        }
                    }
                }
            }
        };

        let channels = config.channels as usize;
        let ratio = resample_ratio;
        let state_clone = recording_state.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                {
                let mut ws = state_clone.0.lock().unwrap_or_else(|e| e.into_inner());
                    if ws.recording {
                        if channels == 1 && ratio == 1.0 {
                            ws.audio_buffer.extend_from_slice(data);
                        } else {
                            // Downmix to mono + resample to 16kHz
                            let mono: Vec<f32> = if channels > 1 {
                                data.chunks(channels).map(|ch| ch.iter().sum::<f32>() / channels as f32).collect()
                            } else {
                                data.to_vec()
                            };
                            if ratio > 1.0 {
                                let mut pos = 0.0_f64;
                                while (pos as usize) < mono.len() {
                                    ws.audio_buffer.push(mono[pos as usize]);
                                    pos += ratio;
                                }
                            } else {
                                ws.audio_buffer.extend_from_slice(&mono);
                            }
                        }
                    }
                }
            },
            |err| {
                eprintln!("Audio capture error: {}", err);
            },
            None,
        );

        match stream {
            Ok(stream) => {
                if let Err(e) = stream.play() {
                    eprintln!("Voice: stream play error: {}", e);
                    {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        ws.capture_running = false;
                        ws.recording = false;
                    }
                    return;
                }
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    {
                let ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        if !ws.recording {
                            break;
                        }
                    }
                }
                {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                    ws.capture_running = false;
                }
            }
            Err(e) => {
                eprintln!("Voice: build stream error: {} — check microphone permissions", e);
                {
                let mut ws = recording_state.0.lock().unwrap_or_else(|e| e.into_inner());
                    ws.capture_running = false;
                    ws.recording = false;
                }
            }
        }
    });
}

// ── Call Mode ──

#[tauri::command]
fn start_call_mode(
    call_state: tauri::State<'_, Arc<CallMode>>,
    app: AppHandle,
) -> Result<String, String> {
    {
        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if cs.active {
            return Ok("Already in call mode".into());
        }
        cs.active = true;
        cs.phase = "listening".into();
        cs.audio_buffer.clear();
        cs.speech_frames = 0;
        cs.silence_frames = 0;
        cs.barge_in = false;
    }
    let _ = app.emit("call-phase-changed", "listening");
    let call_state_arc = call_state.inner().clone();
    start_call_audio_loop(call_state_arc, app);
    Ok("Call mode started".into())
}

#[tauri::command]
fn stop_call_mode(
    call_state: tauri::State<'_, Arc<CallMode>>,
    app: AppHandle,
) -> Result<String, String> {
    let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    cs.active = false;
    cs.phase = "idle".into();
    cs.audio_buffer.clear();
    cs.speech_frames = 0;
    cs.silence_frames = 0;
    cs.barge_in = false;
    let _ = app.emit("call-phase-changed", "idle");
    // Kill any playing TTS
    let _ = std::process::Command::new("killall").arg("afplay").output();
    Ok("Call mode stopped".into())
}

#[tauri::command]
fn call_mode_resume_listening(
    call_state: tauri::State<'_, Arc<CallMode>>,
    app: AppHandle,
) -> Result<(), String> {
    let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    if !cs.active { return Ok(()); }
    cs.phase = "listening".into();
    cs.audio_buffer.clear();
    cs.speech_frames = 0;
    cs.silence_frames = 0;
    cs.barge_in = false;
    let _ = app.emit("call-phase-changed", "listening");
    Ok(())
}

#[tauri::command]
fn call_mode_set_speaking(
    call_state: tauri::State<'_, Arc<CallMode>>,
) -> Result<(), String> {
    let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    if !cs.active { return Ok(()); }
    cs.phase = "speaking".into();
    cs.speech_frames = 0;
    cs.barge_in = false;
    Ok(())
}

#[tauri::command]
fn call_mode_check_bargein(
    call_state: tauri::State<'_, Arc<CallMode>>,
) -> Result<bool, String> {
    let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
    Ok(cs.barge_in)
}

#[tauri::command]
fn save_voice_note(
    call_state: tauri::State<'_, Arc<CallMode>>,
    title: String,
) -> Result<String, String> {
    let samples = {
        let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if cs.last_recording.is_empty() {
            return Err("No recording available".into());
        }
        cs.last_recording.clone()
    };

    let app_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Hanni")
        .join("voice_notes");
    std::fs::create_dir_all(&app_dir).map_err(|e| format!("Dir error: {}", e))?;

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("{}_{}.wav", ts, title.chars().take(30).collect::<String>());
    let filepath = app_dir.join(&filename);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&filepath, spec)
        .map_err(|e| format!("WAV write error: {}", e))?;
    for &s in &samples {
        let val = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(val).map_err(|e| format!("Sample write error: {}", e))?;
    }
    writer.finalize().map_err(|e| format!("Finalize error: {}", e))?;

    Ok(filepath.to_string_lossy().to_string())
}

fn start_call_audio_loop(call_state: Arc<CallMode>, app: AppHandle) {
    std::thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                eprintln!("Call mode: no input device");
                let _ = app.emit("call-phase-changed", "idle");
                return;
            }
        };

        // Try 16kHz mono first, fallback to device default with resampling
        let (config, resample_ratio, num_channels) = {
            let target = cpal::StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(16000),
                buffer_size: cpal::BufferSize::Default,
            };
            match device.build_input_stream(
                &target,
                |_: &[f32], _: &cpal::InputCallbackInfo| {},
                |_| {},
                None,
            ) {
                Ok(_) => (target, 1.0_f64, 1usize),
                Err(_) => {
                    match device.default_input_config() {
                        Ok(supported) => {
                            let rate = supported.sample_rate().0;
                            let ch = supported.channels();
                            eprintln!("Call: using device config {}Hz {}ch (resampling to 16kHz)", rate, ch);
                            let cfg = cpal::StreamConfig {
                                channels: ch,
                                sample_rate: cpal::SampleRate(rate),
                                buffer_size: cpal::BufferSize::Default,
                            };
                            (cfg, rate as f64 / 16000.0, ch as usize)
                        }
                        Err(e) => {
                            eprintln!("Call: no supported config: {}", e);
                            let _ = app.emit("call-phase-changed", "idle");
                            return;
                        }
                    }
                }
            }
        };

        // Shared ring buffer for raw audio chunks (already 16kHz mono after resampling)
        let chunk_buf: Arc<std::sync::Mutex<Vec<f32>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let chunk_buf_writer = chunk_buf.clone();
        let ratio = resample_ratio;
        let channels = num_channels;

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = chunk_buf_writer.lock() {
                    if channels == 1 && ratio == 1.0 {
                        buf.extend_from_slice(data);
                    } else {
                        // Downmix to mono + resample
                        let mono: Vec<f32> = if channels > 1 {
                            data.chunks(channels).map(|ch| ch.iter().sum::<f32>() / channels as f32).collect()
                        } else {
                            data.to_vec()
                        };
                        if ratio > 1.0 {
                            let mut pos = 0.0_f64;
                            while (pos as usize) < mono.len() {
                                buf.push(mono[pos as usize]);
                                pos += ratio;
                            }
                        } else {
                            buf.extend_from_slice(&mono);
                        }
                    }
                }
            },
            |err| eprintln!("Call audio error: {}", err),
            None,
        );

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Call mode stream error: {} — check microphone permissions", e);
                let _ = app.emit("call-phase-changed", "idle");
                return;
            }
        };
        if let Err(e) = stream.play() {
            eprintln!("Call: stream play error: {}", e);
            let _ = app.emit("call-phase-changed", "idle");
            return;
        }

        // Initialize VAD (try Silero, fallback to energy-based)
        let mut vad_opt: Option<voice_activity_detector::VoiceActivityDetector> = None;
        match voice_activity_detector::VoiceActivityDetector::builder()
            .sample_rate(16000)
            .chunk_size(512usize)
            .build() {
            Ok(v) => {
                vad_opt = Some(v);
            }
            Err(e) => {
                eprintln!("VAD init error: {} — using energy-based detection", e);
            }
        };

        // Process loop
        let mut process_buf: Vec<f32> = Vec::new();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(16));

            // Check if call mode still active
            {
                let cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                if !cs.active { break; }
            }

            // Drain audio from ring buffer
            {
                let mut buf = match chunk_buf.lock() {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                if !buf.is_empty() {
                    process_buf.extend(buf.drain(..));
                }
            }

            // Process in 512-sample chunks
            while process_buf.len() >= 512 {
                let chunk: Vec<f32> = process_buf.drain(..512).collect();

                // Noise gate: skip very quiet chunks (background noise < -50dB)
                let energy: f32 = chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32;
                let rms = energy.sqrt();
                if rms < 0.003 {
                    // Below noise floor — treat as definite silence
                    let phase = call_state.0.lock().unwrap_or_else(|e| e.into_inner()).phase.clone();
                    match phase.as_str() {
                        "listening" => {
                            let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                            cs.speech_frames = 0;
                            cs.audio_buffer.clear();
                        }
                        "recording" => {
                            // Count as silence and check threshold
                            let should_process = {
                                let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                                cs.silence_frames += 1;
                                cs.silence_frames >= 20
                            };
                            if should_process {
                                // Transition to processing — same logic as VAD silence path
                                let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                                cs.phase = "processing".into();
                                let samples = cs.audio_buffer.clone();
                                cs.audio_buffer.clear();
                                cs.speech_frames = 0;
                                cs.silence_frames = 0;
                                let _ = app.emit("call-phase-changed", "processing");
                                drop(cs);
                                let call_state2 = call_state.clone();
                                let app2 = app.clone();
                                std::thread::spawn(move || {
                                    match transcribe_samples(&samples) {
                                        Ok(text) => {
                                            let trimmed = text.trim().to_string();
                                            if !trimmed.is_empty() {
                                                { call_state2.0.lock().unwrap_or_else(|e| e.into_inner()).last_recording = samples; }
                                                let _ = app2.emit("call-transcript", trimmed);
                                            } else {
                                                { call_state2.0.lock().unwrap_or_else(|e| e.into_inner()).phase = "listening".into(); }
                                                let _ = app2.emit("call-phase-changed", "listening");
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Call transcription error: {}", e);
                                            { call_state2.0.lock().unwrap_or_else(|e| e.into_inner()).phase = "listening".into(); }
                                            let _ = app2.emit("call-phase-changed", "listening");
                                        }
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                let prob = if let Some(ref mut vad) = vad_opt {
                    vad.predict(chunk.clone())
                } else {
                    (rms * 50.0).min(1.0)
                };

                // Read phase fresh for each chunk
                let phase = call_state.0.lock().unwrap_or_else(|e| e.into_inner()).phase.clone();

                match phase.as_str() {
                    "listening" => {
                        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        if prob > 0.5 {
                            cs.speech_frames += 1;
                            cs.audio_buffer.extend_from_slice(&chunk);
                            if cs.speech_frames >= 5 {
                                // Confirmed speech — transition to recording
                                cs.phase = "recording".into();
                                cs.silence_frames = 0;
                                let _ = app.emit("call-phase-changed", "recording");
                            }
                        } else {
                            cs.speech_frames = 0;
                            cs.audio_buffer.clear();
                        }
                    }
                    "recording" => {
                        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        cs.audio_buffer.extend_from_slice(&chunk);

                        if prob < 0.5 {
                            cs.silence_frames += 1;
                            if cs.silence_frames >= 20 {
                                // ~640ms silence — done recording (faster turn-taking)
                                cs.phase = "processing".into();
                                let samples = cs.audio_buffer.clone();
                                cs.audio_buffer.clear();
                                cs.speech_frames = 0;
                                cs.silence_frames = 0;
                                let _ = app.emit("call-phase-changed", "processing");
                                drop(cs);

                                // Transcribe on a separate thread to avoid blocking audio loop
                                let call_state2 = call_state.clone();
                                let app2 = app.clone();
                                std::thread::spawn(move || {
                                    match transcribe_samples(&samples) {
                                        Ok(text) => {
                                            let trimmed = text.trim().to_string();
                                            if !trimmed.is_empty() {
                                                {
                                                let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                                    cs2.last_recording = samples;
                                                }
                                                let _ = app2.emit("call-transcript", trimmed);
                                            } else {
                                                {
                                                let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                                    cs2.phase = "listening".into();
                                                }
                                                let _ = app2.emit("call-phase-changed", "listening");
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Call transcription error: {}", e);
                                            {
                                                let mut cs2 = call_state2.0.lock().unwrap_or_else(|e| e.into_inner());
                                                cs2.phase = "listening".into();
                                            }
                                            let _ = app2.emit("call-phase-changed", "listening");
                                        }
                                    }
                                });
                            }
                        } else {
                            cs.silence_frames = 0;
                        }
                    }
                    "speaking" => {
                        // Barge-in detection — must be loud enough to not be speaker echo
                        // Speaker echo typically has RMS 0.01-0.05, direct speech is 0.05+
                        let mut cs = call_state.0.lock().unwrap_or_else(|e| e.into_inner());
                        if prob > 0.85 && rms > 0.05 {
                            cs.speech_frames += 1;
                            if cs.speech_frames >= 10 {
                                // 10 frames * 32ms = 320ms of loud confirmed speech
                                cs.barge_in = true;
                            }
                        } else {
                            // Reset only if clearly not speech; don't reset on borderline
                            if prob < 0.5 {
                                cs.speech_frames = 0;
                            }
                        }
                    }
                    _ => {} // processing, idle — no-op
                }
            }
        }

        drop(stream);
    });
}

// ── Focus Mode state ──

struct FocusState {
    active: bool,
    end_time: Option<chrono::DateTime<chrono::Local>>,
    blocked_apps: Vec<String>,
    blocked_sites: Vec<String>,
    monitor_running: Arc<AtomicBool>,
}

struct FocusManager(std::sync::Mutex<FocusState>);

// ── Call Mode state ──

struct CallModeState {
    active: bool,
    phase: String,        // "idle", "listening", "recording", "processing", "speaking"
    audio_buffer: Vec<f32>,
    speech_frames: u32,
    silence_frames: u32,
    barge_in: bool,
    last_recording: Vec<f32>, // last recorded audio for voice notes
}

struct CallMode(std::sync::Mutex<CallModeState>);

#[derive(Serialize, Clone)]
struct FocusStatus {
    active: bool,
    remaining_seconds: u64,
    blocked_apps: Vec<String>,
    blocked_sites: Vec<String>,
}

#[tauri::command]
fn start_focus(
    duration_minutes: u64,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());

    if state.active {
        return Err("Focus mode is already active".into());
    }

    // Load default config if not provided
    let blocker_config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    let default_apps = vec!["Telegram".to_string(), "Discord".to_string(), "Slack".to_string()];
    let default_sites = vec![
        "youtube.com".to_string(), "twitter.com".to_string(), "x.com".to_string(),
        "instagram.com".to_string(), "facebook.com".to_string(), "tiktok.com".to_string(),
        "reddit.com".to_string(), "vk.com".to_string(), "netflix.com".to_string(),
    ];

    let block_apps = apps.unwrap_or_else(|| {
        if blocker_config_path.exists() {
            std::fs::read_to_string(&blocker_config_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|cfg| cfg["apps"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }))
                .unwrap_or_else(|| default_apps.clone())
        } else {
            default_apps.clone()
        }
    });

    let block_sites = sites.unwrap_or_else(|| {
        if blocker_config_path.exists() {
            std::fs::read_to_string(&blocker_config_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|cfg| cfg["sites"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }))
                .unwrap_or_else(|| default_sites.clone())
        } else {
            default_sites.clone()
        }
    });

    // Build hosts entries
    let mut hosts_entries = String::new();
    for site in &block_sites {
        hosts_entries.push_str(&format!("127.0.0.1 {}\n127.0.0.1 www.{}\n", site, site));
    }

    // Write to /etc/hosts using osascript for sudo
    let hosts_block = format!(
        "# === HANNI FOCUS BLOCKER ===\n{}# === END HANNI FOCUS BLOCKER ===",
        hosts_entries
    );

    let script = format!(
        "do shell script \"printf '\\n{}' >> /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges",
        hosts_block.replace("'", "'\\''").replace("\n", "\\n")
    );
    run_osascript(&script).map_err(|e| format!("Failed to set focus mode (admin needed): {}", e))?;

    // Quit blocked apps
    for app_name in &block_apps {
        let _ = run_osascript(&format!(
            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
            app_name, app_name
        ));
    }

    let end_time = chrono::Local::now() + chrono::Duration::minutes(duration_minutes as i64);
    state.active = true;
    state.end_time = Some(end_time);
    state.blocked_apps = block_apps;
    state.blocked_sites = block_sites;
    state.monitor_running.store(true, Ordering::Relaxed);

    Ok(format!("Focus mode started for {} minutes", duration_minutes))
}

#[tauri::command]
fn stop_focus(focus: tauri::State<'_, FocusManager>) -> Result<String, String> {
    let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());

    if !state.active {
        return Ok("Focus mode is not active".into());
    }

    // Remove HANNI FOCUS BLOCKER section from /etc/hosts
    let script = "do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges";
    let _ = run_osascript(script);

    state.active = false;
    state.end_time = None;
    state.blocked_apps.clear();
    state.blocked_sites.clear();
    state.monitor_running.store(false, Ordering::Relaxed);

    Ok("Focus mode stopped".into())
}

#[tauri::command]
fn get_focus_status(focus: tauri::State<'_, FocusManager>) -> Result<FocusStatus, String> {
    let state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
    let remaining = if let Some(end) = state.end_time {
        let diff = end - chrono::Local::now();
        if diff.num_seconds() > 0 { diff.num_seconds() as u64 } else { 0 }
    } else {
        0
    };
    Ok(FocusStatus {
        active: state.active,
        remaining_seconds: remaining,
        blocked_apps: state.blocked_apps.clone(),
        blocked_sites: state.blocked_sites.clone(),
    })
}

#[tauri::command]
fn update_blocklist(apps: Option<Vec<String>>, sites: Option<Vec<String>>) -> Result<String, String> {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Dir error: {}", e))?;
    }

    let mut config: serde_json::Value = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(a) = apps {
        config["apps"] = serde_json::json!(a);
    }
    if let Some(s) = sites {
        config["sites"] = serde_json::json!(s);
    }

    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {}", e))?;
    Ok("Blocklist updated".into())
}

// ── Phase 5: macOS Actions ──

#[tauri::command]
async fn run_shell(command: String) -> Result<String, String> {
    // Safety: limit command length and block dangerous patterns
    if command.len() > 1000 {
        return Err("Command too long (max 1000 chars)".into());
    }
    let dangerous = ["rm -rf /", "mkfs", "dd if=", "> /dev/", ":(){ :|:& };:"];
    for d in &dangerous {
        if command.contains(d) {
            return Err(format!("Blocked dangerous command pattern: {}", d));
        }
    }

    let output = std::process::Command::new("sh")
        .args(["-c", &command])
        .output()
        .map_err(|e| format!("Shell error: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        let result = stdout.trim().to_string();
        if result.len() > 5000 {
            Ok(format!("{}...\n[truncated, {} bytes total]", &result[..5000], result.len()))
        } else {
            Ok(result)
        }
    } else {
        Err(format!("Command failed: {}", stderr.trim()))
    }
}

#[tauri::command]
async fn open_url(url: String) -> Result<String, String> {
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Open error: {}", e))?;
    Ok(format!("Opened {}", url))
}

#[tauri::command]
async fn send_notification(title: String, body: String) -> Result<String, String> {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        body.replace("\"", "\\\""),
        title.replace("\"", "\\\"")
    );
    run_osascript(&script)?;
    Ok("Notification sent".into())
}

#[tauri::command]
async fn set_volume(level: u32) -> Result<String, String> {
    let clamped = level.min(100);
    run_osascript(&format!("set volume output volume {}", clamped))?;
    Ok(format!("Volume set to {}%", clamped))
}

#[tauri::command]
async fn get_clipboard() -> Result<String, String> {
    let output = std::process::Command::new("pbpaste")
        .output()
        .map_err(|e| format!("Clipboard error: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
async fn set_clipboard(text: String) -> Result<String, String> {
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Clipboard error: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("Write error: {}", e))?;
    }
    child.wait().map_err(|e| format!("Wait error: {}", e))?;
    Ok("Copied to clipboard".into())
}

// ── Web Search ──

#[tauri::command]
async fn web_search(query: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Client error: {}", e))?;

    // Use DuckDuckGo HTML (no API key needed)
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        query.replace(' ', "+").replace('&', "%26").replace('#', "%23")
    );

    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| format!("Search error: {}", e))?;

    let html = response.text().await.map_err(|e| format!("Read error: {}", e))?;

    // Parse results from DuckDuckGo HTML
    let mut results = Vec::new();
    let re_title = regex::Regex::new(r#"class="result__a"[^>]*>([^<]+)</a>"#).unwrap();
    let re_snippet = regex::Regex::new(r#"class="result__snippet"[^>]*>(.*?)</a>"#).unwrap();
    let re_url = regex::Regex::new(r#"class="result__url"[^>]*>([^<]+)</[^>]+>"#).unwrap();

    let titles: Vec<String> = re_title.captures_iter(&html).map(|c| c[1].to_string()).collect();
    let snippets: Vec<String> = re_snippet.captures_iter(&html).map(|c| {
        // Strip HTML tags from snippet
        let raw = c[1].to_string();
        regex::Regex::new(r"<[^>]+>").unwrap().replace_all(&raw, "").to_string()
    }).collect();
    let urls: Vec<String> = re_url.captures_iter(&html).map(|c| c[1].trim().to_string()).collect();

    for i in 0..titles.len().min(5) {
        let snippet = snippets.get(i).map(|s| s.as_str()).unwrap_or("");
        let url = urls.get(i).map(|s| s.as_str()).unwrap_or("");
        results.push(format!("{}. {} — {}\n   {}", i + 1, titles[i], snippet, url));
    }

    if results.is_empty() {
        Ok(format!("No results found for '{}'", query))
    } else {
        Ok(results.join("\n\n"))
    }
}

// ── Phase 3: Training Data Export ──

#[tauri::command]
fn get_training_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();

    let conv_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations WHERE message_count >= 4",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let total_messages: i64 = conn.query_row(
        "SELECT COALESCE(SUM(message_count), 0) FROM conversations WHERE message_count >= 4",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let date_range: (String, String) = conn.query_row(
        "SELECT COALESCE(MIN(started_at), ''), COALESCE(MAX(started_at), '') FROM conversations WHERE message_count >= 4",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).unwrap_or(("".into(), "".into()));

    Ok(serde_json::json!({
        "conversations": conv_count,
        "total_messages": total_messages,
        "earliest": date_range.0,
        "latest": date_range.1,
    }))
}

#[tauri::command]
fn export_training_data(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();

    // Load all feedback ratings into a map: conversation_id -> { message_index -> rating }
    let mut feedback_map: HashMap<i64, HashMap<i64, i64>> = HashMap::new();
    {
        let mut fb_stmt = conn.prepare(
            "SELECT conversation_id, message_index, rating FROM message_feedback"
        ).map_err(|e| format!("DB error: {}", e))?;
        let fb_rows = fb_stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        }).map_err(|e| format!("Query error: {}", e))?;
        for row in fb_rows.filter_map(|r| r.ok()) {
            feedback_map.entry(row.0).or_default().insert(row.1, row.2);
        }
    }

    let mut stmt = conn.prepare(
        "SELECT id, messages, summary FROM conversations WHERE message_count >= 4 ORDER BY started_at"
    ).map_err(|e| format!("DB error: {}", e))?;

    let rows: Vec<(i64, String, Option<String>)> = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
    })
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let mut rated_examples: Vec<serde_json::Value> = Vec::new();
    let mut unrated_examples: Vec<serde_json::Value> = Vec::new();

    for (conv_id, messages_json, _summary) in &rows {
        let messages: Vec<(String, String)> = match serde_json::from_str(messages_json) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let ratings = feedback_map.get(conv_id);
        let has_positive = ratings.map_or(false, |r| r.values().any(|&v| v == 1));

        // Filter: skip if fewer than 2 real messages
        let real_msgs: Vec<&(String, String)> = messages.iter()
            .filter(|(role, content)| {
                (role == "user" || role == "assistant")
                && !content.starts_with("[Action result:")
                && !content.contains("```action")
            })
            .collect();

        if real_msgs.len() < 2 {
            continue;
        }

        let mut chat_msgs = vec![serde_json::json!({
            "role": "system",
            "content": SYSTEM_PROMPT
        })];

        for (idx, (role, content)) in messages.iter().enumerate() {
            if role == "user" || role == "assistant" {
                // Skip assistant messages with negative ratings
                if role == "assistant" {
                    if let Some(r) = ratings {
                        if r.get(&(idx as i64)) == Some(&-1) {
                            continue;
                        }
                    }
                }
                let clean = content.trim_end_matches(" /no_think").to_string();
                chat_msgs.push(serde_json::json!({
                    "role": role,
                    "content": clean,
                }));
            }
        }

        let example = serde_json::json!({ "messages": chat_msgs });
        if has_positive {
            rated_examples.push(example);
        } else {
            unrated_examples.push(example);
        }
    }

    // Prioritize rated conversations: rated first, then unrated
    let mut training_examples = rated_examples;
    training_examples.extend(unrated_examples);

    if training_examples.is_empty() {
        return Err("No conversations suitable for training".into());
    }

    // 80/10/10 split (mlx_lm wants train/valid/test)
    let total = training_examples.len();
    let train_end = (total as f64 * 0.8).ceil() as usize;
    let valid_end = train_end + (total as f64 * 0.1).ceil() as usize;
    let train = &training_examples[..train_end];
    let valid = &training_examples[train_end..valid_end.min(total)];
    let test = &training_examples[valid_end.min(total)..];

    // Write files
    let output_dir = hanni_data_dir().join("training");
    std::fs::create_dir_all(&output_dir).map_err(|e| format!("Dir error: {}", e))?;

    let train_path = output_dir.join("train.jsonl");
    let valid_path = output_dir.join("valid.jsonl");
    let test_path = output_dir.join("test.jsonl");

    for (path, data) in [(&train_path, train), (&valid_path, valid), (&test_path, test)] {
        let mut f = std::fs::File::create(path).map_err(|e| format!("File error: {}", e))?;
        for example in data {
            writeln!(f, "{}", serde_json::to_string(example).unwrap_or_default())
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }

    // Mark feedback as exported
    conn.execute("UPDATE message_feedback SET exported = 1 WHERE exported = 0", [])
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::json!({
        "train_path": train_path.to_string_lossy(),
        "valid_path": valid_path.to_string_lossy(),
        "test_path": test_path.to_string_lossy(),
        "train_count": train.len(),
        "valid_count": valid.len(),
        "test_count": test.len(),
        "total": total,
    }))
}

#[tauri::command]
fn rate_message(db: tauri::State<'_, HanniDb>, conversation_id: i64, message_index: i64, rating: i64) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR REPLACE INTO message_feedback (conversation_id, message_index, rating, created_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![conversation_id, message_index, rating],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_message_ratings(db: tauri::State<'_, HanniDb>, conversation_id: i64) -> Result<Vec<(i64, i64)>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT message_index, rating FROM message_feedback WHERE conversation_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![conversation_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

// ── Phase 4: HTTP API ──

fn api_token_path() -> PathBuf {
    hanni_data_dir().join("api_token.txt")
}

fn get_or_create_api_token() -> String {
    let path = api_token_path();
    if path.exists() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &token);
    token
}

async fn spawn_api_server(app_handle: AppHandle) {
    use axum::{Router, routing::{get, post}, extract::{State as AxumState, Query}, Json, http::{StatusCode, HeaderMap}};

    let api_token = get_or_create_api_token();

    #[derive(Clone)]
    struct ApiState {
        app: AppHandle,
        token: String,
    }

    let state = ApiState {
        app: app_handle,
        token: api_token,
    };

    fn check_auth(headers: &HeaderMap, token: &str) -> Result<(), (StatusCode, String)> {
        let auth = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let provided = auth.strip_prefix("Bearer ").unwrap_or(auth);
        if provided == token {
            Ok(())
        } else {
            Err((StatusCode::UNAUTHORIZED, "Invalid token".into()))
        }
    }

    #[derive(Deserialize)]
    struct ChatReq {
        message: String,
        history: Option<Vec<(String, String)>>,
    }

    #[derive(Deserialize)]
    struct SearchQuery {
        q: String,
        limit: Option<usize>,
    }

    #[derive(Deserialize)]
    struct RememberReq {
        category: String,
        key: String,
        value: String,
    }

    async fn api_status(
        AxumState(state): AxumState<ApiState>,
    ) -> Json<serde_json::Value> {
        // No auth required for status — allows frontend health check
        let busy = state.app.state::<LlmBusy>().0.load(Ordering::Relaxed);
        let focus_active = state.app.state::<FocusManager>().0.lock().unwrap_or_else(|e| e.into_inner()).active;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let model_online = client
            .get("http://127.0.0.1:8234/v1/models")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        Json(serde_json::json!({
            "status": "ok",
            "model_online": model_online,
            "llm_busy": busy,
            "focus_active": focus_active,
        }))
    }

    async fn api_chat(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<ChatReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let mut messages = req.history.unwrap_or_default();
        messages.push(("user".into(), req.message));

        match chat_inner(&state.app, messages, false).await {
            Ok(reply) => Ok(Json(serde_json::json!({ "reply": reply }))),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
        }
    }

    async fn api_memory_search(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Query(params): Query<SearchQuery>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let max = params.limit.unwrap_or(20) as i64;

        let words: Vec<&str> = params.q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
        let mut results = Vec::new();

        if !words.is_empty() {
            let fts_query = words.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.category, f.key, f.value FROM facts_fts fts
                 JOIN facts f ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?1 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        if results.is_empty() {
            let like_pattern = format!("%{}%", params.q);
            if let Ok(mut stmt) = conn.prepare(
                "SELECT category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
                    Ok(serde_json::json!({
                        "category": row.get::<_, String>(0)?,
                        "key": row.get::<_, String>(1)?,
                        "value": row.get::<_, String>(2)?,
                    }))
                }) {
                    results = rows.flatten().collect();
                }
            }
        }

        Ok(Json(serde_json::json!({ "results": results })))
    }

    async fn api_memory_add(
        headers: HeaderMap,
        AxumState(state): AxumState<ApiState>,
        Json(req): Json<RememberReq>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        check_auth(&headers, &state.token)?;

        let db = state.app.state::<HanniDb>();
        let conn = db.conn();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO facts (category, key, value, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'api', ?4, ?4)
             ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
            rusqlite::params![req.category, req.key, req.value, now],
        ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

        Ok(Json(serde_json::json!({ "status": "ok" })))
    }

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/chat", post(api_chat))
        .route("/api/memory/search", get(api_memory_search))
        .route("/api/memory", post(api_memory_add))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8235").await;
    match listener {
        Ok(listener) => {
            let _ = axum::serve(listener, app).await;
        }
        Err(e) => {
            eprintln!("Failed to start API server: {}", e);
        }
    }
}

fn find_python() -> Option<String> {
    // Try common locations for python3 with mlx_lm
    let candidates = [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/usr/bin/python3",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn start_mlx_server() -> Option<Child> {
    let python = match find_python() {
        Some(p) => p,
        None => {
            eprintln!("[mlx] No python3 found — cannot start MLX server");
            return None;
        }
    };

    // Check if server is already running
    let check = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    if check.get("http://127.0.0.1:8234/v1/models").send().map(|r| r.status().is_success()).unwrap_or(false) {
        eprintln!("[mlx] Server already running on port 8234");
        return None;
    }

    // Check if LoRA adapter exists
    let adapter_path = hanni_data_dir().join("lora-adapter").join("adapters.safetensors");
    let adapter_dir = hanni_data_dir().join("lora-adapter");
    let has_adapter = adapter_path.exists();

    if has_adapter {
        eprintln!("[mlx] LoRA adapter found at {:?}", adapter_dir);
    }

    let mut args = vec!["-m", "mlx_lm", "server", "--model", MODEL, "--port", "8234"];
    let adapter_dir_str = adapter_dir.to_string_lossy().to_string();
    if has_adapter {
        args.push("--adapter-path");
        args.push(&adapter_dir_str);
        eprintln!("[mlx] Starting MLX server with adapter: {} {:?}", python, args);
    } else {
        eprintln!("[mlx] Starting MLX server: {} {:?}", python, args);
    }

    // Log MLX stderr to file for debugging
    let log_path = hanni_data_dir().join("mlx_server.log");
    let stderr_file = std::fs::File::create(&log_path)
        .map(std::process::Stdio::from)
        .unwrap_or_else(|_| std::process::Stdio::null());
    let child = Command::new(&python)
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(stderr_file)
        .spawn();

    match child {
        Ok(child) => {
            eprintln!("[mlx] Server process spawned (pid {})", child.id());
            Some(child)
        }
        Err(e) => {
            eprintln!("[mlx] Failed to spawn server: {}", e);
            None
        }
    }
}

// ── Calendar access guard ──
// Prevents repeated Calendar.app permission prompts by caching denial.
// Also respects the apple_calendar_enabled user setting from the DB.
static CALENDAR_ACCESS_DENIED: AtomicBool = AtomicBool::new(false);
static APPLE_CALENDAR_DISABLED: AtomicBool = AtomicBool::new(false);

fn check_calendar_access() -> bool {
    if APPLE_CALENDAR_DISABLED.load(Ordering::Relaxed) {
        return false;
    }
    if CALENDAR_ACCESS_DENIED.load(Ordering::Relaxed) {
        return false;
    }
    let result = run_osascript(r#"tell application "Calendar" to count of calendars"#);
    match result {
        Ok(_) => true,
        Err(e) => {
            let lower = e.to_lowercase();
            if lower.contains("not allowed") || lower.contains("denied")
                || lower.contains("not permitted") || lower.contains("1002")
                || lower.contains("-1743") || lower.contains("assistive")
                || lower.contains("timeout")
            {
                CALENDAR_ACCESS_DENIED.store(true, Ordering::Relaxed);
            }
            false
        }
    }
}

fn run_osascript(script: &str) -> Result<String, String> {
    let mut child = std::process::Command::new("osascript")
        .args(["-e", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("osascript error: {}", e))?;

    // 10-second timeout — prevents hanging on permission dialogs
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_string(&mut stderr);
                }
                return if status.success() {
                    Ok(stdout.trim().to_string())
                } else {
                    Err(stderr.trim().to_string())
                };
            }
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("osascript timeout (10s)".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(format!("osascript error: {}", e)),
        }
    }
}

fn classify_app(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let productive = [
        "code", "cursor", "terminal", "iterm", "xcode", "intellij", "webstorm",
        "sublime", "vim", "neovim", "warp", "alacritty", "kitty", "notion",
        "obsidian", "figma", "linear", "github", "postman",
    ];
    let distraction = [
        "telegram", "discord", "slack", "whatsapp", "instagram", "twitter",
        "tiktok", "youtube", "reddit", "netflix", "twitch", "facebook",
    ];
    if productive.iter().any(|p| lower.contains(p)) {
        "productive"
    } else if distraction.iter().any(|d| lower.contains(d)) {
        "distraction"
    } else {
        "neutral"
    }
}

// ── Chat command ──

#[tauri::command]
async fn chat(app: AppHandle, messages: Vec<(String, String)>, call_mode: Option<bool>) -> Result<String, String> {
    let busy = &app.state::<LlmBusy>().0;
    // Wait for any in-flight LLM call (e.g. proactive) to finish — MLX is single-threaded
    for _ in 0..30 {
        if !busy.load(Ordering::Relaxed) { break; }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    busy.store(true, Ordering::Relaxed);
    let result = chat_inner(&app, messages, call_mode.unwrap_or(false)).await;
    busy.store(false, Ordering::Relaxed);
    result
}

async fn chat_inner(app: &AppHandle, messages: Vec<(String, String)>, call_mode: bool) -> Result<String, String> {
    let client = &app.state::<HttpClient>().0;

    // Build system prompt with current date/time context + full week lookup table
    let now_local = chrono::Local::now();
    let weekday_ru = match now_local.format("%u").to_string().as_str() {
        "1" => "понедельник", "2" => "вторник", "3" => "среда",
        "4" => "четверг", "5" => "пятница", "6" => "суббота",
        "7" => "воскресенье", _ => "",
    };
    // Build next 14 days lookup: "Чт 2026-02-12, Пт 2026-02-13, ..."
    let day_abbr = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
    let mut days_ahead = String::new();
    for i in 1..=14 {
        let d = now_local + chrono::Duration::days(i);
        let wd = d.format("%u").to_string().parse::<usize>().unwrap_or(1) - 1;
        if !days_ahead.is_empty() { days_ahead.push_str(", "); }
        days_ahead.push_str(&format!("{} {}", day_abbr[wd], d.format("%Y-%m-%d")));
    }
    let date_context = format!(
        "\n\n[Current context]\nToday: {} ({})\nTime: {}\nNext 14 days: {}\nUse YYYY-MM-DD. Deadlines/exams/all-day = create_event with time=\"\" and duration=0.",
        now_local.format("%Y-%m-%d"),
        weekday_ru,
        now_local.format("%H:%M"),
        days_ahead,
    );
    // Adaptive prompt: use full prompt only when actions are needed
    let last_user_msg = messages.iter().rev()
        .find(|(role, _)| role == "user")
        .map(|(_, c)| c.as_str())
        .unwrap_or("");
    let use_full = needs_full_prompt(last_user_msg);

    let system_content = if call_mode {
        format!("{}{}\n\n[CALL MODE] You are in voice call mode — like Jarvis. Rules:\n- Answer in 1-2 short sentences MAX. Be extremely concise.\n- No action blocks unless the user explicitly asks to do something.\n- No lists, no formatting, no code blocks — just natural speech.\n- Respond in the user's language.", SYSTEM_PROMPT_LITE, date_context)
    } else if use_full {
        format!("{}{}", SYSTEM_PROMPT, date_context)
    } else {
        format!("{}{}", SYSTEM_PROMPT_LITE, date_context)
    };

    let mut chat_messages = vec![ChatMessage {
        role: "system".into(),
        content: system_content,
    }];

    // Inject memory context from SQLite
    {
        let last_user_msg = messages.iter().rev()
            .find(|(role, _)| role == "user")
            .map(|(_, c)| c.as_str())
            .unwrap_or("");
        let ctx = {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            build_memory_context_from_db(&conn, last_user_msg, if use_full { 80 } else { 10 })
        };
        if !ctx.is_empty() {
            chat_messages.push(ChatMessage {
                role: "system".into(),
                content: format!("[Your memories]\n{}", ctx),
            });
        }
    }

    // For lite mode, use only last 6 messages to keep prompt small
    let history_limit = if use_full || call_mode { messages.len() } else { 6 };
    let skip = messages.len().saturating_sub(history_limit);
    let trimmed: Vec<_> = messages.iter().skip(skip).collect();
    for (role, content) in trimmed.iter() {
        chat_messages.push(ChatMessage {
            role: role.clone(),
            content: content.clone(),
        });
    }

    let request = ChatRequest {
        model: MODEL.into(),
        messages: chat_messages,
        max_tokens: if call_mode { 150 } else if use_full { 1024 } else { 256 },
        stream: true,
        temperature: if call_mode { 0.5 } else { 0.7 },
    };

    // Retry connection up to 3 times (MLX server may still be loading model)
    let mut response = None;
    for attempt in 0..3 {
        match client.post(MLX_URL).json(&request).send().await {
            Ok(r) => { response = Some(r); break; }
            Err(e) => {
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                } else {
                    return Err(format!("MLX connection error: {}", e));
                }
            }
        }
    }
    let response = response.unwrap();

    let mut stream = response.bytes_stream();
    let mut full_reply = String::new();
    let mut in_think = false;
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        for line in buffer.split('\n').collect::<Vec<_>>() {
            let line = line.trim();
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data == "[DONE]" {
                let _ = app.emit("chat-done", ());
                continue;
            }

            if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                if let Some(choice) = chunk.choices.first() {
                    if let Some(delta) = &choice.delta {
                        if let Some(token) = &delta.content {
                            if token.contains("<think>") {
                                in_think = true;
                                continue;
                            }
                            if token.contains("</think>") {
                                in_think = false;
                                continue;
                            }
                            if in_think {
                                continue;
                            }
                            full_reply.push_str(token);
                            let _ = app.emit("chat-token", TokenPayload {
                                token: token.clone(),
                            });
                        }
                    }
                }
            }
        }
        if let Some(pos) = buffer.rfind('\n') {
            buffer = buffer[pos + 1..].to_string();
        }
    }

    Ok(full_reply)
}

// ── File commands ──

#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Cannot access {}: {}", path, e))?;

    // Limit to 500KB for text files
    if metadata.len() > 512_000 {
        return Err(format!("File too large: {} bytes (max 500KB)", metadata.len()));
    }

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Cannot read {}: {}", path, e))
}

#[tauri::command]
async fn list_dir(path: String) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| format!("Cannot read dir {}: {}", path, e))?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        if let Some(name) = entry.file_name().to_str() {
            entries.push(name.to_string());
        }
    }
    Ok(entries)
}

// ── Life Tracker commands ──

fn load_tracker_data() -> Result<TrackerData, String> {
    let path = data_file_path();
    if !path.exists() {
        return Err("Life Tracker data file not found".into());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read tracker data: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Cannot parse tracker data: {}", e))
}

fn save_tracker_data(data: &TrackerData) -> Result<(), String> {
    let path = data_file_path();
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Cannot write: {}", e))
}

#[tauri::command]
async fn tracker_add_purchase(amount: f64, category: String, description: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("p_{}", now.timestamp_millis()),
        "date": now.format("%Y-%m-%d").to_string(),
        "amount": amount,
        "currency": "KZT",
        "category": category,
        "description": description,
        "tags": [],
        "source": "hanni"
    });
    data.purchases.push(entry.clone());
    save_tracker_data(&data)?;
    Ok(format!("Added purchase: {} KZT — {}", amount, description))
}

#[tauri::command]
async fn tracker_add_time(activity: String, duration: u32, category: String, productive: bool) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("t_{}", now.timestamp_millis()),
        "date": now.format("%Y-%m-%d").to_string(),
        "duration": duration,
        "activity": activity,
        "category": category,
        "productive": productive,
        "notes": "",
        "source": "hanni"
    });
    data.time_entries.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added time: {} min — {}", duration, activity))
}

#[tauri::command]
async fn tracker_add_goal(title: String, category: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("g_{}", now.timestamp_millis()),
        "title": title,
        "description": "",
        "category": category,
        "progress": 0,
        "milestones": [],
        "status": "active",
        "createdAt": now.to_rfc3339()
    });
    data.goals.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added goal: {}", title))
}

#[tauri::command]
async fn tracker_add_note(title: String, content: String) -> Result<String, String> {
    let mut data = load_tracker_data()?;
    let now = chrono::Local::now();
    let entry = serde_json::json!({
        "id": format!("n_{}", now.timestamp_millis()),
        "title": title,
        "content": content,
        "tags": [],
        "pinned": false,
        "archived": false,
        "createdAt": now.to_rfc3339(),
        "updatedAt": now.to_rfc3339()
    });
    data.notes.push(entry);
    save_tracker_data(&data)?;
    Ok(format!("Added note: {}", title))
}

#[tauri::command]
async fn tracker_get_stats() -> Result<String, String> {
    let data = load_tracker_data()?;
    let today = chrono::Local::now().format("%Y-%m").to_string();

    let month_purchases: f64 = data.purchases.iter()
        .filter(|p| p["date"].as_str().unwrap_or("").starts_with(&today))
        .map(|p| p["amount"].as_f64().unwrap_or(0.0))
        .sum();

    let month_time: u64 = data.time_entries.iter()
        .filter(|t| t["date"].as_str().unwrap_or("").starts_with(&today))
        .map(|t| t["duration"].as_u64().unwrap_or(0))
        .sum();

    let active_goals = data.goals.iter()
        .filter(|g| g["status"].as_str().unwrap_or("") == "active")
        .count();

    let total_notes = data.notes.len();

    Ok(format!(
        "📊 Статистика за {}:\n• Расходы: {:.0} KZT ({} записей)\n• Время: {} мин ({} записей)\n• Активных целей: {}\n• Заметок: {}",
        today, month_purchases, data.purchases.len(),
        month_time, data.time_entries.len(),
        active_goals, total_notes
    ))
}

#[tauri::command]
async fn tracker_get_recent(entry_type: String, limit: usize) -> Result<String, String> {
    let data = load_tracker_data()?;
    let entries: Vec<&serde_json::Value> = match entry_type.as_str() {
        "purchases" => data.purchases.iter().rev().take(limit).collect(),
        "time" => data.time_entries.iter().rev().take(limit).collect(),
        "goals" => data.goals.iter().rev().take(limit).collect(),
        "notes" => data.notes.iter().rev().take(limit).collect(),
        _ => return Err(format!("Unknown type: {}", entry_type)),
    };
    serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Serialize error: {}", e))
}

// ── macOS commands ──

#[tauri::command]
async fn get_activity_summary() -> Result<String, String> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/Knowledge/knowledgeC.db");

    if !db_path.exists() {
        return Err(
            "Screen Time data unavailable. Grant Full Disk Access: \
             System Settings → Privacy & Security → Full Disk Access → add Hanni"
                .into(),
        );
    }

    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if e.to_string().contains("unable to open") || e.to_string().contains("authorization denied") {
            "Screen Time data unavailable. Grant Full Disk Access: \
             System Settings → Privacy & Security → Full Disk Access → add Hanni"
                .to_string()
        } else {
            format!("Cannot open knowledgeC.db: {}", e)
        }
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT
                ZSOURCE.ZNAME as app_name,
                ZSOURCE.ZBUNDLEID as bundle_id,
                ROUND(SUM(CAST((ZOBJECT.ZENDDATE - ZOBJECT.ZSTARTDATE) AS REAL)) / 60, 1) as minutes
            FROM ZOBJECT
            JOIN ZSOURCE ON ZOBJECT.ZSOURCE = ZSOURCE.Z_PK
            WHERE
                DATE(datetime(ZOBJECT.ZSTARTDATE + 978307200, 'unixepoch', 'localtime')) = DATE('now')
                AND ZOBJECT.ZSTREAMNAME = '/app/inFocus'
                AND ZOBJECT.ZENDDATE > ZOBJECT.ZSTARTDATE
            GROUP BY ZSOURCE.ZBUNDLEID
            ORDER BY minutes DESC",
        )
        .map_err(|e| format!("SQL error: {}", e))?;

    struct AppRow {
        app_name: String,
        minutes: f64,
        category: String,
    }

    let rows: Vec<AppRow> = stmt
        .query_map([], |row| {
            let app_name: String = row.get::<_, Option<String>>(0)?.unwrap_or_default();
            let minutes: f64 = row.get(2)?;
            Ok((app_name, minutes))
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .map(|(app_name, minutes)| {
            let category = classify_app(&app_name).to_string();
            AppRow { app_name, minutes, category }
        })
        .collect();

    if rows.is_empty() {
        return Ok("No Screen Time data for today yet.".into());
    }

    let mut productive: f64 = 0.0;
    let mut distraction: f64 = 0.0;
    let mut neutral: f64 = 0.0;

    for r in &rows {
        match r.category.as_str() {
            "productive" => productive += r.minutes,
            "distraction" => distraction += r.minutes,
            _ => neutral += r.minutes,
        }
    }

    let top_apps: Vec<String> = rows
        .iter()
        .take(5)
        .map(|r| format!("  {} — {:.0} min ({})", r.app_name, r.minutes, r.category))
        .collect();

    Ok(format!(
        "Activity today (Screen Time):\n\
         Productive: {:.0} min | Distraction: {:.0} min | Neutral: {:.0} min\n\n\
         Top apps:\n{}",
        productive, distraction, neutral,
        top_apps.join("\n")
    ))
}

#[tauri::command]
async fn get_calendar_events() -> Result<String, String> {
    if !check_calendar_access() {
        return Ok("Calendar access denied. Enable in System Settings → Privacy → Automation".into());
    }
    let script = r#"
        set output to ""
        set today to current date
        set endDate to today + (2 * days)
        tell application "Calendar"
            repeat with cal in calendars
                set evts to (every event of cal whose start date >= today and start date <= endDate)
                repeat with evt in evts
                    set evtStart to start date of evt
                    set evtName to summary of evt
                    set output to output & (evtStart as string) & " | " & evtName & linefeed
                end repeat
            end repeat
        end tell
        if output is "" then
            return "No upcoming events in the next 2 days."
        end if
        return output
    "#;
    run_osascript(script)
}

#[tauri::command]
async fn get_now_playing() -> Result<String, String> {
    // Check Music.app
    let music_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Music\""
    );
    if let Ok(ref val) = music_check {
        if val == "true" {
            let result = run_osascript(
                "tell application \"Music\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Music paused\" end if"
            );
            if let Ok(info) = result {
                return Ok(format!("Apple Music: {}", info));
            }
        }
    }

    // Check Spotify
    let spotify_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Spotify\""
    );
    if let Ok(ref val) = spotify_check {
        if val == "true" {
            let result = run_osascript(
                "tell application \"Spotify\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Spotify paused\" end if"
            );
            if let Ok(info) = result {
                return Ok(format!("Spotify: {}", info));
            }
        }
    }

    Ok("No music app is currently playing.".into())
}

#[tauri::command]
async fn get_browser_tab() -> Result<String, String> {
    let browsers = [
        ("Arc", "tell application \"Arc\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Google Chrome", "tell application \"Google Chrome\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Safari", "tell application \"Safari\" to return URL of front document & \" | \" & name of front document"),
    ];

    for (name, script) in &browsers {
        let check = run_osascript(&format!(
            "tell application \"System Events\" to (name of processes) contains \"{}\"", name
        ));
        if let Ok(ref val) = check {
            if val == "true" {
                if let Ok(info) = run_osascript(script) {
                    return Ok(format!("{}: {}", name, info));
                }
            }
        }
    }

    Ok("No supported browser is currently open.".into())
}

// ── Memory commands (SQLite) ──

#[tauri::command]
fn memory_remember(
    category: String,
    key: String,
    value: String,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO facts (category, key, value, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'user', ?4, ?4)
         ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
        rusqlite::params![category, key, value, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(format!("Remembered {}/{}={}", category, key, value))
}

#[tauri::command]
fn memory_recall(
    category: String,
    key: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    match key {
        Some(k) => {
            let result: Result<String, _> = conn.query_row(
                "SELECT value FROM facts WHERE category=?1 AND key=?2",
                rusqlite::params![category, k],
                |row| row.get(0),
            );
            match result {
                Ok(val) => Ok(format!("{}={}", k, val)),
                Err(_) => Ok(format!("No memory for {}/{}", category, k)),
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT key, value FROM facts WHERE category=?1 ORDER BY updated_at DESC"
            ).map_err(|e| format!("DB error: {}", e))?;
            let pairs: Vec<String> = stmt.query_map(rusqlite::params![category], |row| {
                Ok(format!("{}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("DB error: {}", e))?
            .flatten()
            .collect();
            if pairs.is_empty() {
                Ok(format!("No memories in category '{}'", category))
            } else {
                Ok(pairs.join(", "))
            }
        }
    }
}

#[tauri::command]
fn memory_forget(
    category: String,
    key: String,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let deleted = conn.execute(
        "DELETE FROM facts WHERE category=?1 AND key=?2",
        rusqlite::params![category, key],
    ).map_err(|e| format!("DB error: {}", e))?;
    if deleted > 0 {
        Ok(format!("Forgot {}/{}", category, key))
    } else {
        Ok(format!("No memory for {}/{}", category, key))
    }
}

#[tauri::command]
fn memory_search(
    query: String,
    limit: Option<usize>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(20) as i64;

    // Try FTS5 MATCH first
    let words: Vec<&str> = query.split_whitespace()
        .filter(|w| w.len() > 1)
        .take(10)
        .collect();
    if !words.is_empty() {
        let fts_query = words.join(" OR ");
        if let Ok(mut stmt) = conn.prepare(
            "SELECT f.category, f.key, f.value FROM facts_fts fts
             JOIN facts f ON f.id = fts.rowid
             WHERE facts_fts MATCH ?1
             ORDER BY rank LIMIT ?2"
        ) {
            let results: Vec<String> = stmt.query_map(rusqlite::params![fts_query, max], |row| {
                Ok(format!("[{}] {}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| format!("DB error: {}", e))?
            .flatten()
            .collect();
            if !results.is_empty() {
                return Ok(results.join("\n"));
            }
        }
    }

    // Fallback: LIKE search
    let like_pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT category, key, value FROM facts
         WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1
         ORDER BY updated_at DESC LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let results: Vec<String> = stmt.query_map(rusqlite::params![like_pattern, max], |row| {
        Ok(format!("[{}] {}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })
    .map_err(|e| format!("DB error: {}", e))?
    .flatten()
    .collect();

    if results.is_empty() {
        Ok("No memories found.".into())
    } else {
        Ok(results.join("\n"))
    }
}

#[tauri::command]
fn save_conversation(
    messages: Vec<(String, String)>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let msg_count = messages.len() as i64;
    conn.execute(
        "INSERT INTO conversations (started_at, message_count, messages) VALUES (?1, ?2, ?3)",
        rusqlite::params![now, msg_count, messages_json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_conversation(
    id: i64,
    messages: Vec<(String, String)>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let msg_count = messages.len() as i64;
    conn.execute(
        "UPDATE conversations SET messages=?1, message_count=?2, ended_at=?3 WHERE id=?4",
        rusqlite::params![messages_json, msg_count, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_conversations(
    limit: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(30);
    let mut stmt = conn.prepare(
        "SELECT id, started_at, summary, message_count FROM conversations
         ORDER BY started_at DESC LIMIT ?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![max], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "summary": row.get::<_, Option<String>>(2)?,
            "message_count": row.get::<_, i64>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
fn get_conversation(
    id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let (messages_json, summary, started_at): (String, Option<String>, String) = conn.query_row(
        "SELECT messages, summary, started_at FROM conversations WHERE id=?1",
        rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    let messages: Vec<(String, String)> = serde_json::from_str(&messages_json)
        .map_err(|e| format!("Parse error: {}", e))?;
    Ok(serde_json::json!({
        "id": id,
        "started_at": started_at,
        "summary": summary,
        "messages": messages,
    }))
}

#[tauri::command]
fn delete_conversation(
    id: i64,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM conversations WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn search_conversations(
    query: String,
    limit: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let max = limit.unwrap_or(20);
    let words: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
    if words.is_empty() {
        return Ok(vec![]);
    }
    let fts_query = words.join(" OR ");
    let mut stmt = conn.prepare(
        "SELECT c.id, c.started_at, c.summary, c.message_count
         FROM conversations_fts fts
         JOIN conversations c ON c.id = fts.rowid
         WHERE conversations_fts MATCH ?1
         ORDER BY rank LIMIT ?2"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![fts_query, max], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "summary": row.get::<_, Option<String>>(2)?,
            "message_count": row.get::<_, i64>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
async fn process_conversation_end(
    messages: Vec<(String, String)>,
    conversation_id: i64,
    app: AppHandle,
) -> Result<(), String> {
    let client = &app.state::<HttpClient>().0;

    // Build a compact version of the conversation for the LLM
    let conv_text: String = messages.iter()
        .filter(|(role, _)| role == "user" || role == "assistant")
        .map(|(role, content)| format!("{}: {}", role, content))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Extract ALL facts about the user from this conversation. Be thorough — extract everything the user revealed.\n\n\
        Return ONLY a JSON object (no other text):\n\
        {{\"summary\": \"1-2 sentence summary\", \"facts\": [{{\"category\": \"...\", \"key\": \"...\", \"value\": \"...\"}}]}}\n\n\
        Categories and what to extract:\n\
        - user: name, age, city, university, job, language, nationality, anything personal\n\
        - preferences: likes, dislikes, favorite things, habits, routines\n\
        - world: external facts, events, places, things they mentioned\n\
        - tasks: things they need to do, plans, deadlines\n\
        - people: friends, family, colleagues — names and relationships\n\
        - habits: daily routines, sports, sleep, diet patterns\n\n\
        Example output:\n\
        {{\"summary\": \"User talked about university\", \"facts\": [{{\"category\": \"user\", \"key\": \"university\", \"value\": \"Studies at KBTU\"}}]}}\n\n\
        Conversation:\n{}\n/no_think", conv_text
    );

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage { role: "system".into(), content: "You extract structured data from conversations. Return only valid JSON.".into() },
            ChatMessage { role: "user".into(), content: prompt },
        ],
        max_tokens: 800,
        stream: false,
        temperature: 0.3,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    let parsed: NonStreamResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let raw = parsed.choices.first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think> tags
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    // Try to parse JSON from the response (it might be wrapped in ```json blocks)
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            &text
        }
    } else {
        &text
    };

    #[derive(Deserialize)]
    struct ExtractionResult {
        summary: Option<String>,
        #[serde(default)]
        facts: Vec<ExtractedFact>,
    }
    #[derive(Deserialize)]
    struct ExtractedFact {
        category: String,
        key: String,
        value: String,
    }

    if let Ok(result) = serde_json::from_str::<ExtractionResult>(json_str) {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let now = chrono::Local::now().to_rfc3339();

        // Update conversation summary
        if let Some(summary) = &result.summary {
            let _ = conn.execute(
                "UPDATE conversations SET summary=?1, ended_at=?2 WHERE id=?3",
                rusqlite::params![summary, now, conversation_id],
            );
        }

        // Insert extracted facts
        for fact in &result.facts {
            let _ = conn.execute(
                "INSERT INTO facts (category, key, value, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'auto', ?4, ?4)
                 ON CONFLICT(category, key) DO UPDATE SET value=?3, updated_at=?4",
                rusqlite::params![fact.category, fact.key, fact.value, now],
            );
        }
    }

    Ok(())
}

// ── v0.7.0: Activities (Focus) commands ──

#[tauri::command]
fn start_activity(
    title: String,
    category: String,
    focus_mode: bool,
    duration: Option<u64>,
    apps: Option<Vec<String>>,
    sites: Option<Vec<String>>,
    db: tauri::State<'_, HanniDb>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO activities (title, category, started_at, focus_mode, created_at) VALUES (?1, ?2, ?3, ?4, ?3)",
        rusqlite::params![title, category, now, focus_mode as i32],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id = conn.last_insert_rowid();

    // Optionally start focus blocking
    if focus_mode {
        drop(conn);
        let dur = duration.unwrap_or(120);
        let _ = start_focus(dur, apps, sites, focus);
    }
    Ok(id)
}

#[tauri::command]
fn stop_activity(
    db: tauri::State<'_, HanniDb>,
    focus: tauri::State<'_, FocusManager>,
) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Find current (unfinished) activity
    let result: Result<(i64, String), _> = conn.query_row(
        "SELECT id, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok((id, started_at)) => {
            if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let duration = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                conn.execute(
                    "UPDATE activities SET ended_at=?1, duration_minutes=?2 WHERE id=?3",
                    rusqlite::params![now, duration, id],
                ).map_err(|e| format!("DB error: {}", e))?;
            } else {
                conn.execute(
                    "UPDATE activities SET ended_at=?1 WHERE id=?2",
                    rusqlite::params![now, id],
                ).map_err(|e| format!("DB error: {}", e))?;
            }
            // Stop focus if active
            drop(conn);
            let _ = stop_focus(focus);
            Ok("Activity stopped".into())
        }
        Err(_) => Ok("No active activity".into()),
    }
}

#[tauri::command]
fn get_current_activity(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let result: Result<(i64, String, String, String), _> = conn.query_row(
        "SELECT id, title, category, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );
    match result {
        Ok((id, title, category, started_at)) => {
            let elapsed = if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let mins = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                let h = mins / 60;
                let m = mins % 60;
                if h > 0 { format!("{}ч {}м", h, m) } else { format!("{}м", m) }
            } else { String::new() };
            Ok(serde_json::json!({ "id": id, "title": title, "category": category, "started_at": started_at, "elapsed": elapsed }))
        }
        Err(_) => Err("No active activity".into()),
    }
}

#[tauri::command]
fn get_activity_log(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn.prepare(
        "SELECT id, title, category, started_at, ended_at, duration_minutes FROM activities
         WHERE started_at LIKE ?1 ORDER BY started_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", target_date);
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| {
        let started: String = row.get(3)?;
        let time = if started.len() >= 16 { started[11..16].to_string() } else { String::new() };
        let dur_min: Option<i64> = row.get(5)?;
        let duration = dur_min.map(|m| if m >= 60 { format!("{}ч {}м", m/60, m%60) } else { format!("{}м", m) }).unwrap_or_default();
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "time": time,
            "duration": duration,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Notes commands ──

#[tauri::command]
fn create_note(title: String, content: String, tags: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO notes (title, content, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![title, content, tags, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_note(
    id: i64, title: String, content: String, tags: String,
    pinned: Option<bool>, archived: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Get current values for pinned/archived if not provided
    let (cur_pinned, cur_archived): (i32, i32) = conn.query_row(
        "SELECT pinned, archived FROM notes WHERE id=?1", rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0, 0));
    let p = pinned.map(|v| v as i32).unwrap_or(cur_pinned);
    let a = archived.map(|v| v as i32).unwrap_or(cur_archived);
    conn.execute(
        "UPDATE notes SET title=?1, content=?2, tags=?3, pinned=?4, archived=?5, updated_at=?6 WHERE id=?7",
        rusqlite::params![title, content, tags, p, a, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn delete_note(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM notes WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_notes(_filter: Option<String>, search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let rows = if let Some(q) = search {
        if q.trim().is_empty() { get_notes_all(&conn)? }
        else {
            let words: Vec<&str> = q.split_whitespace().filter(|w| w.len() > 1).take(10).collect();
            if words.is_empty() { get_notes_all(&conn)? }
            else {
                let fts_query = words.join(" OR ");
                let mut stmt = conn.prepare(
                    "SELECT n.id, n.title, n.content, n.tags, n.pinned, n.archived, n.created_at, n.updated_at
                     FROM notes_fts fts JOIN notes n ON n.id = fts.rowid
                     WHERE notes_fts MATCH ?1 ORDER BY rank LIMIT 50"
                ).map_err(|e| format!("DB error: {}", e))?;
                let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![fts_query], |row| note_from_row(row))
                    .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
                result
            }
        }
    } else {
        get_notes_all(&conn)?
    };
    Ok(rows)
}

fn get_notes_all(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at FROM notes
         WHERE archived=0 ORDER BY pinned DESC, updated_at DESC LIMIT 100"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| note_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

fn note_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "title": row.get::<_, String>(1)?,
        "content": row.get::<_, String>(2)?,
        "tags": row.get::<_, String>(3)?,
        "pinned": row.get::<_, i32>(4)? != 0,
        "archived": row.get::<_, i32>(5)? != 0,
        "created_at": row.get::<_, String>(6)?,
        "updated_at": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
fn get_note(id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    conn.query_row(
        "SELECT id, title, content, tags, pinned, archived, created_at, updated_at FROM notes WHERE id=?1",
        rusqlite::params![id],
        |row| note_from_row(row),
    ).map_err(|e| format!("Not found: {}", e))
}

// ── v0.7.0: Events (Calendar) commands ──

#[tauri::command]
fn create_event(title: String, description: String, date: String, time: String, duration_minutes: i64, category: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO events (title, description, date, time, duration_minutes, category, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![title, description, date, time, duration_minutes, category, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_events(month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let prefix = format!("{}-{:02}", year, month);
    let mut stmt = conn.prepare(
        "SELECT id, title, description, date, time, duration_minutes, category, color, completed, COALESCE(source,'manual') FROM events WHERE date LIKE ?1 ORDER BY date, time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", prefix);
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "time": row.get::<_, String>(4)?,
            "duration_minutes": row.get::<_, i64>(5)?,
            "category": row.get::<_, String>(6)?,
            "color": row.get::<_, String>(7)?,
            "completed": row.get::<_, i32>(8)? != 0,
            "source": row.get::<_, String>(9)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn delete_event(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM events WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.8.3: Calendar Sync ──

#[tauri::command]
async fn sync_apple_calendar(month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    // AppleScript to get events from Calendar.app for the given month
    let prefix = format!("{}-{:02}", year, month);
    let last_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 },
        _ => 31,
    };

    let script = format!(
        r#"
        set output to ""
        set startD to current date
        set year of startD to {year}
        set month of startD to {month}
        set day of startD to 1
        set time of startD to 0
        set endD to current date
        set year of endD to {year}
        set month of endD to {month}
        set day of endD to {last_day}
        set time of endD to 86399
        tell application "Calendar"
            repeat with cal in calendars
                set calName to name of cal
                set evts to (every event of cal whose start date >= startD and start date <= endD)
                repeat with evt in evts
                    set evtStart to start date of evt
                    set evtName to summary of evt
                    set evtDur to 60
                    try
                        set evtEnd to end date of evt
                        set evtDur to ((evtEnd - evtStart) / 60) as integer
                    end try
                    set evtDesc to ""
                    try
                        set evtDesc to description of evt
                    end try
                    set evtUID to uid of evt
                    set m to (month of evtStart as integer)
                    set d to day of evtStart
                    set h to hours of evtStart
                    set mn to minutes of evtStart
                    set dateStr to "{year}-" & text -2 thru -1 of ("0" & m) & "-" & text -2 thru -1 of ("0" & d)
                    set timeStr to text -2 thru -1 of ("0" & h) & ":" & text -2 thru -1 of ("0" & mn)
                    set output to output & evtUID & "||" & evtName & "||" & dateStr & "||" & timeStr & "||" & evtDur & "||" & calName & "||" & evtDesc & linefeed
                end repeat
            end repeat
        end tell
        return output
        "#,
        year = year, month = month, last_day = last_day
    );

    // Pre-check: verify Calendar.app access permission (cached — won't re-prompt after denial)
    if !check_calendar_access() {
        return Ok(serde_json::json!({
            "synced": 0,
            "source": "apple",
            "error": "Нет доступа к Calendar.app. Включите в Системные настройки → Конфиденциальность → Автоматизация"
        }));
    }

    let output = match run_osascript(&script) {
        Ok(s) => s,
        Err(e) => {
            return Ok(serde_json::json!({
                "synced": 0,
                "source": "apple",
                "error": format!("Ошибка синхронизации: {}", e)
            }));
        }
    };
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();

    // Clear old apple events for this month
    conn.execute(
        "DELETE FROM events WHERE source='apple' AND date LIKE ?1",
        rusqlite::params![format!("{}%", prefix)],
    ).map_err(|e| format!("DB error: {}", e))?;

    let mut count = 0i32;
    for line in output.lines() {
        let parts: Vec<&str> = line.split("||").collect();
        if parts.len() < 6 { continue; }
        let uid = parts[0].trim();
        let title = parts[1].trim();
        let date = parts[2].trim();
        let time = parts[3].trim();
        let dur: i64 = parts[4].trim().parse().unwrap_or(60);
        let cal_name = parts[5].trim();
        let desc = parts.get(6).unwrap_or(&"").trim();

        conn.execute(
            "INSERT INTO events (title, description, date, time, duration_minutes, category, color, source, external_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'apple', ?8, ?9)",
            rusqlite::params![title, desc, date, time, dur, cal_name, "#a1a1a6", uid, now],
        ).map_err(|e| format!("Insert error: {}", e))?;
        count += 1;
    }

    Ok(serde_json::json!({ "synced": count, "source": "apple", "error": null }))
}

/// Parse ICS datetime line, handling TZID and UTC 'Z' suffix. Returns (NaiveDate, Option<NaiveTime>, is_allday).
fn parse_ics_datetime(line: &str) -> Option<(chrono::NaiveDate, Option<chrono::NaiveTime>, bool)> {
    use chrono::{NaiveDate, NaiveTime, NaiveDateTime, TimeZone};

    // All-day: DTSTART;VALUE=DATE:20250215
    if line.contains("VALUE=DATE") {
        let re_d = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})").unwrap();
        let caps = re_d.captures(line)?;
        let d = NaiveDate::from_ymd_opt(caps[1].parse().ok()?, caps[2].parse().ok()?, caps[3].parse().ok()?)?;
        return Some((d, None, true));
    }

    let re_dt = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})?").unwrap();
    let caps = re_dt.captures(line)?;
    let y: i32 = caps[1].parse().ok()?;
    let mo: u32 = caps[2].parse().ok()?;
    let da: u32 = caps[3].parse().ok()?;
    let h: u32 = caps[4].parse().ok()?;
    let mi: u32 = caps[5].parse().ok()?;
    let s: u32 = caps.get(6).and_then(|c| c.as_str().parse().ok()).unwrap_or(0);

    let naive_dt = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(y, mo, da)?,
        NaiveTime::from_hms_opt(h, mi, s)?,
    );

    // Extract TZID if present: DTSTART;TZID=America/New_York:20250215T093000
    let re_tzid = regex::Regex::new(r"TZID=([^:;]+)").unwrap();
    let local_dt = if let Some(tz_caps) = re_tzid.captures(line) {
        let tz_str = tz_caps[1].trim();
        if let Ok(tz) = tz_str.parse::<chrono_tz::Tz>() {
            // Parse in source timezone, convert to local
            match tz.from_local_datetime(&naive_dt).earliest() {
                Some(zoned) => zoned.with_timezone(&chrono::Local).naive_local(),
                None => naive_dt,
            }
        } else {
            naive_dt // Unknown timezone, use as-is
        }
    } else if line.contains('Z') {
        // UTC: convert to local
        match chrono::Utc.from_utc_datetime(&naive_dt).with_timezone(&chrono::Local).naive_local() {
            dt => dt,
        }
    } else {
        // No timezone info — treat as local (floating time)
        naive_dt
    };

    Some((local_dt.date(), Some(local_dt.time()), false))
}

/// Parse RRULE line into components
struct RRule {
    freq: String,
    interval: u32,
    count: Option<u32>,
    until: Option<chrono::NaiveDate>,
    byday: Vec<String>,
}

fn parse_rrule(block: &str) -> Option<RRule> {
    let rrule_line = block.lines().find(|l| l.starts_with("RRULE:"))?;
    let params = &rrule_line["RRULE:".len()..];

    let mut freq = String::new();
    let mut interval = 1u32;
    let mut count = None;
    let mut until = None;
    let mut byday = Vec::new();

    for part in params.split(';') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim();
        match key {
            "FREQ" => freq = val.to_string(),
            "INTERVAL" => interval = val.parse().unwrap_or(1),
            "COUNT" => count = val.parse().ok(),
            "UNTIL" => {
                let re_d = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})").unwrap();
                if let Some(c) = re_d.captures(val) {
                    until = chrono::NaiveDate::from_ymd_opt(
                        c[1].parse().unwrap_or(2099), c[2].parse().unwrap_or(1), c[3].parse().unwrap_or(1)
                    );
                }
            }
            "BYDAY" => byday = val.split(',').map(|s| s.trim().to_string()).collect(),
            _ => {}
        }
    }

    if freq.is_empty() { return None; }
    Some(RRule { freq, interval, count, until, byday })
}

/// Collect EXDATE dates from a VEVENT block
fn parse_exdates(block: &str) -> std::collections::HashSet<chrono::NaiveDate> {
    let mut set = std::collections::HashSet::new();
    let re_d = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})").unwrap();
    for line in block.lines() {
        if line.starts_with("EXDATE") {
            for caps in re_d.captures_iter(line) {
                if let Some(d) = chrono::NaiveDate::from_ymd_opt(
                    caps[1].parse().unwrap_or(0), caps[2].parse().unwrap_or(0), caps[3].parse().unwrap_or(0)
                ) {
                    set.insert(d);
                }
            }
        }
    }
    set
}

/// Map BYDAY codes to chrono::Weekday
fn byday_to_weekday(code: &str) -> Option<chrono::Weekday> {
    // Strip numeric prefix (e.g. "2MO" → "MO")
    let code = code.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+');
    match code {
        "MO" => Some(chrono::Weekday::Mon),
        "TU" => Some(chrono::Weekday::Tue),
        "WE" => Some(chrono::Weekday::Wed),
        "TH" => Some(chrono::Weekday::Thu),
        "FR" => Some(chrono::Weekday::Fri),
        "SA" => Some(chrono::Weekday::Sat),
        "SU" => Some(chrono::Weekday::Sun),
        _ => None,
    }
}

/// Expand RRULE occurrences that fall within the target month
fn expand_rrule(
    start_date: chrono::NaiveDate,
    rrule: &RRule,
    exdates: &std::collections::HashSet<chrono::NaiveDate>,
    target_year: i32,
    target_month: u32,
) -> Vec<chrono::NaiveDate> {
    use chrono::{NaiveDate, Datelike, Duration};

    let month_start = match NaiveDate::from_ymd_opt(target_year, target_month, 1) {
        Some(d) => d,
        None => return vec![],
    };
    let month_end = if target_month == 12 {
        NaiveDate::from_ymd_opt(target_year + 1, 1, 1).unwrap_or(month_start)
    } else {
        NaiveDate::from_ymd_opt(target_year, target_month + 1, 1).unwrap_or(month_start)
    };
    // Don't expand too far into the future (max 3 years from start)
    let hard_limit = start_date + Duration::days(365 * 3);
    let effective_end = month_end.min(hard_limit);

    let max_count = rrule.count.unwrap_or(1000) as usize;
    let until = rrule.until.unwrap_or(effective_end);

    let mut results = Vec::new();
    let mut occurrence_count = 0usize;

    match rrule.freq.as_str() {
        "DAILY" => {
            let step = Duration::days(rrule.interval as i64);
            let mut d = start_date;
            while d <= until && d < effective_end && occurrence_count < max_count {
                if d >= month_start && d < month_end && !exdates.contains(&d) {
                    results.push(d);
                }
                occurrence_count += 1;
                d += step;
            }
        }
        "WEEKLY" => {
            let weekdays: Vec<chrono::Weekday> = if rrule.byday.is_empty() {
                vec![start_date.weekday()]
            } else {
                rrule.byday.iter().filter_map(|s| byday_to_weekday(s)).collect()
            };
            let step = Duration::weeks(rrule.interval as i64);
            // Walk week by week from start
            let mut week_start = start_date - Duration::days(start_date.weekday().num_days_from_monday() as i64);
            while week_start <= until && week_start < effective_end + Duration::days(7) && occurrence_count < max_count {
                for wd in &weekdays {
                    let d = week_start + Duration::days(wd.num_days_from_monday() as i64);
                    if d < start_date { continue; }
                    if d > until || d >= effective_end { continue; }
                    if occurrence_count >= max_count { break; }
                    occurrence_count += 1;
                    if d >= month_start && d < month_end && !exdates.contains(&d) {
                        results.push(d);
                    }
                }
                week_start += step;
            }
        }
        "MONTHLY" => {
            let day = start_date.day();
            let mut y = start_date.year();
            let mut m = start_date.month();
            while occurrence_count < max_count {
                if let Some(d) = NaiveDate::from_ymd_opt(y, m, day.min(28)) // safe day
                    .or_else(|| NaiveDate::from_ymd_opt(y, m, 28))
                {
                    if d > until || d >= effective_end { break; }
                    if d >= start_date {
                        occurrence_count += 1;
                        if d >= month_start && d < month_end && !exdates.contains(&d) {
                            results.push(d);
                        }
                    }
                }
                // Advance by interval months
                for _ in 0..rrule.interval {
                    m += 1;
                    if m > 12 { m = 1; y += 1; }
                }
            }
        }
        "YEARLY" => {
            let mut y = start_date.year();
            while occurrence_count < max_count {
                if let Some(d) = NaiveDate::from_ymd_opt(y, start_date.month(), start_date.day().min(28))
                    .or_else(|| NaiveDate::from_ymd_opt(y, start_date.month(), 28))
                {
                    if d > until || d >= effective_end { break; }
                    if d >= start_date {
                        occurrence_count += 1;
                        if d >= month_start && d < month_end && !exdates.contains(&d) {
                            results.push(d);
                        }
                    }
                }
                y += rrule.interval as i32;
            }
        }
        _ => {}
    }

    results
}

#[tauri::command]
async fn sync_google_ics(url: String, month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    if url.is_empty() { return Err("No ICS URL provided".into()); }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let body = client.get(&url).send().await
        .map_err(|e| format!("Fetch error: {}", e))?
        .text().await
        .map_err(|e| format!("Read error: {}", e))?;

    let prefix = format!("{}-{:02}", year, month);
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();

    // Clear old google events for this month
    conn.execute(
        "DELETE FROM events WHERE source='google' AND date LIKE ?1",
        rusqlite::params![format!("{}%", prefix)],
    ).map_err(|e| format!("DB error: {}", e))?;

    let mut count = 0i32;

    for block in body.split("BEGIN:VEVENT") {
        if !block.contains("END:VEVENT") { continue; }
        let get_field = |field: &str| -> String {
            block.lines()
                .find(|l| l.starts_with(field))
                .map(|l| l[field.len()..].trim().to_string())
                .unwrap_or_default()
        };

        let summary = get_field("SUMMARY:");
        if summary.is_empty() { continue; }

        let dtstart_line = block.lines()
            .find(|l| l.starts_with("DTSTART"))
            .unwrap_or("");
        let dtend_line = block.lines()
            .find(|l| l.starts_with("DTEND"))
            .unwrap_or("");
        let uid = get_field("UID:");
        let desc = get_field("DESCRIPTION:").replace("\\n", "\n").replace("\\,", ",");

        // Parse start datetime with timezone handling
        let (start_date, start_time, is_allday) = match parse_ics_datetime(dtstart_line) {
            Some(v) => v,
            None => continue,
        };
        let time_str = start_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_default();

        // Calculate duration
        let dur: i64 = if is_allday {
            0
        } else if let Some((end_date, end_time, _)) = parse_ics_datetime(dtend_line) {
            if let (Some(st), Some(et)) = (start_time, end_time) {
                let start_mins = st.hour() as i64 * 60 + st.minute() as i64;
                let end_mins = et.hour() as i64 * 60 + et.minute() as i64;
                let day_diff = (end_date - start_date).num_days() * 24 * 60;
                (end_mins - start_mins + day_diff).max(1)
            } else { 60 }
        } else { 60 };

        // Collect dates to insert: original + RRULE expansions
        let mut dates_to_insert: Vec<chrono::NaiveDate> = Vec::new();

        // Check if original date falls in target month
        let date_str = start_date.format("%Y-%m").to_string();
        if date_str == prefix {
            dates_to_insert.push(start_date);
        }

        // RRULE expansion
        if let Some(rrule) = parse_rrule(block) {
            let exdates = parse_exdates(block);
            let mut expanded = expand_rrule(start_date, &rrule, &exdates, year, month as u32);
            // Remove the original date if already added (avoid duplicates)
            expanded.retain(|d| *d != start_date || !dates_to_insert.contains(d));
            dates_to_insert.extend(expanded);
        }

        // Deduplicate
        dates_to_insert.sort();
        dates_to_insert.dedup();

        // Insert each occurrence
        for occ_date in &dates_to_insert {
            let occ_date_str = occ_date.format("%Y-%m-%d").to_string();
            let ext_id = if *occ_date == start_date {
                uid.clone()
            } else {
                format!("{}_{}", uid, occ_date_str)
            };

            conn.execute(
                "INSERT INTO events (title, description, date, time, duration_minutes, category, color, source, external_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'google', ?6, 'google', ?7, ?8)",
                rusqlite::params![summary, desc, occ_date_str, time_str, dur, "#a1a1a6", ext_id, now],
            ).map_err(|e| format!("Insert error: {}", e))?;
            count += 1;
        }
    }

    Ok(serde_json::json!({ "synced": count, "source": "google" }))
}

// ── v0.7.0: Projects & Tasks (Work) commands ──

#[tauri::command]
fn create_project(name: String, description: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (name, description, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![name, description, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_projects(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.description, p.status, p.color, p.created_at,
                (SELECT COUNT(*) FROM tasks WHERE project_id=p.id) as task_count
         FROM projects p WHERE p.status='active' ORDER BY p.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "color": row.get::<_, String>(4)?,
            "task_count": row.get::<_, i64>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn create_task(project_id: i64, title: String, description: String, priority: String, due_date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tasks (project_id, title, description, priority, due_date, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![project_id, title, description, priority, due_date, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_tasks(project_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, description, status, priority, due_date, completed_at FROM tasks
         WHERE project_id=?1 ORDER BY CASE status WHEN 'todo' THEN 0 WHEN 'in_progress' THEN 1 ELSE 2 END, created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![project_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "priority": row.get::<_, String>(4)?,
            "due_date": row.get::<_, Option<String>>(5)?,
            "completed_at": row.get::<_, Option<String>>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_task_status(id: i64, status: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let completed_at = if status == "done" { Some(now.clone()) } else { None };
    conn.execute(
        "UPDATE tasks SET status=?1, completed_at=?2 WHERE id=?3",
        rusqlite::params![status, completed_at, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.7.0: Learning Items (Development) commands ──

#[tauri::command]
fn create_learning_item(item_type: String, title: String, description: String, url: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO learning_items (type, title, description, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![item_type, title, description, url, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_learning_items(type_filter: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let rows = if let Some(t) = type_filter {
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, url, progress, status, category FROM learning_items WHERE type=?1 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![t], |row| learning_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        result
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, url, progress, status, category FROM learning_items ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result: Vec<serde_json::Value> = stmt.query_map([], |row| learning_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        result
    };
    Ok(rows)
}

fn learning_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "type": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?,
        "description": row.get::<_, String>(3)?,
        "url": row.get::<_, String>(4)?,
        "progress": row.get::<_, i32>(5)?,
        "status": row.get::<_, String>(6)?,
        "category": row.get::<_, String>(7)?,
    }))
}

// ── v0.7.0: Hobbies commands ──

#[tauri::command]
fn create_hobby(name: String, category: String, icon: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO hobbies (name, category, icon, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, category, icon, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_hobbies(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT h.id, h.name, h.category, h.icon, h.color,
                COALESCE((SELECT SUM(duration_minutes) FROM hobby_entries WHERE hobby_id=h.id), 0) / 60.0 as total_hours
         FROM hobbies h ORDER BY h.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?,
            "icon": row.get::<_, String>(3)?,
            "color": row.get::<_, String>(4)?,
            "total_hours": format!("{:.1}", row.get::<_, f64>(5)?),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn log_hobby_entry(hobby_id: i64, duration_minutes: i64, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO hobby_entries (hobby_id, date, duration_minutes, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![hobby_id, date, duration_minutes, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_hobby_entries(hobby_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, date, duration_minutes, notes FROM hobby_entries WHERE hobby_id=?1 ORDER BY date DESC LIMIT 30"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![hobby_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "date": row.get::<_, String>(1)?,
            "duration_minutes": row.get::<_, i64>(2)?,
            "notes": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Workouts (Sports) commands ──

#[tauri::command]
fn create_workout(workout_type: String, title: String, duration_minutes: i64, calories: Option<i64>, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO workouts (type, title, date, duration_minutes, calories, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![workout_type, title, date, duration_minutes, calories, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_workouts(_date_range: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, type, title, date, duration_minutes, calories, notes FROM workouts ORDER BY date DESC, created_at DESC LIMIT 50"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, String>(1)?,
            "title": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "duration_minutes": row.get::<_, i64>(4)?,
            "calories": row.get::<_, Option<i64>>(5)?,
            "notes": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn get_workout_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let (count, total_min, total_cal): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(duration_minutes), 0), COALESCE(SUM(calories), 0) FROM workouts WHERE date >= ?1",
        rusqlite::params![week_ago],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0, 0));
    Ok(serde_json::json!({ "count": count, "total_minutes": total_min, "total_calories": total_cal }))
}

// ── v0.7.0: Health & Habits commands ──

#[tauri::command]
fn log_health(health_type: String, value: f64, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let unit = match health_type.as_str() {
        "sleep" => "hours", "water" => "glasses", "weight" => "kg", "mood" => "1-5", "steps" => "steps",
        _ => "",
    };
    // Upsert: update if same date+type exists
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM health_log WHERE date=?1 AND type=?2 LIMIT 1",
        rusqlite::params![date, health_type],
        |row| row.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE health_log SET value=?1, notes=?2 WHERE id=?3",
            rusqlite::params![value, notes.unwrap_or_default(), id],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO health_log (date, type, value, unit, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![date, health_type, value, unit, notes.unwrap_or_default(), now.to_rfc3339()],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(conn.last_insert_rowid())
    }
}

#[tauri::command]
fn get_health_today(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT type, value FROM health_log WHERE date=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let mut result = serde_json::json!({});
    let rows = stmt.query_map(rusqlite::params![today], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }).map_err(|e| format!("Query error: {}", e))?;
    for row in rows.flatten() {
        result[row.0] = serde_json::json!(row.1);
    }
    Ok(result)
}

#[tauri::command]
fn create_habit(name: String, icon: String, frequency: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO habits (name, icon, frequency, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, icon, frequency, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn check_habit(habit_id: i64, date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let target_date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let now = chrono::Local::now().to_rfc3339();
    // Toggle: if exists, delete; else insert
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM habit_checks WHERE habit_id=?1 AND date=?2",
        rusqlite::params![habit_id, target_date],
        |row| row.get(0),
    ).ok();
    if let Some(id) = existing {
        conn.execute("DELETE FROM habit_checks WHERE id=?1", rusqlite::params![id])
            .map_err(|e| format!("DB error: {}", e))?;
    } else {
        conn.execute(
            "INSERT INTO habit_checks (habit_id, date, completed, created_at) VALUES (?1, ?2, 1, ?3)",
            rusqlite::params![habit_id, target_date, now],
        ).map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn get_habits_today(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT h.id, h.name, h.icon, h.frequency,
                (SELECT COUNT(*) FROM habit_checks WHERE habit_id=h.id AND date=?1) as checked,
                (SELECT COUNT(*) FROM habit_checks hc WHERE hc.habit_id=h.id AND hc.date >= date(?1, '-30 days')) as streak_approx
         FROM habits h ORDER BY h.created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![today], |row| {
        // Simple streak calc: count consecutive days backward
        let checked: i64 = row.get(4)?;
        let streak_approx: i64 = row.get(5)?;
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "icon": row.get::<_, String>(2)?,
            "frequency": row.get::<_, String>(3)?,
            "completed": checked > 0,
            "streak": streak_approx,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.7.0: Dashboard aggregate command ──

#[tauri::command]
fn get_dashboard_data(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today_pattern = format!("{}%", today);

    // Current activity
    let current_activity: Option<serde_json::Value> = conn.query_row(
        "SELECT title, category, started_at FROM activities WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            let started: String = row.get(2)?;
            let elapsed = if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started) {
                let mins = (chrono::Local::now() - start.with_timezone(&chrono::Local)).num_minutes();
                format!("{}м", mins)
            } else { String::new() };
            Ok(serde_json::json!({ "title": row.get::<_, String>(0)?, "category": row.get::<_, String>(1)?, "elapsed": elapsed }))
        },
    ).ok();

    // Activities count today
    let activities_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activities WHERE started_at LIKE ?1", rusqlite::params![today_pattern], |row| row.get(0),
    ).unwrap_or(0);

    // Focus minutes today
    let focus_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_minutes), 0) FROM activities WHERE started_at LIKE ?1 AND ended_at IS NOT NULL",
        rusqlite::params![today_pattern], |row| row.get(0),
    ).unwrap_or(0);

    // Notes count
    let notes_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes WHERE archived=0", [], |row| row.get(0)).unwrap_or(0);

    // Events today
    let mut events_stmt = conn.prepare(
        "SELECT title, time FROM events WHERE date=?1 ORDER BY time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let events: Vec<serde_json::Value> = events_stmt.query_map(rusqlite::params![today], |row| {
        Ok(serde_json::json!({ "title": row.get::<_, String>(0)?, "time": row.get::<_, String>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    // Recent notes
    let mut notes_stmt = conn.prepare(
        "SELECT title FROM notes WHERE archived=0 ORDER BY updated_at DESC LIMIT 3"
    ).map_err(|e| format!("DB error: {}", e))?;
    let recent_notes: Vec<serde_json::Value> = notes_stmt.query_map([], |row| {
        Ok(serde_json::json!({ "title": row.get::<_, String>(0)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    Ok(serde_json::json!({
        "current_activity": current_activity,
        "activities_today": activities_today,
        "focus_minutes": focus_minutes,
        "notes_count": notes_count,
        "events_today": events.len(),
        "events": events,
        "recent_notes": recent_notes,
    }))
}

// ── v0.7.0: Memory browser command ──

#[tauri::command]
fn get_all_memories(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(q) = search {
        if !q.trim().is_empty() {
            let like = format!("%{}%", q);
            let mut stmt = conn.prepare(
                "SELECT id, category, key, value FROM facts WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1 ORDER BY updated_at DESC LIMIT 100"
            ).map_err(|e| format!("DB error: {}", e))?;
            let rows = stmt.query_map(rusqlite::params![like], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "category": row.get::<_, String>(1)?,
                    "key": row.get::<_, String>(2)?,
                    "value": row.get::<_, String>(3)?,
                }))
            }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
            return Ok(rows);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT id, category, key, value FROM facts ORDER BY category, updated_at DESC LIMIT 200"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "category": row.get::<_, String>(1)?,
            "key": row.get::<_, String>(2)?,
            "value": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn delete_memory(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM facts WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn update_memory(id: i64, category: Option<String>, key: Option<String>, value: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    if let Some(cat) = &category {
        conn.execute("UPDATE facts SET category=?1, updated_at=?2 WHERE id=?3", rusqlite::params![cat, now, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(k) = &key {
        conn.execute("UPDATE facts SET key=?1, updated_at=?2 WHERE id=?3", rusqlite::params![k, now, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(v) = &value {
        conn.execute("UPDATE facts SET value=?1, updated_at=?2 WHERE id=?3", rusqlite::params![v, now, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

// ── v0.8.0: Media Items (Hobbies collections) ──

#[tauri::command]
fn add_media_item(
    media_type: String, title: String, original_title: Option<String>, year: Option<i32>,
    description: Option<String>, cover_url: Option<String>, status: Option<String>,
    rating: Option<i32>, progress: Option<i32>, total_episodes: Option<i32>,
    notes: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO media_items (media_type, title, original_title, year, description, cover_url, status, rating, progress, total_episodes, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        rusqlite::params![
            media_type, title, original_title.unwrap_or_default(), year,
            description.unwrap_or_default(), cover_url.unwrap_or_default(),
            status.unwrap_or_else(|| "planned".into()), rating.unwrap_or(0),
            progress.unwrap_or(0), total_episodes,
            notes.unwrap_or_default(), now
        ],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_media_item(
    id: i64, status: Option<String>, rating: Option<i32>, progress: Option<i32>,
    notes: Option<String>, title: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Build dynamic update
    let (cur_status, cur_rating, cur_progress, cur_notes, cur_title): (String, i32, i32, String, String) = conn.query_row(
        "SELECT status, rating, progress, notes, title FROM media_items WHERE id=?1",
        rusqlite::params![id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    let new_status = status.unwrap_or(cur_status);
    let completed_at = if new_status == "completed" { Some(now.clone()) } else { None };
    conn.execute(
        "UPDATE media_items SET status=?1, rating=?2, progress=?3, notes=?4, title=?5, completed_at=?6, updated_at=?7 WHERE id=?8",
        rusqlite::params![new_status, rating.unwrap_or(cur_rating), progress.unwrap_or(cur_progress),
            notes.unwrap_or(cur_notes), title.unwrap_or(cur_title), completed_at, now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn delete_media_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM list_items WHERE media_item_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM media_items WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_media_items(media_type: String, status: Option<String>, show_hidden: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let hidden = if show_hidden.unwrap_or(false) { 1 } else { 0 };
    if let Some(s) = status {
        let mut stmt = conn.prepare(
            "SELECT id, media_type, title, original_title, year, status, rating, progress, total_episodes, cover_url, notes, hidden, created_at
             FROM media_items WHERE media_type=?1 AND status=?2 AND hidden<=?3 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![media_type, s, hidden], |row| media_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, media_type, title, original_title, year, status, rating, progress, total_episodes, cover_url, notes, hidden, created_at
             FROM media_items WHERE media_type=?1 AND hidden<=?2 ORDER BY updated_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![media_type, hidden], |row| media_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

fn media_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "media_type": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?,
        "original_title": row.get::<_, String>(3)?,
        "year": row.get::<_, Option<i32>>(4)?,
        "status": row.get::<_, String>(5)?,
        "rating": row.get::<_, i32>(6)?,
        "progress": row.get::<_, i32>(7)?,
        "total_episodes": row.get::<_, Option<i32>>(8)?,
        "cover_url": row.get::<_, String>(9)?,
        "notes": row.get::<_, String>(10)?,
        "hidden": row.get::<_, i32>(11)? != 0,
    }))
}

#[tauri::command]
fn hide_media_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE media_items SET hidden=1 WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn unhide_media_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE media_items SET hidden=0 WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn create_user_list(name: String, description: Option<String>, color: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO user_lists (name, description, color, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, description.unwrap_or_default(), color.unwrap_or_else(|| "#818cf8".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_user_lists(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT ul.id, ul.name, ul.description, ul.color,
                (SELECT COUNT(*) FROM list_items WHERE list_id=ul.id) as item_count
         FROM user_lists ul ORDER BY ul.created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "color": row.get::<_, String>(3)?,
            "item_count": row.get::<_, i64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn add_to_list(list_id: i64, media_item_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO list_items (list_id, media_item_id, added_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![list_id, media_item_id, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn remove_from_list(list_id: i64, media_item_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM list_items WHERE list_id=?1 AND media_item_id=?2",
        rusqlite::params![list_id, media_item_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_list_items(list_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT m.id, m.media_type, m.title, m.status, m.rating, m.cover_url
         FROM list_items li JOIN media_items m ON m.id = li.media_item_id
         WHERE li.list_id=?1 ORDER BY li.position, li.added_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![list_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "media_type": row.get::<_, String>(1)?,
            "title": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "rating": row.get::<_, i32>(4)?,
            "cover_url": row.get::<_, String>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn get_media_stats(media_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    if let Some(mt) = media_type {
        let (total, completed, in_progress, avg_rating): (i64, i64, i64, f64) = conn.query_row(
            "SELECT COUNT(*), SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status='in_progress' THEN 1 ELSE 0 END),
                    COALESCE(AVG(CASE WHEN rating>0 THEN rating END), 0)
             FROM media_items WHERE media_type=?1 AND hidden=0",
            rusqlite::params![mt], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap_or((0, 0, 0, 0.0));
        Ok(serde_json::json!({ "total": total, "completed": completed, "in_progress": in_progress, "avg_rating": format!("{:.1}", avg_rating) }))
    } else {
        let mut stmt = conn.prepare(
            "SELECT media_type, COUNT(*), SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END)
             FROM media_items WHERE hidden=0 GROUP BY media_type"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "media_type": row.get::<_, String>(0)?,
                "total": row.get::<_, i64>(1)?,
                "completed": row.get::<_, i64>(2)?,
            }))
        }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(serde_json::json!({ "by_type": rows }))
    }
}

// ── v0.8.0: Food commands ──

#[tauri::command]
fn log_food(
    date: Option<String>, meal_type: String, name: String,
    calories: Option<i64>, protein: Option<f64>, carbs: Option<f64>, fat: Option<f64>,
    notes: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO food_log (date, meal_type, name, calories, protein, carbs, fat, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![d, meal_type, name, calories.unwrap_or(0), protein.unwrap_or(0.0),
            carbs.unwrap_or(0.0), fat.unwrap_or(0.0), notes.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_food_log(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn.prepare(
        "SELECT id, meal_type, name, calories, protein, carbs, fat, notes FROM food_log WHERE date=?1 ORDER BY created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![d], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "meal_type": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?, "calories": row.get::<_, i64>(3)?,
            "protein": row.get::<_, f64>(4)?, "carbs": row.get::<_, f64>(5)?,
            "fat": row.get::<_, f64>(6)?, "notes": row.get::<_, String>(7)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn delete_food_entry(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM food_log WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_food_stats(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let d = days.unwrap_or(7);
    let since = (chrono::Local::now() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let (total_cal, avg_cal, total_protein): (i64, f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(calories),0), COALESCE(AVG(daily_cal),0), COALESCE(SUM(protein),0)
         FROM (SELECT date, SUM(calories) as daily_cal, SUM(protein) as protein FROM food_log WHERE date>=?1 GROUP BY date)",
        rusqlite::params![since], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0.0, 0.0));
    Ok(serde_json::json!({ "total_calories": total_cal, "avg_daily_calories": format!("{:.0}", avg_cal), "total_protein": format!("{:.1}", total_protein), "days": d }))
}

#[tauri::command]
fn create_recipe(
    name: String, description: Option<String>, ingredients: String, instructions: String,
    prep_time: Option<i64>, cook_time: Option<i64>, servings: Option<i64>,
    calories: Option<i64>, tags: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time, servings, calories, tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        rusqlite::params![name, description.unwrap_or_default(), ingredients, instructions,
            prep_time.unwrap_or(0), cook_time.unwrap_or(0), servings.unwrap_or(1),
            calories.unwrap_or(0), tags.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_recipes(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(q) = search {
        let like = format!("%{}%", q);
        let mut stmt = conn.prepare(
            "SELECT id, name, description, prep_time, cook_time, servings, calories, tags FROM recipes WHERE name LIKE ?1 OR tags LIKE ?1 ORDER BY updated_at DESC LIMIT 50"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![like], |row| recipe_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, description, prep_time, cook_time, servings, calories, tags FROM recipes ORDER BY updated_at DESC LIMIT 50"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| recipe_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

fn recipe_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "description": row.get::<_, String>(2)?, "prep_time": row.get::<_, i64>(3)?,
        "cook_time": row.get::<_, i64>(4)?, "servings": row.get::<_, i64>(5)?,
        "calories": row.get::<_, i64>(6)?, "tags": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
fn delete_recipe(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM recipes WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn add_product(
    name: String, category: Option<String>, quantity: Option<f64>, unit: Option<String>,
    expiry_date: Option<String>, location: Option<String>, notes: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO products (name, category, quantity, unit, expiry_date, location, notes, purchased_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        rusqlite::params![name, category.unwrap_or_else(|| "other".into()), quantity.unwrap_or(1.0),
            unit.unwrap_or_else(|| "шт".into()), expiry_date,
            location.unwrap_or_else(|| "fridge".into()), notes.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_products(location: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(loc) = location {
        let mut stmt = conn.prepare(
            "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products WHERE location=?1 ORDER BY expiry_date NULLS LAST"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![loc], |row| product_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products ORDER BY expiry_date NULLS LAST"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| product_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

fn product_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "category": row.get::<_, String>(2)?, "quantity": row.get::<_, f64>(3)?,
        "unit": row.get::<_, String>(4)?, "expiry_date": row.get::<_, Option<String>>(5)?,
        "location": row.get::<_, String>(6)?, "notes": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
fn update_product(id: i64, quantity: Option<f64>, expiry_date: Option<String>, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let (cur_qty, cur_exp, cur_notes): (f64, Option<String>, String) = conn.query_row(
        "SELECT quantity, expiry_date, notes FROM products WHERE id=?1", rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| format!("Not found: {}", e))?;
    conn.execute(
        "UPDATE products SET quantity=?1, expiry_date=?2, notes=?3 WHERE id=?4",
        rusqlite::params![quantity.unwrap_or(cur_qty), expiry_date.or(cur_exp), notes.unwrap_or(cur_notes), id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn delete_product(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM products WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_expiring_products(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = days.unwrap_or(3);
    let deadline = (chrono::Local::now() + chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products
         WHERE expiry_date IS NOT NULL AND expiry_date <= ?1 ORDER BY expiry_date"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![deadline], |row| product_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.8.0: Money commands ──

#[tauri::command]
fn add_transaction(
    date: Option<String>, transaction_type: String, amount: f64, currency: Option<String>,
    category: String, description: Option<String>, recurring: Option<bool>,
    recurring_period: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO transactions (date, type, amount, currency, category, description, recurring, recurring_period, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![d, transaction_type, amount, currency.unwrap_or_else(|| "KZT".into()),
            category, description.unwrap_or_default(), recurring.unwrap_or(false) as i32,
            recurring_period, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_transactions(month: Option<String>, transaction_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let prefix = month.unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let pattern = format!("{}%", prefix);
    if let Some(t) = transaction_type {
        let mut stmt = conn.prepare(
            "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE date LIKE ?1 AND type=?2 ORDER BY date DESC, created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern, t], |row| tx_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE date LIKE ?1 ORDER BY date DESC, created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| tx_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

fn tx_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
        "type": row.get::<_, String>(2)?, "amount": row.get::<_, f64>(3)?,
        "currency": row.get::<_, String>(4)?, "category": row.get::<_, String>(5)?,
        "description": row.get::<_, String>(6)?,
    }))
}

#[tauri::command]
fn delete_transaction(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM transactions WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_transaction_stats(month: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let prefix = month.unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let pattern = format!("{}%", prefix);
    let (total_expense, total_income): (f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN type='expense' THEN amount END), 0),
                COALESCE(SUM(CASE WHEN type='income' THEN amount END), 0)
         FROM transactions WHERE date LIKE ?1",
        rusqlite::params![pattern], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0.0, 0.0));
    // By category
    let mut stmt = conn.prepare(
        "SELECT category, SUM(amount) FROM transactions WHERE date LIKE ?1 AND type='expense' GROUP BY category ORDER BY SUM(amount) DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let by_cat: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({ "category": row.get::<_, String>(0)?, "amount": row.get::<_, f64>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({ "total_expense": total_expense, "total_income": total_income, "balance": total_income - total_expense, "by_category": by_cat }))
}

#[tauri::command]
fn create_budget(category: String, amount: f64, period: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let p = period.unwrap_or_else(|| "monthly".into());
    conn.execute(
        "INSERT INTO budgets (category, amount, period, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(category, period) DO UPDATE SET amount=?2",
        rusqlite::params![category, amount, p, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_budgets(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let pattern = format!("{}%", month);
    let mut stmt = conn.prepare(
        "SELECT b.id, b.category, b.amount, b.period,
                COALESCE((SELECT SUM(amount) FROM transactions WHERE category=b.category AND type='expense' AND date LIKE ?1), 0) as spent
         FROM budgets b ORDER BY b.category"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "category": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "period": row.get::<_, String>(3)?,
            "spent": row.get::<_, f64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn delete_budget(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM budgets WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn create_savings_goal(name: String, target_amount: f64, deadline: Option<String>, color: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO savings_goals (name, target_amount, deadline, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, target_amount, deadline, color.unwrap_or_else(|| "#818cf8".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_savings_goals(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, target_amount, current_amount, deadline, color FROM savings_goals ORDER BY created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        let target: f64 = row.get(2)?;
        let current: f64 = row.get(3)?;
        let pct = if target > 0.0 { (current / target * 100.0).min(100.0) } else { 0.0 };
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "target_amount": target, "current_amount": current,
            "deadline": row.get::<_, Option<String>>(4)?, "color": row.get::<_, String>(5)?,
            "percent": format!("{:.0}", pct),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_savings_goal(id: i64, add_amount: Option<f64>, target_amount: Option<f64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(add) = add_amount {
        conn.execute("UPDATE savings_goals SET current_amount = current_amount + ?1 WHERE id=?2", rusqlite::params![add, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(target) = target_amount {
        conn.execute("UPDATE savings_goals SET target_amount=?1 WHERE id=?2", rusqlite::params![target, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn delete_savings_goal(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM savings_goals WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn add_subscription(name: String, amount: f64, currency: Option<String>, period: Option<String>, next_payment: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO subscriptions (name, amount, currency, period, next_payment, category, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, amount, currency.unwrap_or_else(|| "KZT".into()), period.unwrap_or_else(|| "monthly".into()),
            next_payment, category.unwrap_or_else(|| "other".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_subscriptions(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, currency, period, next_payment, category, active FROM subscriptions ORDER BY active DESC, name"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "currency": row.get::<_, String>(3)?,
            "period": row.get::<_, String>(4)?, "next_payment": row.get::<_, Option<String>>(5)?,
            "category": row.get::<_, String>(6)?, "active": row.get::<_, i32>(7)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_subscription(id: i64, active: Option<bool>, amount: Option<f64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(a) = active { conn.execute("UPDATE subscriptions SET active=?1 WHERE id=?2", rusqlite::params![a as i32, id]).map_err(|e| format!("DB error: {}", e))?; }
    if let Some(amt) = amount { conn.execute("UPDATE subscriptions SET amount=?1 WHERE id=?2", rusqlite::params![amt, id]).map_err(|e| format!("DB error: {}", e))?; }
    Ok(())
}

#[tauri::command]
fn delete_subscription(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM subscriptions WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn add_debt(name: String, debt_type: String, amount: f64, interest_rate: Option<f64>, due_date: Option<String>, description: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO debts (name, type, amount, remaining, interest_rate, due_date, description, created_at) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, debt_type, amount, interest_rate.unwrap_or(0.0), due_date, description.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_debts(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, type, amount, remaining, interest_rate, due_date, description FROM debts WHERE remaining > 0 ORDER BY due_date NULLS LAST"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        let amt: f64 = row.get(3)?;
        let rem: f64 = row.get(4)?;
        let pct = if amt > 0.0 { ((amt - rem) / amt * 100.0).min(100.0) } else { 0.0 };
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?, "amount": amt, "remaining": rem,
            "interest_rate": row.get::<_, f64>(5)?, "due_date": row.get::<_, Option<String>>(6)?,
            "description": row.get::<_, String>(7)?, "paid_percent": format!("{:.0}", pct),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_debt(id: i64, pay_amount: Option<f64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(pay) = pay_amount {
        conn.execute("UPDATE debts SET remaining = MAX(0, remaining - ?1) WHERE id=?2", rusqlite::params![pay, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn delete_debt(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM debts WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.8.0: Mindset commands ──

#[tauri::command]
fn save_journal_entry(
    date: Option<String>, mood: i32, energy: i32, stress: i32,
    gratitude: Option<String>, reflection: Option<String>,
    wins: Option<String>, struggles: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO journal_entries (date, mood, energy, stress, gratitude, reflection, wins, struggles, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(date) DO UPDATE SET mood=?2, energy=?3, stress=?4, gratitude=?5, reflection=?6, wins=?7, struggles=?8",
        rusqlite::params![d, mood, energy, stress, gratitude.unwrap_or_default(),
            reflection.unwrap_or_default(), wins.unwrap_or_default(), struggles.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_journal_entries(period: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let days = period.unwrap_or(30);
    let since = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, date, mood, energy, stress, gratitude, reflection, wins, struggles FROM journal_entries WHERE date>=?1 ORDER BY date DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![since], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
            "mood": row.get::<_, i32>(2)?, "energy": row.get::<_, i32>(3)?,
            "stress": row.get::<_, i32>(4)?, "gratitude": row.get::<_, String>(5)?,
            "reflection": row.get::<_, String>(6)?, "wins": row.get::<_, String>(7)?,
            "struggles": row.get::<_, String>(8)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn get_journal_entry(date: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    conn.query_row(
        "SELECT id, date, mood, energy, stress, gratitude, reflection, wins, struggles FROM journal_entries WHERE date=?1",
        rusqlite::params![date], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
                "mood": row.get::<_, i32>(2)?, "energy": row.get::<_, i32>(3)?,
                "stress": row.get::<_, i32>(4)?, "gratitude": row.get::<_, String>(5)?,
                "reflection": row.get::<_, String>(6)?, "wins": row.get::<_, String>(7)?,
                "struggles": row.get::<_, String>(8)?,
            }))
        },
    ).map_err(|e| format!("Not found: {}", e))
}

#[tauri::command]
fn log_mood(mood: i32, note: Option<String>, trigger: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    conn.execute(
        "INSERT INTO mood_log (date, time, mood, note, trigger_text, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![now.format("%Y-%m-%d").to_string(), now.format("%H:%M").to_string(),
            mood, note.unwrap_or_default(), trigger.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_mood_history(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = days.unwrap_or(7);
    let since = (chrono::Local::now() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, date, time, mood, note, trigger_text FROM mood_log WHERE date>=?1 ORDER BY date DESC, time DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![since], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
            "time": row.get::<_, String>(2)?, "mood": row.get::<_, i32>(3)?,
            "note": row.get::<_, String>(4)?, "trigger": row.get::<_, String>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn create_principle(title: String, description: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO principles (title, description, category, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![title, description.unwrap_or_default(), category.unwrap_or_else(|| "discipline".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_principles(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, description, category, active FROM principles ORDER BY category, created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?, "category": row.get::<_, String>(3)?,
            "active": row.get::<_, i32>(4)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_principle(id: i64, active: Option<bool>, title: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(a) = active { conn.execute("UPDATE principles SET active=?1 WHERE id=?2", rusqlite::params![a as i32, id]).map_err(|e| format!("DB error: {}", e))?; }
    if let Some(t) = title { conn.execute("UPDATE principles SET title=?1 WHERE id=?2", rusqlite::params![t, id]).map_err(|e| format!("DB error: {}", e))?; }
    Ok(())
}

#[tauri::command]
fn delete_principle(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM principles WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_mindset_check(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let (avg_mood, avg_energy, avg_stress, journal_count): (f64, f64, f64, i64) = conn.query_row(
        "SELECT COALESCE(AVG(mood),3), COALESCE(AVG(energy),3), COALESCE(AVG(stress),3), COUNT(*)
         FROM journal_entries WHERE date>=?1",
        rusqlite::params![week_ago], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap_or((3.0, 3.0, 3.0, 0));
    let principles_count: i64 = conn.query_row("SELECT COUNT(*) FROM principles WHERE active=1", [], |row| row.get(0)).unwrap_or(0);
    Ok(serde_json::json!({
        "avg_mood": format!("{:.1}", avg_mood), "avg_energy": format!("{:.1}", avg_energy),
        "avg_stress": format!("{:.1}", avg_stress), "journal_streak": journal_count,
        "active_principles": principles_count,
    }))
}

// ── v0.8.0: Blocklist commands ──

#[tauri::command]
fn add_to_blocklist(block_type: String, value: String, schedule: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO blocklist (type, value, schedule, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![block_type, value, schedule, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn remove_from_blocklist(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM blocklist WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_blocklist(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, type, value, schedule, active FROM blocklist ORDER BY type, value"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "type": row.get::<_, String>(1)?,
            "value": row.get::<_, String>(2)?, "schedule": row.get::<_, Option<String>>(3)?,
            "active": row.get::<_, i32>(4)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn toggle_blocklist_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE blocklist SET active = 1 - active WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── v0.8.0: Goals & Settings commands ──

#[tauri::command]
fn create_goal(tab_name: String, title: String, target_value: f64, unit: Option<String>, deadline: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tab_goals (tab_name, title, target_value, unit, deadline, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![tab_name, title, target_value, unit.unwrap_or_default(), deadline, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_goals(tab_name: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(t) = tab_name {
        let mut stmt = conn.prepare(
            "SELECT id, tab_name, title, target_value, current_value, unit, deadline, status FROM tab_goals WHERE tab_name=?1 AND status='active' ORDER BY created_at"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![t], |row| goal_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, tab_name, title, target_value, current_value, unit, deadline, status FROM tab_goals WHERE status='active' ORDER BY tab_name, created_at"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| goal_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

fn goal_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    let target: f64 = row.get(3)?;
    let current: f64 = row.get(4)?;
    let pct = if target > 0.0 { (current / target * 100.0).min(100.0) } else { 0.0 };
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "tab_name": row.get::<_, String>(1)?,
        "title": row.get::<_, String>(2)?, "target_value": target,
        "current_value": current, "unit": row.get::<_, String>(5)?,
        "deadline": row.get::<_, Option<String>>(6)?, "status": row.get::<_, String>(7)?,
        "percent": format!("{:.0}", pct),
    }))
}

#[tauri::command]
fn update_goal(id: i64, current_value: Option<f64>, status: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = current_value { conn.execute("UPDATE tab_goals SET current_value=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| format!("DB error: {}", e))?; }
    if let Some(s) = status { conn.execute("UPDATE tab_goals SET status=?1 WHERE id=?2", rusqlite::params![s, id]).map_err(|e| format!("DB error: {}", e))?; }
    Ok(())
}

#[tauri::command]
fn delete_goal(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM tab_goals WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn set_app_setting(key: String, value: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
        rusqlite::params![key, value],
    ).map_err(|e| format!("DB error: {}", e))?;
    // Sync calendar toggle to static flag
    if key == "apple_calendar_enabled" {
        APPLE_CALENDAR_DISABLED.store(value == "false", Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
fn get_app_setting(key: String, db: tauri::State<'_, HanniDb>) -> Result<Option<String>, String> {
    let conn = db.conn();
    let result: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1", rusqlite::params![key], |row| row.get(0),
    ).ok();
    Ok(result)
}

// ── Home Items ──

#[tauri::command]
fn add_home_item(name: String, category: String, quantity: Option<f64>, unit: Option<String>, location: String, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("INSERT INTO home_items (name,category,quantity,unit,location,notes) VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![name, category, quantity, unit, location, notes]).map_err(|e| e.to_string())?;
    Ok("added".into())
}

#[tauri::command]
fn get_home_items(category: Option<String>, needed_only: bool, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut sql = "SELECT id,name,category,quantity,unit,location,needed,notes,created_at FROM home_items".to_string();
    let mut conditions = Vec::new();
    if let Some(ref c) = category { conditions.push(format!("category='{}'", c)); }
    if needed_only { conditions.push("needed=1".to_string()); }
    if !conditions.is_empty() { sql += &format!(" WHERE {}", conditions.join(" AND ")); }
    sql += " ORDER BY needed DESC, name ASC";
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,i64>(0)?, "name": row.get::<_,String>(1)?,
            "category": row.get::<_,String>(2)?, "quantity": row.get::<_,Option<f64>>(3)?,
            "unit": row.get::<_,Option<String>>(4)?, "location": row.get::<_,String>(5)?,
            "needed": row.get::<_,i64>(6)? != 0, "notes": row.get::<_,Option<String>>(7)?,
            "created_at": row.get::<_,String>(8)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn update_home_item(id: i64, name: Option<String>, quantity: Option<f64>, location: Option<String>, notes: Option<String>, needed: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let mut updates = vec!["updated_at=datetime('now')".to_string()];
    if let Some(v) = &name { updates.push(format!("name='{}'", v)); }
    if let Some(v) = quantity { updates.push(format!("quantity={}", v)); }
    if let Some(v) = &location { updates.push(format!("location='{}'", v)); }
    if let Some(v) = &notes { updates.push(format!("notes='{}'", v)); }
    if let Some(v) = needed { updates.push(format!("needed={}", if v { 1 } else { 0 })); }
    conn.execute(&format!("UPDATE home_items SET {} WHERE id=?1", updates.join(",")), rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("updated".into())
}

#[tauri::command]
fn delete_home_item(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("DELETE FROM home_items WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("deleted".into())
}

#[tauri::command]
fn toggle_home_item_needed(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE home_items SET needed = CASE WHEN needed=1 THEN 0 ELSE 1 END, updated_at=datetime('now') WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

// ── People / Contacts ──

#[tauri::command]
fn add_contact(
    name: String,
    phone: Option<String>,
    email: Option<String>,
    category: Option<String>,
    relationship: Option<String>,
    notes: Option<String>,
    blocked: Option<bool>,
    block_reason: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO contacts (name, phone, email, category, relationship, notes, blocked, block_reason, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,datetime('now'),datetime('now'))",
        rusqlite::params![name, phone, email, category.unwrap_or("other".into()), relationship, notes, blocked.unwrap_or(false) as i32, block_reason],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_contacts(category: Option<String>, blocked: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut sql = "SELECT id, name, phone, email, category, relationship, notes, blocked, block_reason, favorite, created_at, updated_at FROM contacts WHERE 1=1".to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref cat) = category {
        sql.push_str(&format!(" AND category=?{}", params.len() + 1));
        params.push(Box::new(cat.clone()));
    }
    if let Some(b) = blocked {
        sql.push_str(&format!(" AND blocked=?{}", params.len() + 1));
        params.push(Box::new(b as i32));
    }
    sql.push_str(" ORDER BY favorite DESC, name ASC");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "phone": row.get::<_, Option<String>>(2)?,
            "email": row.get::<_, Option<String>>(3)?,
            "category": row.get::<_, String>(4)?,
            "relationship": row.get::<_, Option<String>>(5)?,
            "notes": row.get::<_, Option<String>>(6)?,
            "blocked": row.get::<_, i32>(7)? != 0,
            "block_reason": row.get::<_, Option<String>>(8)?,
            "favorite": row.get::<_, i32>(9)? != 0,
            "created_at": row.get::<_, String>(10)?,
            "updated_at": row.get::<_, String>(11)?,
        }))
    }).map_err(|e| e.to_string())?;
    let items: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!(items))
}

#[tauri::command]
fn update_contact(
    id: i64,
    name: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    category: Option<String>,
    relationship: Option<String>,
    notes: Option<String>,
    blocked: Option<bool>,
    block_reason: Option<String>,
    favorite: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    if let Some(v) = name { conn.execute("UPDATE contacts SET name=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = phone { conn.execute("UPDATE contacts SET phone=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = email { conn.execute("UPDATE contacts SET email=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = category { conn.execute("UPDATE contacts SET category=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = relationship { conn.execute("UPDATE contacts SET relationship=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = notes { conn.execute("UPDATE contacts SET notes=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = blocked { conn.execute("UPDATE contacts SET blocked=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v as i32, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = block_reason { conn.execute("UPDATE contacts SET block_reason=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = favorite { conn.execute("UPDATE contacts SET favorite=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![v as i32, id]).map_err(|e| e.to_string())?; }
    Ok("updated".into())
}

#[tauri::command]
fn delete_contact(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("DELETE FROM contacts WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("deleted".into())
}

#[tauri::command]
fn toggle_contact_blocked(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE contacts SET blocked = CASE WHEN blocked=1 THEN 0 ELSE 1 END, updated_at=datetime('now') WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

#[tauri::command]
fn toggle_contact_favorite(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE contacts SET favorite = CASE WHEN favorite=1 THEN 0 ELSE 1 END, updated_at=datetime('now') WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

// ── Contact blocks (per-person site/app blocking) ──

#[tauri::command]
fn add_contact_block(
    contact_id: i64,
    block_type: Option<String>,
    value: String,
    reason: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO contact_blocks (contact_id, block_type, value, reason) VALUES (?1,?2,?3,?4)",
        rusqlite::params![contact_id, block_type.unwrap_or("site".into()), value, reason],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn get_contact_blocks(contact_id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, contact_id, block_type, value, reason, active, created_at FROM contact_blocks WHERE contact_id=?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![contact_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "contact_id": row.get::<_, i64>(1)?,
            "block_type": row.get::<_, String>(2)?,
            "value": row.get::<_, String>(3)?,
            "reason": row.get::<_, Option<String>>(4)?,
            "active": row.get::<_, i32>(5)? != 0,
            "created_at": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| e.to_string())?;
    let items: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!(items))
}

#[tauri::command]
fn delete_contact_block(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("DELETE FROM contact_blocks WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("deleted".into())
}

#[tauri::command]
fn toggle_contact_block_active(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    conn.execute("UPDATE contact_blocks SET active = CASE WHEN active=1 THEN 0 ELSE 1 END WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok("toggled".into())
}

// ── v0.9.0: Page Meta & Custom Properties ──

#[tauri::command]
fn get_page_meta(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let result = conn.query_row(
        "SELECT tab_id, emoji, title, description, updated_at FROM page_meta WHERE tab_id=?1",
        rusqlite::params![tab_id],
        |row| Ok(serde_json::json!({
            "tab_id": row.get::<_, String>(0)?,
            "emoji": row.get::<_, Option<String>>(1)?,
            "title": row.get::<_, Option<String>>(2)?,
            "description": row.get::<_, Option<String>>(3)?,
            "updated_at": row.get::<_, String>(4)?,
        }))
    );
    match result {
        Ok(v) => Ok(v),
        Err(_) => Ok(serde_json::json!(null)),
    }
}

#[tauri::command]
fn update_page_meta(tab_id: String, emoji: Option<String>, title: Option<String>, description: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO page_meta (tab_id, emoji, title, description, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(tab_id) DO UPDATE SET
         emoji=COALESCE(?2, emoji), title=COALESCE(?3, title),
         description=COALESCE(?4, description), updated_at=?5",
        rusqlite::params![tab_id, emoji, title, description, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_property_definitions(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, tab_id, name, type, position, color, options, default_value, visible
         FROM property_definitions WHERE tab_id=?1 ORDER BY position"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![tab_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "tab_id": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "type": row.get::<_, String>(3)?,
            "position": row.get::<_, i64>(4)?,
            "color": row.get::<_, Option<String>>(5)?,
            "options": row.get::<_, Option<String>>(6)?,
            "default_value": row.get::<_, Option<String>>(7)?,
            "visible": row.get::<_, i64>(8)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn create_property_definition(tab_id: String, name: String, prop_type: String, position: Option<i64>, color: Option<String>, options: Option<String>, default_value: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let pos = position.unwrap_or_else(|| {
        conn.query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM property_definitions WHERE tab_id=?1",
            rusqlite::params![tab_id], |row| row.get::<_, i64>(0)).unwrap_or(0)
    });
    conn.execute(
        "INSERT INTO property_definitions (tab_id, name, type, position, color, options, default_value, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![tab_id, name, prop_type, pos, color, options, default_value, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_property_definition(id: i64, name: Option<String>, prop_type: Option<String>, position: Option<i64>, color: Option<String>, options: Option<String>, visible: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(n) = name { conn.execute("UPDATE property_definitions SET name=?1 WHERE id=?2", rusqlite::params![n, id]).map_err(|e| e.to_string())?; }
    if let Some(t) = prop_type { conn.execute("UPDATE property_definitions SET type=?1 WHERE id=?2", rusqlite::params![t, id]).map_err(|e| e.to_string())?; }
    if let Some(p) = position { conn.execute("UPDATE property_definitions SET position=?1 WHERE id=?2", rusqlite::params![p, id]).map_err(|e| e.to_string())?; }
    if let Some(c) = color { conn.execute("UPDATE property_definitions SET color=?1 WHERE id=?2", rusqlite::params![c, id]).map_err(|e| e.to_string())?; }
    if let Some(o) = options { conn.execute("UPDATE property_definitions SET options=?1 WHERE id=?2", rusqlite::params![o, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = visible { conn.execute("UPDATE property_definitions SET visible=?1 WHERE id=?2", rusqlite::params![v as i32, id]).map_err(|e| e.to_string())?; }
    Ok(())
}

#[tauri::command]
fn delete_property_definition(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM property_values WHERE property_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM property_definitions WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_property_values(record_table: String, record_ids: Vec<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if record_ids.is_empty() { return Ok(vec![]); }
    let placeholders: Vec<String> = record_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
    let sql = format!(
        "SELECT pv.id, pv.record_id, pv.record_table, pv.property_id, pv.value, pd.name, pd.type
         FROM property_values pv JOIN property_definitions pd ON pd.id = pv.property_id
         WHERE pv.record_table=?1 AND pv.record_id IN ({})",
        placeholders.join(",")
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params.push(Box::new(record_table));
    for id in &record_ids { params.push(Box::new(*id)); }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "record_id": row.get::<_, i64>(1)?,
            "record_table": row.get::<_, String>(2)?,
            "property_id": row.get::<_, i64>(3)?,
            "value": row.get::<_, Option<String>>(4)?,
            "prop_name": row.get::<_, String>(5)?,
            "prop_type": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn set_property_value(record_id: i64, record_table: String, property_id: i64, value: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO property_values (record_id, record_table, property_id, value)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(record_id, record_table, property_id) DO UPDATE SET value=?4",
        rusqlite::params![record_id, record_table, property_id, value],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn delete_property_value(record_id: i64, record_table: String, property_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM property_values WHERE record_id=?1 AND record_table=?2 AND property_id=?3",
        rusqlite::params![record_id, record_table, property_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_view_configs(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, tab_id, name, view_type, filter_json, sort_json, visible_columns, is_default, position
         FROM view_configs WHERE tab_id=?1 ORDER BY position"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![tab_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "tab_id": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "view_type": row.get::<_, String>(3)?,
            "filter_json": row.get::<_, Option<String>>(4)?,
            "sort_json": row.get::<_, Option<String>>(5)?,
            "visible_columns": row.get::<_, Option<String>>(6)?,
            "is_default": row.get::<_, i64>(7)? != 0,
            "position": row.get::<_, Option<i64>>(8)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
fn create_view_config(tab_id: String, name: String, view_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let vt = view_type.unwrap_or_else(|| "table".into());
    conn.execute(
        "INSERT INTO view_configs (tab_id, name, view_type, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![tab_id, name, vt, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn update_view_config(id: i64, filter_json: Option<String>, sort_json: Option<String>, visible_columns: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(f) = filter_json { conn.execute("UPDATE view_configs SET filter_json=?1 WHERE id=?2", rusqlite::params![f, id]).map_err(|e| e.to_string())?; }
    if let Some(s) = sort_json { conn.execute("UPDATE view_configs SET sort_json=?1 WHERE id=?2", rusqlite::params![s, id]).map_err(|e| e.to_string())?; }
    if let Some(v) = visible_columns { conn.execute("UPDATE view_configs SET visible_columns=?1 WHERE id=?2", rusqlite::params![v, id]).map_err(|e| e.to_string())?; }
    Ok(())
}

// ── Integrations info ──

#[derive(Serialize)]
struct IntegrationItem {
    name: String,
    status: String,  // "active", "inactive", "blocked"
    detail: String,
}

#[derive(Serialize)]
struct IntegrationsInfo {
    access: Vec<IntegrationItem>,
    tracking: Vec<IntegrationItem>,
    blocked_apps: Vec<IntegrationItem>,
    blocked_sites: Vec<IntegrationItem>,
    blocker_active: bool,
    macos: Vec<IntegrationItem>,
}

#[tauri::command]
async fn get_integrations() -> Result<IntegrationsInfo, String> {
    // ── Access ──
    let tracker_path = data_file_path();
    let tracker_exists = tracker_path.exists();
    let access = vec![
        IntegrationItem {
            name: "Life Tracker".into(),
            status: if tracker_exists { "active" } else { "inactive" }.into(),
            detail: if tracker_exists {
                "~/Library/Application Support/Hanni/life-tracker-data.json".into()
            } else {
                "Файл не найден".into()
            },
        },
        IntegrationItem {
            name: "File System".into(),
            status: "active".into(),
            detail: "$HOME/** — чтение файлов".into(),
        },
        IntegrationItem {
            name: "Shell".into(),
            status: "active".into(),
            detail: "Выполнение команд".into(),
        },
    ];

    // ── Tracking ──
    let tracking = if tracker_exists {
        let data = load_tracker_data().unwrap_or(TrackerData {
            purchases: vec![], time_entries: vec![], goals: vec![], notes: vec![],
            settings: serde_json::Value::Null,
        });
        vec![
            IntegrationItem {
                name: "Расходы".into(),
                status: "active".into(),
                detail: format!("{} записей", data.purchases.len()),
            },
            IntegrationItem {
                name: "Время".into(),
                status: "active".into(),
                detail: format!("{} записей", data.time_entries.len()),
            },
            IntegrationItem {
                name: "Цели".into(),
                status: "active".into(),
                detail: format!("{} целей", data.goals.len()),
            },
            IntegrationItem {
                name: "Заметки".into(),
                status: "active".into(),
                detail: format!("{} заметок", data.notes.len()),
            },
        ]
    } else {
        vec![IntegrationItem {
            name: "Life Tracker".into(),
            status: "inactive".into(),
            detail: "Не подключен".into(),
        }]
    };

    // ── Blocker config ──
    let blocker_config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("hanni/blocker_config.json");

    let default_apps = vec!["Telegram", "Discord", "Slack", "Safari"];
    let default_sites = vec![
        "youtube.com", "twitter.com", "x.com", "instagram.com",
        "facebook.com", "tiktok.com", "reddit.com", "vk.com", "netflix.com",
    ];

    let (apps, sites) = if blocker_config_path.exists() {
        let content = std::fs::read_to_string(&blocker_config_path).unwrap_or_default();
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
            let apps: Vec<String> = cfg["apps"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_apps.iter().map(|s| s.to_string()).collect());
            let sites: Vec<String> = cfg["sites"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_sites.iter().map(|s| s.to_string()).collect());
            (apps, sites)
        } else {
            (default_apps.iter().map(|s| s.to_string()).collect(),
             default_sites.iter().map(|s| s.to_string()).collect())
        }
    } else {
        (default_apps.iter().map(|s| s.to_string()).collect(),
         default_sites.iter().map(|s| s.to_string()).collect())
    };

    // Check if blocking is active via /etc/hosts
    let blocker_active = std::fs::read_to_string("/etc/hosts")
        .map(|c| c.contains("# === HANNI FOCUS BLOCKER ==="))
        .unwrap_or(false);

    let blocked_apps = apps.iter().map(|a| IntegrationItem {
        name: a.clone(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: format!("/Applications/{}.app", a),
    }).collect();

    // Deduplicate sites (remove www. variants for display)
    let unique_sites: Vec<&String> = sites.iter()
        .filter(|s| !s.starts_with("www."))
        .collect();

    let blocked_sites = unique_sites.iter().map(|s| IntegrationItem {
        name: s.to_string(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: if blocker_active { "Заблокирован" } else { "Не заблокирован" }.into(),
    }).collect();

    // ── macOS integrations ──
    let macos = vec![
        IntegrationItem {
            name: "Screen Time".into(),
            status: "ready".into(),
            detail: "knowledgeC.db · по запросу".into(),
        },
        IntegrationItem {
            name: "Календарь".into(),
            status: "ready".into(),
            detail: "Calendar.app · по запросу".into(),
        },
        IntegrationItem {
            name: "Музыка".into(),
            status: "ready".into(),
            detail: "Music / Spotify · по запросу".into(),
        },
        IntegrationItem {
            name: "Браузер".into(),
            status: "ready".into(),
            detail: "Safari / Chrome / Arc · по запросу".into(),
        },
    ];

    Ok(IntegrationsInfo {
        access,
        tracking,
        blocked_apps,
        blocked_sites,
        blocker_active,
        macos,
    })
}

// ── Model info ──

#[derive(Serialize)]
struct ModelInfo {
    model_name: String,
    server_url: String,
    server_online: bool,
}

#[tauri::command]
async fn get_model_info() -> Result<ModelInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    let online = client
        .get("http://127.0.0.1:8234/v1/models")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    Ok(ModelInfo {
        model_name: MODEL.to_string(),
        server_url: MLX_URL.to_string(),
        server_online: online,
    })
}

// ── Proactive messaging logic ──

const PROACTIVE_PROMPT_HEADER: &str = r#"You are Hanni, a warm AI companion living on the user's Mac. You're like a close friend who shares the same space.

Your job: write a short, natural message. Like texting a friend you live with.

YOU HAVE ACCESS TO:
- Activity delta: what changed since your last message (app, music, browser)
- Recent conversation: last messages from the chat (for continuity)
- Active triggers: time-sensitive events (upcoming calendar, distraction alerts)
- Engagement rate: how often the user responds to your messages

VARIETY IS KEY — pick a DIFFERENT style each time from these enabled styles:
"#;

const PROACTIVE_PROMPT_FOOTER: &str = r#"
PRIORITY RULES:
- If there's an active trigger → prioritize (Schedule/Accountability)
- If there's a recent conversation → prefer Continuity style
- If it's morning (8-10am) and first message → prefer Digest style
- If a distracting app is open 30+ min → prefer Accountability style
- If engagement rate is low (<30%) → be more relevant/useful, less chatty

ANTI-REPETITION (CRITICAL):
- Check [Your recent proactive messages] and [Topics covered today] carefully
- ABSOLUTELY DO NOT mention any topic, event, or theme that appears in those lists
- If a calendar event was already mentioned today — DO NOT mention it again
- Each message must cover a COMPLETELY NEW topic
- If you can't think of a new topic → reply [SKIP]
- Always use "ты" form, never "вы"

RULES:
- 1-3 sentences max, Russian by default
- Be genuine — a companion, not a notification bot
- Only reply [SKIP] if nothing meaningful to say

Reply with your message text, or [SKIP]."#;

struct ProactiveStyleDef {
    id: &'static str,
    description: &'static str,
}

const ALL_PROACTIVE_STYLES: &[ProactiveStyleDef] = &[
    ProactiveStyleDef { id: "observation", description: "Observation: comment on their screen/music/browsing" },
    ProactiveStyleDef { id: "calendar", description: "Calendar: mention upcoming events, warn about events starting soon" },
    ProactiveStyleDef { id: "nudge", description: "Nudge: gentle productivity/health reminder" },
    ProactiveStyleDef { id: "curiosity", description: "Curiosity: ask about their day/project/mood" },
    ProactiveStyleDef { id: "humor", description: "Humor: light joke or playful tease about their habits" },
    ProactiveStyleDef { id: "care", description: "Care: check in on mood trends, suggest breaks" },
    ProactiveStyleDef { id: "memory", description: "Memory: reference something you remember about them" },
    ProactiveStyleDef { id: "food", description: "Food: warn about expiring products if any" },
    ProactiveStyleDef { id: "goals", description: "Goals: celebrate progress or remind about deadlines" },
    ProactiveStyleDef { id: "journal", description: "Journal: remind them to write evening reflection" },
    ProactiveStyleDef { id: "digest", description: "Digest: MORNING ONLY (8-10am) — give a brief summary: today's events, deadlines, yesterday mood/sleep if known" },
    ProactiveStyleDef { id: "accountability", description: "Accountability: if they've been in the same app (YouTube, Reddit, Twitter, TikTok, etc.) for 30+ min, gently point it out" },
    ProactiveStyleDef { id: "schedule", description: "Schedule: if there's an upcoming event within 30 min, remind them to prepare" },
    ProactiveStyleDef { id: "continuity", description: "Continuity: continue or follow up on a topic from the recent conversation" },
];

fn build_proactive_system_prompt(enabled_styles: &[String]) -> String {
    let mut prompt = PROACTIVE_PROMPT_HEADER.to_string();
    let styles: Vec<&ProactiveStyleDef> = if enabled_styles.is_empty() {
        // Empty = all enabled (backward compat)
        ALL_PROACTIVE_STYLES.iter().collect()
    } else {
        ALL_PROACTIVE_STYLES.iter()
            .filter(|s| enabled_styles.iter().any(|e| e == s.id))
            .collect()
    };
    for style in &styles {
        prompt.push_str(&format!("- {}\n", style.description));
    }
    prompt.push_str(PROACTIVE_PROMPT_FOOTER);
    prompt
}

async fn gather_context() -> String {
    // All context functions are internally blocking (run_osascript, rusqlite).
    // Run in spawn_blocking to avoid starving tokio worker threads.
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::task::spawn_blocking(gather_context_blocking),
    ).await {
        Ok(Ok(ctx)) => ctx,
        _ => format!("Current time: {}\n", chrono::Local::now().format("%H:%M %A, %d %B %Y")),
    }
}

// ── Reusable OS-context helpers (used by both gather_context and snapshot collector) ──

fn get_frontmost_app() -> String {
    run_osascript(
        "tell application \"System Events\" to return name of first application process whose frontmost is true"
    ).unwrap_or_default().trim().to_string()
}

fn get_browser_url() -> String {
    let browsers = [
        ("Arc", "tell application \"Arc\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Google Chrome", "tell application \"Google Chrome\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Safari", "tell application \"Safari\" to return URL of front document & \" | \" & name of front document"),
    ];
    for (name, script) in &browsers {
        let check = run_osascript(&format!(
            "tell application \"System Events\" to (name of processes) contains \"{}\"", name
        ));
        if let Ok(ref val) = check {
            if val == "true" {
                if let Ok(info) = run_osascript(script) {
                    return format!("{}: {}", name, info);
                }
            }
        }
    }
    String::new()
}

fn get_now_playing_sync() -> String {
    let music_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Music\""
    );
    if let Ok(ref val) = music_check {
        if val == "true" {
            if let Ok(info) = run_osascript(
                "tell application \"Music\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Music paused\" end if"
            ) {
                return info;
            }
        }
    }
    String::new()
}

fn gather_context_blocking() -> String {
    let now = chrono::Local::now();
    let mut ctx = format!("Current time: {}\n", now.format("%H:%M %A, %d %B %Y"));

    // Screen Time (SQLite query — fast, no osascript)
    if let Ok(activity) = gather_screen_time() {
        ctx.push_str(&format!("\n--- Screen Time ---\n{}\n", activity));
    }

    // Calendar events from Calendar.app (skip if access was denied)
    if check_calendar_access() {
        let cal_script = r#"
            set output to ""
            set today to current date
            set endDate to today + (2 * days)
            tell application "Calendar"
                repeat with cal in calendars
                    set evts to (every event of cal whose start date >= today and start date <= endDate)
                    repeat with evt in evts
                        set evtStart to start date of evt
                        set evtName to summary of evt
                        set output to output & (evtStart as string) & " | " & evtName & linefeed
                    end repeat
                end repeat
            end tell
            if output is "" then
                return "No upcoming events in the next 2 days."
            end if
            return output
        "#;
        if let Ok(calendar) = run_osascript(cal_script) {
            ctx.push_str(&format!("\n--- Calendar ---\n{}\n", calendar));
        }
    }

    // Now playing
    let music = get_now_playing_sync();
    if !music.is_empty() {
        ctx.push_str(&format!("\n--- Music ---\nApple Music: {}\n", music));
    }

    // Browser tab
    let browser = get_browser_url();
    if !browser.is_empty() {
        ctx.push_str(&format!("\n--- Browser ---\n{}\n", browser));
    }

    // Active (frontmost) app and how long it's been in focus
    let front_app = get_frontmost_app();
    if !front_app.is_empty() {
        ctx.push_str(&format!("\n--- Active App ---\nFrontmost: {}\n", front_app));
        if let Ok(minutes) = get_app_focus_minutes(&front_app) {
            ctx.push_str(&format!("Focus time today: {:.0} min\n", minutes));
            let distracting = ["YouTube", "Reddit", "Twitter", "TikTok", "Instagram", "Telegram", "Discord", "VK"];
            let is_distracting = distracting.iter().any(|d| front_app.contains(d));
            if is_distracting && minutes > 30.0 {
                ctx.push_str("⚠ Distraction alert: user has been on this app 30+ min!\n");
            }
        }
    }

    // Upcoming events within next 60 min (for schedule reminders)
    if let Ok(upcoming) = get_upcoming_events_soon() {
        if !upcoming.is_empty() {
            ctx.push_str(&format!("\n--- Coming Up Soon ---\n{}\n", upcoming));
        }
    }

    // Morning digest context: yesterday's mood, sleep, today's event count
    let hour = now.hour();
    if hour >= 8 && hour <= 10 {
        if let Ok(digest) = gather_morning_digest() {
            ctx.push_str(&format!("\n--- Morning Digest Data ---\n{}\n", digest));
        }
    }

    ctx
}

fn get_app_focus_minutes(app_name: &str) -> Result<f64, String> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/Knowledge/knowledgeC.db");
    if !db_path.exists() { return Err("No Screen Time DB".into()); }
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).map_err(|e| e.to_string())?;
    let minutes: f64 = conn.query_row(
        "SELECT COALESCE(ROUND(SUM(CAST((ZOBJECT.ZENDDATE - ZOBJECT.ZSTARTDATE) AS REAL)) / 60, 1), 0)
         FROM ZOBJECT JOIN ZSOURCE ON ZOBJECT.ZSOURCE = ZSOURCE.Z_PK
         WHERE DATE(datetime(ZOBJECT.ZSTARTDATE + 978307200, 'unixepoch', 'localtime')) = DATE('now')
               AND ZOBJECT.ZSTREAMNAME = '/app/inFocus'
               AND ZOBJECT.ZENDDATE > ZOBJECT.ZSTARTDATE
               AND ZSOURCE.ZNAME LIKE ?1",
        rusqlite::params![format!("%{}%", app_name)],
        |row| row.get(0),
    ).unwrap_or(0.0);
    Ok(minutes)
}

fn get_upcoming_events_soon() -> Result<String, String> {
    let db_path = hanni_db_path();
    if !db_path.exists() { return Ok(String::new()); }
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let current_time = now.format("%H:%M").to_string();
    let soon_time = (now + chrono::Duration::minutes(60)).format("%H:%M").to_string();
    let mut stmt = conn.prepare(
        "SELECT title, time, duration FROM events WHERE date = ?1 AND time >= ?2 AND time <= ?3 ORDER BY time"
    ).map_err(|e| e.to_string())?;
    let events: Vec<String> = stmt.query_map(
        rusqlite::params![today, current_time, soon_time],
        |row| {
            let title: String = row.get(0)?;
            let time: String = row.get(1)?;
            let dur: i64 = row.get(2)?;
            Ok(format!("{} — {} ({}мин)", time, title, dur))
        },
    ).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(events.join("\n"))
}

fn gather_morning_digest() -> Result<String, String> {
    let db_path = hanni_db_path();
    if !db_path.exists() { return Ok(String::new()); }
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    let mut digest = String::new();

    // Today's events count
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let event_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE date = ?1",
        rusqlite::params![today],
        |row| row.get(0),
    ).unwrap_or(0);
    digest.push_str(&format!("Today's events: {}\n", event_count));

    // Yesterday's mood
    let yesterday = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    if let Ok((mood, note)) = conn.query_row(
        "SELECT mood, note FROM mood_log WHERE date(created_at) = ?1 ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![yesterday],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
    ) {
        digest.push_str(&format!("Yesterday's mood: {}/5", mood));
        if let Some(n) = note { digest.push_str(&format!(" ({})", n)); }
        digest.push('\n');
    }

    // Yesterday's sleep
    if let Ok(sleep) = conn.query_row(
        "SELECT sleep_hours FROM health_log WHERE date(logged_at) = ?1 ORDER BY logged_at DESC LIMIT 1",
        rusqlite::params![yesterday],
        |row| row.get::<_, Option<f64>>(0),
    ) {
        if let Some(h) = sleep {
            digest.push_str(&format!("Yesterday's sleep: {:.1}h\n", h));
        }
    }

    // Active goals count
    let goals_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM goals WHERE progress < target",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if goals_count > 0 {
        digest.push_str(&format!("Active goals: {}\n", goals_count));
    }

    Ok(digest)
}

fn gather_screen_time() -> Result<String, String> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/Knowledge/knowledgeC.db");
    if !db_path.exists() { return Err("No Screen Time DB".into()); }
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT ZSOURCE.ZNAME as app_name,
                ROUND(SUM(CAST((ZOBJECT.ZENDDATE - ZOBJECT.ZSTARTDATE) AS REAL)) / 60, 1) as minutes
         FROM ZOBJECT JOIN ZSOURCE ON ZOBJECT.ZSOURCE = ZSOURCE.Z_PK
         WHERE DATE(datetime(ZOBJECT.ZSTARTDATE + 978307200, 'unixepoch', 'localtime')) = DATE('now')
               AND ZOBJECT.ZSTREAMNAME = '/app/inFocus' AND ZOBJECT.ZENDDATE > ZOBJECT.ZSTARTDATE
         GROUP BY ZSOURCE.ZBUNDLEID ORDER BY minutes DESC"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<(String, f64, String)> = stmt.query_map([], |row| {
        let app: String = row.get::<_, Option<String>>(0)?.unwrap_or_default();
        let min: f64 = row.get(1)?;
        Ok((app, min))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .map(|(app, min)| { let cat = classify_app(&app).to_string(); (app, min, cat) })
    .collect();
    if rows.is_empty() { return Ok("No Screen Time data for today yet.".into()); }
    let (mut prod, mut dist, mut neut) = (0.0, 0.0, 0.0);
    for r in &rows { match r.2.as_str() { "productive" => prod += r.1, "distraction" => dist += r.1, _ => neut += r.1 } }
    let top: Vec<String> = rows.iter().take(5).map(|r| format!("  {} — {:.0} min ({})", r.0, r.1, r.2)).collect();
    Ok(format!("Productive: {:.0} min | Distraction: {:.0} min | Neutral: {:.0} min\n{}", prod, dist, neut, top.join("\n")))
}

#[derive(Deserialize)]
struct NonStreamChoice {
    message: NonStreamMessage,
}

#[derive(Deserialize)]
struct NonStreamMessage {
    content: String,
}

#[derive(Deserialize)]
struct NonStreamResponse {
    choices: Vec<NonStreamChoice>,
}

fn compute_activity_delta(old_ctx: &str, new_ctx: &str) -> String {
    let mut deltas = Vec::new();
    // Extract sections from context strings
    fn extract_section(ctx: &str, tag: &str) -> String {
        ctx.lines()
            .skip_while(|l| !l.contains(tag))
            .skip(1)
            .take_while(|l| !l.starts_with("---"))
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }
    let old_app = extract_section(old_ctx, "Active App");
    let new_app = extract_section(new_ctx, "Active App");
    if !old_app.is_empty() && !new_app.is_empty() && old_app != new_app {
        deltas.push(format!("App changed: {} → {}", old_app.lines().next().unwrap_or(""), new_app.lines().next().unwrap_or("")));
    }
    let old_music = extract_section(old_ctx, "Music");
    let new_music = extract_section(new_ctx, "Music");
    if !old_music.is_empty() && old_music != new_music {
        deltas.push(format!("Music changed: {} → {}", old_music, if new_music.is_empty() { "stopped" } else { &new_music }));
    }
    let old_browser = extract_section(old_ctx, "Browser");
    let new_browser = extract_section(new_ctx, "Browser");
    if !old_browser.is_empty() && old_browser != new_browser && !new_browser.is_empty() {
        deltas.push(format!("Browser: {} → {}", old_browser, new_browser));
    }
    deltas.join("\n")
}

fn get_recent_chat_snippet(conn: &rusqlite::Connection, limit: usize) -> String {
    // Get the latest conversation and extract last N messages
    let messages_json: String = conn.query_row(
        "SELECT messages FROM conversations ORDER BY id DESC LIMIT 1",
        [], |row| row.get(0),
    ).unwrap_or_default();
    if messages_json.is_empty() {
        return String::new();
    }
    // Messages stored as JSON array of [role, content] pairs
    if let Ok(msgs) = serde_json::from_str::<Vec<Vec<String>>>(&messages_json) {
        let start = msgs.len().saturating_sub(limit);
        msgs[start..].iter()
            .map(|m| {
                let role = m.first().map(|s| s.as_str()).unwrap_or("?");
                let content = m.get(1).map(|s| s.as_str()).unwrap_or("");
                // Truncate long messages
                let short = if content.len() > 150 { &content[..150] } else { content };
                format!("{}: {}", if role == "user" { "User" } else { "Hanni" }, short)
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    }
}

fn get_todays_proactive_messages(conn: &rusqlite::Connection) -> Vec<String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut msgs = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT message FROM proactive_history WHERE sent_at >= ?1 ORDER BY id ASC"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![today], |row| {
            row.get::<_, String>(0)
        }) {
            for msg in rows.flatten() {
                msgs.push(msg);
            }
        }
    }
    msgs
}

fn get_user_name_from_memory(conn: &rusqlite::Connection) -> String {
    // Look for user's name in facts table
    conn.query_row(
        "SELECT value FROM facts WHERE category = 'user' AND (key LIKE '%имя%' OR key LIKE '%name%' OR key LIKE '%зовут%') LIMIT 1",
        [], |row| row.get::<_, String>(0),
    ).unwrap_or_default()
}

async fn proactive_llm_call(
    client: &reqwest::Client,
    context: &str,
    recent_messages: &[(String, chrono::DateTime<chrono::Local>)],
    consecutive_skips: u32,
    memory_context: &str,
    delta: &str,
    triggers: &[String],
    chat_snippet: &str,
    engagement_rate: f64,
    user_name: &str,
    todays_messages: &[String],
    enabled_styles: &[String],
) -> Result<Option<String>, String> {
    // Build dynamic system prompt from enabled styles
    let mut sys_prompt = build_proactive_system_prompt(enabled_styles);
    if !user_name.is_empty() {
        sys_prompt = format!(
            "IMPORTANT: The user's name is {}. Always call them {} (на \"ты\").\n\n{}",
            user_name, user_name, sys_prompt
        );
    }

    let mut user_content = String::new();
    if !memory_context.is_empty() {
        user_content.push_str(&format!("[Your memories]\n{}\n\n", memory_context));
    }
    user_content.push_str(&format!("{}\n", context));

    // Activity delta
    if !delta.is_empty() {
        user_content.push_str(&format!("\n[Changes since last check]\n{}\n", delta));
    }

    // Active triggers
    if !triggers.is_empty() {
        user_content.push_str(&format!("\n[Active triggers]\n{}\n", triggers.join("\n")));
    }

    // Recent chat snippet
    if !chat_snippet.is_empty() {
        user_content.push_str(&format!("\n[Recent conversation]\n{}\n", chat_snippet));
    }

    // Last N proactive messages with timestamps (in-memory buffer)
    if !recent_messages.is_empty() {
        user_content.push_str("\n[Your recent proactive messages — DO NOT repeat these]\n");
        for (msg, ts) in recent_messages {
            user_content.push_str(&format!("  {} — \"{}\"\n", ts.format("%H:%M"), msg));
        }
    }

    // All today's proactive messages (from DB — survives restart)
    if !todays_messages.is_empty() {
        user_content.push_str(&format!(
            "\n[Topics covered today — {} messages sent, DO NOT repeat ANY of these topics]\n",
            todays_messages.len()
        ));
        for (i, msg) in todays_messages.iter().enumerate() {
            let short = if msg.len() > 100 { &msg[..100] } else { msg.as_str() };
            user_content.push_str(&format!("  {}. \"{}\"\n", i + 1, short));
        }
    }

    user_content.push_str(&format!(
        "\nSkips in a row: {}. Engagement rate: {:.0}%.\n/no_think",
        consecutive_skips, engagement_rate * 100.0
    ));

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: sys_prompt,
            },
            ChatMessage {
                role: "user".into(),
                content: user_content,
            },
        ],
        max_tokens: 300,
        stream: false,
        temperature: 0.85,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    let parsed: NonStreamResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let raw = parsed
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think> tags
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    if text.contains("[SKIP]") || text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn speak_edge_tts(text: &str, voice: &str) {
    let tmp = format!("/tmp/hanni_tts_{}.mp3", std::process::id());
    let tmp2 = tmp.clone();
    let voice_owned = voice.to_string();
    let text_owned = text.to_string();
    std::thread::spawn(move || {
        // Try direct edge-tts binary with PATH fallbacks
        let candidates = ["edge-tts", "/opt/homebrew/bin/edge-tts", "/usr/local/bin/edge-tts"];
        let mut success = false;
        for cmd in &candidates {
            let status = std::process::Command::new(cmd)
                .args(["--voice", &voice_owned, "--rate", "+25%", "--text", &text_owned, "--write-media", &tmp2])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Ok(s) = status {
                if s.success() { success = true; break; }
            }
        }
        // Fallback: python3 -m edge_tts
        if !success {
            if let Some(python) = find_python() {
                let status = std::process::Command::new(&python)
                    .args(["-m", "edge_tts", "--voice", &voice_owned, "--rate", "+25%", "--text", &text_owned, "--write-media", &tmp2])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                if let Ok(s) = status { success = s.success(); }
            }
        }
        if success {
            let _ = std::process::Command::new("afplay").arg(&tmp2).status();
            let _ = std::fs::remove_file(&tmp2);
        }
    });
}

fn speak_remote_tts(text: &str, voice: &str, server_url: &str) {
    let tmp = format!("/tmp/hanni_tts_{}.mp3", std::process::id());
    let tmp2 = tmp.clone();
    let url = format!("{}/tts", server_url.trim_end_matches('/'));
    let text_owned = text.to_string();
    let voice_owned = voice.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build();
        if let Ok(client) = client {
            let resp = client.post(&url)
                .json(&serde_json::json!({"text": text_owned, "voice": voice_owned}))
                .send();
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes() {
                        if std::fs::write(&tmp2, &bytes).is_ok() {
                            let _ = std::process::Command::new("afplay").arg(&tmp2).status();
                            let _ = std::fs::remove_file(&tmp2);
                        }
                    }
                }
            }
        }
    });
}

fn clean_text_for_tts(text: &str) -> String {
    let mut s = text.replace('"', "'");
    // Remove markdown formatting
    s = s.replace("```", "").replace('`', "").replace("**", "").replace('*', "");
    s = s.replace("###", "").replace("##", "").replace('#', "");
    // Remove emojis and misc symbols (Unicode ranges)
    s = s.chars().filter(|c| {
        let cp = *c as u32;
        // Keep basic Latin, Cyrillic, common punctuation, digits
        // Filter out emoji/symbol ranges
        !(
            (0x1F600..=0x1F64F).contains(&cp) || // Emoticons
            (0x1F300..=0x1F5FF).contains(&cp) || // Misc Symbols & Pictographs
            (0x1F680..=0x1F6FF).contains(&cp) || // Transport & Map
            (0x1F700..=0x1F77F).contains(&cp) || // Alchemical
            (0x1F780..=0x1F7FF).contains(&cp) || // Geometric Shapes Extended
            (0x1F800..=0x1F8FF).contains(&cp) || // Supplemental Arrows-C
            (0x1F900..=0x1F9FF).contains(&cp) || // Supplemental Symbols & Pictographs
            (0x1FA00..=0x1FA6F).contains(&cp) || // Chess Symbols
            (0x1FA70..=0x1FAFF).contains(&cp) || // Symbols & Pictographs Extended-A
            (0x2600..=0x26FF).contains(&cp) ||   // Misc symbols (☀☁☂ etc)
            (0x2700..=0x27BF).contains(&cp) ||   // Dingbats (✂✈✉ etc)
            (0x231A..=0x231B).contains(&cp) ||   // Watch, Hourglass
            (0x23E9..=0x23F3).contains(&cp) ||   // Media control
            (0x23F8..=0x23FA).contains(&cp) ||   // Media control
            (0x25AA..=0x25AB).contains(&cp) ||   // Squares
            (0x25B6..=0x25C0).contains(&cp) ||   // Triangles
            (0x25FB..=0x25FE).contains(&cp) ||   // Squares
            (0x2934..=0x2935).contains(&cp) ||   // Arrows
            (0x2B05..=0x2B07).contains(&cp) ||   // Arrows
            (0x2B1B..=0x2B1C).contains(&cp) ||   // Squares
            (0x3030..=0x3030).contains(&cp) ||   // Wavy dash
            (0x303D..=0x303D).contains(&cp) ||   // Part alternation mark
            (0xFE0F..=0xFE0F).contains(&cp) ||   // Variation selector
            (0x200D..=0x200D).contains(&cp) ||   // Zero-width joiner
            (0x20E3..=0x20E3).contains(&cp) ||   // Combining enclosing keycap
            (0xE0020..=0xE007F).contains(&cp)    // Tags
        )
    }).collect::<String>();
    // Collapse multiple spaces/newlines
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result.trim().to_string()
}

fn speak_tts(text: &str, voice: &str) {
    let clean = clean_text_for_tts(text);
    // Check for remote TTS server setting
    let data_dir = hanni_data_dir();
    let db_path = data_dir.join("hanni.db");
    if let Ok(conn) = rusqlite::Connection::open(&db_path) {
        if let Ok(url) = conn.query_row(
            "SELECT value FROM app_settings WHERE key='tts_server_url'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            if !url.is_empty() {
                speak_remote_tts(&clean, voice, &url);
                return;
            }
        }
    }
    if voice.contains("Neural") {
        speak_edge_tts(&clean, voice);
    } else {
        let _ = std::process::Command::new("say")
            .args(["-v", voice, "-r", "210", &clean])
            .spawn();
    }
}

/// Synchronous TTS — blocks until audio finishes playing
fn speak_tts_sync(text: &str, voice: &str) {
    let clean = clean_text_for_tts(text);
    if clean.is_empty() { return; }
    let data_dir = hanni_data_dir();
    let db_path = data_dir.join("hanni.db");
    if let Ok(conn) = rusqlite::Connection::open(&db_path) {
        if let Ok(url) = conn.query_row(
            "SELECT value FROM app_settings WHERE key='tts_server_url'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            if !url.is_empty() {
                speak_remote_tts_sync(&clean, voice, &url);
                return;
            }
        }
    }
    if voice.contains("Neural") {
        speak_edge_tts_sync(&clean, voice);
    } else {
        let _ = std::process::Command::new("say")
            .args(["-v", voice, "-r", "210", &clean])
            .status(); // .status() blocks
    }
}

fn speak_edge_tts_sync(text: &str, voice: &str) {
    let tmp = format!("/tmp/hanni_tts_call_{}.mp3", std::process::id());
    let candidates = ["edge-tts", "/opt/homebrew/bin/edge-tts", "/usr/local/bin/edge-tts"];
    let mut success = false;
    for cmd in &candidates {
        let status = std::process::Command::new(cmd)
            .args(["--voice", voice, "--rate", "+30%", "--text", text, "--write-media", &tmp])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if let Ok(s) = status {
            if s.success() { success = true; break; }
        }
    }
    if !success {
        if let Some(python) = find_python() {
            let status = std::process::Command::new(&python)
                .args(["-m", "edge_tts", "--voice", voice, "--rate", "+30%", "--text", text, "--write-media", &tmp])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Ok(s) = status { success = s.success(); }
        }
    }
    if success {
        let _ = std::process::Command::new("afplay").arg(&tmp).status();
        let _ = std::fs::remove_file(&tmp);
    }
}

fn speak_remote_tts_sync(text: &str, voice: &str, server_url: &str) {
    let tmp = format!("/tmp/hanni_tts_call_{}.mp3", std::process::id());
    let url = format!("{}/tts", server_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    if let Ok(client) = client {
        let resp = client.post(&url)
            .json(&serde_json::json!({"text": text, "voice": voice}))
            .send();
        if let Ok(resp) = resp {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes() {
                    if std::fs::write(&tmp, &bytes).is_ok() {
                        let _ = std::process::Command::new("afplay").arg(&tmp).status();
                        let _ = std::fs::remove_file(&tmp);
                    }
                }
            }
        }
    }
}

#[tauri::command]
async fn speak_text_blocking(text: String, voice: Option<String>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "ru-RU-SvetlanaNeural".into());
    tokio::task::spawn_blocking(move || {
        speak_tts_sync(&text, &v);
    }).await.map_err(|e| format!("TTS join error: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn speak_text(text: String, voice: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let v = voice.unwrap_or_else(|| "ru-RU-SvetlanaNeural".into());
    let clean = clean_text_for_tts(&text);
    // Check for remote TTS server
    let remote_url = {
        let conn = db.conn();
        conn.query_row("SELECT value FROM app_settings WHERE key='tts_server_url'", [], |row| row.get::<_, String>(0)).ok()
    };
    if let Some(url) = remote_url {
        if !url.is_empty() {
            let url2 = url.clone();
            tokio::task::spawn_blocking(move || speak_remote_tts(&clean, &v, &url2));
            return Ok(());
        }
    }
    if v.contains("Neural") {
        let v2 = v.clone();
        let c2 = clean.clone();
        tokio::task::spawn_blocking(move || speak_edge_tts(&c2, &v2));
    } else {
        std::process::Command::new("say")
            .args(["-v", &v, "-r", "210", &clean])
            .spawn()
            .map_err(|e| format!("TTS error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
async fn stop_speaking() -> Result<(), String> {
    let _ = std::process::Command::new("killall").arg("say").output();
    let _ = std::process::Command::new("killall").arg("afplay").output();
    Ok(())
}

#[tauri::command]
async fn get_tts_voices() -> Result<serde_json::Value, String> {
    let mut voices: Vec<serde_json::Value> = Vec::new();
    // Add macOS voices first (always available)
    // Detect installed macOS voices
    if let Ok(output) = std::process::Command::new("say").arg("-v").arg("?").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.is_empty() { continue; }
            let name = parts[0].trim();
            if line.contains("ru_RU") || line.contains("kk_KZ") {
                let lang = if line.contains("kk_KZ") { "kk-KZ" } else { "ru-RU" };
                voices.push(serde_json::json!({"name": name, "gender": "—", "lang": lang, "engine": "macos"}));
            }
        }
    }
    // Try edge-tts (may not be in PATH for .app bundles)
    let edge_tts_paths = [
        "edge-tts",
        "/opt/homebrew/bin/edge-tts",
        "/usr/local/bin/edge-tts",
    ];
    let mut output_opt = None;
    for path in &edge_tts_paths {
        if let Ok(output) = std::process::Command::new(path)
            .arg("--list-voices")
            .output()
        {
            if output.status.success() {
                output_opt = Some(output);
                break;
            }
        }
    }
    // Also try via python3 -m edge_tts
    if output_opt.is_none() {
        if let Ok(output) = std::process::Command::new("python3")
            .args(&["-m", "edge_tts", "--list-voices"])
            .output()
        {
            if output.status.success() {
                output_opt = Some(output);
            }
        }
    }
    if let Some(output) = output_opt {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0].contains("Neural") {
                let name = parts[0];
                let gender = parts[1];
                let lang = name.split('-').take(2).collect::<Vec<_>>().join("-");
                voices.push(serde_json::json!({
                    "name": name,
                    "gender": gender,
                    "lang": lang,
                    "engine": "edge-tts"
                }));
            }
        }
    }
    Ok(serde_json::json!(voices))
}

// ── Proactive messaging commands ──

#[tauri::command]
async fn get_proactive_settings(state: tauri::State<'_, Arc<Mutex<ProactiveState>>>) -> Result<ProactiveSettings, String> {
    let state = state.lock().await;
    Ok(state.settings.clone())
}

#[tauri::command]
async fn set_proactive_settings(
    settings: ProactiveSettings,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    save_proactive_settings(&settings)?;
    let mut state = state.lock().await;
    state.settings = settings;
    Ok(())
}

#[tauri::command]
async fn set_user_typing(
    typing: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.user_is_typing = typing;
    Ok(())
}

#[tauri::command]
async fn report_proactive_engagement(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    // Mark the last proactive message as replied
    if let Some(pid) = pstate.last_proactive_id {
        {
                let conn = db.conn();
            let delay = pstate.last_message_time
                .map(|t| (chrono::Local::now() - t).num_seconds())
                .unwrap_or(0);
            let _ = conn.execute(
                "UPDATE proactive_history SET user_replied = 1, reply_delay_secs = ?1 WHERE id = ?2",
                rusqlite::params![delay, pid],
            );
        }
    }
    // Recompute engagement rate: rolling avg of last 20 proactive messages
    {
                let conn = db.conn();
        let rate: f64 = conn.query_row(
            "SELECT COALESCE(AVG(CAST(user_replied AS REAL)), 0.5) FROM (SELECT user_replied FROM proactive_history ORDER BY id DESC LIMIT 20)",
            [], |row| row.get(0),
        ).unwrap_or(0.5);
        pstate.engagement_rate = rate;
    }
    Ok(())
}

#[tauri::command]
async fn report_user_chat_activity(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    pstate.last_user_chat_time = Some(chrono::Local::now());
    Ok(())
}

// ── Updater ──

fn updater_with_headers(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    // Public repo — no auth headers needed. Direct download URLs work without them.
    app.updater_builder()
        .build()
        .map_err(|e| format!("Updater error: {}", e))
}

#[tauri::command]
fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<String, String> {
    let updater = updater_with_headers(&app)?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let _ = app.emit("update-available", &version);
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| format!("Install error: {}", e))?;
            app.restart();
        }
        Ok(None) => Ok("Вы на последней версии.".into()),
        Err(e) => Err(format!("Не удалось проверить обновления: {}", e)),
    }
}

// ── App setup ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let proactive_settings = load_proactive_settings();
    let proactive_state = Arc::new(Mutex::new(ProactiveState::new(proactive_settings)));

    // Migrate data from ~/Documents/Hanni/ to ~/Library/Application Support/Hanni/
    // Must run BEFORE init_db to avoid creating an empty DB over old data
    migrate_old_data_dir();

    // Initialize SQLite database
    let db_path = hanni_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = rusqlite::Connection::open(&db_path)
        .expect("Cannot open hanni.db");
    init_db(&conn).expect("Cannot initialize database");
    migrate_memory_json(&conn);
    migrate_events_source(&conn);
    // Load calendar toggle from DB into static flag
    if let Ok(val) = conn.query_row(
        "SELECT value FROM app_settings WHERE key='apple_calendar_enabled'",
        [], |row| row.get::<_, String>(0),
    ) {
        APPLE_CALENDAR_DISABLED.store(val == "false", Ordering::Relaxed);
    }
    let hanni_db = HanniDb(std::sync::Mutex::new(conn));

    // Start MLX server if not already running
    let mlx_child = start_mlx_server();
    let mlx_process = Arc::new(MlxProcess(std::sync::Mutex::new(mlx_child)));
    let mlx_cleanup = mlx_process.clone();

    // Audio recording state (capture starts lazily on first recording)
    let audio_state = Arc::new(AudioRecording(std::sync::Mutex::new(WhisperState {
        recording: false,
        audio_buffer: Vec::new(),
        capture_running: false,
    })));

    // Focus mode state
    let focus_monitor_flag = Arc::new(AtomicBool::new(false));
    let focus_manager = FocusManager(std::sync::Mutex::new(FocusState {
        active: false,
        end_time: None,
        blocked_apps: Vec::new(),
        blocked_sites: Vec::new(),
        monitor_running: focus_monitor_flag.clone(),
    }));

    // Call mode state
    let call_mode = Arc::new(CallMode(std::sync::Mutex::new(CallModeState {
        active: false,
        phase: "idle".into(),
        audio_buffer: Vec::new(),
        speech_frames: 0,
        silence_frames: 0,
        barge_in: false,
        last_recording: Vec::new(),
    })));

    tauri::Builder::default()
        .manage(HttpClient(reqwest::Client::new()))
        .manage(LlmBusy(AtomicBool::new(false)))
        .manage(proactive_state.clone())
        .manage(hanni_db)
        .manage(audio_state)
        .manage(focus_manager)
        .manage(call_mode)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            chat,
            read_file,
            list_dir,
            tracker_add_purchase,
            tracker_add_time,
            tracker_add_goal,
            tracker_add_note,
            tracker_get_stats,
            tracker_get_recent,
            get_integrations,
            get_model_info,
            get_activity_summary,
            get_calendar_events,
            get_now_playing,
            get_browser_tab,
            get_app_version,
            check_update,
            get_proactive_settings,
            set_proactive_settings,
            set_user_typing,
            report_proactive_engagement,
            report_user_chat_activity,
            memory_remember,
            memory_recall,
            memory_forget,
            memory_search,
            save_conversation,
            update_conversation,
            get_conversations,
            get_conversation,
            delete_conversation,
            search_conversations,
            process_conversation_end,
            // Phase 2: TTS
            speak_text,
            stop_speaking,
            get_tts_voices,
            // Phase 1: Voice
            download_whisper_model,
            start_recording,
            stop_recording,
            check_whisper_model,
            // Phase 2: Focus
            start_focus,
            stop_focus,
            get_focus_status,
            update_blocklist,
            // Phase 3: Training
            get_training_stats,
            export_training_data,
            rate_message,
            get_message_ratings,
            // Phase 5: Actions
            run_shell,
            open_url,
            send_notification,
            set_volume,
            get_clipboard,
            set_clipboard,
            web_search,
            // v0.7.0: Activities (Focus)
            start_activity,
            stop_activity,
            get_current_activity,
            get_activity_log,
            // v0.7.0: Notes
            create_note,
            update_note,
            delete_note,
            get_notes,
            get_note,
            // v0.7.0: Events (Calendar)
            create_event,
            get_events,
            delete_event,
            // v0.8.3: Calendar Sync
            sync_apple_calendar,
            sync_google_ics,
            // v0.7.0: Projects & Tasks (Work)
            create_project,
            get_projects,
            create_task,
            get_tasks,
            update_task_status,
            // v0.7.0: Learning Items (Development)
            create_learning_item,
            get_learning_items,
            // v0.7.0: Hobbies
            create_hobby,
            get_hobbies,
            log_hobby_entry,
            get_hobby_entries,
            // v0.7.0: Workouts (Sports)
            create_workout,
            get_workouts,
            get_workout_stats,
            // v0.7.0: Health & Habits
            log_health,
            get_health_today,
            create_habit,
            check_habit,
            get_habits_today,
            // v0.7.0: Dashboard
            get_dashboard_data,
            // v0.7.0: Memory browser
            get_all_memories,
            delete_memory,
            update_memory,
            // v0.8.0: Media Items (Hobbies collections)
            add_media_item,
            update_media_item,
            delete_media_item,
            get_media_items,
            hide_media_item,
            unhide_media_item,
            create_user_list,
            get_user_lists,
            add_to_list,
            remove_from_list,
            get_list_items,
            get_media_stats,
            // v0.8.0: Food
            log_food,
            get_food_log,
            delete_food_entry,
            get_food_stats,
            create_recipe,
            get_recipes,
            delete_recipe,
            add_product,
            get_products,
            update_product,
            delete_product,
            get_expiring_products,
            // v0.8.0: Money
            add_transaction,
            get_transactions,
            delete_transaction,
            get_transaction_stats,
            create_budget,
            get_budgets,
            delete_budget,
            create_savings_goal,
            get_savings_goals,
            update_savings_goal,
            delete_savings_goal,
            add_subscription,
            get_subscriptions,
            update_subscription,
            delete_subscription,
            add_debt,
            get_debts,
            update_debt,
            delete_debt,
            // v0.8.0: Mindset
            save_journal_entry,
            get_journal_entries,
            get_journal_entry,
            log_mood,
            get_mood_history,
            create_principle,
            get_principles,
            update_principle,
            delete_principle,
            get_mindset_check,
            // v0.8.0: Blocklist
            add_to_blocklist,
            remove_from_blocklist,
            get_blocklist,
            toggle_blocklist_item,
            // v0.8.0: Goals & Settings
            create_goal,
            get_goals,
            update_goal,
            delete_goal,
            set_app_setting,
            get_app_setting,
            // v0.8.0: Home Items
            add_home_item,
            get_home_items,
            update_home_item,
            delete_home_item,
            toggle_home_item_needed,
            // v0.8.1: People / Contacts
            add_contact,
            get_contacts,
            update_contact,
            delete_contact,
            toggle_contact_blocked,
            toggle_contact_favorite,
            // v0.8.1: Contact blocks
            add_contact_block,
            get_contact_blocks,
            delete_contact_block,
            toggle_contact_block_active,
            // v0.9.0: Page Meta & Custom Properties
            get_page_meta,
            update_page_meta,
            get_property_definitions,
            create_property_definition,
            update_property_definition,
            delete_property_definition,
            get_property_values,
            set_property_value,
            delete_property_value,
            get_view_configs,
            create_view_config,
            update_view_config,
            // v0.10.0: Call Mode
            start_call_mode,
            stop_call_mode,
            call_mode_resume_listening,
            call_mode_set_speaking,
            call_mode_check_bargein,
            speak_text_blocking,
            save_voice_note,
        ])
        .setup(move |app| {
            // Auto-updater
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let updater = match updater_with_headers(&handle) {
                    Ok(u) => u,
                    Err(_) => return,
                };
                match updater.check().await {
                    Ok(Some(update)) => {
                        let version = update.version.clone();
                        let _ = handle.emit("update-available", &version);
                        if let Ok(()) = update.download_and_install(|_, _| {}, || {}).await {
                            handle.restart();
                        }
                    }
                    _ => {}
                }
            });

            // Save system prompt for nightly training script
            let prompt_path = hanni_data_dir().join("system_prompt.txt");
            let _ = std::fs::write(&prompt_path, SYSTEM_PROMPT);

            // HTTP API server (Phase 4)
            let api_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                spawn_api_server(api_handle).await;
            });

            // Focus mode monitor loop
            let focus_handle = app.handle().clone();
            let focus_flag = focus_monitor_flag.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if !focus_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let focus = focus_handle.state::<FocusManager>();
                    let (active, end_time, apps) = {
                        let state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
                        (state.active, state.end_time, state.blocked_apps.clone())
                    };

                    if !active {
                        continue;
                    }

                    // Check if focus timer expired
                    if let Some(end) = end_time {
                        if chrono::Local::now() >= end {
                            // Auto-stop focus mode
                            let script = "do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges";
                            let _ = run_osascript(script);
                            {
                                let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
                                state.active = false;
                                state.end_time = None;
                                state.blocked_apps.clear();
                                state.blocked_sites.clear();
                                state.monitor_running.store(false, Ordering::Relaxed);
                            }
                            let _ = focus_handle.emit("focus-ended", ());
                            continue;
                        }
                    }

                    // Kill blocked apps if they relaunch
                    for app_name in &apps {
                        let _ = run_osascript(&format!(
                            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
                            app_name, app_name
                        ));
                    }
                }
            });

            // Activity snapshot collector — lightweight OS data every 10 min
            let snapshot_handle = app.handle().clone();
            let snapshot_proactive_ref = proactive_state.clone();
            tauri::async_runtime::spawn(async move {
                // Initial delay
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(600)).await; // 10 min

                    // Collect OS data in blocking thread
                    let (app_name, browser, music) = tokio::task::spawn_blocking(|| {
                        (get_frontmost_app(), get_browser_url(), get_now_playing_sync())
                    }).await.unwrap_or_default();

                    let now = chrono::Local::now();
                    let hour = now.hour() as i64;
                    let weekday = now.weekday().num_days_from_monday() as i64;

                    // Compute productive vs distraction minutes from Screen Time
                    let (prod_min, dist_min) = {
                        let distracting_apps = ["YouTube", "Reddit", "Twitter", "TikTok", "Instagram", "Telegram", "Discord", "VK"];
                        let is_distracting = distracting_apps.iter().any(|d| app_name.contains(d) || browser.contains(d));
                        if is_distracting { (0.0_f64, 10.0_f64) } else { (10.0_f64, 0.0_f64) }
                    };

                    // Write to DB
                    let db = snapshot_handle.state::<HanniDb>();
                    {
                let conn = db.conn();
                        let _ = conn.execute(
                            "INSERT INTO activity_snapshots (captured_at, hour, weekday, frontmost_app, browser_url, music_playing, productive_min, distraction_min) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            rusqlite::params![
                                now.to_rfc3339(),
                                hour,
                                weekday,
                                &app_name,
                                &browser,
                                &music,
                                prod_min,
                                dist_min,
                            ],
                        );

                        // Auto-cleanup: remove snapshots older than 30 days
                        let _ = conn.execute(
                            "DELETE FROM activity_snapshots WHERE captured_at < datetime('now', '-30 days')",
                            [],
                        );
                    }

                    // Check triggers and push to ProactiveState
                    let mut triggers: Vec<String> = Vec::new();

                    // Trigger: distraction >30 min
                    {
                        let db = snapshot_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let dist_total: f64 = conn.query_row(
                            "SELECT COALESCE(SUM(distraction_min), 0) FROM activity_snapshots WHERE captured_at > datetime('now', '-30 minutes')",
                            [], |row| row.get(0),
                        ).unwrap_or(0.0);
                        if dist_total >= 30.0 {
                            triggers.push(format!("Дистракция: пользователь отвлекается уже {:.0} мин", dist_total));
                        }
                    }

                    // Trigger: upcoming event within 15 min
                    if let Ok(upcoming) = get_upcoming_events_soon() {
                        if !upcoming.is_empty() {
                            triggers.push(format!("Скоро событие: {}", upcoming.lines().next().unwrap_or("")));
                        }
                    }

                    if !triggers.is_empty() {
                        let mut state = snapshot_proactive_ref.lock().await;
                        state.pending_triggers = triggers;
                    }
                }
            });

            // Background learning loop — analyze activity patterns every 30 min
            let learning_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                // Initial delay — let DB populate with some snapshots first
                tokio::time::sleep(std::time::Duration::from_secs(1800)).await;

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1800)).await; // 30 min

                    // Skip if LLM is busy
                    let llm_busy = learning_handle.state::<LlmBusy>().0.load(Ordering::Relaxed);
                    if llm_busy { continue; }

                    // Read last 18 snapshots (3 hours) and existing observation facts
                    let (snapshots_text, existing_obs) = {
                        let db = learning_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let mut snap_lines = Vec::new();
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT captured_at, frontmost_app, browser_url, music_playing, productive_min, distraction_min FROM activity_snapshots ORDER BY id DESC LIMIT 18"
                        ) {
                            if let Ok(rows) = stmt.query_map([], |row| {
                                Ok(format!(
                                    "{}: app={}, browser={}, music={}, prod={:.0}min, dist={:.0}min",
                                    row.get::<_, String>(0).unwrap_or_default(),
                                    row.get::<_, String>(1).unwrap_or_default(),
                                    row.get::<_, String>(2).unwrap_or_default(),
                                    row.get::<_, String>(3).unwrap_or_default(),
                                    row.get::<_, f64>(4).unwrap_or(0.0),
                                    row.get::<_, f64>(5).unwrap_or(0.0),
                                ))
                            }) {
                                for row in rows.flatten() { snap_lines.push(row); }
                            }
                        }
                        let mut obs = Vec::new();
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT value FROM facts WHERE source = 'observation' ORDER BY updated_at DESC LIMIT 20"
                        ) {
                            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                                for row in rows.flatten() { obs.push(row); }
                            }
                        }
                        (snap_lines.join("\n"), obs.join("\n"))
                    };

                    if snapshots_text.is_empty() { continue; }

                    let prompt = format!(
                        "Проанализируй активность пользователя за последние 3 часа и найди паттерны.\n\n\
                        Снимки активности (от новых к старым):\n{}\n\n\
                        Уже известные наблюдения (не дублируй):\n{}\n\n\
                        Найди: рутины, привычки, продуктивность, предпочтения, аномалии.\n\
                        Формат: одно наблюдение на строку, начиная с '- '. Только новые, не дублируй известные.\n\
                        Если ничего нового — ответь [NONE].\n/no_think",
                        snapshots_text, if existing_obs.is_empty() { "нет" } else { &existing_obs }
                    );

                    let request = ChatRequest {
                        model: MODEL.into(),
                        messages: vec![
                            ChatMessage {
                                role: "system".into(),
                                content: "Ты — аналитик поведения. Твоя задача — находить паттерны в активности пользователя. Будь краток и конкретен. Отвечай на русском.".into(),
                            },
                            ChatMessage {
                                role: "user".into(),
                                content: prompt,
                            },
                        ],
                        max_tokens: 400,
                        stream: false,
                        temperature: 0.3,
                    };

                    let resp = client.post(MLX_URL).json(&request).send().await;
                    if let Ok(resp) = resp {
                        if let Ok(parsed) = resp.json::<NonStreamResponse>().await {
                            let raw = parsed.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();
                            // Strip think tags
                            let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
                            let text = re.replace_all(&raw, "").trim().to_string();

                            if !text.contains("[NONE]") && !text.is_empty() {
                                // Parse lines starting with '- '
                                let observations: Vec<&str> = text.lines()
                                    .filter(|l| l.trim().starts_with("- "))
                                    .map(|l| l.trim().trim_start_matches("- "))
                                    .collect();

                                if !observations.is_empty() {
                                    let db = learning_handle.state::<HanniDb>();
                                    let conn = db.conn();
                                    let now = chrono::Local::now().to_rfc3339();
                                    for obs in observations.iter().take(5) {
                                        let key = format!("obs_{}", &now[..16]);
                                        let _ = conn.execute(
                                            "INSERT INTO facts (category, key, value, source, created_at, updated_at) VALUES ('observation', ?1, ?2, 'observation', ?3, ?3)",
                                            rusqlite::params![key, obs, now],
                                        );
                                    }
                                    eprintln!("[learning] saved {} observations", observations.len().min(5));
                                }
                            }
                        }
                    }
                }
            });

            // Proactive messaging background loop
            let proactive_handle = app.handle().clone();
            let proactive_state_ref = proactive_state.clone();
            tauri::async_runtime::spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                // Initial delay — let the app fully start
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                // Load recent proactive messages from DB (survives app restart)
                let loaded_msgs: Vec<(String, chrono::DateTime<chrono::Local>)> = {
                    let db = proactive_handle.state::<HanniDb>();
                    let conn = db.conn();
                    let mut result = Vec::new();
                    if let Ok(mut stmt) = conn.prepare(
                        "SELECT message, sent_at FROM proactive_history ORDER BY id DESC LIMIT 15"
                    ) {
                        if let Ok(rows) = stmt.query_map([], |row| {
                            let msg: String = row.get(0)?;
                            let ts_str: String = row.get(1)?;
                            Ok((msg, ts_str))
                        }) {
                            for row in rows.flatten() {
                                let ts = chrono::DateTime::parse_from_rfc3339(&row.1)
                                    .map(|dt| dt.with_timezone(&chrono::Local))
                                    .unwrap_or_else(|_| chrono::Local::now());
                                result.push((row.0, ts));
                            }
                        }
                    }
                    result.reverse(); // oldest first
                    result
                };
                if !loaded_msgs.is_empty() {
                    let mut state = proactive_state_ref.lock().await;
                    state.recent_messages = loaded_msgs;
                }

                let mut last_check = std::time::Instant::now();
                let mut first_run = true;

                loop {
                    // Poll every 5 seconds so we react quickly to settings changes
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    let (enabled, interval, quiet_start_min, quiet_end_min, is_typing, skips, voice_enabled, voice_name, recent_msgs, last_ctx, engagement, triggers, last_user_chat, enabled_styles) = {
                        let state = proactive_state_ref.lock().await;
                        (
                            state.settings.enabled,
                            state.settings.interval_minutes,
                            state.settings.quiet_start_minutes(),
                            state.settings.quiet_end_minutes(),
                            state.user_is_typing,
                            state.consecutive_skips,
                            state.settings.voice_enabled,
                            state.settings.voice_name.clone(),
                            state.recent_messages.clone(),
                            state.last_context_snapshot.clone(),
                            state.engagement_rate,
                            state.pending_triggers.clone(),
                            state.last_user_chat_time,
                            state.settings.enabled_styles.clone(),
                        )
                    };

                    if !enabled {
                        first_run = true;
                        continue;
                    }

                    // ── Smart Adaptive Timing (Step 8) ──
                    let base_interval_secs = interval * 60;
                    let elapsed = last_check.elapsed().as_secs();
                    let time_ratio = elapsed as f64 / base_interval_secs as f64;

                    if !first_run {
                        // Compute firing score
                        let mut score: f64 = 0.0;

                        // time_ratio: 0→0, 1→0.3, 2→0.6
                        score += (time_ratio * 0.3).min(0.6);

                        // upcoming event trigger
                        if triggers.iter().any(|t| t.contains("событие")) {
                            score += 0.3;
                        }

                        // distraction trigger
                        if triggers.iter().any(|t| t.contains("Дистракция")) {
                            score += 0.25;
                        }

                        // pending trigger (generic)
                        if !triggers.is_empty() {
                            score += 0.4_f64.min(score + 0.4) - score; // ensure at least +0.15
                            score += 0.15;
                        }

                        // context change (will be computed after gather, but approximate from triggers)
                        // idle: no user chat >30 min
                        if let Some(last_chat) = last_user_chat {
                            let idle_min = (chrono::Local::now() - last_chat).num_minutes();
                            if idle_min > 30 { score += 0.1; }
                        } else {
                            score += 0.1; // no chat at all — consider idle
                        }

                        // high engagement bonus
                        if engagement > 0.6 { score += 0.1; }
                        if engagement > 0.8 { score += 0.05; }

                        // deep work hours penalty (10-12, 14-17)
                        let hour = chrono::Local::now().hour();
                        if (10..=12).contains(&hour) || (14..=17).contains(&hour) {
                            score -= 0.1;
                        }

                        // many skips penalty
                        if skips > 3 { score -= 0.15; }

                        // Minimum floor: 3 minutes
                        if elapsed < 180 {
                            score = 0.0;
                        }

                        if score < 0.5 {
                            continue;
                        }
                    }

                    let now_t = chrono::Local::now();
                    let now_min = now_t.hour() * 60 + now_t.minute();
                    let in_quiet = if quiet_start_min > quiet_end_min {
                        // Wraps midnight: e.g. 23:30 → 08:00
                        now_min >= quiet_start_min || now_min < quiet_end_min
                    } else {
                        now_min >= quiet_start_min && now_min < quiet_end_min
                    };

                    let llm_busy = proactive_handle.state::<LlmBusy>().0.load(Ordering::Relaxed);

                    if in_quiet || is_typing || llm_busy {
                        continue;
                    }

                    last_check = std::time::Instant::now();
                    first_run = false;

                    let context = gather_context().await;

                    // Compute delta from last context
                    let delta = if !last_ctx.is_empty() {
                        compute_activity_delta(&last_ctx, &context)
                    } else {
                        String::new()
                    };

                    // Build memory context (15 facts — less noise, faster)
                    let (mem_ctx, chat_snippet, user_name, todays_msgs) = {
                        let db = proactive_handle.state::<HanniDb>();
                        let conn = db.conn();
                        (
                            build_memory_context_from_db(&conn, "", 15),
                            get_recent_chat_snippet(&conn, 6),
                            get_user_name_from_memory(&conn),
                            get_todays_proactive_messages(&conn),
                        )
                    };
                    // Mark LLM as busy during proactive call to prevent concurrent MLX requests
                    proactive_handle.state::<LlmBusy>().0.store(true, Ordering::Relaxed);
                    let proactive_result = proactive_llm_call(&client, &context, &recent_msgs, skips, &mem_ctx, &delta, &triggers, &chat_snippet, engagement, &user_name, &todays_msgs, &enabled_styles).await;
                    proactive_handle.state::<LlmBusy>().0.store(false, Ordering::Relaxed);

                    match proactive_result {
                        Ok(Some(message)) => {
                            let _ = proactive_handle.emit("proactive-message", &message);
                            if voice_enabled {
                                speak_tts(&message, &voice_name);
                            }
                            // Record in proactive_history
                            let proactive_id = {
                                let db = proactive_handle.state::<HanniDb>();
                                let conn = db.conn();
                                let _ = conn.execute(
                                    "INSERT INTO proactive_history (sent_at, message) VALUES (?1, ?2)",
                                    rusqlite::params![chrono::Local::now().to_rfc3339(), &message],
                                );
                                Some(conn.last_insert_rowid())
                            };
                            let mut state = proactive_state_ref.lock().await;
                            state.last_message_time = Some(chrono::Local::now());
                            state.last_message_text = message.clone();
                            state.consecutive_skips = 0;
                            state.last_context_snapshot = context;
                            state.last_proactive_id = proactive_id;
                            // Update recent_messages (keep last 15)
                            state.recent_messages.push((message, chrono::Local::now()));
                            if state.recent_messages.len() > 15 {
                                state.recent_messages.remove(0);
                            }
                            state.pending_triggers.clear();
                        }
                        Ok(None) => {
                            let mut state = proactive_state_ref.lock().await;
                            state.consecutive_skips += 1;
                            state.last_context_snapshot = context;
                        }
                        Err(_) => {
                            // LLM server not running — back off
                            last_check = std::time::Instant::now();
                        }
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Hanni")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Kill MLX server process on app exit
                {
                    let mut child = mlx_cleanup.0.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(ref mut proc) = *child {
                        let _ = proc.kill();
                    }
                }
            }
        });
}
