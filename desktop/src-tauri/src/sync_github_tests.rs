use super::*;

fn canonical(timestamp: &str) -> String {
    crate::sync_owner::canonical_sync_timestamp(timestamp, "test").unwrap()
}

#[test]
fn dirty_rows_preserve_integer_primary_keys() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE events (
             id INTEGER PRIMARY KEY, title TEXT NOT NULL, updated_at TEXT NOT NULL
         );
         INSERT INTO events (id, title, updated_at)
         VALUES (42, 'Job', '2026-07-16T12:00:00Z');",
    )
    .unwrap();

    let rows = dirty_rows(&conn, "events", EPOCH_TS).unwrap();
    let row = row_to_json(&conn, "events", &rows[0].0).unwrap().unwrap();

    assert_eq!(
        rows,
        vec![(SqlValue::Integer(42), canonical("2026-07-16T12:00:00Z"))]
    );
    assert_eq!(row["id"], json!(42));
    assert_eq!(row["title"], json!("Job"));
    assert_eq!(row_label("events", &rows[0].0).unwrap(), "row:events_42");
}

#[test]
fn dirty_rows_preserve_text_primary_keys() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE sleep_sessions (
             id TEXT PRIMARY KEY, date TEXT NOT NULL, updated_at TEXT NOT NULL
         );
         CREATE TABLE health_log (
             id TEXT PRIMARY KEY, date TEXT NOT NULL, updated_at TEXT NOT NULL
         );
         CREATE TABLE schedules (
             id TEXT PRIMARY KEY, date TEXT NOT NULL, updated_at TEXT NOT NULL
         );
         INSERT INTO sleep_sessions (id, date, updated_at)
         VALUES (
             'sleep:2026-07-15:02:14', '2026-07-15', '2026-07-16T12:01:00Z'
         );
         INSERT INTO health_log (id, date, updated_at)
         VALUES (
             'health:steps:2026-07-15', '2026-07-15', '2026-07-16T12:02:00Z'
         );
         INSERT INTO schedules (id, date, updated_at)
         VALUES (
             'schedule:morning-walk', '2026-07-15', '2026-07-16T12:03:00Z'
         );",
    )
    .unwrap();

    for (table, id, updated_at) in [
        (
            "sleep_sessions",
            "sleep:2026-07-15:02:14",
            "2026-07-16T12:01:00Z",
        ),
        (
            "health_log",
            "health:steps:2026-07-15",
            "2026-07-16T12:02:00Z",
        ),
        ("schedules", "schedule:morning-walk", "2026-07-16T12:03:00Z"),
    ] {
        let rows = dirty_rows(&conn, table, EPOCH_TS).unwrap();
        let row = row_to_json(&conn, table, &rows[0].0).unwrap().unwrap();

        assert_eq!(rows, vec![(SqlValue::Text(id.into()), canonical(updated_at))]);
        assert_eq!(row["id"], json!(id));
        assert_eq!(row["date"], json!("2026-07-15"));
        assert_eq!(
            row_label(table, &rows[0].0).unwrap(),
            format!("row:{}_{}", table, id)
        );
    }
}

#[test]
fn dirty_rows_report_decode_errors() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE broken (id TEXT PRIMARY KEY, updated_at BLOB NOT NULL);
         INSERT INTO broken (id, updated_at) VALUES ('bad', X'FF');",
    )
    .unwrap();

    let error = dirty_rows(&conn, "broken", EPOCH_TS).unwrap_err();

    assert!(error.contains("decode dirty row"));
    assert!(error.contains("updated_at"));
}

#[test]
fn row_label_rejects_unsupported_primary_keys() {
    let error = row_label("bad_table", &SqlValue::Blob(vec![1, 2, 3])).unwrap_err();
    assert!(error.contains("unsupported primary key"));
}

fn sync_notes_conn(updated_at: &str) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE notes (
             id INTEGER PRIMARY KEY,
             title TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         CREATE TABLE sync_tombstones (
             table_name TEXT NOT NULL,
             row_id TEXT NOT NULL,
             deleted_at TEXT NOT NULL,
             UNIQUE(table_name, row_id)
         );
         CREATE TABLE app_settings (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE sync_row_versions (
             table_name TEXT NOT NULL,
             row_id TEXT NOT NULL,
             updated_at TEXT NOT NULL,
             device_id TEXT NOT NULL,
             PRIMARY KEY(table_name, row_id)
         );
         INSERT INTO app_settings(key,value) VALUES('device_id','local-a');
         CREATE TRIGGER notes_tombstone
         AFTER DELETE ON notes
         FOR EACH ROW
         BEGIN
             INSERT OR REPLACE INTO sync_tombstones(table_name,row_id,deleted_at)
             VALUES ('notes', OLD.id, '2099-01-01T00:00:00Z');
         END;",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO notes(id,title,updated_at) VALUES(1,'local',?1)",
        [updated_at],
    )
    .unwrap();
    // Install the same HLC/apply-context protocol as production only after
    // seeding the exact local timestamp used by the LWW assertions.
    crate::db::migrate_sync_meta(&conn).unwrap();
    conn
}

fn tombstone(timestamp: &str) -> Map<String, Value> {
    serde_json::from_value(json!({
        "_table": "tombstones",
        "_target_table": "notes",
        "_row_id": 1,
        "_deleted": true,
        "_updated_at": timestamp
    }))
    .unwrap()
}

#[test]
fn github_tombstone_stale_loses_to_newer_local_row() {
    let conn = sync_notes_conn("2026-09-01T12:01:00Z");

    assert!(!apply_doc(&conn, &tombstone("2026-09-01T12:00:00Z")).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=1", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM sync_tombstones", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn github_tombstone_equal_timestamp_deletes() {
    let timestamp = "2026-09-01T12:00:00Z";
    let conn = sync_notes_conn(timestamp);

    assert!(apply_doc(&conn, &tombstone(timestamp)).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=1", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    let stored: String = conn.query_row(
        "SELECT deleted_at FROM sync_tombstones WHERE table_name='notes' AND row_id='1'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(stored, canonical(timestamp));
}

#[test]
fn github_tombstone_newer_deletes_and_preserves_remote_timestamp() {
    let conn = sync_notes_conn("2026-09-01T12:00:00Z");
    let remote = "2026-09-01T12:01:00Z";

    assert!(apply_doc(&conn, &tombstone(remote)).unwrap());
    assert!(!apply_doc(&conn, &tombstone(remote)).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=1", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    let stored: String = conn.query_row(
        "SELECT deleted_at FROM sync_tombstones WHERE table_name='notes' AND row_id='1'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(stored, canonical(remote));
}

#[test]
fn github_stale_replay_enforces_newer_known_tombstone() {
    let conn = sync_notes_conn("2026-09-01T12:01:00Z");
    conn.execute(
        "INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES('notes','1',?1)",
        ["2026-09-01T12:02:00Z"],
    ).unwrap();

    assert!(apply_doc(&conn, &tombstone("2026-09-01T12:00:00Z")).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=1", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    let stored: String = conn.query_row(
        "SELECT deleted_at FROM sync_tombstones WHERE table_name='notes' AND row_id='1'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(stored, canonical("2026-09-01T12:02:00Z"));
}

#[test]
fn github_sql_apply_error_is_fail_closed() {
    let conn = sync_notes_conn("2026-09-01T12:00:00Z");
    let missing_required_title = serde_json::from_value(json!({
        "_table": "notes",
        "_device_id": "peer-z",
        "id": 2,
        "updated_at": "2026-09-01T12:01:00Z",
        "_updated_at": "2026-09-01T12:01:00Z"
    })).unwrap();

    let error = apply_doc(&conn, &missing_required_title).unwrap_err();
    assert!(error.contains("upsert") || error.contains("NOT NULL"));
    assert!(!crate::sync_owner::upsert_row(&conn, "notes", &missing_required_title).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=2", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn github_missing_local_schema_is_fail_closed() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let doc = serde_json::from_value(json!({
        "_table": "notes",
        "_device_id": "peer-z",
        "id": 1,
        "_updated_at": "2026-09-01T12:01:00Z"
    })).unwrap();

    assert!(apply_doc(&conn, &doc).is_err());
}

#[test]
fn github_malformed_or_unknown_item_is_error() {
    let conn = sync_notes_conn("2026-09-01T12:00:00Z");
    let missing_row_id = serde_json::from_value(json!({
        "_table": "tombstones",
        "_target_table": "notes",
        "_updated_at": "2026-09-01T12:01:00Z"
    })).unwrap();
    let missing_timestamp = serde_json::from_value(json!({
        "_table": "tombstones",
        "_target_table": "notes",
        "_row_id": 1
    })).unwrap();
    let unknown_table = serde_json::from_value(json!({
        "_table": "not_a_sync_table",
        "_device_id": "peer-z",
        "id": 1,
        "_updated_at": "2026-09-01T12:01:00Z"
    })).unwrap();
    let boolean_id = serde_json::from_value(json!({
        "_table": "notes",
        "_device_id": "peer-z",
        "id": true,
        "title": "invalid",
        "updated_at": "2026-09-01T12:01:00Z",
        "_updated_at": "2026-09-01T12:01:00Z"
    }))
    .unwrap();

    assert!(apply_doc(&conn, &missing_row_id).is_err());
    assert!(apply_doc(&conn, &missing_timestamp).is_err());
    assert!(apply_doc(&conn, &unknown_table).is_err());
    assert!(apply_doc(&conn, &boolean_id).is_err());
}

#[test]
fn github_pull_head_write_is_checked() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    assert!(save_pull_head(&conn, "new-head").is_err());

    conn.execute_batch(
        "CREATE TABLE app_settings (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );",
    ).unwrap();
    save_pull_head(&conn, "new-head").unwrap();
    let stored: String = conn.query_row(
        "SELECT value FROM app_settings WHERE key='cloud_owner_gh_pull_sha'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(stored, "new-head");
}

fn github_row(id: i64, title: Option<&str>, timestamp: &str) -> Map<String, Value> {
    let mut value = json!({
        "_table": "notes",
        "_device_id": "peer-z",
        "id": id,
        "updated_at": timestamp,
        "_updated_at": timestamp
    });
    if let Some(title) = title {
        value["title"] = json!(title);
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn github_valid_broken_valid_batch_keeps_rows_and_head_for_retry() {
    let mut conn = sync_notes_conn("2026-09-01T11:00:00Z");
    set_setting_checked(&conn, "cloud_owner_gh_pull_sha", "old-head").unwrap();
    let expected = Some("old-head".to_string());
    let broken = vec![
        (
            "peer/notes_2.json.enc".into(),
            github_row(2, Some("first"), "2026-09-01T12:00:00Z"),
        ),
        (
            "peer/notes_3.json.enc".into(),
            github_row(3, None, "2026-09-01T12:01:00Z"),
        ),
        (
            "peer/notes_4.json.enc".into(),
            github_row(4, Some("third"), "2026-09-01T12:02:00Z"),
        ),
    ];

    assert!(apply_github_documents(
        &mut conn,
        &expected,
        "new-head",
        &broken,
        "2026-09-01T12:03:00Z"
    )
    .is_err());
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE id IN (2,3,4)",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_gh_pull_sha").unwrap(),
        expected
    );
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_gh_last_pull_ts").unwrap(),
        None
    );

    let corrected = vec![
        (
            "peer/notes_2.json.enc".into(),
            github_row(2, Some("first"), "2026-09-01T12:00:00Z"),
        ),
        (
            "peer/notes_3.json.enc".into(),
            github_row(3, Some("fixed"), "2026-09-01T12:01:00Z"),
        ),
        (
            "peer/notes_4.json.enc".into(),
            github_row(4, Some("third"), "2026-09-01T12:02:00Z"),
        ),
    ];
    assert_eq!(
        apply_github_documents(
            &mut conn,
            &expected,
            "new-head",
            &corrected,
            "2026-09-01T12:03:00Z"
        )
        .unwrap(),
        3
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE id IN (2,3,4)",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        3
    );
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_gh_pull_sha")
            .unwrap()
            .as_deref(),
        Some("new-head")
    );
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_gh_last_pull_ts")
            .unwrap()
            .as_deref(),
        Some("2026-09-01T12:03:00Z")
    );
}

#[test]
fn github_checkpoint_write_failure_rolls_back_rows_and_head() {
    let mut conn = sync_notes_conn("2026-09-01T11:00:00Z");
    set_setting_checked(&conn, "cloud_owner_gh_pull_sha", "old-head").unwrap();
    conn.execute_batch(
        "CREATE TRIGGER reject_last_pull_checkpoint
         BEFORE INSERT ON app_settings
         WHEN NEW.key='cloud_owner_gh_last_pull_ts'
         BEGIN
             SELECT RAISE(ABORT, 'synthetic checkpoint failure');
         END;",
    )
    .unwrap();
    let documents = vec![(
        "peer/notes_2.json.enc".into(),
        github_row(2, Some("row"), "2026-09-01T12:00:00Z"),
    )];

    assert!(apply_github_documents(
        &mut conn,
        &Some("old-head".into()),
        "new-head",
        &documents,
        "2026-09-01T12:01:00Z"
    )
    .is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=2", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_gh_pull_sha")
            .unwrap()
            .as_deref(),
        Some("old-head")
    );
}

#[test]
fn github_compare_is_strict_and_uses_full_snapshot_at_raw_limit() {
    assert!(parse_compare(json!({"status": "ahead"})).is_err());
    assert!(parse_compare(json!({
        "status": "ahead",
        "files": [{"status": "removed"}]
    }))
    .is_err());
    assert!(parse_compare(json!({
        "status": "ahead",
        "files": [{"filename": "peer/a", "status": "mystery", "sha": "1"}]
    }))
    .is_err());

    let files = (0..300)
        .map(|index| json!({"filename": format!("peer/{index}"), "status": "removed"}))
        .collect::<Vec<_>>();
    assert_eq!(
        parse_compare(json!({"status": "ahead", "files": files})).unwrap(),
        ComparePlan::FullSnapshot
    );
    assert_eq!(
        parse_compare(json!({
            "status": "ahead",
            "files": [
                {"filename": "peer/gone", "status": "removed"},
                {"filename": "peer/row", "status": "modified", "sha": "abc"}
            ]
        }))
        .unwrap(),
        ComparePlan::Incremental(vec![("peer/row".into(), "abc".into())])
    );
}
