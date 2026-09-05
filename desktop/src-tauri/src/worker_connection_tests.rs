use super::*;
use rusqlite::ffi;
use std::{ptr, sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}}};

#[derive(Default)]
struct Observed { calls: AtomicUsize, closed: AtomicBool, autocommit: AtomicBool }
struct Probe { statement: *mut ffi::sqlite3_stmt, seen: Arc<Observed>, fail: bool }

unsafe extern "C" fn finalize(ctx: *mut ffi::sqlite3_context, _: i32, _: *mut *mut ffi::sqlite3_value) {
    let probe = unsafe { &mut *(ffi::sqlite3_user_data(ctx) as *mut Probe) };
    probe.seen.calls.fetch_add(1, Ordering::SeqCst);
    probe.seen.autocommit.store(unsafe { ffi::sqlite3_get_autocommit(ffi::sqlite3_context_db_handle(ctx)) } != 0, Ordering::SeqCst);
    if !probe.statement.is_null() {
        unsafe { ffi::sqlite3_finalize(probe.statement); }
        probe.statement = ptr::null_mut();
    }
    if probe.fail { unsafe { ffi::sqlite3_result_error(ctx, c"PUBLIC_SYNTHETIC_FINALIZE_ERROR".as_ptr(), -1); } }
    else { unsafe { ffi::sqlite3_result_int(ctx, 0); } }
}
unsafe extern "C" fn destroyed(data: *mut std::ffi::c_void) {
    let probe = unsafe { Box::from_raw(data as *mut Probe) };
    probe.seen.closed.store(true, Ordering::SeqCst);
}
fn tracked(fail: bool) -> (Connection, Arc<Observed>) {
    let connection = Connection::open_in_memory().unwrap();
    let seen = Arc::new(Observed::default());
    let mut statement = ptr::null_mut();
    unsafe {
        assert_eq!(ffi::sqlite3_prepare_v2(connection.handle(), c"SELECT 42".as_ptr(), -1, &mut statement, ptr::null_mut()), ffi::SQLITE_OK);
        let probe = Box::into_raw(Box::new(Probe { statement, seen: seen.clone(), fail }));
        assert_eq!(ffi::sqlite3_create_function_v2(connection.handle(), c"crsql_finalize".as_ptr(), 0,
            ffi::SQLITE_UTF8, probe.cast(), Some(finalize), None, None, Some(destroyed)), ffi::SQLITE_OK);
    }
    (connection, seen)
}
fn owned(connection: Connection) -> WorkerConnection {
    let mut owned = WorkerConnection::new(connection);
    owned.mark_crsqlite_loaded();
    owned
}

#[test]
fn sqlite_close_is_busy_with_an_extension_owned_statement_and_raii_releases_it() {
    let (connection, seen) = tracked(false);
    let (connection, error) = connection.close().unwrap_err();
    assert_eq!(error.sqlite_error_code(), Some(rusqlite::ErrorCode::DatabaseBusy));
    assert!(!seen.closed.load(Ordering::SeqCst));
    drop(owned(connection));
    assert_eq!(seen.calls.load(Ordering::SeqCst), 1);
    assert!(seen.closed.load(Ordering::SeqCst));
}

#[test]
fn explicit_close_rolls_back_before_finalizing_and_closes_exactly_once() {
    let (connection, seen) = tracked(false);
    connection.execute_batch("BEGIN; CREATE TABLE public_fixture(id)").unwrap();
    assert!(!connection.is_autocommit());
    owned(connection).close().unwrap();
    assert_eq!(seen.calls.load(Ordering::SeqCst), 1);
    assert!(seen.autocommit.load(Ordering::SeqCst));
    assert!(seen.closed.load(Ordering::SeqCst));
}

#[test]
fn initialization_error_and_unwind_both_finalize_owned_connections() {
    let (connection, seen) = tracked(false);
    let attempt = || -> Result<(), &'static str> {
        let _owned = owned(connection);
        Err("public_synthetic_initialization_error")?;
        Ok(())
    };
    assert!(attempt().is_err());
    assert!(seen.closed.load(Ordering::SeqCst));
    let (connection, seen) = tracked(false);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _owned = owned(connection);
        panic!("public_synthetic_unwind");
    }));
    assert!(result.is_err());
    assert!(seen.closed.load(Ordering::SeqCst));
}

#[test]
fn absent_extension_is_valid_and_rollback_preserves_committed_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("public-fixture.db");
    let connection = WorkerConnection::new(Connection::open(&path).unwrap());
    connection.execute_batch("CREATE TABLE fixture(id); INSERT INTO fixture VALUES(1); BEGIN EXCLUSIVE; INSERT INTO fixture VALUES(2)").unwrap();
    connection.close().unwrap();
    let next = Connection::open(path).unwrap();
    assert_eq!(next.query_row("SELECT count(*) FROM fixture", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
}

#[test]
fn finalizer_failure_returns_only_fixed_code_and_still_attempts_close() {
    let (connection, seen) = tracked(true);
    assert_eq!(owned(connection).close(), Err("native_db_close_failed"));
    assert_eq!(seen.calls.load(Ordering::SeqCst), 1);
    assert!(seen.closed.load(Ordering::SeqCst));
}

#[test]
fn handoff_to_hanni_db_retains_cleanup_for_both_connections_and_poisoned_mutex() {
    let (writer, writer_seen) = tracked(false);
    let (reader, reader_seen) = tracked(false);
    let database = owned(writer).into_hanni_db(owned(reader));
    assert_eq!(writer_seen.calls.load(Ordering::SeqCst), 0);
    assert_eq!(reader_seen.calls.load(Ordering::SeqCst), 0);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let writer = database.writer.lock().unwrap();
        writer.execute_batch("BEGIN; CREATE TABLE public_fixture(id)").unwrap();
        panic!("public_synthetic_mutex_poison");
    }));
    drop(database);
    for seen in [writer_seen,reader_seen] {
        assert_eq!(seen.calls.load(Ordering::SeqCst), 1);
        assert!(seen.closed.load(Ordering::SeqCst));
        assert!(seen.autocommit.load(Ordering::SeqCst));
    }
}

#[test]
fn unconfirmed_load_does_not_call_extension_finalizer() {
    let (connection, seen) = tracked(false);
    // Remove the public mock's outstanding statement before simulating a failed load;
    // the wrapper must not call any unconfirmed extension function during cleanup.
    connection.execute_batch("SELECT crsql_finalize()").unwrap();
    seen.calls.store(0, Ordering::SeqCst);
    WorkerConnection::new(connection).close().unwrap();
    assert_eq!(seen.calls.load(Ordering::SeqCst), 0);
    assert!(seen.closed.load(Ordering::SeqCst));
}
