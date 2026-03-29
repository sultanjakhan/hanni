// vacancy.rs — Background vacancy search loop
// Every 4 hours: reads skills + sources from facts, searches via LLM + Playwright MCP

use crate::types::*;
use crate::mcp::McpState;
use crate::agent::{run_agent_task, AgentContext};
use tauri::{AppHandle, Manager};

// ── Tauri command: manual trigger ──

#[tauri::command]
pub async fn vacancy_search_now(app: AppHandle) -> Result<String, String> {
    eprintln!("[vacancy] Manual search triggered");
    run_vacancy_search(&app).await?;
    Ok("Vacancy search complete".into())
}

const SEARCH_INTERVAL_SECS: u64 = 4 * 60 * 60; // 4 hours

/// Background loop: searches for vacancies periodically.
pub async fn vacancy_search_loop(app: AppHandle) {
    // Initial delay — let MCP servers connect first
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    loop {
        let enabled = {
            let db = app.state::<HanniDb>();
            let conn = db.conn();
            conn.query_row(
                "SELECT value FROM app_settings WHERE key='enable_vacancy_search'",
                [], |row| row.get::<_, String>(0),
            ).ok().map(|v| v == "true").unwrap_or(false)
        };

        if enabled {
            eprintln!("[vacancy] Starting search cycle...");
            if let Err(e) = run_vacancy_search(&app).await {
                eprintln!("[vacancy] Search failed: {}", e);
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(SEARCH_INTERVAL_SECS)).await;
    }
}

/// Run one vacancy search cycle.
async fn run_vacancy_search(app: &AppHandle) -> Result<(), String> {
    let (skills, sources) = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();
        let skills: String = conn.query_row(
            "SELECT value FROM facts WHERE category='work' AND key='vacancy_skills'",
            [], |row| row.get(0),
        ).unwrap_or_default();
        let sources: String = conn.query_row(
            "SELECT value FROM facts WHERE category='work' AND key='vacancy_sources'",
            [], |row| row.get(0),
        ).unwrap_or_default();
        (skills, sources)
    };

    if skills.is_empty() {
        eprintln!("[vacancy] No vacancy_skills in facts — skipping");
        return Ok(());
    }
    if sources.is_empty() {
        eprintln!("[vacancy] No vacancy_sources in facts — skipping");
        return Ok(());
    }

    let source_list: Vec<String> = serde_json::from_str(&sources)
        .unwrap_or_else(|_| vec![sources.clone()]);

    let existing_titles = get_existing_titles(app)?;

    let mcp_tools = {
        let mcp_state = app.state::<McpState>();
        let mgr = mcp_state.0.lock().await;
        mgr.tools_as_openai().to_vec()
    };
    if mcp_tools.is_empty() {
        return Err("No MCP tools available (Playwright not connected)".into());
    }

    let mut tools = mcp_tools;
    tools.push(serde_json::json!({
        "type": "function",
        "function": {
            "name": "save_vacancy",
            "description": "Save a found vacancy to the job search database",
            "parameters": {
                "type": "object",
                "properties": {
                    "company": { "type": "string", "description": "Company name" },
                    "position": { "type": "string", "description": "Job title" },
                    "salary": { "type": "string", "description": "Salary range" },
                    "url": { "type": "string", "description": "Link to vacancy" },
                    "notes": { "type": "string", "description": "Requirements, format, city" }
                },
                "required": ["company", "position"]
            }
        }
    }));

    let system_prompt = format!(
        r#"Ты — агент по поиску вакансий. Найди подходящие вакансии и сохрани каждую через save_vacancy.

НАВЫКИ ПОЛЬЗОВАТЕЛЯ:
{skills}

ПРАВИЛА:
1. Используй playwright_browser_navigate чтобы открыть сайт
2. Используй playwright_browser_snapshot чтобы прочитать страницу
3. Для КАЖДОЙ подходящей вакансии вызови save_vacancy
4. НЕ добавляй вакансии которые уже есть: {existing}
5. Максимум 10 вакансий за один поиск"#,
        skills = skills,
        existing = if existing_titles.is_empty() { "нет существующих".into() } else { existing_titles.join(", ") },
    );

    let mut context = AgentContext::new();
    context.insert("save_to_table".into(), serde_json::json!("job_vacancies"));

    for source in &source_list {
        eprintln!("[vacancy] Searching: {}", source);
        let user_prompt = format!("Найди вакансии на: {}", source);
        match run_agent_task(app, &system_prompt, &user_prompt, tools.clone(), context.clone()).await {
            Ok(result) => eprintln!("[vacancy] Source '{}' done: {}...", source, &result[..result.len().min(150)]),
            Err(e) => eprintln!("[vacancy] Source '{}' error: {}", source, e),
        }
    }

    eprintln!("[vacancy] Search cycle complete");
    Ok(())
}

/// Get existing vacancy titles for deduplication.
fn get_existing_titles(app: &AppHandle) -> Result<Vec<String>, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT company || ' — ' || position FROM job_vacancies WHERE stage NOT IN ('rejected', 'ignored')"
    ).map_err(|e| format!("DB error: {}", e))?;
    let titles: Vec<String> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| format!("DB error: {}", e))?.flatten().collect();
    Ok(titles)
}
