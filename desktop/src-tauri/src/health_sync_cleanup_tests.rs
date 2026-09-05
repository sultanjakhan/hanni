use super::migrate_health_sync_cleanup_v1;
use rusqlite::{params, Connection};

fn fixture() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // The repair also encounters older schemas without natural-key indexes.
    // Use real SQLite and execute the whole production migration, not a copy
    // of its DELETE predicate.
    conn.execute_batch(
        "PRAGMA foreign_keys=ON;
         CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT NOT NULL,
           start_time TEXT NOT NULL,end_time TEXT NOT NULL,duration_minutes INTEGER NOT NULL,
           source TEXT NOT NULL,notes TEXT NOT NULL DEFAULT '',quality_score INTEGER);
         CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,
           session_id TEXT REFERENCES sleep_sessions(id) ON DELETE CASCADE,
           start_time TEXT,end_time TEXT,stage TEXT);
         CREATE TABLE health_log(id TEXT PRIMARY KEY,type TEXT,date TEXT,start_time TEXT,
           value REAL,notes TEXT,updated_at TEXT);
         CREATE TABLE timeline_blocks(id INTEGER PRIMARY KEY,source TEXT,date TEXT,
           type_id INTEGER,start_time TEXT,notes TEXT);
         CREATE TABLE events(id INTEGER PRIMARY KEY,source TEXT,date TEXT,title TEXT,time TEXT);
         CREATE TABLE sync_tombstones(table_name TEXT,row_id TEXT,deleted_at TEXT,
           PRIMARY KEY(table_name,row_id));",
    )
    .unwrap();
    conn
}

fn sleep(conn: &Connection, id: &str, start: &str, end: &str, staged: bool) {
    conn.execute(
        "INSERT INTO sleep_sessions(id,date,start_time,end_time,duration_minutes,source)
         VALUES(?1,'2026-01-01',?2,?3,420,'health_connect')",
        params![id, start, end],
    )
    .unwrap();
    if staged {
        conn.execute(
            "INSERT INTO sleep_stages VALUES(?1,?2,?3,?4,'light')",
            params![format!("stage:{id}"), id, start, end],
        )
        .unwrap();
    }
}

fn exists(conn: &Connection, id: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sleep_sessions WHERE id=?1)",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

fn completed(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name='health_sync_cleanup_v1')",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn separate_same_date_sleep_and_incomplete_intervals_are_preserved() {
    for (start, end, other_start, other_end) in [
        ("13:00", "14:00", "00:00", "07:00"),
        ("00:00", "08:00", "00:00", "07:00"),
        ("", "", "", ""),
    ] {
        let conn = fixture();
        sleep(&conn, "unstaged", start, end, false);
        sleep(&conn, "staged", other_start, other_end, true);
        migrate_health_sync_cleanup_v1(&conn);
        assert!(
            completed(&conn),
            "migration must complete, not silently roll back"
        );
        assert!(
            exists(&conn, "unstaged"),
            "a same-date or incomplete interval is not a proven duplicate"
        );
        assert!(exists(&conn, "staged"));
    }
}

#[test]
fn equivalent_unstaged_legacy_duplicate_is_removed_without_touching_stages() {
    let conn = fixture();
    sleep(&conn, "unstaged", "00:00", "07:00", false);
    sleep(&conn, "staged", "00:00", "07:00", true);
    migrate_health_sync_cleanup_v1(&conn);
    assert!(completed(&conn));
    assert!(!exists(&conn, "unstaged"));
    assert!(exists(&conn, "staged"));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sleep_stages WHERE session_id='staged'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn equal_intervals_do_not_discard_distinct_notes_quality_or_duration() {
    for change in [
        "UPDATE sleep_sessions SET notes='retained annotation' WHERE id='unstaged'",
        "UPDATE sleep_sessions SET quality_score=7 WHERE id='unstaged'",
        "UPDATE sleep_sessions SET duration_minutes=415 WHERE id='unstaged'",
    ] {
        let conn = fixture();
        sleep(&conn, "unstaged", "00:00", "07:00", false);
        sleep(&conn, "staged", "00:00", "07:00", true);
        conn.execute(change, []).unwrap();
        migrate_health_sync_cleanup_v1(&conn);
        assert!(completed(&conn));
        assert!(
            exists(&conn, "unstaged"),
            "cleanup must not discard a distinct user-visible value"
        );
        assert!(exists(&conn, "staged"));
    }
}

fn tombstones(conn: &Connection) -> Vec<(String, String, String)> {
    conn.prepare(
        "SELECT table_name,row_id,deleted_at FROM sync_tombstones ORDER BY table_name,row_id",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

#[test]
fn pending_source_deletions_survive_cleanup_and_its_idempotent_retry() {
    let conn = fixture();
    for table in ["sleep_sessions", "sleep_stages", "health_log", "events"] {
        conn.execute("INSERT INTO sync_tombstones VALUES(?1,'deleted-while-peer-offline','2026-01-02T00:00:00Z')", [table]).unwrap();
    }
    let before = tombstones(&conn);
    migrate_health_sync_cleanup_v1(&conn);
    assert!(completed(&conn));
    assert_eq!(
        tombstones(&conn),
        before,
        "no provenance permits wholesale removal of pending deletions"
    );
    migrate_health_sync_cleanup_v1(&conn);
    assert_eq!(tombstones(&conn), before);
}

#[test]
fn existing_completion_marker_does_not_reapply_cleanup_to_new_rows() {
    let conn = fixture();
    migrate_health_sync_cleanup_v1(&conn);
    assert!(completed(&conn));
    sleep(&conn, "later-unstaged", "00:00", "07:00", false);
    sleep(&conn, "later-staged", "00:00", "07:00", true);
    migrate_health_sync_cleanup_v1(&conn);
    assert!(exists(&conn, "later-unstaged"));
    assert!(exists(&conn, "later-staged"));
}
