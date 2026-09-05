//! Local read models from one authoritative Health Connect store.
//! No health rows from an unowned legacy/manual source are adopted or removed.
use chrono::{DateTime, FixedOffset, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const ERROR: &str = "hc_sleep_projection_failed";
const CONFLICT: &str = "hc_sleep_projection_ownership_conflict";
const VERSION: i64 = 1;

fn sql<T>(r: rusqlite::Result<T>) -> Result<T, String> {
    r.map_err(|_| ERROR.into())
}
fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn valid_hash(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str().ok_or_else(|| ERROR.into())
}
fn instant(v: &Value) -> Result<DateTime<Utc>, String> {
    let s = v["seconds"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(ERROR)?;
    let n = v["nanos"]
        .as_u64()
        .filter(|n| *n < 1_000_000_000)
        .ok_or(ERROR)?;
    DateTime::from_timestamp(s, n as u32).ok_or_else(|| ERROR.into())
}
fn offset(v: &Value, at: DateTime<Utc>) -> Result<(FixedOffset, bool), String> {
    let missing = v.is_null();
    let seconds = if missing {
        at.with_timezone(&chrono::Local).offset().local_minus_utc()
    } else {
        v.as_i64()
            .filter(|n| (-64800..=64800).contains(n))
            .ok_or(ERROR)? as i32
    };
    Ok((FixedOffset::east_opt(seconds).ok_or(ERROR)?, missing))
}
fn numeric_id(raw: &str, kind: &str) -> i64 {
    // Negative 48-bit IDs fit INTEGER PRIMARY KEY and JavaScript safe integers.
    // Occupancy is checked, so a digest collision fails rather than overwrites.
    let hash = Sha256::digest(format!("hanni-raw-sleep-v1:{kind}:{raw}").as_bytes());
    let mut number = 0i64;
    for b in &hash[..6] {
        number = (number << 8) | i64::from(*b);
    }
    -number - 1
}

pub(crate) fn initialize(conn: &Connection) -> Result<(), String> {
    sql(conn.execute_batch("CREATE TABLE IF NOT EXISTS hc_sleep_projection_config(
      singleton INTEGER PRIMARY KEY CHECK(singleton=1),source_store_id TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS hc_sleep_projection_state(
      raw_id TEXT PRIMARY KEY,source_revision INTEGER NOT NULL,payload_sha256 TEXT NOT NULL,
      is_deleted INTEGER NOT NULL,projector_version INTEGER NOT NULL,local_timezone INTEGER NOT NULL,
      render_start_offset INTEGER,render_end_offset INTEGER);
      CREATE TABLE IF NOT EXISTS hc_sleep_projection_owned(
      table_name TEXT NOT NULL,row_id TEXT NOT NULL,raw_id TEXT NOT NULL,
      PRIMARY KEY(table_name,row_id));
      CREATE TABLE IF NOT EXISTS hc_sleep_projection_errors(
      raw_id TEXT PRIMARY KEY,source_revision INTEGER NOT NULL,error_code TEXT NOT NULL,
      attempts INTEGER NOT NULL,next_retry_epoch INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS hc_sleep_projection_progress(
      singleton INTEGER PRIMARY KEY CHECK(singleton=1),last_projected_epoch INTEGER NOT NULL,
      projection_revision INTEGER NOT NULL DEFAULT 0 CHECK(projection_revision>=0));
      CREATE INDEX IF NOT EXISTS hc_sleep_projection_owned_raw ON hc_sleep_projection_owned(raw_id,table_name);"))
}

#[derive(Debug, Serialize)]
pub(crate) struct ProjectionStatus {
    pub status: &'static str,
    /// Successfully committed materializations in this pass, not attempted rows.
    pub records: usize,
    /// Eligible work now. Cooling-down failures do not cause immediate continuations.
    pub more_pending: bool,
    pub local_timezone_records: usize,
    pub pending_records: usize,
    pub errors: usize,
    pub retry_needed: bool,
    pub next_retry_epoch: Option<i64>,
    pub retry_after_seconds: Option<i64>,
    pub last_projected_epoch: Option<i64>,
    pub projection_revision: String,
}

const RETRY_BASE: i64 = 30;
const RETRY_MAX: i64 = 3600;
fn validate_authority(authority: &str) -> Result<(), String> {
    if uuid::Uuid::parse_str(authority)
        .map(|u| u.to_string())
        .ok()
        .as_deref()
        != Some(authority)
    {
        return Err(ERROR.into());
    }
    Ok(())
}
fn has_table(conn: &Connection, name: &str) -> Result<bool, String> {
    sql(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |r| r.get(0),
    ))
}
fn check_authority(conn: &Connection, authority: &str) -> Result<(), String> {
    if has_table(conn, "hc_sleep_projection_config")? {
        let prior: Option<String> = sql(conn
            .query_row(
                "SELECT source_store_id FROM hc_sleep_projection_config WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .optional())?;
        if prior.as_deref().is_some_and(|s| s != authority) {
            return Err("hc_sleep_projection_authority_change_required".into());
        }
    }
    Ok(())
}
fn empty_status(status: &'static str) -> ProjectionStatus {
    ProjectionStatus {
        status,
        records: 0,
        more_pending: false,
        local_timezone_records: 0,
        pending_records: 0,
        errors: 0,
        retry_needed: false,
        next_retry_epoch: None,
        retry_after_seconds: None,
        last_projected_epoch: None,
        projection_revision: "0".into(),
    }
}
struct Candidate {
    id: String,
    revision: i64,
    invalid_clock_metadata: bool,
}
struct Pending {
    rows: Vec<Candidate>,
    total: usize,
    eligible: usize,
    errors: usize,
    next_retry: Option<i64>,
    last_projected: Option<i64>,
    revision: i64,
}

/// Stream only lightweight metadata. The caller controls the write transaction.
fn scan_pending(
    conn: &Connection,
    authority: &str,
    limit: usize,
    now: i64,
) -> Result<Pending, String> {
    let has_errors = has_table(conn, "hc_sleep_projection_errors")?;
    let error_join = if has_errors {
        "LEFT JOIN hc_sleep_projection_errors e ON e.raw_id=h.id"
    } else {
        "LEFT JOIN (SELECT NULL raw_id,NULL source_revision,NULL next_retry_epoch WHERE 0) e ON e.raw_id=h.id"
    };
    let statement = format!("SELECT h.id,h.source_revision,p.source_revision,p.projector_version,
        p.render_start_offset,p.render_end_offset,h.time_start_utc,h.time_end_utc,
        h.is_deleted,p.is_deleted,h.payload_sha256,p.payload_sha256,e.source_revision,e.next_retry_epoch
      FROM health_records h LEFT JOIN hc_sleep_projection_state p ON p.raw_id=h.id {error_join}
      WHERE h.source_store_id=?1 AND h.record_type='SleepSessionRecord'
      ORDER BY CASE WHEN e.source_revision=h.source_revision THEN 1 ELSE 0 END,h.id");
    let mut query = sql(conn.prepare(&statement))?;
    let rows = sql(query.query_map([authority], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i32>>(4)?,
            r.get::<_, Option<i32>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, i64>(8)?,
            r.get::<_, Option<i64>>(9)?,
            r.get::<_, String>(10)?,
            r.get::<_, Option<String>>(11)?,
            r.get::<_, Option<i64>>(12)?,
            r.get::<_, Option<i64>>(13)?,
        ))
    }))?;
    let mut result = Pending {
        rows: Vec::new(),
        total: 0,
        eligible: 0,
        errors: 0,
        next_retry: None,
        last_projected: None,
        revision: 0,
    };
    for row in rows {
        let (
            id,
            revision,
            prior,
            version,
            start_offset,
            end_offset,
            start,
            end,
            deleted,
            prior_deleted,
            hash,
            prior_hash,
            failed_revision,
            next_retry,
        ) = sql(row)?;
        let mut changed = prior != Some(revision)
            || version != Some(VERSION)
            || prior_deleted != Some(deleted)
            || prior_hash.as_deref() != Some(hash.as_str());
        let mut invalid_clock_metadata = false;
        if !changed && deleted == 0 {
            for (prior, at) in [(start_offset, start), (end_offset, end)] {
                if let Some(prior) = prior {
                    match at
                        .as_deref()
                        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                    {
                        Some(at) => {
                            changed |=
                                at.with_timezone(&chrono::Local).offset().local_minus_utc() != prior
                        }
                        None => {
                            changed = true;
                            invalid_clock_metadata = true;
                        }
                    }
                }
            }
        }
        if !changed {
            continue;
        }
        result.total += 1;
        let mut ready = true;
        if failed_revision == Some(revision) {
            result.errors += 1;
            let next = next_retry.unwrap_or(now);
            // A backwards system-clock correction must not freeze work for days.
            let due = if next > now.saturating_add(RETRY_MAX) {
                now
            } else {
                next.max(now)
            };
            result.next_retry = Some(result.next_retry.map_or(due, |v| v.min(due)));
            ready = due <= now;
        }
        if ready {
            result.eligible += 1;
            if result.rows.len() < limit {
                result.rows.push(Candidate {
                    id,
                    revision,
                    invalid_clock_metadata,
                });
            }
        }
    }
    if has_table(conn, "hc_sleep_projection_progress")? {
        let progress: Option<(i64,i64)> = sql(conn.query_row("SELECT last_projected_epoch,projection_revision FROM hc_sleep_projection_progress WHERE singleton=1", [], |r| Ok((r.get(0)?,r.get(1)?))).optional())?;
        if let Some((at, revision)) = progress {
            result.last_projected = Some(at);
            result.revision = revision;
        }
    }
    Ok(result)
}
fn status_from_scan(scan: Pending, now: i64) -> ProjectionStatus {
    ProjectionStatus {
        status: if scan.errors > 0 {
            "projection_partial"
        } else if scan.total > 0 {
            "projection_pending"
        } else {
            "projected"
        },
        records: 0,
        more_pending: scan.eligible > 0,
        local_timezone_records: 0,
        pending_records: scan.total,
        errors: scan.errors,
        retry_needed: scan.errors > 0,
        next_retry_epoch: scan.next_retry,
        retry_after_seconds: scan.next_retry.map(|v| v.saturating_sub(now).max(0)),
        last_projected_epoch: scan.last_projected,
        projection_revision: scan.revision.to_string(),
    }
}

/// Read-only aggregate status: never initializes schema, claims authority or retries rows.
pub(crate) fn database_status(
    conn: &Connection,
    authority: Option<&str>,
) -> Result<ProjectionStatus, String> {
    database_status_at(conn, authority, Utc::now().timestamp())
}
fn database_status_at(
    conn: &Connection,
    authority: Option<&str>,
    now: i64,
) -> Result<ProjectionStatus, String> {
    let Some(authority) = authority else {
        return Ok(empty_status("authority_not_configured"));
    };
    validate_authority(authority)?;
    check_authority(conn, authority)?;
    if !has_table(conn, "hc_sleep_projection_state")? || !has_table(conn, "health_records")? {
        let mut status = empty_status("projection_not_initialized");
        if has_table(conn, "health_records")? {
            status.pending_records = sql(conn.query_row("SELECT COUNT(*) FROM health_records WHERE source_store_id=?1 AND record_type='SleepSessionRecord'", [authority], |r| r.get(0)))?;
            status.more_pending = status.pending_records > 0;
        }
        return Ok(status);
    }
    Ok(status_from_scan(
        scan_pending(conn, authority, 0, now)?,
        now,
    ))
}

fn safe_record_error(error: &str) -> &'static str {
    match error {
        CONFLICT => CONFLICT,
        "hc_sleep_projection_timeline_type_required" => {
            "hc_sleep_projection_timeline_type_required"
        }
        "hc_sleep_projection_revision_regressed" => "hc_sleep_projection_revision_regressed",
        _ => ERROR,
    }
}
fn latch_failure(conn: &Connection, row: &Candidate, error: &str, now: i64) -> Result<(), String> {
    let prior: Option<(i64, i64)> = sql(conn
        .query_row(
            "SELECT source_revision,attempts FROM hc_sleep_projection_errors WHERE raw_id=?1",
            [&row.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional())?;
    let attempts = prior
        .filter(|(revision, _)| *revision == row.revision)
        .map_or(1, |(_, n)| n.saturating_add(1).clamp(1, 32));
    let delay = RETRY_BASE
        .saturating_mul(1_i64 << (attempts - 1).min(7))
        .min(RETRY_MAX);
    let next = now.saturating_add(delay);
    let error = safe_record_error(error);
    if prior.is_some() {
        sql(conn.execute("UPDATE hc_sleep_projection_errors SET source_revision=?2,error_code=?3,attempts=?4,next_retry_epoch=?5 WHERE raw_id=?1", params![row.id,row.revision,error,attempts,next]))?;
    } else {
        sql(conn.execute("INSERT INTO hc_sleep_projection_errors(raw_id,source_revision,error_code,attempts,next_retry_epoch) VALUES(?1,?2,?3,?4,?5)", params![row.id,row.revision,error,attempts,next]))?;
    }
    Ok(())
}

/// Caller owns IMMEDIATE transaction and remote-apply controls. One bad row is
/// rolled back to its SAVEPOINT; healthy rows and safe retry latches commit together.
pub(crate) fn reconcile_pending(
    conn: &Connection,
    authority: Option<&str>,
    limit: usize,
) -> Result<ProjectionStatus, String> {
    reconcile_pending_at(conn, authority, limit, Utc::now().timestamp())
}
fn reconcile_pending_at(
    conn: &Connection,
    authority: Option<&str>,
    limit: usize,
    now: i64,
) -> Result<ProjectionStatus, String> {
    let Some(authority) = authority else {
        return Ok(empty_status("authority_not_configured"));
    };
    validate_authority(authority)?;
    if conn.is_autocommit() || limit == 0 || limit > 1000 {
        return Err(ERROR.into());
    }
    initialize(conn)?;
    check_authority(conn, authority)?;
    sql(conn.execute(
        "INSERT OR IGNORE INTO hc_sleep_projection_config VALUES(1,?1)",
        [authority],
    ))?;
    let remote: i64 = sql(conn.query_row(
        "SELECT remote_apply FROM sync_apply_context WHERE singleton=1",
        [],
        |r| r.get(0),
    ))?;
    let applying: i64 = sql(conn.query_row(
        "SELECT applying FROM cloud_relay_control WHERE id=1",
        [],
        |r| r.get(0),
    ))?;
    if remote != 1 || applying != 1 {
        return Err("hc_sleep_projection_context_required".into());
    }
    let scan = scan_pending(conn, authority, limit, now)?;
    let mut count = 0;
    let mut fallback = 0;
    for row in scan.rows {
        sql(conn.execute_batch("SAVEPOINT hc_sleep_projection_row"))?;
        let result = if row.invalid_clock_metadata {
            Err(ERROR.into())
        } else {
            reconcile_one(conn, &row.id, authority)
        };
        match result {
            Ok(local_timezone) => {
                sql(conn.execute(
                    "DELETE FROM hc_sleep_projection_errors WHERE raw_id=?1",
                    [&row.id],
                ))?;
                sql(conn.execute_batch("RELEASE hc_sleep_projection_row"))?;
                count += 1;
                fallback += usize::from(local_timezone);
            }
            Err(error) => {
                sql(conn.execute_batch(
                    "ROLLBACK TO hc_sleep_projection_row; RELEASE hc_sleep_projection_row",
                ))?;
                latch_failure(conn, &row, &error, now)?;
            }
        }
    }
    if count > 0 {
        sql(conn.execute("INSERT OR IGNORE INTO hc_sleep_projection_progress(singleton,last_projected_epoch) VALUES(1,?1)",[now]))?;
        let previous: i64 = sql(conn.query_row(
            "SELECT projection_revision FROM hc_sleep_projection_progress WHERE singleton=1",
            [],
            |r| r.get(0),
        ))?;
        let revision = previous.checked_add(1).ok_or(ERROR)?;
        sql(conn.execute("UPDATE hc_sleep_projection_progress SET last_projected_epoch=?1,projection_revision=?2 WHERE singleton=1",params![now,revision]))?;
    }
    let mut status = status_from_scan(scan_pending(conn, authority, 0, now)?, now);
    status.records = count;
    status.local_timezone_records = fallback;
    Ok(status)
}

struct Sleep {
    date: String,
    start: String,
    end: String,
    minutes: i64,
    notes: String,
    stages: BTreeMap<String, (String, String, String)>,
    fallback: bool,
    render_start_offset: Option<i32>,
    render_end_offset: Option<i32>,
}
fn decode(raw_id: &str, payload: &str) -> Result<Sleep, String> {
    let value: Value = serde_json::from_str(payload).map_err(|_| ERROR)?;
    if value["v"] != 1
        || value["sdk"] != "androidx.health.connect:connect-client:1.1.0"
        || value["record_type"] != "SleepSessionRecord"
    {
        return Err(ERROR.into());
    }
    let r = &value["record"];
    let start = instant(&r["startTime"])?;
    let end = instant(&r["endTime"])?;
    if end <= start {
        return Err(ERROR.into());
    }
    let (start_offset, sf) = offset(&r["startZoneOffset"], start)?;
    let (end_offset, ef) = offset(&r["endZoneOffset"], end)?;
    let local_start = start.with_timezone(&start_offset);
    let local_end = end.with_timezone(&end_offset);
    let notes = if r["notes"].is_null() {
        String::new()
    } else {
        field(r, "notes")?.to_owned()
    };
    let mut stages = BTreeMap::new();
    for stage in r["stages"].as_array().ok_or(ERROR)? {
        let a = instant(&stage["startTime"])?;
        let b = instant(&stage["endTime"])?;
        if a < start || b > end || b <= a {
            return Err(ERROR.into());
        }
        let kind = match stage["stage"].as_i64().ok_or(ERROR)? {
            0 => "unknown".into(),
            1 => "awake".into(),
            2 => "sleeping".into(),
            3 => "out_of_bed".into(),
            4 => "light".into(),
            5 => "deep".into(),
            6 => "rem".into(),
            7 => "awake_in_bed".into(),
            n => format!("unknown:{n}"),
        };
        let stage_start_offset = if sf {
            offset(&Value::Null, a)?.0
        } else {
            start_offset
        };
        let stage_end_offset = if sf {
            offset(&Value::Null, b)?.0
        } else {
            start_offset
        };
        let from = a
            .with_timezone(&stage_start_offset)
            .format("%H:%M")
            .to_string();
        let to = b
            .with_timezone(&stage_end_offset)
            .format("%H:%M")
            .to_string();
        // Existing stage display is minute-resolution. Coalesce identical display tuples;
        // exact instants, offsets, unknown enums and every original sample remain in raw.
        let key = serde_json::to_vec(&[raw_id, &from, &to, &kind]).map_err(|_| ERROR)?;
        let id = format!("raw-stage:{}", digest(&key));
        stages.insert(id, (from, to, kind));
    }
    Ok(Sleep {
        date: local_start.format("%Y-%m-%d").to_string(),
        start: local_start.format("%H:%M").to_string(),
        end: local_end.format("%H:%M").to_string(),
        minutes: (end - start).num_seconds().div_euclid(60).max(1),
        notes,
        stages,
        fallback: sf || ef,
        render_start_offset: sf.then_some(start_offset.local_minus_utc()),
        render_end_offset: ef.then_some(end_offset.local_minus_utc()),
    })
}
fn owned(conn: &Connection, table: &str, id: &str, raw: &str) -> Result<(), String> {
    let previous: Option<String> = sql(conn
        .query_row(
            "SELECT raw_id FROM hc_sleep_projection_owned WHERE table_name=?1 AND row_id=?2",
            params![table, id],
            |r| r.get(0),
        )
        .optional())?;
    if previous.as_deref().is_some_and(|v| v != raw) {
        return Err(CONFLICT.into());
    }
    sql(conn.execute(
        "INSERT OR IGNORE INTO hc_sleep_projection_owned VALUES(?1,?2,?3)",
        params![table, id, raw],
    ))?;
    Ok(())
}
fn source_matches(
    conn: &Connection,
    table: &str,
    id: &str,
    expected: &str,
) -> Result<bool, String> {
    let actual: Option<String> = sql(conn
        .query_row(
            &format!("SELECT source FROM {table} WHERE CAST(id AS TEXT)=?1"),
            [id],
            |r| r.get(0),
        )
        .optional())?;
    if actual.as_deref().is_some_and(|s| s != expected) {
        return Err(CONFLICT.into());
    }
    Ok(actual.is_some())
}
fn stage_ids(conn: &Connection, raw: &str) -> Result<BTreeSet<String>, String> {
    let mut query=sql(conn.prepare("SELECT row_id FROM hc_sleep_projection_owned WHERE raw_id=?1 AND table_name='sleep_stages'"))?;
    let rows = sql(query.query_map([raw], |r| r.get::<_, String>(0)))?;
    sql(rows.collect())
}

fn reconcile_one(conn: &Connection, raw: &str, authority: &str) -> Result<bool, String> {
    if !valid_hash(raw) {
        return Err(ERROR.into());
    }
    let (revision,hash,deleted,payload,updated):(i64,String,i64,String,String)=sql(conn.query_row(
      "SELECT source_revision,payload_sha256,is_deleted,payload_json,updated_at FROM health_records WHERE id=?1 AND source_store_id=?2 AND record_type='SleepSessionRecord'",
      params![raw,authority],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))))?;
    if revision <= 0
        || ![0, 1].contains(&deleted)
        || digest(payload.as_bytes()) != hash
        || DateTime::parse_from_rfc3339(&updated).is_err()
    {
        return Err(ERROR.into());
    }
    let previous: Option<i64> = sql(conn
        .query_row(
            "SELECT source_revision FROM hc_sleep_projection_state WHERE raw_id=?1",
            [raw],
            |r| r.get(0),
        )
        .optional())?;
    if previous.is_some_and(|v| v > revision) {
        return Err("hc_sleep_projection_revision_regressed".into());
    }
    let sid = format!("raw-sleep:{raw}");
    let source = format!("health_connect_raw:{raw}");
    let event = numeric_id(raw, "calendar");
    let timeline = numeric_id(raw, "timeline");
    let auto = format!("auto_health_raw:{raw}");
    let sleep_exists = source_matches(conn, "sleep_sessions", &sid, &source)?;
    let event_exists = source_matches(conn, "events", &event.to_string(), &auto)?;
    let timeline_exists = source_matches(conn, "timeline_blocks", &timeline.to_string(), &auto)?;
    let stages_owned = stage_ids(conn, raw)?;
    // A user-added child under an automatic parent must not disappear by cascade.
    let mut query = sql(conn.prepare("SELECT id FROM sleep_stages WHERE session_id=?1"))?;
    let rows = sql(query.query_map([&sid], |r| r.get::<_, String>(0)))?;
    let existing: Vec<String> = sql(rows.collect())?;
    if existing.iter().any(|id| !stages_owned.contains(id)) {
        return Err(CONFLICT.into());
    }
    owned(conn, "sleep_sessions", &sid, raw)?;
    owned(conn, "events", &event.to_string(), raw)?;
    owned(conn, "timeline_blocks", &timeline.to_string(), raw)?;
    let mut fallback = false;
    let mut render_start_offset = None;
    let mut render_end_offset = None;
    if deleted == 1 {
        for id in &existing {
            sql(conn.execute(
                "DELETE FROM sleep_stages WHERE id=?1 AND session_id=?2",
                params![id, sid],
            ))?;
        }
        if sleep_exists {
            sql(conn.execute(
                "DELETE FROM sleep_sessions WHERE id=?1 AND source=?2",
                params![sid, source],
            ))?;
        }
        if event_exists {
            sql(conn.execute(
                "DELETE FROM events WHERE id=?1 AND source=?2",
                params![event, auto],
            ))?;
        }
        if timeline_exists {
            sql(conn.execute(
                "DELETE FROM timeline_blocks WHERE id=?1 AND source=?2",
                params![timeline, auto],
            ))?;
        }
    } else {
        let sleep = decode(raw, &payload)?;
        fallback = sleep.fallback;
        render_start_offset = sleep.render_start_offset;
        render_end_offset = sleep.render_end_offset;
        if sleep_exists {
            sql(conn.execute("UPDATE sleep_sessions SET date=?2,start_time=?3,end_time=?4,duration_minutes=?5,notes=?6,updated_at=?7 WHERE id=?1 AND (date IS NOT ?2 OR start_time IS NOT ?3 OR end_time IS NOT ?4 OR duration_minutes IS NOT ?5 OR notes IS NOT ?6)",params![sid,sleep.date,sleep.start,sleep.end,sleep.minutes,sleep.notes,updated]))?;
        } else {
            sql(conn.execute("INSERT INTO sleep_sessions(id,date,start_time,end_time,duration_minutes,source,notes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![sid,sleep.date,sleep.start,sleep.end,sleep.minutes,source,sleep.notes,updated]))?;
        }
        for id in &existing {
            if !sleep.stages.contains_key(id) {
                sql(conn.execute(
                    "DELETE FROM sleep_stages WHERE id=?1 AND session_id=?2",
                    params![id, sid],
                ))?;
            }
        }
        for (id, (from, to, kind)) in &sleep.stages {
            let parent: Option<String> = sql(conn
                .query_row(
                    "SELECT session_id FROM sleep_stages WHERE id=?1",
                    [id],
                    |r| r.get(0),
                )
                .optional())?;
            if parent.as_deref().is_some_and(|p| p != sid) {
                return Err(CONFLICT.into());
            }
            owned(conn, "sleep_stages", id, raw)?;
            if parent.is_none() {
                sql(conn.execute("INSERT INTO sleep_stages(id,session_id,start_time,end_time,stage,updated_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,sid,from,to,kind,updated]))?;
            }
        }
        if event_exists {
            sql(conn.execute("UPDATE events SET date=?2,time=?3,duration_minutes=?4,description=?5,updated_at=?6 WHERE id=?1 AND (date IS NOT ?2 OR time IS NOT ?3 OR duration_minutes IS NOT ?4 OR description IS NOT ?5)",params![event,sleep.date,sleep.start,sleep.minutes,sleep.notes,updated]))?;
        } else {
            sql(conn.execute("INSERT INTO events(id,title,description,date,time,duration_minutes,category,color,source,external_id,created_at,updated_at) VALUES(?1,'Сон',?2,?3,?4,?5,'health','#3b82f6',?6,?7,?8,?8)",params![event,sleep.notes,sleep.date,sleep.start,sleep.minutes,auto,sid,updated]))?;
        }
        let mut types = sql(conn.prepare(
            "SELECT id FROM timeline_activity_types WHERE name='Сон' AND is_system=1 LIMIT 2",
        ))?;
        let rows = sql(types.query_map([], |r| r.get::<_, i64>(0)))?;
        let types: Vec<i64> = sql(rows.collect())?;
        if types.len() != 1 {
            return Err("hc_sleep_projection_timeline_type_required".into());
        }
        if timeline_exists {
            sql(conn.execute("UPDATE timeline_blocks SET type_id=?2,date=?3,start_time=?4,end_time=?5,duration_minutes=?6,notes=?7,updated_at=?8 WHERE id=?1 AND (type_id IS NOT ?2 OR date IS NOT ?3 OR start_time IS NOT ?4 OR end_time IS NOT ?5 OR duration_minutes IS NOT ?6 OR notes IS NOT ?7)",params![timeline,types[0],sleep.date,sleep.start,sleep.end,sleep.minutes,sleep.notes,updated]))?;
        } else {
            sql(conn.execute("INSERT INTO timeline_blocks(id,type_id,date,start_time,end_time,duration_minutes,source,notes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",params![timeline,types[0],sleep.date,sleep.start,sleep.end,sleep.minutes,auto,sleep.notes,updated]))?;
        }
    }
    if previous.is_some() {
        sql(conn.execute("UPDATE hc_sleep_projection_state SET source_revision=?2,payload_sha256=?3,is_deleted=?4,projector_version=?5,local_timezone=?6,render_start_offset=?7,render_end_offset=?8 WHERE raw_id=?1",params![raw,revision,hash,deleted,VERSION,fallback as i32,render_start_offset,render_end_offset]))?;
    } else {
        sql(conn.execute(
            "INSERT INTO hc_sleep_projection_state VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                raw,
                revision,
                hash,
                deleted,
                VERSION,
                fallback as i32,
                render_start_offset,
                render_end_offset
            ],
        ))?;
    }
    Ok(fallback)
}

/// Command guard for owned local read models; manual/unbound history stays editable.
pub(crate) fn ensure_user_editable(conn: &Connection, table: &str, id: &str) -> Result<(), String> {
    if is_local_projection(conn, table, id)? {
        return Err(
            "Эта запись получена из Health Connect. Исправьте её в приложении-источнике.".into(),
        );
    }
    Ok(())
}

/// Shared legacy transport guard. Ownership remains after deletion for tomb filtering.
pub(crate) fn is_local_projection(
    conn: &Connection,
    table: &str,
    id: &str,
) -> Result<bool, String> {
    if (table == "sleep_sessions" && id.starts_with("raw-sleep:"))
        || (table == "sleep_stages" && id.starts_with("raw-stage:"))
    {
        return Ok(true);
    }
    if ["events", "timeline_blocks"].contains(&table) && has_source(conn, table)? {
        let marker:bool=sql(conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE CAST(id AS TEXT)=?1 AND source LIKE 'auto_health_raw:%')"),[id],|r|r.get(0)))?;
        if marker {
            return Ok(true);
        }
    }
    let has:bool=sql(conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='hc_sleep_projection_owned' AND type='table')",[],|r|r.get(0)))?;
    if !has {
        return Ok(false);
    }
    sql(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM hc_sleep_projection_owned WHERE table_name=?1 AND row_id=?2)",
        params![table, id],
        |r| r.get(0),
    ))
}

fn ownership_table(conn: &Connection) -> Result<bool, String> {
    sql(conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='hc_sleep_projection_owned' AND type='table')",[],|r|r.get(0)))
}
fn has_source(conn: &Connection, table: &str) -> Result<bool, String> {
    sql(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name='source')",
        [table],
        |r| r.get(0),
    ))
}
/// Apply before LIMIT/cutoff calculation, otherwise local rows can starve a LAN page.
pub(crate) fn transport_row_filter(conn: &Connection, table: &str) -> Result<String, String> {
    if table.is_empty()
        || !table
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(ERROR.into());
    }
    let mut parts = vec!["1".to_string()];
    if table == "sleep_sessions" {
        parts.push(format!("CAST({table}.id AS TEXT) NOT GLOB 'raw-sleep:*'"));
    }
    if table == "sleep_stages" {
        parts.push(format!("CAST({table}.id AS TEXT) NOT GLOB 'raw-stage:*'"));
    }
    if ["events", "timeline_blocks"].contains(&table) && has_source(conn, table)? {
        parts.push(format!(
            "COALESCE({table}.source,'') NOT GLOB 'auto_health_raw:*'"
        ));
    }
    if ownership_table(conn)? {
        parts.push(format!("NOT EXISTS(SELECT 1 FROM hc_sleep_projection_owned o WHERE o.table_name='{table}' AND o.row_id=CAST({table}.id AS TEXT))"));
    }
    Ok(parts.join(" AND "))
}
/// Expressions are internal SQL column references supplied by transport modules.
pub(crate) fn transport_tomb_filter(
    conn: &Connection,
    table_column: &str,
    id_column: &str,
) -> Result<String, String> {
    let mut parts=vec![format!("NOT ({table_column}='sleep_sessions' AND CAST({id_column} AS TEXT) GLOB 'raw-sleep:*')"),
      format!("NOT ({table_column}='sleep_stages' AND CAST({id_column} AS TEXT) GLOB 'raw-stage:*')")];
    if ownership_table(conn)? {
        parts.push(format!("NOT EXISTS(SELECT 1 FROM hc_sleep_projection_owned o WHERE o.table_name={table_column} AND o.row_id=CAST({id_column} AS TEXT))"));
    }
    Ok(parts.join(" AND "))
}

#[cfg(test)]
#[path = "health_raw_sleep_projection_tests.rs"]
mod tests;
