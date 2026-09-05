use super::*;
use serde_json::json;
const STORE: &str = "c9dd6d90-c9f7-4b1d-9d9c-6f7e7b127e00";
const OTHER: &str = "a9dd6d90-c9f7-4b1d-9d9c-6f7e7b127e00";
const RAW: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn fixture() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;
    CREATE TABLE sync_apply_context(singleton INTEGER PRIMARY KEY,remote_apply INTEGER);INSERT INTO sync_apply_context VALUES(1,1);
    CREATE TABLE cloud_relay_control(id INTEGER PRIMARY KEY,applying INTEGER);INSERT INTO cloud_relay_control VALUES(1,1);
    CREATE TABLE health_records(id TEXT PRIMARY KEY,source_store_id TEXT,record_type TEXT,source_revision INTEGER,payload_sha256 TEXT,is_deleted INTEGER,payload_json TEXT,updated_at TEXT,time_start_utc TEXT,time_end_utc TEXT);
    CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT,start_time TEXT,end_time TEXT,duration_minutes INTEGER,source TEXT,notes TEXT DEFAULT '',created_at TEXT,updated_at TEXT,UNIQUE(date,start_time,source));
    CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,session_id TEXT REFERENCES sleep_sessions(id) ON DELETE CASCADE,start_time TEXT,end_time TEXT,stage TEXT,updated_at TEXT,UNIQUE(session_id,start_time,end_time,stage));
    CREATE TABLE events(id INTEGER PRIMARY KEY AUTOINCREMENT,title TEXT,description TEXT,date TEXT,time TEXT,duration_minutes INTEGER,category TEXT,color TEXT,source TEXT,external_id TEXT,created_at TEXT,updated_at TEXT);
    CREATE TABLE timeline_activity_types(id INTEGER PRIMARY KEY,name TEXT,is_system INTEGER);INSERT INTO timeline_activity_types VALUES(17,'Сон',1);
    CREATE TABLE timeline_blocks(id INTEGER PRIMARY KEY AUTOINCREMENT,type_id INTEGER REFERENCES timeline_activity_types(id),date TEXT,start_time TEXT,end_time TEXT,duration_minutes INTEGER,source TEXT,notes TEXT,created_at TEXT,updated_at TEXT);
    INSERT INTO sleep_sessions(id,date,start_time,end_time,duration_minutes,source) VALUES('manual','2026-01-01','01:00','08:00',420,'manual'),('legacy','2026-01-01','01:00','08:00',420,'health_connect');
    INSERT INTO events(title,source,date,time) VALUES('Manual','manual','2026-01-01','01:00'),('Legacy','auto_health','2026-01-01','01:00');
    INSERT INTO timeline_blocks(type_id,source,date,start_time,end_time) VALUES(17,'manual','2026-01-01','01:00','08:00'),(17,'auto_health','2026-01-01','01:00','08:00');").unwrap();
    initialize(&c).unwrap();
    c
}
fn stamp(s: &str) -> Value {
    let t = DateTime::parse_from_rfc3339(s).unwrap();
    json!({"seconds":t.timestamp().to_string(),"nanos":t.timestamp_subsec_nanos()})
}
fn payload(start: &str, end: &str, kind: i64) -> String {
    json!({"v":1,"sdk":"androidx.health.connect:connect-client:1.1.0","record_type":"SleepSessionRecord","record":{
      "metadata":{"id":"actual-hc-id"},"startTime":stamp(start),"endTime":stamp(end),"startZoneOffset":18000,"endZoneOffset":18000,
      "title":null,"notes":"synthetic","stages":[{"startTime":stamp(start),"endTime":stamp(end),"stage":kind}]}}).to_string()
}
fn raw(c: &Connection, revision: i64, deleted: bool, p: &str, store: &str) {
    let value: Value = serde_json::from_str(p).unwrap();
    let start = instant(&value["record"]["startTime"])
        .ok()
        .map(|t| t.to_rfc3339());
    let end = instant(&value["record"]["endTime"])
        .ok()
        .map(|t| t.to_rfc3339());
    c.execute("INSERT INTO health_records VALUES(?1,?2,'SleepSessionRecord',?3,?4,?5,?6,'2026-02-01T00:00:00Z',?7,?8) ON CONFLICT(id) DO UPDATE SET source_revision=excluded.source_revision,payload_sha256=excluded.payload_sha256,is_deleted=excluded.is_deleted,payload_json=excluded.payload_json,time_start_utc=excluded.time_start_utc,time_end_utc=excluded.time_end_utc",
    params![RAW,store,revision,digest(p.as_bytes()),deleted as i64,p,start,end]).unwrap();
}
fn project(c: &Connection) -> ProjectionStatus {
    c.execute_batch("BEGIN IMMEDIATE").unwrap();
    let result = reconcile_pending(c, Some(STORE), 100).unwrap();
    c.execute_batch("COMMIT").unwrap();
    result
}
fn counts(c: &Connection) -> (i64, i64, i64, i64) {
    (
        c.query_row("SELECT COUNT(*) FROM sleep_sessions", [], |r| r.get(0))
            .unwrap(),
        c.query_row("SELECT COUNT(*) FROM sleep_stages", [], |r| r.get(0))
            .unwrap(),
        c.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap(),
        c.query_row("SELECT COUNT(*) FROM timeline_blocks", [], |r| r.get(0))
            .unwrap(),
    )
}

#[test]
fn correction_moves_same_sleep_and_both_automatic_blocks_then_delete_removes_only_owned() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    assert_eq!(project(&c).records, 1);
    let sid = format!("raw-sleep:{RAW}");
    let event = numeric_id(RAW, "calendar");
    let timeline = numeric_id(RAW, "timeline");
    assert_eq!(counts(&c), (3, 1, 3, 3));
    let p = payload("2026-01-02T19:30:00Z", "2026-01-03T02:30:00Z", 7);
    raw(&c, 2, false, &p, STORE);
    project(&c);
    assert_eq!(counts(&c), (3, 1, 3, 3));
    assert_eq!(
        c.query_row(
            "SELECT date||'/'||start_time FROM sleep_sessions WHERE id=?1",
            [&sid],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "2026-01-03/00:30"
    );
    assert_eq!(
        c.query_row(
            "SELECT date||'/'||time FROM events WHERE id=?1",
            [event],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "2026-01-03/00:30"
    );
    assert_eq!(
        c.query_row(
            "SELECT date||'/'||start_time FROM timeline_blocks WHERE id=?1",
            [timeline],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "2026-01-03/00:30"
    );
    assert_eq!(
        c.query_row("SELECT stage FROM sleep_stages", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "awake_in_bed"
    );
    raw(&c, 3, true, &p, STORE);
    project(&c);
    assert_eq!(counts(&c), (2, 0, 2, 2));
    assert!(is_local_projection(&c, "sleep_sessions", &sid).unwrap());
    assert!(is_local_projection(&c, "events", &event.to_string()).unwrap());
    assert!(is_local_projection(&c, "timeline_blocks", &timeline.to_string()).unwrap());
    assert!(!is_local_projection(&c, "sleep_sessions", "manual").unwrap());
    assert!(!is_local_projection(&c, "sleep_sessions", "legacy").unwrap());
}

#[test]
fn unavailable_authority_and_other_store_do_not_claim_or_project() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, OTHER);
    assert_eq!(
        reconcile_pending(&c, None, 100).unwrap().status,
        "authority_not_configured"
    );
    assert_eq!(project(&c).records, 0);
    assert_eq!(counts(&c), (2, 0, 2, 2));
}

#[test]
fn repeat_and_metadata_only_revision_do_not_churn_owned_rows() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    project(&c);
    assert_eq!(project(&c).records, 0);
    c.execute_batch("CREATE TABLE touches(n INTEGER);CREATE TRIGGER observe_projection_update AFTER UPDATE ON sleep_sessions BEGIN INSERT INTO touches VALUES(1); END;
    CREATE TRIGGER observe_event_update AFTER UPDATE ON events BEGIN INSERT INTO touches VALUES(1); END;
    CREATE TRIGGER observe_timeline_update AFTER UPDATE ON timeline_blocks BEGIN INSERT INTO touches VALUES(1); END;
    CREATE TRIGGER observe_stage_delete AFTER DELETE ON sleep_stages BEGIN INSERT INTO touches VALUES(1); END;").unwrap();
    let mut p: Value = serde_json::from_str(&p).unwrap();
    p["record"]["metadata"]["irrelevant"] = json!("changed");
    raw(&c, 2, false, &p.to_string(), STORE);
    project(&c);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM touches", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn malformed_second_stage_rolls_back_every_projection_write_and_progress() {
    let c = fixture();
    let mut p: Value =
        serde_json::from_str(&payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5)).unwrap();
    p["record"]["stages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"stage":5}));
    raw(&c, 1, false, &p.to_string(), STORE);
    let status = project(&c);
    assert_eq!(status.records, 0);
    assert_eq!(status.errors, 1);
    assert_eq!(status.status, "projection_partial");
    assert_eq!(counts(&c), (2, 0, 2, 2));
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM hc_sleep_projection_state", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
}

#[test]
fn identifier_collision_or_user_reclassified_block_is_preserved_and_pending() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    let event = numeric_id(RAW, "calendar");
    c.execute(
        "INSERT INTO events(id,source,title) VALUES(?1,'manual','Keep')",
        [event],
    )
    .unwrap();
    let status = project(&c);
    assert_eq!(status.records, 0);
    assert_eq!(status.errors, 1);
    assert_eq!(
        c.query_row(
            "SELECT error_code FROM hc_sleep_projection_errors",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        CONFLICT
    );
    assert_eq!(
        c.query_row("SELECT title FROM events WHERE id=?1", [event], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Keep"
    );
    assert_eq!(counts(&c), (2, 0, 3, 2));
}

#[test]
fn manually_added_stage_prevents_cascade_data_loss() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    project(&c);
    c.execute("INSERT INTO sleep_stages(id,session_id,start_time,end_time,stage) VALUES('manual-child',?1,'03:00','04:00','light')",[format!("raw-sleep:{RAW}")]).unwrap();
    raw(&c, 2, true, &p, STORE);
    let status = project(&c);
    assert_eq!(status.records, 0);
    assert_eq!(status.errors, 1);
    assert_eq!(status.status, "projection_partial");
    assert_eq!(counts(&c), (3, 2, 3, 3));
}

#[test]
fn budget_continues_and_authority_change_never_silently_replaces_read_models() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    c.execute("INSERT INTO health_records SELECT ?1,source_store_id,record_type,source_revision,payload_sha256,is_deleted,payload_json,updated_at,time_start_utc,time_end_utc FROM health_records",["b".repeat(64)]).unwrap();
    c.execute_batch("BEGIN IMMEDIATE").unwrap();
    let first = reconcile_pending(&c, Some(STORE), 1).unwrap();
    assert!(first.more_pending);
    c.execute_batch("COMMIT").unwrap();
    assert!(!project(&c).more_pending);
    assert_eq!(counts(&c), (4, 2, 4, 4));
    c.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        reconcile_pending(&c, Some(OTHER), 100).unwrap_err(),
        "hc_sleep_projection_authority_change_required"
    );
    c.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn missing_zone_offset_uses_system_rules_at_record_instant_and_rechecks_changed_offset() {
    let mut p: Value =
        serde_json::from_str(&payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 99)).unwrap();
    p["record"]["startZoneOffset"] = Value::Null;
    p["record"]["endZoneOffset"] = Value::Null;
    let sleep = decode(RAW, &p.to_string()).unwrap();
    assert!(sleep.fallback);
    assert_eq!(
        sleep.start,
        instant(&p["record"]["startTime"])
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%H:%M")
            .to_string()
    );
    assert!(sleep.stages.values().any(|s| s.2 == "unknown:99"));
    let c = fixture();
    raw(&c, 1, false, &p.to_string(), STORE);
    project(&c);
    assert_eq!(project(&c).records, 0);
    c.execute(
        "UPDATE hc_sleep_projection_state SET render_start_offset=render_start_offset+60",
        [],
    )
    .unwrap();
    assert_eq!(project(&c).records, 1);
    assert_eq!(project(&c).records, 0);
    p["record"]["startZoneOffset"] = json!(i64::MIN);
    assert!(decode(RAW, &p.to_string()).is_err());
}

#[test]
fn transport_filters_precede_limit_and_keep_manual_legacy_rows_and_tombs() {
    let c = fixture();
    c.execute_batch(
        "CREATE TABLE sync_tombstones(table_name TEXT,row_id TEXT,deleted_at TEXT);
    UPDATE sleep_sessions SET updated_at='2026-01-02T00:00:00Z';
    INSERT INTO sync_tombstones VALUES('sleep_sessions','legacy','2026-01-02T00:00:00Z');",
    )
    .unwrap();
    for n in 0..501 {
        let id = format!("raw-sleep:{n}");
        c.execute("INSERT INTO sleep_sessions(id,source,updated_at) VALUES(?1,'health_connect_raw:test','2026-01-01T00:00:00Z')",[&id]).unwrap();
        c.execute(
            "INSERT INTO sync_tombstones VALUES('sleep_sessions',?1,'2026-01-01T00:00:00Z')",
            [&id],
        )
        .unwrap();
    }
    let f = transport_row_filter(&c, "sleep_sessions").unwrap();
    let query=format!("WITH page AS(SELECT updated_at FROM sleep_sessions WHERE ({f}) ORDER BY updated_at LIMIT 1) SELECT id FROM sleep_sessions WHERE ({f}) AND updated_at<=(SELECT MAX(updated_at) FROM page) ORDER BY id");
    let ids: Vec<String> = c
        .prepare(&query)
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(ids, vec!["legacy", "manual"]);
    let f =
        transport_tomb_filter(&c, "sync_tombstones.table_name", "sync_tombstones.row_id").unwrap();
    let query=format!("WITH page AS(SELECT deleted_at FROM sync_tombstones WHERE ({f}) ORDER BY deleted_at LIMIT 1) SELECT row_id FROM sync_tombstones WHERE ({f}) AND deleted_at<=(SELECT MAX(deleted_at) FROM page)");
    assert_eq!(
        c.query_row(&query, [], |r| r.get::<_, String>(0)).unwrap(),
        "legacy"
    );
}

fn project_at(c: &Connection, limit: usize, now: i64) -> ProjectionStatus {
    c.execute_batch("BEGIN IMMEDIATE").unwrap();
    let result = reconcile_pending_at(c, Some(STORE), limit, now).unwrap();
    c.execute_batch("COMMIT").unwrap();
    result
}
fn duplicate_raw(c: &Connection, id: &str) {
    c.execute("INSERT INTO health_records SELECT ?1,source_store_id,record_type,source_revision,payload_sha256,is_deleted,payload_json,updated_at,time_start_utc,time_end_utc FROM health_records WHERE id=?2",params![id,RAW]).unwrap();
}

#[test]
fn late_row_sql_failure_rolls_back_only_failed_rows_and_latches_safe_error() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    duplicate_raw(&c, &"b".repeat(64));
    c.execute_batch(&format!("CREATE TRIGGER fail_one BEFORE INSERT ON timeline_blocks WHEN NEW.id={} BEGIN SELECT RAISE(FAIL,'private row data must not escape'); END;",numeric_id(RAW,"timeline"))).unwrap();
    let status = project_at(&c, 100, 1000);
    assert_eq!(
        (status.records, status.errors, status.pending_records),
        (1, 1, 1)
    );
    assert_eq!(status.status, "projection_partial");
    assert!(!status.more_pending);
    assert!(status.retry_needed);
    assert_eq!(status.next_retry_epoch, Some(1030));
    assert_eq!(status.last_projected_epoch, Some(1000));
    assert_eq!(counts(&c), (3, 1, 3, 3));
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM hc_sleep_projection_owned WHERE raw_id=?1",
            [RAW],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM hc_sleep_projection_state WHERE raw_id=?1",
            [RAW],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row(
            "SELECT error_code FROM hc_sleep_projection_errors",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        ERROR
    );
    assert_eq!(
        c.query_row("SELECT remote_apply FROM sync_apply_context", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT applying FROM cloud_relay_control", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    c.execute_batch("DROP TRIGGER fail_one").unwrap();
    assert_eq!(project_at(&c, 100, 1029).records, 0);
    let recovered = project_at(&c, 100, 1030);
    assert_eq!(recovered.status, "projected");
    assert_eq!(
        (
            recovered.records,
            recovered.errors,
            recovered.pending_records
        ),
        (1, 0, 0)
    );
    assert!(!recovered.retry_needed);
    assert_eq!(recovered.last_projected_epoch, Some(1030));
    assert_eq!(counts(&c), (4, 2, 4, 4));
}

#[test]
fn fresh_revision_bypasses_old_backoff_and_failed_rows_do_not_starve_healthy_budget() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, "{}", STORE);
    let first = project_at(&c, 1, 1000);
    assert_eq!(first.errors, 1);
    raw(&c, 2, false, &p, STORE);
    let corrected = project_at(&c, 1, 1001);
    assert_eq!(corrected.records, 1);
    assert_eq!(corrected.errors, 0);
    assert_eq!(corrected.status, "projected");
    raw(&c, 3, false, "{}", STORE);
    project_at(&c, 1, 1100);
    duplicate_raw(&c, &"b".repeat(64));
    c.execute(
        "UPDATE health_records SET payload_json=?1,payload_sha256=?2 WHERE id=?3",
        params![p, digest(p.as_bytes()), "b".repeat(64)],
    )
    .unwrap();
    // The old failure is due, but fresh work is first even though its ID sorts later.
    let next = project_at(&c, 1, 1200);
    assert_eq!(next.records, 1);
    assert_eq!(next.errors, 1);
    assert!(next.more_pending);
    let retry = project_at(&c, 1, 1200);
    assert_eq!(retry.records, 0);
    assert_eq!(retry.retry_after_seconds, Some(60));
    assert!(!retry.more_pending);
}

#[test]
fn durable_backoff_survives_sqlite_close_reopen_and_has_bounded_exponential_delay() {
    let c = fixture();
    raw(&c, 1, false, "{}", STORE);
    assert_eq!(project_at(&c, 100, 1000).next_retry_epoch, Some(1030));
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "hanni-projection-synthetic-{}-{suffix}.db",
        std::process::id()
    ));
    c.execute("VACUUM INTO ?1", [path.to_str().unwrap()])
        .unwrap();
    drop(c);
    let c = Connection::open(&path).unwrap();
    let status = project_at(&c, 100, 1029);
    assert_eq!(status.records, 0);
    assert_eq!(status.retry_after_seconds, Some(1));
    assert_eq!(
        c.query_row("SELECT attempts FROM hc_sleep_projection_errors", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let mut now = 1030;
    for attempt in 2..=12 {
        let status = project_at(&c, 100, now);
        let expected = (30_i64.saturating_mul(1_i64 << (attempt - 1).min(7))).min(3600);
        assert_eq!(status.retry_after_seconds, Some(expected));
        assert_eq!(status.errors, 1);
        assert!(!status.more_pending);
        now = status.next_retry_epoch.unwrap();
    }
    // Clock rollback resets an impossible future wait instead of freezing forever.
    assert_eq!(project_at(&c, 100, 1).retry_after_seconds, Some(3600));
    drop(c);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn missing_timeline_type_is_durable_error_without_partial_visible_rows_or_raw_rollback() {
    let c = fixture();
    c.execute("UPDATE timeline_activity_types SET is_system=0", [])
        .unwrap();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    let status = project_at(&c, 100, 1000);
    assert_eq!(status.records, 0);
    assert_eq!(status.errors, 1);
    assert_eq!(counts(&c), (2, 0, 2, 2));
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM health_records", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row(
            "SELECT error_code FROM hc_sleep_projection_errors",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "hc_sleep_projection_timeline_type_required"
    );
    c.execute("UPDATE timeline_activity_types SET is_system=1", [])
        .unwrap();
    assert_eq!(project_at(&c, 100, 1030).status, "projected");
}

#[test]
fn aggregate_status_is_readonly_and_never_claims_empty_uninitialized_or_failed_ready() {
    let empty = Connection::open_in_memory().unwrap();
    empty.execute_batch("PRAGMA query_only=ON").unwrap();
    assert_eq!(
        database_status_at(&empty, None, 1000).unwrap().status,
        "authority_not_configured"
    );
    assert_eq!(
        database_status_at(&empty, Some(STORE), 1000)
            .unwrap()
            .status,
        "projection_not_initialized"
    );
    assert_eq!(
        empty
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let c = fixture();
    raw(&c, 1, false, "{}", STORE);
    let projected = project_at(&c, 100, 1000);
    c.execute_batch("PRAGMA query_only=ON").unwrap();
    let read = database_status_at(&c, Some(STORE), 1001).unwrap();
    assert_eq!(read.status, projected.status);
    assert_eq!((read.records, read.pending_records, read.errors), (0, 1, 1));
    assert_eq!(read.retry_after_seconds, Some(29));
    assert_eq!(read.last_projected_epoch, None);
}

#[test]
fn failed_delete_retains_previous_good_revision_until_retry_preserves_manual_child() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    project_at(&c, 100, 1000);
    c.execute("INSERT INTO sleep_stages(id,session_id,start_time,end_time,stage) VALUES('manual-child',?1,'03:00','04:00','light')",[format!("raw-sleep:{RAW}")]).unwrap();
    raw(&c, 2, true, &p, STORE);
    let status = project_at(&c, 100, 1001);
    assert_eq!((status.records, status.errors), (0, 1));
    assert_eq!(
        c.query_row(
            "SELECT source_revision FROM hc_sleep_projection_state WHERE raw_id=?1",
            [RAW],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(counts(&c), (3, 2, 3, 3));
    assert_eq!(status.last_projected_epoch, Some(1000));
}

#[test]
fn projection_revision_changes_for_correction_and_delete_in_same_second_not_noop_or_failure() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    assert_eq!(project_at(&c, 100, 1000).projection_revision, "1");
    raw(
        &c,
        2,
        false,
        &payload("2026-01-02T20:00:00Z", "2026-01-03T03:00:00Z", 7),
        STORE,
    );
    assert_eq!(project_at(&c, 100, 1000).projection_revision, "2");
    raw(&c, 3, true, &p, STORE);
    assert_eq!(project_at(&c, 100, 1000).projection_revision, "3");
    assert_eq!(project_at(&c, 100, 1000).projection_revision, "3");
    raw(&c, 4, false, "{}", STORE);
    assert_eq!(project_at(&c, 100, 1000).projection_revision, "3");
    let read = database_status_at(&c, Some(STORE), 1000).unwrap();
    assert_eq!(read.projection_revision, "3");
    assert_eq!(read.last_projected_epoch, Some(1000));
    assert_eq!(read.status, "projection_partial");
}

#[test]
fn commands_can_reject_editing_only_owned_read_models_even_after_source_deletion() {
    let c = fixture();
    let p = payload("2026-01-01T20:00:00Z", "2026-01-02T03:00:00Z", 5);
    raw(&c, 1, false, &p, STORE);
    project(&c);
    for (table, id) in [
        ("sleep_sessions", format!("raw-sleep:{RAW}")),
        ("events", numeric_id(RAW, "calendar").to_string()),
        ("timeline_blocks", numeric_id(RAW, "timeline").to_string()),
    ] {
        assert!(ensure_user_editable(&c, table, &id).is_err());
    }
    assert!(ensure_user_editable(&c, "sleep_sessions", "manual").is_ok());
    assert!(ensure_user_editable(&c, "sleep_sessions", "legacy").is_ok());
    assert!(ensure_user_editable(&c, "events", "1").is_ok());
    raw(&c, 2, true, &p, STORE);
    project(&c);
    assert!(ensure_user_editable(&c, "events", &numeric_id(RAW, "calendar").to_string()).is_err());
}
