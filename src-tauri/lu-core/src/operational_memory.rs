//! LU-AIW-01: durable host-owned operational resource memory.
//!
//! Records WHERE an already discovered tool/artifact/dataset was found and
//! which host-verified recipe last succeeded. It does not install, execute,
//! grant access, resolve user permissions, or feed arbitrary stored commands
//! back into a shell. The caller must be the trusted host, not model output.
//!
//! "Previously successful" is historical evidence, not proof of current
//! usability. Each session begins with an empty set of live attestations;
//! a read-only probe checks current filesystem identity before reuse.

use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

const MAX_HASH_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LIST: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Executable,
    File,
    Directory,
}

impl ResourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
    fn from_str(value: &str) -> Result<Self, OperationalError> {
        match value {
            "executable" => Ok(Self::Executable),
            "file" => Ok(Self::File),
            "directory" => Ok(Self::Directory),
            _ => Err(OperationalError::CorruptRecord),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSpec {
    pub resource_id: String,
    pub workspace_id: String,
    pub display_name: String,
    pub kind: ResourceKind,
    /// Host-discovered absolute path on the recorded platform. Never a
    /// model-supplied execution target or an access-grant token.
    pub absolute_path: String,
    pub platform: String,
    pub architecture: String,
    pub version: String,
    /// Optional digest recorded by an authenticated host discovery action.
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastSuccess {
    /// Identifier for a host-approved workflow, not an executable command.
    pub recipe_id: String,
    /// Redacted human-readable evidence only. NEVER blindly run this string.
    pub safe_command_summary: String,
    pub source_sha256: Option<String>,
    pub verified_unix_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceState {
    /// The location was remembered but has not been probed in this call.
    KnownHistorically,
    /// File/dir exists, but its dependencies or behavior have not been
    /// re-tested by a trusted host in this process.
    PresentUnverified,
    /// Host recorded successful execution in this process, and current file
    /// content still matches its observed fingerprint. Not a permission.
    VerifiedUsable,
    Missing,
    Changed,
    Incompatible,
    Unverifiable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSnapshot {
    pub spec: ResourceSpec,
    pub state: ResourceState,
    pub last_success: Option<LastSuccess>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationalError {
    InvalidInput,
    Conflict,
    NotFound,
    NotReady,
    CorruptRecord,
    Database(String),
    Locked,
}
impl From<rusqlite::Error> for OperationalError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128
        && s.bytes().all(|b| b.is_ascii_alphanumeric() ||
            matches!(b, b'-' | b'_' | b'.'))
}
fn valid_label(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max
        && !s.chars().any(char::is_control)
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_spec(spec: &ResourceSpec) -> bool {
    valid_id(&spec.resource_id) && valid_id(&spec.workspace_id)
        && valid_label(&spec.display_name, 128)
        && valid_label(&spec.platform, 64)
        && valid_label(&spec.architecture, 64)
        && valid_label(&spec.version, 128)
        && spec.absolute_path.len() <= 2048
        && !spec.absolute_path.chars().any(char::is_control)
        && Path::new(&spec.absolute_path).is_absolute()
        && spec.expected_sha256.as_deref().map(valid_hash).unwrap_or(true)
        && !(spec.kind == ResourceKind::Directory && spec.expected_sha256.is_some())
}
fn current_time() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}
fn sha256_file(path: &Path) -> Result<String, OperationalError> {
    let mut input = File::open(path).map_err(|_| OperationalError::NotReady)?;
    let mut hasher = Sha256::new();
    let mut read_total = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let count = input.read(&mut buf).map_err(|_| OperationalError::NotReady)?;
        if count == 0 { break; }
        read_total = read_total.saturating_add(count as u64);
        if read_total > MAX_HASH_BYTES { return Err(OperationalError::NotReady); }
        hasher.update(&buf[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Application must place the SQLite database in a trusted profile-owned
/// directory. This library neither authenticates its caller nor grants access.
pub struct OperationalMemory {
    conn: Mutex<Connection>,
    /// Session-only proof, not loaded from SQLite on restart.
    attested_this_session: Mutex<HashSet<(String, String)>>,
}

impl OperationalMemory {
    pub fn open(path: &Path) -> Result<Self, OperationalError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS operational_resources(
                workspace_id TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                kind TEXT NOT NULL,
                absolute_path TEXT NOT NULL,
                platform TEXT NOT NULL,
                architecture TEXT NOT NULL,
                version TEXT NOT NULL,
                expected_sha256 TEXT,
                last_success_sha256 TEXT,
                last_recipe TEXT,
                last_command_summary TEXT,
                last_verified_unix INTEGER,
                PRIMARY KEY(workspace_id,resource_id)
             );
             CREATE INDEX IF NOT EXISTS operational_by_workspace
                ON operational_resources(workspace_id,kind,display_name);"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            attested_this_session: Mutex::new(HashSet::new()),
        })
    }

    /// Host-only discovery write. A repeated ID cannot silently redirect to
    /// another path, workspace, platform or digest; use a new ID for a new
    /// resource revision. This method performs no filesystem execution.
    pub fn remember(&self, spec: &ResourceSpec) -> Result<(), OperationalError> {
        if !valid_spec(spec) { return Err(OperationalError::InvalidInput); }
        let conn = self.conn.lock().map_err(|_| OperationalError::Locked)?;
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO operational_resources
            (workspace_id,resource_id,display_name,kind,absolute_path,
             platform,architecture,version,expected_sha256)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![spec.workspace_id,spec.resource_id,spec.display_name,
                spec.kind.as_str(),spec.absolute_path,spec.platform,
                spec.architecture,spec.version,spec.expected_sha256],
        )?;
        if inserted == 0 {
            let stored = read_resource(&conn, &spec.workspace_id, &spec.resource_id)?
                .ok_or(OperationalError::CorruptRecord)?;
            if stored.0 != *spec { return Err(OperationalError::Conflict); }
        }
        Ok(())
    }

    /// Context lookup stays scoped to the host-verified workspace. Callers
    /// must perform their own user/host authorization before disclosing paths.
    pub fn recall(&self, workspace_id: &str, resource_id: &str)
        -> Result<Option<ResourceSnapshot>, OperationalError> {
        if !valid_id(workspace_id) || !valid_id(resource_id) {
            return Err(OperationalError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| OperationalError::Locked)?;
        Ok(read_resource(&conn, workspace_id, resource_id)?
            .map(|(spec,last_success)| ResourceSnapshot {
                spec,state:ResourceState::KnownHistorically,last_success,
            }))
    }

    /// Only a read-only local existence/integrity probe. A resource never
    /// becomes "verified usable" solely because it has a remembered path.
    pub fn probe(&self, workspace_id: &str, resource_id: &str)
        -> Result<ResourceSnapshot, OperationalError> {
        let mut snapshot = self.recall(workspace_id, resource_id)?
            .ok_or(OperationalError::NotFound)?;
        if snapshot.spec.platform != std::env::consts::OS
            || snapshot.spec.architecture != std::env::consts::ARCH {
            snapshot.state = ResourceState::Incompatible;
            return Ok(snapshot);
        }
        let path = Path::new(&snapshot.spec.absolute_path);
        let metadata = match fs::symlink_metadata(path) {
            Ok(v) => v,
            Err(_) => {
                snapshot.state = ResourceState::Missing;
                return Ok(snapshot);
            }
        };
        if metadata.file_type().is_symlink()
            || (snapshot.spec.kind == ResourceKind::Directory && !metadata.is_dir())
            || (snapshot.spec.kind != ResourceKind::Directory && !metadata.is_file()) {
            snapshot.state = ResourceState::Unverifiable;
            return Ok(snapshot);
        }
        let old = snapshot.last_success.as_ref().and_then(|v| v.source_sha256.as_deref());
        let expected = snapshot.spec.expected_sha256.as_deref();
        let fingerprint = if snapshot.spec.kind == ResourceKind::Directory {
            None
        } else if old.is_some() || expected.is_some() {
            match sha256_file(path) {
                Ok(v) => Some(v),
                Err(_) => {
                    snapshot.state = ResourceState::Unverifiable;
                    return Ok(snapshot);
                }
            }
        } else { None };
        if expected.is_some_and(|digest| fingerprint.as_deref() != Some(digest))
            || old.is_some_and(|digest| fingerprint.as_deref() != Some(digest)) {
            snapshot.state = ResourceState::Changed;
            return Ok(snapshot);
        }
        let verified = self.attested_this_session.lock()
            .map_err(|_| OperationalError::Locked)?
            .contains(&(workspace_id.to_owned(), resource_id.to_owned()));
        snapshot.state = if verified && snapshot.spec.kind != ResourceKind::Directory
            && fingerprint.is_some() {
            ResourceState::VerifiedUsable
        } else {
            ResourceState::PresentUnverified
        };
        Ok(snapshot)
    }

    /// Record a successful check performed independently by the trusted host.
    /// A model-provided command/result is NOT suitable evidence for this API.
    /// No command is executed here. The source fingerprint is sampled locally
    /// to detect later changes, including across process restarts.
    pub fn record_host_success(&self, workspace_id: &str, resource_id: &str,
        recipe_id: &str, safe_command_summary: &str) -> Result<(), OperationalError> {
        if !valid_id(recipe_id) || !valid_label(safe_command_summary, 1024) {
            return Err(OperationalError::InvalidInput);
        }
        let snapshot = self.probe(workspace_id, resource_id)?;
        if !matches!(snapshot.state,
            ResourceState::PresentUnverified | ResourceState::VerifiedUsable) {
            return Err(OperationalError::NotReady);
        }
        let digest = if snapshot.spec.kind == ResourceKind::Directory {
            None
        } else {
            Some(sha256_file(Path::new(&snapshot.spec.absolute_path))?)
        };
        if snapshot.spec.expected_sha256.as_ref().is_some_and(|expected|
            digest.as_ref() != Some(expected)) {
            return Err(OperationalError::NotReady);
        }
        let conn = self.conn.lock().map_err(|_| OperationalError::Locked)?;
        let updated = conn.execute(
            "UPDATE operational_resources SET last_recipe=?3,
                last_command_summary=?4,last_success_sha256=?5,
                last_verified_unix=?6 WHERE workspace_id=?1 AND resource_id=?2",
            params![workspace_id,resource_id,recipe_id,safe_command_summary,
                digest,current_time()],
        )?;
        if updated != 1 { return Err(OperationalError::NotFound); }
        self.attested_this_session.lock().map_err(|_| OperationalError::Locked)?
            .insert((workspace_id.to_owned(), resource_id.to_owned()));
        Ok(())
    }

    /// Returns only metadata, never grants execution or a read capability.
    pub fn list(&self, workspace_id: &str, limit: usize)
        -> Result<Vec<ResourceSnapshot>, OperationalError> {
        if !valid_id(workspace_id) || limit == 0 || limit > MAX_LIST {
            return Err(OperationalError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| OperationalError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT resource_id FROM operational_resources
             WHERE workspace_id=?1 ORDER BY display_name,resource_id LIMIT ?2",
        )?;
        let ids = stmt.query_map(params![workspace_id,limit as i64],
            |row| row.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        ids.into_iter().map(|id| {
            let (spec,last_success) = read_resource(&conn,workspace_id,&id)?
                .ok_or(OperationalError::CorruptRecord)?;
            Ok(ResourceSnapshot {
                spec,last_success,state:ResourceState::KnownHistorically,
            })
        }).collect()
    }
}

fn read_resource(conn: &Connection, workspace_id: &str, resource_id: &str)
    -> Result<Option<(ResourceSpec, Option<LastSuccess>)>, OperationalError> {
    type ResourceRow = (String,String,String,String,String,String,String,String,
        Option<String>,Option<String>,Option<String>,Option<String>,Option<i64>);
    let raw: Option<ResourceRow> = conn.query_row(
        "SELECT resource_id,workspace_id,display_name,kind,absolute_path,
                platform,architecture,version,expected_sha256,
                last_success_sha256,last_recipe,last_command_summary,
                last_verified_unix
         FROM operational_resources
         WHERE workspace_id=?1 AND resource_id=?2",
        params![workspace_id,resource_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,
            row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,
            row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?)),
    ).optional()?;
    raw.map(|r| {
        let spec = ResourceSpec {
            resource_id:r.0, workspace_id:r.1, display_name:r.2,
            kind:ResourceKind::from_str(&r.3)?,absolute_path:r.4,
            platform:r.5,architecture:r.6,version:r.7,expected_sha256:r.8,
        };
        if !valid_spec(&spec) { return Err(OperationalError::CorruptRecord); }
        let last_success = match (r.10,r.11,r.12) {
            (None,None,None) => None,
            (Some(recipe_id),Some(safe_command_summary),Some(verified_unix_seconds)) =>
                Some(LastSuccess {
                    recipe_id,safe_command_summary,
                    source_sha256:r.9,verified_unix_seconds,
                }),
            _ => return Err(OperationalError::CorruptRecord),
        };
        Ok((spec,last_success))
    }).transpose()
}
