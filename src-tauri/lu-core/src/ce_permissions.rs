//! Durable, host-issued CE permission records and synchronous revocation events.
//! A manifest or model cannot mint a grant. Grants bind an exact signed digest,
//! publisher key, profile, scope, tool, effect, and authenticated user turn.
//! A private, host-owned SQLite database is required by the embedding app.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{
    ce_signing::{CePublisherTrust, SignatureError, VerifiedCePackage},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    privilege_guard::enforce_model_ceiling,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CePermissionError {
    Database(String),
    InvalidGrant,
    InsecureStorage,
    AlreadyRecorded,
    NotFound,
    Denied,
    Unavailable,
}

impl From<rusqlite::Error> for CePermissionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

const CE_PRIVATE_SUBDIRECTORY: &str = "lu-ce";
const CE_DB_NAME: &str = "permissions.sqlite";
const CE_PROFILE_ID_DOMAIN: &[u8] = b"Lu-CE-Host-Profile-v1\0";

fn canonical_host_profile(folder: &Path) -> Result<PathBuf, CePermissionError> {
    let stat = std::fs::symlink_metadata(folder).map_err(|_| CePermissionError::InsecureStorage)?;
    if !stat.file_type().is_dir() {
        return Err(CePermissionError::InsecureStorage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if stat.permissions().mode() & 0o077 != 0 {
            return Err(CePermissionError::InsecureStorage);
        }
    }
    folder
        .canonicalize()
        .map_err(|_| CePermissionError::InsecureStorage)
}

fn host_id_for_canonical_folder(canonical: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(CE_PROFILE_ID_DOMAIN);
    // Lossy path-to-UTF8 conversion would conflate distinct non-Unicode
    // profile directories. Hash the native path representation instead.
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        digest.update(canonical.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in canonical.as_os_str().encode_wide() {
            digest.update(unit.to_le_bytes());
        }
    }
    #[cfg(not(any(unix, windows)))]
    digest.update(canonical.as_os_str().to_string_lossy().as_bytes());
    format!("lu.{:x}", digest.finalize())
}

fn bound_profile_matches(
    conn: &Connection,
    claimed_profile_id: &str,
) -> Result<bool, CePermissionError> {
    let bound: Option<String> = conn
        .query_row(
            "SELECT profile_id FROM ce_host_profile_binding WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(bound.is_none_or(|id| id == claimed_profile_id))
}

fn now_secs() -> Result<i64, CePermissionError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CePermissionError::Unavailable)?
        .as_secs();
    i64::try_from(now).map_err(|_| CePermissionError::Unavailable)
}

fn valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn effect_code(effect: ToolEffect) -> i64 {
    match effect {
        ToolEffect::ReadWorkspace => 1,
        ToolEffect::WriteWorkspace => 2,
        ToolEffect::ExecuteCode => 3,
        ToolEffect::NetworkRead => 4,
        ToolEffect::NetworkWrite => 5,
        ToolEffect::DeviceControl => 6,
        ToolEffect::Elevated => 7,
    }
}

/// Host UI must authenticate this user turn from persisted records. The
/// approval is not inferred from a model, manifest, or extension request.
#[derive(Debug, Clone)]
pub struct CeGrantRequest {
    pub profile_id: String,
    pub scope_id: String,
    pub tool_id: String,
    pub effect: ToolEffect,
    pub approved_user_turn_id: String,
    pub expires_at: i64,
}

/// Authenticated host enrollment of a public verification key. Lu never
/// stores publisher private signing keys.
#[derive(Debug, Clone)]
pub struct CePublisherKeyEnrollment {
    pub key_id: String,
    pub publisher: String,
    pub public_key: [u8; 32],
    pub not_before: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeRevocation {
    pub profile_id: String,
    pub package_id: String,
    pub scope_id: String,
    /// None means every tool in this package/scope.
    pub tool_id: Option<String>,
}

type Listener = Arc<dyn Fn(&CeRevocation) + Send + Sync>;

struct Inner {
    // Canonical file path: prevents substituting a copied/foreign SQLite DB
    // when the signed-package desktop consent adapter checks the host profile.
    database_path: PathBuf,
    conn: Mutex<Connection>,
    listeners: Mutex<Vec<Listener>>,
}

/// Clones share one connection and one listener set. A separate process
/// editing the database is NOT an approved revocation pathway: the trusted
/// host must call revoke_* so active worker cancellation is synchronous.
#[derive(Clone)]
pub struct CePermissionStore {
    inner: Arc<Inner>,
}

fn key_authorized(
    conn: &Connection,
    package: &VerifiedCePackage,
    now: i64,
) -> Result<bool, CePermissionError> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ce_publisher_keys
            WHERE key_id=?1 AND publisher=?2 AND public_key=?3
              AND state='active' AND not_before<=?4 AND expires_at>?4
              AND NOT EXISTS (
                SELECT 1 FROM ce_revoked_signing_keys
                WHERE ce_revoked_signing_keys.key_id=ce_publisher_keys.key_id
              )
        )",
        params![
            package.key_id(),
            package.manifest().publisher,
            package.signer_public_key().as_slice(),
            now
        ],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn validated_key(key: &CePublisherKeyEnrollment) -> Result<(), CePermissionError> {
    let start = u64::try_from(key.not_before).map_err(|_| CePermissionError::InvalidGrant)?;
    let end = u64::try_from(key.expires_at).map_err(|_| CePermissionError::InvalidGrant)?;
    let mut trust = CePublisherTrust::new();
    trust
        .pin_key(&key.key_id, &key.publisher, key.public_key, start, end)
        .map_err(|_| CePermissionError::InvalidGrant)
}

fn revoke_grants_for_key(
    tx: &Transaction<'_>,
    key_id: &str,
    now: i64,
) -> Result<Vec<CeRevocation>, CePermissionError> {
    let entries = {
        let mut statement = tx.prepare(
            "SELECT DISTINCT profile,package,scope FROM ce_grants
             WHERE key_id=?1 AND revoked_at IS NULL",
        )?;
        let events = statement
            .query_map(params![key_id], |r| {
                Ok(CeRevocation {
                    profile_id: r.get(0)?,
                    package_id: r.get(1)?,
                    scope_id: r.get(2)?,
                    tool_id: None,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        events
    };
    tx.execute(
        "INSERT OR IGNORE INTO ce_revoked_signing_keys(key_id,revoked_at)
         VALUES (?1,?2)",
        params![key_id, now],
    )?;
    tx.execute(
        "UPDATE ce_grants SET revoked_at=?1
         WHERE key_id=?2 AND revoked_at IS NULL",
        params![now, key_id],
    )?;
    for entry in &entries {
        tx.execute(
            "INSERT INTO ce_permission_events(event,profile,package,scope,tool,at)
             VALUES ('revoke',?1,?2,?3,NULL,?4)",
            params![entry.profile_id, entry.package_id, entry.scope_id, now],
        )?;
    }
    Ok(entries)
}

/// Authoritative grant check under an existing SQLite connection or
/// transaction. Callers must never use a result after releasing the lease
/// to perform a privileged filesystem effect.
fn check_grant_locked(
    conn: &Connection,
    package: &VerifiedCePackage,
    profile: &str,
    scope: &str,
    tool: &str,
    effect: ToolEffect,
    now: i64,
) -> Result<(), CePermissionError> {
    if !valid(profile)
        || !valid(scope)
        || !valid(tool)
        || !package.manifest().tools.iter().any(|id| id == tool)
        || enforce_model_ceiling(effect).is_err()
        || !bound_profile_matches(conn, profile)?
    {
        return Err(CePermissionError::Denied);
    }
    let record: Option<(i64, String, String, i64, Option<i64>)> = conn
        .query_row(
            "SELECT effect,digest,key_id,expires_at,revoked_at
             FROM ce_grants WHERE profile=?1 AND package=?2 AND scope=?3 AND tool=?4
               AND NOT EXISTS (
                 SELECT 1 FROM ce_revoked_signing_keys
                  WHERE key_id=ce_grants.key_id
               )",
            params![profile, package.manifest().id, scope, tool],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    match record {
        Some((stored_effect, digest, key, expiry, None))
            if stored_effect == effect_code(effect)
                && digest == package.artifact_sha256()
                && key == package.key_id()
                && now < expiry
                && key_authorized(conn, package, now)? =>
        {
            Ok(())
        }
        _ => Err(CePermissionError::Denied),
    }
}

impl CePermissionStore {
    pub fn open(path: &Path) -> Result<Self, CePermissionError> {
        // Caller must provide an already protected Lu-owned profile directory.
        // Never accept a path supplied by an extension, model or webview.
        let parent = path.parent().ok_or(CePermissionError::InsecureStorage)?;
        let parent_meta =
            std::fs::symlink_metadata(parent).map_err(|_| CePermissionError::InsecureStorage)?;
        if !parent_meta.file_type().is_dir() {
            return Err(CePermissionError::InsecureStorage);
        }
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if !meta.file_type().is_file() {
                return Err(CePermissionError::InsecureStorage);
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            if parent_meta.permissions().mode() & 0o077 != 0 {
                return Err(CePermissionError::InsecureStorage);
            }
            if let Ok(meta) = std::fs::symlink_metadata(path) {
                if !meta.file_type().is_file() {
                    return Err(CePermissionError::InsecureStorage);
                }
            }
            // The database and SQLite sidecars must never be world-readable.
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).mode(0o600);
            let _file = options
                .open(path)
                .map_err(|e| CePermissionError::Database(e.to_string()))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| CePermissionError::Database(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS ce_grants (
               profile TEXT NOT NULL,
               package TEXT NOT NULL,
               scope TEXT NOT NULL,
               tool TEXT NOT NULL,
               effect INTEGER NOT NULL,
               digest TEXT NOT NULL,
               key_id TEXT NOT NULL,
               user_turn TEXT NOT NULL,
               approved_at INTEGER NOT NULL,
               expires_at INTEGER NOT NULL,
               revoked_at INTEGER,
               PRIMARY KEY(profile, package, scope, tool)
             );
             CREATE TABLE IF NOT EXISTS ce_host_profile_binding (
               singleton INTEGER PRIMARY KEY CHECK(singleton=1),
               profile_id TEXT NOT NULL
             );
             CREATE TRIGGER IF NOT EXISTS ce_profile_binding_immutable_update
               BEFORE UPDATE ON ce_host_profile_binding BEGIN
                 SELECT RAISE(ABORT, 'CE host profile binding is immutable');
               END;
             CREATE TRIGGER IF NOT EXISTS ce_profile_binding_immutable_delete
               BEFORE DELETE ON ce_host_profile_binding BEGIN
                 SELECT RAISE(ABORT, 'CE host profile binding is immutable');
               END;
             CREATE TABLE IF NOT EXISTS ce_publisher_keys (
               key_id TEXT PRIMARY KEY NOT NULL,
               publisher TEXT NOT NULL,
               public_key BLOB NOT NULL CHECK(length(public_key)=32),
               not_before INTEGER NOT NULL,
               expires_at INTEGER NOT NULL,
               state TEXT NOT NULL CHECK(state IN ('active','revoked','retired')),
               replaced_by TEXT,
               enrolled_at INTEGER NOT NULL,
               revoked_at INTEGER
             );
             CREATE TABLE IF NOT EXISTS ce_publisher_key_events (
               seq INTEGER PRIMARY KEY AUTOINCREMENT,
               key_id TEXT NOT NULL,
               event TEXT NOT NULL CHECK(event IN ('enroll','retire','revoke')),
               detail TEXT,
               at INTEGER NOT NULL
             );
             CREATE TRIGGER IF NOT EXISTS ce_key_events_no_update
               BEFORE UPDATE ON ce_publisher_key_events BEGIN
                 SELECT RAISE(ABORT, 'CE trust audit is immutable');
               END;
             CREATE TRIGGER IF NOT EXISTS ce_key_events_no_delete
               BEFORE DELETE ON ce_publisher_key_events BEGIN
                 SELECT RAISE(ABORT, 'CE trust audit is immutable');
               END;
             CREATE TABLE IF NOT EXISTS ce_revoked_signing_keys (
               key_id TEXT PRIMARY KEY NOT NULL,
               revoked_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS ce_grant_versions (
               seq INTEGER PRIMARY KEY AUTOINCREMENT,
               profile TEXT NOT NULL,
               package TEXT NOT NULL,
               scope TEXT NOT NULL,
               tool TEXT NOT NULL,
               key_id TEXT NOT NULL,
               digest TEXT NOT NULL,
               effect INTEGER NOT NULL,
               user_turn TEXT NOT NULL,
               approved_at INTEGER NOT NULL
             );
             CREATE TRIGGER IF NOT EXISTS ce_grant_versions_no_update
               BEFORE UPDATE ON ce_grant_versions BEGIN
                 SELECT RAISE(ABORT, 'CE grant version history is immutable');
               END;
             CREATE TRIGGER IF NOT EXISTS ce_grant_versions_no_delete
               BEFORE DELETE ON ce_grant_versions BEGIN
                 SELECT RAISE(ABORT, 'CE grant version history is immutable');
               END;
             CREATE TABLE IF NOT EXISTS ce_permission_events (
               seq INTEGER PRIMARY KEY AUTOINCREMENT,
               event TEXT NOT NULL CHECK(event IN ('approve','revoke')),
               profile TEXT NOT NULL,
               package TEXT NOT NULL,
               scope TEXT NOT NULL,
               tool TEXT,
               at INTEGER NOT NULL
             );
             CREATE TRIGGER IF NOT EXISTS ce_events_immutable_update
               BEFORE UPDATE ON ce_permission_events BEGIN
                 SELECT RAISE(ABORT, 'CE permission audit event is immutable');
               END;
             CREATE TRIGGER IF NOT EXISTS ce_events_immutable_delete
               BEFORE DELETE ON ce_permission_events BEGIN
                 SELECT RAISE(ABORT, 'CE permission audit event is immutable');
               END;",
        )?;
        let store = Self {
            inner: Arc::new(Inner {
                database_path: path
                    .canonicalize()
                    .map_err(|_| CePermissionError::InsecureStorage)?,
                conn: Mutex::new(conn),
                listeners: Mutex::new(Vec::new()),
            }),
        };
        // Startup is fail-closed: neither expired/retired keys nor legacy
        // approvals lacking an enrolled key may be used after recovery.
        store.recover_publisher_trust()?;
        Ok(store)
    }

    /// TRUSTED DESKTOP HOST ONLY. Derive an identity from the currently
    /// selected, canonical Lu profile directory. It is a namespace marker,
    /// NOT a secret or an authentication token.
    pub fn host_profile_id_for_folder(folder: &Path) -> Result<String, CePermissionError> {
        let canonical = canonical_host_profile(folder)?;
        Ok(host_id_for_canonical_folder(&canonical))
    }

    /// Host-owned profile-local CE store constructor. No caller can nominate
    /// a database filename, stored profile ID, or host-wide root. Existing
    /// unbound stores with grants or publisher pins require explicit migration;
    /// do not silently import permissions into a new profile.
    pub fn open_for_host_profile(folder: &Path) -> Result<Self, CePermissionError> {
        let canonical = canonical_host_profile(folder)?;
        let private = canonical.join(CE_PRIVATE_SUBDIRECTORY);
        if !private.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&private)
                    .map_err(|_| CePermissionError::InsecureStorage)?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir(&private).map_err(|_| CePermissionError::InsecureStorage)?;
            }
        }
        let metadata =
            std::fs::symlink_metadata(&private).map_err(|_| CePermissionError::InsecureStorage)?;
        if !metadata.file_type().is_dir() {
            return Err(CePermissionError::InsecureStorage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(CePermissionError::InsecureStorage);
            }
        }
        let private = private
            .canonicalize()
            .map_err(|_| CePermissionError::InsecureStorage)?;
        let store = Self::open(&private.join(CE_DB_NAME))?;
        let expected_id = host_id_for_canonical_folder(&canonical);
        let mut conn = store
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT profile_id FROM ce_host_profile_binding WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match existing {
            Some(id) if id == expected_id => (),
            Some(_) => return Err(CePermissionError::InsecureStorage),
            None => {
                let legacy: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM ce_grants)
                        OR EXISTS(SELECT 1 FROM ce_publisher_keys)
                        OR EXISTS(SELECT 1 FROM ce_revoked_signing_keys)
                        OR EXISTS(SELECT 1 FROM ce_grant_versions)
                        OR EXISTS(SELECT 1 FROM ce_publisher_key_events)
                        OR EXISTS(SELECT 1 FROM ce_permission_events)",
                    [],
                    |row| row.get(0),
                )?;
                if legacy {
                    return Err(CePermissionError::InsecureStorage);
                }
                tx.execute(
                    "INSERT INTO ce_host_profile_binding(singleton,profile_id) VALUES (1,?1)",
                    params![expected_id],
                )?;
            }
        }
        tx.commit()?;
        drop(conn);
        Ok(store)
    }

    /// Desktop consent guard: verify the canonical active Lu directory,
    /// fixed CE trust DB location, durable bound profile, and exact grant
    /// profile ID. The check is repeated just before native grant commit.
    pub fn require_host_profile(
        &self,
        active_folder: &Path,
        claimed_profile_id: &str,
    ) -> Result<(), CePermissionError> {
        let canonical = canonical_host_profile(active_folder)?;
        let expected_id = host_id_for_canonical_folder(&canonical);
        if claimed_profile_id != expected_id {
            return Err(CePermissionError::Denied);
        }
        let expected_db = canonical.join(CE_PRIVATE_SUBDIRECTORY).join(CE_DB_NAME);
        let expected_db = expected_db
            .canonicalize()
            .map_err(|_| CePermissionError::InsecureStorage)?;
        if expected_db != self.inner.database_path {
            return Err(CePermissionError::Denied);
        }
        let conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let bound: Option<String> = conn
            .query_row(
                "SELECT profile_id FROM ce_host_profile_binding WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if bound.as_deref() != Some(claimed_profile_id) {
            return Err(CePermissionError::Denied);
        }
        Ok(())
    }

    pub fn subscribe_revocations(&self, listener: Listener) -> Result<(), CePermissionError> {
        self.inner
            .listeners
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?
            .push(listener);
        Ok(())
    }

    /// TRUSTED HOST ONLY. Rejects duplicate/replayed approval and silently
    /// changed package bytes. A new digest needs an explicit new approval
    /// after the old row has been deliberately retired.
    pub fn approve(
        &self,
        package: &VerifiedCePackage,
        grant: &CeGrantRequest,
    ) -> Result<(), CePermissionError> {
        self.approve_inner(package, grant, None)
    }

    /// Host-native confirmation deadlines are rechecked after SQLite writer
    /// contention and immediately before committing a durable grant.
    /// The monotonic deadline must originate in trusted desktop state.
    pub fn approve_with_deadline(
        &self,
        package: &VerifiedCePackage,
        grant: &CeGrantRequest,
        deadline: Instant,
    ) -> Result<(), CePermissionError> {
        self.approve_inner(package, grant, Some(deadline))
    }

    fn approve_inner(
        &self,
        package: &VerifiedCePackage,
        grant: &CeGrantRequest,
        deadline: Option<Instant>,
    ) -> Result<(), CePermissionError> {
        if !valid(&grant.profile_id)
            || !valid(&grant.scope_id)
            || !valid(&grant.approved_user_turn_id)
            || !valid(&grant.tool_id)
            || !package.manifest().tools.contains(&grant.tool_id)
            || enforce_model_ceiling(grant.effect).is_err()
        {
            return Err(CePermissionError::InvalidGrant);
        }
        let mut conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if deadline.is_some_and(|expires| Instant::now() >= expires) {
            return Err(CePermissionError::Denied);
        }
        // Admission time must be sampled AFTER acquiring the durable writer
        // lease. A queued native consent may expire while another connection
        // holds SQLite's lock; never commit using its stale pre-wait time.
        let now = now_secs()?;
        if grant.expires_at <= now {
            return Err(CePermissionError::InvalidGrant);
        }
        if !key_authorized(&tx, package, now)? || !bound_profile_matches(&tx, &grant.profile_id)? {
            return Err(CePermissionError::Denied);
        }
        let mut count = tx.execute(
            "INSERT OR IGNORE INTO ce_grants
              (profile,package,scope,tool,effect,digest,key_id,user_turn,approved_at,expires_at,revoked_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,NULL)",
            params![
                grant.profile_id, package.manifest().id, grant.scope_id, grant.tool_id,
                effect_code(grant.effect), package.artifact_sha256(), package.key_id(),
                grant.approved_user_turn_id, now, grant.expires_at
            ]
        )?;
        if count == 0 {
            // A previously revoked version can be replaced ONLY under a
            // distinct user-authored turn. Never overwrite active grants and
            // never silently inherit an old publisher's authorization.
            count = tx.execute(
                "UPDATE ce_grants SET effect=?1,digest=?2,key_id=?3,user_turn=?4,
                    approved_at=?5,expires_at=?6,revoked_at=NULL
                 WHERE profile=?7 AND package=?8 AND scope=?9 AND tool=?10
                   AND revoked_at IS NOT NULL AND user_turn<>?4",
                params![
                    effect_code(grant.effect),
                    package.artifact_sha256(),
                    package.key_id(),
                    grant.approved_user_turn_id,
                    now,
                    grant.expires_at,
                    grant.profile_id,
                    package.manifest().id,
                    grant.scope_id,
                    grant.tool_id
                ],
            )?;
        }
        if count != 1 {
            return Err(CePermissionError::AlreadyRecorded);
        }
        tx.execute(
            "INSERT INTO ce_grant_versions
             (profile,package,scope,tool,key_id,digest,effect,user_turn,approved_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                grant.profile_id,
                package.manifest().id,
                grant.scope_id,
                grant.tool_id,
                package.key_id(),
                package.artifact_sha256(),
                effect_code(grant.effect),
                grant.approved_user_turn_id,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO ce_permission_events(event,profile,package,scope,tool,at)
             VALUES ('approve',?1,?2,?3,?4,?5)",
            params![
                grant.profile_id,
                package.manifest().id,
                grant.scope_id,
                grant.tool_id,
                now
            ],
        )?;
        // Reject native consent that expired while writing grant rows.
        if deadline.is_some_and(|expires| Instant::now() >= expires) {
            return Err(CePermissionError::Denied);
        }
        tx.commit()?;
        Ok(())
    }

    /// Re-read durable state on EVERY broker operation and worker admission.
    /// This plain check does not grant permission for effects after it returns;
    /// use with_authorized_grant for privileged host operations.
    pub fn require(
        &self,
        package: &VerifiedCePackage,
        profile: &str,
        scope: &str,
        tool: &str,
        effect: ToolEffect,
    ) -> Result<(), CePermissionError> {
        let conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let now = now_secs()?;
        check_grant_locked(&conn, package, profile, scope, tool, effect, now)
    }

    /// Execute a short host-controlled effect while holding an IMMEDIATE
    /// SQLite transaction against the authoritative trust/grant store.
    ///
    /// This fences revocation and publisher key rotation from all connections
    /// using the same SQLite database (not only cloned Rust store handles).
    /// Revocation cannot commit until this operation completes, and a new
    /// operation begun after revocation is denied. Do not use for unbounded
    /// execution, network requests, or user interaction.
    pub fn with_authorized_grant<R>(
        &self,
        package: &VerifiedCePackage,
        profile: &str,
        scope: &str,
        tool: &str,
        effect: ToolEffect,
        operation: impl FnOnce() -> R,
    ) -> Result<R, CePermissionError> {
        let mut conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Evaluate expiry AFTER waiting for the database lease. A stale
        // timestamp taken before lock acquisition must not admit an expired
        // grant following contention with another host connection.
        let now = now_secs()?;
        check_grant_locked(&tx, package, profile, scope, tool, effect, now)?;
        let outcome = operation();
        // No persistent changes are made by this lease. Drop the reserved
        // writer lock before any revocation listeners are notified.
        tx.rollback()?;
        Ok(outcome)
    }

    pub fn revoke(
        &self,
        profile: &str,
        package: &str,
        scope: &str,
        tool: Option<&str>,
    ) -> Result<(), CePermissionError> {
        if !valid(profile)
            || !valid(package)
            || !valid(scope)
            || tool.is_some_and(|tool| !valid(tool))
        {
            return Err(CePermissionError::InvalidGrant);
        }
        let now = now_secs()?;
        {
            let mut conn = self
                .inner
                .conn
                .lock()
                .map_err(|_| CePermissionError::Unavailable)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let affected = if let Some(tool) = tool {
                tx.execute(
                    "UPDATE ce_grants SET revoked_at=?1 WHERE profile=?2 AND package=?3
                     AND scope=?4 AND tool=?5 AND revoked_at IS NULL",
                    params![now, profile, package, scope, tool],
                )?
            } else {
                tx.execute(
                    "UPDATE ce_grants SET revoked_at=?1 WHERE profile=?2 AND package=?3
                     AND scope=?4 AND revoked_at IS NULL",
                    params![now, profile, package, scope],
                )?
            };
            if affected == 0 {
                return Err(CePermissionError::NotFound);
            }
            tx.execute(
                "INSERT INTO ce_permission_events(event,profile,package,scope,tool,at)
                 VALUES ('revoke',?1,?2,?3,?4,?5)",
                params![profile, package, scope, tool, now],
            )?;
            tx.commit()?;
        }
        let event = CeRevocation {
            profile_id: profile.to_owned(),
            package_id: package.to_owned(),
            scope_id: scope.to_owned(),
            tool_id: tool.map(str::to_owned),
        };
        let listeners = self
            .inner
            .listeners
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?
            .clone();
        for listener in listeners {
            listener(&event);
        }
        Ok(())
    }

    /// TRUSTED HOST ONLY. The key must be provisioned from an authenticated
    /// local publisher enrollment flow, never from an extension manifest.
    /// A key ID cannot be reused after retirement or revocation.
    pub fn enroll_publisher_key(
        &self,
        key: &CePublisherKeyEnrollment,
    ) -> Result<(), CePermissionError> {
        validated_key(key)?;
        let now = now_secs()?;
        if key.expires_at <= now {
            return Err(CePermissionError::InvalidGrant);
        }
        let mut conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(
              SELECT 1 FROM ce_publisher_keys WHERE key_id=?1
              UNION SELECT 1 FROM ce_revoked_signing_keys WHERE key_id=?1
             )",
            params![key.key_id],
            |row| row.get(0),
        )?;
        if exists {
            return Err(CePermissionError::AlreadyRecorded);
        }
        tx.execute(
            "INSERT INTO ce_publisher_keys
             (key_id,publisher,public_key,not_before,expires_at,state,replaced_by,enrolled_at,revoked_at)
             VALUES (?1,?2,?3,?4,?5,'active',NULL,?6,NULL)",
            params![key.key_id, key.publisher, key.public_key.as_slice(),
                key.not_before, key.expires_at, now],
        )?;
        tx.execute(
            "INSERT INTO ce_publisher_key_events(key_id,event,detail,at)
             VALUES (?1,'enroll',?2,?3)",
            params![key.key_id, key.publisher, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Query the authoritative database afresh on every signature verification.
    /// There is no mutable in-memory publisher pin cache that can outlive a
    /// committed revocation. A successful verification is not a grant.
    pub fn verify_signed_package_at(
        &self,
        envelope_json: &str,
        artifact: &[u8],
        now: u64,
    ) -> Result<VerifiedCePackage, SignatureError> {
        let conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| SignatureError::UnknownKey)?;
        let mut statement = conn
            .prepare(
                "SELECT key_id,publisher,public_key,not_before,expires_at
             FROM ce_publisher_keys
             WHERE state='active'
               AND key_id NOT IN (SELECT key_id FROM ce_revoked_signing_keys)",
            )
            .map_err(|_| SignatureError::UnknownKey)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|_| SignatureError::UnknownKey)?;
        let mut trust = CePublisherTrust::new();
        for entry in rows {
            let (id, publisher, bytes, start, end) =
                entry.map_err(|_| SignatureError::InvalidKey)?;
            let key: [u8; 32] = bytes.try_into().map_err(|_| SignatureError::InvalidKey)?;
            trust.pin_key(
                &id,
                &publisher,
                key,
                u64::try_from(start).map_err(|_| SignatureError::InvalidKey)?,
                u64::try_from(end).map_err(|_| SignatureError::InvalidKey)?,
            )?;
        }
        // Hold read of authoritative state through verification to serialize
        // against a concurrent writer on this store. Broker still rechecks.
        trust.verify_at(envelope_json, artifact, now)
    }

    pub fn verify_signed_package(
        &self,
        envelope_json: &str,
        artifact: &[u8],
    ) -> Result<VerifiedCePackage, SignatureError> {
        let now = now_secs().map_err(|_| SignatureError::KeyExpired)?;
        self.verify_signed_package_at(
            envelope_json,
            artifact,
            u64::try_from(now).map_err(|_| SignatureError::KeyExpired)?,
        )
    }

    fn notify_key_revocation(&self, events: &[CeRevocation]) -> Result<(), CePermissionError> {
        let listeners = self
            .inner
            .listeners
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?
            .clone();
        for event in events {
            for listener in &listeners {
                listener(event);
            }
        }
        Ok(())
    }

    /// One host-controlled transaction retires the old identity, adds the
    /// successor, denies the old key forever, and cancels all its grants.
    /// Rotation never migrates user grants: the new signer needs fresh consent.
    pub fn rotate_publisher_key(
        &self,
        previous_key_id: &str,
        successor: &CePublisherKeyEnrollment,
    ) -> Result<usize, CePermissionError> {
        validated_key(successor)?;
        if !valid(previous_key_id)
            || previous_key_id == successor.key_id
            || successor.expires_at <= now_secs()?
        {
            return Err(CePermissionError::InvalidGrant);
        }
        let now = now_secs()?;
        let mut conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior: Option<(String, Vec<u8>)> = tx
            .query_row(
                "SELECT publisher,public_key FROM ce_publisher_keys
             WHERE key_id=?1 AND state='active'",
                params![previous_key_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (publisher, prior_bytes) = prior.ok_or(CePermissionError::NotFound)?;
        if publisher != successor.publisher || prior_bytes == successor.public_key {
            return Err(CePermissionError::InvalidGrant);
        }
        let exists: bool = tx.query_row(
            "SELECT EXISTS(
              SELECT 1 FROM ce_publisher_keys WHERE key_id=?1
              UNION SELECT 1 FROM ce_revoked_signing_keys WHERE key_id=?1
             )",
            params![successor.key_id],
            |row| row.get(0),
        )?;
        if exists {
            return Err(CePermissionError::AlreadyRecorded);
        }
        tx.execute(
            "INSERT INTO ce_publisher_keys
             (key_id,publisher,public_key,not_before,expires_at,state,replaced_by,enrolled_at,revoked_at)
             VALUES (?1,?2,?3,?4,?5,'active',NULL,?6,NULL)",
            params![successor.key_id, successor.publisher, successor.public_key.as_slice(),
                successor.not_before, successor.expires_at, now],
        )?;
        tx.execute(
            "UPDATE ce_publisher_keys SET state='retired',replaced_by=?1,revoked_at=?2
             WHERE key_id=?3 AND state='active'",
            params![successor.key_id, now, previous_key_id],
        )?;
        let events = revoke_grants_for_key(&tx, previous_key_id, now)?;
        tx.execute(
            "INSERT INTO ce_publisher_key_events(key_id,event,detail,at)
             VALUES (?1,'retire',?2,?3)",
            params![previous_key_id, successor.key_id, now],
        )?;
        tx.execute(
            "INSERT INTO ce_publisher_key_events(key_id,event,detail,at)
             VALUES (?1,'enroll',?2,?3)",
            params![successor.key_id, successor.publisher, now],
        )?;
        tx.commit()?;
        drop(conn);
        self.notify_key_revocation(&events)?;
        Ok(events.len())
    }

    /// Unified trusted-host publisher revocation. Same transaction updates
    /// verification trust, durable grant denial, and immutable audit events;
    /// synchronously notifying worker supervisors after commit.
    pub fn revoke_publisher_key(&self, key_id: &str) -> Result<usize, CePermissionError> {
        if !valid(key_id) {
            return Err(CePermissionError::InvalidGrant);
        }
        let now = now_secs()?;
        let mut conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM ce_publisher_keys WHERE key_id=?1)",
            params![key_id],
            |row| row.get(0),
        )?;
        // A revoked/retired identity is permanently denied: idempotent.
        if !exists {
            return Err(CePermissionError::NotFound);
        }
        tx.execute(
            "UPDATE ce_publisher_keys SET state='revoked',revoked_at=COALESCE(revoked_at,?1)
             WHERE key_id=?2 AND state='active'",
            params![now, key_id],
        )?;
        let events = revoke_grants_for_key(&tx, key_id, now)?;
        tx.execute(
            "INSERT INTO ce_publisher_key_events(key_id,event,detail,at)
             VALUES (?1,'revoke',NULL,?2)",
            params![key_id, now],
        )?;
        tx.commit()?;
        drop(conn);
        self.notify_key_revocation(&events)?;
        Ok(events.len())
    }

    /// Startup recovery: revoke orphaned legacy grants, retired or expired
    /// identities. Returns the count of distinct affected package scopes.
    /// Already-running workers are NOT resurrected after a host restart.
    pub fn recover_publisher_trust(&self) -> Result<usize, CePermissionError> {
        let now = now_secs()?;
        let mut conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let affected = {
            let mut statement = tx.prepare(
                "SELECT DISTINCT profile,package,scope FROM ce_grants
                 WHERE revoked_at IS NULL AND NOT EXISTS (
                   SELECT 1 FROM ce_publisher_keys p
                   WHERE p.key_id=ce_grants.key_id AND p.state='active'
                     AND p.not_before<=?1 AND p.expires_at>?1
                     AND p.key_id NOT IN (SELECT key_id FROM ce_revoked_signing_keys)
                 )",
            )?;
            let events = statement
                .query_map(params![now], |r| {
                    Ok(CeRevocation {
                        profile_id: r.get(0)?,
                        package_id: r.get(1)?,
                        scope_id: r.get(2)?,
                        tool_id: None,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            events
        };
        tx.execute(
            "UPDATE ce_grants SET revoked_at=?1 WHERE revoked_at IS NULL
             AND NOT EXISTS (
                SELECT 1 FROM ce_publisher_keys p
                WHERE p.key_id=ce_grants.key_id AND p.state='active'
                  AND p.not_before<=?1 AND p.expires_at>?1
                  AND p.key_id NOT IN (SELECT key_id FROM ce_revoked_signing_keys)
             )",
            params![now],
        )?;
        for event in &affected {
            tx.execute(
                "INSERT INTO ce_permission_events(event,profile,package,scope,tool,at)
                 VALUES ('revoke',?1,?2,?3,NULL,?4)",
                params![event.profile_id, event.package_id, event.scope_id, now],
            )?;
        }
        tx.commit()?;
        drop(conn);
        self.notify_key_revocation(&affected)?;
        Ok(affected.len())
    }

    pub fn is_publisher_key_revoked(&self, key_id: &str) -> Result<bool, CePermissionError> {
        if !valid(key_id) {
            return Err(CePermissionError::InvalidGrant);
        }
        let conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM ce_revoked_signing_keys WHERE key_id=?1
                UNION SELECT 1 FROM ce_publisher_keys WHERE key_id=?1 AND state!='active'
             )",
            params![key_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn audit_event_count(&self) -> Result<u64, CePermissionError> {
        let conn = self
            .inner
            .conn
            .lock()
            .map_err(|_| CePermissionError::Unavailable)?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM ce_permission_events", [], |r| {
            r.get(0)
        })?;
        u64::try_from(count).map_err(|_| CePermissionError::Unavailable)
    }
}

/// Immutable package/scope binding, provided by the trusted host when a task
/// starts. This is an additional policy layer, not a substitute for user-turn
/// verification by ConversationAuthority.
pub struct CeGrantAuthorizer {
    pub store: CePermissionStore,
    pub package: VerifiedCePackage,
    pub profile_id: String,
    pub scope_id: String,
}

impl ToolAuthorizer for CeGrantAuthorizer {
    fn authorize(
        &self,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError> {
        if invocation.workspace_id.as_deref() != Some(self.scope_id.as_str())
            || invocation.tool_id != descriptor.id
        {
            return Err(AuthorizationError::new("CE scope or tool mismatch"));
        }
        self.store
            .require(
                &self.package,
                &self.profile_id,
                &self.scope_id,
                &descriptor.id,
                descriptor.effect,
            )
            .map_err(|_| AuthorizationError::new("CE permission missing, changed or revoked"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_signing::CePublisherTrust;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};

    fn package() -> VerifiedCePackage {
        let signing = SigningKey::from_bytes(&[13u8; 32]);
        let mut trust = CePublisherTrust::new();
        trust
            .pin_key(
                "key",
                "Publisher",
                signing.verifying_key().to_bytes(),
                0,
                u64::MAX,
            )
            .unwrap();
        let manifest = serde_json::json!({
            "format_version":1,"id":"publisher.extension","version":"1.0.0",
            "contract_version":1,"publisher":"Publisher",
            "artifact_sha256":format!("{:x}",Sha256::digest(b"worker")),
            "dependencies":[],"tools":["publisher.extension.run"]
        })
        .to_string();
        let mut message = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
        message.extend_from_slice(&(3u32).to_be_bytes());
        message.extend_from_slice(b"key");
        message.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
        message.extend_from_slice(manifest.as_bytes());
        let signature = signing.sign(&message);
        let envelope = serde_json::json!({
            "format_version":1,"key_id":"key","manifest_json":manifest,
            "signature_hex":signature.to_bytes().iter().map(|b|format!("{b:02x}")).collect::<String>()
        }).to_string();
        trust.verify_at(&envelope, b"worker", 200).unwrap()
    }

    #[test]
    fn grants_survive_restart_and_revocations_notify_and_deny() {
        let root = std::env::temp_dir().join(format!(
            "lu_ce_permission_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let path = root.join("permissions.db");
        let _ = std::fs::remove_file(&path);
        let pkg = package();
        let grant = CeGrantRequest {
            profile_id: "profile".into(),
            scope_id: "scope".into(),
            tool_id: "publisher.extension.run".into(),
            effect: ToolEffect::ReadWorkspace,
            approved_user_turn_id: "verified-turn".into(),
            expires_at: i64::MAX,
        };
        let store = CePermissionStore::open(&path).unwrap();
        store
            .enroll_publisher_key(&CePublisherKeyEnrollment {
                key_id: "key".into(),
                publisher: "Publisher".into(),
                public_key: ed25519_dalek::SigningKey::from_bytes(&[13u8; 32])
                    .verifying_key()
                    .to_bytes(),
                not_before: 0,
                expires_at: i64::MAX,
            })
            .unwrap();
        store.approve(&pkg, &grant).unwrap();
        assert_eq!(
            store.approve(&pkg, &grant),
            Err(CePermissionError::AlreadyRecorded)
        );
        assert_eq!(store.audit_event_count().unwrap(), 1);
        drop(store);
        let store = CePermissionStore::open(&path).unwrap();
        store
            .require(
                &pkg,
                "profile",
                "scope",
                &grant.tool_id,
                ToolEffect::ReadWorkspace,
            )
            .unwrap();
        assert_eq!(
            store.require(
                &pkg,
                "other",
                "scope",
                &grant.tool_id,
                ToolEffect::ReadWorkspace
            ),
            Err(CePermissionError::Denied)
        );
        assert_eq!(
            store.require(
                &pkg,
                "profile",
                "scope",
                &grant.tool_id,
                ToolEffect::WriteWorkspace
            ),
            Err(CePermissionError::Denied)
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        store
            .subscribe_revocations(Arc::new(move |event| {
                captured.lock().unwrap().push(event.clone())
            }))
            .unwrap();
        store
            .revoke(
                "profile",
                "publisher.extension",
                "scope",
                Some(&grant.tool_id),
            )
            .unwrap();
        assert_eq!(events.lock().unwrap().len(), 1);
        assert_eq!(store.audit_event_count().unwrap(), 2);
        assert_eq!(
            store.require(
                &pkg,
                "profile",
                "scope",
                &grant.tool_id,
                ToolEffect::ReadWorkspace
            ),
            Err(CePermissionError::Denied)
        );
        drop(store);
        let reopened = CePermissionStore::open(&path).unwrap();
        assert_eq!(
            reopened.require(
                &pkg,
                "profile",
                "scope",
                &grant.tool_id,
                ToolEffect::ReadWorkspace
            ),
            Err(CePermissionError::Denied)
        );
        drop(reopened);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn publisher_key_revocation_is_durable_and_cancels_every_scoped_grant() {
        let root = std::env::temp_dir().join(format!(
            "lu_ce_key_revoke_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let path = root.join("permissions.db");
        let _ = std::fs::remove_file(&path);
        let pkg = package();
        let store = CePermissionStore::open(&path).unwrap();
        store
            .enroll_publisher_key(&CePublisherKeyEnrollment {
                key_id: "key".into(),
                publisher: "Publisher".into(),
                public_key: ed25519_dalek::SigningKey::from_bytes(&[13u8; 32])
                    .verifying_key()
                    .to_bytes(),
                not_before: 0,
                expires_at: i64::MAX,
            })
            .unwrap();
        let mut grant = CeGrantRequest {
            profile_id: "profile".into(),
            scope_id: "workspace1".into(),
            tool_id: "publisher.extension.run".into(),
            effect: ToolEffect::ReadWorkspace,
            approved_user_turn_id: "verified-turn".into(),
            expires_at: i64::MAX,
        };
        store.approve(&pkg, &grant).unwrap();
        grant.scope_id = "workspace2".into();
        store.approve(&pkg, &grant).unwrap();
        let events = Arc::new(Mutex::new(Vec::<CeRevocation>::new()));
        let events_out = Arc::clone(&events);
        store
            .subscribe_revocations(Arc::new(move |event| {
                events_out.lock().unwrap().push(event.clone());
            }))
            .unwrap();
        assert_eq!(store.revoke_publisher_key("key").unwrap(), 2);
        assert!(store.is_publisher_key_revoked("key").unwrap());
        let revoked = events.lock().unwrap();
        assert_eq!(revoked.len(), 2);
        assert!(revoked.iter().all(|r| r.tool_id.is_none()));
        drop(revoked);
        for scope in ["workspace1", "workspace2"] {
            assert_eq!(
                store.require(
                    &pkg,
                    "profile",
                    scope,
                    &grant.tool_id,
                    ToolEffect::ReadWorkspace
                ),
                Err(CePermissionError::Denied)
            );
        }
        grant.scope_id = "workspace3".into();
        assert_eq!(store.approve(&pkg, &grant), Err(CePermissionError::Denied));
        assert_eq!(store.revoke_publisher_key("key").unwrap(), 0);
        drop(store);
        let reopened = CePermissionStore::open(&path).unwrap();
        assert!(reopened.is_publisher_key_revoked("key").unwrap());
        assert_eq!(
            reopened.require(
                &pkg,
                "profile",
                "workspace1",
                &grant.tool_id,
                ToolEffect::ReadWorkspace
            ),
            Err(CePermissionError::Denied)
        );
        assert_eq!(
            reopened.approve(&pkg, &grant),
            Err(CePermissionError::Denied)
        );
        drop(reopened);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
