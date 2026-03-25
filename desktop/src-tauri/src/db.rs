// db.rs — Database initialization, migrations, auto-backup
use serde::Deserialize;
use std::collections::HashMap;
use crate::types::hanni_data_dir;
use chrono;

/// Migrate data from old ~/Documents/Hanni/ to ~/Library/Application Support/Hanni/
pub fn migrate_old_data_dir() {
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

pub fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
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

/// Create a timestamped backup of hanni.db, keep last 5
pub fn backup_db() {
    let data_dir = hanni_data_dir();
    let db_path = data_dir.join("hanni.db");
    if !db_path.exists() { return; }
    let backup_dir = data_dir.join("backups");
    let _ = std::fs::create_dir_all(&backup_dir);
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let dest = backup_dir.join(format!("hanni_{}.db", ts));
    if let Err(e) = std::fs::copy(&db_path, &dest) {
        eprintln!("Backup failed: {}", e);
        return;
    }
    // Also copy WAL if present
    let wal = data_dir.join("hanni.db-wal");
    if wal.exists() {
        let _ = std::fs::copy(&wal, backup_dir.join(format!("hanni_{}.db-wal", ts)));
    }
    // Keep only last 5 backups
    let mut backups: Vec<_> = std::fs::read_dir(&backup_dir)
        .into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("hanni_") && e.file_name().to_string_lossy().ends_with(".db"))
        .collect();
    backups.sort_by_key(|e| e.file_name());
    while backups.len() > 5 {
        let old = backups.remove(0);
        let _ = std::fs::remove_file(old.path());
        // Remove matching WAL
        let wal_path = old.path().with_extension("db-wal");
        let _ = std::fs::remove_file(wal_path);
    }
    eprintln!("DB backup: {}", dest.display());
}

pub fn init_db(conn: &rusqlite::Connection) -> Result<(), String> {
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

        -- v0.17.0: Vector embeddings for semantic memory search (sqlite-vec)
        CREATE VIRTUAL TABLE IF NOT EXISTS vec_facts USING vec0(
            fact_id integer primary key,
            embedding float[384]
        );

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

        -- v0.27.6: UI state (replaces localStorage for persistence across updates)
        CREATE TABLE IF NOT EXISTS ui_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
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
        );

        -- v0.18.0: Conversation insights (decisions, open questions, action items)
        CREATE TABLE IF NOT EXISTS conversation_insights (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            insight_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        -- v0.18.0: Reminders & timers
        CREATE TABLE IF NOT EXISTS reminders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            remind_at TEXT NOT NULL,
            repeat TEXT,
            fired INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );

        -- v0.18.0: Indexes for query performance
        CREATE INDEX IF NOT EXISTS idx_events_date ON events(date);
        CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
        CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category);
        CREATE INDEX IF NOT EXISTS idx_food_log_date ON food_log(date);
        CREATE INDEX IF NOT EXISTS idx_health_log_date ON health_log(date);
        CREATE INDEX IF NOT EXISTS idx_media_items_type_status ON media_items(media_type, status);
        CREATE INDEX IF NOT EXISTS idx_tasks_project_status ON tasks(project_id, status);
        CREATE INDEX IF NOT EXISTS idx_proactive_history_sent ON proactive_history(sent_at);
        CREATE INDEX IF NOT EXISTS idx_facts_category ON facts(category);
        CREATE INDEX IF NOT EXISTS idx_conversations_started ON conversations(started_at);
        CREATE INDEX IF NOT EXISTS idx_journal_date ON journal_entries(date);
        CREATE INDEX IF NOT EXISTS idx_mood_date ON mood_log(date);
        CREATE INDEX IF NOT EXISTS idx_activities_started ON activities(started_at);
        CREATE INDEX IF NOT EXISTS idx_habit_checks_date ON habit_checks(date);
        CREATE INDEX IF NOT EXISTS idx_conversation_insights_conv ON conversation_insights(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_message_feedback_conv ON message_feedback(conversation_id);

        -- v0.18.0 Wave 3: Flywheel cycles
        CREATE TABLE IF NOT EXISTS flywheel_cycles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            status TEXT NOT NULL DEFAULT 'running',
            train_pairs INTEGER DEFAULT 0,
            eval_score REAL,
            notes TEXT,
            adapter_path TEXT
        );

        -- v0.24.0: Custom Pages
        CREATE TABLE IF NOT EXISTS custom_pages (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT 'Новая страница',
            icon TEXT DEFAULT '📄',
            description TEXT DEFAULT '',
            content TEXT DEFAULT '',
            sub_tabs TEXT DEFAULT '[]',
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- v0.26.0: Tab page blocks (block editor per tab/sub-tab)
        CREATE TABLE IF NOT EXISTS tab_page_blocks (
            tab_id TEXT NOT NULL,
            sub_tab TEXT NOT NULL DEFAULT '',
            blocks_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL,
            PRIMARY KEY (tab_id, sub_tab)
        );"
    ).map_err(|e| format!("DB init error: {}", e))
}

pub fn migrate_memory_json(conn: &rusqlite::Connection) {
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

pub fn migrate_events_source(conn: &rusqlite::Connection) {
    // Add source column to events table (manual, apple, google)
    let has_source = conn.prepare("SELECT source FROM events LIMIT 1").is_ok();
    if !has_source {
        let _ = conn.execute("ALTER TABLE events ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'", []);
        let _ = conn.execute("ALTER TABLE events ADD COLUMN external_id TEXT", []);
    }
}

pub fn migrate_conversations_category(conn: &rusqlite::Connection) {
    // CH8: Add category column for auto-categorization
    let has_category = conn.prepare("SELECT category FROM conversations LIMIT 1").is_ok();
    if !has_category {
        let _ = conn.execute("ALTER TABLE conversations ADD COLUMN category TEXT", []);
    }
}

pub fn migrate_proactive_history_v2(conn: &rusqlite::Connection) {
    // v0.22: Add rating and style columns to proactive_history
    let has_rating = conn.prepare("SELECT rating FROM proactive_history LIMIT 1").is_ok();
    if !has_rating {
        let _ = conn.execute("ALTER TABLE proactive_history ADD COLUMN rating INTEGER DEFAULT 0", []);
    }
    let has_style = conn.prepare("SELECT style FROM proactive_history LIMIT 1").is_ok();
    if !has_style {
        let _ = conn.execute("ALTER TABLE proactive_history ADD COLUMN style TEXT DEFAULT ''", []);
    }
}

pub fn migrate_facts_decay(conn: &rusqlite::Connection) {
    // ME1: Add access tracking columns for memory decay
    let has_access_count = conn.prepare("SELECT access_count FROM facts LIMIT 1").is_ok();
    if !has_access_count {
        let _ = conn.execute("ALTER TABLE facts ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE facts ADD COLUMN last_accessed TEXT", []);
    }
}

pub fn migrate_notes_v2(conn: &rusqlite::Connection) {
    // Notes enhancement: tab linking, tasks, reminders, DnD sort, colors
    conn.execute("ALTER TABLE notes ADD COLUMN tab_name TEXT", []).ok();
    conn.execute("ALTER TABLE notes ADD COLUMN status TEXT NOT NULL DEFAULT 'note'", []).ok();
    conn.execute("ALTER TABLE notes ADD COLUMN due_date TEXT", []).ok();
    conn.execute("ALTER TABLE notes ADD COLUMN reminder_at TEXT", []).ok();
    conn.execute("ALTER TABLE notes ADD COLUMN sort_order INTEGER DEFAULT 0", []).ok();
    conn.execute("ALTER TABLE notes ADD COLUMN color TEXT", []).ok();

    // Tag colors table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS note_tags (
            name TEXT PRIMARY KEY,
            color TEXT NOT NULL DEFAULT 'blue'
        );"
    ).ok();
}

pub fn migrate_content_blocks(conn: &rusqlite::Connection) {
    // Editor.js block editor: JSON storage for structured content
    conn.execute("ALTER TABLE notes ADD COLUMN content_blocks TEXT", []).ok();
    conn.execute("ALTER TABLE custom_pages ADD COLUMN content_blocks TEXT", []).ok();
}

pub fn migrate_schedules(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schedules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'other',
            frequency TEXT NOT NULL DEFAULT 'daily',
            frequency_days TEXT,
            time_of_day TEXT,
            details TEXT DEFAULT '',
            is_active INTEGER DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS schedule_completions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            schedule_id INTEGER NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
            date TEXT NOT NULL,
            completed INTEGER DEFAULT 0,
            completed_at TEXT,
            UNIQUE(schedule_id, date)
        );
        CREATE TABLE IF NOT EXISTS dan_koe_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL UNIQUE,
            contemplation INTEGER DEFAULT 0,
            pattern_interrupt INTEGER DEFAULT 0,
            vision INTEGER DEFAULT 0,
            integration INTEGER DEFAULT 0,
            notes TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS proactive_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            read INTEGER DEFAULT 0,
            archived INTEGER DEFAULT 0
        );"
    ).ok();
}

pub fn migrate_activity_tracking(conn: &rusqlite::Connection) {
    // v0.27: Enhanced activity tracking — idle, window title, category
    conn.execute("ALTER TABLE activity_snapshots ADD COLUMN idle_secs REAL DEFAULT 0", []).ok();
    conn.execute("ALTER TABLE activity_snapshots ADD COLUMN window_title TEXT DEFAULT ''", []).ok();
    conn.execute("ALTER TABLE activity_snapshots ADD COLUMN category TEXT DEFAULT 'other'", []).ok();
    // v0.28: Screen lock detection for AFK ground truth
    conn.execute("ALTER TABLE activity_snapshots ADD COLUMN screen_locked INTEGER DEFAULT 0", []).ok();
    // Index for daily queries
    conn.execute("CREATE INDEX IF NOT EXISTS idx_snapshots_captured ON activity_snapshots(captured_at)", []).ok();
}
