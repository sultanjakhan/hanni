// proactive/triggers.rs — Context gathering functions and trigger detection
use crate::types::*;
use crate::macos::{run_osascript, check_calendar_access, classify_app};
use chrono::Timelike;

// ── Reusable OS-context helpers (used by both gather_context and snapshot collector) ──

pub fn get_frontmost_app() -> String {
    let name = run_osascript(
        "tell application \"System Events\" to return name of first application process whose frontmost is true"
    ).unwrap_or_default().trim().to_string();
    // Tauri WebView reports as "Electron" on macOS — map to real app name
    if name == "Electron" { "Hanni".to_string() } else { name }
}

pub fn get_browser_url() -> String {
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

pub fn get_window_title() -> String {
    run_osascript(
        "tell application \"System Events\" to return name of front window of first application process whose frontmost is true"
    ).unwrap_or_default().trim().to_string()
}

pub fn classify_activity(app: &str, url: &str, title: &str) -> &'static str {
    let app_l = app.to_lowercase();
    let url_l = url.to_lowercase();
    let title_l = title.to_lowercase();

    // Coding
    if ["code", "cursor", "xcode", "iterm", "terminal", "warp", "neovim", "vim", "jetbrains", "intellij", "pycharm", "webstorm"]
        .iter().any(|k| app_l.contains(k)) {
        return "coding";
    }
    if url_l.contains("github.com") || url_l.contains("stackoverflow.com") || url_l.contains("gitlab.com") {
        return "coding";
    }

    // Social media / distraction
    if ["twitter", "x.com", "reddit.com", "tiktok", "instagram", "vk.com", "facebook"]
        .iter().any(|k| url_l.contains(k) || app_l.contains(k)) {
        return "social";
    }

    // Media / entertainment
    if ["youtube", "netflix", "twitch", "crunchyroll", "kinopoisk", "spotify"]
        .iter().any(|k| url_l.contains(k) || app_l.contains(k) || title_l.contains(k)) {
        return "media";
    }
    if app_l == "music" || app_l == "spotify" || app_l == "vlc" || app_l == "iina" {
        return "media";
    }

    // Communication
    if ["telegram", "discord", "slack", "zoom", "facetime", "whatsapp", "mail", "messages"]
        .iter().any(|k| app_l.contains(k)) {
        return "communication";
    }

    // Writing / productivity
    if ["notion", "obsidian", "notes", "pages", "word", "google docs"]
        .iter().any(|k| app_l.contains(k) || title_l.contains(k)) {
        return "writing";
    }

    // Browsing (browser but not matched above)
    if ["safari", "chrome", "arc", "firefox", "brave", "edge"]
        .iter().any(|k| app_l.contains(k)) {
        return "browsing";
    }

    // Reading (specific patterns)
    if url_l.contains("chatgpt.com") || url_l.contains("claude.ai") || url_l.contains("docs.") {
        return "learning";
    }

    "other"
}

pub fn get_now_playing_sync() -> String {
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

pub async fn gather_context() -> String {
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

pub fn gather_context_blocking() -> String {
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

    // Active (frontmost) app — only show for distraction alerts
    // Skip "Hanni" — no point telling the model the user is in our own app
    let front_app = get_frontmost_app();
    if !front_app.is_empty() && front_app != "Hanni" {
        let distracting = ["YouTube", "Reddit", "Twitter", "TikTok", "Instagram", "Telegram", "Discord", "VK"];
        let is_distracting = distracting.iter().any(|d| front_app.contains(d));
        if is_distracting {
            if let Ok(minutes) = get_app_focus_minutes(&front_app) {
                if minutes > 30.0 {
                    ctx.push_str(&format!("\n--- ⚠ Дистракция ---\n{}: {:.0} мин (30+ мин — залип!)\n", front_app, minutes));
                }
            }
        }
        // For non-distracting apps, don't show app name or time — the model fixates on it
    }

    // Upcoming events within next 60 min (for schedule reminders)
    if let Ok(upcoming) = get_upcoming_events_soon() {
        if !upcoming.is_empty() {
            ctx.push_str(&format!("\n--- Coming Up Soon ---\n{}\n", upcoming));
        }
    }

    // Morning digest context: yesterday's mood, sleep, today's event count, goals
    let hour = now.hour();
    if hour >= 8 && hour <= 10 {
        if let Ok(digest) = gather_morning_digest() {
            ctx.push_str(&format!("\n--- Morning Digest Data ---\n{}\n", digest));
        }
    }

    // Evening reflection context (21-23)
    if hour >= 21 && hour <= 23 {
        if let Ok(evening) = gather_evening_context() {
            ctx.push_str(&format!("\n--- Evening Reflection Data ---\n{}\n", evening));
        }
    }

    ctx
}

pub fn gather_evening_context() -> Result<String, String> {
    let db_path = hanni_db_path();
    if !db_path.exists() { return Ok(String::new()); }
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    let mut ctx = String::new();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Tasks completed today
    let completed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE date(completed_at) = ?1 AND project_id NOT IN (SELECT id FROM projects WHERE name = 'Вакансии')",
        rusqlite::params![&today], |row| row.get(0),
    ).unwrap_or(0);
    if completed > 0 {
        ctx.push_str(&format!("Tasks completed today: {}\n", completed));
    }

    // Mood logged today?
    let mood_logged: bool = conn.query_row(
        "SELECT COUNT(*) FROM mood_log WHERE date = ?1",
        rusqlite::params![&today], |row| row.get::<_, i64>(0),
    ).unwrap_or(0) > 0;
    ctx.push_str(&format!("Mood logged today: {}\n", if mood_logged { "yes" } else { "no" }));

    // Workouts today
    let workouts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM workouts WHERE date = ?1",
        rusqlite::params![&today], |row| row.get(0),
    ).unwrap_or(0);
    if workouts > 0 {
        ctx.push_str(&format!("Workouts today: {}\n", workouts));
    }

    // Journal entry today?
    let journal: bool = conn.query_row(
        "SELECT COUNT(*) FROM journal_entries WHERE date = ?1",
        rusqlite::params![&today], |row| row.get::<_, i64>(0),
    ).unwrap_or(0) > 0;
    ctx.push_str(&format!("Journal today: {}\n", if journal { "yes" } else { "no" }));

    Ok(ctx)
}

pub fn get_app_focus_minutes(app_name: &str) -> Result<f64, String> {
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

pub fn get_upcoming_events_soon() -> Result<String, String> {
    let db_path = hanni_db_path();
    if !db_path.exists() { return Ok(String::new()); }
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let current_time = now.format("%H:%M").to_string();
    let soon_time = (now + chrono::Duration::minutes(90)).format("%H:%M").to_string();
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

/// Smart triggers from DB: overdue tasks, near-deadline goals, health gaps
pub fn gather_smart_triggers() -> Vec<String> {
    let db_path = hanni_db_path();
    if !db_path.exists() { return Vec::new(); }
    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut triggers = Vec::new();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // 1. Overdue tasks (due_date < today, not completed) — from both notes and tasks tables
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title, due_date FROM tasks WHERE due_date < ?1 AND due_date != '' AND completed_at IS NULL AND status != 'done' AND project_id NOT IN (SELECT id FROM projects WHERE name = 'Вакансии') ORDER BY due_date DESC LIMIT 3"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![&today], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for r in rows.flatten() {
                triggers.push(format!("Просроченная задача: \"{}\" (дедлайн {})", r.0, r.1));
            }
        }
    }
    // Also check notes with task status
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title, due_date FROM notes WHERE status = 'task' AND due_date < ?1 AND due_date != '' AND archived = 0 ORDER BY due_date DESC LIMIT 3"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![&today], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for r in rows.flatten() {
                triggers.push(format!("Просроченная задача: \"{}\" (дедлайн {})", r.0, r.1));
            }
        }
    }

    // 2. Goals near deadline (<3 days, progress < target)
    let soon = (chrono::Local::now() + chrono::Duration::days(3)).format("%Y-%m-%d").to_string();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title, current_value, target_value, deadline FROM tab_goals WHERE deadline IS NOT NULL AND deadline != '' AND deadline <= ?1 AND deadline >= ?2 AND current_value < target_value AND status = 'active' LIMIT 3"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![&soon, &today], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, f64>(2)?, row.get::<_, String>(3)?))
        }) {
            for r in rows.flatten() {
                triggers.push(format!("Цель на грани дедлайна: \"{}\" — {:.0}/{:.0}, дедлайн {}", r.0, r.1, r.2, r.3));
            }
        }
    }

    // 3. No mood logged today (after 20:00)
    let hour = chrono::Local::now().hour();
    if hour >= 20 {
        let mood_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mood_log WHERE date = ?1",
            rusqlite::params![&today], |row| row.get(0),
        ).unwrap_or(0);
        if mood_today == 0 {
            triggers.push("Настроение сегодня не записано".to_string());
        }
    }

    // 4. No water logged today (after 14:00)
    if hour >= 14 {
        let water_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM health_log WHERE date = ?1 AND type = 'water'",
            rusqlite::params![&today], |row| row.get(0),
        ).unwrap_or(0);
        if water_today == 0 {
            triggers.push("Вода сегодня не записана".to_string());
        }
    }

    triggers
}

pub fn gather_morning_digest() -> Result<String, String> {
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

    // Active goals with progress
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title, current_value, target_value, unit, deadline FROM tab_goals WHERE current_value < target_value AND status = 'active' ORDER BY deadline ASC LIMIT 5"
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        }) {
            let goals: Vec<_> = rows.flatten().collect();
            if !goals.is_empty() {
                digest.push_str(&format!("Active goals: {}\n", goals.len()));
                for (title, current, target, unit, deadline) in &goals {
                    let u = unit.as_deref().unwrap_or("");
                    let dl = deadline.as_deref().unwrap_or("no deadline");
                    digest.push_str(&format!("  - {} ({:.0}/{:.0}{}, {})\n", title, current, target, u, dl));
                }
            }
        }
    }

    // Overdue tasks (from tasks table)
    let overdue_tasks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE due_date < ?1 AND due_date != '' AND completed_at IS NULL AND status != 'done' AND project_id NOT IN (SELECT id FROM projects WHERE name = 'Вакансии')",
        rusqlite::params![today], |row| row.get(0),
    ).unwrap_or(0);
    // Also from notes table
    let overdue_notes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes WHERE status = 'task' AND due_date < ?1 AND due_date != '' AND archived = 0",
        rusqlite::params![today], |row| row.get(0),
    ).unwrap_or(0);
    let overdue = overdue_tasks + overdue_notes;
    if overdue > 0 {
        digest.push_str(&format!("Overdue tasks: {}\n", overdue));
    }

    // Today's tasks
    let mut today_tasks: Vec<String> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title FROM tasks WHERE due_date = ?1 AND completed_at IS NULL AND status != 'done' AND project_id NOT IN (SELECT id FROM projects WHERE name = 'Вакансии') LIMIT 5"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![today], |row| row.get::<_, String>(0)) {
            today_tasks.extend(rows.flatten());
        }
    }
    if let Ok(mut stmt) = conn.prepare(
        "SELECT title FROM notes WHERE status = 'task' AND due_date = ?1 AND archived = 0 LIMIT 5"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![today], |row| row.get::<_, String>(0)) {
            today_tasks.extend(rows.flatten());
        }
    }
    if !today_tasks.is_empty() {
        digest.push_str(&format!("Today's tasks: {}\n", today_tasks.join(", ")));
    }

    Ok(digest)
}

pub fn gather_screen_time() -> Result<String, String> {
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

pub fn compute_activity_delta(old_ctx: &str, new_ctx: &str) -> String {
    let mut deltas = Vec::new();
    // Extract sections from context strings
    pub fn extract_section(ctx: &str, tag: &str) -> String {
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

/// Truncate a UTF-8 string to at most `max_bytes` bytes on a char boundary.
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn get_recent_chat_snippet(conn: &rusqlite::Connection, limit: usize) -> String {
    // Get the latest conversation and extract last N messages
    let messages_json: String = conn.query_row(
        "SELECT messages FROM conversations ORDER BY id DESC LIMIT 1",
        [], |row| row.get(0),
    ).unwrap_or_default();
    if messages_json.is_empty() {
        return String::new();
    }
    // Messages stored as JSON array — handle both old [role, content] and new {role, content} formats
    if let Ok(msgs) = serde_json::from_str::<Vec<serde_json::Value>>(&messages_json) {
        let start = msgs.len().saturating_sub(limit);
        msgs[start..].iter()
            .filter_map(|m| {
                let (role, content) = if let Some(arr) = m.as_array() {
                    // Old format: ["role", "content"]
                    (arr.first().and_then(|v| v.as_str()).unwrap_or("?"),
                     arr.get(1).and_then(|v| v.as_str()).unwrap_or(""))
                } else {
                    // New format: {role, content, ...}
                    (m.get("role").and_then(|v| v.as_str()).unwrap_or("?"),
                     m.get("content").and_then(|v| v.as_str()).unwrap_or(""))
                };
                if role == "tool" { return None; }
                let short = truncate_utf8(content, 150);
                Some(format!("{}: {}", if role == "user" { "User" } else { "Hanni" }, short))
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    }
}

pub fn get_recent_proactive_styles(conn: &rusqlite::Connection, count: usize) -> Vec<String> {
    let mut styles = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT style FROM proactive_history WHERE style != '' ORDER BY id DESC LIMIT ?1"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![count as i64], |row| {
            row.get::<_, String>(0)
        }) {
            for s in rows.flatten() {
                if !styles.contains(&s) {
                    styles.push(s);
                }
            }
        }
    }
    styles
}

pub fn get_todays_proactive_messages(conn: &rusqlite::Connection) -> Vec<(String, String)> {
    // Include yesterday's messages for cross-day anti-repetition
    let yesterday = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let mut msgs = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT message, sent_at FROM proactive_history WHERE sent_at >= ?1 ORDER BY id ASC"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![yesterday], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for pair in rows.flatten() {
                msgs.push(pair);
            }
        }
    }
    msgs
}

pub fn get_user_name_from_memory(conn: &rusqlite::Connection) -> String {
    // Look for user's name in facts table
    conn.query_row(
        "SELECT value FROM facts WHERE category = 'user' AND (key LIKE '%имя%' OR key LIKE '%name%' OR key LIKE '%зовут%') LIMIT 1",
        [], |row| row.get::<_, String>(0),
    ).unwrap_or_default()
}
