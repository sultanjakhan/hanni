// proactive_context.rs — Reusable OS-context helpers (frontmost app, browser URL, activity classification, context gathering, smart triggers, screen time, digests)
use crate::types::*;
use crate::macos::{run_osascript, check_calendar_access, classify_app};
use chrono::Timelike;
use crate::proactive::build_proactive_system_prompt;

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

pub fn gather_context_blocking() -> String {
    let now = chrono::Local::now();
    let mut ctx = format!("Current time: {}\n", now.format("%H:%M %A, %d %B %Y"));

    // Screen Time (SQLite query — fast, no osascript)
    if let Ok(activity) = gather_screen_time() {
        ctx.push_str(&format!("\n--- Screen Time ---\n{}\n", activity));
    }

    // Calendar events from Calendar.app (skip if access was denied or app not running)
    if check_calendar_access() {
        let cal_script = r#"
            -- Only query Calendar if it's already running (don't launch it)
            if application "Calendar" is not running then
                return "Calendar.app not running — skipped"
            end if
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
        "SELECT COUNT(*) FROM tasks WHERE date(completed_at) = ?1",
        rusqlite::params![&today], |row| row.get(0),
    ).unwrap_or(0);
    if completed > 0 {
        ctx.push_str(&format!("Tasks completed today: {}\n", completed));
    }

    // Workouts today
    let workouts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM workouts WHERE date = ?1",
        rusqlite::params![&today], |row| row.get(0),
    ).unwrap_or(0);
    if workouts > 0 {
        ctx.push_str(&format!("Workouts today: {}\n", workouts));
    }

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
        "SELECT title, due_date FROM tasks WHERE due_date < ?1 AND due_date != '' AND completed_at IS NULL AND status != 'done' ORDER BY due_date DESC LIMIT 3"
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

    // 4. No water logged today (after 14:00)
    let hour = chrono::Local::now().hour();
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

    let yesterday = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();

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
        "SELECT COUNT(*) FROM tasks WHERE due_date < ?1 AND due_date != '' AND completed_at IS NULL AND status != 'done'",
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
        "SELECT title FROM tasks WHERE due_date = ?1 AND completed_at IS NULL AND status != 'done' LIMIT 5"
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

pub async fn proactive_llm_call(
    client: &reqwest::Client,
    context: &str,
    _recent_messages: &[(String, chrono::DateTime<chrono::Local>)],
    _consecutive_skips: u32,
    memory_context: &str,
    delta: &str,
    triggers: &[String],
    chat_snippet: &str,
    engagement_rate: f64,
    user_name: &str,
    todays_messages: &[(String, String)],
    enabled_styles: &[String],
    recent_styles: &[String],
) -> Result<Option<String>, String> {
    // Build dynamic system prompt from enabled styles (with time-gating & cooldown)
    let mut sys_prompt = build_proactive_system_prompt(enabled_styles, recent_styles);
    if !user_name.is_empty() {
        sys_prompt = format!(
            "Пользователя зовут {}. Обращайся к нему по имени, на \"ты\".\n\n{}",
            user_name, sys_prompt
        );
    }

    let mut user_content = String::new();

    // Active triggers FIRST (highest priority)
    if !triggers.is_empty() {
        user_content.push_str(&format!("[Триггеры]\n{}\n\n", triggers.join("\n")));
    }

    // Current context (activity, music, browser)
    user_content.push_str(&format!("{}\n", context));

    // Activity delta (what changed)
    if !delta.is_empty() {
        user_content.push_str(&format!("\n[Изменения]\n{}\n", delta));
    }

    // Recent chat (for continuity, last 4 messages)
    if !chat_snippet.is_empty() {
        user_content.push_str(&format!("\n[Последний разговор]\n{}\n", chat_snippet));
    }

    // Memory (only 5 most relevant facts — less noise)
    if !memory_context.is_empty() {
        user_content.push_str(&format!("\n[Память]\n{}\n", memory_context));
    }

    // Anti-repetition: last 5 topics with timestamps
    if !todays_messages.is_empty() {
        let last_n: Vec<_> = todays_messages.iter().rev().take(5).collect();
        user_content.push_str("\n[Уже сказано сегодня]\n");
        for (msg, sent_at) in last_n.iter().rev() {
            let short = truncate_utf8(msg, 60);
            // Extract HH:MM from RFC3339 timestamp
            let hm = if sent_at.len() >= 16 { &sent_at[11..16] } else { "" };
            user_content.push_str(&format!("- {} \"{}\"\n", hm, short));
        }
    }

    // Engagement-adaptive guidance (skip penalty for new users with <5 messages)
    let has_enough_history = todays_messages.len() >= 3;
    if has_enough_history && engagement_rate < 0.2 {
        user_content.push_str("\nВовлечённость очень низкая — только [SKIP] или критичный триггер.\n");
    } else if has_enough_history && engagement_rate < 0.4 {
        user_content.push_str("\nВовлечённость низкая — пиши только если есть триггер или полезное действие.\n");
    }

    // Time-of-day tone hint
    let hour = chrono::Local::now().hour();
    if hour >= 8 && hour <= 10 {
        user_content.push_str("\n[Тон: бодрый, конкретный — утренний план]\n");
    } else if hour >= 21 && hour <= 23 {
        user_content.push_str("\n[Тон: тёплый, рефлексивный — вечерний чекин]\n");
    } else if hour >= 12 && hour <= 14 {
        user_content.push_str("\n[Тон: ненавязчивый, лёгкий]\n");
    }

    let request = ChatRequest {
        model: llm_model(),
        messages: vec![
            ChatMessage::text("system", &sys_prompt),
            ChatMessage::text("user", &user_content),
        ],
        max_tokens: 350,
        stream: false,
        temperature: 0.6,
        repetition_penalty: None,
        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
        tools: None,
    };

    // Ensure MLX is running on-demand
    tokio::task::spawn_blocking(|| crate::mlx_manager::ensure_mlx()).await.ok();

    let response = client
        .post(llm_chat_url())
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("MLX proactive error {}: {}", status, &body[..body.len().min(200)]));
    }

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
        return Ok(None);
    }

    // Validate output: reject gibberish (too short, no Cyrillic, or single-word answers)
    let word_count = text.split_whitespace().count();
    let has_cyrillic = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if word_count < 3 || !has_cyrillic {
        return Ok(None);
    }

    // Reject if model hallucinates food/drink/topics not in context
    let lower = text.to_lowercase();
    let ctx_lower = context.to_lowercase();
    let mem_lower = memory_context.to_lowercase();

    // Hard-ban food/drink words — always reject unless context is about food
    let food_words = ["чай", "чаёк", "чайку", "чайн", "кофе", "кофейку", "кофеёк", "латте",
        "капучино", "перекус", "перекуси", "покушать", "поешь", "перерыв на обед",
        "чайная пауза", "чайную паузу", "кола", "напиток"];
    let ctx_is_food = ["чай", "кофе", "еда", "блюд", "рецепт", "готов", "food", "кухн", "обед", "ужин", "завтрак"]
        .iter().any(|m| ctx_lower.contains(m));
    if !ctx_is_food && food_words.iter().any(|w| lower.contains(w)) {
        return Ok(None);
    }

    // Reject empty template phrases
    let empty_templates = [
        "отличный день для",
        "если что-то появится",
        "если что-то нужно",
        "просто дай знать",
        "желаю продуктивного",
        "желаю приятного",
        "хорошего дня",
        "наслаждайся",
        "начать день с",
    ];
    if empty_templates.iter().any(|t| lower.contains(t)) {
        return Ok(None);
    }

    // Reject if model mentions things not in memory or context (common hallucinations)
    let hallucination_words = ["книга", "книгу", "книге", "книги",
        "castlewebtoon", "тяпляп", "афкш"];
    for hw in &hallucination_words {
        if lower.contains(hw) && !ctx_lower.contains(hw) && !mem_lower.contains(hw) {
            return Ok(None);
        }
    }

    // Reject messages that are too long (model rambling)
    if text.split_whitespace().count() > 40 {
        return Ok(None);
    }

    // Reject near-duplicate of recent messages (simple word overlap check)
    for (prev_msg, _) in todays_messages.iter().rev().take(10) {
        let prev_lower = prev_msg.to_lowercase();
        let prev_words: std::collections::HashSet<&str> = prev_lower.split_whitespace().collect();
        let new_words: std::collections::HashSet<&str> = lower.split_whitespace().collect();
        if prev_words.len() >= 3 && new_words.len() >= 3 {
            let overlap = prev_words.intersection(&new_words).count();
            let max_len = prev_words.len().max(new_words.len());
            if max_len > 0 && (overlap as f64 / max_len as f64) > 0.7 {
                return Ok(None); // >70% word overlap = too similar
            }
        }
    }

    Ok(Some(text))
}

