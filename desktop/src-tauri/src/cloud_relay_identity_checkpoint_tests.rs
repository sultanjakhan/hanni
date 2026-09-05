use super::super::{initialize, translate_row, unresolved_tomb_floor};
use super::*;
use serde_json::{json, Map};

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
      "source":"health_connect","updated_at":updated,"_updated_at":updated,"_device_id":"synthetic-source"}).as_object().unwrap().clone()
}
fn stage(id: &str, parent: &str, updated: &str) -> Map<String, Value> {
    json!({"id":id,"session_id":parent,"start_time":"01:00","end_time":"02:00","stage":"deep",
      "updated_at":updated,"_updated_at":updated,"_device_id":"synthetic-source"})
    .as_object()
    .unwrap()
    .clone()
}
fn upsert(conn: &Connection, table: &str, row: &Map<String, Value>) -> bool {
    crate::sync_owner::upsert_row_fail_closed(conn, table, row).unwrap()
}
fn seed(conn: &Connection, parent: &str, child: &str, updated: &str) {
    assert!(upsert(
        conn,
        "sleep_sessions",
        &session(parent, "01:00", updated)
    ));
    assert!(upsert(conn, "sleep_stages", &stage(child, parent, updated)));
}
fn export(conn: &Connection) -> Value {
    conn.execute_batch("BEGIN").unwrap();
    let value = export_checkpoint(conn).unwrap();
    conn.execute_batch("COMMIT").unwrap();
    value
}
fn alias(conn: &Connection, table: &str, remote: &str) -> String {
    local(conn, table, remote).unwrap()
}
fn import_empty(conn: &Connection, value: &Value) {
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    import_checkpoint_before_rows(conn, value).unwrap();
    import_checkpoint_after_rows(conn, value).unwrap();
    conn.execute_batch("COMMIT").unwrap();
}

#[test]
fn export_is_read_only_and_preserves_original_unresolved_sequence() {
    let conn = fixture();
    seed(&conn, "p", "c", OLD);
    logical::unresolved(&conn, "sleep_sessions", "unknown", DELETED, 7).unwrap();
    let before: i64 = conn
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let value = export(&conn);
    let after: i64 = conn
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(value["unresolved_tombs"][0]["first_seq"], json!(7));
    assert!(value["logical_keys"]
        .as_array()
        .unwrap()
        .iter()
        .all(|k| k["key_hash"].as_str().unwrap().len() == 64));
    assert_eq!(
        export_checkpoint(&conn).unwrap_err(),
        "relay_checkpoint_transaction_required"
    );
}

#[test]
fn aliases_and_parent_ids_remap_without_replacing_newer_recipient_rows() {
    let source = fixture();
    seed(&source, "a", "ca", OLD);
    source
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','foreign','a')",
            [],
        )
        .unwrap();
    source
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_stages','foreign-child','ca')",
            [],
        )
        .unwrap();
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "b", "cb", NEW);
    receiver
        .execute(
            "INSERT INTO cloud_relay_dirty(table_name,row_id) VALUES('sleep_sessions','b')",
            [],
        )
        .unwrap();
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    import_checkpoint_before_rows(&receiver, &value).unwrap();
    let row = translate_row(&receiver, "sleep_sessions", &session("a", "01:00", OLD))
        .unwrap()
        .unwrap();
    assert_eq!(row["id"], json!("b"));
    assert!(!upsert(&receiver, "sleep_sessions", &row));
    let row = translate_row(&receiver, "sleep_stages", &stage("ca", "a", OLD))
        .unwrap()
        .unwrap();
    assert_eq!(row["id"], json!("cb"));
    assert_eq!(row["session_id"], json!("b"));
    assert!(!upsert(&receiver, "sleep_stages", &row));
    import_checkpoint_after_rows(&receiver, &value).unwrap();
    receiver.execute_batch("COMMIT").unwrap();
    assert_eq!(alias(&receiver, "sleep_sessions", "foreign"), "b");
    assert_eq!(alias(&receiver, "sleep_stages", "foreign-child"), "cb");
    let dirty: i64 = receiver
        .query_row("SELECT COUNT(*) FROM cloud_relay_dirty", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dirty, 1);
    assert_eq!(
        receiver
            .query_row("SELECT updated_at FROM sleep_sessions", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        NEW
    );
}

#[test]
fn logical_deletion_floor_remaps_and_blocks_resurrection_with_cascade() {
    let source = fixture();
    seed(&source, "a", "ca", OLD);
    apply_tombstone(&source, "sleep_sessions", &json!("a"), DELETED).unwrap();
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "b", "cb", NEW);
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    import_checkpoint_before_rows(&receiver, &value).unwrap();
    assert!(!exists(&receiver, "sleep_sessions", "b").unwrap());
    assert!(!exists(&receiver, "sleep_stages", "cb").unwrap());
    let row = translate_row(&receiver, "sleep_sessions", &session("a", "01:00", OLD))
        .unwrap()
        .unwrap();
    assert!(!upsert(&receiver, "sleep_sessions", &row));
    assert!(
        translate_row(&receiver, "sleep_stages", &stage("ca", "a", OLD))
            .unwrap()
            .is_none()
    );
    import_checkpoint_after_rows(&receiver, &value).unwrap();
    receiver.execute_batch("COMMIT").unwrap();
    assert_eq!(
        tomb_timestamp(&receiver, "sleep_sessions", "b")
            .unwrap()
            .as_deref(),
        Some(DELETED)
    );
    assert_eq!(
        tomb_timestamp(&receiver, "sleep_stages", "cb")
            .unwrap()
            .as_deref(),
        Some(DELETED)
    );
    assert_eq!(alias(&receiver, "sleep_stages", "ca"), "cb");
}

#[test]
fn unresolved_union_keeps_earliest_first_seq_and_latest_delete() {
    let source = fixture();
    logical::unresolved(&source, "sleep_sessions", "unknown", DELETED, 17).unwrap();
    let value = export(&source);
    let receiver = fixture();
    logical::unresolved(&receiver, "sleep_sessions", "unknown", OLD, 3).unwrap();
    logical::unresolved(&receiver, "heart_rate_samples", "recipient-only", NEW, 9).unwrap();
    import_empty(&receiver, &value);
    assert_eq!(unresolved_tomb_floor(&receiver).unwrap(), Some(2));
    assert_eq!(
        receiver
            .query_row(
                "SELECT COUNT(*) FROM cloud_relay_unresolved_tombs",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    let entry:(String,i64)=receiver.query_row("SELECT deleted_at,first_seq FROM cloud_relay_unresolved_tombs WHERE remote_id='unknown'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(entry, (DELETED.into(), 3));
    import_empty(&receiver, &value);
    assert_eq!(unresolved_tomb_floor(&receiver).unwrap(), Some(2));
}

#[test]
fn ambiguous_mapping_rolls_back_complete_import() {
    let source = fixture();
    seed(&source, "a", "ca", OLD);
    source
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','shared','a')",
            [],
        )
        .unwrap();
    logical::unresolved(&source, "health_log", "unknown", DELETED, 5).unwrap();
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "b", "cb", NEW);
    assert!(upsert(
        &receiver,
        "sleep_sessions",
        &session("different", "02:00", NEW)
    ));
    receiver
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','shared','different')",
            [],
        )
        .unwrap();
    let before = export(&receiver);
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        import_checkpoint_before_rows(&receiver, &value).unwrap_err(),
        AMBIGUOUS
    );
    receiver.execute_batch("ROLLBACK").unwrap();
    assert_eq!(export(&receiver), before);
    assert_eq!(alias(&receiver, "sleep_sessions", "shared"), "different");
}

#[test]
fn retained_deleted_identity_maps_without_live_row_and_keeps_newer_local_floor() {
    let source = fixture();
    seed(&source, "a", "ca", OLD);
    apply_tombstone(&source, "sleep_sessions", &json!("a"), NEW).unwrap();
    source
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','foreign','a')",
            [],
        )
        .unwrap();
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "b", "cb", OLD);
    apply_tombstone(&receiver, "sleep_sessions", &json!("b"), DELETED).unwrap();
    import_empty(&receiver, &value);
    assert_eq!(alias(&receiver, "sleep_sessions", "a"), "b");
    assert_eq!(alias(&receiver, "sleep_sessions", "foreign"), "b");
    assert_eq!(
        tomb_timestamp(&receiver, "sleep_sessions", "b")
            .unwrap()
            .as_deref(),
        Some(DELETED)
    );
}

#[test]
fn fresh_receiver_can_apply_parent_then_stage_with_imported_aliases() {
    let source = fixture();
    seed(&source, "a", "ca", OLD);
    source
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','foreign','a')",
            [],
        )
        .unwrap();
    let value = export(&source);
    let receiver = fixture();
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    import_checkpoint_before_rows(&receiver, &value).unwrap();
    let parent = translate_row(&receiver, "sleep_sessions", &session("a", "01:00", OLD))
        .unwrap()
        .unwrap();
    assert!(upsert(&receiver, "sleep_sessions", &parent));
    let child = translate_row(&receiver, "sleep_stages", &stage("ca", "a", OLD))
        .unwrap()
        .unwrap();
    assert!(upsert(&receiver, "sleep_stages", &child));
    import_checkpoint_after_rows(&receiver, &value).unwrap();
    receiver.execute_batch("COMMIT").unwrap();
    assert_eq!(alias(&receiver, "sleep_sessions", "foreign"), "a");
    assert!(exists(&receiver, "sleep_stages", "ca").unwrap());
}

#[test]
fn newer_recipient_natural_edit_is_not_overwritten_by_old_snapshot_registry() {
    let source = fixture();
    assert!(upsert(
        &source,
        "sleep_sessions",
        &session("same", "01:00", OLD)
    ));
    let value = export(&source);
    let receiver = fixture();
    assert!(upsert(
        &receiver,
        "sleep_sessions",
        &session("same", "00:30", NEW)
    ));
    let before = logical::exported(&receiver, "sleep_sessions", &json!("same"))
        .unwrap()
        .unwrap()
        .natural_key_sha256;
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    import_checkpoint_before_rows(&receiver, &value).unwrap();
    let row = translate_row(&receiver, "sleep_sessions", &session("same", "01:00", OLD))
        .unwrap()
        .unwrap();
    assert!(!upsert(&receiver, "sleep_sessions", &row));
    import_checkpoint_after_rows(&receiver, &value).unwrap();
    receiver.execute_batch("COMMIT").unwrap();
    assert_eq!(
        logical::exported(&receiver, "sleep_sessions", &json!("same"))
            .unwrap()
            .unwrap()
            .natural_key_sha256,
        before
    );
}

#[test]
fn rejects_hash_mismatch_alias_cycles_and_unknown_fields_before_writes() {
    let source = fixture();
    seed(&source, "a", "ca", OLD);
    let value = export(&source);
    let receiver = fixture();
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut invalid = value.clone();
    invalid["logical_keys"][0]["key_hash"] = json!("0".repeat(64));
    assert!(import_checkpoint_before_rows(&receiver, &invalid).is_err());
    let mut invalid = value.clone();
    invalid["aliases"] = json!([
      {"table_name":"sleep_sessions","remote_id":"a","local_id":"b"},
      {"table_name":"sleep_sessions","remote_id":"b","local_id":"a"}]);
    assert!(import_checkpoint_before_rows(&receiver, &invalid).is_err());
    let mut invalid = value;
    invalid["unexpected"] = json!(true);
    assert!(import_checkpoint_before_rows(&receiver, &invalid).is_err());
    assert_eq!(
        receiver
            .query_row("SELECT COUNT(*) FROM cloud_relay_logical_keys", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
    receiver.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn deleted_anchor_rebind_flattens_prior_aliases_and_preserves_parent_translation() {
    const LIVE: &str = "2026-01-01T04:00:00.000000000Z";
    const LATER_DELETE: &str = "2026-01-01T05:00:00.000000000Z";
    let source = fixture();
    assert!(upsert(
        &source,
        "sleep_sessions",
        &session("a", "01:00", LIVE)
    ));
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "a", "old-child", OLD);
    receiver
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','r','a')",
            [],
        )
        .unwrap();
    apply_tombstone(&receiver, "sleep_sessions", &json!("a"), NEW).unwrap();
    seed(&receiver, "b", "new-child", LIVE);
    import_empty(&receiver, &value);
    assert_eq!(alias(&receiver, "sleep_sessions", "a"), "b");
    assert_eq!(alias(&receiver, "sleep_sessions", "r"), "b");
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    let child = translate_row(
        &receiver,
        "sleep_stages",
        &stage("foreign-child", "r", LIVE),
    )
    .unwrap()
    .unwrap();
    assert_eq!(child["session_id"], json!("b"));
    assert_eq!(child["id"], json!("new-child"));
    receiver.execute_batch("COMMIT").unwrap();
    let snapshot = export(&receiver); // Snapshot validation rejects any alias chain.
    assert!(snapshot["aliases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["table_name"] == "sleep_sessions")
        .all(|a| a["local_id"] == "b"));
    apply_tombstone_with_identity(
        &receiver,
        "sleep_sessions",
        &json!("r"),
        LATER_DELETE,
        None,
        9,
    )
    .unwrap();
    assert!(!exists(&receiver, "sleep_sessions", "b").unwrap());
    assert!(!exists(&receiver, "sleep_stages", "new-child").unwrap());
    assert_eq!(
        tomb_timestamp(&receiver, "sleep_sessions", "b")
            .unwrap()
            .as_deref(),
        Some(LATER_DELETE)
    );
}

#[test]
fn rebinding_forwards_legacy_tombs_from_all_redirected_aliases() {
    let source = fixture();
    assert!(upsert(
        &source,
        "sleep_sessions",
        &session("a", "01:00", OLD)
    ));
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "a", "old-child", OLD);
    apply_tombstone(&receiver, "sleep_sessions", &json!("a"), OLD).unwrap();
    seed(&receiver, "b", "new-child", NEW);
    receiver
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','r','a')",
            [],
        )
        .unwrap();
    // Represents a retained historical deletion which was not translated at arrival.
    receiver
        .execute(
            "INSERT INTO sync_tombstones VALUES('sleep_sessions','r',?1)",
            [DELETED],
        )
        .unwrap();
    import_empty(&receiver, &value);
    assert_eq!(alias(&receiver, "sleep_sessions", "r"), "b");
    assert!(!exists(&receiver, "sleep_sessions", "b").unwrap());
    assert!(!exists(&receiver, "sleep_stages", "new-child").unwrap());
    assert_eq!(
        tomb_timestamp(&receiver, "sleep_sessions", "b")
            .unwrap()
            .as_deref(),
        Some(DELETED)
    );
    assert_eq!(
        tomb_timestamp(&receiver, "sleep_sessions", "r")
            .unwrap()
            .as_deref(),
        Some(DELETED)
    );
    export(&receiver);
}

#[test]
fn rebinding_rejects_live_alias_collision_and_rolls_back_all_state() {
    let source = fixture();
    assert!(upsert(
        &source,
        "sleep_sessions",
        &session("a", "01:00", OLD)
    ));
    let value = export(&source);
    let receiver = fixture();
    seed(&receiver, "a", "old-child", OLD);
    apply_tombstone(&receiver, "sleep_sessions", &json!("a"), OLD).unwrap();
    seed(&receiver, "b", "new-child", NEW);
    assert!(upsert(
        &receiver,
        "sleep_sessions",
        &session("r", "02:00", NEW)
    ));
    receiver
        .execute(
            "INSERT INTO cloud_relay_aliases VALUES('sleep_sessions','r','a')",
            [],
        )
        .unwrap();
    let before = export(&receiver);
    receiver.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        import_checkpoint_before_rows(&receiver, &value).unwrap_err(),
        AMBIGUOUS
    );
    receiver.execute_batch("ROLLBACK").unwrap();
    assert_eq!(export(&receiver), before);
    assert!(exists(&receiver, "sleep_sessions", "b").unwrap());
    assert!(exists(&receiver, "sleep_sessions", "r").unwrap());
    assert_eq!(receiver.query_row("SELECT local_id FROM cloud_relay_aliases WHERE table_name='sleep_sessions' AND remote_id='r'", [], |r| r.get::<_, String>(0)).unwrap(), "a");
}
