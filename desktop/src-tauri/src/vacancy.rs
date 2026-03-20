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
const VACANCY_PROJECT_NAME: &str = "Вакансии";

/// Background loop: searches for vacancies periodically.
pub async fn vacancy_search_loop(app: AppHandle) {
    // Initial delay — let MCP servers connect first
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    loop {
        // Check if vacancy search is enabled
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
    // 1. Read skills and sources from facts
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

    // Parse sources (JSON array of strings)
    let source_list: Vec<String> = serde_json::from_str(&sources)
        .unwrap_or_else(|_| vec![sources.clone()]);

    // 2. Ensure "Вакансии" project exists
    let project_id = ensure_vacancy_project(app)?;

    // 3. Get existing vacancy titles for deduplication
    let existing_titles = get_existing_titles(app, project_id)?;

    // 4. Get MCP tools (Playwright)
    let mcp_tools = {
        let mcp_state = app.state::<McpState>();
        let mgr = mcp_state.0.lock().await;
        mgr.tools_as_openai().to_vec()
    };

    if mcp_tools.is_empty() {
        return Err("No MCP tools available (Playwright not connected)".into());
    }

    // Add create_task tool so the agent can save results
    let mut tools = mcp_tools;
    tools.push(serde_json::json!({
        "type": "function",
        "function": {
            "name": "create_task",
            "description": "Создать задачу-вакансию в проекте Вакансии",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Название вакансии (компания + должность + зарплата)" },
                    "description": { "type": "string", "description": "Подробности: требования, ссылка, формат работы" },
                    "priority": { "type": "string", "enum": ["low", "normal", "high"], "description": "Приоритет" }
                },
                "required": ["title"]
            }
        }
    }));

    // 5. For each source, run agent search
    let system_prompt = format!(
        r#"Ты — агент по поиску вакансий. Твоя задача: зайти на сайт, найти подходящие вакансии и сохранить каждую через create_task.

НАВЫКИ ПОЛЬЗОВАТЕЛЯ:
{skills}

ПРАВИЛА:
1. Используй playwright_browser_navigate чтобы открыть сайт
2. Используй playwright_browser_snapshot чтобы прочитать страницу
3. При необходимости используй playwright_browser_click для навигации
4. Для КАЖДОЙ подходящей вакансии вызови create_task с:
   - title: "Компания — Должность (зарплата)"
   - description: требования, ссылка, формат (удалённо/офис), город
   - priority: "high" если идеально подходит, "normal" если частично
5. НЕ добавляй вакансии которые уже есть: {existing}
6. Если вакансий нет — просто скажи "Новых вакансий не найдено"
7. Максимум 10 вакансий за один поиск

project_id для create_task: {pid}"#,
        skills = skills,
        existing = if existing_titles.is_empty() { "нет существующих".into() } else { existing_titles.join(", ") },
        pid = project_id,
    );

    // Context override: force project_id for all create_task calls
    let mut context = AgentContext::new();
    context.insert("project_id".into(), serde_json::json!(project_id));

    for source in &source_list {
        eprintln!("[vacancy] Searching: {}", source);
        let user_prompt = format!("Найди вакансии на: {}", source);

        match run_agent_task(app, &system_prompt, &user_prompt, tools.clone(), context.clone()).await {
            Ok(result) => {
                eprintln!("[vacancy] Source '{}' done: {}...",
                    source, &result[..result.len().min(150)]);
            }
            Err(e) => {
                eprintln!("[vacancy] Source '{}' error: {}", source, e);
            }
        }
    }

    eprintln!("[vacancy] Search cycle complete");
    Ok(())
}

/// Ensure "Вакансии" project exists, return its ID.
fn ensure_vacancy_project(app: &AppHandle) -> Result<i64, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();

    // Check if exists
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM projects WHERE name = ?1",
        rusqlite::params![VACANCY_PROJECT_NAME],
        |row| row.get(0),
    ).ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create it
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (name, description, status, color, created_at, updated_at) VALUES (?1, ?2, 'active', '#f59e0b', ?3, ?3)",
        rusqlite::params![VACANCY_PROJECT_NAME, "Автопоиск вакансий", now],
    ).map_err(|e| format!("DB error: {}", e))?;

    Ok(conn.last_insert_rowid())
}

/// Get existing task titles in the vacancy project (for deduplication).
fn get_existing_titles(app: &AppHandle, project_id: i64) -> Result<Vec<String>, String> {
    let db = app.state::<HanniDb>();
    let conn = db.conn();

    let mut stmt = conn.prepare(
        "SELECT title FROM tasks WHERE project_id = ?1 AND status != 'done'"
    ).map_err(|e| format!("DB error: {}", e))?;

    let titles: Vec<String> = stmt.query_map(
        rusqlite::params![project_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?
        .flatten()
        .collect();

    Ok(titles)
}
