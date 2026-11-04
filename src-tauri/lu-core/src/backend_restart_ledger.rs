//! BACKEND-CTRL-01D — durable host-owned backend restart admission.
//! Count restart *attempts before spawn*, not just successful restarts. This
//! avoids an endless crash→relaunch cycle across desktop process restarts.
//! This ledger is not a tool permission and must not use model-generated keys.
use rusqlite::{params, Connection, TransactionBehavior};
use std::{path::Path, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

pub const RESTART_WINDOW_MS: i64 = 300_000;
pub const MAX_RESTARTS_PER_WINDOW: i64 = 2;

#[derive(Debug, PartialEq, Eq)]
pub enum RestartLedgerError {
    InvalidHostKey,
    InvalidTime,
    Poisoned,
    Database(String),
}
impl From<rusqlite::Error> for RestartLedgerError {
    fn from(err:rusqlite::Error)->Self { Self::Database(err.to_string()) }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RestartAdmission {
    Allowed,
    CircuitOpen,
}

pub struct BackendRestartLedger {
    connection: Mutex<Connection>,
}
impl BackendRestartLedger {
    pub fn open(path:&Path)->Result<Self,RestartLedgerError> {
        let db=Connection::open(path)?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_backend_restart_attempts (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 host_backend_key TEXT NOT NULL,
                 attempted_at_ms INTEGER NOT NULL,
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE INDEX IF NOT EXISTS lu_backend_restart_key_time
                ON lu_backend_restart_attempts(host_backend_key, attempted_at_ms);"
        )?;
        Ok(Self { connection:Mutex::new(db) })
    }
    fn validate(key:&str)->Result<(),RestartLedgerError>{
        if key.is_empty() || key.len()>128 || !key.bytes().all(|b|
            b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.')) {
            return Err(RestartLedgerError::InvalidHostKey);
        }
        Ok(())
    }
    /// Called by the trusted backend supervisor, before launching a worker.
    /// Returns CircuitOpen without inserting an attempt when already capped.
    /// Future-dated records also count, so a backwards wall-clock adjustment
    /// cannot reset an exhausted circuit.
    pub fn admit_at(&self,host_backend_key:&str,now_ms:i64)
        ->Result<RestartAdmission,RestartLedgerError>{
        Self::validate(host_backend_key)?;
        if now_ms<0 { return Err(RestartLedgerError::InvalidTime); }
        let mut guard=self.connection.lock().map_err(|_|RestartLedgerError::Poisoned)?;
        let tx=guard.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let recent:i64=tx.query_row(
            "SELECT COUNT(*) FROM lu_backend_restart_attempts
               WHERE host_backend_key=?1 AND attempted_at_ms>=?2",
            params![host_backend_key,now_ms.saturating_sub(RESTART_WINDOW_MS)],
            |row|row.get(0))?;
        if recent>=MAX_RESTARTS_PER_WINDOW {
            return Ok(RestartAdmission::CircuitOpen);
        }
        tx.execute(
            "INSERT INTO lu_backend_restart_attempts(host_backend_key,attempted_at_ms)
             VALUES (?1,?2)",params![host_backend_key,now_ms])?;
        // Cleanup is not part of deciding current admission; retain a bounded
        // recent history plus any future timestamps in case clocks move back.
        tx.execute(
            "DELETE FROM lu_backend_restart_attempts
             WHERE attempted_at_ms<?1",
            params![now_ms.saturating_sub(RESTART_WINDOW_MS*4)])?;
        tx.commit()?;
        Ok(RestartAdmission::Allowed)
    }
    pub fn admit_now(&self,key:&str)->Result<RestartAdmission,RestartLedgerError>{
        let elapsed=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_|RestartLedgerError::InvalidTime)?;
        let ms=i64::try_from(elapsed.as_millis())
            .map_err(|_|RestartLedgerError::InvalidTime)?;
        self.admit_at(key,ms)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn circuit_survives_database_reopen_and_does_not_erase_after_a_denial() {
        let path=std::env::temp_dir().join(format!(
            "lu-backend-restarts-{}-{:?}.sqlite",
            std::process::id(),std::thread::current().id()));
        let _=fs::remove_file(&path);
        {
            let ledger=BackendRestartLedger::open(&path).unwrap();
            assert_eq!(ledger.admit_at("llama.model-a",1000).unwrap(),
                RestartAdmission::Allowed);
            assert_eq!(ledger.admit_at("llama.model-a",1100).unwrap(),
                RestartAdmission::Allowed);
            assert_eq!(ledger.admit_at("llama.model-a",1200).unwrap(),
                RestartAdmission::CircuitOpen);
        }
        {
            let ledger=BackendRestartLedger::open(&path).unwrap();
            assert_eq!(ledger.admit_at("llama.model-a",1500).unwrap(),
                RestartAdmission::CircuitOpen);
            assert_eq!(ledger.admit_at("vllm.model-b",1500).unwrap(),
                RestartAdmission::Allowed);
            assert_eq!(ledger.admit_at("llama.model-a",301_100).unwrap(),
                RestartAdmission::Allowed);
            assert_eq!(ledger.admit_at("../../foreign",301_200),
                Err(RestartLedgerError::InvalidHostKey));
        }
        let _=fs::remove_file(&path);
        let _=fs::remove_file(path.with_extension("sqlite-wal"));
        let _=fs::remove_file(path.with_extension("sqlite-shm"));
    }
}
