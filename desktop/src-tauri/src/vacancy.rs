// vacancy.rs — Background vacancy search with per-source scheduling
// Reads roles from jobs_positions, sources+schedules from jobs_sources.
// Each source has its own HH:MM schedule; loop checks every 60s.

use crate::types::*;
use crate::mcp::McpState;
use crate::agent::{run_agent_task, AgentContext};
use tauri::{AppHandle, Manager};
use chrono::Timelike;

// ── Tauri command: manual trigger ──

#[tauri::command]
pub async fn vacancy_search_now(app: AppHandle) -> Result<String, String> {
    eprintln!("[vacancy] Manual search triggered");
    let sources = load_sources(&app)?;
    if sources.is_empty() {
        return Err("No sources in jobs_sources".into());
    }
    for (key, url, name) in &sources {
        eprintln!("[vacancy] Searching: {} ({})", name, url);
        if let Err(e) = search_one_source(&app, url, name).await {
            eprintln!("[vacancy] {} error: {}", key, e);
        }
    }
    Ok("Vacancy search complete".into())
}

/// Background loop: checks every 60s if any source is due.
pub async fn vacancy_search_loop(app: AppHandle) {
    // Let MCP servers connect first
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;

    let mut fired_today: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut last_date = chrono::Local::now().format("%Y-%m-%d").to_string();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Reset fired set at midnight
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if today != last_date {
            fired_today.clear();
            last_date = today;
        }

        // Check if enabled
        let enabled = {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            conn.query_row(
                "SELECT value FROM app_settings WHERE key='enable_vacancy_search'",
                [], |row| row.get::<_, String>(0),
            ).ok().map(|v| v == "true").unwrap_or(false)
        };
        if !enabled { continue; }

        // Load sources with schedules
        let sources = match load_sources_with_schedule(&app) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let now = chrono::Local::now();
        let now_min = now.hour() * 60 + now.minute();

        for (key, url, name, schedule_min) in &sources {
            if fired_today.contains(key) { continue; }
            // Fire if current time >= schedule and within 5 min window
            if now_min >= *schedule_min && now_min < schedule_min + 5 {
                eprintln!("[vacancy] Schedule hit: {} at {:02}:{:02}", name, schedule_min / 60, schedule_min % 60);
                match search_one_source(&app, url, name).await {
                    Ok(_) => eprintln!("[vacancy] {} done", key),
                    Err(e) => eprintln!("[vacancy] {} error: {}", key, e),
                }
                fired_today.insert(key.clone());
            }
        }
    }
}

/// Search one source via LLM + Playwright.
async fn search_one_source(app: &AppHandle, source_url: &str, source_name: &str) -> Result<(), String> {
    let roles = load_roles(app)?;
    if roles.is_empty() {
        return Err("No roles in jobs_positions".into());
    }

    let existing = get_existing_titles(app)?;

    let mcp_tools = {
        let mcp_state = app.state::<McpState>();
        let mgr = mcp_state.0.lock().await;
        mgr.tools_as_openai().to_vec()
    };
    if mcp_tools.is_empty() {
        return Err("No MCP tools (Playwright not connected)".into());
    }

    let mut tools = mcp_tools;
    tools.push(serde_json::json!({
        "type": "function",
        "function": {
            "name": "save_vacancy",
            "description": "Save a found vacancy to the database",
            "parameters": {
                "type": "object",
                "properties": {
                    "company": { "type": "string", "description": "Company name" },
                    "position": { "type": "string", "description": "Job title" },
                    "salary": { "type": "string", "description": "Salary range" },
                    "url": { "type": "string", "description": "Link to vacancy" },
                    "source": { "type": "string", "description": "Source name" },
                    "notes": { "type": "string", "description": "Requirements, format, city" }
                },
                "required": ["company", "position", "url"]
            }
        }
    }));

    let system_prompt = format!(
        r#"Ты — агент по поиску вакансий. Найди подходящие вакансии и сохрани через save_vacancy.

РОЛИ КОТОРЫЕ ИЩЕМ:
{roles}

ПРАВИЛА:
1. Открой {url} через browser_navigate
2. Ищи вакансии по каждой роли (используй поиск на сайте)
3. Читай страницу через browser_snapshot
4. Для КАЖДОЙ подходящей вакансии вызови save_vacancy
5. НЕ добавляй дубликаты: {existing}
6. Максимум 10 вакансий за источник
7. Если сайт требует логин или капчу — пропусти, не застревай"#,
        roles = roles.join(", "),
        url = source_url,
        existing = if existing.is_empty() { "нет".into() } else { existing.join(", ") },
    );

    let user_prompt = format!("Найди вакансии на {} ({})", source_name, source_url);
    let context = AgentContext::new();

    run_agent_task(app, &system_prompt, &user_prompt, tools, context).await?;
    Ok(())
}

/// Load roles from jobs_positions.
fn load_roles(app: &AppHandle) -> Result<Vec<String>, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT value FROM facts WHERE category='jobs_positions' AND key != '_settings'"
    ).map_err(|e| e.to_string())?;
    let roles: Vec<String> = stmt.query_map([], |row| {
        let val: String = row.get(0)?;
        let parsed: serde_json::Value = serde_json::from_str(&val).unwrap_or_default();
        Ok(parsed["title"].as_str().unwrap_or("").to_string())
    }).map_err(|e| e.to_string())?.flatten().filter(|s| !s.is_empty()).collect();
    Ok(roles)
}

/// Load all sources (key, url, name) — for manual search.
fn load_sources(app: &AppHandle) -> Result<Vec<(String, String, String)>, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT key, value FROM facts WHERE category='jobs_sources'"
    ).map_err(|e| e.to_string())?;
    let sources: Vec<(String, String, String)> = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let val: String = row.get(1)?;
        let parsed: serde_json::Value = serde_json::from_str(&val).unwrap_or_default();
        let url = parsed["url"].as_str().unwrap_or("").to_string();
        let name = parsed["name"].as_str().unwrap_or(&key).to_string();
        Ok((key, url, name))
    }).map_err(|e| e.to_string())?.flatten().filter(|s| !s.1.is_empty()).collect();
    Ok(sources)
}

/// Load sources with schedule (key, url, name, schedule_minutes).
fn load_sources_with_schedule(app: &AppHandle) -> Result<Vec<(String, String, String, u32)>, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT key, value FROM facts WHERE category='jobs_sources'"
    ).map_err(|e| e.to_string())?;
    let sources: Vec<(String, String, String, u32)> = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let val: String = row.get(1)?;
        let parsed: serde_json::Value = serde_json::from_str(&val).unwrap_or_default();
        let url = parsed["url"].as_str().unwrap_or("").to_string();
        let name = parsed["name"].as_str().unwrap_or(&key).to_string();
        let schedule = parsed["schedule"].as_str().unwrap_or("");
        let mins = parse_hhmm(schedule);
        Ok((key, url, name, mins))
    }).map_err(|e| e.to_string())?.flatten().filter(|s| !s.1.is_empty() && s.3 > 0).collect();
    Ok(sources)
}

/// Parse "HH:MM" to minutes since midnight.
fn parse_hhmm(s: &str) -> u32 {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 { return 0; }
    let h: u32 = parts[0].parse().unwrap_or(0);
    let m: u32 = parts[1].parse().unwrap_or(0);
    h * 60 + m
}

/// Get existing vacancy titles for deduplication.
fn get_existing_titles(app: &AppHandle) -> Result<Vec<String>, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    // Try job_vacancies table, gracefully handle if it doesn't exist
    let mut stmt = match conn.prepare(
        "SELECT company || ' — ' || position FROM job_vacancies WHERE stage NOT IN ('rejected', 'ignored')"
    ) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };
    let titles: Vec<String> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| format!("DB: {}", e))?.flatten().collect();
    Ok(titles)
}
