//! Identity metadata for encrypted checkpoints; merge, never replace recipient state.
//! Caller owns one IMMEDIATE transaction spanning before/rows/tombs/after/cursor.
use super::{
    apply_tombstone, apply_tombstone_with_identity, exists, logical, resolve, row_id, sql,
    timestamp, tomb_timestamp, TombIdentity,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const TABLES: &[&str] = &[
    "health_log",
    "sleep_sessions",
    "sleep_stages",
    "heart_rate_samples",
];
const INVALID: &str = "relay_checkpoint_identity_invalid";
const AMBIGUOUS: &str = "relay_checkpoint_identity_ambiguous";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    v: u8,
    aliases: Vec<Alias>,
    logical_keys: Vec<Key>,
    logical_tombs: Vec<LogicalTomb>,
    unresolved_tombs: Vec<Unresolved>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Alias {
    table_name: String,
    remote_id: String,
    local_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    table_name: String,
    row_id: String,
    natural_key: Option<String>,
    key_hash: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogicalTomb {
    table_name: String,
    key_hash: String,
    deleted_at: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Unresolved {
    table_name: String,
    remote_id: String,
    deleted_at: String,
    first_seq: i64,
}

fn table(raw: &str) -> Result<(), String> {
    if TABLES.contains(&raw) {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}
fn id(raw: &str) -> Result<(), String> {
    if !raw.is_empty() && raw.len() <= 8192 && !raw.contains('\0') {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}
fn hash(raw: &str) -> Result<(), String> {
    if raw.len() == 64
        && raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}
fn tuple(mut raw: &str) -> Result<Vec<&str>, String> {
    let mut result = Vec::new();
    while !raw.is_empty() {
        let (size, tail) = raw.split_once(':').ok_or(INVALID)?;
        let count: usize = size.parse().map_err(|_| INVALID)?;
        if count.to_string() != size || count > tail.len() || !tail.is_char_boundary(count) {
            return Err(INVALID.into());
        }
        result.push(&tail[..count]);
        raw = &tail[count..];
        if result.len() > 4 {
            return Err(INVALID.into());
        }
    }
    Ok(result)
}
fn validate_tuple(table: &str, raw: &str) -> Result<(), String> {
    let parts = tuple(raw)?;
    let valid = match table {
        "sleep_sessions" | "heart_rate_samples" => {
            parts.len() == 3 && parts.iter().all(|s| !s.is_empty())
        }
        "sleep_stages" if parts.len() == 4 && parts.iter().all(|s| !s.is_empty()) => {
            if let Some(parent) = parts[0].strip_prefix('!') {
                hash(parent)?;
            } else {
                validate_tuple("sleep_sessions", parts[0])?;
            }
            true
        }
        "health_log" => {
            (parts.len() == 2 && parts[0] == "steps" && !parts[1].is_empty())
                || (parts.len() == 4 && parts[0] == "exercise" && !parts[1].is_empty())
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}
impl Snapshot {
    fn parse(value: &Value) -> Result<Self, String> {
        let snapshot: Self = serde_json::from_value(value.clone()).map_err(|_| INVALID)?;
        if snapshot.v != 1 {
            return Err(INVALID.into());
        }
        let mut seen = BTreeSet::new();
        for key in &snapshot.logical_keys {
            table(&key.table_name)?;
            id(&key.row_id)?;
            if !seen.insert((key.table_name.as_str(), key.row_id.as_str())) {
                return Err(INVALID.into());
            }
            if let Some(hash_value) = &key.key_hash {
                hash(hash_value)?;
            }
            if let Some(raw) = &key.natural_key {
                validate_tuple(&key.table_name, raw)?;
                if key.key_hash.as_deref()
                    != Some(logical::key_hash(&key.table_name, raw)?.as_str())
                {
                    return Err(INVALID.into());
                }
            }
        }
        let key_ids = seen.clone();
        seen.clear();
        for alias in &snapshot.aliases {
            table(&alias.table_name)?;
            id(&alias.remote_id)?;
            id(&alias.local_id)?;
            if alias.remote_id == alias.local_id
                || !seen.insert((&alias.table_name, &alias.remote_id))
                || !key_ids.contains(&(alias.table_name.as_str(), alias.local_id.as_str()))
            {
                return Err(INVALID.into());
            }
        }
        // Source mappings are flattened; accepting chains would depend on input order.
        for alias in &snapshot.aliases {
            if seen.contains(&(alias.table_name.as_str(), alias.local_id.as_str())) {
                return Err(INVALID.into());
            }
        }
        seen.clear();
        for tomb in &snapshot.logical_tombs {
            table(&tomb.table_name)?;
            hash(&tomb.key_hash)?;
            timestamp(&tomb.deleted_at)?;
            if !seen.insert((&tomb.table_name, &tomb.key_hash)) {
                return Err(INVALID.into());
            }
        }
        seen.clear();
        for tomb in &snapshot.unresolved_tombs {
            table(&tomb.table_name)?;
            id(&tomb.remote_id)?;
            timestamp(&tomb.deleted_at)?;
            if tomb.first_seq <= 0 || !seen.insert((&tomb.table_name, &tomb.remote_id)) {
                return Err(INVALID.into());
            }
        }
        Ok(snapshot)
    }
}
fn transaction_required(conn: &Connection) -> Result<(), String> {
    if conn.is_autocommit() {
        Err("relay_checkpoint_transaction_required".into())
    } else {
        Ok(())
    }
}

/// Read-only, including hash computation: does not populate the source hash cache.
pub(crate) fn export_checkpoint(conn: &Connection) -> Result<Value, String> {
    transaction_required(conn)?;
    let mut query = sql(conn.prepare("SELECT table_name,remote_id,local_id FROM cloud_relay_aliases ORDER BY table_name,remote_id"))?;
    let aliases = sql(query.query_map([], |r| {
        Ok(Alias {
            table_name: r.get(0)?,
            remote_id: r.get(1)?,
            local_id: r.get(2)?,
        })
    }))?;
    let aliases: Vec<Alias> = sql(aliases.collect())?;
    let aliases: Vec<Alias> = aliases.into_iter().filter(|a| !a.remote_id.starts_with("raw-sleep:") && !a.remote_id.starts_with("raw-stage:") && !a.local_id.starts_with("raw-sleep:") && !a.local_id.starts_with("raw-stage:")).collect();
    let mut query = sql(conn.prepare("SELECT table_name,row_id,natural_key,key_hash FROM cloud_relay_logical_keys ORDER BY table_name,row_id"))?;
    let keys = sql(query.query_map([], |r| {
        Ok(Key {
            table_name: r.get(0)?,
            row_id: r.get(1)?,
            natural_key: r.get(2)?,
            key_hash: r.get(3)?,
        })
    }))?;
    let mut keys: Vec<Key> = sql(keys.collect())?;
    keys.retain(|k| !k.row_id.starts_with("raw-sleep:") && !k.row_id.starts_with("raw-stage:"));
    for key in &mut keys {
        if let Some(raw) = &key.natural_key {
            key.key_hash = Some(logical::key_hash(&key.table_name, raw)?);
        }
    }
    // Old aliases can predate registry installation; retain the opaque anchor.
    let mut known_keys: BTreeSet<(String, String)> = keys
        .iter()
        .map(|key| (key.table_name.clone(), key.row_id.clone()))
        .collect();
    for alias in &aliases {
        if known_keys.insert((alias.table_name.clone(), alias.local_id.clone())) {
            keys.push(Key {
                table_name: alias.table_name.clone(),
                row_id: alias.local_id.clone(),
                natural_key: None,
                key_hash: None,
            });
        }
    }
    keys.sort_by(|a, b| (&a.table_name, &a.row_id).cmp(&(&b.table_name, &b.row_id)));
    let mut query = sql(conn.prepare("SELECT table_name,key_hash,deleted_at FROM cloud_relay_logical_tombs ORDER BY table_name,key_hash"))?;
    let tombs = sql(query.query_map([], |r| {
        Ok(LogicalTomb {
            table_name: r.get(0)?,
            key_hash: r.get(1)?,
            deleted_at: r.get(2)?,
        })
    }))?;
    let logical_tombs = sql(tombs.collect())?;
    let mut query = sql(conn.prepare("SELECT table_name,remote_id,deleted_at,first_seq FROM cloud_relay_unresolved_tombs ORDER BY table_name,remote_id"))?;
    let unresolved = sql(query.query_map([], |r| {
        Ok(Unresolved {
            table_name: r.get(0)?,
            remote_id: r.get(1)?,
            deleted_at: r.get(2)?,
            first_seq: r.get(3)?,
        })
    }))?;
    let value = serde_json::to_value(Snapshot {
        v: 1,
        aliases,
        logical_keys: keys,
        logical_tombs,
        unresolved_tombs: sql(unresolved.collect())?,
    })
    .map_err(|_| INVALID)?;
    Snapshot::parse(&value)?;
    Ok(value)
}

fn local(conn: &Connection, table: &str, remote: &str) -> Result<String, String> {
    Ok(row_id(&resolve(conn, table, &Value::String(remote.to_owned()))?)?.to_owned())
}
fn bind(conn: &Connection, table: &str, remote: &str, target: &str) -> Result<(), String> {
    let target = local(conn, table, target)?;
    // resolve() deliberately reads one hop. Do not introduce a mapping to another
    // alias; a pre-existing ambiguous terminal must fail the caller's transaction.
    if sql(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cloud_relay_aliases WHERE table_name=?1 AND remote_id=?2)",
        params![table, target],
        |r| r.get::<_, bool>(0),
    ))? {
        return Err(AMBIGUOUS.into());
    }
    let current = local(conn, table, remote)?;
    if (current != remote && current != target)
        || (remote != target && exists(conn, table, remote)?)
    {
        return Err(AMBIGUOUS.into());
    }
    // A deleted publisher anchor A may now match the recipient's live B while
    // old aliases still point R -> A. Retarget the complete reverse closure to B
    // in this same transaction; otherwise parent translation/export sees a chain.
    // UNION terminates even for malformed cycles, which the checks below reject.
    let mut query = sql(conn.prepare(
        "WITH RECURSIVE affected(id) AS (
            SELECT ?2
            UNION
            SELECT a.remote_id FROM cloud_relay_aliases a JOIN affected f ON a.local_id=f.id
            WHERE a.table_name=?1
        ) SELECT id FROM affected ORDER BY id",
    ))?;
    let rows = sql(query.query_map(params![table, remote], |r| r.get::<_, String>(0)))?;
    let affected: Vec<String> = sql(rows.collect())?;
    for alias in &affected {
        if alias == &target {
            if remote != target {
                return Err(AMBIGUOUS.into());
            }
        } else if exists(conn, table, alias)? {
            // An alias which is also a live primary key cannot safely be moved.
            return Err(AMBIGUOUS.into());
        }
    }
    for alias in &affected {
        if alias != &target {
            sql(conn.execute(
                "INSERT INTO cloud_relay_aliases VALUES(?1,?2,?3)
                 ON CONFLICT(table_name,remote_id) DO UPDATE SET local_id=excluded.local_id
                 WHERE cloud_relay_aliases.local_id<>excluded.local_id",
                params![table, alias, target],
            ))?;
        }
    }
    // Legacy tombs may have remained under any affected foreign ID, not just A.
    // Forward all of them after flattening; LWW keeps the strongest deletion and
    // preserves newer recipient rows. Original tombs and local outbox stay intact.
    for alias in &affected {
        if let Some(deleted) = tomb_timestamp(conn, table, alias)? {
            apply_tombstone_with_identity(
                conn,
                table,
                &Value::String(alias.clone()),
                &deleted,
                None,
                0,
            )?;
        }
    }
    Ok(())
}
fn target_for_key(conn: &Connection, key: &Key) -> Result<String, String> {
    let mapped = local(conn, &key.table_name, &key.row_id)?;
    let Some(hash_value) = &key.key_hash else {
        return Ok(mapped);
    };
    let identity = TombIdentity {
        v: 1,
        natural_key_sha256: hash_value.clone(),
    };
    if let Some(found) = logical::matching(conn, &key.table_name, &identity)? {
        if found != mapped && (mapped != key.row_id || exists(conn, &key.table_name, &mapped)?) {
            return Err(AMBIGUOUS.into());
        }
        return Ok(found);
    }
    if mapped != key.row_id || exists(conn, &key.table_name, &mapped)? {
        return Ok(mapped);
    }
    // No live match: an already deleted local identity still needs stable remapping.
    let mut query = sql(conn.prepare(
        "SELECT row_id FROM cloud_relay_logical_keys WHERE table_name=?1 AND key_hash=?2",
    ))?;
    let rows = sql(query.query_map(params![key.table_name, hash_value], |r| {
        r.get::<_, String>(0)
    }))?;
    let rows: Vec<String> = sql(rows.collect())?;
    let mut targets = BTreeSet::new();
    for row in rows {
        targets.insert(local(conn, &key.table_name, &row)?);
    }
    if targets.len() > 1 {
        return Err(AMBIGUOUS.into());
    }
    Ok(targets.into_iter().next().unwrap_or(mapped))
}
fn merge_keys_and_aliases(conn: &Connection, snapshot: &Snapshot) -> Result<(), String> {
    let mut mapped = BTreeMap::new();
    // Resolve all publisher anchors against recipient state before adding any keys.
    for key in &snapshot.logical_keys {
        mapped.insert(
            (key.table_name.as_str(), key.row_id.as_str()),
            target_for_key(conn, key)?,
        );
    }
    for key in &snapshot.logical_keys {
        let target = &mapped[&(key.table_name.as_str(), key.row_id.as_str())];
        bind(conn, &key.table_name, &key.row_id, target)?;
        let prior = logical::exported(conn, &key.table_name, &Value::String(target.clone()))?;
        if key.key_hash.is_some()
            && prior.as_ref().map(|p| p.natural_key_sha256.as_str()) != key.key_hash.as_deref()
            && prior.is_some()
            && !exists(conn, &key.table_name, target)?
        {
            return Err(AMBIGUOUS.into());
        }
        // A live recipient row may be newer and have a changed natural tuple.
        // Its SQL registry is authoritative until the normal row LWW decides.
        sql(conn.execute(
            "INSERT OR IGNORE INTO cloud_relay_logical_keys VALUES(?1,?2,?3,?4)",
            params![key.table_name, target, key.natural_key, key.key_hash],
        ))?;
        if key.key_hash.is_some() {
            sql(conn.execute("UPDATE cloud_relay_logical_keys SET natural_key=?3,key_hash=?4 WHERE table_name=?1 AND row_id=?2 AND natural_key IS NULL AND key_hash IS NULL",params![key.table_name,target,key.natural_key,key.key_hash]))?;
        }
    }
    for alias in &snapshot.aliases {
        let target = local(conn, &alias.table_name, &alias.local_id)?;
        bind(conn, &alias.table_name, &alias.remote_id, &target)?;
    }
    Ok(())
}
fn merge_tomb_floors(conn: &Connection, snapshot: &Snapshot) -> Result<(), String> {
    for tomb in &snapshot.logical_tombs {
        let deleted = timestamp(&tomb.deleted_at)?;
        sql(conn.execute("INSERT INTO cloud_relay_logical_tombs VALUES(?1,?2,?3) ON CONFLICT(table_name,key_hash) DO UPDATE SET deleted_at=MAX(deleted_at,excluded.deleted_at)",params![tomb.table_name,tomb.key_hash,deleted]))?;
    }
    // Include pre-existing recipient floors, not just those in this snapshot.
    let mut query = sql(conn.prepare("SELECT table_name,key_hash,deleted_at FROM cloud_relay_logical_tombs ORDER BY table_name,key_hash"))?;
    let rows = sql(query.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    }))?;
    let tombs: Vec<(String, String, String)> = sql(rows.collect())?;
    for (table, hash_value, deleted) in tombs {
        let identity = TombIdentity {
            v: 1,
            natural_key_sha256: hash_value,
        };
        if let Some(target) = logical::matching(conn, &table, &identity)? {
            apply_tombstone(conn, &table, &Value::String(target), &timestamp(&deleted)?)?;
        }
    }
    Ok(())
}
fn merge_unresolved(conn: &Connection, snapshot: &Snapshot) -> Result<(), String> {
    for tomb in &snapshot.unresolved_tombs {
        logical::unresolved(
            conn,
            &tomb.table_name,
            &tomb.remote_id,
            &timestamp(&tomb.deleted_at)?,
            tomb.first_seq,
        )?;
    }
    let mut query = sql(conn.prepare("SELECT table_name,remote_id,deleted_at,first_seq FROM cloud_relay_unresolved_tombs ORDER BY table_name,remote_id"))?;
    let rows = sql(query.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    }))?;
    let tombs: Vec<(String, String, String, i64)> = sql(rows.collect())?;
    for (table, remote, deleted, first_seq) in tombs {
        apply_tombstone_with_identity(
            conn,
            &table,
            &Value::String(remote),
            &deleted,
            None,
            first_seq,
        )?;
    }
    Ok(())
}
pub(crate) fn import_checkpoint_before_rows(
    conn: &Connection,
    value: &Value,
) -> Result<(), String> {
    transaction_required(conn)?;
    let snapshot = Snapshot::parse(value)?;
    // Retain keys before floor application deletes matching live rows/cascades.
    merge_keys_and_aliases(conn, &snapshot)?;
    merge_tomb_floors(conn, &snapshot)?;
    merge_unresolved(conn, &snapshot)
}
pub(crate) fn import_checkpoint_after_rows(conn: &Connection, value: &Value) -> Result<(), String> {
    transaction_required(conn)?;
    let snapshot = Snapshot::parse(value)?;
    merge_keys_and_aliases(conn, &snapshot)?;
    merge_tomb_floors(conn, &snapshot)?;
    merge_unresolved(conn, &snapshot)
}

#[cfg(test)]
#[path = "cloud_relay_identity_checkpoint_tests.rs"]
mod tests;
