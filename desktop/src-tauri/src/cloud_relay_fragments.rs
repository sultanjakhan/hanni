//! Large rows remain immutable in the outbox and become visible only after
//! every authenticated part is durable and the full row digest is verified.
use super::*;

const PART_BYTES: usize = 40_000;
const ROW_BYTES: usize = 8 * 1024 * 1024;
const STAGING_BYTES: i64 = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fragment {
    sha256: String,
    part: usize,
    parts: usize,
    bytes: usize,
    data: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointPart {
    sender: String,
    first_seq: i64,
    fragment: Fragment,
}

pub(super) fn initialize(conn: &Connection) -> Result<(), String> {
    sql(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cloud_relay_fragments(
        sender TEXT NOT NULL,sha256 TEXT NOT NULL,part INTEGER NOT NULL,parts INTEGER NOT NULL,
        bytes INTEGER NOT NULL,first_seq INTEGER NOT NULL,data BLOB NOT NULL,
        PRIMARY KEY(sender,sha256,part));",
    ))
}

pub(super) fn applied_cursor(conn: &Connection) -> Result<i64, String> {
    let fragments=scalar(conn, "SELECT MIN(receive_seq,COALESCE((SELECT MIN(first_seq)-1 FROM cloud_relay_fragments),receive_seq))
        FROM cloud_relay_state WHERE id=1")?;
    Ok(fragments.min(identity::unresolved_tomb_floor(conn)?.unwrap_or(fragments)))
}

pub(super) fn enqueue(
    conn: &Connection,
    cfg: &RelayConfig,
    row: &Row,
    applied_seq: i64,
    captured_seq: i64,
) -> Result<(), String> {
    // The caller owns one IMMEDIATE transaction containing all encrypted
    // parts plus removal of the captured journal revision. A crash cannot
    // leave a half-enqueued record or lose a newer local mutation.
    let encoded = serde_json::to_vec(row).map_err(|_| "relay_encode_failed")?;
    if encoded.len() > ROW_BYTES {
        return Err("relay_record_exceeds_8mib".into());
    }
    let digest = hash(&encoded);
    let count = encoded.len().div_ceil(PART_BYTES);
    for (part, data) in encoded.chunks(PART_BYTES).enumerate() {
        let payload = Payload {
            v: 1,
            kind: "fragment".into(),
            applied_seq,
            rows: vec![],
            tombs: vec![],
            fragment: Some(Fragment {
                sha256: digest.clone(),
                part,
                parts: count,
                bytes: encoded.len(),
                data: B64.encode(data),
            }),
        };
        persist_payload(conn, cfg, &payload)?;
    }
    sql(conn.execute("DELETE FROM cloud_relay_dirty WHERE seq=?1", [captured_seq]))?;
    sql(conn.execute(
        "UPDATE cloud_relay_state SET receipt_needed=0 WHERE id=1",
        [],
    ))?;
    Ok(())
}

pub(super) fn accept(
    conn: &Connection,
    sender: &str,
    seq: i64,
    part: Fragment,
) -> Result<Option<Row>, String> {
    store_part(conn, sender, seq, &part)?;
    let count: i64 = sql(conn.query_row(
        "SELECT COUNT(*) FROM cloud_relay_fragments WHERE sender=?1 AND sha256=?2",
        params![sender, part.sha256],
        |r| r.get(0),
    ))?;
    if count != part.parts as i64 {
        return Ok(None);
    }
    let mut data = Vec::with_capacity(part.bytes);
    {
        let mut stmt = sql(conn.prepare(
            "SELECT data FROM cloud_relay_fragments WHERE sender=?1 AND sha256=?2 ORDER BY part",
        ))?;
        let rows = sql(stmt.query_map(params![sender, part.sha256], |r| r.get::<_, Vec<u8>>(0)))?;
        for row in rows {
            data.extend(sql(row)?);
        }
    }
    if data.len() != part.bytes || hash(&data) != part.sha256 {
        return Err("relay_fragment_digest_mismatch".into());
    }
    let row: Row = serde_json::from_slice(&data).map_err(|_| "relay_invalid_fragment_row")?;
    sql(conn.execute(
        "DELETE FROM cloud_relay_fragments WHERE sender=?1 AND sha256=?2",
        params![sender, part.sha256],
    ))?;
    Ok(Some(row))
}

fn store_part(conn: &Connection, sender: &str, seq: i64, part: &Fragment) -> Result<(), String> {
    if !opaque_id(sender)
        || seq <= 0
        || part.bytes == 0
        || part.bytes > ROW_BYTES
        || part.parts != part.bytes.div_ceil(PART_BYTES)
        || part.part >= part.parts
        || part.sha256.len() != 64
        || !part
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("relay_invalid_fragment".into());
    }
    let expected = if part.part + 1 == part.parts {
        part.bytes - part.part * PART_BYTES
    } else {
        PART_BYTES
    };
    let bytes = decode(&part.data, expected)?;
    let previous: Option<(i64, i64)> = sql(conn
        .query_row(
            "SELECT parts,bytes FROM cloud_relay_fragments WHERE sender=?1 AND sha256=?2 LIMIT 1",
            params![sender, part.sha256],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional())?;
    if previous.is_some_and(|meta| meta != (part.parts as i64, part.bytes as i64)) {
        return Err("relay_fragment_conflict".into());
    }
    let stored: Option<Vec<u8>> = sql(conn
        .query_row(
            "SELECT data FROM cloud_relay_fragments WHERE sender=?1 AND sha256=?2 AND part=?3",
            params![sender, part.sha256, part.part as i64],
            |r| r.get(0),
        )
        .optional())?;
    if let Some(old) = stored {
        if old != bytes {
            return Err("relay_fragment_conflict".into());
        }
        sql(conn.execute(
            "UPDATE cloud_relay_fragments SET first_seq=MIN(first_seq,?1)
            WHERE sender=?2 AND sha256=?3 AND part=?4",
            params![seq, sender, part.sha256, part.part as i64],
        ))?;
    } else {
        if scalar(
            conn,
            "SELECT COALESCE(SUM(LENGTH(data)),0) FROM cloud_relay_fragments",
        )? + bytes.len() as i64
            > STAGING_BYTES
        {
            return Err("relay_fragment_staging_full".into());
        }
        sql(conn.execute(
            "INSERT INTO cloud_relay_fragments VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                sender,
                part.sha256,
                part.part as i64,
                part.parts as i64,
                part.bytes as i64,
                seq,
                bytes
            ],
        ))?;
    }
    Ok(())
}

// Called inside the checkpoint's consistent read transaction. Each entry is
// bounded by one part, without materializing the full staging table in memory.
pub(super) fn checkpoint_export(
    conn: &Connection,
    emit: &mut impl FnMut(Value) -> Result<(), String>,
) -> Result<(), String> {
    let mut stmt = sql(conn.prepare(
        "SELECT sender,first_seq,sha256,part,parts,bytes,data
        FROM cloud_relay_fragments ORDER BY sender,sha256,part",
    ))?;
    let rows = sql(stmt.query_map([], |r| {
        Ok(CheckpointPart {
            sender: r.get(0)?,
            first_seq: r.get(1)?,
            fragment: Fragment {
                sha256: r.get(2)?,
                part: r.get::<_, i64>(3)? as usize,
                parts: r.get::<_, i64>(4)? as usize,
                bytes: r.get::<_, i64>(5)? as usize,
                data: B64.encode(r.get::<_, Vec<u8>>(6)?),
            },
        })
    }))?;
    for row in rows {
        emit(serde_json::to_value(sql(row)?).map_err(|_| "relay_encode_failed")?)?;
    }
    Ok(())
}

/// Caller owns the bootstrap IMMEDIATE transaction and first deletes covered
/// local parts. Complete groups are forbidden: published staging is partial.
pub(super) fn checkpoint_import_entry(
    conn: &Connection,
    value: Value,
    base_seq: i64,
) -> Result<(), String> {
    let item: CheckpointPart =
        serde_json::from_value(value).map_err(|_| "relay_invalid_checkpoint_fragment")?;
    if item.first_seq > base_seq {
        return Err("relay_invalid_checkpoint_fragment".into());
    }
    store_part(conn, &item.sender, item.first_seq, &item.fragment)?;
    let count: i64 = sql(conn.query_row(
        "SELECT COUNT(*) FROM cloud_relay_fragments WHERE sender=?1 AND sha256=?2",
        params![item.sender, item.fragment.sha256],
        |r| r.get(0),
    ))?;
    if count >= item.fragment.parts as i64 {
        return Err("relay_invalid_checkpoint_fragment".into());
    }
    Ok(())
}
