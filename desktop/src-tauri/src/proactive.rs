// proactive.rs — Proactive messaging logic
use crate::types::*;
use crate::memory::{build_memory_context_from_db, save_proactive_settings};
use crate::macos::{run_osascript, check_calendar_access, classify_app, get_macos_idle_seconds, is_screen_locked};
use crate::voice::speak_tts;
use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use chrono::Timelike;

// ── Proactive messaging logic ──

const PROACTIVE_PROMPT_HEADER: &str = r#"Ты — Ханни, тёплый AI-компаньон. Пиши как друг, который рядом.

Задача: написать ОДНО короткое сообщение (1-2 предложения). По-русски, на "ты".

ДЕЙСТВИЯ (опционально):
Если нужно СДЕЛАТЬ что-то (напомнить, создать задачу, уведомить) — добавь блок после текста:
```action
{"action":"set_reminder","title":"текст","remind_at":"YYYY-MM-DDTHH:MM:SS"}
```
Разрешённые действия: set_reminder, create_task, send_notification, log_health, create_event.
НЕ используй действия без причины. Текст ОБЯЗАТЕЛЕН, действие — дополнение.

Выбери ОДИН стиль:
"#;

const PROACTIVE_PROMPT_FOOTER: &str = r#"
ПРИОРИТЕТЫ:
1. Есть триггер (событие/дистракция/просроченная задача/цель) → пиши про него
2. Есть свежий разговор → продолжи тему с новой стороны
3. Утро (8-10) → дайджест: план дня, задачи, цели
4. Вечер (21-23) → рефлексия: что получилось, что нет
5. Есть данные о здоровье/привычках → мягкое напоминание
6. Иначе → любопытство, забота, юмор (без привязки к приложению)

СТРОГИЕ ЗАПРЕТЫ:
- ЗАПРЕЩЕНО упоминать чай, кофе, перекус, еду, напитки — ВООБЩЕ, НИКОГДА, если контекст НЕ про еду
- ЗАПРЕЩЕНО писать "ты уже X часов/минут в [приложение]" — НЕ комментируй экранное время кроме триггера дистракции
- ЗАПРЕЩЕНО выдумывать факты. Если чего-то нет в [Память] — НЕ приписывай. Нет книги — не упоминай книгу. Нет проекта — не упоминай проект
- ЗАПРЕЩЕНО повторять темы/формулировки из [Уже сказано]. Если вчера было "Доброе утро! Нет событий" — сегодня НЕЛЬЗЯ так же
- ЗАПРЕЩЕНО использовать одинаковые шаблоны: "отличный день для...", "если что-то появится — дай знать", "желаю продуктивного дня"
- ЗАПРЕЩЕНО писать бессмыслицу, метафоры про чай, абсурдные сравнения
- Если нечего сказать — ответь [SKIP]. Лучше [SKIP] чем повтор или банальщина
- При дистракции (YouTube/Reddit) — ОДНО сообщение, потом [SKIP] пока пользователь не переключится

СТИЛЬ:
- МАКСИМУМ 1-2 коротких предложения. Не 3, не 4 — ровно 1-2
- Каждое сообщение должно быть УНИКАЛЬНЫМ по формулировке
- Привязывай к конкретным данным из контекста (цель, задача, событие, музыка)
- Утром — конкретный план (не "отличный день для..."). Вечером — конкретный вопрос. Днём — минимум
- Нет данных для сообщения → [SKIP]

ПРИМЕРЫ:
✅ Контекст: Музыка: Radiohead — Creep
[style:observation] Creep от Radiohead — настроение такое?

❌ Плохо: "Может чайку под Radiohead?" (чай!), "Ты уже 3 часа слушаешь" (время!)

✅ Контекст: Утро, цели: "Найти работу 0/10", "Пить воду 0/8"
[style:digest] Утро! Цель "Найти работу" — 0 из 10 откликов. Сегодня хотя бы один?

❌ Плохо: "Доброе утро! Нет событий — отличный день для отдыха!" (шаблон, пустота)

✅ Контекст: Триггер: просроченная задача "Купить молоко" (вчера)
[style:accountability] "Купить молоко" висит со вчера — ещё актуально?

✅ Контекст: Вечер, настроение не записано, 2 задачи завершены
[style:journal] Два дела сделаны сегодня. Как настроение?

✅ Контекст: Событие через 15 мин: Созвон
[style:calendar] Созвон через 15 минут!
```action
{"action":"send_notification","title":"Ханни","body":"Созвон через 15 минут!"}
```

✅ Контекст: Ничего интересного, нет триггеров
[SKIP]

❌ Плохо: "Как дела?", "Желаю продуктивного дня!", "Если что — дай знать" (пустота → [SKIP])

Формат: [style:ID] текст (+ опциональный ```action блок), или [SKIP]."#;

pub fn build_proactive_system_prompt(enabled_styles: &[String], recent_styles: &[String]) -> String {
    let hour = chrono::Local::now().hour();
    let mut prompt = PROACTIVE_PROMPT_HEADER.to_string();
    let styles: Vec<&ProactiveStyleDef> = if enabled_styles.is_empty() {
        ALL_PROACTIVE_STYLES.iter().collect()
    } else {
        ALL_PROACTIVE_STYLES.iter()
            .filter(|s| enabled_styles.iter().any(|e| e == s.id))
            .collect()
    };
    // Time-gate: digest only 8-10, journal only 19-23
    let filtered: Vec<&&ProactiveStyleDef> = styles.iter()
        .filter(|s| {
            if s.id == "digest" && !(8..=10).contains(&hour) { return false; }
            if s.id == "journal" && !(19..=23).contains(&hour) { return false; }
            true
        })
        .collect();
    for style in &filtered {
        prompt.push_str(&format!("- {}\n", style.description));
    }
    // Per-style cooldown hint
    if !recent_styles.is_empty() {
        prompt.push_str(&format!("\nНе используй эти стили (были недавно): {}\n", recent_styles.join(", ")));
    }
    prompt.push_str(PROACTIVE_PROMPT_FOOTER);
    prompt
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
        "SELECT COUNT(*) FROM tasks WHERE date(completed_at) = ?1",
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
        model: MODEL.into(),
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

    let response = client
        .post(MLX_URL)
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

// ── Proactive messaging commands ──
// ── Proactive messaging commands ──

#[tauri::command]
pub async fn get_proactive_settings(state: tauri::State<'_, Arc<Mutex<ProactiveState>>>) -> Result<ProactiveSettings, String> {
    let state = state.lock().await;
    Ok(state.settings.clone())
}

#[tauri::command]
pub async fn set_proactive_settings(
    settings: ProactiveSettings,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    save_proactive_settings(&settings)?;
    let mut state = state.lock().await;
    state.settings = settings;
    Ok(())
}

#[tauri::command]
pub async fn set_user_typing(
    typing: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.user_is_typing = typing;
    Ok(())
}

#[tauri::command]
pub async fn set_recording_state(
    recording: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    state.lock().await.is_recording = recording;
    Ok(())
}

#[tauri::command]
pub async fn report_proactive_engagement(
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
pub async fn report_user_chat_activity(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    pstate.last_user_chat_time = Some(chrono::Local::now());
    Ok(())
}

#[tauri::command]
pub fn rate_proactive(db: tauri::State<'_, HanniDb>, proactive_id: i64, rating: i64) -> Result<(), String> {
    db.conn().execute(
        "UPDATE proactive_history SET rating = ?1 WHERE id = ?2",
        rusqlite::params![rating, proactive_id],
    ).map_err(|e| format!("DB: {}", e))?;
    Ok(())
}

// ── Proactive background loop ──

pub async fn proactive_loop(proactive_handle: AppHandle, proactive_state_ref: Arc<Mutex<ProactiveState>>) {
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

    // Compute initial engagement rate from DB history (last 20 messages)
    let initial_engagement = {
        let db = proactive_handle.state::<HanniDb>();
        let conn = db.conn();
        let replied: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (SELECT user_replied FROM proactive_history ORDER BY id DESC LIMIT 20) WHERE user_replied=1",
            [], |row| row.get(0),
        ).unwrap_or(0);
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (SELECT id FROM proactive_history ORDER BY id DESC LIMIT 20)",
            [], |row| row.get(0),
        ).unwrap_or(0);
        if total > 0 { Some(replied as f64 / total as f64) } else { None }
    }; // conn dropped here before await
    if let Some(eng) = initial_engagement {
        let mut state = proactive_state_ref.lock().await;
        state.engagement_rate = eng;
    }

    let mut last_check = std::time::Instant::now();
    let mut first_run = true;
    let mut prev_auto_quiet = false;
    let mut wake_up_pending = false;

    loop {
        // Poll every 5 seconds so we react quickly to settings changes
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let (enabled, interval, quiet_start_min, quiet_end_min, is_typing, is_recording, skips, voice_enabled, voice_name, recent_msgs, last_ctx, engagement, triggers, last_user_chat, enabled_styles, daily_limit) = {
            let state = proactive_state_ref.lock().await;
            (
                state.settings.enabled,
                state.settings.interval_minutes,
                state.settings.quiet_start_minutes(),
                state.settings.quiet_end_minutes(),
                state.user_is_typing,
                state.is_recording,
                state.consecutive_skips,
                state.settings.voice_enabled,
                state.settings.voice_name.clone(),
                state.recent_messages.clone(),
                state.last_context_snapshot.clone(),
                state.engagement_rate,
                state.pending_triggers.iter()
                    .filter(|(_, created)| created.elapsed().as_secs() < 600)
                    .map(|(t, _)| t.clone())
                    .collect::<Vec<String>>(),
                state.last_user_chat_time,
                state.settings.enabled_styles.clone(),
                state.settings.daily_limit,
            )
        };

        if !enabled {
            first_run = true;
            continue;
        }

        // Skip proactive when OpenClaw is active — it has its own proactive system
        let openclaw_active: bool = {
            let db = proactive_handle.state::<HanniDb>();
            let conn = db.conn();
            conn.query_row(
                "SELECT value FROM app_settings WHERE key='use_openclaw'",
                [], |row| row.get::<_, String>(0),
            ).map(|v| v == "true" || v == "1").unwrap_or(false)
        };
        if openclaw_active {
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

            // pending trigger (generic — includes smart triggers from DB)
            if !triggers.is_empty() {
                score += 0.4_f64.min(score + 0.4) - score; // ensure at least +0.15
                score += 0.15;
            }

            // Smart triggers from DB (overdue tasks, goals) — checked before LLM call
            // These are lower priority than real-time triggers but still boost score
            // We check DB triggers at LLM call time, but hint the score here based on time
            let hour = chrono::Local::now().hour();
            // Morning & evening are prime times for smart triggers
            if (8..=10).contains(&hour) || (20..=22).contains(&hour) {
                // Will be populated with smart_triggers later; give a small bonus
                // to increase chance of firing during ritual times
            }

            // Block proactive while user is actively chatting
            if let Some(last_chat) = last_user_chat {
                let chat_idle_min = (chrono::Local::now() - last_chat).num_minutes();
                if chat_idle_min < 5 {
                    // User chatted in last 5 min — suppress proactive completely
                    continue;
                }
                if chat_idle_min > 30 { score += 0.1; }
            } else {
                score += 0.1; // no chat at all — consider idle
            }

            // Engagement-adaptive: high engagement → slightly more proactive
            if engagement > 0.6 { score += 0.1; }
            if engagement > 0.8 { score += 0.05; }
            // Low engagement → harder to fire (but only with enough history)
            let total_msgs = {
                let db = proactive_handle.state::<HanniDb>();
                let conn = db.conn();
                conn.query_row("SELECT COUNT(*) FROM proactive_history", [], |r| r.get::<_, i64>(0)).unwrap_or(0)
            };
            if total_msgs > 10 {
                if engagement < 0.2 { score -= 0.2; }
                else if engagement < 0.4 { score -= 0.1; }
            }

            // deep work hours penalty (10-12, 14-17)
            let hour = chrono::Local::now().hour();
            if (10..=12).contains(&hour) || (14..=17).contains(&hour) {
                score -= 0.1;
            }

            // Morning/evening bonus (ritual times)
            if (8..=10).contains(&hour) || (21..=23).contains(&hour) {
                score += 0.1;
            }

            // many skips penalty
            if skips > 3 { score -= 0.15; }
            if skips > 6 { score -= 0.15; } // progressive backoff

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

        let llm_busy = proactive_handle.state::<LlmBusy>().0.available_permits() == 0;
        let call_active = {
            let cm = proactive_handle.state::<Arc<CallMode>>();
            let guard = cm.0.lock().unwrap_or_else(|e| e.into_inner());
            guard.active
        };
        let is_speaking_now = IS_SPEAKING.load(Ordering::Relaxed);

        // Auto-quiet: detect idle/sleep without fixed quiet hours
        let (idle_secs, screen_locked) = tokio::task::spawn_blocking(|| {
            (get_macos_idle_seconds(), is_screen_locked())
        }).await.unwrap_or((0.0, false));
        let idle_min = idle_secs / 60.0;
        let hour = now_t.hour();
        let is_night = hour >= 22 || hour < 8;
        let auto_quiet = (screen_locked && is_night)  // locked at night = sleeping
            || (idle_min > 15.0 && is_night)          // 15 min idle at night
            || idle_min > 30.0;                        // 30 min idle anytime = away

        // Emit event on state change
        if auto_quiet != prev_auto_quiet {
            let _ = proactive_handle.emit("proactive-auto-quiet", auto_quiet);
            // Update ProactiveState for external queries
            proactive_state_ref.lock().await.auto_quiet = auto_quiet;
            prev_auto_quiet = auto_quiet;
        }

        eprintln!("[proactive] gate: quiet={} auto={} typing={} rec={} llm={} call={} speak={} wake={} idle={:.0}s",
            in_quiet, auto_quiet, is_typing, is_recording, llm_busy, call_active, is_speaking_now, wake_up_pending, idle_secs);

        if in_quiet || auto_quiet {
            wake_up_pending = true;
            continue;
        }

        // Smooth wake-up: wait for user activity after quiet period
        if wake_up_pending {
            if idle_min > 5.0 {
                continue; // user not active yet
            }
            wake_up_pending = false;
        }

        if is_typing || llm_busy || call_active || is_speaking_now || is_recording {
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

        // Gather smart triggers from DB (overdue tasks, goals, health)
        let smart_triggers: Vec<String> = tokio::task::spawn_blocking(gather_smart_triggers)
            .await.unwrap_or_default();
        // Merge with pending triggers (event/distraction triggers from snapshot loop)
        let mut all_triggers = triggers.clone();
        for st in &smart_triggers {
            if !all_triggers.contains(st) {
                all_triggers.push(st.clone());
            }
        }

        // Build memory context (8 core facts — better personalization)
        let (mem_ctx, chat_snippet, user_name, todays_msgs, recent_styles) = {
            let db = proactive_handle.state::<HanniDb>();
            let conn = db.conn();
            // Pass current app as context hint for memory search
            let ctx_hint = context.lines()
                .find(|l| l.contains("Frontmost:"))
                .unwrap_or("")
                .to_string();
            (
                build_memory_context_from_db(&conn, &ctx_hint, 8, None),
                get_recent_chat_snippet(&conn, 4),
                get_user_name_from_memory(&conn),
                get_todays_proactive_messages(&conn),
                get_recent_proactive_styles(&conn, 3),
            )
        };

        // Daily limit check: skip if limit reached and no triggers
        if daily_limit > 0 && todays_msgs.len() as u32 >= daily_limit && triggers.is_empty() {
            eprintln!("[proactive] daily_count={} limit={} triggers=0 — skipping (daily limit reached)", todays_msgs.len(), daily_limit);
            continue;
        }
        eprintln!("[proactive] daily_count={} limit={} triggers={}", todays_msgs.len(), daily_limit, triggers.len());

        // Acquire LLM semaphore during proactive call to prevent concurrent MLX requests
        let proactive_sem = proactive_handle.state::<LlmBusy>();
        let _proactive_permit = match proactive_sem.0.try_acquire() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let proactive_result = proactive_llm_call(&client, &context, &recent_msgs, skips, &mem_ctx, &delta, &all_triggers, &chat_snippet, engagement, &user_name, &todays_msgs, &enabled_styles, &recent_styles).await;
        drop(_proactive_permit);

        // P4: Re-check typing after LLM call — discard proactive if user started chatting
        let typing_during_call = proactive_state_ref.lock().await.user_is_typing;

        match proactive_result {
            Ok(Some(raw_message)) if !typing_during_call => {
                // Parse [style:X] prefix if present
                let (style, message) = if raw_message.starts_with("[style:") {
                    if let Some(end) = raw_message.find(']') {
                        let s = raw_message[7..end].to_string();
                        let m = raw_message[end+1..].trim().to_string();
                        (s, if m.is_empty() { raw_message.clone() } else { m })
                    } else { (String::new(), raw_message) }
                } else { (String::new(), raw_message) };

                // Record in proactive_history (with style)
                let proactive_id = {
                    let db = proactive_handle.state::<HanniDb>();
                    let conn = db.conn();
                    let _ = conn.execute(
                        "INSERT INTO proactive_history (sent_at, message, style) VALUES (?1, ?2, ?3)",
                        rusqlite::params![chrono::Local::now().to_rfc3339(), &message, &style],
                    );
                    conn.last_insert_rowid()
                };

                // Emit as JSON {text, id} for frontend feedback buttons
                let _ = proactive_handle.emit("proactive-message", serde_json::json!({
                    "text": &message,
                    "id": proactive_id,
                }));
                // Voice: once per period (morning 8-11, day 12-19, evening 20-23)
                if voice_enabled {
                    let vh = chrono::Local::now().hour();
                    let period = match vh {
                        8..=11 => "morning",
                        12..=19 => "day",
                        20..=23 => "evening",
                        _ => "",
                    };
                    let mut state = proactive_state_ref.lock().await;
                    // Reset periods on new day
                    let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();
                    if state.voiced_periods_date != today_str {
                        state.voiced_periods_today.clear();
                        state.voiced_periods_date = today_str;
                    }
                    if !period.is_empty() && !state.voiced_periods_today.contains(&period.to_string()) {
                        state.voiced_periods_today.push(period.to_string());
                        drop(state);
                        speak_tts(&message, &voice_name);
                    } else {
                        drop(state);
                    }
                }

                let mut state = proactive_state_ref.lock().await;
                state.last_message_time = Some(chrono::Local::now());
                state.last_message_text = message.clone();
                state.consecutive_skips = 0;
                state.last_context_snapshot = context;
                state.last_proactive_id = Some(proactive_id);
                // Update recent_messages (keep last 15)
                state.recent_messages.push((message, chrono::Local::now()));
                if state.recent_messages.len() > 15 {
                    state.recent_messages.remove(0);
                }
                state.pending_triggers.clear();
            }
            // P4: User started typing during LLM call — discard message
            Ok(Some(_)) => {
                let mut state = proactive_state_ref.lock().await;
                state.consecutive_skips += 1;
                state.last_context_snapshot = context;
            }
            Ok(None) => {
                let mut state = proactive_state_ref.lock().await;
                state.consecutive_skips += 1;
                state.last_context_snapshot = context;
            }
            Err(e) => {
                // LLM error — retry once after 30s by setting short backoff
                eprintln!("[proactive] LLM error: {} — will retry sooner", e);
                let base_interval_secs = interval * 60;
                last_check = std::time::Instant::now() - std::time::Duration::from_secs(base_interval_secs.saturating_sub(30));
            }
        }
    }
}
