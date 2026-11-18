//! SQLite lifetime writer lease for exclusive Lu desktop startup and safe journal recovery.
//! The lock database is separate from ordinary conversation/effect databases.
use std::{path::Path, sync::Mutex};
use rusqlite::{Connection, ErrorCode, OpenFlags};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerError { AlreadyRunning, Database(String) }

/// Hold for the full lifetime of the desktop. A crashed process releases its
/// OS-managed SQLite writer lock; do not commit its transaction while running.
pub struct DesktopOwner { connection: Mutex<Connection> }
impl DesktopOwner {
    pub fn acquire(path: &Path) -> Result<Self, OwnerError> {
        let connection = Connection::open_with_flags(
            path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE |
                OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        ).map_err(|e| OwnerError::Database(e.to_string()))?;
        connection.busy_timeout(std::time::Duration::from_millis(0))
            .map_err(|e| OwnerError::Database(e.to_string()))?;
        let map_error = |e: rusqlite::Error| match e {
            rusqlite::Error::SqliteFailure(code, _)
                if code.code == ErrorCode::DatabaseBusy || code.code == ErrorCode::DatabaseLocked
                    => OwnerError::AlreadyRunning,
            other => OwnerError::Database(other.to_string()),
        };
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS lu_instance_fence(
               id INTEGER PRIMARY KEY CHECK(id=1), token TEXT NOT NULL
             );
             INSERT OR IGNORE INTO lu_instance_fence(id,token) VALUES (1,'owner');"
        ).map_err(map_error)?;
        connection.execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE lu_instance_fence SET token='owner' WHERE id=1;"
        ).map_err(map_error)?;
        Ok(Self { connection: Mutex::new(connection) })
    }
}
impl Drop for DesktopOwner {
    fn drop(&mut self) {
        if let Ok(connection) = self.connection.get_mut() {
            let _ = connection.execute_batch("ROLLBACK");
        }
    }
}
