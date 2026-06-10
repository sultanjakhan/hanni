// api_jobs.rs — HTTP API for the browser extension (apps/jobs-extension):
// lookup a vacancy by URL and upsert application status into job_vacancies.
use crate::types::*;
use crate::commands_meta::{check_auth, ApiState};
use axum::extract::{Query, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use tauri::Manager;

const STAGES: [&str; 9] = [
    "found", "saved", "applied", "responded", "interview",
    "offer", "accepted", "rejected", "ignored",
];

#[derive(Deserialize)]
pub struct VacancyLookupQuery {
    pub url: String,
}

#[derive(Deserialize)]
pub struct VacancySaveReq {
    pub url: String,
    pub company: Option<String>,
    pub position: Option<String>,
    pub salary: Option<String>,
    pub stage: Option<String>,
    pub contact: Option<String>,
    pub source: Option<String>,
    pub notes: Option<String>,
}

/// GET /api/vacancy?url=… — find an existing (non-deleted) vacancy by URL.
pub async fn api_vacancy_lookup(
    headers: HeaderMap,
    AxumState(state): AxumState<ApiState>,
    Query(q): Query<VacancyLookupQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.token)?;
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let row = conn.query_row(
        "SELECT id, company, position, stage, salary, contact, applied_at, source, notes
         FROM job_vacancies WHERE url = ?1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
        rusqlite::params![q.url],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?, "company": row.get::<_, String>(1)?,
                "position": row.get::<_, String>(2)?, "stage": row.get::<_, String>(3)?,
                "salary": row.get::<_, String>(4)?, "contact": row.get::<_, String>(5)?,
                "applied_at": row.get::<_, Option<String>>(6)?, "source": row.get::<_, String>(7)?,
                "notes": row.get::<_, String>(8)?,
            }))
        },
    );
    match row {
        Ok(v) => Ok(Json(serde_json::json!({ "found": true, "vacancy": v }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Json(serde_json::json!({ "found": false }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e))),
    }
}

/// POST /api/vacancy — upsert by URL: update the existing row when the URL is
/// already tracked, insert otherwise. Moving to 'applied' stamps applied_at once.
pub async fn api_vacancy_save(
    headers: HeaderMap,
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<VacancySaveReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.token)?;
    if req.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".into()));
    }
    let stage = req.stage.unwrap_or_else(|| "applied".into());
    if !STAGES.contains(&stage.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("invalid stage '{}'", stage)));
    }
    let now = chrono::Local::now().to_rfc3339();
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let existing: Option<i64> = conn.query_row(
        "SELECT id FROM job_vacancies WHERE url = ?1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
        rusqlite::params![req.url], |row| row.get(0),
    ).ok();
    let err500 = |e: rusqlite::Error| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e));

    let (id, created) = match existing {
        Some(id) => {
            // COALESCE-style update: only overwrite fields the extension sent.
            conn.execute(
                "UPDATE job_vacancies SET
                    stage = ?2,
                    company  = COALESCE(?3, company),  position = COALESCE(?4, position),
                    salary   = COALESCE(?5, salary),   contact  = COALESCE(?6, contact),
                    source   = COALESCE(?7, source),   notes    = COALESCE(?8, notes),
                    applied_at = CASE WHEN ?2 = 'applied' AND applied_at IS NULL THEN ?9 ELSE applied_at END,
                    updated_at = ?9
                 WHERE id = ?1",
                rusqlite::params![
                    id, stage, req.company, req.position, req.salary,
                    req.contact, req.source, req.notes, now
                ],
            ).map_err(err500)?;
            (id, false)
        }
        None => {
            let applied_at: Option<String> = (stage == "applied").then(|| now.clone());
            conn.execute(
                "INSERT INTO job_vacancies
                    (company, position, salary, url, stage, contact, source, notes, applied_at, found_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                rusqlite::params![
                    req.company.unwrap_or_default(), req.position.unwrap_or_default(),
                    req.salary.unwrap_or_default(), req.url, stage,
                    req.contact.unwrap_or_default(), req.source.unwrap_or_default(),
                    req.notes.unwrap_or_default(), applied_at, now
                ],
            ).map_err(err500)?;
            (conn.last_insert_rowid(), true)
        }
    };
    Ok(Json(serde_json::json!({ "status": "ok", "id": id, "created": created })))
}
