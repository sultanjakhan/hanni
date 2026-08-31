// api_jobs.rs — HTTP API for the browser extension (apps/jobs-extension):
// lookup a vacancy by URL and upsert application status into job_vacancies.
use crate::types::*;
use crate::commands_meta::{check_auth, ApiState};
use axum::extract::{Query, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use tauri::Manager;

const STAGES: [&str; 9] = [
    "found", "saved", "applied", "responded", "interview",
    "offer", "accepted", "rejected", "ignored",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VacancyLookupQuery {
    pub url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

fn normalized_vacancy_url(raw: &str) -> Result<String, (StatusCode, String)> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 2048 || raw.chars().any(char::is_control) {
        return Err((StatusCode::BAD_REQUEST, "url is invalid or too long".into()));
    }
    let mut url = reqwest::Url::parse(raw).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "url must be an absolute http(s) URL".into(),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "url must be a public http(s) URL without credentials".into(),
        ));
    }
    url.set_fragment(None);
    let normalized = url.to_string();
    if normalized.len() > 2048 {
        return Err((StatusCode::BAD_REQUEST, "normalized url is too long".into()));
    }
    Ok(normalized)
}

fn clean_optional_field(
    name: &str,
    value: Option<String>,
    max_bytes: usize,
    multiline: bool,
) -> Result<Option<String>, (StatusCode, String)> {
    let Some(value) = value else { return Ok(None) };
    let value = value.trim().to_string();
    let bad_control = value
        .chars()
        .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')));
    if value.len() > max_bytes || bad_control {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{name} is invalid or too long"),
        ));
    }
    Ok(Some(value))
}

/// GET /api/vacancy?url=… — find an existing (non-deleted) vacancy by URL.
pub async fn api_vacancy_lookup(
    headers: HeaderMap,
    AxumState(state): AxumState<ApiState>,
    Query(q): Query<VacancyLookupQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.jobs_token)?;
    let url = normalized_vacancy_url(&q.url)?;
    let db = state.app.state::<HanniDb>();
    let conn = db.conn();
    let row = conn.query_row(
        "SELECT id, company, position, stage, salary, contact, applied_at, source, notes
         FROM job_vacancies WHERE url = ?1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
        rusqlite::params![url],
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
    check_auth(&headers, &state.jobs_token)?;
    let url = normalized_vacancy_url(&req.url)?;
    let stage = req.stage.unwrap_or_else(|| "applied".into());
    if !STAGES.contains(&stage.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("invalid stage '{}'", stage)));
    }
    let company = clean_optional_field("company", req.company, 200, false)?;
    let position = clean_optional_field("position", req.position, 300, false)?;
    let salary = clean_optional_field("salary", req.salary, 120, false)?;
    let contact = clean_optional_field("contact", req.contact, 300, false)?;
    let source = clean_optional_field("source", req.source, 100, false)?;
    let notes = clean_optional_field("notes", req.notes, 4000, true)?;
    let now = chrono::Local::now().to_rfc3339();
    let db = state.app.state::<HanniDb>();
    let mut conn = db.conn();
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB transaction: {e}"),
            )
        })?;
    let existing: Option<i64> = tx.query_row(
        "SELECT id FROM job_vacancies WHERE url = ?1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
        rusqlite::params![url], |row| row.get(0),
    ).optional().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB lookup: {e}")))?;
    let err500 = |e: rusqlite::Error| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e));

    let (id, created) = match existing {
        Some(id) => {
            // COALESCE-style update: only overwrite fields the extension sent.
            tx.execute(
                "UPDATE job_vacancies SET
                    stage = ?2,
                    company  = COALESCE(?3, company),  position = COALESCE(?4, position),
                    salary   = COALESCE(?5, salary),   contact  = COALESCE(?6, contact),
                    source   = COALESCE(?7, source),   notes    = COALESCE(?8, notes),
                    applied_at = CASE WHEN ?2 = 'applied' AND applied_at IS NULL THEN ?9 ELSE applied_at END,
                    updated_at = ?9
                 WHERE id = ?1",
                rusqlite::params![
                    id, stage, company, position, salary,
                    contact, source, notes, now
                ],
            ).map_err(err500)?;
            (id, false)
        }
        None => {
            let applied_at: Option<String> = (stage == "applied").then(|| now.clone());
            tx.execute(
                "INSERT INTO job_vacancies
                    (company, position, salary, url, stage, contact, source, notes, applied_at, found_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                rusqlite::params![
                    company.unwrap_or_default(), position.unwrap_or_default(),
                    salary.unwrap_or_default(), url, stage,
                    contact.unwrap_or_default(), source.unwrap_or_default(),
                    notes.unwrap_or_default(), applied_at, now
                ],
            ).map_err(err500)?;
            (tx.last_insert_rowid(), true)
        }
    };
    tx.commit().map_err(err500)?;
    Ok(Json(serde_json::json!({ "status": "ok", "id": id, "created": created })))
}
