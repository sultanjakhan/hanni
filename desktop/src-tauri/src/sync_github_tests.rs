use super::*;

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
        vec![(SqlValue::Integer(42), "2026-07-16T12:00:00Z".into())]
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

        assert_eq!(rows, vec![(SqlValue::Text(id.into()), updated_at.into())]);
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

    assert!(error.contains("dirty row broken"));
}

#[test]
fn row_label_rejects_unsupported_primary_keys() {
    let error = row_label("bad_table", &SqlValue::Blob(vec![1, 2, 3])).unwrap_err();
    assert!(error.contains("unsupported primary key"));
}
