//! Internal Android SQL facade: every connection uses the application's rusqlite engine.
//! No Tauri command exposes this interface. Replies never contain raw SQLite errors.
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params_from_iter, types::{Value as SqlValue, ValueRef}};
use crate::worker_connection::WorkerConnection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const MESSAGE_LIMIT: usize = 16 * 1024 * 1024;
const HANDLE_LIMIT: usize = 16;
type Failure = &'static str;
type OpenExisting = fn(&str) -> Result<WorkerConnection, String>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "t", content = "v", deny_unknown_fields)]
enum Cell {
    #[serde(rename = "n")] Null,
    #[serde(rename = "i")] Integer(String),
    #[serde(rename = "f")] Float(String),
    #[serde(rename = "s")] Text(String),
    #[serde(rename = "b")] Blob(String),
}

impl Cell {
    fn bind(self) -> Result<SqlValue, Failure> {
        Ok(match self {
            Self::Null => SqlValue::Null,
            Self::Integer(s) => {
                let n: i64 = s.parse().map_err(|_| "native_db_arguments")?;
                if n.to_string() != s { return Err("native_db_arguments"); }
                SqlValue::Integer(n)
            }
            Self::Float(s) => {
                let n: f64 = s.parse().map_err(|_| "native_db_arguments")?;
                if !n.is_finite() { return Err("native_db_arguments"); }
                SqlValue::Real(n)
            }
            Self::Text(s) => SqlValue::Text(s),
            Self::Blob(s) => SqlValue::Blob(STANDARD.decode(s).map_err(|_| "native_db_arguments")?),
        })
    }
    fn read(value: ValueRef<'_>) -> Result<Self, Failure> {
        Ok(match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(n) => Self::Integer(n.to_string()),
            ValueRef::Real(n) if n.is_finite() => Self::Float(n.to_string()),
            ValueRef::Text(s) => Self::Text(std::str::from_utf8(s).map_err(|_| "native_db_value")?.to_owned()),
            ValueRef::Blob(b) => Self::Blob(STANDARD.encode(b)),
            _ => return Err("native_db_value"),
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum Request {
    #[serde(rename = "open")] Open { path: String },
    #[serde(rename = "close")] Close { handle: String },
    #[serde(rename = "query")] Query { handle: String, sql: String, args: Vec<Cell> },
    #[serde(rename = "execute")] Execute { handle: String, sql: String, args: Vec<Cell> },
    #[serde(rename = "begin")] Begin { handle: String },
    #[serde(rename = "successful")] Successful { handle: String },
    #[serde(rename = "end")] End { handle: String },
    #[serde(rename = "in_transaction")] InTransaction { handle: String },
}

struct Session { connection: Option<WorkerConnection>, successful: bool }
#[derive(Default)]
struct Registry { next: u64, handles: HashMap<u64, Arc<Mutex<Session>>> }

impl Registry {
    fn insert(&mut self, connection: WorkerConnection) -> Result<u64, (Failure, WorkerConnection)> {
        // Return rejected ownership; dropping SQLite may perform IO and must happen
        // after the caller releases the registry mutex, including capacity failures.
        if self.handles.len() >= HANDLE_LIMIT { return Err(("native_db_limit", connection)); }
        let Some(next) = self.next.checked_add(1).filter(|n| *n <= i64::MAX as u64) else {
            return Err(("native_db_limit", connection));
        };
        self.next = next;
        self.handles.insert(self.next, Arc::new(Mutex::new(Session { connection: Some(connection), successful: false })));
        Ok(self.next)
    }
}

fn handle_id(text: &str) -> Result<u64, Failure> {
    let id: u64 = text.parse().map_err(|_| "native_db_closed")?;
    if id == 0 || id.to_string() != text { return Err("native_db_closed"); }
    Ok(id)
}

fn sql<T>(value: rusqlite::Result<T>) -> Result<T, Failure> { value.map_err(|_| "native_db_failed") }

fn dispatch(registry: &Mutex<Registry>, request: Request, open: OpenExisting) -> Result<Value, Failure> {
    let (handle, request) = match request {
        Request::Open { path } => {
            if path.len() > 4096 || !std::path::Path::new(&path).is_absolute() { return Err("native_db_arguments"); }
            // The production opener is cloud_relay::open_existing: READ_WRITE without CREATE,
            // existing WAL/schema checks and the same cr-sqlite extension as native sync.
            let connection = open(&path).map_err(|_| "native_db_not_ready")?;
            let inserted = {
                let mut registry = registry.lock().map_err(|_| "native_db_failed")?;
                registry.insert(connection)
            };
            let id = inserted.map_err(|(code, rejected)| { drop(rejected); code })?;
            return Ok(json!({"handle": id.to_string()}));
        }
        Request::Close { handle } => {
            let session = registry.lock().map_err(|_| "native_db_failed")?.handles.remove(&handle_id(&handle)?)
                .ok_or("native_db_closed")?;
            // Remove first; outstanding callers find a closed session after acquiring its mutex.
            // Recover a poisoned session only to release it, never to continue using its data.
            let mut session = session.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(connection) = session.connection.take() {
                connection.close()?;
            }
            return Ok(Value::Null);
        }
        Request::Query { ref handle, .. } | Request::Execute { ref handle, .. }
        | Request::Begin { ref handle } | Request::Successful { ref handle }
        | Request::End { ref handle } | Request::InTransaction { ref handle } => (handle_id(handle)?, request),
    };
    // Never keep the registry mutex during SQLite IO: another handle may need to commit
    // while this connection waits for its write lock.
    let session = registry.lock().map_err(|_| "native_db_failed")?.handles.get(&handle).cloned().ok_or("native_db_closed")?;
    let mut session = session.lock().map_err(|_| "native_db_failed")?;
    let connection = session.connection.as_ref().ok_or("native_db_closed")?;
    let read = matches!(&request, Request::Query { .. });
    match request {
        Request::Begin { .. } => {
            if !connection.is_autocommit() { return Err("native_db_transaction"); }
            sql(connection.execute_batch("BEGIN EXCLUSIVE"))?;
            session.successful = false;
            Ok(Value::Null)
        }
        Request::Successful { .. } => {
            if connection.is_autocommit() || session.successful { return Err("native_db_transaction"); }
            session.successful = true;
            Ok(Value::Null)
        }
        Request::End { .. } => {
            if connection.is_autocommit() { return Err("native_db_transaction"); }
            let result = sql(connection.execute_batch(if session.successful { "COMMIT" } else { "ROLLBACK" }));
            if result.is_err() && !connection.is_autocommit() { let _ = connection.execute_batch("ROLLBACK"); }
            session.successful = false;
            result.map(|_| Value::Null)
        }
        Request::InTransaction { .. } => Ok(json!(!connection.is_autocommit())),
        Request::Execute { sql: query, args, .. } | Request::Query { sql: query, args, .. } => {
            if query.len() > 256 * 1024 || args.len() > 1000 { return Err("native_db_limit"); }
            let args = args.into_iter().map(Cell::bind).collect::<Result<Vec<_>, _>>()?;
            let mut statement = sql(connection.prepare(&query))?;
            if !read {
                let changes = sql(statement.execute(params_from_iter(args)))?;
                return Ok(json!({"changes": changes.to_string(), "row_id": connection.last_insert_rowid().to_string()}));
            }
            // SQLite marks even the observational journal_mode pragma as non-readonly.
            // Permit only this exact bootstrap probe, never journal_mode assignments.
            if !statement.readonly() && !(query.trim().eq_ignore_ascii_case("PRAGMA journal_mode") && args.is_empty()) {
                return Err("native_db_arguments");
            }
            let columns: Vec<String> = statement.column_names().into_iter().map(str::to_owned).collect();
            let mut cursor = sql(statement.query(params_from_iter(args)))?;
            let mut rows = Vec::new();
            let mut bytes = 0usize;
            while let Some(row) = sql(cursor.next())? {
                if rows.len() >= 10_000 { return Err("native_db_limit"); }
                let cells = (0..columns.len()).map(|i| Cell::read(sql(row.get_ref(i))?)).collect::<Result<Vec<_>, _>>()?;
                bytes += serde_json::to_vec(&cells).map_err(|_| "native_db_value")?.len();
                if bytes > MESSAGE_LIMIT { return Err("native_db_limit"); }
                rows.push(cells);
            }
            Ok(json!({"columns": columns, "rows": rows}))
        }
        _ => Err("native_db_arguments"),
    }
}

pub(crate) fn reply(request: &str, open: OpenExisting) -> String {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    let result = if request.len() > MESSAGE_LIMIT { Err("native_db_limit") } else {
        serde_json::from_str(request).map_err(|_| "native_db_arguments")
            .and_then(|request| dispatch(REGISTRY.get_or_init(Default::default), request, open))
    };
    match result {
        Ok(result) => {
            let reply = json!({"ok": true, "result": result}).to_string();
            if reply.len() <= MESSAGE_LIMIT { reply } else { r#"{"ok":false,"error":"native_db_limit"}"#.to_owned() }
        }
        Err(code) => json!({"ok": false, "error": code}).to_string(),
    }
}

/// If JNI cannot allocate the open response, the caller never receives its handle.
/// Release that connection rather than leaving it registered for the process lifetime.
pub(crate) fn discard_undelivered_reply(response: &str) {
    fn no_open(_: &str) -> Result<WorkerConnection, String> { Err("native_db_arguments".into()) }
    if let Ok(value) = serde_json::from_str::<Value>(response) {
        if let Some(handle) = value.get("result").and_then(|v| v.get("handle")).and_then(Value::as_str) {
            let _ = reply(&json!({"op":"close", "handle":handle}).to_string(), no_open);
        }
    }
}

#[cfg(test)]
#[path = "health_database_tests.rs"]
mod tests;
