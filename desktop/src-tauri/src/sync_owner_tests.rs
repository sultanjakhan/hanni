use super::*;

const SYNCED_AT: &str = "2026-09-01T12:00:00Z";
const PREFIX: &str = "projects/test/databases/(default)/documents/owners/user/data/";

fn sync_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE app_settings (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE notes (
             id INTEGER PRIMARY KEY,
             title TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         CREATE TABLE sync_tombstones (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             table_name TEXT NOT NULL,
             row_id TEXT NOT NULL,
             deleted_at TEXT NOT NULL,
             UNIQUE(table_name, row_id)
         );
         CREATE TABLE sync_row_versions (
             table_name TEXT NOT NULL,
             row_id TEXT NOT NULL,
             updated_at TEXT NOT NULL,
             device_id TEXT NOT NULL,
             PRIMARY KEY(table_name, row_id)
         );
         INSERT INTO app_settings(key,value) VALUES('device_id','local-a');",
    )
    .unwrap();
    crate::db::migrate_sync_meta(&conn).unwrap();
    conn
}

fn firestore_note(
    sequence: usize,
    id: i64,
    title: Option<&str>,
    synced_at: &str,
) -> FirestoreDocument {
    let mut fields = serde_json::Map::new();
    fields.insert("_synced_at".into(), json!(synced_at));
    fields.insert("_device_id".into(), json!("peer"));
    fields.insert("_table".into(), json!("notes"));
    fields.insert("_updated_at".into(), json!(synced_at));
    fields.insert("id".into(), json!(id));
    fields.insert("updated_at".into(), json!(synced_at));
    if let Some(title) = title {
        fields.insert("title".into(), json!(title));
    }
    FirestoreDocument {
        name: format!("{PREFIX}row_{sequence:04}"),
        update_time: canonical_sync_timestamp(synced_at, "test").unwrap(),
        fields,
    }
}

fn encoded_note(sequence: usize) -> Value {
    json!({
        "document": {
            "name": format!("{PREFIX}row_{sequence:04}"),
            "updateTime": "2026-09-01T12:00:01Z",
            "fields": {
                "_synced_at": {"timestampValue": SYNCED_AT},
                "_device_id": {"stringValue": "peer"},
                "_table": {"stringValue": "notes"},
                "_updated_at": {"stringValue": SYNCED_AT},
                "id": {"integerValue": sequence.to_string()},
                "title": {"stringValue": format!("note {sequence}")},
                "updated_at": {"stringValue": SYNCED_AT}
            }
        },
        "readTime": "2026-09-01T12:00:01Z"
    })
}

#[test]
fn firestore_query_uses_composite_start_after() {
    let cursor = FirestoreCursor {
        synced_at: canonical_sync_timestamp(SYNCED_AT, "test").unwrap(),
        document_name: Some(format!("{PREFIX}row_0500")),
    };
    let body = firestore_query_body("data", &cursor, false);

    assert_eq!(
        body.pointer("/structuredQuery/where/fieldFilter/op"),
        Some(&json!("GREATER_THAN_OR_EQUAL"))
    );
    assert_eq!(
        body.pointer("/structuredQuery/where/fieldFilter/value/timestampValue"),
        Some(&json!(canonical_sync_timestamp(SYNCED_AT, "test").unwrap()))
    );
    assert_eq!(
        body.pointer("/structuredQuery/orderBy/0/field/fieldPath"),
        Some(&json!("_synced_at"))
    );
    assert_eq!(
        body.pointer("/structuredQuery/orderBy/1/field/fieldPath"),
        Some(&json!("__name__"))
    );
    assert_eq!(
        body.pointer("/structuredQuery/startAt/values/0/timestampValue"),
        Some(&json!(canonical_sync_timestamp(SYNCED_AT, "test").unwrap()))
    );
    assert_eq!(
        body.pointer("/structuredQuery/startAt/values/1/referenceValue"),
        Some(&json!(format!("{PREFIX}row_0500")))
    );
    assert_eq!(
        body.pointer("/structuredQuery/startAt/before"),
        Some(&json!(false))
    );
}

#[test]
fn firestore_cursor_migration_uses_one_snapshot_transaction_for_every_page() {
    assert_eq!(
        parse_begin_transaction_payload(json!({"transaction": "opaque-read-tx"})).unwrap(),
        "opaque-read-tx"
    );
    for malformed in [json!({}), json!({"transaction": ""}), json!({"transaction": 1})] {
        assert!(parse_begin_transaction_payload(malformed).is_err());
    }

    let first = firestore_list_query("opaque-read-tx", None);
    let second = firestore_list_query("opaque-read-tx", Some("opaque-page-token"));
    for query in [&first, &second] {
        assert!(query
            .iter()
            .any(|(key, value)| *key == "transaction" && value == "opaque-read-tx"));
        assert!(query
            .iter()
            .any(|(key, value)| *key == "orderBy" && value == "__name__"));
    }
    assert!(!first.iter().any(|(key, _)| *key == "pageToken"));
    assert!(second.iter().any(|(key, value)| {
        *key == "pageToken" && value == "opaque-page-token"
    }));
}

#[test]
fn firestore_commit_uses_server_request_time() {
    let encoded = encode_doc(
        &json!({"id": 1, "title": "row", "updated_at": SYNCED_AT}),
        "local-a",
        SYNCED_AT,
        "notes",
    );
    let commit = firestore_commit_body("project", "owners/user/data", "notes_1", &encoded).unwrap();

    assert_eq!(
        commit.pointer("/writes/0/update/name"),
        Some(&json!(
            "projects/project/databases/(default)/documents/owners/user/data/notes_1"
        ))
    );
    assert_eq!(
        commit.pointer("/writes/0/updateTransforms/0/setToServerValue"),
        Some(&json!("REQUEST_TIME"))
    );
    assert!(commit
        .pointer("/writes/0/update/fields/_synced_at")
        .is_none());
}

#[test]
fn firestore_decoder_accepts_metadata_only_and_rejects_malformed_sources() {
    assert!(
        parse_run_query_payload(json!([{"readTime": "2026-09-01T12:00:01Z"}]), PREFIX)
            .unwrap()
            .is_empty()
    );
    let decoded = parse_run_query_payload(json!([encoded_note(1)]), PREFIX).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(
        decoded[0].update_time,
        canonical_sync_timestamp("2026-09-01T12:00:01Z", "test").unwrap()
    );

    for malformed in [
        json!({}),
        json!([]),
        json!([{}]),
        json!([{"readTime": 1}]),
        json!([{"readTime": "ok", "skippedResults": 1}]),
        json!([{"document": null}]),
        json!([{
            "document": {
                "name": format!("{PREFIX}row_bad"),
                "fields": {
                    "_synced_at": {"stringValue": SYNCED_AT, "integerValue": "1"},
                    "_device_id": {"stringValue": "peer"},
                    "_table": {"stringValue": "notes"}
                }
            }
        }]),
        json!([{
            "document": {
                "name": "projects/test/databases/(default)/documents/owners/other/data/row",
                "fields": {
                    "_synced_at": {"stringValue": SYNCED_AT},
                    "_device_id": {"stringValue": "peer"},
                    "_table": {"stringValue": "notes"}
                }
            }
        }]),
    ] {
        assert!(parse_run_query_payload(malformed, PREFIX).is_err());
    }
}

#[test]
fn firestore_cursor_migration_scans_all_cursor_shapes_by_document_name() {
    let current = encoded_note(1)["document"].clone();
    let mut legacy = encoded_note(2)["document"].clone();
    legacy["fields"]["_synced_at"] = json!({"stringValue": SYNCED_AT});
    let mut missing = encoded_note(3)["document"].clone();
    missing["fields"]
        .as_object_mut()
        .unwrap()
        .remove("_synced_at");
    let mut future = encoded_note(4)["document"].clone();
    future["fields"]["_synced_at"] = json!({"timestampValue": "2099-01-01T00:00:00Z"});

    let page = parse_list_documents_payload(json!({
        "documents": [current.clone(), legacy.clone(), missing.clone(), future.clone()],
        "nextPageToken": "opaque-token"
    }))
    .unwrap();
    assert_eq!(page.documents.len(), 4);
    assert_eq!(page.next_page_token.as_deref(), Some("opaque-token"));
    assert!(
        !scan_firestore_document(&current, 0, PREFIX)
            .unwrap()
            .needs_server_timestamp
    );
    assert!(
        scan_firestore_document(&legacy, 1, PREFIX)
            .unwrap()
            .needs_server_timestamp
    );
    assert!(
        scan_firestore_document(&missing, 2, PREFIX)
            .unwrap()
            .needs_server_timestamp
    );
    assert!(
        scan_firestore_document(&future, 3, PREFIX)
            .unwrap()
            .needs_server_timestamp
    );

    assert!(parse_list_documents_payload(json!({"nextPageToken": ""})).is_err());
    let mut broken = legacy;
    broken["fields"]["title"] = json!({"stringValue": "bad", "integerValue": "1"});
    assert!(scan_firestore_document(&broken, 3, PREFIX).is_err());
}

#[test]
fn firestore_cursor_migration_commit_is_transform_only() {
    let name = format!("{PREFIX}row_0001");
    let update_time = "2026-09-01T12:00:01.000000000Z";
    let body = firestore_synced_at_transform_body(&name, update_time);

    assert_eq!(
        body.pointer("/writes/0/transform/document"),
        Some(&json!(name))
    );
    assert_eq!(
        body.pointer("/writes/0/transform/fieldTransforms/0/setToServerValue"),
        Some(&json!("REQUEST_TIME"))
    );
    assert!(body.pointer("/writes/0/update").is_none());
    assert!(body.pointer("/writes/0/updateTransforms").is_none());
    assert_eq!(
        body.pointer("/writes/0/currentDocument/updateTime"),
        Some(&json!(update_time))
    );
    assert!(!body.to_string().contains("title"));
}

#[test]
fn incremental_pull_requests_migration_for_legacy_string_cursor() {
    let mut legacy = encoded_note(1);
    legacy["document"]["fields"]["_synced_at"] = json!({"stringValue": SYNCED_AT});

    let error = parse_run_query_payload(json!([legacy]), PREFIX).unwrap_err();
    assert!(error.contains(FIRESTORE_CURSOR_MIGRATION_REQUIRED));

    let mut future = encoded_note(2);
    future["document"]["fields"]["_synced_at"] =
        json!({"timestampValue": "2099-01-01T00:00:00Z"});
    let error = parse_run_query_payload(json!([future]), PREFIX).unwrap_err();
    assert!(error.contains(FIRESTORE_CURSOR_MIGRATION_REQUIRED));
}

#[test]
fn firestore_same_timestamp_501_documents_resume_without_gap() {
    let mut conn = sync_conn();
    let initial = load_firestore_cursor(&conn).unwrap();
    let documents = (1..=501)
        .map(|index| firestore_note(index, index as i64, Some("same time"), SYNCED_AT))
        .collect::<Vec<_>>();

    let (first_totals, first_cursor, first_page_cursor) =
        apply_firestore_page(&mut conn, &initial, &initial, &documents[..500], "local").unwrap();
    assert_eq!(first_totals.get("notes").and_then(Value::as_u64), Some(500));
    assert_eq!(
        first_cursor.document_name.as_deref(),
        Some(format!("{PREFIX}row_0500").as_str())
    );

    let (second_totals, second_cursor, _) = apply_firestore_page(
        &mut conn,
        &first_cursor,
        &first_page_cursor,
        &documents[500..],
        "local",
    )
    .unwrap();
    assert_eq!(second_totals.get("notes").and_then(Value::as_u64), Some(1));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        501
    );
    assert_eq!(load_firestore_cursor(&conn).unwrap(), second_cursor);
}

#[test]
fn firestore_valid_broken_valid_page_rolls_back_and_retries() {
    let mut conn = sync_conn();
    let initial = load_firestore_cursor(&conn).unwrap();
    let broken = vec![
        firestore_note(1, 1, Some("first"), SYNCED_AT),
        firestore_note(2, 2, None, SYNCED_AT),
        firestore_note(3, 3, Some("third"), SYNCED_AT),
    ];

    assert!(apply_firestore_page(&mut conn, &initial, &initial, &broken, "local").is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(load_firestore_cursor(&conn).unwrap(), initial);

    let corrected = vec![
        firestore_note(1, 1, Some("first"), SYNCED_AT),
        firestore_note(2, 2, Some("fixed"), SYNCED_AT),
        firestore_note(3, 3, Some("third"), SYNCED_AT),
    ];
    let (_, cursor, _) =
        apply_firestore_page(&mut conn, &initial, &initial, &corrected, "local").unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(load_firestore_cursor(&conn).unwrap(), cursor);
}

#[test]
fn firestore_cursor_write_failure_rolls_back_applied_rows() {
    let mut conn = sync_conn();
    conn.execute_batch(
        "CREATE TRIGGER reject_firestore_cursor
         BEFORE INSERT ON app_settings
         WHEN NEW.key = 'cloud_owner_v2_pull_name'
         BEGIN
             SELECT RAISE(ABORT, 'synthetic cursor failure');
         END;",
    )
    .unwrap();
    let initial = load_firestore_cursor(&conn).unwrap();

    assert!(apply_firestore_page(
        &mut conn,
        &initial,
        &initial,
        &[firestore_note(1, 1, Some("row"), SYNCED_AT)],
        "local"
    )
    .is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(load_firestore_cursor(&conn).unwrap(), initial);
}

#[test]
fn legacy_timestamp_only_firestore_cursor_replays_from_epoch() {
    let conn = sync_conn();
    set_setting_checked(&conn, FIRESTORE_PULL_TS_KEY, SYNCED_AT).unwrap();

    assert_eq!(
        load_firestore_cursor(&conn).unwrap(),
        FirestoreCursor {
            synced_at: canonical_sync_timestamp(EPOCH_TS, "test").unwrap(),
            document_name: None
        }
    );
}

#[test]
fn firestore_overlap_replays_same_server_millisecond_without_cursor_regression() {
    let mut conn = sync_conn();
    let initial = load_firestore_cursor(&conn).unwrap();
    let original = firestore_note(1, 1, Some("original"), SYNCED_AT);
    let (_, persisted, _) =
        apply_firestore_page(&mut conn, &initial, &initial, &[original], "local").unwrap();

    let overlap = firestore_overlap_cursor(&persisted).unwrap();
    assert_eq!(overlap.document_name, None);
    assert!(overlap.synced_at < persisted.synced_at);

    let mut replay = firestore_note(1, 1, Some("same-ms replay"), SYNCED_AT);
    let logical_update = "2026-09-01T12:00:00.001Z";
    replay.fields.insert("_updated_at".into(), json!(logical_update));
    replay.fields.insert("updated_at".into(), json!(logical_update));
    replay.update_time =
        canonical_sync_timestamp("2026-09-01T12:00:00.000001Z", "test").unwrap();
    let (_, next_persisted, next_page) = apply_firestore_page(
        &mut conn,
        &persisted,
        &overlap,
        &[replay],
        "local",
    )
    .unwrap();

    assert_eq!(next_persisted, persisted);
    assert_eq!(next_page, persisted);
    assert_eq!(load_firestore_cursor(&conn).unwrap(), persisted);
    assert_eq!(
        conn.query_row("SELECT title FROM notes WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap(),
        "same-ms replay"
    );
}

#[test]
fn firestore_overlap_clamps_at_epoch() {
    let epoch = FirestoreCursor {
        synced_at: canonical_sync_timestamp(EPOCH_TS, "test").unwrap(),
        document_name: Some(format!("{PREFIX}row_0001")),
    };
    assert_eq!(
        firestore_overlap_cursor(&epoch).unwrap(),
        FirestoreCursor {
            synced_at: canonical_sync_timestamp(EPOCH_TS, "test").unwrap(),
            document_name: None,
        }
    );
}

#[test]
fn outbound_tuple_cursors_cover_501_equal_timestamp_integer_and_text_ids() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE integer_rows (id INTEGER PRIMARY KEY, updated_at TEXT NOT NULL);
         CREATE TABLE text_rows (id TEXT PRIMARY KEY, updated_at TEXT NOT NULL);",
    )
    .unwrap();
    for index in 1..=501 {
        conn.execute(
            "INSERT INTO integer_rows(id,updated_at) VALUES(?1,?2)",
            rusqlite::params![index as i64, SYNCED_AT],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO text_rows(id,updated_at) VALUES(?1,?2)",
            rusqlite::params![format!("id_{index:04}"), SYNCED_AT],
        )
        .unwrap();
    }

    for table in ["integer_rows", "text_rows"] {
        let initial = RowCursor {
            timestamp: EPOCH_TS.into(),
            id: None,
        };
        let first = dirty_rows_after(&conn, table, &initial, 500).unwrap();
        let cursor = RowCursor {
            timestamp: first.last().unwrap().1.clone(),
            id: Some(first.last().unwrap().0.clone()),
        };
        let second = dirty_rows_after(&conn, table, &cursor, 500).unwrap();
        assert_eq!(first.len(), 500, "{table}");
        assert_eq!(second.len(), 1, "{table}");
    }
}

#[test]
fn outbound_tombstone_tuple_cursor_covers_501_equal_timestamps() {
    let conn = sync_conn();
    for index in 1..=501 {
        conn.execute(
            "INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES('notes',?1,?2)",
            rusqlite::params![format!("{index:04}"), SYNCED_AT],
        )
        .unwrap();
    }
    let initial = TombstoneCursor {
        timestamp: EPOCH_TS.into(),
        table: None,
        row_id: None,
    };
    let first = dirty_tombstones_after(&conn, &initial, 500).unwrap();
    let last = first.last().unwrap();
    let cursor = TombstoneCursor {
        timestamp: last.2.clone(),
        table: Some(last.0.clone()),
        row_id: Some(last.1.clone()),
    };
    let second = dirty_tombstones_after(&conn, &cursor, 500).unwrap();

    assert_eq!(first.len(), 500);
    assert_eq!(second.len(), 1);
}

#[test]
fn legacy_timestamp_only_outbound_cursors_replay_equal_timestamp_rows() {
    let conn = sync_conn();
    conn.execute_batch(
        "CREATE TABLE integer_rows (id INTEGER PRIMARY KEY, updated_at TEXT NOT NULL);",
    )
    .unwrap();
    for index in 1..=501 {
        conn.execute(
            "INSERT INTO integer_rows(id,updated_at) VALUES(?1,?2)",
            rusqlite::params![index as i64, SYNCED_AT],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sync_tombstones(table_name,row_id,deleted_at)
             VALUES('notes',?1,?2)",
            rusqlite::params![format!("{index:04}"), SYNCED_AT],
        )
        .unwrap();
    }

    set_setting_checked(&conn, "legacy_rows", SYNCED_AT).unwrap();
    let row_cursor = load_row_cursor(&conn, "legacy_rows").unwrap();
    assert_eq!(row_cursor.id, None);
    let first_rows = dirty_rows_after(&conn, "integer_rows", &row_cursor, 500).unwrap();
    let last_row = first_rows.last().unwrap();
    save_row_cursor(
        &conn,
        "legacy_rows",
        &RowCursor {
            timestamp: last_row.1.clone(),
            id: Some(last_row.0.clone()),
        },
    )
    .unwrap();
    assert_eq!(
        dirty_rows_after(
            &conn,
            "integer_rows",
            &load_row_cursor(&conn, "legacy_rows").unwrap(),
            500,
        )
        .unwrap()
        .len(),
        1
    );

    set_setting_checked(&conn, "legacy_tombstones", SYNCED_AT).unwrap();
    let tombstone_cursor = load_tombstone_cursor(&conn, "legacy_tombstones").unwrap();
    assert_eq!(tombstone_cursor.table, None);
    let first_tombstones = dirty_tombstones_after(&conn, &tombstone_cursor, 500).unwrap();
    let last_tombstone = first_tombstones.last().unwrap();
    save_tombstone_cursor(
        &conn,
        "legacy_tombstones",
        &TombstoneCursor {
            timestamp: last_tombstone.2.clone(),
            table: Some(last_tombstone.0.clone()),
            row_id: Some(last_tombstone.1.clone()),
        },
    )
    .unwrap();
    assert_eq!(
        dirty_tombstones_after(
            &conn,
            &load_tombstone_cursor(&conn, "legacy_tombstones").unwrap(),
            500,
        )
        .unwrap()
        .len(),
        1
    );
}

#[test]
fn outbound_cursor_does_not_lose_newer_submillisecond_lower_id() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE rows (id INTEGER PRIMARY KEY, updated_at TEXT NOT NULL);
         INSERT INTO rows VALUES(2,'2026-09-01T12:00:00.000100000Z');
         INSERT INTO rows VALUES(1,'2026-09-01T12:00:00.000400000Z');",
    )
    .unwrap();
    let dirty = dirty_rows_after(
        &conn,
        "rows",
        &RowCursor {
            timestamp: "2026-09-01T12:00:00.000100000Z".into(),
            id: Some(SqlValue::Integer(2)),
        },
        500,
    )
    .unwrap();

    assert_eq!(
        dirty,
        vec![(
            SqlValue::Integer(1),
            "2026-09-01T12:00:00.000400000Z".into()
        )]
    );
}

#[test]
fn firestore_text_document_ids_are_opaque_and_deterministic() {
    let unsafe_id = SqlValue::Text("notes/with ? and # fragments".into());
    let row = json!({"id": "notes/with ? and # fragments"});
    let first = data_doc_id("health_log", &unsafe_id, &row, "device-a").unwrap();
    let second = data_doc_id("health_log", &unsafe_id, &row, "device-a").unwrap();

    assert_eq!(first, second);
    assert!(first.starts_with("row_"));
    assert!(!first.contains('/'));
    assert!(!first.contains('?'));
    assert!(!first.contains('#'));
}

#[test]
fn firestore_event_category_document_identity_is_name_not_local_id() {
    let work = data_doc_id(
        "event_categories",
        &SqlValue::Integer(1),
        &json!({"id": 1, "name": "work"}),
        "device-a",
    )
    .unwrap();
    let same_work_other_local_id = data_doc_id(
        "event_categories",
        &SqlValue::Integer(99),
        &json!({"id": 99, "name": "work"}),
        "device-a",
    )
    .unwrap();
    let home = data_doc_id(
        "event_categories",
        &SqlValue::Integer(1),
        &json!({"id": 1, "name": "home"}),
        "device-a",
    )
    .unwrap();

    assert_eq!(work, same_work_other_local_id);
    assert_ne!(work, home);
    assert_ne!(
        work,
        data_doc_id(
            "event_categories",
            &SqlValue::Integer(1),
            &json!({"id": 1, "name": "work"}),
            "device-b",
        )
        .unwrap()
    );
    assert_ne!(
        tombstone_doc_id("notes", "1", "device-a"),
        tombstone_doc_id("notes", "1", "device-b")
    );
}

#[test]
fn event_category_identity_migration_resets_push_and_pull_once() {
    let conn = sync_conn();
    for (key, value) in [
        ("cloud_owner_v2_push_event_categories", SYNCED_AT),
        ("cloud_owner_v2_push_event_categories_id", "i:7"),
        (FIRESTORE_PULL_TS_KEY, SYNCED_AT),
        (FIRESTORE_PULL_NAME_KEY, "projects/test/documents/old"),
    ] {
        set_setting_checked(&conn, key, value).unwrap();
    }

    assert!(prepare_firestore_cursor_v3_replay(&conn, "scope-a").unwrap());
    for key in [
        "cloud_owner_v2_push_event_categories",
        "cloud_owner_v2_push_event_categories_id",
        FIRESTORE_PULL_TS_KEY,
        FIRESTORE_PULL_NAME_KEY,
    ] {
        assert_eq!(get_setting_checked(&conn, key).unwrap(), None, "{key}");
    }
    set_setting_checked(&conn, "cloud_owner_v2_push_event_categories", "new-cursor").unwrap();
    assert!(!prepare_firestore_cursor_v3_replay(&conn, "scope-a").unwrap());
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_v2_push_event_categories")
            .unwrap()
            .as_deref(),
        Some("new-cursor")
    );

    assert!(prepare_firestore_cursor_v3_replay(&conn, "scope-b").unwrap());
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_v2_push_event_categories").unwrap(),
        None
    );
    assert_eq!(
        get_setting_checked(&conn, FIRESTORE_SCOPE_KEY)
            .unwrap()
            .as_deref(),
        Some("scope-b")
    );
}

#[test]
fn firestore_cursor_generation_reset_is_atomic_on_marker_failure() {
    let conn = sync_conn();
    set_setting_checked(&conn, "cloud_owner_v2_push_notes", "2026-09-01T12:00:00Z").unwrap();
    conn.execute_batch(&format!(
        "CREATE TRIGGER reject_firestore_generation_marker
         BEFORE INSERT ON app_settings
         WHEN NEW.key = '{FIRESTORE_CURSOR_V3_MARKER}'
         BEGIN
             SELECT RAISE(ABORT, 'synthetic marker failure');
         END;"
    ))
    .unwrap();

    assert!(prepare_firestore_cursor_v3_replay(&conn, "scope-a").is_err());
    assert_eq!(
        get_setting_checked(&conn, "cloud_owner_v2_push_notes")
            .unwrap()
            .as_deref(),
        Some("2026-09-01T12:00:00Z")
    );
    assert_eq!(
        get_setting_checked(&conn, FIRESTORE_CURSOR_V3_MARKER).unwrap(),
        None
    );
}

#[test]
fn equal_timestamp_rows_converge_by_writer_id() {
    let a = sync_conn();
    let b = sync_conn();
    set_setting_checked(&a, "device_id", "device-a").unwrap();
    set_setting_checked(&b, "device_id", "device-z").unwrap();
    crate::db::migrate_sync_meta(&a).unwrap();
    crate::db::migrate_sync_meta(&b).unwrap();
    let canonical_timestamp = canonical_sync_timestamp(SYNCED_AT, "test").unwrap();
    for (conn, title) in [(&a, "from-a"), (&b, "from-z")] {
        let local_writer = device_id(conn).unwrap();
        with_remote_sync_apply(conn, || {
            conn.execute(
                "INSERT INTO notes(id,title,updated_at) VALUES(1,?1,?2)",
                rusqlite::params![title, &canonical_timestamp],
            )
            .map_err(|error| error.to_string())?;
            record_row_version(
                conn,
                "notes",
                "1",
                &canonical_timestamp,
                &local_writer,
            )
        })
        .unwrap();
    }
    let from_a = serde_json::from_value(json!({
        "id": 1,
        "title": "from-a",
        "updated_at": SYNCED_AT,
        "_updated_at": SYNCED_AT,
        "_device_id": "device-a"
    }))
    .unwrap();
    let from_z = serde_json::from_value(json!({
        "id": 1,
        "title": "from-z",
        "updated_at": SYNCED_AT,
        "_updated_at": SYNCED_AT,
        "_device_id": "device-z"
    }))
    .unwrap();

    assert!(upsert_row_fail_closed(&a, "notes", &from_z).unwrap());
    assert!(!upsert_row_fail_closed(&b, "notes", &from_a).unwrap());
    assert!(!upsert_row_fail_closed(&a, "notes", &from_a).unwrap());
    for conn in [&a, &b] {
        assert_eq!(
            conn.query_row("SELECT title FROM notes WHERE id=1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "from-z"
        );
    }
    assert_eq!(
        a.query_row("SELECT updated_at FROM notes WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap(),
        canonical_timestamp
    );
    assert_eq!(
        a.query_row(
            "SELECT device_id FROM sync_row_versions
             WHERE table_name='notes' AND row_id='1'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "device-z"
    );
}

#[test]
fn global_hlc_survives_clock_rollback_across_rows_updates_and_tombstones() {
    let conn = sync_conn();
    crate::db::observe_sync_hlc_timestamp(&conn, "2099-01-01T00:00:00Z").unwrap();

    conn.execute(
        "INSERT INTO notes(id,title,updated_at) VALUES(1,'one','2020-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    let first: String = conn
        .query_row("SELECT updated_at FROM notes WHERE id=1", [], |row| row.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO notes(id,title,updated_at) VALUES(2,'two','2020-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    let second: String = conn
        .query_row("SELECT updated_at FROM notes WHERE id=2", [], |row| row.get(0))
        .unwrap();
    conn.execute(
        "UPDATE notes SET title='one updated',updated_at='2020-01-01T00:00:00Z' WHERE id=1",
        [],
    )
    .unwrap();
    let third: String = conn
        .query_row("SELECT updated_at FROM notes WHERE id=1", [], |row| row.get(0))
        .unwrap();
    conn.execute("DELETE FROM notes WHERE id=2", []).unwrap();
    let tombstone: String = conn
        .query_row(
            "SELECT deleted_at FROM sync_tombstones WHERE table_name='notes' AND row_id='2'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(first > "2099-01-01T00:00:00.000Z".to_string());
    assert!(second > first);
    assert!(third > second);
    assert!(tombstone > third);
}

#[test]
fn remote_apply_preserves_timestamp_observes_hlc_and_rolls_back_atomically() {
    let conn = sync_conn();
    let remote_timestamp = "2099-01-01T00:00:00.000000123Z";
    let remote = serde_json::from_value(json!({
        "id": 1,
        "title": "remote",
        "updated_at": remote_timestamp,
        "_updated_at": remote_timestamp,
        "_device_id": "peer-z"
    }))
    .unwrap();
    assert!(upsert_row_fail_closed(&conn, "notes", &remote).unwrap());
    assert_eq!(
        conn.query_row("SELECT updated_at FROM notes WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap(),
        canonical_sync_timestamp(remote_timestamp, "test").unwrap()
    );

    conn.execute(
        "UPDATE notes SET title='local after remote',updated_at='2020-01-01T00:00:00Z' WHERE id=1",
        [],
    )
    .unwrap();
    let local_timestamp: String = conn
        .query_row("SELECT updated_at FROM notes WHERE id=1", [], |row| row.get(0))
        .unwrap();
    assert!(
        canonical_sync_timestamp(&local_timestamp, "test").unwrap()
            > canonical_sync_timestamp(remote_timestamp, "test").unwrap()
    );

    conn.execute_batch(
        "CREATE TRIGGER reject_remote_hlc_observe
         BEFORE UPDATE ON sync_hlc_state
         BEGIN SELECT RAISE(ABORT,'synthetic HLC observe failure'); END;",
    )
    .unwrap();
    let failed = serde_json::from_value(json!({
        "id": 2,
        "title": "must roll back",
        "updated_at": "2099-01-02T00:00:00Z",
        "_updated_at": "2099-01-02T00:00:00Z",
        "_device_id": "peer-z"
    }))
    .unwrap();
    assert!(upsert_row_fail_closed(&conn, "notes", &failed).is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM notes WHERE id=2", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT remote_apply,stamp_depth FROM sync_apply_context WHERE singleton=1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        )
        .unwrap(),
        (0, 0)
    );
}

#[test]
fn hlc_generation_migration_is_atomic_and_resets_exact_outbound_cursors() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
         CREATE TABLE notes(id INTEGER PRIMARY KEY,title TEXT NOT NULL,updated_at TEXT NOT NULL);
         CREATE TABLE sync_tombstones(
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             table_name TEXT NOT NULL,row_id TEXT NOT NULL,deleted_at TEXT NOT NULL,
             UNIQUE(table_name,row_id));
         CREATE TABLE sync_row_versions(
             table_name TEXT NOT NULL,row_id TEXT NOT NULL,updated_at TEXT NOT NULL,
             device_id TEXT NOT NULL,PRIMARY KEY(table_name,row_id));
         INSERT INTO app_settings VALUES('device_id','local-a');
         INSERT INTO app_settings VALUES('cloud_owner_v2_push_notes','2026-09-01T12:00:00Z');
         INSERT INTO app_settings VALUES('cloud_owner_v2_push_notes_id','t:2099-01-01T00:00:00Z');
         INSERT INTO app_settings VALUES('cloud_owner_gh_push_notes','2026-09-01T12:00:00Z');
         CREATE TRIGGER reject_hlc_generation_marker
         BEFORE INSERT ON app_settings
         WHEN NEW.key='{}'
         BEGIN SELECT RAISE(ABORT,'synthetic marker failure'); END;",
        crate::db::SYNC_HLC_GENERATION_MARKER
    ))
    .unwrap();

    assert!(crate::db::migrate_sync_meta(&conn).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='cloud_owner_v2_push_notes'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "2026-09-01T12:00:00Z"
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sync_hlc_state'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );

    conn.execute_batch("DROP TRIGGER reject_hlc_generation_marker;")
        .unwrap();
    crate::db::migrate_sync_meta(&conn).unwrap();
    for key in [
        "cloud_owner_v2_push_notes",
        "cloud_owner_v2_push_notes_id",
        "cloud_owner_gh_push_notes",
    ] {
        assert_eq!(get_setting_checked(&conn, key).unwrap(), None);
    }
    assert_eq!(
        get_setting_checked(&conn, crate::db::SYNC_HLC_GENERATION_MARKER)
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let last_millis: i64 = conn
        .query_row(
            "SELECT last_millis FROM sync_hlc_state WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let poison_millis = chrono::DateTime::parse_from_rfc3339("2099-01-01T00:00:00Z")
        .unwrap()
        .timestamp_millis();
    assert!(last_millis < poison_millis);
}

#[test]
fn hlc_internal_stamps_preserve_fts_with_recursive_triggers_off_and_on() {
    for recursive in [0, 1] {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "PRAGMA recursive_triggers={recursive};
             CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_settings VALUES('device_id','local-a');
             CREATE TABLE facts(
                 id INTEGER PRIMARY KEY,category TEXT NOT NULL,key TEXT NOT NULL,
                 value TEXT NOT NULL,source TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);
             CREATE VIRTUAL TABLE facts_fts USING fts5(
                 category,key,value,content='facts',content_rowid='id');
             CREATE TRIGGER facts_ai AFTER INSERT ON facts BEGIN
                 INSERT INTO facts_fts(rowid,category,key,value)
                 VALUES(new.id,new.category,new.key,new.value); END;
             CREATE TRIGGER facts_au AFTER UPDATE ON facts BEGIN
                 INSERT INTO facts_fts(facts_fts,rowid,category,key,value)
                 VALUES('delete',old.id,old.category,old.key,old.value);
                 INSERT INTO facts_fts(rowid,category,key,value)
                 VALUES(new.id,new.category,new.key,new.value); END;
             CREATE TABLE conversations(
                 id INTEGER PRIMARY KEY,started_at TEXT NOT NULL,ended_at TEXT,
                 summary TEXT,message_count INTEGER,messages TEXT NOT NULL,updated_at TEXT NOT NULL);
             CREATE VIRTUAL TABLE conversations_fts USING fts5(
                 summary,messages,content='conversations',content_rowid='id');
             CREATE TRIGGER conv_ai AFTER INSERT ON conversations BEGIN
                 INSERT INTO conversations_fts(rowid,summary,messages)
                 VALUES(new.id,COALESCE(new.summary,''),new.messages); END;
             CREATE TRIGGER conv_au AFTER UPDATE ON conversations BEGIN
                 INSERT INTO conversations_fts(conversations_fts,rowid,summary,messages)
                 VALUES('delete',old.id,COALESCE(old.summary,''),old.messages);
                 INSERT INTO conversations_fts(rowid,summary,messages)
                 VALUES(new.id,COALESCE(new.summary,''),new.messages); END;
             CREATE TABLE notes(
                 id INTEGER PRIMARY KEY,title TEXT NOT NULL,content TEXT NOT NULL,tags TEXT NOT NULL,
                 created_at TEXT NOT NULL,updated_at TEXT NOT NULL);
             CREATE VIRTUAL TABLE notes_fts USING fts5(
                 title,content,tags,content='notes',content_rowid='id');
             CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
                 INSERT INTO notes_fts(rowid,title,content,tags)
                 VALUES(new.id,new.title,new.content,new.tags); END;
             CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
                 INSERT INTO notes_fts(notes_fts,rowid,title,content,tags)
                 VALUES('delete',old.id,old.title,old.content,old.tags);
                 INSERT INTO notes_fts(rowid,title,content,tags)
                 VALUES(new.id,new.title,new.content,new.tags); END;"
        ))
        .unwrap();
        crate::db::migrate_sync_meta(&conn).unwrap();

        conn.execute_batch(
            "INSERT INTO facts(id,category,key,value,source,created_at,updated_at)
             VALUES(1,'alpha','fact','value','user','2020-01-01','2020-01-01');
             INSERT INTO conversations(id,started_at,summary,message_count,messages,updated_at)
             VALUES(1,'2020-01-01','alpha',1,'alpha message','2020-01-01');
             INSERT INTO notes(id,title,content,tags,created_at,updated_at)
             VALUES(1,'alpha','note body','tag','2020-01-01','2020-01-01');",
        )
        .unwrap();
        for (table, query) in [
            ("facts_fts", "alpha"),
            ("conversations_fts", "alpha"),
            ("notes_fts", "alpha"),
        ] {
            assert_eq!(
                conn.query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {table} MATCH ?1"),
                    [query],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                1,
                "recursive_triggers={recursive}, table={table}"
            );
        }

        conn.execute_batch(
            "UPDATE facts SET category='beta',updated_at='2020-01-01' WHERE id=1;
             UPDATE conversations SET summary='beta',updated_at='2020-01-01' WHERE id=1;
             UPDATE notes SET title='beta',updated_at='2020-01-01' WHERE id=1;",
        )
        .unwrap();
        for table in ["facts_fts", "conversations_fts", "notes_fts"] {
            assert_eq!(
                conn.query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {table} MATCH 'beta'"),
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                1,
                "recursive_triggers={recursive}, table={table}"
            );
        }
    }
}

#[test]
fn sync_schema_preflight_accepts_complete_subset_and_rejects_missing_trigger() {
    let conn = sync_conn();
    crate::db::migrate_sync_meta(&conn).unwrap();
    crate::db::verify_sync_schema_for_tables(&conn, &["notes"]).unwrap();

    conn.execute_batch("DROP TRIGGER notes_tombstone;").unwrap();
    let error = crate::db::verify_sync_schema_for_tables(&conn, &["notes"]).unwrap_err();
    assert!(error.contains("notes_tombstone"));
}

#[test]
fn sync_schema_preflight_rejects_incomplete_metadata_table() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
         CREATE TABLE sync_tombstones(
             table_name TEXT NOT NULL,
             row_id TEXT NOT NULL,
             deleted_at TEXT NOT NULL
         );
         CREATE TABLE sync_row_versions(table_name TEXT NOT NULL);
         CREATE TABLE notes(id INTEGER PRIMARY KEY,updated_at TEXT NOT NULL);",
    )
    .unwrap();

    let error = crate::db::verify_sync_schema_for_tables(&conn, &["notes"]).unwrap_err();
    assert!(error.contains("sync_row_versions.row_id"));
}

#[test]
fn sync_schema_preflight_rejects_wrong_hlc_singleton_and_active_context() {
    let conn = sync_conn();
    conn.execute("UPDATE sync_apply_context SET remote_apply=1", [])
        .unwrap();
    assert!(crate::db::verify_sync_schema_for_tables(&conn, &["notes"])
        .unwrap_err()
        .contains("not idle"));

    conn.execute("UPDATE sync_apply_context SET remote_apply=0", [])
        .unwrap();
    conn.execute_batch(
        "PRAGMA ignore_check_constraints=ON;
         DELETE FROM sync_hlc_state;
         INSERT INTO sync_hlc_state(singleton,last_millis) VALUES(2,1);",
    )
    .unwrap();
    assert!(crate::db::verify_sync_schema_for_tables(&conn, &["notes"])
        .unwrap_err()
        .contains("one non-negative clock row"));
}

#[test]
fn sync_schema_preflight_rejects_legacy_event_category_rename_trigger() {
    let conn = sync_conn();
    conn.execute_batch(
        "CREATE TABLE event_categories(
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL UNIQUE,
             color TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );",
    )
    .unwrap();
    crate::db::migrate_sync_meta(&conn).unwrap();
    conn.execute_batch(
        "DROP TRIGGER event_categories_name_tombstone;
         CREATE TRIGGER event_categories_name_tombstone
         AFTER UPDATE OF name ON event_categories
         BEGIN SELECT 1; END;",
    )
    .unwrap();

    assert!(crate::db::verify_sync_schema_for_tables(&conn, &["event_categories"])
        .unwrap_err()
        .contains("not HLC-bound"));
}

#[test]
fn numeric_looking_text_tombstone_uses_text_primary_key() {
    let conn = sync_conn();
    conn.execute_batch(
        "CREATE TABLE health_log (
             id TEXT PRIMARY KEY,
             value TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         INSERT INTO health_log(id,value,updated_at)
         VALUES('123','local','2026-09-01T11:00:00Z');",
    )
    .unwrap();

    assert!(apply_tombstone_lww(&conn, "health_log", &json!("123"), SYNCED_AT).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM health_log", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn event_category_tombstones_use_semantic_name_and_ignore_legacy_ids() {
    let conn = sync_conn();
    conn.execute_batch(
        "CREATE TABLE event_categories (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL UNIQUE,
             color TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         INSERT INTO event_categories(id,name,color,updated_at)
         VALUES(2,'work','#111111','2026-09-01T11:00:00Z');
         INSERT INTO event_categories(id,name,color,updated_at)
         VALUES(3,'home','#222222','2026-09-01T11:00:00Z');",
    )
    .unwrap();

    assert!(!apply_tombstone_lww(&conn, "event_categories", &json!(2), SYNCED_AT).unwrap());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM event_categories", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        2
    );

    assert!(
        apply_tombstone_lww(&conn, "event_categories", &json!("name:work"), SYNCED_AT).unwrap()
    );
    assert_eq!(
        conn.query_row(
            "SELECT GROUP_CONCAT(name) FROM event_categories",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "home"
    );

    let stale = serde_json::from_value(json!({
        "_table": "event_categories",
        "_device_id": "peer-z",
        "id": 99,
        "name": "work",
        "color": "#333333",
        "updated_at": "2026-09-01T11:30:00Z",
        "_updated_at": "2026-09-01T11:30:00Z"
    }))
    .unwrap();
    assert!(!upsert_row_fail_closed(&conn, "event_categories", &stale).unwrap());
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM event_categories WHERE name='work'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn event_category_rename_emits_old_semantic_name_tombstone() {
    let conn = sync_conn();
    conn.execute_batch(
        "CREATE TABLE event_categories (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL UNIQUE,
             color TEXT NOT NULL,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         INSERT INTO event_categories(id,name,color,created_at,updated_at)
         VALUES(1,'work','#111111','2026-09-01T11:00:00Z','2026-09-01T11:00:00Z');",
    )
    .unwrap();
    crate::db::migrate_sync_meta(&conn).unwrap();

    conn.execute(
        "UPDATE event_categories SET name='deep work', updated_at=?1 WHERE id=1",
        [SYNCED_AT],
    )
    .unwrap();

    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sync_tombstones
             WHERE table_name='event_categories' AND row_id='name:work'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}
