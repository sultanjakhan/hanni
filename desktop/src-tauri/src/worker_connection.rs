//! Own a short-lived worker connection, including cr-sqlite teardown on every exit.
use rusqlite::Connection;
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub(crate) struct WorkerConnection { connection: Option<Connection>, crsqlite_loaded: bool }

impl WorkerConnection {
    pub(crate) fn new(connection: Connection) -> Self { Self { connection: Some(connection), crsqlite_loaded: false } }

    // Call immediately after successful load. SQLite unloads the library on init error,
    // so never invoke an extension callback following an unsuccessful load attempt.
    pub(crate) fn mark_crsqlite_loaded(&mut self) { self.crsqlite_loaded = true; }

    pub(crate) fn close(mut self) -> Result<(), &'static str> { self.finish() }

    // The sole ownership transfer is directly into the long-lived owner, whose Drop
    // performs the same teardown. Do all fallible reader/writer setup before this call.
    pub(crate) fn into_hanni_db(mut self, mut reader: Self) -> crate::types::HanniDb {
        crate::types::HanniDb {
            writer: std::sync::Mutex::new(self.connection.take().expect("worker_connection_closed")),
            reader: std::sync::Mutex::new(reader.connection.take().expect("worker_connection_closed")),
        }
    }

    fn finish(&mut self) -> Result<(), &'static str> {
        let Some(connection) = self.connection.take() else { return Ok(()); };
        // No commit on an error/drop path. Release our statements before the extension's.
        let rollback = if connection.is_autocommit() { Ok(()) } else {
            connection.execute_batch("ROLLBACK").map_err(|_| "native_db_close_failed")
        };
        connection.flush_prepared_statement_cache();
        // No schema/function probing on a connection that failed before extension load.
        let finalize = if self.crsqlite_loaded {
            connection.execute_batch("SELECT crsql_finalize()").map_err(|_| "native_db_close_failed")
        } else { Ok(()) };
        // Always attempt close even if rollback/finalize failed. Never emit raw errors.
        let closed = connection.close().map_err(|(_connection, _error)| "native_db_close_failed");
        rollback.and(finalize).and(closed)
    }
}

impl Deref for WorkerConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection { self.connection.as_ref().expect("worker_connection_closed") }
}
impl DerefMut for WorkerConnection {
    fn deref_mut(&mut self) -> &mut Connection { self.connection.as_mut().expect("worker_connection_closed") }
}
impl Drop for WorkerConnection {
    fn drop(&mut self) {
        if self.finish().is_err() { eprintln!("[hanni-worker] native_db_close_failed"); }
    }
}

#[cfg(test)]
#[path = "worker_connection_tests.rs"]
mod tests;
