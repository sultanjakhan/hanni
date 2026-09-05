// db.rs — Database initialization, migrations, auto-backup
use serde::Deserialize;
use std::collections::HashMap;
use crate::types::hanni_data_dir;
use crate::secure_fs;
use chrono;
use rusqlite::OptionalExtension;

/// Migrate data from old ~/Documents/Hanni/ to ~/Library/Application Support/Hanni/
#[cfg(not(target_os = "android"))]
pub fn migrate_old_data_dir() -> Result<(), String> {
    let new_dir = hanni_data_dir();
    secure_fs::ensure_private_dir(&new_dir)
        .map_err(|e| format!("secure data directory: {e}"))?;
    let marker = new_dir.join(".migrated");
    if crate::types::is_isolated_dev() {
        // An explicitly isolated debug session must never import legacy user data.
        if !marker.exists() {
            std::fs::write(&marker, "isolated-dev: legacy import disabled\n")
                .map_err(|_| "Cannot mark isolated dev migration boundary")?;
            secure_fs::restrict_file(&marker)
                .map_err(|_| "Cannot secure isolated dev migration marker")?;
        }
        return Ok(());
    }
    if marker.exists() { return Ok(()); } // already migrated — skip without touching ~/Documents
    let old_dir = dirs::home_dir().unwrap_or_default().join("Documents/Hanni");
    if !old_dir.exists() {
        // No old data, create marker so we never check ~/Documents again
        std::fs::write(&marker, "migrated")
            .map_err(|e| format!("write migration marker: {e}"))?;
        return Ok(());
    }
    let old_db = old_dir.join("hanni.db");
    let new_db = new_dir.join("hanni.db");
    // Never overwrite a destination produced by an earlier partial migration.
    match std::fs::symlink_metadata(&old_db) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "legacy database symlink requires manual review: {}",
                old_db.display()
            ));
        }
        Ok(metadata) if metadata.is_file() => {
            if destination_is_absent(&new_db)
                .map_err(|e| format!("inspect migrated database destination: {e}"))?
            {
                std::fs::copy(&old_db, &new_db)
                    .map_err(|e| format!("copy legacy database: {e}"))?;
            }
        }
        Ok(_) => return Err(format!("legacy database is not a file: {}", old_db.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("inspect legacy database: {error}")),
    }
    // Copy other files (settings, audio, etc.)
    let entries = std::fs::read_dir(&old_dir)
        .map_err(|e| format!("read legacy data directory: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read legacy data entry: {e}"))?;
        if entry.file_name() == "hanni.db" { continue; } // already handled
        let dest = new_dir.join(entry.file_name());
        let file_type = entry.file_type()
            .map_err(|e| format!("inspect legacy entry {}: {e}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "legacy symlink requires manual review: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            // Retry/merge a directory left partial by an earlier failed run.
            copy_dir_recursive(&entry.path(), &dest)
                .map_err(|e| format!("copy legacy directory {}: {e}", entry.path().display()))?;
        } else if file_type.is_file() && destination_is_absent(&dest)
            .map_err(|e| format!("inspect legacy destination {}: {e}", dest.display()))?
        {
            std::fs::copy(entry.path(), &dest)
                .map_err(|e| format!("copy legacy file {}: {e}", entry.path().display()))?;
        }
    }
    std::fs::write(&marker, "migrated")
        .map_err(|e| format!("write migration marker: {e}"))?;
    eprintln!("Migrated data from {:?} to {:?}", old_dir, new_dir);
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(dst) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("legacy destination is not a real directory: {}", dst.display()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(dst)?;
        }
        Err(error) => return Err(error),
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("legacy symlink requires manual review: {}", entry.path().display()),
            ));
        }
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else if file_type.is_file() && destination_is_absent(&dest)? {
            std::fs::copy(&entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn destination_is_absent(path: &std::path::Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("legacy destination is a symlink: {}", path.display()),
        )),
        Ok(metadata) if metadata.is_file() => Ok(false),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("legacy file destination has the wrong type: {}", path.display()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

pub fn restrict_file(path: &std::path::Path) -> std::io::Result<()> {
    secure_fs::restrict_file(path)
}

pub fn restrict_dir(path: &std::path::Path) -> std::io::Result<()> {
    secure_fs::restrict_dir(path)
}

#[derive(Debug)]
pub enum BackupError {
    Data(String),
    Security(String),
}

impl BackupError {
    pub fn is_security(&self) -> bool {
        matches!(self, Self::Security(_))
    }
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data(message) | Self::Security(message) => f.write_str(message),
        }
    }
}

/// Create a timestamped backup of hanni.db, keep last 5
pub fn backup_db() -> Result<(), BackupError> {
    let data_dir = hanni_data_dir();
    let db_path = data_dir.join("hanni.db");
    if !db_path.exists() { return Ok(()); }
    let backup_dir = data_dir.join("backups");
    secure_fs::ensure_private_dir(&backup_dir)
        .map_err(|e| BackupError::Security(format!("secure backup directory: {e}")))?;
    // Throttle to at most one backup per day. Copying the (ever-growing) DB on
    // every launch sat on the Android cold-start hot path for little value.
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let prefix = format!("hanni_{}_", today);
    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| BackupError::Data(format!("scan backup directory: {e}")))?
    {
        let entry = entry
            .map_err(|e| BackupError::Data(format!("read backup directory entry: {e}")))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".db") {
            return Ok(());
        }
    }
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let dest = backup_dir.join(format!("hanni_{}.db", ts));
    std::fs::copy(&db_path, &dest).map_err(|error| {
        let _ = std::fs::remove_file(&dest);
        BackupError::Data(format!("copy database backup: {error}"))
    })?;
    if let Err(error) = restrict_file(&dest) {
        let cleanup = std::fs::remove_file(&dest)
            .err()
            .map(|e| format!("; cleanup failed: {e}"))
            .unwrap_or_default();
        return Err(BackupError::Security(format!(
            "secure database backup: {error}{cleanup}"
        )));
    }
    // Also copy WAL if present
    let wal = data_dir.join("hanni.db-wal");
    if wal.exists() {
        let wal_dest = backup_dir.join(format!("hanni_{}.db-wal", ts));
        if let Err(error) = std::fs::copy(&wal, &wal_dest) {
            let _ = std::fs::remove_file(&wal_dest);
            let _ = std::fs::remove_file(&dest);
            return Err(BackupError::Data(format!("copy WAL backup: {error}")));
        }
        if let Err(error) = restrict_file(&wal_dest) {
            let wal_cleanup = std::fs::remove_file(&wal_dest).err();
            let db_cleanup = std::fs::remove_file(&dest).err();
            let cleanup = match (wal_cleanup, db_cleanup) {
                (None, None) => String::new(),
                (wal, db) => format!("; cleanup failed: wal={wal:?}, db={db:?}"),
            };
            return Err(BackupError::Security(format!(
                "secure WAL backup: {error}{cleanup}"
            )));
        }
    }
    // Keep only last 5 backups
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| BackupError::Data(format!("scan backup retention directory: {e}")))?
    {
        let entry = entry
            .map_err(|e| BackupError::Data(format!("read backup retention entry: {e}")))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("hanni_") && name.ends_with(".db") {
            backups.push(entry);
        }
    }
    backups.sort_by_key(|e| e.file_name());
    while backups.len() > 5 {
        let old = backups.remove(0);
        std::fs::remove_file(old.path())
            .map_err(|e| BackupError::Data(format!("remove old database backup: {e}")))?;
        // Remove matching WAL
        let wal_path = old.path().with_extension("db-wal");
        if let Err(error) = std::fs::remove_file(wal_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(BackupError::Data(format!("remove old WAL backup: {error}")));
            }
        }
    }
    eprintln!("DB backup: {}", dest.display());
    Ok(())
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
            id TEXT PRIMARY KEY,
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
            id TEXT PRIMARY KEY,
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
            id TEXT PRIMARY KEY,
            schedule_id TEXT NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
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

/// v0.92: extra schedule columns added AFTER migrate_schedules_to_uuid_pk
/// (which recreates the table from a fixed column set and would otherwise drop
/// columns added earlier in migrate_schedules):
///   • auto_source  — links a schedule to a real data source so its daily
///     completion fills automatically ('steps'/'sleep'/'walking'/'cooking'…).
///   • visible_from — "HH:MM"; when set, the schedule is hidden from the
///     tasker (Список + picker) on the current day until that time, so evening
///     items don't clutter the morning. NULL/'' = always visible.
pub fn migrate_schedule_auto_source(conn: &rusqlite::Connection) {
    conn.execute("ALTER TABLE schedules ADD COLUMN auto_source TEXT", []).ok();
    conn.execute("ALTER TABLE schedules ADD COLUMN visible_from TEXT", []).ok();
}

/// Per-chain time trigger. trigger_time = "HH:MM" or a comma-list "09:00,12:00,18:00"
/// (one entry per launch slot) — drives the "due now" highlight and per-slot launch.
pub fn migrate_routine_chain_trigger_time(conn: &rusqlite::Connection) {
    conn.execute("ALTER TABLE routine_chains ADD COLUMN trigger_time TEXT", []).ok();
}

/// chain_only schedules live ONLY inside a routine run — hidden from the flat
/// tasker (Список / picker / day-view) so chain steps don't double as loose tasks.
pub fn migrate_schedule_chain_only(conn: &rusqlite::Connection) {
    conn.execute("ALTER TABLE schedules ADD COLUMN chain_only INTEGER NOT NULL DEFAULT 0", []).ok();
}

/// Allow a chain to run several times a day (breakfast/lunch/dinner): add a
/// `slot` to routine_runs and key uniqueness on (chain_id, date, slot) instead
/// of (chain_id, date). SQLite can't drop an inline UNIQUE, so rebuild the table
/// (ids preserved → routine_node_status FK stays valid). Idempotent.
pub fn migrate_routine_run_slots(conn: &rusqlite::Connection) {
    let has_slot: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('routine_runs') WHERE name='slot'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if has_slot > 0 { return; }
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='routine_runs'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if exists == 0 { return; }
    // PRAGMA foreign_keys is a no-op inside a transaction → toggle outside.
    let _ = conn.execute("PRAGMA foreign_keys=OFF", []);
    let _ = conn.execute_batch(
        "BEGIN;
         CREATE TABLE routine_runs_new (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             chain_id INTEGER NOT NULL REFERENCES routine_chains(id) ON DELETE CASCADE,
             date TEXT NOT NULL,
             slot TEXT NOT NULL DEFAULT '',
             state TEXT NOT NULL DEFAULT 'active',
             started_at TEXT NOT NULL DEFAULT (datetime('now')),
             completed_at TEXT,
             UNIQUE(chain_id, date, slot)
         );
         INSERT INTO routine_runs_new (id, chain_id, date, slot, state, started_at, completed_at)
             SELECT id, chain_id, date, '', state, started_at, completed_at FROM routine_runs;
         DROP TABLE routine_runs;
         ALTER TABLE routine_runs_new RENAME TO routine_runs;
         COMMIT;"
    );
    let _ = conn.execute("PRAGMA foreign_keys=ON", []);
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
    seed_reflection_routine(conn);
    seed_night_routine(conn);
    seed_meal_routine(conn);
    seed_workout_routine(conn);
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
    let chain_id = crate::types::deterministic_id("chain:Утро");
    if conn.execute(
        "INSERT INTO routine_chains (id, title, trigger_type, sort_order) VALUES (?1, 'Утро', 'sleep_end', 0)",
        rusqlite::params![chain_id],
    ).is_ok() {
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
            let nid = crate::types::deterministic_id(&format!("node:c{}:{}", chain_id, title));
            conn.execute(
                "INSERT INTO routine_nodes
                 (id, chain_id, source_type, title, category, priority, requirement, pos_x, pos_y, is_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![nid, chain_id, stype, title, cat, pri, req, x, y, is_start],
            ).ok();
            ids.insert(key, nid);
        }
        let edges = [
            ("start","up"), ("up","bed"), ("bed","toil"), ("bed","wash"),
            ("bed","teeth"), ("toil","vit"), ("wash","vit"), ("teeth","vit"), ("vit","exer"),
        ];
        for (from, to) in edges {
            let eid = crate::types::deterministic_id(&format!("edge:c{}:{}>{}", chain_id, ids[from], ids[to]));
            conn.execute(
                "INSERT INTO routine_edges (id, chain_id, from_node_id, to_node_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![eid, chain_id, ids[from], ids[to]],
            ).ok();
        }
    }
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_morning_seed_v2')", []).ok();
}

/// Seed the "Reflection" graph: an evening checklist of `challenge` habits +
/// `growth` outcomes + Dan Koe practices. All nodes are autonomous and optional —
/// the user marks each done or skipped depending on what actually happened today.
/// All edges fan out from start (no inter-node order).
fn seed_reflection_routine(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_reflection_seed_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let chain_id = crate::types::deterministic_id("chain:Рефлексия");
    if conn.execute(
        "INSERT INTO routine_chains (id, title, trigger_type, sort_order) VALUES (?1, 'Рефлексия', 'manual', 10)",
        rusqlite::params![chain_id],
    ).is_ok() {
        // (key, title, category, pri, x, y, is_start)
        let nodes: [(&str, &str, &str, i32, i32, i32, i32); 32] = [
            ("start", "Подведу день",                    "other",     0, 30,  400, 1),
            // Column 1 (x=200) — сладкое
            ("sw1",   "Без сладкого",                    "challenge", 4, 200, 40,  0),
            ("sw2",   "Без выпечки",                     "challenge", 3, 200, 120, 0),
            ("sw3",   "Без шоколада",                    "challenge", 3, 200, 200, 0),
            ("sw4",   "Без мороженого",                  "challenge", 3, 200, 280, 0),
            ("sw5",   "Без печенья",                     "challenge", 3, 200, 360, 0),
            ("sw6",   "Без конфет",                      "challenge", 3, 200, 440, 0),
            ("sw7",   "Без сахара в чай/кофе",           "challenge", 3, 200, 520, 0),
            // Column 2 (x=420) — напитки + еда
            ("dr1",   "Без газировки",                   "challenge", 3, 420, 40,  0),
            ("dr2",   "Без энергетиков",                 "challenge", 3, 420, 120, 0),
            ("fd1",   "Без фастфуда",                    "challenge", 3, 420, 200, 0),
            ("fd2",   "Без чипсов/снеков",               "challenge", 3, 420, 280, 0),
            ("fd3",   "Не ел перед сном",                "challenge", 3, 420, 360, 0),
            ("fd4",   "Не переедал",                     "challenge", 3, 420, 440, 0),
            ("fd5",   "Готовил сам",                     "challenge", 3, 420, 520, 0),
            // Column 3 (x=640) — экраны/цифровое
            ("dg1",   "Без YouTube Shorts/TikTok",       "challenge", 4, 640, 40,  0),
            ("dg2",   "Без соцсетей",                    "challenge", 4, 640, 120, 0),
            ("dg3",   "Без порно",                       "challenge", 4, 640, 200, 0),
            ("dg4",   "Без мастурбации",                 "challenge", 4, 640, 280, 0),
            ("dg5",   "Без телефона перед сном",         "challenge", 4, 640, 360, 0),
            ("dg6",   "Телефон < 1ч в день",             "challenge", 4, 640, 440, 0),
            ("dg7",   "Не играл в комп игры",            "challenge", 3, 640, 520, 0),
            ("dg8",   "Не одевал наушники просто так",   "challenge", 2, 640, 600, 0),
            // Column 4 (x=860) — здоровье/growth/Dan Koe
            ("hl1",   "Перерыв от экрана каждый час",    "challenge", 3, 860, 40,  0),
            ("hl2",   "Следил за осанкой",               "challenge", 3, 860, 120, 0),
            ("gr1",   "Изучил что-то новое",             "growth",    4, 860, 200, 0),
            ("gr2",   "Научил/объяснил другому",         "growth",    3, 860, 280, 0),
            ("gr3",   "Получил фидбек и осмыслил",       "growth",    3, 860, 360, 0),
            ("gr4",   "Применил новый навык",            "growth",    3, 860, 440, 0),
            ("dk1",   "Contemplation (Dan Koe)",         "practice",  4, 860, 520, 0),
            ("dk2",   "Vision (Dan Koe)",                "practice",  4, 860, 600, 0),
            ("dk3",   "Integration (Dan Koe)",           "practice",  4, 860, 680, 0),
        ];
        let mut ids = std::collections::HashMap::new();
        for (key, title, cat, pri, x, y, is_start) in nodes {
            let stype = if is_start == 1 { "start" } else { "schedule" };
            let req = if is_start == 1 { "required" } else { "optional" };
            let nid = crate::types::deterministic_id(&format!("node:c{}:{}", chain_id, title));
            conn.execute(
                "INSERT INTO routine_nodes
                 (id, chain_id, source_type, title, category, priority, requirement, pos_x, pos_y, is_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![nid, chain_id, stype, title, cat, pri, req, x, y, is_start],
            ).ok();
            ids.insert(key, nid);
        }
        // Fan-out: every non-start node has an edge from start.
        for (key, _, _, _, _, _, is_start) in nodes {
            if is_start == 1 { continue; }
            let eid = crate::types::deterministic_id(&format!("edge:c{}:{}>{}", chain_id, ids["start"], ids[key]));
            conn.execute(
                "INSERT INTO routine_edges (id, chain_id, from_node_id, to_node_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![eid, chain_id, ids["start"], ids[key]],
            ).ok();
        }
    }
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_reflection_seed_v1')", []).ok();
}

/// Seed the "Night" graph: linear wind-down before sleep.
fn seed_night_routine(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_night_seed_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let chain_id = crate::types::deterministic_id("chain:Ночь");
    if conn.execute(
        "INSERT INTO routine_chains (id, title, trigger_type, sort_order) VALUES (?1, 'Ночь', 'manual', 20)",
        rusqlite::params![chain_id],
    ).is_ok() {
        let nodes = [
            ("start",  "Готовлюсь ко сну",            "other",     0, "required", 30,  220, 1),
            ("shower", "Душ",                          "hygiene",   4, "required", 200, 220, 0),
            ("teeth",  "Зубы",                         "hygiene",   5, "required", 370, 220, 0),
            ("clothes","Одежда на завтра",             "home",      2, "optional", 540, 120, 0),
            ("phone",  "Убрать телефон с тумбочки",    "challenge", 4, "required", 540, 320, 0),
            ("read",   "Книга/подкаст 15 мин",         "growth",    2, "optional", 710, 220, 0),
            ("bed",    "Лёг в кровать",                "other",     3, "required", 880, 220, 0),
        ];
        let mut ids = std::collections::HashMap::new();
        for (key, title, cat, pri, req, x, y, is_start) in nodes {
            let stype = if is_start == 1 { "start" } else { "schedule" };
            let nid = crate::types::deterministic_id(&format!("node:c{}:{}", chain_id, title));
            conn.execute(
                "INSERT INTO routine_nodes
                 (id, chain_id, source_type, title, category, priority, requirement, pos_x, pos_y, is_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![nid, chain_id, stype, title, cat, pri, req, x, y, is_start],
            ).ok();
            ids.insert(key, nid);
        }
        let edges = [
            ("start","shower"), ("shower","teeth"),
            ("teeth","clothes"), ("teeth","phone"),
            ("clothes","read"), ("phone","read"),
            ("read","bed"),
        ];
        for (from, to) in edges {
            let eid = crate::types::deterministic_id(&format!("edge:c{}:{}>{}", chain_id, ids[from], ids[to]));
            conn.execute(
                "INSERT INTO routine_edges (id, chain_id, from_node_id, to_node_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![eid, chain_id, ids[from], ids[to]],
            ).ok();
        }
    }
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_night_seed_v1')", []).ok();
}

/// Seed the "Meal" graph: one eat-cycle per day (UNIQUE(chain_id, date) on runs).
fn seed_meal_routine(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_meal_seed_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let chain_id = crate::types::deterministic_id("chain:Покушать");
    if conn.execute(
        "INSERT INTO routine_chains (id, title, trigger_type, sort_order) VALUES (?1, 'Покушать', 'manual', 30)",
        rusqlite::params![chain_id],
    ).is_ok() {
        let nodes = [
            ("start",  "Время поесть",            "other",   0, "required", 30,  220, 1),
            ("hands",  "Помыл руки",              "hygiene", 4, "required", 200, 220, 0),
            ("cook",   "Выбрал/приготовил блюдо", "other",   3, "required", 370, 220, 0),
            ("eat",    "Поел без телефона",       "health",  4, "required", 540, 220, 0),
            ("dishes", "Помыл посуду",            "home",    3, "required", 710, 220, 0),
            ("log",    "Записал в food log",      "health",  2, "optional", 880, 220, 0),
        ];
        let mut ids = std::collections::HashMap::new();
        for (key, title, cat, pri, req, x, y, is_start) in nodes {
            let stype = if is_start == 1 { "start" } else { "schedule" };
            let nid = crate::types::deterministic_id(&format!("node:c{}:{}", chain_id, title));
            conn.execute(
                "INSERT INTO routine_nodes
                 (id, chain_id, source_type, title, category, priority, requirement, pos_x, pos_y, is_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![nid, chain_id, stype, title, cat, pri, req, x, y, is_start],
            ).ok();
            ids.insert(key, nid);
        }
        let edges = [
            ("start","hands"), ("hands","cook"), ("cook","eat"),
            ("eat","dishes"), ("dishes","log"),
        ];
        for (from, to) in edges {
            let eid = crate::types::deterministic_id(&format!("edge:c{}:{}>{}", chain_id, ids[from], ids[to]));
            conn.execute(
                "INSERT INTO routine_edges (id, chain_id, from_node_id, to_node_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![eid, chain_id, ids[from], ids[to]],
            ).ok();
        }
    }
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_meal_seed_v1')", []).ok();
}

/// Seed the "Workout" graph: every node (except start) is optional, so the user
/// can complete just a warm-up or just a stretch and still finish the run.
fn seed_workout_routine(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_workout_seed_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let chain_id = crate::types::deterministic_id("chain:Спорт");
    if conn.execute(
        "INSERT INTO routine_chains (id, title, trigger_type, sort_order) VALUES (?1, 'Спорт', 'manual', 40)",
        rusqlite::params![chain_id],
    ).is_ok() {
        let nodes = [
            ("start",   "На тренировку",          "other",   0, "required", 30,  220, 1),
            ("muscle",  "Выбрал группу мышц",     "sport",   3, "optional", 200, 220, 0),
            ("warm",    "Разминка",               "sport",   3, "optional", 370, 220, 0),
            ("main",    "Силовая тренировка",     "sport",   4, "optional", 540, 220, 0),
            ("stretch", "Растяжка",               "sport",   3, "optional", 710, 220, 0),
            ("shower",  "Душ",                    "hygiene", 2, "optional", 880, 220, 0),
        ];
        let mut ids = std::collections::HashMap::new();
        for (key, title, cat, pri, req, x, y, is_start) in nodes {
            let stype = if is_start == 1 { "start" } else { "schedule" };
            let nid = crate::types::deterministic_id(&format!("node:c{}:{}", chain_id, title));
            conn.execute(
                "INSERT INTO routine_nodes
                 (id, chain_id, source_type, title, category, priority, requirement, pos_x, pos_y, is_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![nid, chain_id, stype, title, cat, pri, req, x, y, is_start],
            ).ok();
            ids.insert(key, nid);
        }
        let edges = [
            ("start","muscle"), ("muscle","warm"), ("warm","main"),
            ("main","stretch"), ("stretch","shower"),
        ];
        for (from, to) in edges {
            let eid = crate::types::deterministic_id(&format!("edge:c{}:{}>{}", chain_id, ids[from], ids[to]));
            conn.execute(
                "INSERT INTO routine_edges (id, chain_id, from_node_id, to_node_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![eid, chain_id, ids[from], ids[to]],
            ).ok();
        }
    }
    conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_workout_seed_v1')", []).ok();
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

// Normalise " shared-by:" (space) → ",shared-by:" (comma). The old axum
// create_recipe joined existing tags with the auto-injected author tag
// using a space, so the UI's split(",") rendered it as a single bogus chip.
// Idempotent — REPLACE is a no-op when there's no space variant left.
pub fn migrate_recipe_tags_separator(conn: &rusqlite::Connection) {
    let _ = conn.execute(
        "UPDATE recipes SET tags = REPLACE(tags, ' shared-by:', ',shared-by:') \
         WHERE tags LIKE '% shared-by:%'",
        [],
    );
}

pub fn migrate_sleep(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sleep_sessions (
            id TEXT PRIMARY KEY,
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
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sleep_sessions(id) ON DELETE CASCADE,
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
            id TEXT PRIMARY KEY,
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

// v0.92: richer exercise catalog (difficulty + structured equipment/muscles)
// and a one-time seed from the bundled public-domain dataset.
pub fn migrate_sports_catalog_v2(conn: &rusqlite::Connection) {
    if conn.prepare("SELECT difficulty FROM exercise_catalog LIMIT 1").is_err() {
        let _ = conn.execute("ALTER TABLE exercise_catalog ADD COLUMN difficulty TEXT NOT NULL DEFAULT 'medium'", []);
        let _ = conn.execute("ALTER TABLE exercise_catalog ADD COLUMN primary_muscles TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE exercise_catalog ADD COLUMN secondary_muscles TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE exercise_catalog ADD COLUMN category TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE exercise_catalog ADD COLUMN force TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE exercise_catalog ADD COLUMN images TEXT NOT NULL DEFAULT ''", []);
    }
    crate::sports_seed::seed_exercise_catalog(conn);
}

// v0.93: multi-day workout programs (monthly / split / muscle-focus / warmup).
// A program references existing workout_templates per day; a run tracks progress.
pub fn migrate_workout_programs(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS workout_programs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'custom',
            cycle_length_days INTEGER NOT NULL DEFAULT 7,
            duration_weeks INTEGER NOT NULL DEFAULT 0,
            target_muscle_groups TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            favorite INTEGER NOT NULL DEFAULT 0,
            active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS program_days (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            program_id INTEGER NOT NULL,
            day_index INTEGER NOT NULL DEFAULT 0,
            label TEXT NOT NULL DEFAULT '',
            template_id INTEGER,
            is_rest INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            order_index INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (program_id) REFERENCES workout_programs(id) ON DELETE CASCADE,
            FOREIGN KEY (template_id) REFERENCES workout_templates(id)
        );
        CREATE TABLE IF NOT EXISTS program_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            program_id INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            current_day INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active',
            completed_days INTEGER NOT NULL DEFAULT 0,
            finished_at TEXT,
            FOREIGN KEY (program_id) REFERENCES workout_programs(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_program_days_program ON program_days(program_id, day_index, order_index);
        CREATE INDEX IF NOT EXISTS idx_program_runs_active ON program_runs(program_id, status);"
    ).ok();
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

const AUTOMATION_LOG_SCRUB_KEY: &str = "automation_log_metadata_v1";
const AUTOMATION_LOG_SCRUB_PENDING: &str = "pending";
const AUTOMATION_LOG_SCRUB_COMPLETE: &str = "complete";

#[derive(Debug, PartialEq, Eq)]
struct SqliteColumnSpec {
    name: String,
    declared_type: String,
    not_null: bool,
    default_value: Option<String>,
    primary_key: i64,
    hidden: i64,
}

fn column_spec(
    name: &str,
    declared_type: &str,
    not_null: bool,
    default_value: Option<&str>,
    primary_key: i64,
) -> SqliteColumnSpec {
    SqliteColumnSpec {
        name: name.to_string(),
        declared_type: declared_type.to_string(),
        not_null,
        default_value: default_value.map(str::to_string),
        primary_key,
        hidden: 0,
    }
}

fn automation_log_metadata_schema() -> Vec<SqliteColumnSpec> {
    vec![
        column_spec("id", "INTEGER", false, None, 1),
        column_spec("ts", "INTEGER", true, None, 0),
        column_spec("script_hash", "TEXT", true, None, 0),
        column_spec("success", "INTEGER", true, None, 0),
        column_spec("duration_ms", "INTEGER", true, Some("0"), 0),
    ]
}

fn automation_log_legacy_schema() -> Vec<SqliteColumnSpec> {
    vec![
        column_spec("id", "INTEGER", false, None, 1),
        column_spec("ts", "INTEGER", true, None, 0),
        column_spec("script_hash", "TEXT", true, None, 0),
        column_spec("script_preview", "TEXT", true, Some("''"), 0),
        column_spec("success", "INTEGER", true, None, 0),
        column_spec("duration_ms", "INTEGER", true, Some("0"), 0),
    ]
}

fn table_xinfo_in(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<Vec<SqliteColumnSpec>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_xinfo({table})"))
        .map_err(|error| format!("table_xinfo {table}: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SqliteColumnSpec {
                name: row.get(1)?,
                declared_type: row.get::<_, String>(2)?.to_ascii_uppercase(),
                not_null: row.get::<_, i64>(3)? != 0,
                default_value: row.get(4)?,
                primary_key: row.get(5)?,
                hidden: row.get(6)?,
            })
        })
        .map_err(|error| format!("query table_xinfo {table}: {error}"))?;
    let mut schema = Vec::new();
    for row in rows {
        schema.push(row.map_err(|error| format!("decode table_xinfo {table}: {error}"))?);
    }
    Ok(schema)
}

fn table_exists_in(conn: &rusqlite::Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
    .map_err(|error| format!("inspect table {table}: {error}"))
}

fn set_automation_log_scrub_state(
    conn: &rusqlite::Connection,
    state: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO _hanni_security_migrations(name, state) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET state=excluded.state",
        rusqlite::params![AUTOMATION_LOG_SCRUB_KEY, state],
    )
    .map_err(|error| format!("record automation log scrub state: {error}"))?;
    Ok(())
}

fn complete_automation_log_scrub(conn: &rusqlite::Connection) -> Result<(), String> {
    crate::secret_store::checkpoint_truncate(conn, "before automation log scrub")?;
    conn.execute_batch("VACUUM;")
        .map_err(|error| format!("scrub historical automation log pages: {error}"))?;
    set_automation_log_scrub_state(conn, AUTOMATION_LOG_SCRUB_COMPLETE)?;
    crate::secret_store::checkpoint_truncate(conn, "after automation log scrub")?;
    Ok(())
}

/// Replace the legacy automation audit log with a metadata-only schema and
/// physically scrub old script previews from SQLite pages and WAL sidecars.
///
/// This migration is deliberately independent of `PRAGMA user_version`: an
/// already-v10 database can still contain the legacy preview column. The
/// durable pending marker also makes a crash after the table rebuild but
/// before VACUUM resume the physical scrub on the next startup.
pub fn migrate_automation_log(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.pragma_update(None, "secure_delete", "ON")
        .map_err(|error| format!("enable automation log secure delete: {error}"))?;

    let tx = rusqlite::Transaction::new_unchecked(
        conn,
        rusqlite::TransactionBehavior::Immediate,
    )
    .map_err(|error| format!("begin automation log migration: {error}"))?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS _hanni_security_migrations (
            name TEXT PRIMARY KEY,
            state TEXT NOT NULL CHECK(state IN ('pending', 'complete'))
         );",
    )
    .map_err(|error| format!("create security migration state: {error}"))?;

    if table_exists_in(&tx, "_automation_log_metadata_v1")? {
        return Err("automation log staging table already exists".into());
    }

    if !table_exists_in(&tx, "automation_log")? {
        tx.execute_batch(
            "CREATE TABLE automation_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                script_hash TEXT NOT NULL,
                success INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_automation_log_ts ON automation_log(ts);",
        )
        .map_err(|error| format!("create metadata-only automation log: {error}"))?;
        // Absence does not prove that an older table never existed: dropped
        // pages may remain in the database or WAL. Persist pending and perform
        // the same physical scrub as a legacy-table rebuild.
        set_automation_log_scrub_state(&tx, AUTOMATION_LOG_SCRUB_PENDING)?;
        tx.commit()
            .map_err(|error| format!("commit fresh automation log schema: {error}"))?;
        return complete_automation_log_scrub(conn);
    }

    let schema = table_xinfo_in(&tx, "automation_log")?;
    let metadata_schema = automation_log_metadata_schema();
    let legacy_schema = automation_log_legacy_schema();
    let marker: Option<String> = tx
        .query_row(
            "SELECT state FROM _hanni_security_migrations WHERE name=?1",
            [AUTOMATION_LOG_SCRUB_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("read automation log scrub state: {error}"))?;

    let scrub_required = if schema == legacy_schema {
        set_automation_log_scrub_state(&tx, AUTOMATION_LOG_SCRUB_PENDING)?;
        let source_count: i64 = tx
            .query_row("SELECT COUNT(*) FROM automation_log", [], |row| row.get(0))
            .map_err(|error| format!("count legacy automation rows: {error}"))?;
        tx.execute_batch(
            "CREATE TABLE _automation_log_metadata_v1 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                script_hash TEXT NOT NULL,
                success INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO _automation_log_metadata_v1
                (id, ts, script_hash, success, duration_ms)
             SELECT id, ts, script_hash, success, duration_ms
             FROM automation_log;",
        )
        .map_err(|error| format!("copy automation metadata: {error}"))?;
        let copied_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM _automation_log_metadata_v1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("verify copied automation rows: {error}"))?;
        if copied_count != source_count {
            return Err(format!(
                "automation metadata row count mismatch: expected {source_count}, copied {copied_count}"
            ));
        }
        tx.execute_batch(
            "DROP TABLE automation_log;
             ALTER TABLE _automation_log_metadata_v1 RENAME TO automation_log;
             CREATE INDEX idx_automation_log_ts ON automation_log(ts);",
        )
        .map_err(|error| format!("publish metadata-only automation log: {error}"))?;
        true
    } else if schema == metadata_schema {
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_automation_log_ts ON automation_log(ts);",
        )
        .map_err(|error| format!("repair automation log index: {error}"))?;
        if marker.as_deref() == Some(AUTOMATION_LOG_SCRUB_COMPLETE) {
            false
        } else {
            set_automation_log_scrub_state(&tx, AUTOMATION_LOG_SCRUB_PENDING)?;
            true
        }
    } else {
        return Err(
            "unexpected automation_log schema; expected exact legacy or metadata-only shape"
                .into(),
        );
    };

    tx.commit()
        .map_err(|error| format!("commit automation log migration: {error}"))?;
    if !scrub_required {
        // A previous run can crash after writing `complete` but before its
        // final checkpoint. Re-verify/truncate on the completed path so that
        // the durable marker never turns a prior busy checkpoint into a skip.
        return crate::secret_store::checkpoint_truncate(
            conn,
            "verify completed automation log scrub",
        );
    }

    complete_automation_log_scrub(conn)
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

pub fn migrate_event_linked_tab(conn: &rusqlite::Connection) {
    // Optional link from a calendar event to a Hanni tab (food, sports, …).
    // Empty string = no link. Mirrors the notes.tab_name pattern.
    conn.execute("ALTER TABLE events ADD COLUMN linked_tab TEXT NOT NULL DEFAULT ''", []).ok();
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
///    AFTER DELETE triggers on the sync targets so deletes are observable
///    without touching the existing delete handlers.
/// 3. Generates a stable `device_id` UUID stored in app_settings.
/// Tables synced by Stage D owner-sync. Every entry must:
///   - have a stable INTEGER or TEXT `id` PK,
///   - own a stable `created_at` (or analogous) text column to backfill from,
///   - be `id`-addressable (no composite PKs).
/// Composite-PK/config tables (page_meta, ui_state, custom_pages, note_tags,
/// tab_page_blocks) remain excluded.
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
    // Routine graph + run state. ORDER IS A FK CONTRACT: push/apply walk this
    // array in order, so a parent must precede its children — chains → nodes →
    // edges → runs → node_status (node_status FKs both runs and nodes; edges FK
    // nodes; runs FK chains). migrate_routine_ids_deterministic_v2 content-keys
    // the whole graph (chain=title, node=chain|title, edge=chain|from|to) so the
    // same logical row gets the same id on every device — pulling chains/nodes/
    // edges then CONVERGES instead of duplicating, which is what previously
    // forced graph rows out of sync and made node_status pulls fail the FK on a
    // missing node.
    "routine_chains", "routine_nodes", "routine_edges",
    "routine_runs", "routine_node_status",
];

/// Whether `table.column` is declared TEXT in the current schema. Used
/// by UUID migrations (Phase 1+) so they're idempotent — re-running on
/// an already-migrated DB is a no-op.
pub fn column_is_text(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        &format!("SELECT type FROM pragma_table_info('{}') WHERE name=?1", table),
        rusqlite::params![column],
        |r| r.get::<_, String>(0),
    ).map(|t| t.to_uppercase().contains("TEXT")).unwrap_or(false)
}

pub(crate) const SYNC_HLC_GENERATION_MARKER: &str = "sync_hlc_protocol_v1";

fn sync_hlc_millis_ceil(raw: &str) -> Result<i64, String> {
    let raw = raw.trim();
    let timestamp = if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(raw) {
        timestamp.with_timezone(&chrono::Utc)
    } else {
        let mut parsed = None;
        for format in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
            if let Ok(timestamp) = chrono::NaiveDateTime::parse_from_str(raw, format) {
                parsed = Some(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                    timestamp,
                    chrono::Utc,
                ));
                break;
            }
        }
        match parsed {
            Some(timestamp) => timestamp,
            None => {
                let date = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                    .map_err(|_| "sync HLC timestamp has an unsupported format".to_string())?;
                let timestamp = date
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| "sync HLC timestamp has an invalid date".to_string())?;
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                    timestamp,
                    chrono::Utc,
                )
            }
        }
    };
    if timestamp.timestamp() < 0 {
        return Err("sync HLC timestamp predates the Unix epoch".into());
    }
    let whole_millis = timestamp
        .timestamp()
        .checked_mul(1_000)
        .ok_or_else(|| "sync HLC timestamp overflows milliseconds".to_string())?;
    let fractional_millis =
        (i64::from(timestamp.timestamp_subsec_nanos()) + 999_999) / 1_000_000;
    whole_millis
        .checked_add(fractional_millis)
        .ok_or_else(|| "sync HLC timestamp overflows milliseconds".to_string())
}

pub(crate) fn observe_sync_hlc_timestamp(
    conn: &rusqlite::Connection,
    timestamp: &str,
) -> Result<(), String> {
    let millis = sync_hlc_millis_ceil(timestamp)?;
    let changed = conn
        .execute(
            "UPDATE sync_hlc_state
             SET last_millis=MAX(last_millis,?1)
             WHERE singleton=1",
            [millis],
        )
        .map_err(|error| format!("observe sync HLC timestamp: {error}"))?;
    if changed != 1 {
        return Err("sync HLC state row is missing".into());
    }
    Ok(())
}

fn sync_outbound_timestamp_cursor_keys() -> Vec<String> {
    let mut timestamp_keys = Vec::new();
    for table in SYNC_TABLES {
        for prefix in ["cloud_owner_v2_push_", "cloud_owner_gh_push_"] {
            timestamp_keys.push(format!("{prefix}{table}"));
        }
    }
    for prefix in ["cloud_owner_v2_push_tombstones", "cloud_owner_gh_push_tombstones"] {
        timestamp_keys.push(prefix.to_string());
    }
    timestamp_keys
}

fn sync_outbound_cursor_keys() -> Vec<String> {
    let mut cursor_keys = Vec::new();
    for timestamp_key in sync_outbound_timestamp_cursor_keys() {
        cursor_keys.push(timestamp_key.clone());
        if timestamp_key.ends_with("tombstones") {
            cursor_keys.push(format!("{timestamp_key}_table"));
            cursor_keys.push(format!("{timestamp_key}_row_id"));
        } else {
            cursor_keys.push(format!("{timestamp_key}_id"));
        }
    }
    cursor_keys
}

fn seed_sync_hlc_state(conn: &rusqlite::Connection) -> Result<(), String> {
    for table in SYNC_TABLES {
        if table_columns_in(conn, table).is_err() {
            continue;
        }
        let candidate = conn.query_row(
            &format!(
                "SELECT updated_at FROM {table}
                 WHERE julianday(updated_at) IS NOT NULL
                 ORDER BY julianday(updated_at) DESC, updated_at DESC
                 LIMIT 1"
            ),
            [],
            |row| row.get::<_, String>(0),
        );
        match candidate {
            Ok(timestamp) => observe_sync_hlc_timestamp(conn, &timestamp)?,
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(error) => return Err(format!("seed sync HLC from {table}: {error}")),
        }
    }
    let tombstone = conn.query_row(
        "SELECT deleted_at FROM sync_tombstones
         WHERE julianday(deleted_at) IS NOT NULL
         ORDER BY julianday(deleted_at) DESC, deleted_at DESC
         LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    );
    match tombstone {
        Ok(timestamp) => observe_sync_hlc_timestamp(conn, &timestamp)?,
        Err(rusqlite::Error::QueryReturnedNoRows) => {}
        Err(error) => return Err(format!("seed sync HLC from tombstones: {error}")),
    }

    for key in sync_outbound_timestamp_cursor_keys() {
        let value = conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [&key],
            |row| row.get::<_, String>(0),
        );
        match value {
            Ok(timestamp) if sync_hlc_millis_ceil(&timestamp).is_ok() => {
                observe_sync_hlc_timestamp(conn, &timestamp)?;
            }
            Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(error) => return Err(format!("seed sync HLC from cursor {key}: {error}")),
        }
    }
    Ok(())
}

fn install_sync_hlc_protocol(conn: &rusqlite::Connection) -> Result<(), String> {
    const SAVEPOINT: &str = "hanni_sync_hlc_protocol";
    conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(|error| format!("start sync HLC migration: {error}"))?;
    let result = (|| -> Result<(), String> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_hlc_state (
                 singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1),
                 last_millis INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO sync_hlc_state(singleton,last_millis) VALUES(1,0);
             CREATE TABLE IF NOT EXISTS sync_apply_context (
                 singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1),
                 remote_apply INTEGER NOT NULL CHECK(remote_apply IN (0,1)),
                 stamp_depth INTEGER NOT NULL CHECK(stamp_depth >= 0)
             );
             INSERT OR IGNORE INTO sync_apply_context(singleton,remote_apply,stamp_depth)
                 VALUES(1,0,0);
             UPDATE sync_apply_context SET remote_apply=0,stamp_depth=0 WHERE singleton=1;",
        )
        .map_err(|error| format!("create sync HLC metadata: {error}"))?;
        seed_sync_hlc_state(conn)?;

        // HLC stamps updated_at with an internal UPDATE. Limit external-content
        // FTS maintenance to indexed business columns so the internal stamp
        // cannot run an *_au delete before the sibling *_ai insert.
        if sync_schema_object_exists(conn, "table", "facts_fts")? {
            conn.execute_batch(
                "DROP TRIGGER IF EXISTS facts_au;
                 CREATE TRIGGER facts_au AFTER UPDATE OF category,key,value ON facts BEGIN
                     INSERT INTO facts_fts(facts_fts,rowid,category,key,value)
                     VALUES('delete',old.id,old.category,old.key,old.value);
                     INSERT INTO facts_fts(rowid,category,key,value)
                     VALUES(new.id,new.category,new.key,new.value);
                 END;",
            )
            .map_err(|error| format!("bind facts FTS to HLC updates: {error}"))?;
        }
        if sync_schema_object_exists(conn, "table", "conversations_fts")? {
            conn.execute_batch(
                "DROP TRIGGER IF EXISTS conv_au;
                 CREATE TRIGGER conv_au AFTER UPDATE OF summary,messages ON conversations BEGIN
                     INSERT INTO conversations_fts(conversations_fts,rowid,summary,messages)
                     VALUES('delete',old.id,COALESCE(old.summary,''),old.messages);
                     INSERT INTO conversations_fts(rowid,summary,messages)
                     VALUES(new.id,COALESCE(new.summary,''),new.messages);
                 END;",
            )
            .map_err(|error| format!("bind conversations FTS to HLC updates: {error}"))?;
        }
        if sync_schema_object_exists(conn, "table", "notes_fts")? {
            conn.execute_batch(
                "DROP TRIGGER IF EXISTS notes_au;
                 CREATE TRIGGER notes_au AFTER UPDATE OF title,content,tags ON notes BEGIN
                     INSERT INTO notes_fts(notes_fts,rowid,title,content,tags)
                     VALUES('delete',old.id,old.title,old.content,old.tags);
                     INSERT INTO notes_fts(rowid,title,content,tags)
                     VALUES(new.id,new.title,new.content,new.tags);
                 END;",
            )
            .map_err(|error| format!("bind notes FTS to HLC updates: {error}"))?;
        }

        let wall_millis = "(CAST(strftime('%s','now') AS INTEGER)*1000 + \
             CAST(substr(strftime('%f','now'),4,3) AS INTEGER))";
        let rendered_hlc = "strftime('%Y-%m-%dT%H:%M:%fZ', \
             (SELECT last_millis FROM sync_hlc_state WHERE singleton=1)/1000.0, 'unixepoch')";
        for table in SYNC_TABLES {
            if table_columns_in(conn, table).is_err() {
                continue;
            }
            let tombstone_row_id = if *table == "event_categories" {
                "'name:' || OLD.name"
            } else {
                "CAST(OLD.id AS TEXT)"
            };
            let version_row_id = tombstone_row_id;
            let triggers = format!(
                "DROP TRIGGER IF EXISTS {table}_set_updated_at_on_insert; \
                 CREATE TRIGGER {table}_set_updated_at_on_insert \
                 AFTER INSERT ON {table} FOR EACH ROW \
                 WHEN (SELECT remote_apply=0 AND stamp_depth=0 \
                       FROM sync_apply_context WHERE singleton=1) \
                 BEGIN \
                     UPDATE sync_hlc_state \
                     SET last_millis=MAX(last_millis+1,{wall_millis}) \
                     WHERE singleton=1; \
                     UPDATE sync_apply_context SET stamp_depth=stamp_depth+1 \
                     WHERE singleton=1; \
                     UPDATE {table} SET updated_at={rendered_hlc} WHERE rowid=NEW.rowid; \
                     UPDATE sync_apply_context SET stamp_depth=stamp_depth-1 \
                     WHERE singleton=1; \
                 END; \
                 DROP TRIGGER IF EXISTS {table}_bump_updated_at; \
                 CREATE TRIGGER {table}_bump_updated_at \
                 AFTER UPDATE ON {table} FOR EACH ROW \
                 WHEN (SELECT remote_apply=0 AND stamp_depth=0 \
                       FROM sync_apply_context WHERE singleton=1) \
                 BEGIN \
                     UPDATE sync_hlc_state \
                     SET last_millis=MAX(last_millis+1,{wall_millis}) \
                     WHERE singleton=1; \
                     UPDATE sync_apply_context SET stamp_depth=stamp_depth+1 \
                     WHERE singleton=1; \
                     UPDATE {table} SET updated_at={rendered_hlc} WHERE rowid=NEW.rowid; \
                     UPDATE sync_apply_context SET stamp_depth=stamp_depth-1 \
                     WHERE singleton=1; \
                 END; \
                 DROP TRIGGER IF EXISTS {table}_tombstone; \
                 CREATE TRIGGER {table}_tombstone \
                 AFTER DELETE ON {table} FOR EACH ROW \
                 BEGIN \
                     DELETE FROM sync_row_versions \
                     WHERE table_name='{table}' AND row_id={version_row_id}; \
                     UPDATE sync_hlc_state \
                     SET last_millis=MAX(last_millis+1,{wall_millis}) \
                     WHERE singleton=1 AND \
                         (SELECT remote_apply=0 FROM sync_apply_context WHERE singleton=1); \
                     INSERT OR REPLACE INTO sync_tombstones(table_name,row_id,deleted_at) \
                     SELECT '{table}',{tombstone_row_id},{rendered_hlc} \
                     WHERE (SELECT remote_apply=0 FROM sync_apply_context WHERE singleton=1); \
                 END"
            );
            conn.execute_batch(&triggers)
                .map_err(|error| format!("install sync HLC triggers for {table}: {error}"))?;
        }

        if table_columns_in(conn, "event_categories").is_ok() {
            conn.execute_batch(&format!(
                "DROP TRIGGER IF EXISTS event_categories_name_tombstone; \
                 CREATE TRIGGER event_categories_name_tombstone \
                 AFTER UPDATE OF name ON event_categories FOR EACH ROW \
                 WHEN NEW.name<>OLD.name AND \
                      (SELECT remote_apply=0 AND stamp_depth=0 \
                       FROM sync_apply_context WHERE singleton=1) \
                 BEGIN \
                     DELETE FROM sync_row_versions \
                     WHERE table_name='event_categories' AND row_id='name:' || OLD.name; \
                     UPDATE sync_hlc_state \
                     SET last_millis=MAX(last_millis+1,{wall_millis}) \
                     WHERE singleton=1; \
                     INSERT OR REPLACE INTO sync_tombstones(table_name,row_id,deleted_at) \
                     VALUES('event_categories','name:' || OLD.name,{rendered_hlc}); \
                 END"
            ))
            .map_err(|error| format!("install event category HLC rename trigger: {error}"))?;
        }

        let generation_exists = conn
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key=?1",
                [SYNC_HLC_GENERATION_MARKER],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("read sync HLC generation marker: {error}"))?
            > 0;
        if !generation_exists {
            for key in sync_outbound_cursor_keys() {
                conn.execute("DELETE FROM app_settings WHERE key=?1", [&key])
                    .map_err(|error| format!("reset outbound cursor {key}: {error}"))?;
            }
            conn.execute(
                "INSERT INTO app_settings(key,value) VALUES(?1,'1')",
                [SYNC_HLC_GENERATION_MARKER],
            )
            .map_err(|error| format!("write sync HLC generation marker: {error}"))?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => conn
            .execute_batch(&format!("RELEASE {SAVEPOINT}"))
            .map_err(|error| format!("commit sync HLC migration: {error}")),
        Err(error) => {
            let cleanup = conn.execute_batch(&format!(
                "ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"
            ));
            match cleanup {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(format!(
                    "{error}; rollback sync HLC migration: {cleanup_error}"
                )),
            }
        }
    }
}

pub fn migrate_sync_meta(conn: &rusqlite::Connection) -> Result<(), String> {
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
        );",
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
    // updated_at / deleted_at must string-compare against the owner-sync
    // cursor, which holds chrono RFC3339 values. SQLite datetime('now') yields
    // a space-separated UTC form ("2026-05-19 01:02:03") that sorts *below*
    // RFC3339 ("...T...") — push silently skipped every trigger-stamped row.
    // Emit one canonical UTC form. Sync also normalizes historical space-form
    // and offset timestamps before comparing or advancing a cursor.
    let ts_expr = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";
    let bump_ts_expr = format!(
        "CASE \
             WHEN julianday({ts_expr}) <= julianday(OLD.updated_at) \
             THEN strftime('%Y-%m-%dT%H:%M:%fZ', OLD.updated_at, '+0.001 seconds') \
             ELSE {ts_expr} \
         END"
    );

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
        coalesce_args.push(ts_expr.into());
        let sql = format!(
            "UPDATE {table} SET updated_at = COALESCE({}) \
             WHERE updated_at = '' OR updated_at IS NULL",
            coalesce_args.join(", ")
        );
        conn.execute(&sql, []).ok();
    }

    // The previous trigger emitted a `T`-separated local wall-clock value with
    // no offset. Convert that exact legacy shape using this installation's
    // local timezone before the new UTC trigger/cursors take over. Space-form
    // `datetime('now')` values are already UTC and intentionally stay as-is.
    for table in SYNC_TABLES {
        let sql = format!(
            "UPDATE {table}
             SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', updated_at, 'utc')
             WHERE length(updated_at) >= 19
               AND substr(updated_at, 11, 1) = 'T'
               AND upper(substr(updated_at, -1, 1)) <> 'Z'
               AND instr(substr(updated_at, 20), '+') = 0
               AND instr(substr(updated_at, 20), '-') = 0
               AND julianday(updated_at) IS NOT NULL"
        );
        conn.execute(&sql, []).ok();
    }

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
                 UPDATE {table} SET updated_at = {bump_ts_expr} WHERE rowid = NEW.rowid; \
             END"
        );
        conn.execute_batch(&trig).ok();
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_row_versions (
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            device_id TEXT NOT NULL,
            PRIMARY KEY(table_name, row_id)
        );",
    )
    .ok();

    // A locally inserted or edited row belongs to this device, not to the
    // remote writer recorded by the previous pull. Pull apply records the new
    // remote version after these triggers finish.
    for table in SYNC_TABLES {
        let old_row_id = if *table == "event_categories" {
            "'name:' || OLD.name"
        } else {
            "CAST(OLD.id AS TEXT)"
        };
        let new_row_id = if *table == "event_categories" {
            "'name:' || NEW.name"
        } else {
            "CAST(NEW.id AS TEXT)"
        };
        let triggers = format!(
            "DROP TRIGGER IF EXISTS {table}_clear_sync_version_on_insert; \
             CREATE TRIGGER {table}_clear_sync_version_on_insert \
             AFTER INSERT ON {table} FOR EACH ROW \
             BEGIN \
                 DELETE FROM sync_row_versions \
                 WHERE table_name='{table}' AND row_id={new_row_id}; \
             END; \
             DROP TRIGGER IF EXISTS {table}_clear_sync_version_on_update; \
             CREATE TRIGGER {table}_clear_sync_version_on_update \
             AFTER UPDATE ON {table} FOR EACH ROW \
             BEGIN \
                 DELETE FROM sync_row_versions \
                 WHERE table_name='{table}' \
                   AND row_id IN ({old_row_id}, {new_row_id}); \
             END"
        );
        conn.execute_batch(&triggers).ok();
    }

    // 3. Tombstones table + AFTER DELETE triggers
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_row_versions (
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            device_id TEXT NOT NULL,
            PRIMARY KEY(table_name, row_id)
        );
        CREATE TABLE IF NOT EXISTS sync_tombstones (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            deleted_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(table_name, row_id)
        );
        CREATE INDEX IF NOT EXISTS idx_sync_tombstones_deleted_at
            ON sync_tombstones(deleted_at);",
    ).ok();
    // Migrate row_id from INTEGER to TEXT for installs that shipped the
    // old schema. SQLite stores values per their declared affinity, so an
    // INTEGER column comparing against a UUID parameter (TEXT) would try
    // to coerce the UUID to 0 and silently match the wrong tombstone.
    if !column_is_text(conn, "sync_tombstones", "row_id") {
        conn.execute_batch(
            "ALTER TABLE sync_tombstones RENAME TO sync_tombstones_legacy_int;
             CREATE TABLE sync_tombstones (
                 id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                 table_name TEXT NOT NULL,
                 row_id TEXT NOT NULL,
                 deleted_at TEXT NOT NULL DEFAULT (datetime('now')),
                 UNIQUE(table_name, row_id)
             );
             INSERT INTO sync_tombstones (table_name, row_id, deleted_at)
                 SELECT table_name, CAST(row_id AS TEXT), deleted_at
                 FROM sync_tombstones_legacy_int;
             DROP TABLE sync_tombstones_legacy_int;
             CREATE INDEX IF NOT EXISTS idx_sync_tombstones_deleted_at
                 ON sync_tombstones(deleted_at);",
        ).ok();
    }
    conn.execute(
        "UPDATE sync_tombstones
         SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at, 'utc')
         WHERE length(deleted_at) >= 19
           AND substr(deleted_at, 11, 1) = 'T'
           AND upper(substr(deleted_at, -1, 1)) <> 'Z'
           AND instr(substr(deleted_at, 20), '+') = 0
           AND instr(substr(deleted_at, 20), '-') = 0
           AND julianday(deleted_at) IS NOT NULL",
        [],
    )
    .ok();
    for table in SYNC_TABLES {
        // event_categories converges by UNIQUE name, not by its device-local
        // AUTOINCREMENT id. Its tombstone must use the same logical identity;
        // a numeric id can point at a different category on another device.
        let tombstone_row_id = if *table == "event_categories" {
            "'name:' || OLD.name"
        } else {
            "OLD.id"
        };
        let version_row_id = if *table == "event_categories" {
            "'name:' || OLD.name"
        } else {
            "CAST(OLD.id AS TEXT)"
        };
        let trig = format!(
            "DROP TRIGGER IF EXISTS {table}_tombstone; \
             CREATE TRIGGER {table}_tombstone \
             AFTER DELETE ON {table} \
             FOR EACH ROW \
             BEGIN \
                 DELETE FROM sync_row_versions \
                 WHERE table_name='{table}' AND row_id={version_row_id}; \
                 INSERT OR REPLACE INTO sync_tombstones (table_name, row_id, deleted_at) \
                 VALUES ('{table}', {tombstone_row_id}, {ts_expr}); \
             END"
        );
        conn.execute_batch(&trig).ok();
    }
    // event_categories converges by logical name. A local rename is both an
    // upsert of the new name and a deletion of the old name on every peer.
    conn.execute_batch(&format!(
        "DROP TRIGGER IF EXISTS event_categories_name_tombstone; \
         CREATE TRIGGER event_categories_name_tombstone \
         AFTER UPDATE OF name ON event_categories \
         FOR EACH ROW WHEN NEW.name <> OLD.name \
         BEGIN \
             DELETE FROM sync_row_versions \
             WHERE table_name='event_categories' AND row_id='name:' || OLD.name; \
             INSERT OR REPLACE INTO sync_tombstones (table_name, row_id, deleted_at) \
             VALUES ('event_categories', 'name:' || OLD.name, {ts_expr}); \
         END"
    ))
    .ok();

    install_sync_hlc_protocol(conn)?;

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
    Ok(())
}

fn sync_schema_object_exists(
    conn: &rusqlite::Connection,
    object_type: &str,
    name: &str,
) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2
         )",
        rusqlite::params![object_type, name],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|error| format!("inspect sync schema object {name}: {error}"))
}

fn sync_schema_columns(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<std::collections::HashMap<String, (String, i64)>, String> {
    if !table
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!("invalid sync schema table name: {table}"));
    }
    let mut statement = conn
        .prepare(&format!(
            "SELECT name,type,pk FROM pragma_table_info('{table}')"
        ))
        .map_err(|error| format!("inspect sync table {table}: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| format!("query sync table {table}: {error}"))?;
    let mut columns = std::collections::HashMap::new();
    for row in rows {
        let (name, column_type, primary_key) =
            row.map_err(|error| format!("decode sync table {table}: {error}"))?;
        columns.insert(name, (column_type.to_uppercase(), primary_key));
    }
    Ok(columns)
}

fn require_sync_columns(
    conn: &rusqlite::Connection,
    table: &str,
    required: &[&str],
) -> Result<std::collections::HashMap<String, (String, i64)>, String> {
    if !sync_schema_object_exists(conn, "table", table)? {
        return Err(format!("sync schema is missing table {table}"));
    }
    let columns = sync_schema_columns(conn, table)?;
    for column in required {
        if !columns.contains_key(*column) {
            return Err(format!("sync schema is missing column {table}.{column}"));
        }
    }
    Ok(columns)
}

pub(crate) fn verify_sync_schema_for_tables(
    conn: &rusqlite::Connection,
    tables: &[&str],
) -> Result<(), String> {
    require_sync_columns(conn, "app_settings", &["key", "value"])?;
    let tombstone_columns = require_sync_columns(
        conn,
        "sync_tombstones",
        &["table_name", "row_id", "deleted_at"],
    )?;
    if !tombstone_columns
        .get("row_id")
        .is_some_and(|(column_type, _)| column_type.contains("TEXT"))
    {
        return Err("sync schema requires sync_tombstones.row_id TEXT".into());
    }
    require_sync_columns(
        conn,
        "sync_row_versions",
        &["table_name", "row_id", "updated_at", "device_id"],
    )?;
    let hlc_columns =
        require_sync_columns(conn, "sync_hlc_state", &["singleton", "last_millis"])?;
    if !hlc_columns
        .get("last_millis")
        .is_some_and(|(column_type, _)| column_type.contains("INT"))
    {
        return Err("sync schema requires sync_hlc_state.last_millis INTEGER".into());
    }
    let hlc_state: (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN singleton=1 THEN 1 ELSE 0 END),0),
                    COALESCE(MAX(CASE WHEN singleton=1 THEN last_millis END),-1)
             FROM sync_hlc_state",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| format!("read sync HLC state: {error}"))?;
    if hlc_state.0 != 1 || hlc_state.1 != 1 || hlc_state.2 < 0 {
        return Err("sync HLC state must contain one non-negative clock row".into());
    }
    require_sync_columns(
        conn,
        "sync_apply_context",
        &["singleton", "remote_apply", "stamp_depth"],
    )?;
    let apply_context: (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN singleton=1 THEN 1 ELSE 0 END),0),
                    COALESCE(MAX(CASE WHEN singleton=1 THEN remote_apply END),-1),
                    COALESCE(MAX(CASE WHEN singleton=1 THEN stamp_depth END),-1)
             FROM sync_apply_context",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| format!("read sync apply context: {error}"))?;
    if apply_context != (1, 1, 0, 0) {
        return Err("sync apply context is not idle".into());
    }
    let generation_marker: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM app_settings WHERE key=?1",
            [SYNC_HLC_GENERATION_MARKER],
            |row| row.get(0),
        )
        .map_err(|error| format!("read sync HLC generation marker: {error}"))?;
    if generation_marker != 1 {
        return Err("sync schema is missing the sync HLC generation marker".into());
    }

    for table in tables {
        let columns = require_sync_columns(conn, table, &["id", "updated_at"])?;
        let (id_type, primary_key) = columns
            .get("id")
            .ok_or_else(|| format!("sync schema is missing column {table}.id"))?;
        if *primary_key == 0 || !(id_type.contains("INT") || id_type.contains("TEXT")) {
            return Err(format!(
                "sync schema requires {table}.id to be an INTEGER or TEXT primary key"
            ));
        }
        if !columns
            .get("updated_at")
            .is_some_and(|(column_type, _)| column_type.contains("TEXT"))
        {
            return Err(format!("sync schema requires {table}.updated_at TEXT"));
        }
        for suffix in [
            "set_updated_at_on_insert",
            "bump_updated_at",
            "clear_sync_version_on_insert",
            "clear_sync_version_on_update",
            "tombstone",
        ] {
            let trigger = format!("{table}_{suffix}");
            if !sync_schema_object_exists(conn, "trigger", &trigger)? {
                return Err(format!("sync schema is missing trigger {trigger}"));
            }
            if matches!(suffix, "set_updated_at_on_insert" | "bump_updated_at" | "tombstone") {
                let sql: String = conn
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?1",
                        [&trigger],
                        |row| row.get(0),
                    )
                    .map_err(|error| format!("inspect sync trigger {trigger}: {error}"))?;
                if !sql.contains("sync_hlc_state") || !sql.contains("sync_apply_context") {
                    return Err(format!("sync trigger {trigger} is not HLC-bound"));
                }
            }
        }
    }
    if tables.contains(&"event_categories") {
        if !sync_schema_object_exists(conn, "trigger", "event_categories_name_tombstone")? {
            return Err("sync schema is missing trigger event_categories_name_tombstone".into());
        }
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type='trigger' AND name='event_categories_name_tombstone'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("inspect event category sync trigger: {error}"))?;
        if !sql.contains("sync_hlc_state") || !sql.contains("sync_apply_context") {
            return Err("event_categories_name_tombstone is not HLC-bound".into());
        }
    }
    Ok(())
}

pub(crate) fn verify_sync_schema(conn: &rusqlite::Connection) -> Result<(), String> {
    verify_sync_schema_for_tables(conn, SYNC_TABLES)
}

/// Phase 1 of UUID-PK migration: replace AUTOINCREMENT INTEGER ids in
/// sleep_sessions + sleep_stages with UUIDv7 TEXT ids so cross-device
/// sync stops orphaning stages (two devices' Mac/phone independent
/// auto-increments collided, FK by id sent invalid session_id to peer
/// and Hanni UI showed `avg_deep_minutes=0`). Idempotent — re-running
/// on an already-migrated DB short-circuits.
pub fn migrate_sleep_to_uuid_pk(conn: &rusqlite::Connection) {
    if column_is_text(conn, "sleep_sessions", "id") {
        return; // already migrated
    }
    // sleep_sessions may not exist on a fresh install that's about to
    // get its first init_db pass — that's fine, the new init_db schema
    // will create the TEXT-pk version.
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sleep_sessions'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if exists == 0 { return; }

    use std::collections::HashMap;
    let mut session_id_map: HashMap<i64, String> = HashMap::new();

    let result: Result<(), rusqlite::Error> = (|| {
        // 1. Collect existing sessions + build i64 → UUIDv7 map.
        let mut stmt = conn.prepare("SELECT id FROM sleep_sessions")?;
        let ids: Vec<i64> = stmt.query_map([], |r| r.get(0))?
            .filter_map(Result::ok).collect();
        drop(stmt);
        for id in ids {
            session_id_map.insert(id, crate::types::new_uuid_v7());
        }

        conn.execute_batch(
            "BEGIN;
             CREATE TABLE sleep_sessions_new (
                 id TEXT PRIMARY KEY,
                 date TEXT NOT NULL,
                 start_time TEXT NOT NULL,
                 end_time TEXT NOT NULL,
                 duration_minutes INTEGER NOT NULL,
                 source TEXT NOT NULL DEFAULT 'manual',
                 quality_score INTEGER,
                 notes TEXT NOT NULL DEFAULT '',
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT '',
                 UNIQUE(date, start_time, source)
             );
             CREATE TABLE sleep_stages_new (
                 id TEXT PRIMARY KEY,
                 session_id TEXT NOT NULL REFERENCES sleep_sessions_new(id) ON DELETE CASCADE,
                 start_time TEXT NOT NULL,
                 end_time TEXT NOT NULL,
                 stage TEXT NOT NULL,
                 updated_at TEXT NOT NULL DEFAULT ''
             );"
        )?;

        // 2. Copy sessions with new UUIDs.
        let mut sel = conn.prepare(
            "SELECT id, date, start_time, end_time, duration_minutes, source,
                    quality_score, notes, created_at,
                    COALESCE(updated_at, '')
             FROM sleep_sessions"
        )?;
        let rows: Vec<(i64, String, String, String, i64, String, Option<i64>, String, String, String)> =
            sel.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?,
            )))?.filter_map(Result::ok).collect();
        drop(sel);
        for (old_id, date, st, en, dur, src, qs, notes, ca, ua) in &rows {
            let new_id = session_id_map.get(old_id).cloned().unwrap_or_default();
            conn.execute(
                "INSERT INTO sleep_sessions_new
                 (id, date, start_time, end_time, duration_minutes, source,
                  quality_score, notes, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![new_id, date, st, en, dur, src, qs, notes, ca, ua],
            )?;
        }

        // 3. Copy stages — rewrite session_id from i64 to mapped UUID.
        // Drop orphans whose session_id no longer exists.
        let mut sel = conn.prepare(
            "SELECT session_id, start_time, end_time, stage,
                    COALESCE(updated_at, '')
             FROM sleep_stages"
        )?;
        let stage_rows: Vec<(i64, String, String, String, String)> =
            sel.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
            )))?.filter_map(Result::ok).collect();
        drop(sel);
        for (old_sid, st, en, stage, ua) in &stage_rows {
            let parent_uuid = match session_id_map.get(old_sid) {
                Some(u) => u.clone(),
                None => continue, // orphan — parent gone
            };
            let stage_uuid = crate::types::new_uuid_v7();
            conn.execute(
                "INSERT INTO sleep_stages_new
                 (id, session_id, start_time, end_time, stage, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                rusqlite::params![stage_uuid, parent_uuid, st, en, stage, ua],
            )?;
        }

        // 4. Swap old tables out. sync_tombstones for sleep_* now carries
        // stringified old-INTEGER ids that mean nothing post-migration —
        // wipe so re-pushes of pre-migration tombstones don't poison.
        conn.execute_batch(
            "DELETE FROM sync_tombstones WHERE table_name IN ('sleep_sessions','sleep_stages');
             DROP TABLE sleep_stages;
             DROP TABLE sleep_sessions;
             ALTER TABLE sleep_sessions_new RENAME TO sleep_sessions;
             ALTER TABLE sleep_stages_new RENAME TO sleep_stages;
             CREATE INDEX IF NOT EXISTS idx_sleep_date ON sleep_sessions(date);
             CREATE INDEX IF NOT EXISTS idx_sleep_stages_session ON sleep_stages(session_id);
             COMMIT;"
        )?;
        Ok(())
    })();

    if let Err(e) = result {
        eprintln!("[migrate_sleep_to_uuid_pk] failed: {} — rolling back", e);
        let _ = conn.execute_batch("ROLLBACK;");
    } else {
        eprintln!("[migrate_sleep_to_uuid_pk] migrated {} sessions to UUID pk",
                  session_id_map.len());
    }
}

/// Phase 2 of UUID-PK migration: health_log + heart_rate_samples.
/// Same motivation as Phase 1 — auto-increment ids collide across devices
/// so peer-pushed rows either overwrite our local row (LWW silent overwrite)
/// or pile up as duplicates. Idempotent.
pub fn migrate_health_to_uuid_pk(conn: &rusqlite::Connection) {
    use std::collections::HashMap;

    // ── health_log ──
    if !column_is_text(conn, "health_log", "id") && {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='health_log'",
            [], |r| r.get(0),
        ).unwrap_or(0);
        n > 0
    } {
        let result: Result<usize, rusqlite::Error> = (|| {
            // Build id → UUIDv7 map for existing rows.
            let mut id_map: HashMap<i64, String> = HashMap::new();
            let ids: Vec<i64> = conn.prepare("SELECT id FROM health_log")?
                .query_map([], |r| r.get(0))?.filter_map(Result::ok).collect();
            for id in ids { id_map.insert(id, crate::types::new_uuid_v7()); }
            let n = id_map.len();

            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE health_log_new (
                     id TEXT PRIMARY KEY,
                     date TEXT NOT NULL,
                     type TEXT NOT NULL,
                     value REAL NOT NULL,
                     unit TEXT NOT NULL DEFAULT '',
                     notes TEXT NOT NULL DEFAULT '',
                     created_at TEXT NOT NULL,
                     start_time TEXT NOT NULL DEFAULT '',
                     updated_at TEXT NOT NULL DEFAULT ''
                 );"
            )?;

            let mut sel = conn.prepare(
                "SELECT id, date, type, value, unit, notes, created_at,
                        COALESCE(start_time, ''), COALESCE(updated_at, '')
                 FROM health_log"
            )?;
            let rows: Vec<(i64, String, String, f64, String, String, String, String, String)> =
                sel.query_map([], |r| Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                    r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?,
                )))?.filter_map(Result::ok).collect();
            drop(sel);
            for (old_id, date, ty, val, unit, notes, ca, st, ua) in &rows {
                let new_id = id_map.get(old_id).cloned().unwrap_or_default();
                conn.execute(
                    "INSERT INTO health_log_new
                     (id, date, type, value, unit, notes, created_at, start_time, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    rusqlite::params![new_id, date, ty, val, unit, notes, ca, st, ua],
                )?;
            }

            conn.execute_batch(
                "DELETE FROM sync_tombstones WHERE table_name='health_log';
                 DROP TABLE health_log;
                 ALTER TABLE health_log_new RENAME TO health_log;
                 CREATE INDEX IF NOT EXISTS idx_health_log_date ON health_log(date);
                 COMMIT;"
            )?;
            Ok(n)
        })();
        match result {
            Ok(n) => eprintln!("[migrate_health_to_uuid_pk] health_log: migrated {n} rows"),
            Err(e) => {
                eprintln!("[migrate_health_to_uuid_pk] health_log failed: {e}");
                let _ = conn.execute_batch("ROLLBACK;");
            }
        }
    }

    // ── heart_rate_samples ──
    if !column_is_text(conn, "heart_rate_samples", "id") && {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='heart_rate_samples'",
            [], |r| r.get(0),
        ).unwrap_or(0);
        n > 0
    } {
        let result: Result<usize, rusqlite::Error> = (|| {
            let mut id_map: HashMap<i64, String> = HashMap::new();
            let ids: Vec<i64> = conn.prepare("SELECT id FROM heart_rate_samples")?
                .query_map([], |r| r.get(0))?.filter_map(Result::ok).collect();
            for id in ids { id_map.insert(id, crate::types::new_uuid_v7()); }
            let n = id_map.len();

            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE heart_rate_samples_new (
                     id TEXT PRIMARY KEY,
                     date TEXT NOT NULL,
                     time TEXT NOT NULL,
                     bpm INTEGER NOT NULL,
                     source TEXT NOT NULL DEFAULT 'health_connect',
                     created_at TEXT NOT NULL DEFAULT (datetime('now')),
                     updated_at TEXT NOT NULL DEFAULT '',
                     UNIQUE(date, time, source)
                 );"
            )?;

            let mut sel = conn.prepare(
                "SELECT id, date, time, bpm, source, created_at,
                        COALESCE(updated_at, '')
                 FROM heart_rate_samples"
            )?;
            let rows: Vec<(i64, String, String, i64, String, String, String)> =
                sel.query_map([], |r| Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                    r.get(4)?, r.get(5)?, r.get(6)?,
                )))?.filter_map(Result::ok).collect();
            drop(sel);
            for (old_id, date, time, bpm, src, ca, ua) in &rows {
                let new_id = id_map.get(old_id).cloned().unwrap_or_default();
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO heart_rate_samples_new
                     (id, date, time, bpm, source, created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    rusqlite::params![new_id, date, time, bpm, src, ca, ua],
                );
            }

            conn.execute_batch(
                "DELETE FROM sync_tombstones WHERE table_name='heart_rate_samples';
                 DROP TABLE heart_rate_samples;
                 ALTER TABLE heart_rate_samples_new RENAME TO heart_rate_samples;
                 CREATE INDEX IF NOT EXISTS idx_hr_samples_date ON heart_rate_samples(date);
                 COMMIT;"
            )?;
            Ok(n)
        })();
        match result {
            Ok(n) => eprintln!("[migrate_health_to_uuid_pk] heart_rate_samples: migrated {n} rows"),
            Err(e) => {
                eprintln!("[migrate_health_to_uuid_pk] heart_rate_samples failed: {e}");
                let _ = conn.execute_batch("ROLLBACK;");
            }
        }
    }
}

/// Phase 3 of UUID-PK migration: schedules + schedule_completions.
/// schedule_completions.schedule_id FK is rewritten via the parent's
/// i64 → UUID map; orphan completions are dropped. Idempotent.
pub fn migrate_schedules_to_uuid_pk(conn: &rusqlite::Connection) {
    use std::collections::HashMap;

    if column_is_text(conn, "schedules", "id") { return; }
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schedules'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if exists == 0 { return; }

    let mut id_map: HashMap<i64, String> = HashMap::new();

    let result: Result<(usize, usize), rusqlite::Error> = (|| {
        // Map old schedules.id → UUIDv7.
        let ids: Vec<i64> = conn.prepare("SELECT id FROM schedules")?
            .query_map([], |r| r.get(0))?.filter_map(Result::ok).collect();
        for id in ids { id_map.insert(id, crate::types::new_uuid_v7()); }

        // Detect which optional columns exist on the live `schedules` so the
        // migration also tolerates older installs that haven't run the
        // priority/stage_id/etc. ALTERs yet.
        let cols: std::collections::HashSet<String> = conn.prepare(
            "SELECT name FROM pragma_table_info('schedules')"
        )?.query_map([], |r| r.get::<_, String>(0))?
          .filter_map(Result::ok).collect();
        let has = |c: &str| cols.contains(c);

        conn.execute_batch(
            "BEGIN;
             CREATE TABLE schedules_new (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 category TEXT NOT NULL DEFAULT 'other',
                 frequency TEXT NOT NULL DEFAULT 'daily',
                 frequency_days TEXT,
                 time_of_day TEXT,
                 details TEXT DEFAULT '',
                 is_active INTEGER DEFAULT 1,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 marks_previous_day INTEGER DEFAULT 0,
                 until_date TEXT,
                 track_overdue INTEGER NOT NULL DEFAULT 0,
                 target_minutes INTEGER,
                 updated_at TEXT NOT NULL DEFAULT '',
                 tracking_mode TEXT NOT NULL DEFAULT 'track',
                 stage_id INTEGER,
                 priority INTEGER NOT NULL DEFAULT 0,
                 requirement TEXT NOT NULL DEFAULT 'required',
                 task_order INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE schedule_completions_new (
                 id TEXT PRIMARY KEY,
                 schedule_id TEXT NOT NULL REFERENCES schedules_new(id) ON DELETE CASCADE,
                 date TEXT NOT NULL,
                 completed INTEGER DEFAULT 0,
                 completed_at TEXT,
                 status TEXT DEFAULT 'done',
                 updated_at TEXT NOT NULL DEFAULT '',
                 UNIQUE(schedule_id, date)
             );"
        )?;

        // Copy schedules. Use COALESCE for columns that may not exist on
        // older installs (sentinel default from the new table).
        let select_extras = [
            ("marks_previous_day", "0"),
            ("until_date", "NULL"),
            ("track_overdue", "0"),
            ("target_minutes", "NULL"),
            ("updated_at", "''"),
            ("tracking_mode", "'track'"),
            ("stage_id", "NULL"),
            ("priority", "0"),
            ("requirement", "'required'"),
            ("task_order", "0"),
        ];
        let extras_select = select_extras.iter()
            .map(|(c, d)| if has(c) { format!(", {c}") } else { format!(", {d} AS {c}") })
            .collect::<Vec<_>>().join("");
        let sel_sql = format!(
            "SELECT id, title, category, frequency, frequency_days, time_of_day,
                    details, is_active, created_at{extras_select}
             FROM schedules"
        );
        let mut stmt = conn.prepare(&sel_sql)?;
        let rows: Vec<(i64, String, String, String, Option<String>, Option<String>,
                        Option<String>, Option<i64>, String,
                        i64, Option<String>, i64, Option<i64>, String, String, Option<i64>, i64, String, i64)> =
            stmt.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                r.get(6)?, r.get(7)?, r.get(8)?,
                r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?, r.get(14)?,
                r.get(15)?, r.get(16)?, r.get(17)?, r.get(18)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt);
        let n_sched = rows.len();
        for r in &rows {
            let new_id = id_map.get(&r.0).cloned().unwrap_or_default();
            conn.execute(
                "INSERT INTO schedules_new
                 (id, title, category, frequency, frequency_days, time_of_day,
                  details, is_active, created_at,
                  marks_previous_day, until_date, track_overdue, target_minutes,
                  updated_at, tracking_mode, stage_id, priority, requirement, task_order)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
                rusqlite::params![
                    new_id, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8,
                    r.9, r.10, r.11, r.12, r.13, r.14, r.15, r.16, r.17, r.18,
                ],
            )?;
        }

        // Copy schedule_completions — rewrite schedule_id via id_map.
        let comp_cols: std::collections::HashSet<String> = conn.prepare(
            "SELECT name FROM pragma_table_info('schedule_completions')"
        )?.query_map([], |r| r.get::<_, String>(0))?
          .filter_map(Result::ok).collect();
        let status_col = if comp_cols.contains("status") { "COALESCE(status,'done')" } else { "'done'" };
        let updated_at_col = if comp_cols.contains("updated_at") { "COALESCE(updated_at,'')" } else { "''" };
        let sel = format!(
            "SELECT schedule_id, date, COALESCE(completed,0), completed_at, {status_col}, {updated_at_col}
             FROM schedule_completions"
        );
        let mut stmt = conn.prepare(&sel)?;
        let crows: Vec<(i64, String, i64, Option<String>, String, String)> =
            stmt.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt);
        let mut n_comp = 0;
        for (old_sid, date, completed, completed_at, status, ua) in &crows {
            let parent_uuid = match id_map.get(old_sid) {
                Some(u) => u.clone(),
                None => continue, // orphan completion — parent gone
            };
            let new_id = crate::types::new_uuid_v7();
            conn.execute(
                "INSERT OR IGNORE INTO schedule_completions_new
                 (id, schedule_id, date, completed, completed_at, status, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![new_id, parent_uuid, date, completed, completed_at, status, ua],
            )?;
            n_comp += 1;
        }

        // routine_nodes.source_id used to be INTEGER pointing at schedule
        // ids; rewrite via id_map. routine_nodes itself isn't in SYNC_TABLES
        // (it's local-only), but if we leave INTEGER source_id pointing at
        // ids that no longer exist, routine→schedule resolution silently
        // falls back to the title heuristic.
        if conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='routine_nodes'",
            [], |r| r.get::<_, i64>(0),
        ).unwrap_or(0) > 0 {
            // Read all routine_nodes.source_id (INTEGER) and remap via id_map
            // for schedule-typed nodes, writing back as TEXT — SQLite's lax
            // typing lets us put strings into an INTEGER column transparently.
            let pairs: Vec<(i64, i64)> = conn.prepare(
                "SELECT id, source_id FROM routine_nodes
                 WHERE source_type='schedule' AND source_id IS NOT NULL"
            )?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
              .filter_map(Result::ok).collect();
            for (rn_id, old_sid) in &pairs {
                if let Some(new_uuid) = id_map.get(old_sid) {
                    let _ = conn.execute(
                        "UPDATE routine_nodes SET source_id=?1 WHERE id=?2",
                        rusqlite::params![new_uuid, rn_id],
                    );
                } else {
                    // Parent schedule is gone — clear the dangling reference.
                    let _ = conn.execute(
                        "UPDATE routine_nodes SET source_id=NULL WHERE id=?1",
                        rusqlite::params![rn_id],
                    );
                }
            }
        }

        conn.execute_batch(
            "DELETE FROM sync_tombstones WHERE table_name IN ('schedules','schedule_completions');
             DROP TABLE schedule_completions;
             DROP TABLE schedules;
             ALTER TABLE schedules_new RENAME TO schedules;
             ALTER TABLE schedule_completions_new RENAME TO schedule_completions;
             COMMIT;"
        )?;
        Ok((n_sched, n_comp))
    })();
    match result {
        Ok((s, c)) => eprintln!("[migrate_schedules_to_uuid_pk] migrated {s} schedules + {c} completions"),
        Err(e) => {
            eprintln!("[migrate_schedules_to_uuid_pk] failed: {e}");
            let _ = conn.execute_batch("ROLLBACK;");
        }
    }
}

/// Re-key the routine graph onto DETERMINISTIC integer ids so both devices
/// converge on the same id for the same logical row — the prerequisite for
/// syncing routine run/completion state (the per-device AUTOINCREMENT ids would
/// otherwise collide). Seeded chains key on title (identical in source on both
/// devices); user/renamed chains/nodes/edges key on device_id+old_id (they live
/// on one device, just need a stable unique id). runs key on (chain,date,slot),
/// node_status on (run,node) — matching start_routine_run / set_routine_node_status
/// so a freshly-seeded device and a migrated one line up. Preserves all run +
/// completion state. One-time (gated by _migrations); rebuilds the 5 tables in
/// one transaction with foreign_keys OFF. Mirrors migrate_schedules_to_uuid_pk.
pub fn migrate_routine_ids_deterministic(conn: &rusqlite::Connection) {
    use std::collections::HashMap;
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_ids_deterministic_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    if conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='routine_chains'",
        [], |r| r.get::<_, i64>(0),
    ).unwrap_or(0) == 0 { return; }

    let det = crate::types::deterministic_id;
    let device_id: String = conn.query_row(
        "SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get::<_, String>(0),
    ).unwrap_or_else(|_| "local".into());
    let seeded: std::collections::HashSet<&str> =
        ["Утро", "Рефлексия", "Ночь", "Покушать", "Спорт"].into_iter().collect();
    let now_t = chrono::Local::now().to_rfc3339();
    let norm = |s: &str| -> String {
        if s.trim().is_empty() { now_t.clone() } else { s.replace(' ', "T") }
    };

    let _ = conn.execute_batch("PRAGMA foreign_keys=OFF;");
    let result: Result<(usize, usize, usize, usize, usize), rusqlite::Error> = (|| {
        // ── id maps (chains → nodes → edges → runs) ──
        let mut chain_map: HashMap<i64, i64> = HashMap::new();
        let mut chain_seeded: HashMap<i64, bool> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, title FROM routine_chains")?;
            let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .filter_map(Result::ok).collect();
            for (old, title) in rows {
                let is_seed = seeded.contains(title.as_str());
                let new = if is_seed { det(&format!("chain:{}", title)) }
                          else { det(&format!("uchain:{}:{}", device_id, old)) };
                chain_map.insert(old, new);
                chain_seeded.insert(old, is_seed);
            }
        }
        let mut node_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, chain_id, title FROM routine_nodes")?;
            let rows: Vec<(i64, i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .filter_map(Result::ok).collect();
            for (old, cid, title) in rows {
                let new_cid = match chain_map.get(&cid) { Some(c) => *c, None => continue };
                let new = if *chain_seeded.get(&cid).unwrap_or(&false) {
                    det(&format!("node:c{}:{}", new_cid, title))
                } else { det(&format!("unode:{}:{}", device_id, old)) };
                node_map.insert(old, new);
            }
        }
        let mut edge_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, chain_id, from_node_id, to_node_id FROM routine_edges")?;
            let rows: Vec<(i64, i64, i64, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .filter_map(Result::ok).collect();
            for (old, cid, from, to) in rows {
                let new_cid = match chain_map.get(&cid) { Some(c) => *c, None => continue };
                let (nf, nt) = match (node_map.get(&from), node_map.get(&to)) {
                    (Some(a), Some(b)) => (*a, *b), _ => continue };
                let new = if *chain_seeded.get(&cid).unwrap_or(&false) {
                    det(&format!("edge:c{}:{}>{}", new_cid, nf, nt))
                } else { det(&format!("uedge:{}:{}", device_id, old)) };
                edge_map.insert(old, new);
            }
        }
        let mut run_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, chain_id, date, slot FROM routine_runs")?;
            let rows: Vec<(i64, i64, String, String)> = stmt.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            )))?.filter_map(Result::ok).collect();
            for (old, cid, date, slot) in rows {
                let new_cid = match chain_map.get(&cid) { Some(c) => *c, None => continue };
                run_map.insert(old, crate::types::routine_run_id(new_cid, &date, &slot));
            }
        }

        // ── rebuild the 5 tables, applying the maps to ids + every FK ──
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE routine_chains_new (
                 id INTEGER PRIMARY KEY, title TEXT NOT NULL,
                 trigger_type TEXT NOT NULL DEFAULT 'manual',
                 is_active INTEGER NOT NULL DEFAULT 1,
                 sort_order INTEGER NOT NULL DEFAULT 0,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 trigger_time TEXT,
                 updated_at TEXT NOT NULL DEFAULT '');
             CREATE TABLE routine_nodes_new (
                 id INTEGER PRIMARY KEY,
                 chain_id INTEGER NOT NULL REFERENCES routine_chains_new(id) ON DELETE CASCADE,
                 source_type TEXT NOT NULL DEFAULT 'schedule',
                 source_id TEXT,
                 title TEXT NOT NULL,
                 category TEXT NOT NULL DEFAULT 'other',
                 icon TEXT,
                 pos_x INTEGER NOT NULL DEFAULT 0,
                 pos_y INTEGER NOT NULL DEFAULT 0,
                 priority INTEGER NOT NULL DEFAULT 0,
                 requirement TEXT NOT NULL DEFAULT 'required',
                 is_start INTEGER NOT NULL DEFAULT 0,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT '');
             CREATE TABLE routine_edges_new (
                 id INTEGER PRIMARY KEY,
                 chain_id INTEGER NOT NULL REFERENCES routine_chains_new(id) ON DELETE CASCADE,
                 from_node_id INTEGER NOT NULL REFERENCES routine_nodes_new(id) ON DELETE CASCADE,
                 to_node_id INTEGER NOT NULL REFERENCES routine_nodes_new(id) ON DELETE CASCADE,
                 trigger_type TEXT NOT NULL DEFAULT 'after_completion',
                 trigger_value INTEGER,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT '');
             CREATE TABLE routine_runs_new (
                 id INTEGER PRIMARY KEY,
                 chain_id INTEGER NOT NULL REFERENCES routine_chains_new(id) ON DELETE CASCADE,
                 date TEXT NOT NULL,
                 slot TEXT NOT NULL DEFAULT '',
                 state TEXT NOT NULL DEFAULT 'active',
                 started_at TEXT NOT NULL DEFAULT (datetime('now')),
                 completed_at TEXT,
                 updated_at TEXT NOT NULL DEFAULT '',
                 UNIQUE(chain_id, date, slot));
             CREATE TABLE routine_node_status_new (
                 id INTEGER PRIMARY KEY,
                 run_id INTEGER NOT NULL REFERENCES routine_runs_new(id) ON DELETE CASCADE,
                 node_id INTEGER NOT NULL REFERENCES routine_nodes_new(id) ON DELETE CASCADE,
                 state TEXT NOT NULL DEFAULT 'done',
                 updated_at TEXT NOT NULL DEFAULT '',
                 UNIQUE(run_id, node_id));"
        )?;

        // chains
        let mut stmt_c = conn.prepare(
            "SELECT id, title, trigger_type, is_active, sort_order, created_at, trigger_time FROM routine_chains")?;
        let chain_rows: Vec<(i64, String, String, i64, i64, String, Option<String>)> =
            stmt_c.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt_c);
        let n_chains = chain_rows.len();
        for (old, title, tt, act, so, ca, trt) in &chain_rows {
            let nid = chain_map[old];
            conn.execute(
                "INSERT OR IGNORE INTO routine_chains_new
                 (id, title, trigger_type, is_active, sort_order, created_at, trigger_time, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![nid, title, tt, act, so, ca, trt, norm(ca)],
            )?;
        }

        // nodes (source_id may be TEXT UUID or legacy INTEGER — read as string)
        let mut stmt_n = conn.prepare(
            "SELECT id, chain_id, source_type, source_id, title, category, icon, pos_x, pos_y,
                    priority, requirement, is_start, created_at FROM routine_nodes")?;
        let node_rows: Vec<(i64, i64, String, Option<String>, String, String, Option<String>, i64, i64, i64, String, i64, String)> =
            stmt_n.query_map([], |r| {
                let sid: Option<String> = match r.get::<_, Option<String>>(3) {
                    Ok(v) => v,
                    Err(_) => r.get::<_, Option<i64>>(3).ok().flatten().map(|i| i.to_string()),
                };
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, sid, r.get(4)?, r.get(5)?, r.get(6)?,
                    r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?))
            })?.filter_map(Result::ok).collect();
        drop(stmt_n);
        let mut n_nodes = 0;
        for (old, cid, stype, sid, title, cat, icon, x, y, pri, req, is_start, ca) in &node_rows {
            let nid = match node_map.get(old) { Some(n) => *n, None => continue };
            let ncid = match chain_map.get(cid) { Some(c) => *c, None => continue };
            conn.execute(
                "INSERT OR IGNORE INTO routine_nodes_new
                 (id, chain_id, source_type, source_id, title, category, icon, pos_x, pos_y,
                  priority, requirement, is_start, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                rusqlite::params![nid, ncid, stype, sid, title, cat, icon, x, y, pri, req, is_start, ca, norm(ca)],
            )?;
            n_nodes += 1;
        }

        // edges (skip orphans whose endpoints didn't map)
        let mut stmt_e = conn.prepare(
            "SELECT id, chain_id, from_node_id, to_node_id, trigger_type, trigger_value, created_at FROM routine_edges")?;
        let edge_rows: Vec<(i64, i64, i64, i64, String, Option<i64>, String)> =
            stmt_e.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt_e);
        let mut n_edges = 0;
        for (old, cid, from, to, tt, tv, ca) in &edge_rows {
            let eid = match edge_map.get(old) { Some(e) => *e, None => continue };
            let ncid = match chain_map.get(cid) { Some(c) => *c, None => continue };
            let (nf, nt) = match (node_map.get(from), node_map.get(to)) {
                (Some(a), Some(b)) => (*a, *b), _ => continue };
            conn.execute(
                "INSERT OR IGNORE INTO routine_edges_new
                 (id, chain_id, from_node_id, to_node_id, trigger_type, trigger_value, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![eid, ncid, nf, nt, tt, tv, ca, norm(ca)],
            )?;
            n_edges += 1;
        }

        // runs (preserve state/completed_at; updated_at from real timestamp so LWW works)
        let mut stmt_r = conn.prepare(
            "SELECT id, chain_id, date, slot, state, started_at, completed_at FROM routine_runs")?;
        let run_rows: Vec<(i64, i64, String, String, String, String, Option<String>)> =
            stmt_r.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get(4)?, r.get(5)?, r.get(6)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt_r);
        let mut n_runs = 0;
        for (old, cid, date, slot, state, started, completed) in &run_rows {
            let nid = match run_map.get(old) { Some(n) => *n, None => continue };
            let ncid = match chain_map.get(cid) { Some(c) => *c, None => continue };
            let ua = norm(completed.as_deref().unwrap_or(started));
            conn.execute(
                "INSERT OR IGNORE INTO routine_runs_new
                 (id, chain_id, date, slot, state, started_at, completed_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![nid, ncid, date, slot, state, started, completed, ua],
            )?;
            n_runs += 1;
        }

        // node_status (the actual completions; skip rows whose run/node didn't map)
        let mut stmt_s = conn.prepare(
            "SELECT id, run_id, node_id, state, updated_at FROM routine_node_status")?;
        let stat_rows: Vec<(i64, i64, i64, String, String)> =
            stmt_s.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            )))?.filter_map(Result::ok).collect();
        drop(stmt_s);
        let mut n_stat = 0;
        for (_old, rid, nid, state, ua) in &stat_rows {
            let nrid = match run_map.get(rid) { Some(r) => *r, None => continue };
            let nnid = match node_map.get(nid) { Some(n) => *n, None => continue };
            let new_sid = crate::types::routine_node_status_id(nrid, nnid);
            conn.execute(
                "INSERT OR IGNORE INTO routine_node_status_new
                 (id, run_id, node_id, state, updated_at) VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![new_sid, nrid, nnid, state, norm(ua)],
            )?;
            n_stat += 1;
        }

        conn.execute_batch(
            "DROP TABLE routine_node_status;
             DROP TABLE routine_edges;
             DROP TABLE routine_runs;
             DROP TABLE routine_nodes;
             DROP TABLE routine_chains;
             ALTER TABLE routine_chains_new RENAME TO routine_chains;
             ALTER TABLE routine_nodes_new RENAME TO routine_nodes;
             ALTER TABLE routine_edges_new RENAME TO routine_edges;
             ALTER TABLE routine_runs_new RENAME TO routine_runs;
             ALTER TABLE routine_node_status_new RENAME TO routine_node_status;
             CREATE INDEX IF NOT EXISTS idx_routine_nodes_chain ON routine_nodes(chain_id);
             CREATE INDEX IF NOT EXISTS idx_routine_edges_chain ON routine_edges(chain_id);
             CREATE INDEX IF NOT EXISTS idx_routine_runs_date ON routine_runs(date);
             CREATE INDEX IF NOT EXISTS idx_routine_node_status_run ON routine_node_status(run_id);
             DELETE FROM sync_tombstones WHERE table_name IN
               ('routine_chains','routine_nodes','routine_edges','routine_runs','routine_node_status');
             INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_ids_deterministic_v1');
             COMMIT;"
        )?;
        Ok((n_chains, n_nodes, n_edges, n_runs, n_stat))
    })();
    let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");
    match result {
        Ok((c, n, e, r, s)) => eprintln!(
            "[migrate_routine_ids_deterministic] re-keyed {c} chains, {n} nodes, {e} edges, {r} runs, {s} statuses"),
        Err(err) => {
            eprintln!("[migrate_routine_ids_deterministic] failed: {err}");
            let _ = conn.execute_batch("ROLLBACK;");
        }
    }
}

/// v2 of the deterministic re-key. v1 only content-keyed five hardcoded seed
/// titles (`Утро`/`Рефлексия`/…); every other chain — renamed seeds (`Еда`,
/// `Вечер`) and user chains (`Уборка`, `Dan Koe`) — got a device-local id, so
/// two devices held the *same* chain under different ids. Pulling the graph
/// then duplicated it, which is why the graph was kept out of SYNC_TABLES and
/// node_status pulls failed the FK on the missing node. v2 keys the WHOLE graph
/// by content (chain=title, node=chain|title, edge=chain|from|to) so identical
/// rows converge on identical ids across devices — the prerequisite for syncing
/// chains/nodes/edges. One-time (gated), rebuilds the 5 tables in one FK-off
/// transaction. Run AFTER v1 and BEFORE the trigger-binding migrate_sync_meta.
pub fn migrate_routine_ids_deterministic_v2(conn: &rusqlite::Connection) {
    use std::collections::HashMap;
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_ids_deterministic_v2'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    if conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='routine_chains'",
        [], |r| r.get::<_, i64>(0),
    ).unwrap_or(0) == 0 { return; }

    let det = crate::types::deterministic_id;
    let now_t = chrono::Local::now().to_rfc3339();
    let norm = |s: &str| -> String {
        if s.trim().is_empty() { now_t.clone() } else { s.replace(' ', "T") }
    };

    let _ = conn.execute_batch("PRAGMA foreign_keys=OFF;");
    let result: Result<(usize, usize, usize, usize, usize), rusqlite::Error> = (|| {
        // ── id maps, keyed purely by content so both devices agree ──
        let mut chain_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, title FROM routine_chains")?;
            let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .filter_map(Result::ok).collect();
            for (old, title) in rows { chain_map.insert(old, det(&format!("chain:{}", title))); }
        }
        let mut node_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, chain_id, title FROM routine_nodes")?;
            let rows: Vec<(i64, i64, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .filter_map(Result::ok).collect();
            for (old, cid, title) in rows {
                let ncid = match chain_map.get(&cid) { Some(c) => *c, None => continue };
                node_map.insert(old, det(&format!("node:c{}:{}", ncid, title)));
            }
        }
        let mut edge_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, chain_id, from_node_id, to_node_id FROM routine_edges")?;
            let rows: Vec<(i64, i64, i64, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .filter_map(Result::ok).collect();
            for (old, cid, from, to) in rows {
                let ncid = match chain_map.get(&cid) { Some(c) => *c, None => continue };
                let (nf, nt) = match (node_map.get(&from), node_map.get(&to)) {
                    (Some(a), Some(b)) => (*a, *b), _ => continue };
                edge_map.insert(old, det(&format!("edge:c{}:{}>{}", ncid, nf, nt)));
            }
        }
        let mut run_map: HashMap<i64, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, chain_id, date, slot FROM routine_runs")?;
            let rows: Vec<(i64, i64, String, String)> = stmt.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            )))?.filter_map(Result::ok).collect();
            for (old, cid, date, slot) in rows {
                let ncid = match chain_map.get(&cid) { Some(c) => *c, None => continue };
                run_map.insert(old, crate::types::routine_run_id(ncid, &date, &slot));
            }
        }

        // ── rebuild the 5 tables (current schema), applying the maps ──
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE routine_chains_new (
                 id INTEGER PRIMARY KEY, title TEXT NOT NULL,
                 trigger_type TEXT NOT NULL DEFAULT 'manual',
                 is_active INTEGER NOT NULL DEFAULT 1,
                 sort_order INTEGER NOT NULL DEFAULT 0,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 trigger_time TEXT,
                 updated_at TEXT NOT NULL DEFAULT '');
             CREATE TABLE routine_nodes_new (
                 id INTEGER PRIMARY KEY,
                 chain_id INTEGER NOT NULL REFERENCES routine_chains_new(id) ON DELETE CASCADE,
                 source_type TEXT NOT NULL DEFAULT 'schedule',
                 source_id TEXT,
                 title TEXT NOT NULL,
                 category TEXT NOT NULL DEFAULT 'other',
                 icon TEXT,
                 pos_x INTEGER NOT NULL DEFAULT 0,
                 pos_y INTEGER NOT NULL DEFAULT 0,
                 priority INTEGER NOT NULL DEFAULT 0,
                 requirement TEXT NOT NULL DEFAULT 'required',
                 is_start INTEGER NOT NULL DEFAULT 0,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT '');
             CREATE TABLE routine_edges_new (
                 id INTEGER PRIMARY KEY,
                 chain_id INTEGER NOT NULL REFERENCES routine_chains_new(id) ON DELETE CASCADE,
                 from_node_id INTEGER NOT NULL REFERENCES routine_nodes_new(id) ON DELETE CASCADE,
                 to_node_id INTEGER NOT NULL REFERENCES routine_nodes_new(id) ON DELETE CASCADE,
                 trigger_type TEXT NOT NULL DEFAULT 'after_completion',
                 trigger_value INTEGER,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT '');
             CREATE TABLE routine_runs_new (
                 id INTEGER PRIMARY KEY,
                 chain_id INTEGER NOT NULL REFERENCES routine_chains_new(id) ON DELETE CASCADE,
                 date TEXT NOT NULL,
                 slot TEXT NOT NULL DEFAULT '',
                 state TEXT NOT NULL DEFAULT 'active',
                 started_at TEXT NOT NULL DEFAULT (datetime('now')),
                 completed_at TEXT,
                 updated_at TEXT NOT NULL DEFAULT '',
                 UNIQUE(chain_id, date, slot));
             CREATE TABLE routine_node_status_new (
                 id INTEGER PRIMARY KEY,
                 run_id INTEGER NOT NULL REFERENCES routine_runs_new(id) ON DELETE CASCADE,
                 node_id INTEGER NOT NULL REFERENCES routine_nodes_new(id) ON DELETE CASCADE,
                 state TEXT NOT NULL DEFAULT 'done',
                 updated_at TEXT NOT NULL DEFAULT '',
                 UNIQUE(run_id, node_id));"
        )?;

        // chains
        let mut stmt_c = conn.prepare(
            "SELECT id, title, trigger_type, is_active, sort_order, created_at, trigger_time FROM routine_chains")?;
        let chain_rows: Vec<(i64, String, String, i64, i64, String, Option<String>)> =
            stmt_c.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt_c);
        let n_chains = chain_rows.len();
        for (old, title, tt, act, so, ca, trt) in &chain_rows {
            let nid = chain_map[old];
            conn.execute(
                "INSERT OR IGNORE INTO routine_chains_new
                 (id, title, trigger_type, is_active, sort_order, created_at, trigger_time, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![nid, title, tt, act, so, ca, trt, norm(ca)],
            )?;
        }

        // nodes (source_id may be TEXT UUID or legacy INTEGER — read as string)
        let mut stmt_n = conn.prepare(
            "SELECT id, chain_id, source_type, source_id, title, category, icon, pos_x, pos_y,
                    priority, requirement, is_start, created_at FROM routine_nodes")?;
        let node_rows: Vec<(i64, i64, String, Option<String>, String, String, Option<String>, i64, i64, i64, String, i64, String)> =
            stmt_n.query_map([], |r| {
                let sid: Option<String> = match r.get::<_, Option<String>>(3) {
                    Ok(v) => v,
                    Err(_) => r.get::<_, Option<i64>>(3).ok().flatten().map(|i| i.to_string()),
                };
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, sid, r.get(4)?, r.get(5)?, r.get(6)?,
                    r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?))
            })?.filter_map(Result::ok).collect();
        drop(stmt_n);
        let mut n_nodes = 0;
        for (old, cid, stype, sid, title, cat, icon, x, y, pri, req, is_start, ca) in &node_rows {
            let nid = match node_map.get(old) { Some(n) => *n, None => continue };
            let ncid = match chain_map.get(cid) { Some(c) => *c, None => continue };
            conn.execute(
                "INSERT OR IGNORE INTO routine_nodes_new
                 (id, chain_id, source_type, source_id, title, category, icon, pos_x, pos_y,
                  priority, requirement, is_start, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                rusqlite::params![nid, ncid, stype, sid, title, cat, icon, x, y, pri, req, is_start, ca, norm(ca)],
            )?;
            n_nodes += 1;
        }

        // edges (skip orphans whose endpoints didn't map)
        let mut stmt_e = conn.prepare(
            "SELECT id, chain_id, from_node_id, to_node_id, trigger_type, trigger_value, created_at FROM routine_edges")?;
        let edge_rows: Vec<(i64, i64, i64, i64, String, Option<i64>, String)> =
            stmt_e.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt_e);
        let mut n_edges = 0;
        for (old, cid, from, to, tt, tv, ca) in &edge_rows {
            let eid = match edge_map.get(old) { Some(e) => *e, None => continue };
            let ncid = match chain_map.get(cid) { Some(c) => *c, None => continue };
            let (nf, nt) = match (node_map.get(from), node_map.get(to)) {
                (Some(a), Some(b)) => (*a, *b), _ => continue };
            conn.execute(
                "INSERT OR IGNORE INTO routine_edges_new
                 (id, chain_id, from_node_id, to_node_id, trigger_type, trigger_value, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![eid, ncid, nf, nt, tt, tv, ca, norm(ca)],
            )?;
            n_edges += 1;
        }

        // runs (preserve state/completed_at; updated_at from real timestamp so LWW works)
        let mut stmt_r = conn.prepare(
            "SELECT id, chain_id, date, slot, state, started_at, completed_at FROM routine_runs")?;
        let run_rows: Vec<(i64, i64, String, String, String, String, Option<String>)> =
            stmt_r.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get(4)?, r.get(5)?, r.get(6)?,
            )))?.filter_map(Result::ok).collect();
        drop(stmt_r);
        let mut n_runs = 0;
        for (old, cid, date, slot, state, started, completed) in &run_rows {
            let nid = match run_map.get(old) { Some(n) => *n, None => continue };
            let ncid = match chain_map.get(cid) { Some(c) => *c, None => continue };
            let ua = norm(completed.as_deref().unwrap_or(started));
            conn.execute(
                "INSERT OR IGNORE INTO routine_runs_new
                 (id, chain_id, date, slot, state, started_at, completed_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![nid, ncid, date, slot, state, started, completed, ua],
            )?;
            n_runs += 1;
        }

        // node_status (the actual completions; skip rows whose run/node didn't map)
        let mut stmt_s = conn.prepare(
            "SELECT id, run_id, node_id, state, updated_at FROM routine_node_status")?;
        let stat_rows: Vec<(i64, i64, i64, String, String)> =
            stmt_s.query_map([], |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            )))?.filter_map(Result::ok).collect();
        drop(stmt_s);
        let mut n_stat = 0;
        for (_old, rid, nid, state, ua) in &stat_rows {
            let nrid = match run_map.get(rid) { Some(r) => *r, None => continue };
            let nnid = match node_map.get(nid) { Some(n) => *n, None => continue };
            let new_sid = crate::types::routine_node_status_id(nrid, nnid);
            conn.execute(
                "INSERT OR IGNORE INTO routine_node_status_new
                 (id, run_id, node_id, state, updated_at) VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![new_sid, nrid, nnid, state, norm(ua)],
            )?;
            n_stat += 1;
        }

        conn.execute_batch(
            "DROP TABLE routine_node_status;
             DROP TABLE routine_edges;
             DROP TABLE routine_runs;
             DROP TABLE routine_nodes;
             DROP TABLE routine_chains;
             ALTER TABLE routine_chains_new RENAME TO routine_chains;
             ALTER TABLE routine_nodes_new RENAME TO routine_nodes;
             ALTER TABLE routine_edges_new RENAME TO routine_edges;
             ALTER TABLE routine_runs_new RENAME TO routine_runs;
             ALTER TABLE routine_node_status_new RENAME TO routine_node_status;
             CREATE INDEX IF NOT EXISTS idx_routine_nodes_chain ON routine_nodes(chain_id);
             CREATE INDEX IF NOT EXISTS idx_routine_edges_chain ON routine_edges(chain_id);
             CREATE INDEX IF NOT EXISTS idx_routine_runs_date ON routine_runs(date);
             CREATE INDEX IF NOT EXISTS idx_routine_node_status_run ON routine_node_status(run_id);
             DELETE FROM sync_tombstones WHERE table_name IN
               ('routine_chains','routine_nodes','routine_edges','routine_runs','routine_node_status');
             INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_ids_deterministic_v2');
             COMMIT;"
        )?;
        Ok((n_chains, n_nodes, n_edges, n_runs, n_stat))
    })();
    let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");
    match result {
        Ok((c, n, e, r, s)) => eprintln!(
            "[migrate_routine_ids_deterministic_v2] re-keyed {c} chains, {n} nodes, {e} edges, {r} runs, {s} statuses"),
        Err(err) => {
            eprintln!("[migrate_routine_ids_deterministic_v2] failed: {err}");
            let _ = conn.execute_batch("ROLLBACK;");
        }
    }
}

/// One-time data fix after the v1.0.6 routine-graph converge: two seed chains
/// had been renamed on the Mac («Покушать»→«Еда», «Ночь»→«Вечер»), so the
/// content-keyed converge (see migrate_routine_ids_deterministic_v2) brought
/// the other device's fresh seeds in as duplicates, and «Рефлексия» ended up
/// holding BOTH its old linear edge chain and the new seed's star fan-out.
/// Drops the duplicate chains, keeps the linear reflection (deletes the star
/// edges, chains the four new-seed-only nodes onto the tail) and removes
/// 2099-dated test runs. Everything is keyed by deterministic ids, so each
/// device converges to the same result on its own — tombstone push was broken
/// until v1.0.8, so cross-device delete propagation can't be relied on here.
pub fn migrate_routine_dedup_cleanup(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare("SELECT 1 FROM _migrations WHERE name='routine_dedup_cleanup_v1'").ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok()).is_some();
    if done { return; }
    let det = crate::types::deterministic_id;

    // Duplicate chains — explicit bottom-up deletes so every row fires its
    // tombstone trigger (FK cascades are off on this connection).
    for title in ["Ночь", "Покушать"] {
        let cid = det(&format!("chain:{}", title));
        let _ = conn.execute(
            "DELETE FROM routine_node_status
              WHERE node_id IN (SELECT id FROM routine_nodes WHERE chain_id=?1)
                 OR run_id  IN (SELECT id FROM routine_runs  WHERE chain_id=?1)",
            rusqlite::params![cid]);
        let _ = conn.execute("DELETE FROM routine_runs  WHERE chain_id=?1", rusqlite::params![cid]);
        let _ = conn.execute("DELETE FROM routine_edges WHERE chain_id=?1", rusqlite::params![cid]);
        let _ = conn.execute("DELETE FROM routine_nodes WHERE chain_id=?1", rusqlite::params![cid]);
        let _ = conn.execute("DELETE FROM routine_chains WHERE id=?1", rusqlite::params![cid]);
    }

    // «Рефлексия» → linear: drop the star fan-out, i.e. every start→X edge
    // except the linear chain's entry edge start→Pattern Interrupt.
    let r = det("chain:Рефлексия");
    let nid = |title: &str| det(&format!("node:c{}:{}", r, title));
    let start = nid("Подведу день");
    let _ = conn.execute(
        "DELETE FROM routine_edges WHERE chain_id=?1 AND from_node_id=?2 AND to_node_id<>?3",
        rusqlite::params![r, start, nid("Pattern Interrupt")]);

    // The four nodes that only exist in the new seed get appended to the tail
    // of the linear chain so they stay part of the one-at-a-time flow.
    let tail = ["Выспался 7ч+", "Без сладкого",
                "Contemplation (Dan Koe)", "Vision (Dan Koe)", "Integration (Dan Koe)"];
    for w in tail.windows(2) {
        let (from, to) = (nid(w[0]), nid(w[1]));
        let eid = det(&format!("edge:c{}:{}>{}", r, from, to));
        let _ = conn.execute(
            "INSERT OR IGNORE INTO routine_edges (id, chain_id, from_node_id, to_node_id)
             VALUES (?1,?2,?3,?4)",
            rusqlite::params![eid, r, from, to]);
    }

    // Test runs parked on 2099 dates.
    let _ = conn.execute(
        "DELETE FROM routine_node_status WHERE run_id IN
           (SELECT id FROM routine_runs WHERE date >= '2099')", []);
    let _ = conn.execute("DELETE FROM routine_runs WHERE date >= '2099'", []);

    // The v1.0.8 tombstone-push fix starts from this cursor; park it at "now"
    // so the multi-year tombstone backlog (52K+ rows that never pushed while
    // the reader was broken) doesn't flood the first push. Format matches the
    // tombstone triggers' strftime('%Y-%m-%dT%H:%M:%f','now','localtime').
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
    for key in ["cloud_owner_gh_push_tombstones", "cloud_owner_v2_push_tombstones"] {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, now]);
    }

    let _ = conn.execute("INSERT OR IGNORE INTO _migrations (name) VALUES ('routine_dedup_cleanup_v1')", []);
    eprintln!("[migrate_routine_dedup_cleanup] duplicate chains dropped, reflection back to linear");
}

/// Orphan backfill for installs that ran migrate_schedules_to_uuid_pk BEFORE
/// the routine_nodes remap was added inside it: schedules now have UUID ids
/// but routine_nodes.source_id still holds the old INTEGER ids that no longer
/// match anything. Without this, routine→schedule mirroring in
/// set_routine_node_status falls back to a title heuristic, which silently
/// fails when two schedules share a substring (e.g. "Зубы утром" / "Зубы
/// вечером"). Idempotent: skips rows whose source_id is already TEXT.
pub fn backfill_routine_nodes_source_id(conn: &rusqlite::Connection) {
    // Only relevant if both tables exist and schedules is already UUID-typed.
    if !column_is_text(conn, "schedules", "id") { return; }
    let has_routine: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='routine_nodes'",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if has_routine == 0 { return; }

    // Find nodes whose source_id is still stored as INTEGER — those are the
    // orphans. typeof() reports the per-row storage class, so post-migration
    // remapped rows (stored as TEXT) are skipped automatically.
    let orphans: Vec<(i64, String)> = match conn.prepare(
        "SELECT id, title FROM routine_nodes
         WHERE source_type='schedule' AND source_id IS NOT NULL
           AND typeof(source_id)='integer'"
    ) {
        Ok(mut stmt) => stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(|rs| rs.filter_map(|r| r.ok()).collect()).unwrap_or_default(),
        Err(_) => return,
    };
    if orphans.is_empty() { return; }

    // Snapshot active schedules so the title lookup is Rust-side (Unicode-
    // aware lowercasing — SQLite LOWER() is ASCII-only and would miss
    // Cyrillic). Same matching strategy as routine_engine's runtime fallback:
    // exact lowercase match, then unambiguous substring.
    let scheds: Vec<(String, String)> = match conn.prepare(
        "SELECT id, title FROM schedules WHERE is_active = 1"
    ) {
        Ok(mut stmt) => stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map(|rs| rs.filter_map(|r| r.ok())
                .map(|(i, t)| (i, t.to_lowercase())).collect())
            .unwrap_or_default(),
        Err(_) => return,
    };

    let mut fixed = 0i64;
    let mut cleared = 0i64;
    for (rn_id, title) in &orphans {
        let want = title.to_lowercase();
        let exact = scheds.iter().find(|(_, t)| *t == want).map(|(i, _)| i.clone());
        let resolved = if let Some(uuid) = exact {
            Some(uuid)
        } else {
            let subs: Vec<&String> = scheds.iter()
                .filter(|(_, t)| t.contains(&want) || want.contains(t))
                .map(|(i, _)| i).collect();
            if subs.len() == 1 { Some(subs[0].clone()) } else { None }
        };
        match resolved {
            Some(uuid) => {
                if conn.execute(
                    "UPDATE routine_nodes SET source_id=?1 WHERE id=?2",
                    rusqlite::params![uuid, rn_id],
                ).is_ok() { fixed += 1; }
            }
            None => {
                // No safe match — clear the dangling reference so future
                // resolves don't try to use a stale INTEGER id again.
                if conn.execute(
                    "UPDATE routine_nodes SET source_id=NULL WHERE id=?1",
                    rusqlite::params![rn_id],
                ).is_ok() { cleared += 1; }
            }
        }
    }
    eprintln!("[backfill_routine_nodes_source_id] fixed {fixed}, cleared {cleared} orphans");
}

/// Collapse schedules with identical (lowercased) titles into a single canonical
/// row, remap their completions, and tombstone the losers so the deletion
/// propagates across LAN sync. Needed because Phase 3 migrated Mac and phone
/// independently — each device generated its own UUIDv7 for the same logical
/// schedule, and a subsequent LAN exchange creates two rows where there should
/// be one.
///
/// Canonical = id with smallest lex order: UUIDv7 sorts by generation time, so
/// the device that migrated earliest (Mac) wins automatically — matches the
/// user's "Mac is authority" decision without needing peer metadata.
///
/// Completion merge for overlapping dates: canonical wins (loser's row is
/// dropped). For non-overlapping dates, loser's completion is remapped to
/// canonical's schedule_id. Idempotent — when no two active schedules share a
/// lowercased title, no rows are touched.
pub fn dedup_schedules_by_title(conn: &rusqlite::Connection) -> (usize, usize) {
    use std::collections::HashMap;
    if !column_is_text(conn, "schedules", "id") { return (0, 0); }

    // Includes inactive schedules: archived rows still sync via SYNC_TABLES,
    // so a Phase 3 migration collision can leave two soft-deleted copies of
    // the same logical row — collapse those too. Completion remap is safe
    // for inactive parents (no UI ever shows them).
    let groups: Vec<(String, Vec<String>)> = match conn.prepare(
        "SELECT lower(title), id FROM schedules ORDER BY id"
    ) {
        Ok(mut stmt) => {
            let rows: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rs| rs.filter_map(|r| r.ok()).collect()).unwrap_or_default();
            let mut g: HashMap<String, Vec<String>> = HashMap::new();
            for (t, id) in rows { g.entry(t).or_default().push(id); }
            g.into_iter().filter(|(_, v)| v.len() > 1).collect()
        }
        Err(_) => return (0, 0),
    };
    if groups.is_empty() { return (0, 0); }

    let now = chrono::Local::now().to_rfc3339();
    let mut deleted = 0usize;
    let mut remapped = 0usize;

    for (_title, ids) in groups {
        let canonical = &ids[0]; // smallest lex = oldest UUIDv7
        for loser in ids.iter().skip(1) {
            // 1) Drop loser completions whose date is already covered by
            //    canonical (canonical wins on conflict).
            let _ = conn.execute(
                "DELETE FROM schedule_completions
                 WHERE schedule_id=?1
                   AND date IN (SELECT date FROM schedule_completions WHERE schedule_id=?2)",
                rusqlite::params![loser, canonical],
            );
            // 2) Remap remaining loser completions to canonical.
            let r = conn.execute(
                "UPDATE schedule_completions
                    SET schedule_id=?1, updated_at=?2
                  WHERE schedule_id=?3",
                rusqlite::params![canonical, now, loser],
            ).unwrap_or(0);
            remapped += r;
            // 3) Tombstone the loser so other devices delete their copy.
            let _ = conn.execute(
                "INSERT OR REPLACE INTO sync_tombstones (table_name, row_id, deleted_at)
                 VALUES ('schedules', ?1, ?2)",
                rusqlite::params![loser, now],
            );
            // 4) Delete the duplicate schedule locally.
            let d = conn.execute(
                "DELETE FROM schedules WHERE id=?1",
                rusqlite::params![loser],
            ).unwrap_or(0);
            deleted += d;
        }
    }

    if deleted > 0 || remapped > 0 {
        eprintln!("[dedup_schedules_by_title] deleted {deleted} dup schedules, remapped {remapped} completions");
    }
    (deleted, remapped)
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
            GROUP BY date, type, COALESCE(start_time,''), value, notes
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

/// health_log used to drop the per-session start time from Health Connect,
/// so every walking/exercise row landed at the default 12:00 slot. Add a
/// start_time TEXT column ("HH:MM") so import_exercise can persist the real
/// start, and sync_health_to_calendar/timeline can use it. Idempotent (the
/// ALTER fails silently if the column already exists).
pub fn migrate_health_log_start_time(conn: &rusqlite::Connection) {
    let _ = conn.execute(
        "ALTER TABLE health_log ADD COLUMN start_time TEXT DEFAULT ''",
        [],
    );
}

/// One-time-per-launch cleanup: an earlier sync_health_to_calendar did
/// DELETE+INSERT on every poll, so LAN-sync ended up with stale tombstones
/// and the receiver accumulated duplicate Sleep/Exercise events. Collapse
/// to one row per (date, title, time, duration_minutes) on both phone and
/// Mac. Idempotent.
pub fn migrate_dedup_auto_health_events(conn: &rusqlite::Connection) {
    // Same start_time with different duration_minutes is the same session
    // re-imported with a corrected length — drop the older row (smaller id)
    // and keep the latest reading. Idempotent.
    conn.execute(
        "DELETE FROM events WHERE source='auto_health' AND id NOT IN (
            SELECT MAX(id) FROM events WHERE source='auto_health'
            GROUP BY date, title, time
        )",
        [],
    ).ok();
}

/// Repair data produced by the old Health Connect importer. It recreated
/// sleep stages and derived timeline rows on every poll, while LAN tombstones
/// could not represent their TEXT ids. Run once on every device before normal
/// sync resumes; stable natural-key indexes prevent the pile-up returning.
pub fn migrate_health_sync_cleanup_v1(conn: &rusqlite::Connection) {
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);
    let done = conn.prepare(
        "SELECT 1 FROM _migrations WHERE name='health_sync_cleanup_v1'"
    ).ok().and_then(|mut s| s.exists([]).ok()).unwrap_or(false);
    if done { return; }

    // Same-day sleep can be a separate nap. Retire an unstaged legacy row only
    // when the staged row has the same interval and user-visible values.
    // Existing tombstones have no cleanup provenance: preserve them so genuine
    // source deletions can still reach devices that were offline.
    let result = conn.execute_batch(
        "BEGIN IMMEDIATE;
         DROP TRIGGER IF EXISTS sleep_stages_tombstone;
         DROP TRIGGER IF EXISTS sleep_sessions_tombstone;
         DROP TRIGGER IF EXISTS health_log_tombstone;
         DROP TRIGGER IF EXISTS timeline_blocks_tombstone;
         DROP TRIGGER IF EXISTS events_tombstone;

         DELETE FROM sleep_stages WHERE id NOT IN (
           SELECT MIN(id) FROM sleep_stages
           GROUP BY session_id,start_time,end_time,stage
         );
         DELETE FROM sleep_sessions
         WHERE source='health_connect'
           AND NOT EXISTS (SELECT 1 FROM sleep_stages WHERE session_id=sleep_sessions.id)
           AND EXISTS (
             SELECT 1 FROM sleep_sessions AS other
             WHERE other.date=sleep_sessions.date AND other.source='health_connect'
               AND other.id<>sleep_sessions.id
               AND length(trim(sleep_sessions.start_time))>0
               AND length(trim(sleep_sessions.end_time))>0
               AND other.start_time=sleep_sessions.start_time
               AND other.end_time=sleep_sessions.end_time
               AND other.duration_minutes=sleep_sessions.duration_minutes
               AND other.notes IS sleep_sessions.notes
               AND other.quality_score IS sleep_sessions.quality_score
               AND EXISTS (SELECT 1 FROM sleep_stages WHERE session_id=other.id)
           );
         DELETE FROM health_log WHERE health_log.type='steps' AND EXISTS (
           SELECT 1 FROM health_log AS b WHERE b.type='steps' AND b.date=health_log.date
             AND (COALESCE(b.updated_at,'')>COALESCE(health_log.updated_at,'') OR
                  (COALESCE(b.updated_at,'')=COALESCE(health_log.updated_at,'') AND b.id<health_log.id))
         );
         DELETE FROM health_log WHERE health_log.type='exercise' AND EXISTS (
           SELECT 1 FROM health_log AS b WHERE b.type='exercise' AND b.date=health_log.date
             AND COALESCE(b.start_time,'')=COALESCE(health_log.start_time,'') AND b.notes=health_log.notes
             AND (COALESCE(b.updated_at,'')>COALESCE(health_log.updated_at,'') OR
                  (COALESCE(b.updated_at,'')=COALESCE(health_log.updated_at,'') AND b.id<health_log.id))
         );
         DELETE FROM timeline_blocks WHERE timeline_blocks.source='auto_health' AND EXISTS (
           SELECT 1 FROM timeline_blocks AS b WHERE b.source='auto_health'
             AND b.date=timeline_blocks.date AND b.type_id=timeline_blocks.type_id AND b.start_time=timeline_blocks.start_time
             AND COALESCE(b.notes,'')=COALESCE(timeline_blocks.notes,'') AND b.id<timeline_blocks.id
         );
         DELETE FROM events WHERE events.source='auto_health' AND EXISTS (
           SELECT 1 FROM events AS b WHERE b.source='auto_health'
             AND b.date=events.date AND b.title=events.title AND b.time=events.time AND b.id>events.id
         );

         CREATE UNIQUE INDEX IF NOT EXISTS uq_sleep_stage_natural
           ON sleep_stages(session_id,start_time,end_time,stage);
         CREATE UNIQUE INDEX IF NOT EXISTS uq_health_steps_date
           ON health_log(date) WHERE type='steps';
         CREATE UNIQUE INDEX IF NOT EXISTS uq_health_exercise_natural
           ON health_log(date,COALESCE(start_time,''),notes) WHERE type='exercise';
         INSERT OR IGNORE INTO _migrations(name) VALUES ('health_sync_cleanup_v1');
         COMMIT;"
    );
    if let Err(e) = result {
        let _ = conn.execute_batch("ROLLBACK;");
        eprintln!("[migrate_health_sync_cleanup_v1] failed: {e}");
    }
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

#[cfg(test)]
#[path = "health_sync_cleanup_tests.rs"]
mod health_sync_cleanup_tests;

#[cfg(test)]
mod automation_log_security_tests {
    use super::*;
    use std::path::Path;

    fn create_legacy_schema(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "PRAGMA user_version=10;
             CREATE TABLE automation_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                script_hash TEXT NOT NULL,
                script_preview TEXT NOT NULL DEFAULT '',
                success INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_automation_log_ts ON automation_log(ts);",
        )
        .expect("create legacy automation log");
    }

    fn create_metadata_schema(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "CREATE TABLE automation_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                script_hash TEXT NOT NULL,
                success INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("create metadata-only automation log");
    }

    fn create_scrub_marker(conn: &rusqlite::Connection, state: &str) {
        conn.execute_batch(
            "CREATE TABLE _hanni_security_migrations (
                name TEXT PRIMARY KEY,
                state TEXT NOT NULL CHECK(state IN ('pending', 'complete'))
             );",
        )
        .expect("create security marker table");
        conn.execute(
            "INSERT INTO _hanni_security_migrations(name, state) VALUES (?1, ?2)",
            rusqlite::params![AUTOMATION_LOG_SCRUB_KEY, state],
        )
        .expect("seed scrub marker");
    }

    fn scrub_marker(conn: &rusqlite::Connection) -> String {
        conn.query_row(
            "SELECT state FROM _hanni_security_migrations WHERE name=?1",
            [AUTOMATION_LOG_SCRUB_KEY],
            |row| row.get(0),
        )
        .expect("read scrub marker")
    }

    fn sqlite_artifacts(path: &Path) -> [std::path::PathBuf; 3] {
        [
            path.to_path_buf(),
            path.with_extension("db-wal"),
            path.with_extension("db-shm"),
        ]
    }

    fn artifact_contains(path: &Path, needle: &[u8]) -> bool {
        sqlite_artifacts(path)
            .iter()
            .any(|candidate| file_contains(candidate, needle))
    }

    fn file_contains(path: &Path, needle: &[u8]) -> bool {
        std::fs::read(path)
            .map(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
            .unwrap_or(false)
    }

    fn assert_artifacts_do_not_contain(path: &Path, needle: &[u8]) {
        for candidate in sqlite_artifacts(path) {
            if !candidate.exists() {
                continue;
            }
            let bytes = std::fs::read(&candidate).expect("read SQLite artifact");
            assert!(
                !bytes.windows(needle.len()).any(|window| window == needle),
                "sensitive preview remained in {}",
                candidate.display()
            );
        }
    }

    #[test]
    fn missing_table_gets_metadata_schema_and_one_time_physical_scrub() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("missing-table.db");
        let prefix = b"hanni-absent-table-preview-canary-1432";
        {
            let conn = rusqlite::Connection::open(&path).expect("open database");
            conn.pragma_update(None, "secure_delete", "OFF")
                .expect("disable legacy secure delete");
            conn.execute_batch("CREATE TABLE discarded_preview(value TEXT NOT NULL);")
                .expect("create discarded table");
            let payload = format!("{}{}", String::from_utf8_lossy(prefix), "x".repeat(65_536));
            conn.execute("INSERT INTO discarded_preview(value) VALUES (?1)", [&payload])
                .expect("seed discarded preview");
            conn.execute_batch("DROP TABLE discarded_preview;")
                .expect("drop discarded table");
        }
        assert!(
            artifact_contains(&path, prefix),
            "fixture must contain dropped-table residue"
        );

        {
            let conn = rusqlite::Connection::open(&path).expect("reopen database");
            migrate_automation_log(&conn).expect("create and scrub automation log");
            assert_eq!(
                table_xinfo_in(&conn, "automation_log").expect("read metadata schema"),
                automation_log_metadata_schema()
            );
            assert_eq!(scrub_marker(&conn), AUTOMATION_LOG_SCRUB_COMPLETE);
            migrate_automation_log(&conn).expect("migration is idempotent");
        }
        assert_artifacts_do_not_contain(&path, prefix);
    }

    #[test]
    fn legacy_v10_rebuild_preserves_metadata_and_scrubs_db_wal_and_shm() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("legacy-wal.db");
        let canary = b"hanni-legacy-script-preview-canary-8825";
        let conn = rusqlite::Connection::open(&path).expect("open database");
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("enable WAL");
        conn.pragma_update(None, "secure_delete", "OFF")
            .expect("disable legacy secure delete");
        create_legacy_schema(&conn);
        let user_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read legacy user_version");
        assert_eq!(user_version, 10, "fixture must represent an already-v10 install");
        conn.execute(
            "INSERT INTO automation_log(ts, script_hash, script_preview, success, duration_ms)
             VALUES (123, 'known-action-hash', ?1, 1, 17)",
            [String::from_utf8_lossy(canary).as_ref()],
        )
        .expect("seed legacy automation row");
        assert!(
            file_contains(&path.with_extension("db-wal"), canary),
            "fixture must place the preview specifically in the WAL"
        );

        migrate_automation_log(&conn).expect("migrate legacy automation log");
        assert_eq!(
            table_xinfo_in(&conn, "automation_log").expect("read metadata schema"),
            automation_log_metadata_schema()
        );
        let row: (i64, String, i64, i64) = conn
            .query_row(
                "SELECT ts, script_hash, success, duration_ms FROM automation_log",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read preserved metadata");
        assert_eq!(row, (123, "known-action-hash".into(), 1, 17));
        assert_eq!(scrub_marker(&conn), AUTOMATION_LOG_SCRUB_COMPLETE);
        assert_artifacts_do_not_contain(&path, canary);
    }

    #[test]
    fn migration_scrubs_historical_preview_from_freelist() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("historical-preview.db");
        let prefix = b"hanni-freelist-script-preview-canary-3364";
        {
            let conn = rusqlite::Connection::open(&path).expect("open database");
            conn.pragma_update(None, "secure_delete", "OFF")
                .expect("disable legacy secure delete");
            create_legacy_schema(&conn);
            let historical = format!(
                "{}{}",
                String::from_utf8_lossy(prefix),
                "y".repeat(65_536)
            );
            conn.execute(
                "INSERT INTO automation_log(ts, script_hash, script_preview, success, duration_ms)
                 VALUES (1, 'old-hash', ?1, 0, 5)",
                [&historical],
            )
            .expect("seed historical preview");
            conn.execute(
                "UPDATE automation_log SET script_preview='replaced' WHERE id=1",
                [],
            )
            .expect("replace historical preview");
        }
        assert!(
            artifact_contains(&path, prefix),
            "fixture must retain historical preview bytes"
        );

        {
            let conn = rusqlite::Connection::open(&path).expect("reopen database");
            migrate_automation_log(&conn).expect("scrub historical preview");
            assert_eq!(scrub_marker(&conn), AUTOMATION_LOG_SCRUB_COMPLETE);
        }
        assert_artifacts_do_not_contain(&path, prefix);
    }

    #[test]
    fn pending_marker_resumes_scrub_after_rebuild_before_vacuum() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("pending-scrub.db");
        let prefix = b"hanni-pending-scrub-canary-9107";
        {
            let conn = rusqlite::Connection::open(&path).expect("open database");
            conn.pragma_update(None, "secure_delete", "OFF")
                .expect("disable legacy secure delete");
            create_metadata_schema(&conn);
            create_scrub_marker(&conn, AUTOMATION_LOG_SCRUB_PENDING);
            conn.execute_batch("CREATE TABLE discarded_preview(value TEXT NOT NULL);")
                .expect("create discarded preview table");
            let payload = format!("{}{}", String::from_utf8_lossy(prefix), "z".repeat(65_536));
            conn.execute("INSERT INTO discarded_preview(value) VALUES (?1)", [&payload])
                .expect("seed discarded preview");
            conn.execute_batch("DROP TABLE discarded_preview;")
                .expect("drop discarded preview table");
        }
        assert!(artifact_contains(&path, prefix), "fixture must contain residue");

        {
            let conn = rusqlite::Connection::open(&path).expect("reopen database");
            migrate_automation_log(&conn).expect("resume pending scrub");
            assert_eq!(scrub_marker(&conn), AUTOMATION_LOG_SCRUB_COMPLETE);
        }
        assert_artifacts_do_not_contain(&path, prefix);
    }

    #[test]
    fn pending_marker_after_vacuum_completes_and_is_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().expect("open database");
        create_metadata_schema(&conn);
        create_scrub_marker(&conn, AUTOMATION_LOG_SCRUB_PENDING);
        conn.execute_batch("VACUUM;").expect("simulate completed vacuum");

        migrate_automation_log(&conn).expect("finish pending marker");
        assert_eq!(scrub_marker(&conn), AUTOMATION_LOG_SCRUB_COMPLETE);
        migrate_automation_log(&conn).expect("completed migration is idempotent");
    }

    #[test]
    fn unknown_schema_fails_closed_without_completion_marker() {
        let conn = rusqlite::Connection::open_in_memory().expect("open database");
        conn.execute_batch(
            "CREATE TABLE automation_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                script_hash BLOB NOT NULL,
                success INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("create corrupt schema");

        let error = migrate_automation_log(&conn).expect_err("unknown schema must fail");
        assert!(error.contains("unexpected automation_log schema"));
        assert!(!table_exists_in(&conn, "_hanni_security_migrations")
            .expect("inspect rolled-back marker table"));
        assert_eq!(
            conn.query_row(
                "SELECT type FROM pragma_table_xinfo('automation_log') WHERE name='script_hash'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read preserved corrupt schema"),
            "BLOB"
        );
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod migration_copy_tests {
    use super::*;

    #[test]
    fn recursive_retry_never_overwrites_existing_destination_files() {
        let source = tempfile::tempdir().expect("source temp dir");
        let destination = tempfile::tempdir().expect("destination temp dir");
        let source_nested = source.path().join("nested");
        let destination_nested = destination.path().join("nested");
        std::fs::create_dir(&source_nested).expect("create source nested dir");
        std::fs::create_dir(&destination_nested).expect("create destination nested dir");
        std::fs::write(source_nested.join("existing.txt"), b"legacy")
            .expect("write legacy existing file");
        std::fs::write(destination_nested.join("existing.txt"), b"current")
            .expect("write current destination file");
        std::fs::write(source_nested.join("missing.txt"), b"copy me")
            .expect("write legacy missing file");

        copy_dir_recursive(source.path(), destination.path()).expect("retry copy");

        assert_eq!(
            std::fs::read(destination_nested.join("existing.txt"))
                .expect("read existing destination"),
            b"current"
        );
        assert_eq!(
            std::fs::read(destination_nested.join("missing.txt"))
                .expect("read copied destination"),
            b"copy me"
        );
    }
}
