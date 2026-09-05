use super::*;
use rusqlite::OpenFlags;
use std::{sync::mpsc, time::Duration};

fn open_fixture(path: &str) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|_| "fixture_open_failed".into())
}
fn request(registry: &Mutex<Registry>, value: Value) -> Result<Value, Failure> {
    dispatch(registry, serde_json::from_value(value).unwrap(), open_fixture)
}
fn memory() -> (Mutex<Registry>, String) {
    let mut registry = Registry::default();
    let handle = registry.insert(Connection::open_in_memory().unwrap()).unwrap().to_string();
    (Mutex::new(registry), handle)
}
fn execute(registry: &Mutex<Registry>, handle: &str, sql: &str) -> Result<Value, Failure> {
    request(registry, json!({"op":"execute","handle":handle,"sql":sql,"args":[]}))
}
fn query(registry: &Mutex<Registry>, handle: &str, sql: &str) -> Value {
    request(registry, json!({"op":"query","handle":handle,"sql":sql,"args":[]})).unwrap()
}

#[test]
fn all_sql_types_round_trip_without_int64_or_utf8_loss() {
    let (registry, handle) = memory();
    execute(&registry, &handle, "CREATE TABLE fixture(a,b,c,d,e,f)").unwrap();
    let args = json!([
        {"t":"i","v":i64::MAX.to_string()}, {"t":"i","v":i64::MIN.to_string()},
        {"t":"f","v":"-0.125"}, {"t":"s","v":"Aλ😀\u{0000}\n"},
        {"t":"b","v":"AP8BgA=="}, {"t":"n"}
    ]);
    request(&registry, json!({"op":"execute","handle":handle,"sql":"INSERT INTO fixture VALUES(?,?,?,?,?,?)","args":args})).unwrap();
    let result = query(&registry, &handle, "SELECT * FROM fixture");
    assert_eq!(result["rows"][0], args);
}

#[test]
fn unsupported_or_lossy_bindings_fail_without_modifying_data() {
    let (registry, handle) = memory();
    execute(&registry, &handle, "CREATE TABLE fixture(a)").unwrap();
    for cell in [json!({"t":"i","v":"9223372036854775808"}), json!({"t":"i","v":"01"}),
        json!({"t":"f","v":"NaN"}), json!({"t":"f","v":"inf"}), json!({"t":"b","v":"!"})] {
        assert_eq!(request(&registry, json!({"op":"execute","handle":handle,"sql":"INSERT INTO fixture VALUES(?)","args":[cell]})), Err("native_db_arguments"));
    }
    assert_eq!(query(&registry, &handle, "SELECT count(*) FROM fixture")["rows"][0][0]["v"], "0");
}

#[test]
fn bootstrap_can_inspect_wal_mode_but_query_cannot_change_it() {
    let (registry, handle) = memory();
    assert_eq!(query(&registry, &handle, "PRAGMA journal_mode")["rows"][0][0]["v"], "memory");
    assert_eq!(request(&registry, json!({"op":"query","handle":handle,"sql":"PRAGMA journal_mode=OFF","args":[]})), Err("native_db_arguments"));
}

#[test]
fn failed_jni_delivery_releases_the_unreceived_handle() {
    fn open_memory(_: &str) -> Result<Connection, String> { Connection::open_in_memory().map_err(|_| "fixture_failed".into()) }
    let directory = tempfile::tempdir().unwrap();
    let response = reply(&json!({"op":"open", "path":directory.path().join("fixture.db")}).to_string(), open_memory);
    let parsed: Value = serde_json::from_str(&response).unwrap();
    let handle = parsed["result"]["handle"].as_str().unwrap();
    discard_undelivered_reply(&response);
    let closed: Value = serde_json::from_str(&reply(&json!({"op":"in_transaction", "handle":handle}).to_string(), open_memory)).unwrap();
    assert_eq!(closed["error"], "native_db_closed");
}

#[test]
fn full_registry_returns_connection_ownership_for_close_outside_its_mutex() {
    let registry = Mutex::new(Registry::default());
    for _ in 0..HANDLE_LIMIT {
        registry.lock().unwrap().insert(Connection::open_in_memory().unwrap()).unwrap();
    }
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("BEGIN; CREATE TABLE fixture(id)").unwrap();
    let rejected = { registry.lock().unwrap().insert(connection) };
    let (code, connection) = rejected.unwrap_err();
    assert_eq!(code, "native_db_limit");
    assert!(!connection.is_autocommit());
    assert!(registry.try_lock().is_ok());
    drop(connection);
}

#[test]
fn failed_page_rolls_back_rows_and_cursor_together() {
    let (registry, handle) = memory();
    execute(&registry, &handle, "CREATE TABLE fixture(id PRIMARY KEY)").unwrap();
    execute(&registry, &handle, "CREATE TABLE cursor(version NOT NULL)").unwrap();
    execute(&registry, &handle, "INSERT INTO cursor VALUES(0)").unwrap();
    request(&registry, json!({"op":"begin","handle":handle})).unwrap();
    execute(&registry, &handle, "INSERT INTO fixture VALUES(1)").unwrap();
    execute(&registry, &handle, "UPDATE cursor SET version=1 WHERE version=0").unwrap();
    assert_eq!(execute(&registry, &handle, "INSERT INTO fixture VALUES(1)"), Err("native_db_failed"));
    request(&registry, json!({"op":"end","handle":handle})).unwrap();
    assert_eq!(query(&registry, &handle, "SELECT count(*) FROM fixture")["rows"][0][0]["v"], "0");
    assert_eq!(query(&registry, &handle, "SELECT version FROM cursor")["rows"][0][0]["v"], "0");
    request(&registry, json!({"op":"begin","handle":handle})).unwrap();
    execute(&registry, &handle, "INSERT INTO fixture VALUES(2)").unwrap();
    execute(&registry, &handle, "UPDATE cursor SET version=1 WHERE version=0").unwrap();
    request(&registry, json!({"op":"successful","handle":handle})).unwrap();
    request(&registry, json!({"op":"end","handle":handle})).unwrap();
    assert_eq!(query(&registry, &handle, "SELECT version FROM cursor")["rows"][0][0]["v"], "1");
    assert_eq!(execute(&registry, &handle, "UPDATE cursor SET version=9 WHERE version=0").unwrap()["changes"], "0");
}

#[test]
fn close_rolls_back_and_stale_handle_cannot_reach_a_reopened_database() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.db");
    Connection::open(&path).unwrap().execute_batch("CREATE TABLE fixture(id)").unwrap();
    let registry = Mutex::new(Registry::default());
    let first = request(&registry, json!({"op":"open","path":path})).unwrap()["handle"].as_str().unwrap().to_owned();
    request(&registry, json!({"op":"begin","handle":first})).unwrap();
    execute(&registry, &first, "INSERT INTO fixture VALUES(1)").unwrap();
    request(&registry, json!({"op":"close","handle":first})).unwrap();
    let second = request(&registry, json!({"op":"open","path":path})).unwrap()["handle"].as_str().unwrap().to_owned();
    assert_ne!(first, second);
    assert_eq!(execute(&registry, &first, "DELETE FROM fixture"), Err("native_db_closed"));
    assert_eq!(query(&registry, &second, "SELECT count(*) FROM fixture")["rows"][0][0]["v"], "0");
    request(&registry, json!({"op":"close","handle":second})).unwrap();
    assert!(registry.lock().unwrap().handles.is_empty());
}

#[test]
fn missing_and_corrupt_files_are_not_created_replaced_or_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.db");
    let registry = Mutex::new(Registry::default());
    assert_eq!(request(&registry, json!({"op":"open","path":path})), Err("native_db_not_ready"));
    assert!(!path.exists());
    let sentinel = b"synthetic-corrupt-fixture";
    std::fs::write(&path, sentinel).unwrap();
    // SQLite may defer detecting corruption until the first statement; either way no recovery runs.
    if let Ok(result) = request(&registry, json!({"op":"open","path":path})) {
        let handle = result["handle"].as_str().unwrap();
        assert_eq!(execute(&registry, handle, "CREATE TABLE test(id)"), Err("native_db_failed"));
        request(&registry, json!({"op":"close","handle":handle})).unwrap();
    }
    assert_eq!(std::fs::read(path).unwrap(), sentinel);
}

#[test]
fn waiting_writer_does_not_hold_registry_mutex_needed_by_committing_writer() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("concurrency.db");
    let first = Connection::open(&path).unwrap();
    first.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE fixture(id)").unwrap();
    let second = Connection::open(&path).unwrap();
    second.busy_timeout(Duration::from_secs(2)).unwrap();
    let mut registry = Registry::default();
    let first = registry.insert(first).unwrap().to_string();
    let second = registry.insert(second).unwrap().to_string();
    let registry = Arc::new(Mutex::new(registry));
    request(&registry, json!({"op":"begin","handle":first})).unwrap();
    execute(&registry, &first, "INSERT INTO fixture VALUES(1)").unwrap();
    let (started, ready) = mpsc::channel();
    let other_registry = registry.clone();
    let other = std::thread::spawn(move || {
        started.send(()).unwrap();
        execute(&other_registry, &second, "INSERT INTO fixture VALUES(2)")
    });
    ready.recv().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    request(&registry, json!({"op":"successful","handle":first})).unwrap();
    request(&registry, json!({"op":"end","handle":first})).unwrap();
    other.join().unwrap().unwrap();
    assert_eq!(query(&registry, &first, "SELECT count(*) FROM fixture")["rows"][0][0]["v"], "2");
    assert_eq!(query(&registry, &first, "PRAGMA integrity_check")["rows"][0][0]["v"], "ok");
}
