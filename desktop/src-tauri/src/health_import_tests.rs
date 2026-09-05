use super::*;

#[test]
fn exercise_cleanup_preserves_separate_walks_at_different_times() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE health_log(id TEXT PRIMARY KEY,date TEXT,type TEXT,value REAL,notes TEXT,start_time TEXT);
        CREATE TABLE timeline_blocks(id TEXT PRIMARY KEY,date TEXT,start_time TEXT,end_time TEXT,type_id TEXT,source TEXT);
        INSERT INTO health_log VALUES('a','2026-09-05','exercise',20,'walking','08:00'),
          ('b','2026-09-05','exercise',20,'walking','19:00'),
          ('c','2026-09-05','exercise',20,'walking','19:00');").unwrap();
    crate::db::migrate_dedup_health_exercise(&conn);
    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM health_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 2);
    let starts: String = conn.query_row("SELECT GROUP_CONCAT(start_time,',') FROM (SELECT start_time FROM health_log ORDER BY start_time)", [], |r| r.get(0)).unwrap();
    assert_eq!(starts, "08:00,19:00");
    crate::db::migrate_dedup_health_exercise(&conn);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM health_log", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::Mutex;

fn database() -> HanniDb {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE observed_mutations(revision INTEGER PRIMARY KEY AUTOINCREMENT);
        CREATE TABLE cloud_relay_control(id INTEGER PRIMARY KEY,applying INTEGER NOT NULL);
        INSERT INTO cloud_relay_control VALUES(1,0);
        CREATE TABLE cloud_relay_dirty(seq INTEGER PRIMARY KEY AUTOINCREMENT,table_name TEXT NOT NULL,
          row_id TEXT NOT NULL,UNIQUE(table_name,row_id));
        CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT,start_time TEXT,end_time TEXT,
          duration_minutes INTEGER,source TEXT,created_at TEXT,updated_at TEXT DEFAULT 'initial',UNIQUE(date,start_time,source));
        CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,session_id TEXT,start_time TEXT,end_time TEXT,
          stage TEXT,updated_at TEXT DEFAULT 'initial');
        CREATE TABLE health_log(id TEXT PRIMARY KEY,date TEXT,type TEXT,value REAL,unit TEXT,notes TEXT,
          start_time TEXT,created_at TEXT,updated_at TEXT DEFAULT 'initial');
        CREATE TABLE heart_rate_samples(id TEXT PRIMARY KEY,date TEXT,time TEXT,bpm INTEGER,
          updated_at TEXT DEFAULT 'initial');").unwrap();
    for (table, columns) in [
        (
            "sleep_sessions",
            "date,start_time,end_time,duration_minutes,source",
        ),
        ("sleep_stages", "session_id,start_time,end_time,stage"),
        ("health_log", "date,type,value,unit,notes,start_time"),
        ("heart_rate_samples", "date,time,bpm"),
    ] {
        // Same dirty-journal trigger SQL used by cloud_relay::initialize.
        for (action, reference) in [("INSERT", "NEW"), ("UPDATE", "NEW"), ("DELETE", "OLD")] {
            conn.execute_batch(&format!(
                "CREATE TRIGGER relay_{table}_{action} AFTER {action} ON {table}
              WHEN (SELECT applying FROM cloud_relay_control WHERE id=1)=0
              BEGIN INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id)
              VALUES('{table}',CAST({reference}.id AS TEXT)); END;"
            ))
            .unwrap();
        }
        // Observe every real write, as updated_at/relay triggers do. No import SQL is mocked.
        for (suffix, operation) in [
            ("insert", "INSERT".to_string()),
            ("update", format!("UPDATE OF {columns}")),
        ] {
            conn.execute_batch(&format!("CREATE TRIGGER observe_{table}_{suffix} AFTER {operation} ON {table}
              BEGIN INSERT INTO observed_mutations(revision) VALUES(NULL);
              UPDATE {table} SET updated_at='revision:' || last_insert_rowid() WHERE id=NEW.id; END;"))
                .unwrap();
        }
    }
    HanniDb {
        writer: Mutex::new(conn),
        reader: Mutex::new(Connection::open_in_memory().unwrap()),
    }
}

fn records() -> [Value; 4] {
    [
        json!({"record_id":"sleep-one","date":"2026-01-01","start_time":"23:00","end_time":"07:00",
          "start_iso":"2026-01-01T23:00:00Z","end_iso":"2026-01-02T07:00:00Z","duration_minutes":480,
          "stages":[{"start_time":"23:00","end_time":"07:00","stage":"deep"}]}),
        json!({"date":"2026-01-01","steps":123.0}),
        json!({"record_id":"exercise-one","date":"2026-01-01","start_time":"10:00",
          "duration_minutes":15.0,"type":"walking","title":"synthetic"}),
        json!({"record_id":"hr-one","sample_index":0,"date":"2026-01-01","time":"10:01","bpm":70}),
    ]
}

fn import(db: &HanniDb, records: &[Value; 4]) {
    import_sleep_sessions(db, &records[0..1]);
    import_steps(db, &records[1..2]);
    import_exercise(db, &records[2..3]);
    import_heart_rate(db, &records[3..4]);
}

fn snapshot(db: &HanniDb) -> (i64, Vec<String>) {
    let conn = db.conn();
    let writes = conn
        .query_row("SELECT COUNT(*) FROM observed_mutations", [], |r| r.get(0))
        .unwrap();
    let mut stamps = Vec::new();
    for table in [
        "sleep_sessions",
        "sleep_stages",
        "health_log",
        "heart_rate_samples",
    ] {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id || ':' || updated_at FROM {table} ORDER BY id"
            ))
            .unwrap();
        stamps.extend(
            stmt.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap),
        );
    }
    (writes, stamps)
}

#[test]
fn repeated_health_window_does_not_stamp_or_queue_unchanged_rows() {
    let db = database();
    let values = records();
    import(&db, &values);
    let first = snapshot(&db);
    assert_eq!(first.1.len(), 5);
    db.conn()
        .execute("DELETE FROM cloud_relay_dirty", [])
        .unwrap();
    import(&db, &values);
    import(&db, &values);
    assert_eq!(snapshot(&db), first);
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM cloud_relay_dirty", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn corrected_measurements_and_exercise_metadata_update_once_with_stable_ids() {
    let db = database();
    let mut values = records();
    import(&db, &values);
    let first = snapshot(&db);
    values[0]["end_time"] = json!("07:30");
    values[0]["end_iso"] = json!("2026-01-02T07:30:00Z");
    values[1]["steps"] = json!(456.0);
    // Duration is unchanged: metadata corrections alone still need delivery.
    values[2]["date"] = json!("2026-01-02");
    values[2]["start_time"] = json!("11:00");
    values[2]["title"] = json!("renamed");
    values[3]["time"] = json!("10:02");
    values[3]["bpm"] = json!(80);
    import(&db, &values);
    {
        let conn = db.conn();
        assert_eq!(
            conn.query_row(
                "SELECT duration_minutes FROM sleep_sessions WHERE id='sleep-one'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            510
        );
        assert_eq!(
            conn.query_row("SELECT value FROM health_log WHERE type='steps'", [], |r| r
                .get::<_, f64>(0))
                .unwrap(),
            456.0
        );
        assert_eq!(conn.query_row("SELECT date || ':' || start_time || ':' || notes FROM health_log WHERE id='health:exercise:exercise-one'", [], |r| r.get::<_,String>(0)).unwrap(), "2026-01-02:11:00:walking: renamed");
        assert_eq!(
            conn.query_row(
                "SELECT bpm FROM heart_rate_samples WHERE id='health:hr:hr-one:0'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            80
        );
        assert_eq!(
            conn.query_row(
                "SELECT time FROM heart_rate_samples WHERE id='health:hr:hr-one:0'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "10:02"
        );
    }
    let corrected = snapshot(&db);
    assert_eq!(corrected.0, first.0 + 4);
    assert_eq!(corrected.1.len(), first.1.len());
    db.conn()
        .execute("DELETE FROM cloud_relay_dirty", [])
        .unwrap();
    import(&db, &values);
    assert_eq!(snapshot(&db), corrected);
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM cloud_relay_dirty", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn corrected_sleep_date_and_start_keep_original_session_and_stage_ids() {
    let db = database();
    let mut values = records();
    import(&db, &values);
    let before = snapshot(&db);
    values[0]["date"] = json!("2026-01-02");
    values[0]["start_time"] = json!("22:30");
    values[0]["start_iso"] = json!("2026-01-02T22:30:00Z");
    values[0]["end_iso"] = json!("2026-01-03T07:00:00Z");
    import(&db, &values);
    assert_eq!(db.conn().query_row("SELECT date || ':' || start_time || ':' || duration_minutes FROM sleep_sessions WHERE id='sleep-one'", [], |r| r.get::<_,String>(0)).unwrap(), "2026-01-02:22:30:510");
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM sleep_sessions", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let after = snapshot(&db);
    assert_eq!(after.0, before.0 + 1);
    assert_eq!(after.1.len(), before.1.len());
    db.conn()
        .execute("DELETE FROM cloud_relay_dirty", [])
        .unwrap();
    import(&db, &values);
    assert_eq!(snapshot(&db), after);
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM cloud_relay_dirty", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn corrected_sleep_conflicting_with_another_session_leaves_both_and_stages_unchanged() {
    let db = database();
    let mut values = records();
    import(&db, &values);
    db.conn().execute("INSERT INTO sleep_sessions(id,date,start_time,end_time,duration_minutes,source) VALUES('sleep-other','2026-01-02','22:30','07:00',510,'health_connect')", []).unwrap();
    let before = snapshot(&db);
    db.conn()
        .execute("DELETE FROM cloud_relay_dirty", [])
        .unwrap();
    values[0]["date"] = json!("2026-01-02");
    values[0]["start_time"] = json!("22:30");
    values[0]["start_iso"] = json!("2026-01-02T22:30:00Z");
    values[0]["end_iso"] = json!("2026-01-03T07:00:00Z");
    values[0]["stages"] = json!([{"start_time":"22:30","end_time":"07:00","stage":"light"}]);
    import_sleep_sessions(&db, &values[0..1]);
    assert_eq!(snapshot(&db), before);
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM cloud_relay_dirty", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
