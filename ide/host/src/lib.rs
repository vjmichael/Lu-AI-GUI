//! IDE-01B: native, non-executing IDE project registration and lifecycle.
//!
//! Standalone project catalog and lifecycle. This crate does NOT depend on
//! Lu Core, perform tool dispatch, grant authority, or launch processes.
//! Project roots are obtained ONLY from a host-implemented native picker;
//! Protobuf UI messages never contain paths or grants.
#![forbid(unsafe_code)]

pub mod browser;
pub mod editor;
pub mod inbox;
pub mod proposal;
pub mod review;
pub mod source_span;
pub mod wire;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use wire::proto::{self, workspace_reply::Outcome, workspace_request::Operation};

const CATALOG_VERSION: u32 = 1;
const MAX_CATALOG_BYTES: u64 = 128 * 1024;
const MAX_PROJECTS: usize = wire::MAX_PROJECTS;

/// Implement this only in the trusted native host (e.g. Tauri OS dialog).
/// Never construct the selected directory from an LLM or WebView string.
pub trait NativeDirectoryPicker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError>;
}

#[derive(Debug)]
pub enum IdeError {
    Io(std::io::Error),
    Format(serde_json::Error),
    AlreadyActive,
    NotActive,
    Poisoned,
    InvalidSelection,
    InvalidCatalog,
    UnknownProject,
    Capacity,
}

impl IdeError {
    fn stable_code(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO_UNAVAILABLE",
            Self::Format(_) | Self::InvalidCatalog => "CATALOG_INVALID",
            Self::AlreadyActive => "PROJECT_ALREADY_ACTIVE",
            Self::NotActive => "PROJECT_NOT_ACTIVE",
            Self::Poisoned => "HOST_UNAVAILABLE",
            Self::InvalidSelection => "SELECTION_INVALID",
            Self::UnknownProject => "PROJECT_NOT_FOUND",
            Self::Capacity => "PROJECT_CAPACITY",
        }
    }
}
impl From<std::io::Error> for IdeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for IdeError {
    fn from(e: serde_json::Error) -> Self {
        Self::Format(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRecord {
    id: String,
    // Host-only metadata. Never sent through the IDE Protobuf UI boundary.
    root: PathBuf,
    display_name: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    projects: Vec<ProjectRecord>,
}

fn fresh_project_id() -> String {
    let mut random = [0_u8; 16];
    OsRng.fill_bytes(&mut random);
    let mut id = String::from("ide-");
    for byte in random {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

fn safe_root(path: &Path) -> Result<PathBuf, IdeError> {
    // Reject the user-selected root when it itself is a symlink/reparse point.
    // This is only a metadata gate, NOT a persistent filesystem capability.
    let metadata = fs::symlink_metadata(path).map_err(IdeError::Io)?;
    if !metadata.file_type().is_dir() {
        return Err(IdeError::InvalidSelection);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(IdeError::InvalidSelection);
        }
    }
    let canonical = fs::canonicalize(path)?;
    if canonical.parent().is_none() {
        return Err(IdeError::InvalidSelection);
    }
    let canonical_meta = fs::symlink_metadata(&canonical)?;
    if !canonical_meta.file_type().is_dir() {
        return Err(IdeError::InvalidSelection);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if canonical_meta.file_attributes() & 0x400 != 0 {
            return Err(IdeError::InvalidSelection);
        }
    }
    Ok(canonical)
}
fn display_name(root: &Path) -> String {
    let candidate = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Project");
    if candidate.is_empty() || candidate.len() > 255 || candidate.chars().any(char::is_control) {
        "Project".into()
    } else {
        candidate.to_owned()
    }
}
fn read_catalog(path: &Path) -> Result<BTreeMap<String, ProjectRecord>, IdeError> {
    let mut data = Vec::new();
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.file_type().is_file() || meta.len() > MAX_CATALOG_BYTES {
                return Err(IdeError::InvalidCatalog);
            }
            File::open(path)?
                .take(MAX_CATALOG_BYTES + 1)
                .read_to_end(&mut data)?;
            if data.len() as u64 > MAX_CATALOG_BYTES {
                return Err(IdeError::InvalidCatalog);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(IdeError::Io(e)),
    }
    let catalog: Catalog = serde_json::from_slice(&data)?;
    if catalog.version != CATALOG_VERSION || catalog.projects.len() > MAX_PROJECTS {
        return Err(IdeError::InvalidCatalog);
    }
    let mut records = BTreeMap::new();
    for record in catalog.projects {
        if !wire::valid_project_id(&record.id)
            || !record.root.is_absolute()
            || record.root.parent().is_none()
            || record.display_name.is_empty()
            || record.display_name.len() > 255
            || record.display_name.chars().any(char::is_control)
            || records.insert(record.id.clone(), record).is_some()
        {
            return Err(IdeError::InvalidCatalog);
        }
    }
    Ok(records)
}

/// Atomic replacement of host-only metadata. This is NOT an executable effect
/// journal; future Lu integration must use a separately authenticated bridge.
fn save_catalog(path: &Path, records: &BTreeMap<String, ProjectRecord>) -> Result<(), IdeError> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.file_type().is_file() {
            return Err(IdeError::InvalidCatalog);
        }
    }
    let bytes = serde_json::to_vec_pretty(&Catalog {
        version: CATALOG_VERSION,
        projects: records.values().cloned().collect(),
    })?;
    if bytes.len() as u64 > MAX_CATALOG_BYTES {
        return Err(IdeError::Capacity);
    }
    let parent = path.parent().ok_or(IdeError::InvalidCatalog)?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".lu-ide-{}.tmp", fresh_project_id()));
    let result = (|| -> Result<(), IdeError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Standalone project metadata catalog. This only records the user's selected
/// directories and currently opened project IDs; it creates no execution rights.
/// Lock ordering: records -> active for every operation that uses both.
pub struct IdeWorkspaceRegistry {
    catalog: PathBuf,
    records: Mutex<BTreeMap<String, ProjectRecord>>,
    active: Mutex<BTreeSet<String>>,
    // Serialize IDE-origin writes. External edits are guarded by revisions.
    save_lock: Mutex<()>,
}

impl IdeWorkspaceRegistry {
    /// The catalog is located under this app's native user-data directory,
    /// selected by the trusted desktop process rather than the WebView.
    pub fn open(catalog: PathBuf) -> Result<Self, IdeError> {
        let records = read_catalog(&catalog)?;
        Ok(Self {
            catalog,
            records: Mutex::new(records),
            active: Mutex::new(BTreeSet::new()),
            save_lock: Mutex::new(()),
        })
    }

    pub fn register_from_native_picker(
        &self,
        picker: &mut dyn NativeDirectoryPicker,
    ) -> Result<Option<proto::ProjectView>, IdeError> {
        let selected = match picker.choose_project_directory()? {
            Some(path) => path,
            None => return Ok(None),
        };
        let root = safe_root(&selected)?;
        let mut records = self.records.lock().map_err(|_| IdeError::Poisoned)?;
        if records.len() >= MAX_PROJECTS {
            return Err(IdeError::Capacity);
        }
        // Duplicate roots are not silently registered under different IDs.
        if records.values().any(|r| r.root == root) {
            return Err(IdeError::InvalidSelection);
        }
        let id = (0..8)
            .map(|_| fresh_project_id())
            .find(|id| !records.contains_key(id))
            .ok_or(IdeError::Capacity)?;
        let record = ProjectRecord {
            display_name: display_name(&root),
            id: id.clone(),
            root,
        };
        let mut next = records.clone();
        next.insert(id.clone(), record.clone());
        save_catalog(&self.catalog, &next)?;
        let mut active = self.active.lock().map_err(|_| IdeError::Poisoned)?;
        if !active.insert(id.clone()) {
            return Err(IdeError::AlreadyActive);
        }
        *records = next;
        Ok(Some(proto::ProjectView {
            project_id: id,
            display_name: record.display_name,
            active: true,
            available: true,
        }))
    }

    /// Revalidates path shape before reopening a saved record. No project code
    /// executes and no permission to read/write files is created by reopening.
    pub fn reopen(&self, id: &str) -> Result<proto::ProjectView, IdeError> {
        if !wire::valid_project_id(id) {
            return Err(IdeError::UnknownProject);
        }
        let records = self.records.lock().map_err(|_| IdeError::Poisoned)?;
        let record = records.get(id).ok_or(IdeError::UnknownProject)?;
        if safe_root(&record.root)? != record.root {
            return Err(IdeError::InvalidSelection);
        }
        let mut active = self.active.lock().map_err(|_| IdeError::Poisoned)?;
        if !active.insert(id.to_owned()) {
            return Err(IdeError::AlreadyActive);
        }
        Ok(proto::ProjectView {
            project_id: id.to_owned(),
            display_name: record.display_name.clone(),
            active: true,
            available: true,
        })
    }

    pub fn close(&self, id: &str) -> Result<(), IdeError> {
        let records = self.records.lock().map_err(|_| IdeError::Poisoned)?;
        if !records.contains_key(id) {
            return Err(IdeError::UnknownProject);
        }
        let mut active = self.active.lock().map_err(|_| IdeError::Poisoned)?;
        if !active.remove(id) {
            return Err(IdeError::NotActive);
        }
        Ok(())
    }

    /// Remove a project from the IDE's registered catalog. This is *never*
    /// filesystem deletion: no project root or file is opened, removed or
    /// moved here. The user may re-register the directory with the OS picker.
    ///
    /// Serialize with IDE saves so an in-progress write finishes before its
    /// registration is revoked. Persistent catalog replacement must succeed
    /// before the live record and active state are changed.
    pub fn unregister(&self, id: &str) -> Result<(), IdeError> {
        if !wire::valid_project_id(id) {
            return Err(IdeError::UnknownProject);
        }
        let _save = self.save_lock.lock().map_err(|_| IdeError::Poisoned)?;
        let mut records = self.records.lock().map_err(|_| IdeError::Poisoned)?;
        if !records.contains_key(id) {
            return Err(IdeError::UnknownProject);
        }
        let mut active = self.active.lock().map_err(|_| IdeError::Poisoned)?;
        let mut next = records.clone();
        next.remove(id);
        save_catalog(&self.catalog, &next)?;
        *records = next;
        active.remove(id);
        Ok(())
    }

    pub fn projects(&self) -> Result<Vec<proto::ProjectView>, IdeError> {
        let records = self.records.lock().map_err(|_| IdeError::Poisoned)?;
        let active = self.active.lock().map_err(|_| IdeError::Poisoned)?;
        Ok(records
            .values()
            .map(|r| proto::ProjectView {
                project_id: r.id.clone(),
                display_name: r.display_name.clone(),
                active: active.contains(&r.id),
                available: safe_root(&r.root).is_ok_and(|canonical| canonical == r.root),
            })
            .collect())
    }

    /// Only the native UI host should call this. These messages have no path
    /// or executable effect fields; workers and model tools never use this API.
    pub fn handle_ui_request(
        &self,
        frame: &[u8],
        picker: &mut dyn NativeDirectoryPicker,
    ) -> Result<Vec<u8>, wire::WireError> {
        let request = wire::decode_request(frame)?;
        let outcome: Result<Outcome, IdeError> = match request.operation.expect("validated") {
            Operation::List(_) => self
                .projects()
                .map(|projects| Outcome::Projects(proto::ProjectList { projects })),
            Operation::Select(_) => self
                .register_from_native_picker(picker)
                .and_then(|project| match project {
                    Some(project) => Ok(Outcome::Project(project)),
                    None => self
                        .projects()
                        .map(|projects| Outcome::Projects(proto::ProjectList { projects })),
                }),
            Operation::Reopen(op) => self.reopen(&op.project_id).map(Outcome::Project),
            Operation::Close(op) => self.close(&op.project_id).and_then(|()| {
                self.projects()
                    .map(|projects| Outcome::Projects(proto::ProjectList { projects }))
            }),
            Operation::Unregister(op) => self.unregister(&op.project_id).and_then(|()| {
                self.projects()
                    .map(|projects| Outcome::Projects(proto::ProjectList { projects }))
            }),
        };
        let reply = proto::WorkspaceReply {
            protocol_version: wire::VERSION,
            request_id: request.request_id,
            outcome: Some(outcome.unwrap_or_else(|error| {
                Outcome::Failure(proto::WorkspaceFailure {
                    code: error.stable_code().into(),
                })
            })),
        };
        wire::encode_reply(&reply)
    }
}
