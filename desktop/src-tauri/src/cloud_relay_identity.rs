//! Translate device-local health identities using existing UNIQUE constraints.
//! Must run inside the caller's IMMEDIATE receive transaction. No PK rewrite.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};
#[path = "cloud_relay_tomb_identity.rs"]
mod logical;
pub(crate) use logical::TombIdentity;
#[path = "cloud_relay_identity_checkpoint.rs"]
mod checkpoint;
pub(crate) use checkpoint::{
    export_checkpoint, import_checkpoint_after_rows, import_checkpoint_before_rows,
};

pub(crate) fn tomb_identity(
    conn: &Connection,
    table: &str,
    id: &Value,
) -> Result<Option<TombIdentity>, String> {
    let local = resolve(conn, table, id)?;
    let identity = logical::exported(conn, table, &local)?;
    // The sender skips its own echoed batches. Its local DELETE must therefore
    // establish the logical tomb before ACK, not wait for a foreign echo.
    if let Some(identity) = identity.as_ref() {
        if let Some(deleted) = tomb_timestamp(conn, table, row_id(&local)?)? {
            logical::retain_delete(conn, table, row_id(&local)?, identity, &deleted)?;
        }
    }
    Ok(identity)
}
pub(crate) fn unresolved_tomb_count(conn: &Connection) -> Result<i64, String> {
    sql(conn.query_row(
        "SELECT COUNT(*) FROM cloud_relay_unresolved_tombs",
        [],
        |r| r.get(0),
    ))
}
pub(crate) fn unresolved_tomb_floor(conn: &Connection) -> Result<Option<i64>, String> {
    sql(conn.query_row(
        "SELECT MIN(first_seq)-1 FROM cloud_relay_unresolved_tombs WHERE first_seq>0",
        [],
        |r| r.get(0),
    ))
}

fn sql<T>(result: rusqlite::Result<T>) -> Result<T, String> {
    result.map_err(|_| "relay_identity_database_failed".into())
}
fn text<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "relay_identity_field_missing".into())
}
fn row_id(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "relay_identity_invalid_id".into())
}
fn check_table(table: &str) -> Result<(), String> {
    if crate::cloud_relay::TABLES.contains(&table) {
        Ok(())
    } else {
        Err("relay_identity_invalid_table".into())
    }
}
fn timestamp(raw: &str) -> Result<String, String> {
    crate::sync_owner::canonical_sync_timestamp(raw, "relay")
        .map_err(|_| "relay_identity_invalid_timestamp".into())
}
fn tomb_timestamp(conn: &Connection, table: &str, id: &str) -> Result<Option<String>, String> {
    sql(conn
        .query_row(
            "SELECT deleted_at FROM sync_tombstones WHERE table_name=?1 AND row_id=?2",
            params![table, id],
            |row| row.get::<_, String>(0),
        )
        .optional())?
    .map(|raw| timestamp(&raw))
    .transpose()
}
fn exists(conn: &Connection, table: &str, id: &str) -> Result<bool, String> {
    check_table(table)?;
    sql(conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
        [id],
        |r| r.get(0),
    ))
}

pub(crate) fn initialize(conn: &Connection) -> Result<(), String> {
    sql(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cloud_relay_aliases(
        table_name TEXT NOT NULL,remote_id TEXT NOT NULL,local_id TEXT NOT NULL,
        PRIMARY KEY(table_name,remote_id));",
    ))?;
    logical::initialize(conn)
}

pub(crate) fn resolve(conn: &Connection, table: &str, id: &Value) -> Result<Value, String> {
    check_table(table)?;
    let remote = row_id(id)?;
    let local: Option<String> = sql(conn
        .query_row(
            "SELECT local_id FROM cloud_relay_aliases
        WHERE table_name=?1 AND remote_id=?2",
            params![table, remote],
            |r| r.get(0),
        )
        .optional())?;
    if local.as_deref().is_some_and(|local| local != remote) && exists(conn, table, remote)? {
        return Err("relay_identity_ambiguous".into());
    }
    Ok(Value::String(local.unwrap_or_else(|| remote.to_owned())))
}

fn natural_id(
    conn: &Connection,
    table: &str,
    fields: &Map<String, Value>,
) -> Result<Option<String>, String> {
    let (statement, args): (&str, Vec<&str>) = match table {
        "sleep_sessions" => ("SELECT id FROM sleep_sessions WHERE date=?1 AND start_time=?2 AND source=?3 LIMIT 2",
            vec![text(fields,"date")?, text(fields,"start_time")?, text(fields,"source")?]),
        "sleep_stages" => ("SELECT id FROM sleep_stages WHERE session_id=?1 AND start_time=?2 AND end_time=?3 AND stage=?4 LIMIT 2",
            vec![text(fields,"session_id")?,text(fields,"start_time")?,text(fields,"end_time")?,text(fields,"stage")?]),
        "heart_rate_samples" => ("SELECT id FROM heart_rate_samples WHERE date=?1 AND time=?2 AND source=?3 LIMIT 2",
            vec![text(fields,"date")?,text(fields,"time")?,text(fields,"source")?]),
        "health_log" if text(fields,"type")? == "steps" => (
            "SELECT id FROM health_log WHERE type='steps' AND date=?1 LIMIT 2", vec![text(fields,"date")?]),
        "health_log" if text(fields,"type")? == "exercise" => {
            // These are exactly the partial UNIQUE expression's NULL semantics.
            let start = match fields.get("start_time") {
                Some(Value::String(value)) => value.as_str(), Some(Value::Null) => "",
                _ => return Err("relay_identity_field_missing".into()),
            };
            let Some(notes) = fields.get("notes") else { return Err("relay_identity_field_missing".into()); };
            // SQLite UNIQUE permits multiple NULL notes, so NULL is not identity.
            if notes.is_null() { return Ok(None); }
            let notes = notes.as_str().ok_or("relay_identity_field_missing")?;
            ("SELECT id FROM health_log WHERE type='exercise' AND date=?1 AND COALESCE(start_time,'')=?2 AND notes=?3 LIMIT 2",
                vec![text(fields,"date")?,start,notes])
        }
        "health_log" => return Ok(None),
        _ => return Err("relay_identity_invalid_table".into()),
    };
    let mut statement = sql(conn.prepare(statement))?;
    let rows = sql(
        statement.query_map(rusqlite::params_from_iter(args), |row| {
            row.get::<_, String>(0)
        }),
    )?;
    let ids: Vec<String> = sql(rows.collect())?;
    if ids.len() > 1 {
        return Err("relay_identity_ambiguous".into());
    }
    Ok(ids.into_iter().next())
}

/// Save an alias even if LWW later keeps a newer local row. The alias and any
/// forwarded tombstone roll back if any other row in the page cannot apply.
pub(crate) fn translate_row(
    conn: &Connection,
    table: &str,
    fields: &Map<String, Value>,
) -> Result<Option<Map<String, Value>>, String> {
    check_table(table)?;
    let original = text(fields, "id")?.to_owned();
    let mapped = resolve(conn, table, &Value::String(original.clone()))?;
    let mapped = row_id(&mapped)?.to_owned();
    let mut translated = fields.clone();
    if table == "sleep_stages" {
        let parent = text(fields, "session_id")?;
        let local = resolve(conn, "sleep_sessions", &Value::String(parent.to_owned()))?;
        translated.insert("session_id".into(), local);
    }
    let natural = natural_id(conn, table, &translated)?;
    let direct_exists = exists(conn, table, &original)?;
    let mapped_exists = exists(conn, table, &mapped)?;
    if original != mapped && direct_exists {
        // An alias must never redirect a newly reused ID of an existing row.
        return Err("relay_identity_ambiguous".into());
    }
    let local = match natural {
        Some(natural) if natural != mapped && (mapped_exists || mapped != original) => {
            return Err("relay_identity_ambiguous".into());
        }
        Some(natural) => natural,
        None => mapped,
    };
    if local != original {
        sql(conn.execute(
            "INSERT INTO cloud_relay_aliases(table_name,remote_id,local_id) VALUES(?1,?2,?3)
            ON CONFLICT(table_name,remote_id) DO NOTHING",
            params![table, original, local],
        ))?;
        if resolve(conn, table, &Value::String(original.clone()))? != Value::String(local.clone()) {
            return Err("relay_identity_ambiguous".into());
        }
        // A tombstone may have arrived before enough row data existed to find
        // the alias. Apply it to the local identity now, before the row's LWW.
        if let Some(deleted) = tomb_timestamp(conn, table, &original)? {
            apply_tombstone(conn, table, &Value::String(local.clone()), &deleted)?;
        }
    }
    translated.insert("id".into(), Value::String(local.clone()));
    logical::remember_absent(conn, table, &local, &translated)?;
    if let Some(deleted) = logical::known_delete(conn, table, &translated)? {
        apply_tombstone(conn, table, &Value::String(local.clone()), &deleted)?;
    }
    // Now the original id has enough row data to resolve any earlier legacy tomb.
    if let Some(deleted) = tomb_timestamp(conn, table, &original)? {
        apply_tombstone(conn, table, &Value::String(local.clone()), &deleted)?;
    }
    logical::resolved(conn, table, &original)?;
    if table == "sleep_stages" {
        let parent = text(&translated, "session_id")?;
        if !exists(conn, "sleep_sessions", parent)? {
            if let Some(deleted) = tomb_timestamp(conn, "sleep_sessions", parent)? {
                let updated = timestamp(text(fields, "_updated_at")?)?;
                // A deleted parent cannot have a live child, even if an offline
                // peer edited that child later. Propagate the derived deletion;
                // otherwise its healing tomb may be behind this blocked cursor.
                let before = tomb_timestamp(conn, table, &local)?;
                let effective = deleted.max(updated).max(before.clone().unwrap_or_default());
                apply_tombstone(conn, table, &Value::String(local.clone()), &effective)?;
                if before.as_deref() != Some(effective.as_str()) {
                    sql(conn.execute("INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id) VALUES('sleep_stages',?1)",[&local]))?;
                }
                return Ok(None);
            }
            return Err("relay_identity_parent_missing".into());
        }
    }
    Ok(Some(translated))
}

/// Source seq is the actual received batch seq, used only to keep ambiguous
/// historical deletions below the applied-receipt floor while fetching continues.
pub(crate) fn apply_tombstone_with_identity(
    conn: &Connection,
    table: &str,
    id: &Value,
    deleted: &str,
    provided: Option<&TombIdentity>,
    source_seq: i64,
) -> Result<bool, String> {
    check_table(table)?;
    let remote = row_id(id)?;
    let deleted = timestamp(deleted)?;
    let mut local = resolve(conn, table, id)?;
    let identity = provided
        .cloned()
        .or(logical::exported(conn, table, &local)?);
    if let Some(identity) = identity {
        if row_id(&local)? == remote && !exists(conn, table, remote)? {
            if let Some(found) = logical::matching(conn, table, &identity)? {
                sql(conn.execute("INSERT INTO cloud_relay_aliases VALUES(?1,?2,?3) ON CONFLICT(table_name,remote_id) DO NOTHING",params![table,remote,found]))?;
                local = resolve(conn, table, id)?;
                if row_id(&local)? != found {
                    return Err("relay_identity_ambiguous".into());
                }
            }
        }
        logical::retain_delete(conn, table, remote, &identity, &deleted)?;
    } else if source_seq > 0 && row_id(&local)? == remote && !exists(conn, table, remote)? {
        // No tuple exists for a deletion predating registry installation. Do not
        // guess another device's local id or report this seq as fully applied.
        logical::unresolved(conn, table, remote, &deleted, source_seq)?;
    }
    apply_tombstone(conn, table, &local, &deleted)
}

/// Preserve SQL cascade deletions as tombstones. A pre-existing dirty child
/// must not become an unresolvable journal key after its parent disappears.
pub(crate) fn apply_tombstone(
    conn: &Connection,
    table: &str,
    id: &Value,
    deleted: &str,
) -> Result<bool, String> {
    check_table(table)?;
    let local = resolve(conn, table, id)?;
    let local_id = row_id(&local)?;
    let deleted = timestamp(deleted)?;
    if let Some(identity) = logical::exported(conn, table, &local)? {
        logical::retain_delete(conn, table, local_id, &identity, &deleted)?;
    }
    let children: Vec<(String, String, Option<i64>)> = if table == "sleep_sessions" {
        let mut query = sql(conn.prepare("SELECT s.id,s.updated_at,d.seq FROM sleep_stages s
            LEFT JOIN cloud_relay_dirty d ON d.table_name='sleep_stages' AND d.row_id=s.id WHERE s.session_id=?1"))?;
        let rows = sql(query.query_map([local_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))?;
        sql(rows.collect())?
    } else {
        vec![]
    };
    let changed = crate::sync_owner::apply_tombstone_lww(conn, table, &local, &deleted)
        .map_err(|_| "relay_identity_delete_failed")?;
    if table == "sleep_sessions" && !exists(conn, table, local_id)? {
        let effective =
            tomb_timestamp(conn, table, local_id)?.ok_or("relay_identity_tomb_missing")?;
        for (child, updated, dirty_seq) in children {
            if exists(conn, "sleep_stages", &child)? {
                return Err("relay_identity_cascade_failed".into());
            }
            // SQL already deleted the child with its parent. Represent that
            // deletion at least at the last known child version, never older.
            let child_deleted = effective
                .clone()
                .max(timestamp(&updated)?)
                .max(tomb_timestamp(conn, "sleep_stages", &child)?.unwrap_or_default());
            crate::sync_owner::apply_tombstone_lww(
                conn,
                "sleep_stages",
                &Value::String(child.clone()),
                &child_deleted,
            )
            .map_err(|_| "relay_identity_delete_failed")?;
            if let Some(seq) = dirty_seq {
                // Exact captured sequence only. Immutable outbox is untouched;
                // writes after this transaction will receive a different seq.
                sql(conn.execute("DELETE FROM cloud_relay_dirty WHERE seq=?1 AND table_name='sleep_stages' AND row_id=?2",
                    params![seq,child]))?;
            }
            // This is a newly derived cascade tomb, not an echo of a received
            // child tomb. Keep it deliverable to peers which lacked the child
            // when they received the parent deletion. At most one key survives.
            sql(conn.execute("INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id) VALUES('sleep_stages',?1)",
                [&child]))?;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const OLD: &str = "2026-01-01T01:00:00.000000000Z";
    const NEW: &str = "2026-01-01T02:00:00.000000000Z";
    const DELETED: &str = "2026-01-01T03:00:00.000000000Z";

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            INSERT INTO app_settings VALUES('device_id','synthetic-local');
            CREATE TABLE sync_hlc_state(singleton INTEGER PRIMARY KEY,last_millis INTEGER NOT NULL);
            INSERT INTO sync_hlc_state VALUES(1,0);
            CREATE TABLE sync_apply_context(singleton INTEGER PRIMARY KEY,remote_apply INTEGER NOT NULL,stamp_depth INTEGER NOT NULL);
            INSERT INTO sync_apply_context VALUES(1,0,0);
            CREATE TABLE sync_row_versions(table_name TEXT,row_id TEXT,updated_at TEXT,device_id TEXT,PRIMARY KEY(table_name,row_id));
            CREATE TABLE sync_tombstones(table_name TEXT,row_id TEXT,deleted_at TEXT,UNIQUE(table_name,row_id));
            CREATE TABLE cloud_relay_dirty(seq INTEGER PRIMARY KEY AUTOINCREMENT,table_name TEXT,row_id TEXT,UNIQUE(table_name,row_id));
            CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT NOT NULL,start_time TEXT NOT NULL,end_time TEXT NOT NULL,
                duration_minutes INTEGER NOT NULL,source TEXT NOT NULL,updated_at TEXT NOT NULL,UNIQUE(date,start_time,source));
            CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sleep_sessions(id) ON DELETE CASCADE,
                start_time TEXT NOT NULL,end_time TEXT NOT NULL,stage TEXT NOT NULL,updated_at TEXT NOT NULL,
                UNIQUE(session_id,start_time,end_time,stage));
            CREATE TABLE health_log(id TEXT PRIMARY KEY,date TEXT NOT NULL,type TEXT NOT NULL,start_time TEXT,notes TEXT,updated_at TEXT NOT NULL);
            CREATE UNIQUE INDEX steps_natural ON health_log(date) WHERE type='steps';
            CREATE UNIQUE INDEX exercise_natural ON health_log(date,COALESCE(start_time,''),notes) WHERE type='exercise';
            CREATE TABLE heart_rate_samples(id TEXT PRIMARY KEY,date TEXT NOT NULL,time TEXT NOT NULL,source TEXT NOT NULL,
                bpm INTEGER NOT NULL,updated_at TEXT NOT NULL,UNIQUE(date,time,source));").unwrap();
        initialize(&conn).unwrap();
        conn
    }
    fn session(id: &str, start: &str, updated: &str) -> Map<String, Value> {
        json!({"id":id,"date":"2026-01-01","start_time":start,"end_time":"08:00","duration_minutes":420,
            "source":"health_connect","updated_at":updated,"_updated_at":updated,"_device_id":"synthetic-remote"})
            .as_object().unwrap().clone()
    }
    fn stage(id: &str, parent: &str, updated: &str) -> Map<String, Value> {
        json!({"id":id,"session_id":parent,"start_time":"01:00","end_time":"02:00","stage":"deep",
            "updated_at":updated,"_updated_at":updated,"_device_id":"synthetic-remote"})
        .as_object()
        .unwrap()
        .clone()
    }
    fn upsert(conn: &Connection, table: &str, fields: &Map<String, Value>) -> bool {
        crate::sync_owner::upsert_row_fail_closed(conn, table, fields).unwrap()
    }
    fn seed_sleep(conn: &Connection) {
        assert!(upsert(
            conn,
            "sleep_sessions",
            &session("local-session", "01:00", NEW)
        ));
        assert!(upsert(
            conn,
            "sleep_stages",
            &stage("local-stage", "local-session", NEW)
        ));
    }

    #[test]
    fn different_sleep_ids_alias_even_when_lww_keeps_local_and_parent_translates() {
        let mut conn = fixture();
        seed_sleep(&conn);
        let tx = conn.transaction().unwrap();
        let translated = translate_row(
            &tx,
            "sleep_sessions",
            &session("remote-session", "01:00", OLD),
        )
        .unwrap()
        .unwrap();
        assert_eq!(translated["id"], json!("local-session"));
        assert!(!upsert(&tx, "sleep_sessions", &translated));
        let translated = translate_row(
            &tx,
            "sleep_stages",
            &stage("remote-stage", "remote-session", OLD),
        )
        .unwrap()
        .unwrap();
        assert_eq!(translated["id"], json!("local-stage"));
        assert_eq!(translated["session_id"], json!("local-session"));
        assert!(!upsert(&tx, "sleep_stages", &translated));
        tx.commit().unwrap();
        assert_eq!(
            resolve(&conn, "sleep_stages", &json!("remote-stage")).unwrap(),
            json!("local-stage")
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM sleep_sessions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn all_existing_health_unique_keys_map_without_new_dedup_rules() {
        let conn = fixture();
        conn.execute(
            "INSERT INTO health_log VALUES('local-step','2026-01-01','steps',NULL,'',?1)",
            [NEW],
        )
        .unwrap();
        conn.execute("INSERT INTO health_log VALUES('local-exercise','2026-01-01','exercise',NULL,'walking',?1)",[NEW]).unwrap();
        conn.execute("INSERT INTO heart_rate_samples VALUES('local-heart','2026-01-01','12:00','health_connect',70,?1)",[NEW]).unwrap();
        for (table, incoming, expected) in [
            (
                "health_log",
                json!({"id":"remote-step","date":"2026-01-01","type":"steps"}),
                "local-step",
            ),
            (
                "health_log",
                json!({"id":"remote-exercise","date":"2026-01-01","type":"exercise","start_time":"","notes":"walking"}),
                "local-exercise",
            ),
            (
                "heart_rate_samples",
                json!({"id":"remote-heart","date":"2026-01-01","time":"12:00","source":"health_connect"}),
                "local-heart",
            ),
        ] {
            let translated = translate_row(&conn, table, incoming.as_object().unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(translated["id"], json!(expected));
        }
        // Multiple NULL notes are allowed by the real UNIQUE expression.
        let incoming = json!({"id":"independent-null","date":"2026-01-01","type":"exercise","start_time":null,"notes":null});
        assert_eq!(
            translate_row(&conn, "health_log", incoming.as_object().unwrap())
                .unwrap()
                .unwrap()["id"],
            json!("independent-null")
        );
    }

    #[test]
    fn conflicting_primary_and_natural_id_fails_without_rewriting_user_rows() {
        let conn = fixture();
        upsert(&conn, "sleep_sessions", &session("one", "01:00", NEW));
        upsert(&conn, "sleep_sessions", &session("two", "02:00", NEW));
        assert!(translate_row(&conn, "sleep_sessions", &session("one", "02:00", DELETED)).is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM sleep_sessions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM cloud_relay_aliases", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn tomb_before_alias_prevents_resurrection_and_alias_changes_roll_back() {
        let mut conn = fixture();
        seed_sleep(&conn);
        apply_tombstone(&conn, "sleep_sessions", &json!("remote-session"), DELETED).unwrap();
        {
            let tx = conn.transaction().unwrap();
            let translated = translate_row(
                &tx,
                "sleep_sessions",
                &session("remote-session", "01:00", OLD),
            )
            .unwrap()
            .unwrap();
            assert!(!upsert(&tx, "sleep_sessions", &translated));
            assert!(!exists(&tx, "sleep_sessions", "local-session").unwrap());
            // Simulate later failure in the same received page.
        }
        assert!(exists(&conn, "sleep_sessions", "local-session").unwrap());
        assert!(exists(&conn, "sleep_stages", "local-stage").unwrap());
        assert_eq!(
            resolve(&conn, "sleep_sessions", &json!("remote-session")).unwrap(),
            json!("remote-session")
        );
        let tx = conn.transaction().unwrap();
        let translated = translate_row(
            &tx,
            "sleep_sessions",
            &session("remote-session", "01:00", OLD),
        )
        .unwrap()
        .unwrap();
        assert!(!upsert(&tx, "sleep_sessions", &translated));
        tx.commit().unwrap();
        assert!(!exists(&conn, "sleep_sessions", "local-session").unwrap());
        assert!(tomb_timestamp(&conn, "sleep_stages", "local-stage")
            .unwrap()
            .is_some());
    }

    #[test]
    fn cascading_delete_materializes_child_tomb_and_replaces_only_captured_dirty_revision() {
        let mut conn = fixture();
        seed_sleep(&conn);
        conn.execute(
            "INSERT INTO cloud_relay_dirty(table_name,row_id) VALUES('sleep_stages','local-stage')",
            [],
        )
        .unwrap();
        let old_seq = conn.last_insert_rowid();
        let tx = conn.transaction().unwrap();
        assert!(apply_tombstone(&tx, "sleep_sessions", &json!("local-session"), DELETED).unwrap());
        assert!(!exists(&tx, "sleep_stages", "local-stage").unwrap());
        assert_eq!(
            tomb_timestamp(&tx, "sleep_stages", "local-stage")
                .unwrap()
                .as_deref(),
            Some(DELETED)
        );
        let derived_seq: i64 = tx
            .query_row(
                "SELECT seq FROM cloud_relay_dirty WHERE row_id='local-stage'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(derived_seq > old_seq);
        tx.commit().unwrap();
        conn.execute("INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id) VALUES('sleep_stages','local-stage')",[]).unwrap();
        assert!(conn.last_insert_rowid() > derived_seq);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM cloud_relay_dirty", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn newer_orphan_stage_becomes_tomb_and_does_not_block_later_batches() {
        let conn = fixture();
        seed_sleep(&conn);
        apply_tombstone(&conn, "sleep_sessions", &json!("local-session"), DELETED).unwrap();
        let later = "2026-01-01T04:00:00.000000000Z";
        let incoming = stage("remote-offline-stage", "local-session", later);
        assert!(translate_row(&conn, "sleep_stages", &incoming)
            .unwrap()
            .is_none());
        assert_eq!(
            tomb_timestamp(&conn, "sleep_stages", "remote-offline-stage")
                .unwrap()
                .as_deref(),
            Some(later)
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM cloud_relay_dirty WHERE row_id='remote-offline-stage'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let revision: i64 = conn
            .query_row(
                "SELECT seq FROM cloud_relay_dirty WHERE row_id='remote-offline-stage'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(translate_row(&conn, "sleep_stages", &incoming)
            .unwrap()
            .is_none());
        assert_eq!(
            conn.query_row(
                "SELECT seq FROM cloud_relay_dirty WHERE row_id='remote-offline-stage'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            revision
        );
        assert!(translate_row(
            &conn,
            "sleep_stages",
            &stage("unrelated", "unknown-parent", later)
        )
        .is_err());
    }

    #[test]
    fn delete_before_first_row_exchange_matches_natural_identity_without_resurrection() {
        let a = fixture();
        let b = fixture();
        assert!(upsert(
            &a,
            "sleep_sessions",
            &session("a-session", "01:00", OLD)
        ));
        assert!(upsert(
            &b,
            "sleep_sessions",
            &session("b-session", "01:00", OLD)
        ));
        assert!(upsert(
            &b,
            "sleep_stages",
            &stage("b-stage", "b-session", OLD)
        ));
        // Local SQL deletion retains OLD's identity before any relay packet existed.
        a.execute("DELETE FROM sleep_sessions WHERE id='a-session'", [])
            .unwrap();
        a.execute(
            "INSERT INTO sync_tombstones VALUES('sleep_sessions','a-session',?1)",
            [DELETED],
        )
        .unwrap();
        let identity = tomb_identity(&a, "sleep_sessions", &json!("a-session"))
            .unwrap()
            .unwrap();
        apply_tombstone_with_identity(
            &b,
            "sleep_sessions",
            &json!("a-session"),
            DELETED,
            Some(&identity),
            1,
        )
        .unwrap();
        assert!(!exists(&b, "sleep_sessions", "b-session").unwrap());
        assert!(!exists(&b, "sleep_stages", "b-stage").unwrap());
        assert_eq!(unresolved_tomb_count(&b).unwrap(), 0);
        let old_peer_row = translate_row(&a, "sleep_sessions", &session("b-session", "01:00", OLD))
            .unwrap()
            .unwrap();
        assert!(!upsert(&a, "sleep_sessions", &old_peer_row));
        // The logical tomb also protects a previously unseen device-local id.
        let incoming = session("third-session", "01:00", OLD);
        let translated = translate_row(&b, "sleep_sessions", &incoming)
            .unwrap()
            .unwrap();
        assert!(!upsert(&b, "sleep_sessions", &translated));
    }

    #[test]
    fn stage_identity_uses_parent_natural_key_and_survives_sql_cascade() {
        let a = fixture();
        let b = fixture();
        for (conn, parent, child) in [(&a, "a-parent", "a-child"), (&b, "b-parent", "b-child")] {
            upsert(conn, "sleep_sessions", &session(parent, "01:00", OLD));
            upsert(conn, "sleep_stages", &stage(child, parent, OLD));
        }
        let before = tomb_identity(&a, "sleep_stages", &json!("a-child"))
            .unwrap()
            .unwrap();
        a.execute("DELETE FROM sleep_sessions WHERE id='a-parent'", [])
            .unwrap();
        let after = tomb_identity(&a, "sleep_stages", &json!("a-child"))
            .unwrap()
            .unwrap();
        let peer = tomb_identity(&b, "sleep_stages", &json!("b-child"))
            .unwrap()
            .unwrap();
        assert_eq!(before.natural_key_sha256, after.natural_key_sha256);
        assert_eq!(after.natural_key_sha256, peer.natural_key_sha256);
        apply_tombstone_with_identity(
            &b,
            "sleep_stages",
            &json!("a-child"),
            DELETED,
            Some(&after),
            2,
        )
        .unwrap();
        assert!(!exists(&b, "sleep_stages", "b-child").unwrap());
        assert!(exists(&b, "sleep_sessions", "b-parent").unwrap());
    }

    #[test]
    fn legacy_unknown_tomb_remains_explicit_until_row_data_resolves_it() {
        let conn = fixture();
        seed_sleep(&conn);
        apply_tombstone_with_identity(
            &conn,
            "sleep_sessions",
            &json!("legacy-deleted"),
            DELETED,
            None,
            7,
        )
        .unwrap();
        assert!(exists(&conn, "sleep_sessions", "local-session").unwrap());
        assert_eq!(unresolved_tomb_count(&conn).unwrap(), 1);
        assert_eq!(unresolved_tomb_floor(&conn).unwrap(), Some(6));
        let row = session("legacy-deleted", "01:00", OLD);
        let translated = translate_row(&conn, "sleep_sessions", &row)
            .unwrap()
            .unwrap();
        assert!(!upsert(&conn, "sleep_sessions", &translated));
        assert_eq!(unresolved_tomb_count(&conn).unwrap(), 0);
        assert_eq!(unresolved_tomb_floor(&conn).unwrap(), None);
    }
}
