// vacancy.rs — Background vacancy search with per-source scheduling
// Reads roles from jobs_positions, sources+schedules from jobs_sources.
// Each source has its own HH:MM schedule; loop checks every 60s.

use crate::types::*;
use crate::mcp::McpState;
use crate::agent::{run_agent_task, AgentContext};
use tauri::{AppHandle, Manager};
use chrono::Timelike;

// ── Tauri commands ──

#[tauri::command]
pub async fn vacancy_search_now(app: AppHandle) -> Result<String, String> {
    eprintln!("[vacancy] Manual search triggered (all sources)");
    let sources = load_sources(&app)?;
    if sources.is_empty() {
        return Err("No sources in jobs_sources".into());
    }
    let mut found = 0;
    for (key, url, name) in &sources {
        eprintln!("[vacancy] Searching: {} ({})", name, url);
        match search_one_source(&app, url, name).await {
            Ok(n) => { found += n; eprintln!("[vacancy] {} done, {} saved", key, n); }
            Err(e) => eprintln!("[vacancy] {} error: {}", key, e),
        }
    }
    Ok(format!("Done: {} vacancies from {} sources", found, sources.len()))
}

#[tauri::command]
pub async fn vacancy_search_source(app: AppHandle, key: String) -> Result<String, String> {
    eprintln!("[vacancy] Search single source: {}", key);
    let sources = load_sources(&app)?;
    let (_, url, name) = sources.iter()
        .find(|(k, _, _)| k == &key)
        .ok_or_else(|| format!("Source '{}' not found", key))?;
    let n = search_one_source(&app, url, name).await?;
    Ok(format!("Done: {} vacancies from {}", n, name))
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

/// Route search: HH API for hh.kz/hh.ru, LLM+Playwright for everything else.
async fn search_one_source(app: &AppHandle, source_url: &str, source_name: &str) -> Result<usize, String> {
    if source_url.contains("hh.kz") || source_url.contains("hh.ru") {
        return search_hh_api(app).await;
    }
    search_via_llm(app, source_url, source_name).await
}

/// Search HH via public API — no Playwright, no LLM needed.
async fn search_hh_api(app: &AppHandle) -> Result<usize, String> {
    let roles = load_roles(app)?;
    if roles.is_empty() { return Err("No roles".into()); }
    let settings = load_settings_raw(app);
    let existing = get_existing_titles(app)?;
    let blacklist = load_blacklist(app);
    let client = &app.state::<HttpClient>().0;

    // area: numeric ID or skip for worldwide search
    let area_raw = settings.get("area").and_then(|v| v.as_str()).unwrap_or("");
    let area = if area_raw.chars().all(|c| c.is_ascii_digit()) && !area_raw.is_empty() { Some(area_raw) } else { None };
    let mut saved = 0;

    for role in &roles {
        // Wrap in quotes for exact phrase match
        let query = format!("\"{}\"", role);
        let mut url = format!(
            "https://api.hh.kz/vacancies?text={}&per_page=20&order_by=publication_time&experience=noExperience&experience=between1And3&professional_role=73&professional_role=107&industry=7&industry=9&industry=11&industry=41&industry=43&industry=44",
            encode_uri(&query),
        );
        if let Some(a) = area { url.push_str(&format!("&area={}", a)); }
        // Add salary filter if set
        if let Some(sal) = settings.get("salary").and_then(|v| v.as_str()) {
            if let Some(num) = sal.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse::<u64>().ok() {
                url.push_str(&format!("&salary={}&currency=KZT", num));
            }
        }
        // Experience already hardcoded in base URL (noExperience + between1And3)

        eprintln!("[vacancy/hh] Fetching: {}", url);
        let resp = client.get(&url)
            .header("User-Agent", "Hanni/1.0")
            .send().await.map_err(|e| format!("HH API: {}", e))?;

        if !resp.status().is_success() {
            eprintln!("[vacancy/hh] API error: {}", resp.status());
            continue;
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| format!("HH parse: {}", e))?;
        let items = body["items"].as_array().ok_or("HH: no items")?;

        let db = app.state::<HanniDb>();
        let conn = db.conn();

        for item in items {
            let company = item["employer"]["name"].as_str().unwrap_or("");
            let position = item["name"].as_str().unwrap_or("");
            let vacancy_url = item["alternate_url"].as_str().unwrap_or("");
            let dedup_key = format!("{} — {}", company, position);
            if existing.contains(&dedup_key) { continue; }
            // Skip blacklisted companies
            let company_lower = company.to_lowercase();
            if blacklist.iter().any(|b| company_lower.contains(b)) {
                eprintln!("[vacancy/hh] Skipped (blacklist): {}", company);
                continue;
            }

            let salary = match (item["salary"]["from"].as_u64(), item["salary"]["to"].as_u64()) {
                (Some(from), Some(to)) => format!("{}-{}", from, to),
                (Some(from), None) => format!("от {}", from),
                (None, Some(to)) => format!("до {}", to),
                _ => String::new(),
            };
            let currency = item["salary"]["currency"].as_str().unwrap_or("");
            let salary_full = if salary.is_empty() { String::new() } else { format!("{} {}", salary, currency) };

            let req = item["snippet"]["requirement"].as_str().unwrap_or("");
            let resp_text = item["snippet"]["responsibility"].as_str().unwrap_or("");
            let city = item["area"]["name"].as_str().unwrap_or("");
            let notes = format!("hh.kz | {}\nТребования: {}\nОбязанности: {}", city, req, resp_text);
            let now = chrono::Local::now().to_rfc3339();

            match conn.execute(
                "INSERT INTO job_vacancies (company, position, salary, url, stage, notes, found_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'found', ?5, ?6, ?6)",
                rusqlite::params![company, position, salary_full, vacancy_url, notes, now],
            ) {
                Ok(_) => { saved += 1; eprintln!("[vacancy/hh] Saved: {} — {}", company, position); }
                Err(e) => eprintln!("[vacancy/hh] DB error: {}", e),
            }
        }
    }
    eprintln!("[vacancy/hh] Total saved: {}", saved);
    Ok(saved)
}

/// Search one source via LLM + Playwright.
async fn search_via_llm(app: &AppHandle, source_url: &str, source_name: &str) -> Result<usize, String> {
    let roles = load_roles(app)?;
    if roles.is_empty() {
        return Err("No roles in jobs_positions".into());
    }
    let settings = load_settings(app);

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

    let filters = if settings.is_empty() { String::new() } else {
        format!("\nФИЛЬТРЫ (сохраняй ТОЛЬКО подходящие):\n{}", settings)
    };

    let system_prompt = format!(
        r#"Ты — агент по поиску вакансий. Найди подходящие вакансии и сохрани через save_vacancy.

РОЛИ КОТОРЫЕ ИЩЕМ:
{roles}
{filters}
ПРАВИЛА:
1. Открой {url} через browser_navigate
2. Ищи вакансии по каждой роли (используй поиск на сайте)
3. Читай страницу через browser_snapshot
4. Для КАЖДОЙ подходящей вакансии вызови save_vacancy
5. НЕ добавляй дубликаты: {existing}
6. Максимум 10 вакансий за источник
7. Если сайт требует логин или капчу — пропусти, не застревай
8. Сохраняй ТОЛЬКО вакансии, которые соответствуют ролям и фильтрам выше"#,
        roles = roles.join(", "),
        filters = filters,
        url = source_url,
        existing = if existing.is_empty() { "нет".into() } else { existing.join(", ") },
    );

    let user_prompt = format!("Найди вакансии на {} ({})", source_name, source_url);
    let context = AgentContext::new();

    run_agent_task(app, &system_prompt, &user_prompt, tools, context).await?;
    Ok(0) // exact count tracked via agent logs
}

/// Load blacklisted company names (lowercase) from jobs_blacklist.
fn load_blacklist(app: &AppHandle) -> Vec<String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    conn.prepare("SELECT value FROM facts WHERE category='jobs_blacklist'")
        .and_then(|mut s| {
            let v: Vec<String> = s.query_map([], |row| row.get(0))?
                .flatten().map(|s: String| s.to_lowercase()).collect();
            Ok(v)
        })
        .unwrap_or_default()
}

/// Load raw settings JSON for HH API filters.
fn load_settings_raw(app: &AppHandle) -> serde_json::Value {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    let val: String = conn.query_row(
        "SELECT value FROM facts WHERE category='jobs_positions' AND key='_settings'",
        [], |row| row.get(0),
    ).unwrap_or_default();
    serde_json::from_str(&val).unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
}

/// Load search settings as human-readable text for LLM prompt.
fn load_settings(app: &AppHandle) -> String {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    let val: String = conn.query_row(
        "SELECT value FROM facts WHERE category='jobs_positions' AND key='_settings'",
        [], |row| row.get(0),
    ).unwrap_or_default();
    if val.is_empty() { return String::new(); }
    let parsed: serde_json::Value = serde_json::from_str(&val).unwrap_or_default();
    let mut lines = Vec::new();
    if let Some(v) = parsed.get("area").and_then(|v| v.as_str()) { lines.push(format!("- Регион: {}", v)); }
    if let Some(v) = parsed.get("salary").and_then(|v| v.as_str()) { lines.push(format!("- Зарплата: {}", v)); }
    if let Some(v) = parsed.get("experience").and_then(|v| v.as_str()) { lines.push(format!("- Опыт: {}", v)); }
    if let Some(v) = parsed.get("format").and_then(|v| v.as_str()) { lines.push(format!("- Формат: {}", v)); }
    if let Some(v) = parsed.get("city").and_then(|v| v.as_str()) { lines.push(format!("- Город: {}", v)); }
    lines.join("\n")
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

/// Simple percent-encoding for URL query params.
fn encode_uri(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => { out.push('%'); out.push_str(&format!("{:02X}", b)); }
        }
    }
    out
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
