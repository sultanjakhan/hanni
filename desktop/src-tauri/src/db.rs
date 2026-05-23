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
            category TEXT NOT NULL DEFAULT 'other',
            tags TEXT NOT NULL DEFAULT ''
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
    // (name, category, tags)
    let items: Vec<(&str, &str, &str)> = vec![
        // meat — птица
        ("курица", "meat", "птица"), ("куриное филе", "meat", "птица"),
        ("куриные бёдра", "meat", "птица"), ("куриные крылышки", "meat", "птица"),
        ("индейка", "meat", "птица"), ("утка", "meat", "птица"),
        ("фарш куриный", "meat", "птица"), ("печень куриная", "meat", "птица,субпродукты"),
        // meat — говядина
        ("говядина", "meat", "говядина"), ("телятина", "meat", "говядина"),
        ("фарш говяжий", "meat", "говядина"), ("печень говяжья", "meat", "говядина,субпродукты"),
        ("язык говяжий", "meat", "говядина,субпродукты"),
        // meat — баранина, конина, прочее
        ("баранина", "meat", "баранина"), ("конина", "meat", "конина"),
        ("кролик", "meat", ""), ("колбаса варёная", "meat", "полуфабрикаты"),
        ("колбаса копчёная", "meat", "полуфабрикаты"), ("сосиски", "meat", "полуфабрикаты"),
        ("тушёнка", "meat", "полуфабрикаты"),
        // fish — красная рыба
        ("лосось", "fish", "красная рыба"), ("сёмга", "fish", "красная рыба"),
        ("форель", "fish", "красная рыба"), ("икра красная", "fish", "красная рыба"),
        // fish — белая рыба
        ("треска", "fish", "белая рыба"), ("минтай", "fish", "белая рыба"),
        ("скумбрия", "fish", "белая рыба"), ("сельдь", "fish", "белая рыба"),
        ("карп", "fish", "белая рыба"), ("тунец", "fish", "белая рыба"),
        ("шпроты", "fish", "белая рыба"),
        // fish — морепродукты
        ("креветки", "fish", "морепродукты"), ("кальмар", "fish", "морепродукты"),
        ("мидии", "fish", "морепродукты"), ("крабовые палочки", "fish", "морепродукты"),
        // veg — корнеплоды
        ("морковь", "veg", "корнеплоды"), ("картофель", "veg", "корнеплоды"),
        ("свёкла", "veg", "корнеплоды"), ("редис", "veg", "корнеплоды"),
        ("редька", "veg", "корнеплоды"), ("имбирь", "veg", "корнеплоды"),
        // veg — паслёновые
        ("помидор", "veg", "паслёновые"), ("перец болгарский", "veg", "паслёновые"),
        ("перец чили", "veg", "паслёновые"), ("баклажан", "veg", "паслёновые"),
        // veg — капустные
        ("капуста белокочанная", "veg", "капустные"), ("капуста пекинская", "veg", "капустные"),
        ("капуста цветная", "veg", "капустные"), ("брокколи", "veg", "капустные"),
        // veg — зелень
        ("шпинат", "veg", "зелень"), ("салат айсберг", "veg", "зелень"),
        ("руккола", "veg", "зелень"), ("сельдерей", "veg", "зелень"),
        // veg — прочие
        ("лук", "veg", ""), ("лук красный", "veg", ""), ("лук-порей", "veg", ""),
        ("чеснок", "veg", ""), ("огурец", "veg", ""), ("кабачок", "veg", ""),
        ("тыква", "veg", ""), ("кукуруза", "veg", ""),
        ("горошек зелёный", "veg", ""), ("стручковая фасоль", "veg", ""),
        ("грибы шампиньоны", "veg", ""), ("грибы вёшенки", "veg", ""),
        // fruit — цитрусовые
        ("апельсин", "fruit", "цитрусовые"), ("лимон", "fruit", "цитрусовые"),
        ("лайм", "fruit", "цитрусовые"), ("мандарин", "fruit", "цитрусовые"),
        ("грейпфрут", "fruit", "цитрусовые"),
        // fruit — ягоды
        ("клубника", "fruit", "ягоды"), ("малина", "fruit", "ягоды"),
        ("черника", "fruit", "ягоды"), ("вишня", "fruit", "ягоды"),
        ("виноград", "fruit", "ягоды"),
        // fruit — тропические
        ("манго", "fruit", "тропические"), ("ананас", "fruit", "тропические"),
        ("киви", "fruit", "тропические"), ("авокадо", "fruit", "тропические"),
        ("банан", "fruit", "тропические"),
        // fruit — косточковые
        ("персик", "fruit", "косточковые"), ("слива", "fruit", "косточковые"),
        ("хурма", "fruit", "косточковые"),
        // fruit — прочие
        ("яблоко", "fruit", ""), ("груша", "fruit", ""),
        ("арбуз", "fruit", ""), ("дыня", "fruit", ""), ("гранат", "fruit", ""),
        // fruit — сухофрукты
        ("изюм", "fruit", "сухофрукты"), ("курага", "fruit", "сухофрукты"),
        ("чернослив", "fruit", "сухофрукты"), ("финики", "fruit", "сухофрукты"),
        // grain — каша
        ("рис", "grain", "каша"), ("рис басмати", "grain", "каша"),
        ("гречка", "grain", "каша"), ("овсяные хлопья", "grain", "каша"),
        ("пшено", "grain", "каша"), ("булгур", "grain", "каша"),
        ("кус-кус", "grain", "каша"), ("перловка", "grain", "каша"),
        ("манка", "grain", "каша"), ("кукурузная крупа", "grain", "каша"),
        ("киноа", "grain", "каша"),
        // grain — макароны
        ("макароны", "grain", "макароны"), ("спагетти", "grain", "макароны"),
        ("лапша", "grain", "макароны"), ("лапша рисовая", "grain", "макароны"),
        ("фунчоза", "grain", "макароны"),
        // grain — мука
        ("мука пшеничная", "grain", "мука"), ("мука кукурузная", "grain", "мука"),
        ("панировочные сухари", "grain", "мука"),
        // grain — хлеб
        ("хлеб белый", "grain", "хлеб"), ("хлеб чёрный", "grain", "хлеб"),
        ("лаваш", "grain", "хлеб"), ("батон", "grain", "хлеб"),
        // dairy — кисломолочные
        ("кефир", "dairy", "кисломолочные"), ("ряженка", "dairy", "кисломолочные"),
        ("йогурт", "dairy", "кисломолочные"), ("сметана", "dairy", "кисломолочные"),
        ("творог", "dairy", "кисломолочные"), ("творожный сыр", "dairy", "кисломолочные"),
        // dairy — сыр
        ("сыр твёрдый", "dairy", "сыр"), ("пармезан", "dairy", "сыр"),
        ("моцарелла", "dairy", "сыр"), ("фета", "dairy", "сыр"),
        ("брынза", "dairy", "сыр"), ("плавленый сыр", "dairy", "сыр"),
        // dairy — прочие
        ("молоко", "dairy", ""), ("сливки", "dairy", ""),
        ("масло сливочное", "dairy", ""), ("яйца куриные", "dairy", ""),
        ("яйца перепелиные", "dairy", ""), ("сгущённое молоко", "dairy", ""),
        ("кокосовое молоко", "dairy", ""),
        // legumes
        ("фасоль", "legumes", ""), ("фасоль красная", "legumes", ""),
        ("фасоль белая", "legumes", ""), ("чечевица", "legumes", ""),
        ("чечевица красная", "legumes", ""), ("горох", "legumes", ""),
        ("нут", "legumes", ""), ("маш", "legumes", ""),
        ("соя", "legumes", ""), ("тофу", "legumes", ""),
        // nuts — орехи
        ("грецкий орех", "nuts", ""), ("миндаль", "nuts", ""),
        ("фундук", "nuts", ""), ("кешью", "nuts", ""),
        ("арахис", "nuts", ""), ("фисташки", "nuts", ""),
        ("кедровые орехи", "nuts", ""), ("кокосовая стружка", "nuts", ""),
        // nuts — семена
        ("семена подсолнечника", "nuts", "семена"), ("семена тыквы", "nuts", "семена"),
        ("семена кунжута", "nuts", "семена"), ("семена льна", "nuts", "семена"),
        ("семена чиа", "nuts", "семена"),
        // spice — приправы
        ("соль", "spice", "приправы"), ("перец чёрный", "spice", "приправы"),
        ("перец красный", "spice", "приправы"), ("паприка", "spice", "приправы"),
        ("куркума", "spice", "приправы"), ("зира", "spice", "приправы"),
        ("кориандр", "spice", "приправы"), ("корица", "spice", "приправы"),
        ("мускатный орех", "spice", "приправы"), ("гвоздика", "spice", "приправы"),
        ("лавровый лист", "spice", "приправы"), ("орегано", "spice", "приправы"),
        ("базилик", "spice", "приправы"), ("тимьян", "spice", "приправы"),
        ("розмарин", "spice", "приправы"), ("ваниль", "spice", "приправы"),
        // spice — зелень
        ("укроп", "spice", "зелень"), ("петрушка", "spice", "зелень"),
        ("кинза", "spice", "зелень"), ("мята", "spice", "зелень"),
        ("зелёный лук", "spice", "зелень"),
        // spice — соусы
        ("соевый соус", "spice", "соусы"), ("томатная паста", "spice", "соусы"),
        ("горчица", "spice", "соусы"), ("майонез", "spice", "соусы"),
        ("кетчуп", "spice", "соусы"), ("сметанный соус", "spice", "соусы"),
        ("аджика", "spice", "соусы"), ("уксус", "spice", "соусы"),
        // spice — прочие
        ("сахар", "spice", ""), ("мёд", "spice", ""),
        // oil — растительные
        ("масло растительное", "oil", "растительные"), ("масло оливковое", "oil", "растительные"),
        ("масло подсолнечное", "oil", "растительные"), ("масло кунжутное", "oil", "растительные"),
        ("масло кокосовое", "oil", "растительные"), ("масло льняное", "oil", "растительные"),
        // bakery — тесто
        ("дрожжи", "bakery", "тесто"), ("разрыхлитель", "bakery", "тесто"),
        ("крахмал", "bakery", "тесто"), ("желатин", "bakery", "тесто"),
        ("сахарная пудра", "bakery", "тесто"),
        // bakery — шоколад
        ("какао", "bakery", ""), ("шоколад тёмный", "bakery", ""),
        ("шоколад молочный", "bakery", ""),
        // drinks
        ("чай чёрный", "drinks", "чай"), ("чай зелёный", "drinks", "чай"),
        ("кофе", "drinks", "кофе"), ("какао-порошок", "drinks", ""),
        ("сок апельсиновый", "drinks", "сок"),
        ("вода минеральная", "drinks", ""), ("компот", "drinks", ""),
    ];
    for (name, cat, tags) in items {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO ingredient_catalog (name, category, tags) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, cat, tags],
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

// Photo (data URL), taste rating (0-5) and post-cooking note for recipes.
pub fn migrate_recipe_media(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT image FROM recipes LIMIT 1").is_err() {
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN image TEXT", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN taste_rating INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE recipes ADD COLUMN cook_note TEXT NOT NULL DEFAULT ''", []);
    }
}

// Alternative ingredients per recipe row: comma-separated names that can be
// substituted (e.g. баранина → говядина / курица). MatchIngr in JS reads the
// flattened recipes.ingredients text so filter-by-ingredient picks them up.
pub fn migrate_ingredient_alternatives(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT alternatives FROM recipe_ingredients LIMIT 1").is_err() {
        let _ = conn.execute(
            "ALTER TABLE recipe_ingredients ADD COLUMN alternatives TEXT NOT NULL DEFAULT ''",
            [],
        );
    }
}

// Per-cooking history: each cooking of a recipe is one immutable row with its
// own date + taste rating + note, optionally linked to a calendar event.
pub fn migrate_cooking_log(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cooking_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recipe_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            taste_rating INTEGER NOT NULL DEFAULT 0,
            cook_note TEXT NOT NULL DEFAULT '',
            event_id INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_cooking_log_recipe ON cooking_log(recipe_id);"
    ).ok();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO event_categories (name, color, icon, sort_order, created_at) VALUES ('Готовка', '#cb8a05', '🍳', 7, ?1)",
        rusqlite::params![now],
    ).ok();
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

pub fn migrate_catalog_tags_v3(conn: &rusqlite::Connection) {
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='catalog_tags_v3'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    // Add tags column if missing
    let has_tags = conn.prepare("SELECT tags FROM ingredient_catalog LIMIT 1").is_ok();
    if !has_tags {
        let _ = conn.execute("ALTER TABLE ingredient_catalog ADD COLUMN tags TEXT NOT NULL DEFAULT ''", []);
    }
    // Re-seed: clear and re-populate with tags + no pork
    let _ = conn.execute("DELETE FROM ingredient_catalog", []);
    seed_ingredient_catalog(conn);
    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('catalog_tags_v3')", []);
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
            contemplation_text TEXT NOT NULL DEFAULT '',
            vision_text TEXT NOT NULL DEFAULT '',
            integration_text TEXT NOT NULL DEFAULT '',
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
    // v0.40: schedule end date (after which it's considered expired)
    conn.execute("ALTER TABLE schedules ADD COLUMN until_date TEXT", []).ok();
    // v0.70: Dan Koe text responses for contemplation/vision/integration
    conn.execute("ALTER TABLE dan_koe_entries ADD COLUMN contemplation_text TEXT NOT NULL DEFAULT ''", []).ok();
    conn.execute("ALTER TABLE dan_koe_entries ADD COLUMN vision_text TEXT NOT NULL DEFAULT ''", []).ok();
    conn.execute("ALTER TABLE dan_koe_entries ADD COLUMN integration_text TEXT NOT NULL DEFAULT ''", []).ok();
    // v0.74: track_overdue — show missed schedule occurrences as overdue (manual flag per item)
    conn.execute("ALTER TABLE schedules ADD COLUMN track_overdue INTEGER NOT NULL DEFAULT 0", []).ok();
    // v0.74: target_minutes — daily target duration for the schedule (NULL = no target, single completion)
    conn.execute("ALTER TABLE schedules ADD COLUMN target_minutes INTEGER", []).ok();
    // v0.79: tracking_mode — how the user interacts with this schedule from the "+" popup.
    // 'track' (default) starts a timeline_block timer; 'check' instantly marks it done.
    conn.execute("ALTER TABLE schedules ADD COLUMN tracking_mode TEXT NOT NULL DEFAULT 'track'", []).ok();
    // v0.74: reflection fields
    // notes.estimate_minutes — planned duration set by user
    conn.execute("ALTER TABLE notes ADD COLUMN estimate_minutes INTEGER", []).ok();
    // timeline_blocks.quality (0..5), reflection (text), mood ('happy'|'neutral'|'sad') — collected on ✓ Готово
    conn.execute("ALTER TABLE timeline_blocks ADD COLUMN quality INTEGER NOT NULL DEFAULT 0", []).ok();
    conn.execute("ALTER TABLE timeline_blocks ADD COLUMN reflection TEXT", []).ok();
    conn.execute("ALTER TABLE timeline_blocks ADD COLUMN mood TEXT", []).ok();
}

/// Next-action engine — graph model: a chain is a canvas, a node is a task
/// (referencing a schedule/note/event, or a start trigger), an edge is an arrow
/// with a transition trigger. routine_node_status tracks a node's state inside
/// one routine_run (a daily pass of the chain).
pub fn migrate_routine_engine(conn: &rusqlite::Connection) {
    // v1 of this engine used stage-based tables; drop the unused stage table.
    conn.execute("DROP TABLE IF EXISTS routine_stages", []).ok();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS routine_chains (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            trigger_type TEXT NOT NULL DEFAULT 'manual',
            is_active INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS routine_nodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chain_id INTEGER NOT NULL REFERENCES routine_chains(id) ON DELETE CASCADE,
            source_type TEXT NOT NULL DEFAULT 'schedule',
            source_id INTEGER,
            title TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'other',
            icon TEXT,
            pos_x INTEGER NOT NULL DEFAULT 0,
            pos_y INTEGER NOT NULL DEFAULT 0,
            priority INTEGER NOT NULL DEFAULT 0,
            requirement TEXT NOT NULL DEFAULT 'required',
            is_start INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS routine_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chain_id INTEGER NOT NULL REFERENCES routine_chains(id) ON DELETE CASCADE,
            from_node_id INTEGER NOT NULL REFERENCES routine_nodes(id) ON DELETE CASCADE,
            to_node_id INTEGER NOT NULL REFERENCES routine_nodes(id) ON DELETE CASCADE,
            trigger_type TEXT NOT NULL DEFAULT 'after_completion',
            trigger_value INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS routine_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chain_id INTEGER NOT NULL REFERENCES routine_chains(id) ON DELETE CASCADE,
            date TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT 'active',
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at TEXT,
            UNIQUE(chain_id, date)
        );
        CREATE TABLE IF NOT EXISTS routine_node_status (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL REFERENCES routine_runs(id) ON DELETE CASCADE,
            node_id INTEGER NOT NULL REFERENCES routine_nodes(id) ON DELETE CASCADE,
            state TEXT NOT NULL DEFAULT 'done',
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(run_id, node_id)
        );
        CREATE INDEX IF NOT EXISTS idx_routine_nodes_chain ON routine_nodes(chain_id);
        CREATE INDEX IF NOT EXISTS idx_routine_edges_chain ON routine_edges(chain_id);
        CREATE INDEX IF NOT EXISTS idx_routine_runs_date ON routine_runs(date);
        CREATE INDEX IF NOT EXISTS idx_routine_node_status_run ON routine_node_status(run_id);"
    ).ok();
    cleanup_v1_routine_chains(conn);
    seed_morning_routine(conn);
}

/// One-time cleanup: v1 seeded an empty stage-based "Утро" chain (no nodes).
/// Remove any chain that has no nodes. Runs once via _migrations.
fn cleanup_v1_routine_chains(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_v1_cleanup'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    conn.execute(
        "DELETE FROM routine_chains WHERE id NOT IN (SELECT DISTINCT chain_id FROM routine_nodes)",
        [],
    ).ok();
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_v1_cleanup')", []).ok();
}

/// Seed the "Morning" graph: a start node + task nodes + edges. Idempotent via _migrations.
/// Task nodes are autonomous (source_id NULL) — the user attaches them to real
/// schedules/notes/events later in the constructor.
fn seed_morning_routine(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_morning_seed_v2'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    if conn.execute(
        "INSERT INTO routine_chains (title, trigger_type, sort_order) VALUES ('Утро', 'sleep_end', 0)",
        [],
    ).is_ok() {
        let chain_id = conn.last_insert_rowid();
        // (key, title, category, pri, req, x, y, is_start)
        let nodes = [
            ("start", "Проснулся",         "other",   0, "required", 30,  210, 1),
            ("up",    "Встал",             "home",    5, "required", 200, 200, 0),
            ("bed",   "Заправил кровать",  "home",    3, "required", 200, 340, 0),
            ("toil",  "Туалет",            "hygiene", 4, "required", 445, 30,  0),
            ("wash",  "Умылся",            "hygiene", 4, "required", 445, 200, 0),
            ("teeth", "Зубы",              "hygiene", 5, "required", 445, 370, 0),
            ("vit",   "Витамины",          "health",  4, "required", 710, 120, 0),
            ("exer",  "Зарядка 10 мин",    "sport",   2, "optional", 710, 300, 0),
        ];
        let mut ids = std::collections::HashMap::new();
        for (key, title, cat, pri, req, x, y, is_start) in nodes {
            let stype = if is_start == 1 { "start" } else { "schedule" };
            conn.execute(
                "INSERT INTO routine_nodes
                 (chain_id, source_type, title, category, priority, requirement, pos_x, pos_y, is_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![chain_id, stype, title, cat, pri, req, x, y, is_start],
            ).ok();
            ids.insert(key, conn.last_insert_rowid());
        }
        let edges = [
            ("start","up"), ("up","bed"), ("bed","toil"), ("bed","wash"),
            ("bed","teeth"), ("toil","vit"), ("wash","vit"), ("teeth","vit"), ("vit","exer"),
        ];
        for (from, to) in edges {
            conn.execute(
                "INSERT INTO routine_edges (chain_id, from_node_id, to_node_id) VALUES (?1, ?2, ?3)",
                rusqlite::params![chain_id, ids[from], ids[to]],
            ).ok();
        }
    }
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_morning_seed_v2')", []).ok();
}

/// v0.70: Remove Mindset tab data (journal_entries, mood_log, principles)
pub fn migrate_drop_mindset(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='drop_mindset_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if !done {
        let _ = conn.execute("DROP TABLE IF EXISTS journal_entries", []);
        let _ = conn.execute("DROP TABLE IF EXISTS mood_log", []);
        let _ = conn.execute("DROP TABLE IF EXISTS principles", []);
        let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('drop_mindset_v1')", []);
    }
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

// Today timeline: link blocks to source (Calendar/Schedule/Notes), track active block
pub fn migrate_timeline_today(conn: &rusqlite::Connection) {
    conn.execute("ALTER TABLE timeline_blocks ADD COLUMN is_active INTEGER DEFAULT 0", []).ok();
    conn.execute("ALTER TABLE timeline_blocks ADD COLUMN source_type TEXT", []).ok();
    conn.execute("ALTER TABLE timeline_blocks ADD COLUMN source_id INTEGER", []).ok();
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tl_blocks_active ON timeline_blocks(date) WHERE is_active = 1", []).ok();
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tl_blocks_source ON timeline_blocks(source_type, source_id)", []).ok();
    conn.execute("ALTER TABLE schedule_completions ADD COLUMN status TEXT DEFAULT 'done'", []).ok();
    conn.execute(
        "INSERT INTO timeline_activity_types (name, color, icon, is_system, sort_order)
         SELECT ?1, ?2, ?3, 1, ?4 WHERE NOT EXISTS (
             SELECT 1 FROM timeline_activity_types WHERE name=?1 AND is_system=1
         )",
        rusqlite::params!["Запланировано", "#3b82f6", "📋", 6i64],
    ).ok();
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
            overview TEXT NOT NULL DEFAULT '',
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );
        -- v0.82.0: competency matrix — single tree table (area/competency/skill)
        CREATE TABLE IF NOT EXISTS dev_nodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES dev_projects(id) ON DELETE CASCADE,
            parent_id INTEGER REFERENCES dev_nodes(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            score INTEGER DEFAULT 0,
            theory TEXT NOT NULL DEFAULT '',
            material TEXT NOT NULL DEFAULT '',
            priority INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS dev_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id INTEGER NOT NULL REFERENCES dev_nodes(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            url TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            score INTEGER DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            solved_at TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_dev_nodes_project ON dev_nodes(project_id);
        CREATE INDEX IF NOT EXISTS idx_dev_nodes_parent ON dev_nodes(parent_id);
        CREATE INDEX IF NOT EXISTS idx_dev_cases_node ON dev_cases(node_id);

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

    // v0.81.0: per-project wiki overview column (idempotent for existing installs)
    conn.execute("ALTER TABLE dev_projects ADD COLUMN overview TEXT NOT NULL DEFAULT ''", []).ok();

    // PM project row; the competency matrix is seeded by migrate_dev_matrix().
    seed_pm_project(conn);
}

fn seed_pm_project(conn: &rusqlite::Connection) {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM dev_projects", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 { return; }
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO dev_projects (name, icon, sort_order, created_at) VALUES ('PM', '📦', 0, ?1)",
        rusqlite::params![now],
    ).ok();
}

/// Migrate the dev tab from flat skills to the 3-level competency matrix.
/// Drops the superseded dev_skills table, rebuilds dev_cases with a node_id
/// FK, and seeds the PM matrix. Idempotent — safe on fresh/repeat runs.
pub fn migrate_dev_matrix(conn: &rusqlite::Connection) {
    conn.execute("DROP TABLE IF EXISTS dev_skills", []).ok();

    // dev_cases moved skill_id -> node_id; rebuild if still on the old schema.
    let has_node_id: bool = conn.query_row(
        "SELECT COUNT(*)>0 FROM pragma_table_info('dev_cases') WHERE name='node_id'",
        [], |r| r.get(0),
    ).unwrap_or(false);
    if !has_node_id {
        conn.execute("DROP TABLE IF EXISTS dev_cases", []).ok();
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS dev_nodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES dev_projects(id) ON DELETE CASCADE,
            parent_id INTEGER REFERENCES dev_nodes(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            score INTEGER DEFAULT 0,
            theory TEXT NOT NULL DEFAULT '',
            material TEXT NOT NULL DEFAULT '',
            priority INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            level TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS dev_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id INTEGER NOT NULL REFERENCES dev_nodes(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            url TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            score INTEGER DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            solved_at TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_dev_nodes_project ON dev_nodes(project_id);
        CREATE INDEX IF NOT EXISTS idx_dev_nodes_parent ON dev_nodes(parent_id);
        CREATE INDEX IF NOT EXISTS idx_dev_cases_node ON dev_cases(node_id);"
    ).ok();

    // Add level column for pre-existing dev_nodes tables (CEFR / difficulty tag).
    conn.execute("ALTER TABLE dev_nodes ADD COLUMN level TEXT NOT NULL DEFAULT ''", []).ok();

    seed_pm_matrix(conn);
}

/// Seed the PM project with the competency matrix (areas → competencies →
/// skills) plus practice cases. Guarded by ui_state['pm_matrix_seed_v3'] —
/// bump the version to re-apply updated content (re-seed wipes PM nodes).
fn seed_pm_matrix(conn: &rusqlite::Connection) {
    let done: String = conn.query_row(
        "SELECT value FROM ui_state WHERE key='pm_matrix_seed_v3'", [], |r| r.get(0),
    ).unwrap_or_default();
    if done == "done" { return; }
    let pid: i64 = conn.query_row(
        "SELECT id FROM dev_projects WHERE name='PM'", [], |r| r.get(0),
    ).unwrap_or(0);
    if pid == 0 { return; }
    let now = chrono::Local::now().to_rfc3339();

    // Clean slate so a guarded re-seed (bumped version) stays consistent.
    conn.execute(
        "DELETE FROM dev_cases WHERE node_id IN (SELECT id FROM dev_nodes WHERE project_id=?1)",
        rusqlite::params![pid],
    ).ok();
    conn.execute("DELETE FROM dev_nodes WHERE project_id=?1", rusqlite::params![pid]).ok();

    conn.execute("UPDATE dev_projects SET overview=?1 WHERE id=?2",
        rusqlite::params![crate::pm_matrix::overview(), pid]).ok();

    for (ai, area) in crate::pm_matrix::matrix().iter().enumerate() {
        conn.execute(
            "INSERT INTO dev_nodes (project_id, parent_id, kind, name, sort_order, created_at, updated_at) \
             VALUES (?1, NULL, 'area', ?2, ?3, ?4, ?4)",
            rusqlite::params![pid, area.name, ai as i32, now]).ok();
        let area_id = conn.last_insert_rowid();
        for (ci, comp) in area.competencies.iter().enumerate() {
            conn.execute(
                "INSERT INTO dev_nodes (project_id, parent_id, kind, name, theory, sort_order, created_at, updated_at) \
                 VALUES (?1, ?2, 'competency', ?3, ?4, ?5, ?6, ?6)",
                rusqlite::params![pid, area_id, comp.name, comp.theory, ci as i32, now]).ok();
            let comp_id = conn.last_insert_rowid();
            for (si, sk) in comp.skills.iter().enumerate() {
                conn.execute(
                    "INSERT INTO dev_nodes (project_id, parent_id, kind, name, score, priority, sort_order, created_at, updated_at) \
                     VALUES (?1, ?2, 'skill', ?3, ?4, ?5, ?6, ?7, ?7)",
                    rusqlite::params![pid, comp_id, sk.name, sk.score, sk.priority as i32, si as i32, now]).ok();
            }
        }
    }

    for (comp_name, title, description) in crate::pm_matrix::seed_cases() {
        let cid: i64 = conn.query_row(
            "SELECT id FROM dev_nodes WHERE project_id=?1 AND kind='competency' AND name=?2",
            rusqlite::params![pid, comp_name], |r| r.get(0),
        ).unwrap_or(0);
        if cid == 0 { continue; }
        conn.execute(
            "INSERT INTO dev_cases (node_id, title, url, description, score, notes, created_at) \
             VALUES (?1,?2,'',?3,0,'',?4)",
            rusqlite::params![cid, title, description, now]).ok();
    }

    conn.execute("INSERT OR REPLACE INTO ui_state (key, value) VALUES ('pm_matrix_seed_v3','done')", []).ok();
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
        "subscriptions", "debts", "blocklist", "tab_goals", "home_items",
        "contacts", "contact_blocks", "page_meta", "property_definitions",
        "property_values", "view_configs", "ui_state", "activity_snapshots",
        "proactive_history", "message_feedback", "conversation_insights",
        "reminders", "flywheel_cycles", "custom_pages", "tab_page_blocks",
        "note_tags", "schedules", "schedule_completions", "dan_koe_entries",
        "proactive_messages", "project_records", "body_records",
        "job_sources", "job_roles", "job_vacancies", "job_search_log",
        "dashboard_widgets", "timeline_activity_types", "timeline_blocks",
        "timeline_goals", "sleep_sessions", "sleep_stages", "heart_rate_samples",
        "cooking_log", "shopping_list",
    ];

    for table in &tables {
        let sql = format!("SELECT crsql_as_crr('{}')", table);
        if let Err(e) = conn.execute_batch(&sql) {
            eprintln!("CRR skip {}: {}", table, e);
        }
    }
    eprintln!("CRR enabled for {} tables", tables.len());
}

pub fn migrate_food_blacklist(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS food_blacklist (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL CHECK(type IN ('tag','product','category','keyword','recipe')),
            value TEXT NOT NULL,
            level TEXT NOT NULL DEFAULT 'hard' CHECK(level IN ('hard','soft','love')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(type, value)
        );"
    ).ok();

    // Two-level blacklist: hard ("не ем") hides everywhere; soft ("не люблю")
    // deprioritises. Existing rows default to hard. ALTER can't add CHECK — the
    // constraint lives in CREATE TABLE above (fresh installs) + Rust validation.
    if conn.prepare("SELECT level FROM food_blacklist LIMIT 1").is_err() {
        conn.execute(
            "ALTER TABLE food_blacklist ADD COLUMN level TEXT NOT NULL DEFAULT 'hard'",
            [],
        ).ok();
    }

    // Blacklist references the product catalog hierarchy (category / subgroup /
    // product) — the free-text "keyword" type is dropped. Convert keyword entries
    // that name a real catalog subgroup into subgroup-blocks (stored as type='tag',
    // which the detector matches by subgroup); drop keyword entries with no match.
    conn.execute(
        "UPDATE OR IGNORE food_blacklist SET type='tag' WHERE type='keyword' \
         AND lower(value) IN (SELECT DISTINCT lower(subgroup) FROM ingredient_catalog WHERE subgroup<>'')",
        [],
    ).ok();
    conn.execute("DELETE FROM food_blacklist WHERE type='keyword'", []).ok();

    // One-shot: migrate legacy blacklist from facts (category='food', key contains 'лэклист')
    let already: i64 = conn.query_row("SELECT COUNT(*) FROM food_blacklist", [], |r| r.get(0)).unwrap_or(0);
    if already > 0 { return; }

    let mut stmt = match conn.prepare(
        "SELECT id, value FROM facts WHERE category='food' AND (key LIKE '%лэклист%' OR key LIKE '%blacklist%')"
    ) { Ok(s) => s, Err(_) => return };

    let rows: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map(|m| m.filter_map(|x| x.ok()).collect())
        .unwrap_or_default();

    for (fact_id, val) in &rows {
        for raw in val.split(',') {
            let item = raw.trim().to_lowercase();
            if item.is_empty() { continue; }
            let entry_type = classify_blacklist_item(conn, &item);
            let _ = conn.execute(
                "INSERT OR IGNORE INTO food_blacklist (type, value) VALUES (?1, ?2)",
                rusqlite::params![entry_type, item],
            );
        }
        let _ = conn.execute("DELETE FROM facts WHERE id=?1", rusqlite::params![fact_id]);
    }
}

// Add the 'love' level (positive marker) to food_blacklist. The CHECK constraint
// lives inside CREATE TABLE and can't be ALTERed, so existing DBs whose table SQL
// still forbids 'love' are rebuilt. food_blacklist isn't a CRR table, so the
// drop/rename is safe. Must run after migrate_catalog_links (catalog_id column).
pub fn migrate_food_blacklist_love(conn: &rusqlite::Connection) {
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='food_blacklist'",
        [], |r| r.get(0),
    ).unwrap_or_default();
    if sql.is_empty() || sql.contains("love") { return; }

    conn.execute_batch(
        "CREATE TABLE food_blacklist_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL CHECK(type IN ('tag','product','category','keyword')),
            value TEXT NOT NULL,
            level TEXT NOT NULL DEFAULT 'hard' CHECK(level IN ('hard','soft','love')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            catalog_id INTEGER REFERENCES ingredient_catalog(id) ON DELETE SET NULL,
            UNIQUE(type, value)
        );
        INSERT INTO food_blacklist_new (id, type, value, level, created_at, catalog_id)
            SELECT id, type, value, level, created_at, catalog_id FROM food_blacklist;
        DROP TABLE food_blacklist;
        ALTER TABLE food_blacklist_new RENAME TO food_blacklist;"
    ).ok();
}

// Add the 'recipe' type (preferences on whole dishes) to food_blacklist. Same
// rebuild approach as the 'love' migration: the type CHECK can't be ALTERed, so
// existing DBs whose table SQL still forbids 'recipe' are rebuilt to the final
// canonical schema. Idempotent. Must run after migrate_food_blacklist_love.
pub fn migrate_food_blacklist_recipe(conn: &rusqlite::Connection) {
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='food_blacklist'",
        [], |r| r.get(0),
    ).unwrap_or_default();
    if sql.is_empty() || sql.contains("recipe") { return; }

    conn.execute_batch(
        "CREATE TABLE food_blacklist_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL CHECK(type IN ('tag','product','category','keyword','recipe')),
            value TEXT NOT NULL,
            level TEXT NOT NULL DEFAULT 'hard' CHECK(level IN ('hard','soft','love')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            catalog_id INTEGER REFERENCES ingredient_catalog(id) ON DELETE SET NULL,
            UNIQUE(type, value)
        );
        INSERT INTO food_blacklist_new (id, type, value, level, created_at, catalog_id)
            SELECT id, type, value, level, created_at, catalog_id FROM food_blacklist;
        DROP TABLE food_blacklist;
        ALTER TABLE food_blacklist_new RENAME TO food_blacklist;"
    ).ok();
}

/// Classify a blacklist string: category code, tag in catalog, product name, else keyword.
fn classify_blacklist_item(conn: &rusqlite::Connection, item: &str) -> &'static str {
    const CATS: &[&str] = &["meat","fish","veg","fruit","grain","dairy","legumes","nuts","spice","oil","bakery","drinks","other"];
    if CATS.contains(&item) { return "category"; }
    let product_hit: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ingredient_catalog WHERE name = ?1 COLLATE NOCASE",
        rusqlite::params![item], |r| r.get(0),
    ).unwrap_or(0);
    if product_hit > 0 { return "product"; }
    let tag_hit: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ingredient_catalog WHERE (',' || tags || ',') LIKE ?1",
        rusqlite::params![format!("%,{},%", item)], |r| r.get(0),
    ).unwrap_or(0);
    if tag_hit > 0 { return "tag"; }
    "keyword"
}

pub fn migrate_catalog_subgroup(conn: &rusqlite::Connection) {
    let has_col = conn.prepare("SELECT subgroup FROM ingredient_catalog LIMIT 1").is_ok();
    if !has_col {
        let _ = conn.execute("ALTER TABLE ingredient_catalog ADD COLUMN subgroup TEXT", []);
    }

    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='catalog_subgroup_autofill'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);

    // Autogroup: first non-empty tag becomes subgroup for rows with NULL subgroup.
    let mut stmt = match conn.prepare(
        "SELECT id, tags FROM ingredient_catalog WHERE (subgroup IS NULL OR subgroup = '') AND tags != ''"
    ) { Ok(s) => s, Err(_) => return };
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map(|m| m.filter_map(|x| x.ok()).collect())
        .unwrap_or_default();
    for (id, tags) in &rows {
        if let Some(first) = tags.split(',').map(|t| t.trim()).find(|t| !t.is_empty()) {
            let _ = conn.execute(
                "UPDATE ingredient_catalog SET subgroup=?1 WHERE id=?2",
                rusqlite::params![first, id],
            );
        }
    }
    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('catalog_subgroup_autofill')", []);
}

// v0.53: parent_id hierarchy in ingredient_catalog (Stage 1: meat + fish)
pub fn migrate_catalog_parent(conn: &rusqlite::Connection) {
    let has_col = conn.prepare("SELECT parent_id FROM ingredient_catalog LIMIT 1").is_ok();
    if !has_col {
        let _ = conn.execute(
            "ALTER TABLE ingredient_catalog ADD COLUMN parent_id INTEGER REFERENCES ingredient_catalog(id) ON DELETE SET NULL",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_catalog_parent ON ingredient_catalog(parent_id)",
            [],
        );
    }

    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='catalog_parent_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }

    seed_catalog_hierarchy(conn);
    relink_legacy_catalog_parents(conn);

    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('catalog_parent_v1')", []);
}

// Inserts new meat/fish/semifinished items and links them to parents by name.
fn seed_catalog_hierarchy(conn: &rusqlite::Connection) {
    // (name, category, subgroup, tags, parent_name)
    let items: &[(&str, &str, &str, &str, Option<&str>)] = &[
        // New top-level roots
        ("свинина", "meat", "свинина", "свинина", None),
        ("полуфабрикаты мясные", "meat", "полуфабрикаты", "полуфабрикаты", None),

        // Курица children
        ("куриная грудка", "meat", "птица", "птица", Some("курица")),
        ("куриная голень", "meat", "птица", "птица", Some("курица")),
        ("куриный окорочок", "meat", "птица", "птица", Some("курица")),
        ("куриные сердечки", "meat", "субпродукты", "субпродукты,птица", Some("курица")),
        ("куриные желудки", "meat", "субпродукты", "субпродукты,птица", Some("курица")),

        // Говядина children
        ("говяжья вырезка", "meat", "говядина", "говядина", Some("говядина")),
        ("говяжья грудинка", "meat", "говядина", "говядина", Some("говядина")),
        ("говяжья лопатка", "meat", "говядина", "говядина", Some("говядина")),
        ("говяжья голяшка", "meat", "говядина", "говядина", Some("говядина")),
        ("говяжьи рёбра", "meat", "говядина", "говядина", Some("говядина")),
        ("говяжье сердце", "meat", "субпродукты", "субпродукты,говядина", Some("говядина")),
        ("говяжьи почки", "meat", "субпродукты", "субпродукты,говядина", Some("говядина")),

        // Свинина children
        ("свиная вырезка", "meat", "свинина", "свинина", Some("свинина")),
        ("свиная корейка", "meat", "свинина", "свинина", Some("свинина")),
        ("свиная шея", "meat", "свинина", "свинина", Some("свинина")),
        ("свиные рёбра", "meat", "свинина", "свинина", Some("свинина")),
        ("свиная грудинка", "meat", "свинина", "свинина", Some("свинина")),
        ("свиная лопатка", "meat", "свинина", "свинина", Some("свинина")),
        ("фарш свиной", "meat", "свинина", "свинина,фарш", Some("свинина")),
        ("сало", "meat", "свинина", "свинина,сало", Some("свинина")),
        ("свиная печень", "meat", "субпродукты", "субпродукты,свинина", Some("свинина")),

        // Баранина children
        ("баранья лопатка", "meat", "баранина", "баранина", Some("баранина")),
        ("бараньи рёбрышки", "meat", "баранина", "баранина", Some("баранина")),
        ("баранья корейка", "meat", "баранина", "баранина", Some("баранина")),
        ("баранья нога", "meat", "баранина", "баранина", Some("баранина")),
        ("баранья голяшка", "meat", "баранина", "баранина", Some("баранина")),
        ("фарш бараний", "meat", "баранина", "баранина,фарш", Some("баранина")),
        ("баранья печень", "meat", "субпродукты", "субпродукты,баранина", Some("баранина")),

        // Полуфабрикаты children
        ("пельмени", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("манты", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("вареники мясные", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("котлеты", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("тефтели", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("бургер-патти", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("наггетсы куриные", "meat", "полуфабрикаты", "полуфабрикаты,птица", Some("полуфабрикаты мясные")),
        ("чебуреки", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("хинкали", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("купаты", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),
        ("шашлык маринованный", "meat", "полуфабрикаты", "полуфабрикаты", Some("полуфабрикаты мясные")),

        // Fish — red fish breakdown
        ("лосось филе", "fish", "красная рыба", "красная рыба", Some("лосось")),
        ("лосось стейк", "fish", "красная рыба", "красная рыба", Some("лосось")),
        ("лосось слабосолёный", "fish", "красная рыба", "красная рыба", Some("лосось")),
        ("сёмга филе", "fish", "красная рыба", "красная рыба", Some("сёмга")),
        ("сёмга стейк", "fish", "красная рыба", "красная рыба", Some("сёмга")),
        ("форель филе", "fish", "красная рыба", "красная рыба", Some("форель")),
        ("форель радужная", "fish", "красная рыба", "красная рыба", Some("форель")),

        // Fish — white fish breakdown
        ("треска филе", "fish", "белая рыба", "белая рыба", Some("треска")),
        ("треска стейк", "fish", "белая рыба", "белая рыба", Some("треска")),
        ("минтай филе", "fish", "белая рыба", "белая рыба", Some("минтай")),
        ("тунец стейк", "fish", "белая рыба", "белая рыба", Some("тунец")),
        ("тунец консервированный", "fish", "белая рыба", "белая рыба,консервы", Some("тунец")),

        // Fish — seafood variants
        ("креветки тигровые", "fish", "морепродукты", "морепродукты", Some("креветки")),
        ("креветки королевские", "fish", "морепродукты", "морепродукты", Some("креветки")),
        ("креветки коктейльные", "fish", "морепродукты", "морепродукты", Some("креветки")),
        ("креветки очищенные", "fish", "морепродукты", "морепродукты", Some("креветки")),
        ("кальмар тушка", "fish", "морепродукты", "морепродукты", Some("кальмар")),
        ("кальмар кольца", "fish", "морепродукты", "морепродукты", Some("кальмар")),
        ("кальмар филе", "fish", "морепродукты", "морепродукты", Some("кальмар")),
        ("мидии в раковинах", "fish", "морепродукты", "морепродукты", Some("мидии")),
        ("мидии очищенные", "fish", "морепродукты", "морепродукты", Some("мидии")),
    ];

    // Pass 1: insert all rows (parents auto-created if absent; children with parent_id=NULL initially)
    for (name, cat, sg, tags, _parent) in items {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO ingredient_catalog (name, category, tags, subgroup) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, cat, tags, sg],
        );
    }

    // Pass 2: resolve parent_id by name lookup. Restrict to the same category so a user's
    // pre-existing row with the same name in a different category isn't silently re-parented.
    for (name, cat, _sg, _tags, parent) in items {
        if let Some(parent_name) = parent {
            let _ = conn.execute(
                "UPDATE ingredient_catalog \
                 SET parent_id = (SELECT id FROM ingredient_catalog WHERE name=?1 COLLATE NOCASE AND category=?3) \
                 WHERE name=?2 COLLATE NOCASE AND category=?3 AND parent_id IS NULL",
                rusqlite::params![parent_name, name, cat],
            );
        }
    }
}

// Trim + Unicode-aware lowercase. SQLite's built-in LOWER() is ASCII-only,
// so all name normalization is done in Rust.
pub fn normalize_name(s: &str) -> String { s.trim().to_lowercase() }

// Unicode-aware cascade rename for legacy rows (catalog_id IS NULL) where SQLite COLLATE NOCASE
// can't fold Cyrillic. Scans rows in Rust and updates only those whose normalized name matches.
pub fn rename_legacy_by_name(
    conn: &rusqlite::Connection,
    table: &str,
    name_col: &str,
    old_name: &str,
    new_name: &str,
    extra_where: &str,
) {
    let target = normalize_name(old_name);
    if target.is_empty() { return; }
    let select_sql = format!(
        "SELECT id, {} FROM {} WHERE catalog_id IS NULL{}",
        name_col, table,
        if extra_where.is_empty() { "".to_string() } else { format!(" AND {}", extra_where) },
    );
    let rows: Vec<(i64, String)> = match conn.prepare(&select_sql) {
        Ok(mut stmt) => stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map(|m| m.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(_) => return,
    };
    let update_sql = format!("UPDATE {} SET {}=?1 WHERE id=?2", table, name_col);
    for (row_id, raw_name) in rows {
        if normalize_name(&raw_name) == target {
            let _ = conn.execute(&update_sql, rusqlite::params![new_name, row_id]);
        }
    }
}

// Look up a catalog row by name with Unicode-aware case-insensitive comparison.
pub fn resolve_catalog_id_by_name(conn: &rusqlite::Connection, name: &str) -> Option<i64> {
    let target = normalize_name(name);
    if target.is_empty() { return None; }
    let mut stmt = conn.prepare("SELECT id, name FROM ingredient_catalog").ok()?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).ok()?;
    for row in rows.flatten() {
        if normalize_name(&row.1) == target { return Some(row.0); }
    }
    None
}

// v0.54: catalog_id soft-link in products / recipe_ingredients / food_blacklist.
// Existing rows are auto-linked via Unicode-aware strict equality on trimmed lowercase names.
pub fn migrate_catalog_links(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT catalog_id FROM products LIMIT 1").is_err() {
        let _ = conn.execute(
            "ALTER TABLE products ADD COLUMN catalog_id INTEGER REFERENCES ingredient_catalog(id) ON DELETE SET NULL",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_products_catalog ON products(catalog_id)",
            [],
        );
    }
    if conn.prepare("SELECT catalog_id FROM recipe_ingredients LIMIT 1").is_err() {
        let _ = conn.execute(
            "ALTER TABLE recipe_ingredients ADD COLUMN catalog_id INTEGER REFERENCES ingredient_catalog(id) ON DELETE SET NULL",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recipe_ingredients_catalog ON recipe_ingredients(catalog_id)",
            [],
        );
    }
    if conn.prepare("SELECT catalog_id FROM food_blacklist LIMIT 1").is_err() {
        let _ = conn.execute(
            "ALTER TABLE food_blacklist ADD COLUMN catalog_id INTEGER REFERENCES ingredient_catalog(id) ON DELETE SET NULL",
            [],
        );
    }

    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    // v2 supersedes the broken v1 (which used SQLite LOWER() that doesn't fold Cyrillic).
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='catalog_link_v2'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }

    let mut catalog: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id, name FROM ingredient_catalog") {
        if let Ok(iter) = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))) {
            for row in iter.flatten() {
                catalog.entry(normalize_name(&row.1)).or_insert(row.0);
            }
        }
    }
    if !catalog.is_empty() {
        backfill_catalog_id(conn, "products", "name", &catalog, "catalog_id IS NULL");
        backfill_catalog_id(conn, "recipe_ingredients", "name", &catalog, "catalog_id IS NULL");
        backfill_catalog_id(conn, "food_blacklist", "value", &catalog, "catalog_id IS NULL AND type='product'");
    }

    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('catalog_link_v2')", []);
}

fn backfill_catalog_id(
    conn: &rusqlite::Connection,
    table: &str,
    name_col: &str,
    catalog: &std::collections::HashMap<String, i64>,
    where_clause: &str,
) {
    let select_sql = format!("SELECT id, {} FROM {} WHERE {}", name_col, table, where_clause);
    let rows: Vec<(i64, String)> = match conn.prepare(&select_sql) {
        Ok(mut stmt) => stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map(|m| m.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(_) => return,
    };
    let update_sql = format!("UPDATE {} SET catalog_id=?1 WHERE id=?2", table);
    for (row_id, raw_name) in rows {
        if let Some(cid) = catalog.get(&normalize_name(&raw_name)) {
            let _ = conn.execute(&update_sql, rusqlite::params![cid, row_id]);
        }
    }
}

// Re-parents existing flat catalog entries (forshmaks, organs of курица/говядина) under their species.
fn relink_legacy_catalog_parents(conn: &rusqlite::Connection) {
    let pairs: &[(&str, &str)] = &[
        // Курица
        ("куриное филе", "курица"),
        ("куриные бёдра", "курица"),
        ("куриные крылышки", "курица"),
        ("фарш куриный", "курица"),
        ("печень куриная", "курица"),
        // Говядина
        ("фарш говяжий", "говядина"),
        ("печень говяжья", "говядина"),
        ("язык говяжий", "говядина"),
    ];
    for (child, parent) in pairs {
        let _ = conn.execute(
            "UPDATE ingredient_catalog \
             SET parent_id = (SELECT id FROM ingredient_catalog WHERE name=?1 COLLATE NOCASE) \
             WHERE name=?2 COLLATE NOCASE AND parent_id IS NULL",
            rusqlite::params![parent, child],
        );
    }
    // Tag legacy organ names with subgroup='субпродукты' for cleaner UI grouping.
    let _ = conn.execute(
        "UPDATE ingredient_catalog SET subgroup='субпродукты' \
         WHERE name IN ('печень куриная','печень говяжья','язык говяжий') \
         AND (subgroup IS NULL OR subgroup='' OR subgroup<>'субпродукты')",
        [],
    );
}

pub fn migrate_sports_catalog(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS exercise_catalog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            muscle_group TEXT NOT NULL DEFAULT 'full_body',
            equipment TEXT NOT NULL DEFAULT '',
            type TEXT NOT NULL DEFAULT 'strength',
            description TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS workout_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            type TEXT NOT NULL DEFAULT 'gym',
            difficulty TEXT NOT NULL DEFAULT 'easy',
            target_muscle_groups TEXT NOT NULL DEFAULT '',
            favorite INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS template_exercises (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            template_id INTEGER NOT NULL,
            exercise_catalog_id INTEGER,
            name TEXT NOT NULL,
            sets INTEGER DEFAULT 3,
            reps INTEGER DEFAULT 10,
            weight_kg REAL DEFAULT 0,
            duration_seconds INTEGER DEFAULT 0,
            rest_seconds INTEGER DEFAULT 60,
            order_index INTEGER DEFAULT 0,
            FOREIGN KEY (template_id) REFERENCES workout_templates(id) ON DELETE CASCADE,
            FOREIGN KEY (exercise_catalog_id) REFERENCES exercise_catalog(id)
        );"
    ).ok();
    // Add template_id FK to existing workouts table
    conn.execute("ALTER TABLE workouts ADD COLUMN template_id INTEGER", []).ok();
}

pub fn migrate_share_links(conn: &rusqlite::Connection) {
    // v0.41: public share links exposed via Cloudflare Tunnel
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS share_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            token TEXT NOT NULL UNIQUE,
            tab TEXT NOT NULL,
            scope TEXT NOT NULL DEFAULT 'all',
            permissions TEXT NOT NULL DEFAULT '[\"view\"]',
            label TEXT NOT NULL DEFAULT '',
            lifetime TEXT NOT NULL DEFAULT 'permanent',
            expires_at TEXT,
            used_count INTEGER NOT NULL DEFAULT 0,
            revoked_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_share_token ON share_links(token);

        CREATE TABLE IF NOT EXISTS share_activity (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            link_id INTEGER NOT NULL,
            action TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '',
            guest_ip TEXT NOT NULL DEFAULT '',
            user_agent TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            FOREIGN KEY (link_id) REFERENCES share_links(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_share_activity_link ON share_activity(link_id);

        CREATE TABLE IF NOT EXISTS share_comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            link_id INTEGER NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id INTEGER NOT NULL,
            author TEXT NOT NULL DEFAULT 'Guest',
            text TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (link_id) REFERENCES share_links(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_share_comments_entity ON share_comments(entity_type, entity_id);"
    ).ok();
}

/// Returns the column names of `table` from `PRAGMA table_info`. Returns
/// Err if the table doesn't exist (caller decides whether to skip).
pub fn table_columns_in(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| format!("table_info {}: {}", table, e))?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| format!("query: {}", e))?;
    let mut out = Vec::new();
    for r in rows { out.push(r.map_err(|e| e.to_string())?); }
    if out.is_empty() {
        return Err(format!("table {} not found", table));
    }
    Ok(out)
}

pub fn migrate_priority(conn: &rusqlite::Connection) {
    // Importance/priority for tasks (notes with status='task') and calendar events.
    // 0 = none, 1..5 from green to red (low → critical).
    conn.execute("ALTER TABLE notes ADD COLUMN priority INTEGER NOT NULL DEFAULT 0", []).ok();
    conn.execute("ALTER TABLE events ADD COLUMN priority INTEGER NOT NULL DEFAULT 0", []).ok();
}

pub fn migrate_schedule_priority(conn: &rusqlite::Connection) {
    // Same 0..5 importance scale as migrate_priority, extended to schedules so the
    // task picker can rank recurring tasks alongside events/notes.
    conn.execute("ALTER TABLE schedules ADD COLUMN priority INTEGER NOT NULL DEFAULT 0", []).ok();
}

pub fn migrate_task_pins(conn: &rusqlite::Connection) {
    // Manually pinned tasks in the "Запустить таск" picker. Local-only (not CRR);
    // keyed by the (source_type, source_id) the picker already uses.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS task_pins (
            source_type TEXT NOT NULL,
            source_id INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (source_type, source_id)
        );"
    ).ok();
}

pub fn migrate_event_categories(conn: &rusqlite::Connection) {
    // User-managed list of calendar event categories. Seeded once with sensible
    // defaults; users can rename/recolor/delete from the UI.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS event_categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            color TEXT NOT NULL DEFAULT '#9B9B9B',
            icon TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT ''
        );"
    ).ok();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM event_categories", [], |r| r.get(0)
    ).unwrap_or(0);
    if count == 0 {
        let now = chrono::Local::now().to_rfc3339();
        let seed: &[(&str, &str, &str, i64)] = &[
            ("general",  "#9B9B9B", "",   0),
            ("Работа",   "#2383e2", "💼", 1),
            ("Личное",   "#9065b0", "🏠", 2),
            ("Здоровье", "#448361", "💚", 3),
            ("Спорт",    "#d9730d", "🏋", 4),
            ("Еда",      "#cb8a05", "🍽", 5),
            ("Учёба",    "#c14c8a", "📚", 6),
        ];
        for (name, color, icon, ord) in seed {
            conn.execute(
                "INSERT OR IGNORE INTO event_categories (name, color, icon, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![name, color, icon, ord, now],
            ).ok();
        }
    }
}

/// Stage D — schema prep for snapshot-based owner sync.
///
/// 1. Adds `updated_at` to tables that didn't have it (events, transactions,
///    body_records, conversations) and an AFTER UPDATE trigger that keeps it
///    fresh. LWW conflict resolution needs a per-row timestamp.
/// 2. Creates `sync_tombstones (table_name, row_id, deleted_at)` plus
///    AFTER DELETE triggers on the 7 sync targets so deletes are observable
///    without touching the existing delete handlers.
/// 3. Generates a stable `device_id` UUID stored in app_settings.
/// Tables synced by Stage D owner-sync. Every entry must:
///   - have an INTEGER `id` PK,
///   - own a stable `created_at` (or analogous) text column to backfill from,
///   - be `id`-addressable (no composite PKs).
/// TEXT PK / composite PK tables (page_meta, ui_state, custom_pages, note_tags,
/// tab_page_blocks) are excluded — they're config-shaped and rarely diverge.
pub const SYNC_TABLES: &[&str] = &[
    "facts", "conversations", "activities", "notes", "events",
    "projects", "tasks", "learning_items", "hobbies", "hobby_entries",
    "workouts", "exercises", "health_log", "habits", "habit_checks",
    "media_items", "user_lists", "list_items", "food_log", "recipes",
    "products", "transactions", "budgets", "savings_goals",
    "subscriptions", "debts", "blocklist", "tab_goals", "home_items",
    "contacts", "contact_blocks", "property_definitions",
    "property_values", "view_configs", "activity_snapshots",
    "proactive_history", "message_feedback", "conversation_insights",
    "reminders", "flywheel_cycles", "schedules", "schedule_completions",
    "dan_koe_entries", "proactive_messages", "project_records",
    "body_records", "job_sources", "job_roles", "job_vacancies",
    "job_search_log", "dashboard_widgets", "timeline_activity_types",
    "timeline_blocks", "timeline_goals", "sleep_sessions", "sleep_stages",
    "heart_rate_samples", "event_categories",
];

pub fn migrate_sync_meta(conn: &rusqlite::Connection) {
    // 0. Heal divergent installs that shipped earlier init_db without the
    // projects/tasks tables (e.g. Android v0.73.x). Idempotent for any host
    // that already has them.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
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
        );"
    ).ok();

    // 1. Add `updated_at TEXT NOT NULL DEFAULT ''` everywhere it's missing.
    // SQLite forbids non-constant DEFAULTs in ALTER ADD, hence the empty
    // string sentinel + backfill loop below.
    for table in SYNC_TABLES {
        let sql = format!(
            "ALTER TABLE {table} ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''"
        );
        conn.execute(&sql, []).ok();
    }

    // 2. Backfill existing rows. We try the most-likely timestamp columns
    // in order and fall back to now(). Trying a non-existent column would
    // raise SQL error, so probe each table's schema first.
    let candidates = ["created_at", "started_at", "date", "logged_at"];
    for table in SYNC_TABLES {
        let cols = match table_columns_in(conn, table) {
            Ok(c) => c,
            Err(_) => continue, // table may not exist on this install
        };
        let mut coalesce_args = Vec::<String>::new();
        for col in &candidates {
            if cols.iter().any(|c| c == *col) {
                coalesce_args.push(format!("NULLIF({col}, '')"));
            }
        }
        coalesce_args.push("datetime('now')".into());
        let sql = format!(
            "UPDATE {table} SET updated_at = COALESCE({}) \
             WHERE updated_at = '' OR updated_at IS NULL",
            coalesce_args.join(", ")
        );
        conn.execute(&sql, []).ok();
    }

    // updated_at / deleted_at must string-compare against the owner-sync
    // cursor, which holds chrono RFC3339 values. SQLite datetime('now') yields
    // a space-separated UTC form ("2026-05-19 01:02:03") that sorts *below*
    // RFC3339 ("...T...") — push silently skipped every trigger-stamped row.
    // Use a 'T'-separated local form so both paths order consistently.
    let ts_expr = "strftime('%Y-%m-%dT%H:%M:%f','now','localtime')";

    // 3. AFTER INSERT triggers — set updated_at for fresh rows when the
    // INSERT didn't supply one. Avoids NULL/'' rows breaking LWW.
    // DROP first: CREATE ... IF NOT EXISTS won't refresh an old-format trigger.
    for table in SYNC_TABLES {
        let trig = format!(
            "DROP TRIGGER IF EXISTS {table}_set_updated_at_on_insert; \
             CREATE TRIGGER {table}_set_updated_at_on_insert \
             AFTER INSERT ON {table} \
             FOR EACH ROW \
             WHEN NEW.updated_at IS NULL OR NEW.updated_at = '' \
             BEGIN \
                 UPDATE {table} SET updated_at = {ts_expr} WHERE rowid = NEW.rowid; \
             END"
        );
        conn.execute_batch(&trig).ok();
    }

    // 4. AFTER UPDATE triggers — bump updated_at on every row mutation. Skip
    // when the new updated_at differs from old (caller already set it, e.g.
    // sync_owner pulling remote rows with a remote timestamp).
    for table in SYNC_TABLES {
        let trig = format!(
            "DROP TRIGGER IF EXISTS {table}_bump_updated_at; \
             CREATE TRIGGER {table}_bump_updated_at \
             AFTER UPDATE ON {table} \
             FOR EACH ROW \
             WHEN NEW.updated_at = OLD.updated_at \
             BEGIN \
                 UPDATE {table} SET updated_at = {ts_expr} WHERE rowid = NEW.rowid; \
             END"
        );
        conn.execute_batch(&trig).ok();
    }

    // 3. Tombstones table + AFTER DELETE triggers
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_tombstones (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            table_name TEXT NOT NULL,
            row_id INTEGER NOT NULL,
            deleted_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(table_name, row_id)
        );
        CREATE INDEX IF NOT EXISTS idx_sync_tombstones_deleted_at
            ON sync_tombstones(deleted_at);"
    ).ok();
    for table in SYNC_TABLES {
        let trig = format!(
            "DROP TRIGGER IF EXISTS {table}_tombstone; \
             CREATE TRIGGER {table}_tombstone \
             AFTER DELETE ON {table} \
             FOR EACH ROW \
             BEGIN \
                 INSERT OR REPLACE INTO sync_tombstones (table_name, row_id, deleted_at) \
                 VALUES ('{table}', OLD.id, {ts_expr}); \
             END"
        );
        conn.execute_batch(&trig).ok();
    }

    // 4. Stable device_id (used by sync to skip echoes from this device)
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM app_settings WHERE key='device_id'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if exists == 0 {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('device_id', ?1)",
            rusqlite::params![id],
        );
    }
}

/// One-time cleanup: an earlier import_exercise inserted a fresh health_log row
/// on every Health Connect sync, so identical exercises piled up — and
/// sync_health_to_timeline then turned each into its own timeline_block.
/// Collapse both tables to one row per distinct entry. Idempotent: a second run
/// finds no duplicates and deletes nothing.
pub fn migrate_dedup_health_exercise(conn: &rusqlite::Connection) {
    conn.execute(
        "DELETE FROM health_log WHERE type='exercise' AND id NOT IN (
            SELECT MIN(id) FROM health_log WHERE type='exercise'
            GROUP BY date, type, value, notes
        )",
        [],
    ).ok();
    conn.execute(
        "DELETE FROM timeline_blocks WHERE source='auto_health' AND id NOT IN (
            SELECT MIN(id) FROM timeline_blocks WHERE source='auto_health'
            GROUP BY date, start_time, end_time, type_id, source
        )",
        [],
    ).ok();
}

/// One-time-per-launch cleanup: an earlier sync_health_to_calendar did
/// DELETE+INSERT on every poll, so LAN-sync ended up with stale tombstones
/// and the receiver accumulated duplicate Sleep/Exercise events. Collapse
/// to one row per (date, title, time, duration_minutes) on both phone and
/// Mac. Idempotent.
pub fn migrate_dedup_auto_health_events(conn: &rusqlite::Connection) {
    conn.execute(
        "DELETE FROM events WHERE source='auto_health' AND id NOT IN (
            SELECT MIN(id) FROM events WHERE source='auto_health'
            GROUP BY date, title, time, duration_minutes
        )",
        [],
    ).ok();
}

/// Shopping list — items the user adds from fridge / freely to buy next time.
/// Used by the "🛒 Закупка" event template (multi-select picker fills the
/// event description with selected items and marks them bought_at on save).
pub fn migrate_shopping_list(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT id FROM shopping_list LIMIT 1").is_err() {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS shopping_list (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                qty TEXT NOT NULL DEFAULT '',
                note TEXT NOT NULL DEFAULT '',
                added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                bought_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_shopping_list_open
                ON shopping_list(bought_at) WHERE bought_at IS NULL;"
        ).ok();
    }
}
