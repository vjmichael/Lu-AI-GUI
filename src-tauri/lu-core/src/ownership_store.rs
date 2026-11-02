//! Durable, exact owner binding. This is a storage primitive, not a caller
//! authenticator: only trusted host enrollment code may call enroll().
use crate::host_resource_authority::{
    AuthorizedResource, HostPrincipal, HostResourceAuthority, ResourceAction, ResourceDenial,
    ResourceRecord, Sensitivity,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipStoreError {
    Database(String),
    Denied(ResourceDenial),
    Locked,
}

impl From<rusqlite::Error> for OwnershipStoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value.to_string())
    }
}

fn code(value: Sensitivity) -> i64 {
    match value {
        Sensitivity::Public => 0,
        Sensitivity::Internal => 1,
        Sensitivity::Confidential => 2,
        Sensitivity::Restricted => 3,
    }
}

fn decode(value: i64) -> Result<Sensitivity, OwnershipStoreError> {
    match value {
        0 => Ok(Sensitivity::Public),
        1 => Ok(Sensitivity::Internal),
        2 => Ok(Sensitivity::Confidential),
        3 => Ok(Sensitivity::Restricted),
        _ => Err(OwnershipStoreError::Database(
            "invalid stored sensitivity".into(),
        )),
    }
}

/// Production callers must restrict db_path to a verified app-owned directory.
pub struct OwnershipStore {
    conn: Mutex<Connection>,
}

impl OwnershipStore {
    pub fn open(db_path: &Path) -> Result<Self, OwnershipStoreError> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversation_owners (
               conversation_id TEXT PRIMARY KEY NOT NULL,
               owner_profile_id TEXT NOT NULL,
               project_id TEXT NOT NULL,
               sensitivity INTEGER NOT NULL CHECK(sensitivity BETWEEN 0 AND 3)
             );
             CREATE TRIGGER IF NOT EXISTS owners_no_update
               BEFORE UPDATE ON conversation_owners BEGIN
               SELECT RAISE(ABORT,'ownership mutation prohibited'); END;
             CREATE TRIGGER IF NOT EXISTS owners_no_delete
               BEFORE DELETE ON conversation_owners BEGIN
               SELECT RAISE(ABORT,'ownership deletion prohibited'); END;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Deliberately no import-from-chat/LLM pathway. Re-enrollment with
    /// different ownership or sensitivity is rejected, including after restart.
    pub fn enroll(&self, record: &ResourceRecord) -> Result<(), OwnershipStoreError> {
        let mut validator = HostResourceAuthority::new();
        validator
            .enroll(record.clone())
            .map_err(OwnershipStoreError::Denied)?;
        let conn = self.conn.lock().map_err(|_| OwnershipStoreError::Locked)?;
        conn.execute(
            "INSERT OR IGNORE INTO conversation_owners
             (conversation_id,owner_profile_id,project_id,sensitivity)
             VALUES (?1,?2,?3,?4)",
            params![
                record.conversation_id,
                record.owner_profile_id,
                record.project_id,
                code(record.sensitivity)
            ],
        )?;
        let stored = Self::read(&conn, &record.conversation_id)?
            .ok_or(OwnershipStoreError::Denied(ResourceDenial::MissingRecord))?;
        if stored != *record {
            return Err(OwnershipStoreError::Denied(ResourceDenial::WrongOwner));
        }
        Ok(())
    }

    fn read(conn: &Connection, id: &str) -> Result<Option<ResourceRecord>, OwnershipStoreError> {
        let raw: Option<(String, String, String, i64)> = conn
            .query_row(
                "SELECT conversation_id,owner_profile_id,project_id,sensitivity
                 FROM conversation_owners WHERE conversation_id=?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        raw.map(
            |(conversation_id, owner_profile_id, project_id, sensitivity)| {
                Ok(ResourceRecord {
                    conversation_id,
                    owner_profile_id,
                    project_id,
                    sensitivity: decode(sensitivity)?,
                })
            },
        )
        .transpose()
    }

    /// Reads the authoritative row anew on each call. HDC scores and arbitrary
    /// client-provided identities are NOT accepted by this API.
    pub fn require(
        &self,
        principal: &HostPrincipal,
        conversation_id: &str,
        action: ResourceAction,
    ) -> Result<AuthorizedResource, OwnershipStoreError> {
        let conn = self.conn.lock().map_err(|_| OwnershipStoreError::Locked)?;
        let record = Self::read(&conn, conversation_id)?
            .ok_or(OwnershipStoreError::Denied(ResourceDenial::MissingRecord))?;
        let mut authority = HostResourceAuthority::new();
        authority
            .enroll(record)
            .map_err(OwnershipStoreError::Denied)?;
        authority
            .require(principal, conversation_id, action)
            .map_err(OwnershipStoreError::Denied)
    }
}
