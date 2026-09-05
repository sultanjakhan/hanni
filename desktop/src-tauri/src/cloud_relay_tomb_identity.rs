//! Logical deletion identities. Only the small hash travels inside encrypted tombs.
//! Registry triggers use ordinary SQLite expressions, including on Kotlin writers.
use super::{check_table, exists, row_id, sql, text};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TombIdentity {
    pub v: u8,
    pub natural_key_sha256: String,
}
impl TombIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.v != 1
            || self.natural_key_sha256.len() != 64
            || !self
                .natural_key_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("relay_identity_invalid_tomb".into());
        }
        Ok(())
    }
}
fn encoded(parts: &[&str]) -> String {
    parts.iter().map(|p| format!("{}:{p}", p.len())).collect()
}
fn decoded(mut raw: &str) -> Result<Vec<&str>, String> {
    let mut parts = Vec::new();
    while !raw.is_empty() {
        let (size, tail) = raw.split_once(':').ok_or("relay_identity_invalid_tuple")?;
        let size: usize = size.parse().map_err(|_| "relay_identity_invalid_tuple")?;
        if size > tail.len() || !tail.is_char_boundary(size) {
            return Err("relay_identity_invalid_tuple".into());
        }
        parts.push(&tail[..size]);
        raw = &tail[size..];
    }
    Ok(parts)
}
fn digest(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}
pub(super) fn key_hash(table: &str, raw: &str) -> Result<String, String> {
    if table != "sleep_stages" {
        return Ok(digest(&encoded(&["hanni-natural-v1", table, raw])));
    }
    let parts = decoded(raw)?;
    if parts.len() != 4 {
        return Err("relay_identity_invalid_tuple".into());
    }
    // Parent snapshot is its natural tuple, never its device-local session_id.
    // A tomb received before its row can supply the already-hashed parent identity.
    let parent = match parts[0].strip_prefix('!') {
        Some(hash) => {
            TombIdentity {
                v: 1,
                natural_key_sha256: hash.into(),
            }
            .validate()?;
            hash.to_owned()
        }
        None => key_hash("sleep_sessions", parts[0])?,
    };
    Ok(digest(&encoded(&[
        "hanni-natural-v1",
        table,
        &parent,
        parts[1],
        parts[2],
        parts[3],
    ])))
}
fn sql_tuple(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| format!("CAST(length(CAST(({p}) AS BLOB)) AS TEXT)||':'||({p})"))
        .collect::<Vec<_>>()
        .join("||")
}
fn expression(table: &str, alias: &str) -> Result<String, String> {
    let col = |name: &str| format!("{alias}.{name}");
    let strict = |names: &[&str]| {
        let checks = names
            .iter()
            .map(|n| format!("{} IS NOT NULL AND {}<>''", col(n), col(n)))
            .collect::<Vec<_>>()
            .join(" AND ");
        let tuple = sql_tuple(&names.iter().map(|n| col(n)).collect::<Vec<_>>());
        format!("CASE WHEN {checks} THEN {tuple} END")
    };
    Ok(match table {
        "sleep_sessions" => strict(&["date", "start_time", "source"]),
        "heart_rate_samples" => strict(&["date", "time", "source"]),
        "health_log" => {
            let steps = sql_tuple(&["'steps'".into(), col("date")]);
            let exercise = sql_tuple(&[
                "'exercise'".into(),
                col("date"),
                format!("COALESCE({},'')", col("start_time")),
                col("notes"),
            ]);
            format!("CASE WHEN {}='steps' THEN {steps} WHEN {}='exercise' AND {} IS NOT NULL THEN {exercise} END",col("type"),col("type"),col("notes"))
        }
        "sleep_stages" => {
            let parent=format!("(SELECT COALESCE(natural_key,'!'||key_hash) FROM cloud_relay_logical_keys WHERE table_name='sleep_sessions' AND row_id=CAST({alias}.session_id AS TEXT))");
            sql_tuple(&[parent, col("start_time"), col("end_time"), col("stage")])
        }
        _ => return Err("relay_identity_invalid_table".into()),
    })
}
fn capture(table: &str, alias: &str, selection: &str) -> Result<String, String> {
    Ok(format!("INSERT INTO cloud_relay_logical_keys(table_name,row_id,natural_key,key_hash) SELECT '{table}',CAST({alias}.id AS TEXT),{},NULL {selection} ON CONFLICT(table_name,row_id) DO UPDATE SET natural_key=excluded.natural_key,key_hash=NULL;",expression(table,alias)?))
}
pub(super) fn initialize(conn: &Connection) -> Result<(), String> {
    let fresh: bool = sql(conn.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM sqlite_master WHERE name='cloud_relay_logical_keys')",
        [],
        |r| r.get(0),
    ))?;
    sql(conn.execute_batch("CREATE TABLE IF NOT EXISTS cloud_relay_logical_keys(table_name TEXT NOT NULL,row_id TEXT NOT NULL,natural_key TEXT,key_hash TEXT,PRIMARY KEY(table_name,row_id));
        CREATE INDEX IF NOT EXISTS cloud_relay_logical_hash ON cloud_relay_logical_keys(table_name,key_hash);
        CREATE TABLE IF NOT EXISTS cloud_relay_logical_tombs(table_name TEXT NOT NULL,key_hash TEXT NOT NULL,deleted_at TEXT NOT NULL,PRIMARY KEY(table_name,key_hash));
        CREATE TABLE IF NOT EXISTS cloud_relay_unresolved_tombs(table_name TEXT NOT NULL,remote_id TEXT NOT NULL,deleted_at TEXT NOT NULL,first_seq INTEGER NOT NULL,PRIMARY KEY(table_name,remote_id));"))?;
    // Parent precedes child: its retained registry survives SQL cascades.
    for table in [
        "health_log",
        "sleep_sessions",
        "sleep_stages",
        "heart_rate_samples",
    ] {
        if fresh {
            sql(conn.execute_batch(&capture(table, "r", &format!("FROM {table} r WHERE 1"))?))?;
        }
        for (action, alias) in [("INSERT", "NEW"), ("UPDATE", "NEW"), ("DELETE", "OLD")] {
            let mut body = capture(table, alias, "WHERE 1")?;
            if table == "sleep_sessions" && action == "UPDATE" {
                // A parent's identity edit changes only the live children's snapshot.
                body.push_str(&capture(
                    "sleep_stages",
                    "s",
                    "FROM sleep_stages s WHERE s.session_id=NEW.id",
                )?);
            }
            sql(conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS relay_logical_{table}_{action} AFTER {action} ON {table} BEGIN {body} END;")))?;
        }
    }
    Ok(())
}
fn parent_snapshot(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    sql(conn.query_row("SELECT COALESCE(natural_key,'!'||key_hash) FROM cloud_relay_logical_keys WHERE table_name='sleep_sessions' AND row_id=?1",[id],|r|r.get(0)).optional()).map(Option::flatten)
}
fn tuple_for_fields(
    conn: &Connection,
    table: &str,
    f: &Map<String, Value>,
) -> Result<Option<String>, String> {
    Ok(Some(match table {
        "sleep_sessions" => {
            encoded(&[text(f, "date")?, text(f, "start_time")?, text(f, "source")?])
        }
        "heart_rate_samples" => encoded(&[text(f, "date")?, text(f, "time")?, text(f, "source")?]),
        "sleep_stages" => {
            let Some(parent) = parent_snapshot(conn, text(f, "session_id")?)? else {
                return Ok(None);
            };
            encoded(&[
                &parent,
                text(f, "start_time")?,
                text(f, "end_time")?,
                text(f, "stage")?,
            ])
        }
        "health_log" if text(f, "type")? == "steps" => encoded(&["steps", text(f, "date")?]),
        "health_log" if text(f, "type")? == "exercise" => {
            let Some(notes) = f.get("notes").and_then(Value::as_str) else {
                return Ok(None);
            };
            let start = match f.get("start_time") {
                Some(Value::String(s)) => s.as_str(),
                Some(Value::Null) => "",
                _ => return Err("relay_identity_field_missing".into()),
            };
            encoded(&["exercise", text(f, "date")?, start, notes])
        }
        "health_log" => return Ok(None),
        _ => return Err("relay_identity_invalid_table".into()),
    }))
}
pub(super) fn remember_absent(
    conn: &Connection,
    table: &str,
    id: &str,
    fields: &Map<String, Value>,
) -> Result<(), String> {
    if !exists(conn, table, id)? {
        if let Some(raw) = tuple_for_fields(conn, table, fields)? {
            sql(conn.execute(
                "INSERT OR IGNORE INTO cloud_relay_logical_keys VALUES(?1,?2,?3,?4)",
                params![table, id, raw, key_hash(table, &raw)?],
            ))?;
        }
    }
    Ok(())
}
pub(super) fn exported(
    conn: &Connection,
    table: &str,
    id: &Value,
) -> Result<Option<TombIdentity>, String> {
    check_table(table)?;
    let value: Option<(Option<String>,Option<String>)>=sql(conn.query_row("SELECT natural_key,key_hash FROM cloud_relay_logical_keys WHERE table_name=?1 AND row_id=?2",params![table,row_id(id)?],|r|Ok((r.get(0)?,r.get(1)?))).optional())?;
    let Some((raw, cached)) = value else {
        return Ok(None);
    };
    let hash = match (cached, raw) {
        (Some(hash), _) => hash,
        (None, Some(raw)) => key_hash(table, &raw)?,
        _ => return Ok(None),
    };
    let result = TombIdentity {
        v: 1,
        natural_key_sha256: hash,
    };
    result.validate()?;
    Ok(Some(result))
}
pub(super) fn matching(
    conn: &Connection,
    table: &str,
    identity: &TombIdentity,
) -> Result<Option<String>, String> {
    identity.validate()?;
    check_table(table)?;
    // Hash new tuples once. No SQL extension is required on other DB connections.
    let dirty: Vec<(String, String)> = {
        let mut stmt=sql(conn.prepare("SELECT row_id,natural_key FROM cloud_relay_logical_keys WHERE table_name=?1 AND key_hash IS NULL AND natural_key IS NOT NULL"))?;
        let rows = sql(stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?))))?;
        sql(rows.collect())?
    };
    for (id, raw) in dirty {
        sql(conn.execute(
            "UPDATE cloud_relay_logical_keys SET key_hash=?1 WHERE table_name=?2 AND row_id=?3",
            params![key_hash(table, &raw)?, table, id],
        ))?;
    }
    let mut stmt=sql(conn.prepare(&format!("SELECT k.row_id FROM cloud_relay_logical_keys k JOIN {table} r ON CAST(r.id AS TEXT)=k.row_id WHERE k.table_name=?1 AND k.key_hash=?2 LIMIT 2")))?;
    let rows = sql(
        stmt.query_map(params![table, identity.natural_key_sha256], |r| {
            r.get::<_, String>(0)
        }),
    )?;
    let ids: Vec<String> = sql(rows.collect())?;
    if ids.len() > 1 {
        return Err("relay_identity_ambiguous".into());
    }
    Ok(ids.into_iter().next())
}
pub(super) fn retain_delete(
    conn: &Connection,
    table: &str,
    remote: &str,
    identity: &TombIdentity,
    deleted: &str,
) -> Result<(), String> {
    identity.validate()?;
    sql(conn.execute("INSERT INTO cloud_relay_logical_tombs VALUES(?1,?2,?3) ON CONFLICT(table_name,key_hash) DO UPDATE SET deleted_at=MAX(deleted_at,excluded.deleted_at)",params![table,identity.natural_key_sha256,deleted]))?;
    // Enough information for a later orphan stage even if this parent row never arrived.
    sql(conn.execute(
        "INSERT OR IGNORE INTO cloud_relay_logical_keys VALUES(?1,?2,NULL,?3)",
        params![table, remote, identity.natural_key_sha256],
    ))?;
    sql(conn.execute(
        "DELETE FROM cloud_relay_unresolved_tombs WHERE table_name=?1 AND remote_id=?2",
        params![table, remote],
    ))?;
    Ok(())
}
pub(super) fn known_delete(
    conn: &Connection,
    table: &str,
    fields: &Map<String, Value>,
) -> Result<Option<String>, String> {
    let Some(raw) = tuple_for_fields(conn, table, fields)? else {
        return Ok(None);
    };
    sql(conn
        .query_row(
            "SELECT deleted_at FROM cloud_relay_logical_tombs WHERE table_name=?1 AND key_hash=?2",
            params![table, key_hash(table, &raw)?],
            |r| r.get(0),
        )
        .optional())
}
pub(super) fn unresolved(
    conn: &Connection,
    table: &str,
    remote: &str,
    deleted: &str,
    seq: i64,
) -> Result<(), String> {
    sql(conn.execute("INSERT INTO cloud_relay_unresolved_tombs VALUES(?1,?2,?3,?4) ON CONFLICT(table_name,remote_id) DO UPDATE SET deleted_at=MAX(deleted_at,excluded.deleted_at),first_seq=MIN(first_seq,excluded.first_seq)",params![table,remote,deleted,seq]))?;
    Ok(())
}
pub(super) fn resolved(conn: &Connection, table: &str, remote: &str) -> Result<(), String> {
    sql(conn.execute(
        "DELETE FROM cloud_relay_unresolved_tombs WHERE table_name=?1 AND remote_id=?2",
        params![table, remote],
    ))?;
    Ok(())
}
