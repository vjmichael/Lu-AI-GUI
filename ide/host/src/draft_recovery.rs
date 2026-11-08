//! DEV-01.4: local, bounded, app-data-only unsaved draft recovery.
//! Project files are NEVER modified by these operations. The trusted Tauri
//! host supplies the vault root, not the WebView or an LLM. A fresh random
//! session ID prevents a new launch from silently overwriting older drafts.
use std::{
    fs, io::{self, Read, Write}, path::PathBuf, sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use cap_std::{ambient_authority, fs::{Dir, OpenOptions}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    browser::{revision_for, validate_relative, MAX_TEXT_BYTES},
    fresh_project_id, source_create::portable_components, wire,
    IdeWorkspaceRegistry,
};

const MAX_RECORD_BYTES: u64 = MAX_TEXT_BYTES + 4096;
const MAX_RECORDS: usize = 64;
const MAX_ENTRIES_TO_SCAN: usize = 128;

#[derive(Debug)]
pub enum DraftRecoveryError {
    Invalid,
    Io(io::Error),
    Stale,
    PreviousSession,
    Capacity,
    HostUnavailable,
}
impl DraftRecoveryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "DRAFT_RECOVERY_INVALID",
            Self::Io(_) => "DRAFT_RECOVERY_IO_UNAVAILABLE",
            Self::Stale => "DRAFT_RECOVERY_STALE",
            Self::PreviousSession => "DRAFT_RECOVERY_PREVIOUS_SESSION_REVIEW_REQUIRED",
            Self::Capacity => "DRAFT_RECOVERY_CAPACITY",
            Self::HostUnavailable => "DRAFT_RECOVERY_LOCK_UNAVAILABLE",
        }
    }
}
impl From<io::Error> for DraftRecoveryError {
    fn from(error: io::Error) -> Self { Self::Io(error) }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    version: u32,
    project_id: String,
    relative_path: String,
    base_revision: String,
    buffer_revision: u64,
    draft_sha256: String,
    draft_text: String,
    session_id: String,
    updated_at_unix_ms: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftRecoverySummary {
    pub project_id: String,
    pub relative_path: String,
    pub base_revision: String,
    pub buffer_revision: u64,
    pub draft_sha256: String,
    pub from_previous_session: bool,
    pub updated_at_unix_ms: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredDraft {
    pub project_id: String,
    pub relative_path: String,
    pub base_revision: String,
    pub buffer_revision: u64,
    pub draft_sha256: String,
    pub draft_text: String,
}

pub struct DraftRecoveryStore {
    dir: Dir,
    session_id: String,
    write_lock: Mutex<()>,
}

fn valid_revision(rev: &str) -> bool {
    rev.len() == 64 &&
        rev.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn valid_session_id(id: &str) -> bool {
    id.len() == 36 && id.starts_with("ide-") &&
        id[4..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn valid_key(project_id: &str, relative_path: &str) -> Result<(), DraftRecoveryError> {
    if !wire::valid_project_id(project_id) { return Err(DraftRecoveryError::Invalid); }
    let parts = validate_relative(relative_path, false).map_err(|_| DraftRecoveryError::Invalid)?;
    portable_components(&parts).map_err(|_| DraftRecoveryError::Invalid)?;
    Ok(())
}
fn file_name(project_id: &str, path: &str) -> String {
    let key = format!("{project_id}\u{0}{path}");
    format!("{:x}.json", Sha256::digest(key.as_bytes()))
}
fn summary(record: &Record, active_session: &str) -> DraftRecoverySummary {
    DraftRecoverySummary {
        project_id: record.project_id.clone(),
        relative_path: record.relative_path.clone(),
        base_revision: record.base_revision.clone(),
        buffer_revision: record.buffer_revision,
        draft_sha256: record.draft_sha256.clone(),
        from_previous_session: record.session_id != active_session,
        updated_at_unix_ms: record.updated_at_unix_ms,
    }
}
fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis().min(u128::from(u64::MAX)) as u64
}
fn read_one(dir: &Dir, name: &str) -> Result<Record, DraftRecoveryError> {
    let meta = dir.symlink_metadata(name)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_RECORD_BYTES {
        return Err(DraftRecoveryError::Invalid);
    }
    let mut bytes = Vec::new();
    dir.open(name)?.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES { return Err(DraftRecoveryError::Invalid); }
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| DraftRecoveryError::Invalid)?;
    if record.version != 1 ||
        valid_key(&record.project_id, &record.relative_path).is_err() ||
        !valid_revision(&record.base_revision) ||
        !valid_revision(&record.draft_sha256) ||
        !valid_session_id(&record.session_id) ||
        record.buffer_revision == 0 ||
        record.draft_text.len() as u64 > MAX_TEXT_BYTES ||
        record.draft_text.contains('\0') ||
        revision_for(record.draft_text.as_bytes()) != record.draft_sha256 ||
        file_name(&record.project_id, &record.relative_path) != name {
        return Err(DraftRecoveryError::Invalid);
    }
    Ok(record)
}
fn sync_dir(dir: &Dir) -> Result<(), DraftRecoveryError> {
    #[cfg(unix)] { dir.open(".")?.sync_all()?; }
    #[cfg(not(unix))] { let _ = dir; }
    Ok(())
}

impl DraftRecoveryStore {
    /// Only the trusted desktop passes its OS user-app-data directory here.
    /// This path is NEVER read from JavaScript, a project, or a connector.
    pub fn open(root: PathBuf) -> Result<Self, DraftRecoveryError> {
        match fs::symlink_metadata(&root) {
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() =>
                return Err(DraftRecoveryError::Invalid),
            Ok(_) => (),
            Err(err) if err.kind() == io::ErrorKind::NotFound =>
                fs::create_dir(&root)?,
            Err(err) => return Err(DraftRecoveryError::Io(err)),
        }
        let meta = fs::symlink_metadata(&root)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(DraftRecoveryError::Invalid);
        }
        #[cfg(windows)] {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(DraftRecoveryError::Invalid);
            }
        }
        #[cfg(unix)] {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
            if fs::symlink_metadata(&root)?.mode() & 0o077 != 0 {
                return Err(DraftRecoveryError::Invalid);
            }
        }
        let dir = Dir::open_ambient_dir(root, ambient_authority())?;
        Ok(Self { dir, session_id: fresh_project_id(), write_lock: Mutex::new(()) })
    }

    fn scan(&self) -> Result<Vec<Record>, DraftRecoveryError> {
        let mut records = Vec::new();
        let mut scanned = 0;
        for entry in self.dir.entries()? {
            scanned += 1;
            if scanned > MAX_ENTRIES_TO_SCAN { return Err(DraftRecoveryError::Capacity); }
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".json") { continue; } // unfinished .pending files are never trusted
            records.push(read_one(&self.dir, &name)?);
            if records.len() > MAX_RECORDS { return Err(DraftRecoveryError::Capacity); }
        }
        Ok(records)
    }
    fn read_existing(
        &self, project_id: &str, relative_path: &str,
    ) -> Result<Option<Record>, DraftRecoveryError> {
        valid_key(project_id, relative_path)?;
        let name = file_name(project_id, relative_path);
        match self.dir.symlink_metadata(&name) {
            Ok(_) => Ok(Some(read_one(&self.dir, &name)?)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(DraftRecoveryError::Io(e)),
        }
    }
    fn durable_record(&self, record: &Record) -> Result<(), DraftRecoveryError> {
        let bytes = serde_json::to_vec(record).map_err(|_| DraftRecoveryError::Invalid)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES { return Err(DraftRecoveryError::Invalid); }
        let temp = format!(".pending-{}-{}", self.session_id, fresh_project_id());
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)] {
            use cap_std::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = self.dir.open_with(&temp, &opts)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        // Overwrite is deliberately confined to this *private app-data
        // cache filename*, never a user source path, and guarded by a lock.
        self.dir.rename(&temp, &self.dir, file_name(&record.project_id, &record.relative_path))?;
        sync_dir(&self.dir)?;
        Ok(())
    }
    pub fn checkpoint(
        &self, project_id: &str, relative_path: &str,
        base_revision: &str, buffer_revision: u64, draft_text: &str,
    ) -> Result<DraftRecoverySummary, DraftRecoveryError> {
        valid_key(project_id, relative_path)?;
        if !valid_revision(base_revision) || buffer_revision == 0 ||
            draft_text.len() as u64 > MAX_TEXT_BYTES ||
            draft_text.contains('\0') {
            return Err(DraftRecoveryError::Invalid);
        }
        let _lock = self.write_lock.lock().map_err(|_| DraftRecoveryError::HostUnavailable)?;
        let current = self.read_existing(project_id, relative_path)?;
        let sha = revision_for(draft_text.as_bytes());
        if let Some(ref previous) = current {
            if previous.session_id != self.session_id {
                return Err(DraftRecoveryError::PreviousSession);
            }
            if previous.base_revision != base_revision ||
                buffer_revision < previous.buffer_revision ||
                (buffer_revision == previous.buffer_revision && sha != previous.draft_sha256) {
                return Err(DraftRecoveryError::Stale);
            }
            if buffer_revision == previous.buffer_revision && sha == previous.draft_sha256 {
                return Ok(summary(previous, &self.session_id));
            }
        } else if self.scan()?.len() >= MAX_RECORDS {
            return Err(DraftRecoveryError::Capacity);
        }
        let record = Record {
            version: 1, project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            base_revision: base_revision.to_owned(), buffer_revision,
            draft_sha256: sha, draft_text: draft_text.to_owned(),
            session_id: self.session_id.clone(), updated_at_unix_ms: now_millis(),
        };
        self.durable_record(&record)?;
        Ok(summary(&record, &self.session_id))
    }
    pub fn list(&self, project_id: &str) -> Result<Vec<DraftRecoverySummary>, DraftRecoveryError> {
        if !wire::valid_project_id(project_id) { return Err(DraftRecoveryError::Invalid); }
        let _lock = self.write_lock.lock().map_err(|_| DraftRecoveryError::HostUnavailable)?;
        let mut records: Vec<_> = self.scan()?.iter()
            .filter(|r| r.project_id == project_id)
            .map(|r| summary(r, &self.session_id)).collect();
        records.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(records)
    }
    /// Adopt exactly one matching persisted draft into this app session. The
    /// caller must compare base_revision to a fresh native saved-file preview
    /// before applying the text to CodeMirror. No source file is written here.
    pub fn adopt(
        &self, project_id: &str, relative_path: &str, expected_draft_sha256: &str,
    ) -> Result<RecoveredDraft, DraftRecoveryError> {
        let _lock = self.write_lock.lock().map_err(|_| DraftRecoveryError::HostUnavailable)?;
        let Some(mut record) = self.read_existing(project_id, relative_path)? else {
            return Err(DraftRecoveryError::Stale);
        };
        if record.draft_sha256 != expected_draft_sha256 { return Err(DraftRecoveryError::Stale); }
        record.session_id = self.session_id.clone();
        self.durable_record(&record)?;
        Ok(RecoveredDraft {
            project_id: record.project_id, relative_path: record.relative_path,
            base_revision: record.base_revision, buffer_revision: record.buffer_revision,
            draft_sha256: record.draft_sha256, draft_text: record.draft_text,
        })
    }
    pub fn forget(
        &self, project_id: &str, relative_path: &str, expected_draft_sha256: &str,
    ) -> Result<(), DraftRecoveryError> {
        let _lock = self.write_lock.lock().map_err(|_| DraftRecoveryError::HostUnavailable)?;
        let Some(record) = self.read_existing(project_id, relative_path)? else { return Ok(()) };
        if record.draft_sha256 != expected_draft_sha256 ||
            !valid_revision(expected_draft_sha256) {
            return Err(DraftRecoveryError::Stale);
        }
        self.dir.remove_file(file_name(project_id, relative_path))?;
        sync_dir(&self.dir)?;
        Ok(())
    }
}
impl IdeWorkspaceRegistry {
    pub fn check_recovery_project(&self, project_id: &str)
        -> Result<(), crate::browser::BrowserError> {
        self.source_capability(project_id).map(|_| ())
    }
}
