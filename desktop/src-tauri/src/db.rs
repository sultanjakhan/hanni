// db.rs — Database initialization, migrations, auto-backup
use serde::Deserialize;
use std::collections::HashMap;
use crate::types::hanni_data_dir;
use chrono;

/// Migrate data from old ~/Documents/Hanni/ to ~/Library/Application Support/Hanni/
#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
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

        CREATE TABLE IF NOT EXISTS recipe_ingredients (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recipe_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            amount REAL NOT NULL DEFAULT 0,
            unit TEXT NOT NULL DEFAULT 'г',
            FOREIGN KEY (recipe_id) REFERENCES recipes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS meal_plan (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            meal_type TEXT NOT NULL DEFAULT 'lunch',
            recipe_id INTEGER NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            FOREIGN KEY (recipe_id) REFERENCES recipes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS ingredient_catalog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            category TEXT NOT NULL DEFAULT 'other'
        );

        CREATE TABLE IF NOT EXISTS custom_cuisines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            emoji TEXT NOT NULL DEFAULT '🌍',
            is_default INTEGER NOT NULL DEFAULT 0
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

/// Seed ingredient catalog with common ingredients
pub fn seed_ingredient_catalog(conn: &rusqlite::Connection) {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM ingredient_catalog", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 { return; }
    let items: Vec<(&str, &str)> = vec![
        // meat
        ("курица", "meat"), ("куриное филе", "meat"), ("куриные бёдра", "meat"),
        ("куриные крылышки", "meat"), ("говядина", "meat"), ("телятина", "meat"),
        ("свинина", "meat"), ("баранина", "meat"), ("конина", "meat"),
        ("фарш говяжий", "meat"), ("фарш куриный", "meat"), ("фарш свиной", "meat"),
        ("индейка", "meat"), ("утка", "meat"), ("кролик", "meat"),
        ("бекон", "meat"), ("ветчина", "meat"), ("колбаса варёная", "meat"),
        ("колбаса копчёная", "meat"), ("сосиски", "meat"), ("тушёнка", "meat"),
        ("печень куриная", "meat"), ("печень говяжья", "meat"), ("язык говяжий", "meat"),
        // fish
        ("лосось", "fish"), ("сёмга", "fish"), ("форель", "fish"),
        ("треска", "fish"), ("минтай", "fish"), ("скумбрия", "fish"),
        ("тунец", "fish"), ("сельдь", "fish"), ("карп", "fish"),
        ("креветки", "fish"), ("кальмар", "fish"), ("мидии", "fish"),
        ("крабовые палочки", "fish"), ("икра красная", "fish"), ("шпроты", "fish"),
        // veg
        ("лук", "veg"), ("лук красный", "veg"), ("лук-порей", "veg"),
        ("морковь", "veg"), ("картофель", "veg"), ("помидор", "veg"),
        ("огурец", "veg"), ("перец болгарский", "veg"), ("перец чили", "veg"),
        ("капуста белокочанная", "veg"), ("капуста пекинская", "veg"), ("капуста цветная", "veg"),
        ("брокколи", "veg"), ("чеснок", "veg"), ("кабачок", "veg"),
        ("баклажан", "veg"), ("тыква", "veg"), ("свёкла", "veg"),
        ("редис", "veg"), ("редька", "veg"), ("шпинат", "veg"),
        ("салат айсберг", "veg"), ("руккола", "veg"), ("сельдерей", "veg"),
        ("кукуруза", "veg"), ("горошек зелёный", "veg"), ("стручковая фасоль", "veg"),
        ("имбирь", "veg"), ("грибы шампиньоны", "veg"), ("грибы вёшенки", "veg"),
        // fruit
        ("банан", "fruit"), ("яблоко", "fruit"), ("груша", "fruit"),
        ("апельсин", "fruit"), ("лимон", "fruit"), ("лайм", "fruit"),
        ("мандарин", "fruit"), ("грейпфрут", "fruit"), ("авокадо", "fruit"),
        ("манго", "fruit"), ("ананас", "fruit"), ("киви", "fruit"),
        ("виноград", "fruit"), ("клубника", "fruit"), ("малина", "fruit"),
        ("черника", "fruit"), ("вишня", "fruit"), ("арбуз", "fruit"),
        ("дыня", "fruit"), ("хурма", "fruit"), ("гранат", "fruit"),
        ("персик", "fruit"), ("слива", "fruit"), ("изюм", "fruit"),
        ("курага", "fruit"), ("чернослив", "fruit"), ("финики", "fruit"),
        // grain
        ("рис", "grain"), ("рис басмати", "grain"), ("гречка", "grain"),
        ("овсяные хлопья", "grain"), ("пшено", "grain"), ("булгур", "grain"),
        ("кус-кус", "grain"), ("перловка", "grain"), ("манка", "grain"),
        ("кукурузная крупа", "grain"), ("киноа", "grain"),
        ("макароны", "grain"), ("спагетти", "grain"), ("лапша", "grain"),
        ("лапша рисовая", "grain"), ("фунчоза", "grain"),
        ("мука пшеничная", "grain"), ("мука кукурузная", "grain"),
        ("хлеб белый", "grain"), ("хлеб чёрный", "grain"), ("лаваш", "grain"),
        ("батон", "grain"), ("панировочные сухари", "grain"),
        // dairy
        ("молоко", "dairy"), ("кефир", "dairy"), ("ряженка", "dairy"),
        ("йогурт", "dairy"), ("сметана", "dairy"), ("сливки", "dairy"),
        ("масло сливочное", "dairy"), ("творог", "dairy"), ("творожный сыр", "dairy"),
        ("сыр твёрдый", "dairy"), ("пармезан", "dairy"), ("моцарелла", "dairy"),
        ("фета", "dairy"), ("брынза", "dairy"), ("плавленый сыр", "dairy"),
        ("яйца куриные", "dairy"), ("яйца перепелиные", "dairy"),
        ("сгущённое молоко", "dairy"), ("кокосовое молоко", "dairy"),
        // legumes
        ("фасоль", "legumes"), ("фасоль красная", "legumes"), ("фасоль белая", "legumes"),
        ("чечевица", "legumes"), ("чечевица красная", "legumes"),
        ("горох", "legumes"), ("нут", "legumes"), ("маш", "legumes"),
        ("соя", "legumes"), ("тофу", "legumes"),
        // nuts
        ("грецкий орех", "nuts"), ("миндаль", "nuts"), ("фундук", "nuts"),
        ("кешью", "nuts"), ("арахис", "nuts"), ("фисташки", "nuts"),
        ("кедровые орехи", "nuts"), ("семена подсолнечника", "nuts"),
        ("семена тыквы", "nuts"), ("семена кунжута", "nuts"),
        ("семена льна", "nuts"), ("семена чиа", "nuts"), ("кокосовая стружка", "nuts"),
        // spice
        ("соль", "spice"), ("перец чёрный", "spice"), ("перец красный", "spice"),
        ("паприка", "spice"), ("куркума", "spice"), ("зира", "spice"),
        ("кориандр", "spice"), ("корица", "spice"), ("мускатный орех", "spice"),
        ("гвоздика", "spice"), ("лавровый лист", "spice"), ("орегано", "spice"),
        ("базилик", "spice"), ("тимьян", "spice"), ("розмарин", "spice"),
        ("укроп", "spice"), ("петрушка", "spice"), ("кинза", "spice"),
        ("мята", "spice"), ("зелёный лук", "spice"),
        ("сахар", "spice"), ("мёд", "spice"), ("ваниль", "spice"),
        ("уксус", "spice"), ("соевый соус", "spice"), ("томатная паста", "spice"),
        ("горчица", "spice"), ("майонез", "spice"), ("кетчуп", "spice"),
        ("сметанный соус", "spice"), ("аджика", "spice"),
        // oil
        ("масло растительное", "oil"), ("масло оливковое", "oil"),
        ("масло подсолнечное", "oil"), ("масло кунжутное", "oil"),
        ("масло кокосовое", "oil"), ("масло льняное", "oil"),
        // bakery
        ("дрожжи", "bakery"), ("разрыхлитель", "bakery"), ("какао", "bakery"),
        ("шоколад тёмный", "bakery"), ("шоколад молочный", "bakery"),
        ("крахмал", "bakery"), ("желатин", "bakery"), ("сахарная пудра", "bakery"),
        // drinks
        ("чай чёрный", "drinks"), ("чай зелёный", "drinks"), ("кофе", "drinks"),
        ("какао-порошок", "drinks"), ("сок апельсиновый", "drinks"),
        ("вода минеральная", "drinks"), ("компот", "drinks"),
    ];
    for (name, cat) in items {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO ingredient_catalog (name, category) VALUES (?1, ?2)",
            rusqlite::params![name, cat],
        );
    }
}

/// Seed default cuisines
pub fn seed_default_cuisines(conn: &rusqlite::Connection) {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM custom_cuisines", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 { return; }
    let cuisines: Vec<(&str, &str, &str)> = vec![
        ("kz", "Казахская", "🇰🇿"), ("ru", "Русская", "🇷🇺"),
        ("it", "Итальянская", "🇮🇹"), ("jp", "Японская", "🇯🇵"),
        ("ge", "Грузинская", "🇬🇪"), ("tr", "Турецкая", "🇹🇷"),
        ("uz", "Узбекская", "🇺🇿"), ("kr", "Корейская", "🇰🇷"),
        ("us", "Американская", "🇺🇸"), ("mx", "Мексиканская", "🇲🇽"),
        ("other", "Другая", "🌍"),
    ];
    for (code, name, emoji) in cuisines {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO custom_cuisines (code, name, emoji, is_default) VALUES (?1, ?2, ?3, 1)",
            rusqlite::params![code, name, emoji],
        );
    }
}

fn parse_amount_unit(s: &str) -> (f64, &str) {
    let s = s.trim();
    let num_end = s.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(s.len());
    let amount = s[..num_end].parse::<f64>().unwrap_or(0.0);
    let unit = s[num_end..].trim();
    if unit.is_empty() { (amount, "шт") } else { (amount, unit) }
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

pub fn migrate_proactive_messages_rating(conn: &rusqlite::Connection) {
    let has_rating = conn.prepare("SELECT rating FROM proactive_messages LIMIT 1").is_ok();
    if !has_rating {
        let _ = conn.execute("ALTER TABLE proactive_messages ADD COLUMN rating INTEGER DEFAULT 0", []);
    }
}

pub fn migrate_recipe_difficulty(conn: &rusqlite::Connection) {
    let has_difficulty = conn.prepare("SELECT difficulty FROM recipes LIMIT 1").is_ok();
    if !has_difficulty {
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN difficulty TEXT NOT NULL DEFAULT 'easy'", []);
    }
}

pub fn migrate_recipe_extra(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT cuisine FROM recipes LIMIT 1").is_err() {
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN cuisine TEXT NOT NULL DEFAULT 'kz'", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN health_score INTEGER NOT NULL DEFAULT 5", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN price_score INTEGER NOT NULL DEFAULT 5", []);
    }
}

pub fn migrate_recipe_extra2(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT protein FROM recipes LIMIT 1").is_err() {
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN protein INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN fat INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN carbs INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN last_cooked TEXT", []);
    }
}

/// One-time migration: clear seed recipes (v0.36)
pub fn migrate_clear_seed_recipes(conn: &rusqlite::Connection) {
    let has_flag = conn.prepare("SELECT 1 FROM _migrations WHERE name='clear_seed_recipes'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if has_flag { return; }
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let _ = conn.execute("DELETE FROM recipe_ingredients", []);
    let _ = conn.execute("DELETE FROM recipes", []);
    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('clear_seed_recipes')", []);
}

pub fn migrate_reseed_ingredient_catalog(conn: &rusqlite::Connection) {
    let has_flag = conn.prepare("SELECT 1 FROM _migrations WHERE name='reseed_catalog_v2'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if has_flag { return; }
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    // Clear old catalog and re-seed with expanded version
    let _ = conn.execute("DELETE FROM ingredient_catalog", []);
    seed_ingredient_catalog(conn);
    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('reseed_catalog_v2')", []);
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
        ALTER TABLE schedules ADD COLUMN marks_previous_day INTEGER DEFAULT 0;
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

pub fn migrate_custom_projects(conn: &rusqlite::Connection) {
    // page_type: 'page' (default) or 'project' (unified layout with table)
    conn.execute("ALTER TABLE custom_pages ADD COLUMN page_type TEXT DEFAULT 'page'", []).ok();
    // Generic records table for custom projects
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_project_records_project ON project_records(project_id);"
    ).ok();
}

pub fn migrate_body_records(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS body_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            zone TEXT NOT NULL,
            zone_label TEXT NOT NULL DEFAULT '',
            record_type TEXT NOT NULL,
            intensity INTEGER,
            pain_type TEXT,
            goal_type TEXT,
            value REAL,
            unit TEXT,
            treatment_type TEXT,
            note TEXT NOT NULL DEFAULT '',
            date TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_body_records_zone ON body_records(zone);
        CREATE INDEX IF NOT EXISTS idx_body_records_date ON body_records(date);"
    ).ok();
}

pub fn migrate_job_search(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS job_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            type TEXT NOT NULL DEFAULT 'other',
            url TEXT NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS job_roles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            keywords TEXT NOT NULL DEFAULT '',
            salary_min INTEGER,
            priority TEXT NOT NULL DEFAULT 'medium',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS job_vacancies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            company TEXT NOT NULL DEFAULT '',
            position TEXT NOT NULL DEFAULT '',
            source_id INTEGER,
            role_id INTEGER,
            salary TEXT NOT NULL DEFAULT '',
            url TEXT NOT NULL DEFAULT '',
            stage TEXT NOT NULL DEFAULT 'found',
            notes TEXT NOT NULL DEFAULT '',
            found_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (source_id) REFERENCES job_sources(id),
            FOREIGN KEY (role_id) REFERENCES job_roles(id)
        );
        CREATE INDEX IF NOT EXISTS idx_job_vacancies_stage ON job_vacancies(stage);
        CREATE INDEX IF NOT EXISTS idx_job_vacancies_source ON job_vacancies(source_id);
        CREATE TABLE IF NOT EXISTS job_search_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER,
            searched_at TEXT NOT NULL DEFAULT (datetime('now')),
            found_count INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            FOREIGN KEY (source_id) REFERENCES job_sources(id)
        );
        DROP TABLE IF EXISTS projects;
        DROP TABLE IF EXISTS tasks;"
    ).ok();
    // New columns for simplified vacancy table
    conn.execute("ALTER TABLE job_vacancies ADD COLUMN contact TEXT NOT NULL DEFAULT ''", []).ok();
    conn.execute("ALTER TABLE job_vacancies ADD COLUMN applied_at TEXT", []).ok();
    conn.execute("ALTER TABLE job_vacancies ADD COLUMN source TEXT NOT NULL DEFAULT ''", []).ok();
    conn.execute("ALTER TABLE job_vacancies ADD COLUMN deleted_at TEXT", []).ok();
}

pub fn migrate_dashboard_widgets(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS dashboard_widgets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tab_id TEXT NOT NULL,
            widget_type TEXT NOT NULL,
            position INTEGER NOT NULL,
            config TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_dw_tab ON dashboard_widgets(tab_id);"
    ).ok();
}

pub fn migrate_timeline(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS timeline_activity_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            color TEXT NOT NULL DEFAULT '#2383e2',
            icon TEXT NOT NULL DEFAULT '',
            is_system INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS timeline_blocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type_id INTEGER NOT NULL REFERENCES timeline_activity_types(id),
            date TEXT NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_tl_blocks_date ON timeline_blocks(date);
        CREATE INDEX IF NOT EXISTS idx_tl_blocks_type ON timeline_blocks(type_id);
        CREATE TABLE IF NOT EXISTS timeline_goals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type_id INTEGER NOT NULL REFERENCES timeline_activity_types(id),
            operator TEXT NOT NULL DEFAULT '<=',
            target_minutes INTEGER NOT NULL,
            period TEXT NOT NULL DEFAULT 'daily',
            active INTEGER DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );"
    ).ok();
    // Seed default system types (idempotent — skips if already exist)
    let defaults = [
        ("Сон", "#6366f1", "🌙", 1),
        ("Фокус", "#22c55e", "💻", 2),
        ("АФК", "#9ca3af", "💤", 3),
        ("Еда", "#f97316", "🍽️", 4),
        ("Спорт", "#ef4444", "🏋️", 5),
    ];
    for (name, color, icon, order) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO timeline_activity_types (name, color, icon, is_system, sort_order)
             SELECT ?1, ?2, ?3, 1, ?4 WHERE NOT EXISTS (
                 SELECT 1 FROM timeline_activity_types WHERE name=?1 AND is_system=1
             )",
            rusqlite::params![name, color, icon, order],
        ).ok();
    }
}

pub fn migrate_sleep(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sleep_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            quality_score INTEGER,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(date, start_time, source)
        );
        CREATE TABLE IF NOT EXISTS sleep_stages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL REFERENCES sleep_sessions(id) ON DELETE CASCADE,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            stage TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sleep_date ON sleep_sessions(date);
        CREATE INDEX IF NOT EXISTS idx_sleep_stages_session ON sleep_stages(session_id);

        -- v0.32.0: Development Projects, Skills, Cases
        CREATE TABLE IF NOT EXISTS dev_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            icon TEXT NOT NULL DEFAULT '📁',
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS dev_skills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES dev_projects(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            theory TEXT NOT NULL DEFAULT '',
            score INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS dev_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_id INTEGER NOT NULL REFERENCES dev_skills(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            url TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            score INTEGER DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            solved_at TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_dev_skills_project ON dev_skills(project_id);
        CREATE INDEX IF NOT EXISTS idx_dev_cases_skill ON dev_cases(skill_id);

        -- v0.34.0: Heart rate samples for Health Connect integration
        CREATE TABLE IF NOT EXISTS heart_rate_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            time TEXT NOT NULL,
            bpm INTEGER NOT NULL,
            source TEXT NOT NULL DEFAULT 'health_connect',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(date, time, source)
        );
        CREATE INDEX IF NOT EXISTS idx_hr_samples_date ON heart_rate_samples(date);"
    ).ok();

    // Seed PM project with skills if not exists
    seed_pm_project(conn);
}

fn seed_pm_project(conn: &rusqlite::Connection) {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM dev_projects", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 {
        // Add theory column if missing (migration from earlier version)
        conn.execute_batch("ALTER TABLE dev_skills ADD COLUMN theory TEXT NOT NULL DEFAULT ''").ok();
        // Add new skills + update existing ones with missing theory/description
        let pid: i64 = conn.query_row("SELECT id FROM dev_projects WHERE name='PM'", [], |r| r.get(0)).unwrap_or(0);
        if pid > 0 {
            let now = chrono::Local::now().to_rfc3339();
            for (i, (name, desc, theory)) in pm_skills().iter().enumerate() {
                let exists: bool = conn.query_row(
                    "SELECT COUNT(*)>0 FROM dev_skills WHERE project_id=?1 AND name=?2",
                    rusqlite::params![pid, name], |r| r.get(0)
                ).unwrap_or(false);
                if exists {
                    // Update empty description/theory on existing skills
                    conn.execute(
                        "UPDATE dev_skills SET description=?1, theory=?2, sort_order=?3 WHERE project_id=?4 AND name=?5 AND (description='' OR theory='')",
                        rusqlite::params![desc, theory, i as i32, pid, name],
                    ).ok();
                } else {
                    conn.execute(
                        "INSERT INTO dev_skills (project_id, name, description, theory, score, sort_order, created_at, updated_at) VALUES (?1,?2,?3,?4,0,?5,?6,?6)",
                        rusqlite::params![pid, name, desc, theory, i as i32, now],
                    ).ok();
                }
            }
        }
        return;
    }
    let now = chrono::Local::now().to_rfc3339();
    conn.execute("INSERT INTO dev_projects (name, icon, sort_order, created_at) VALUES ('PM', '📦', 0, ?1)", rusqlite::params![now]).ok();
    let pid: i64 = conn.last_insert_rowid();
    for (i, (name, desc, theory)) in pm_skills().iter().enumerate() {
        conn.execute(
            "INSERT INTO dev_skills (project_id, name, description, theory, score, sort_order, created_at, updated_at) VALUES (?1,?2,?3,?4,0,?5,?6,?6)",
            rusqlite::params![pid, name, desc, theory, i as i32, now],
        ).ok();
    }
}

fn pm_skills() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("Discovery & User Research",
         "Выявление проблем пользователей, проведение интервью, анализ потребностей",
         "## Что это\nПроцесс поиска и валидации проблем пользователей до начала разработки.\n\n## Ключевые методы\n- **CustDev-интервью** — глубинные интервью с пользователями\n- **Jobs To Be Done (JTBD)** — какую «работу» нанимает пользователь\n- **Персоны** — архетипы целевых пользователей\n- **Customer Journey Map (CJM)** — карта пути пользователя\n- **Surveys & Questionnaires** — количественная валидация\n\n## Ключевые вопросы\n- Какую проблему решаем?\n- Для кого?\n- Как пользователь решает это сейчас?\n- Готов ли платить?"),
        ("Prioritization",
         "Фреймворки приоритизации: RICE, ICE, MoSCoW, Kano",
         "## Что это\nУмение выбирать, что делать первым при ограниченных ресурсах.\n\n## Фреймворки\n- **RICE** — Reach × Impact × Confidence / Effort\n- **ICE** — Impact × Confidence × Ease\n- **MoSCoW** — Must / Should / Could / Won't\n- **Kano Model** — Basic / Performance / Excitement фичи\n- **Value vs Effort Matrix** — 2×2 матрица\n\n## Когда применять\n- Планирование спринта / квартала\n- Backlog grooming\n- Защита roadmap перед стейкхолдерами"),
        ("Metrics & Analytics",
         "Продуктовые метрики, AARRR, North Star, юнит-экономика",
         "## Что это\nИзмерение успеха продукта через данные.\n\n## Фреймворки\n- **AARRR (Pirate Metrics)** — Acquisition, Activation, Retention, Revenue, Referral\n- **North Star Metric** — одна метрика, отражающая ценность для пользователя\n- **HEART** (Google) — Happiness, Engagement, Adoption, Retention, Task Success\n\n## Юнит-экономика\n- **LTV** (Lifetime Value) — сколько приносит один пользователь за всё время\n- **CAC** (Customer Acquisition Cost) — стоимость привлечения\n- **LTV/CAC > 3** — здоровый бизнес\n- **Payback Period** — время окупаемости CAC\n- **ARPU** — средний доход на пользователя\n- **Churn Rate** — процент оттока"),
        ("Roadmapping",
         "Составление и защита продуктового роадмапа",
         "## Что это\nСтратегический план развития продукта, привязанный ко времени и целям.\n\n## Типы роадмапов\n- **Now / Next / Later** — гибкий, без точных дат\n- **Timeline-based** — привязан к кварталам/спринтам\n- **Outcome-based** — привязан к метрикам, а не фичам\n\n## Как защищать\n- Привязывать к бизнес-целям\n- Показывать trade-off (что НЕ делаем и почему)\n- Использовать данные, а не мнения"),
        ("Stakeholder Management",
         "Работа с заинтересованными сторонами: CEO, разработка, маркетинг, поддержка",
         "## Что это\nУмение управлять ожиданиями и коммуникацией с разными сторонами.\n\n## Ключевые навыки\n- **Stakeholder Mapping** — кто влияет, кто заинтересован\n- **Управление ожиданиями** — прозрачность, регулярные апдейты\n- **Negotiation** — умение говорить «нет» с обоснованием\n- **Alignment** — синхронизация целей между командами\n\n## Типичные стейкхолдеры\nCEO/Founder, CTO, Marketing, Sales, Support, Design, Engineering"),
        ("User Stories & Requirements",
         "Написание требований, user stories, acceptance criteria",
         "## Что это\nПеревод бизнес-потребностей в понятные задачи для разработки.\n\n## Форматы\n- **User Story** — As a [user], I want [action] so that [benefit]\n- **Job Story** — When [situation], I want to [motivation], so I can [outcome]\n- **Acceptance Criteria** — Given/When/Then (Gherkin)\n\n## Что включать в PRD\n- Проблема и контекст\n- Целевая аудитория\n- User stories + acceptance criteria\n- Wireframes / mockups\n- Метрики успеха\n- Edge cases"),
        ("A/B Testing & Experimentation",
         "Дизайн экспериментов, статзначимость, анализ результатов",
         "## Что это\nПроверка гипотез через контролируемые эксперименты.\n\n## Процесс\n1. Сформулировать гипотезу (If… Then… Because…)\n2. Определить метрику и размер выборки\n3. Запустить тест (контроль vs вариант)\n4. Дождаться статзначимости (p < 0.05)\n5. Принять решение\n\n## Ключевые понятия\n- **Статзначимость** — p-value < 0.05\n- **MDE** (Minimum Detectable Effect)\n- **Sample Size** — калькулятор Эвана Миллера\n- **Ошибки Type I / Type II**"),
        ("Go-to-Market",
         "Запуск продукта/фичи, позиционирование, каналы",
         "## Что это\nСтратегия вывода продукта или фичи на рынок.\n\n## Компоненты GTM\n- **Positioning** — для кого, чем отличаемся\n- **Messaging** — как объясняем ценность\n- **Channels** — где достигаем пользователей\n- **Pricing** — модель монетизации\n- **Launch Plan** — этапы запуска\n\n## Чеклист запуска\n- [ ] Документация готова\n- [ ] Support обучен\n- [ ] Метрики настроены\n- [ ] Rollback plan есть"),
        ("Technical Understanding",
         "Понимание архитектуры, API, баз данных, инфраструктуры",
         "## Что это\nДостаточное техническое понимание для продуктивной работы с разработкой.\n\n## Минимум для PM\n- **API** — REST, endpoints, request/response\n- **Базы данных** — SQL basics, реляционные vs NoSQL\n- **Frontend vs Backend** — где что происходит\n- **CI/CD** — деплой, staging, production\n- **Архитектура** — микросервисы, монолит, serverless\n\n## Зачем\n- Оценивать сложность задач\n- Говорить с разработчиками на одном языке\n- Понимать технические ограничения"),
        ("Communication & Presentation",
         "Питчи, презентации, документация, storytelling",
         "## Что это\nУмение ясно доносить идеи устно и письменно.\n\n## Навыки\n- **Storytelling** — проблема → решение → результат\n- **Executive Summary** — суть на 1 странице\n- **Презентации** — структура, визуал, delivery\n- **Written Communication** — PRD, RFC, emails\n- **Active Listening** — задавать правильные вопросы\n\n## Форматы\n- **Elevator Pitch** — 30 секунд\n- **Product Review** — 15 мин для стейкхолдеров\n- **All-Hands** — широкая аудитория"),
        ("Competitive Analysis",
         "Анализ рынка, конкурентов, позиционирование",
         "## Что это\nСистемный анализ конкурентной среды для принятия продуктовых решений.\n\n## Методы\n- **Feature Matrix** — сравнение фич с конкурентами\n- **SWOT** — Strengths, Weaknesses, Opportunities, Threats\n- **Porter's Five Forces** — анализ отрасли\n- **Blue Ocean Strategy** — новые рыночные пространства\n\n## Что отслеживать\n- Фичи и pricing конкурентов\n- Отзывы их пользователей\n- Их positioning и messaging\n- Тренды рынка"),
        ("Strategy & Vision",
         "Продуктовое видение, стратегия, OKR",
         "## Что это\nДолгосрочное видение продукта и стратегия его достижения.\n\n## Компоненты\n- **Vision** — куда идём через 3-5 лет\n- **Mission** — зачем существуем\n- **Strategy** — как достигнем vision\n- **OKR** — Objectives and Key Results (квартальные цели)\n- **KPI** — ключевые метрики\n\n## Фреймворки\n- **Product Vision Board** (Roman Pichler)\n- **Lean Canvas** — бизнес-модель на 1 странице\n- **Strategy Canvas** — визуализация конкурентной позиции"),
        ("SQL & Data Analysis",
         "SQL-запросы, работа с данными, дашборды, Excel/Sheets",
         "## Что это\nПрактический навык извлечения и анализа данных для принятия решений.\n\n## SQL основы\n- **SELECT, WHERE, GROUP BY, HAVING, ORDER BY**\n- **JOIN** — INNER, LEFT, RIGHT\n- **Агрегации** — COUNT, SUM, AVG, MIN, MAX\n- **Подзапросы и CTE** (WITH)\n- **Window Functions** — ROW_NUMBER, LAG, LEAD\n\n## Инструменты\n- SQL (PostgreSQL, MySQL, BigQuery)\n- Excel / Google Sheets (pivot tables, VLOOKUP)\n- BI-инструменты (Metabase, Looker, Tableau, Power BI)\n\n## Применение\n- Построение дашбордов\n- Ad-hoc анализ для product decisions\n- Когортный анализ"),
        ("Agile & Scrum",
         "Agile-методологии, Scrum, Kanban, спринты, ретроспективы",
         "## Что это\nИтеративный подход к разработке продукта.\n\n## Scrum\n- **Sprint** — 1-2 недели\n- **Ceremonies** — Planning, Daily, Review, Retro\n- **Roles** — PO, Scrum Master, Dev Team\n- **Artifacts** — Backlog, Sprint Backlog, Increment\n\n## Kanban\n- Визуализация потока (To Do → In Progress → Done)\n- WIP-лимиты\n- Continuous delivery\n\n## PM в Agile\n- Grooming backlog\n- Приоритизация задач\n- Принятие решений по scope"),
        ("UX/UI Fundamentals",
         "Основы дизайна, wireframes, user flows, юзабилити",
         "## Что это\nПонимание принципов дизайна для эффективной работы с дизайнерами.\n\n## UX основы\n- **Information Architecture** — структура контента\n- **User Flow** — путь пользователя по продукту\n- **Wireframes** — скелетная структура экранов\n- **Prototyping** — интерактивные прототипы (Figma)\n- **Usability Testing** — тестирование с реальными пользователями\n\n## UI основы\n- Типографика, цвет, spacing\n- Design System / Component Library\n- Responsive design\n- Accessibility (a11y)"),
        ("Customer Development",
         "CustDev-интервью, проблемные и решенческие интервью, Product-Market Fit",
         "## Что это\nМетодология валидации бизнес-гипотез через общение с клиентами.\n\n## Типы интервью\n- **Проблемное** — есть ли проблема? Как решают сейчас?\n- **Решенческое** — подходит ли наше решение?\n- **Экспертное** — мнение специалистов рынка\n\n## Product-Market Fit\n- **Sean Ellis Test** — >40% ответили «very disappointed» без продукта\n- **Retention Curve** — выходит на плато\n- **Organic Growth** — пользователи приходят сами\n\n## The Mom Test (Rob Fitzpatrick)\n- Не спрашивай «нравится ли тебе идея»\n- Спрашивай про реальный опыт и поведение\n- Ищи факты, а не комплименты"),
        ("Pricing & Monetization",
         "Модели монетизации, ценообразование, unit economics",
         "## Что это\nОпределение того, как продукт зарабатывает деньги.\n\n## Модели монетизации\n- **Freemium** — бесплатный базовый + платный premium\n- **Subscription** — ежемесячная/годовая подписка\n- **Transaction Fee** — комиссия с каждой транзакции\n- **Advertising** — рекламная модель\n- **Marketplace** — комиссия с обеих сторон\n\n## Ценообразование\n- **Value-based** — цена = воспринимаемая ценность\n- **Cost-plus** — себестоимость + маржа\n- **Competitive** — относительно конкурентов\n\n## Метрики\n- MRR/ARR, ARPU, Conversion Rate, Churn"),
        ("Growth & Retention",
         "Воронки роста, retention, activation, виральность",
         "## Что это\nСтратегии привлечения, активации и удержания пользователей.\n\n## Воронка\n- **Acquisition** — откуда приходят пользователи\n- **Activation** — первый «aha moment»\n- **Retention** — возвращаются ли?\n- **Revenue** — платят ли?\n- **Referral** — рекомендуют ли?\n\n## Retention\n- **Day 1 / Day 7 / Day 30 Retention**\n- **Cohort Analysis** — сравнение когорт по времени\n- **Retention Curve** — цель: выход на плато\n\n## Growth Loops\n- Viral loop (invite friends)\n- Content loop (user-generated content → SEO)\n- Paid loop (revenue → ads → users)"),
    ]
}

/// Convert regular tables to CRRs (conflict-free replicated relations) for sync.
/// Skips FTS5, vec0, and device-specific tables. Safe to call repeatedly.
pub fn enable_crr_tables(conn: &rusqlite::Connection) {
    // Check if cr-sqlite is loaded
    let loaded: bool = conn.query_row(
        "SELECT count(*) > 0 FROM pragma_function_list WHERE name='crsql_as_crr'",
        [], |r| r.get(0),
    ).unwrap_or(false);
    if !loaded {
        eprintln!("cr-sqlite not loaded, skipping CRR setup");
        return;
    }

    let tables = [
        "facts", "conversations", "activities", "notes", "events",
        "projects", "tasks", "learning_items", "hobbies", "hobby_entries",
        "workouts", "exercises", "health_log", "habits", "habit_checks",
        "media_items", "user_lists", "list_items", "food_log", "recipes",
        "products", "transactions", "budgets", "savings_goals",
        "subscriptions", "debts", "journal_entries", "mood_log",
        "principles", "blocklist", "tab_goals", "home_items",
        "contacts", "contact_blocks", "page_meta", "property_definitions",
        "property_values", "view_configs", "ui_state", "activity_snapshots",
        "proactive_history", "message_feedback", "conversation_insights",
        "reminders", "flywheel_cycles", "custom_pages", "tab_page_blocks",
        "note_tags", "schedules", "schedule_completions", "dan_koe_entries",
        "proactive_messages", "project_records", "body_records",
        "job_sources", "job_roles", "job_vacancies", "job_search_log",
        "dashboard_widgets", "timeline_activity_types", "timeline_blocks",
        "timeline_goals", "sleep_sessions", "sleep_stages", "heart_rate_samples",
    ];

    for table in &tables {
        let sql = format!("SELECT crsql_as_crr('{}')", table);
        if let Err(e) = conn.execute_batch(&sql) {
            eprintln!("CRR skip {}: {}", table, e);
        }
    }
    eprintln!("CRR enabled for {} tables", tables.len());
}
