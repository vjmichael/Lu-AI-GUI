//! Wave A3: host-owned development events and bounded project continuity.
//! Records are operational history, NOT effect-journal receipts, permissions,
//! user-approved project canon, or verification of external resources.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    browser::{validate_relative, BrowserError},
    git_status::GitWorktreeInspection,
    inventory::EvidenceInventory,
    IdeWorkspaceRegistry,
};

const MAX_LEDGER_BYTES: u64 = 2 * 1024 * 1024;
const MAX_EVENTS: usize = 40;
const MAX_NOTE_CHARS: usize = 240;
const MAX_EXCERPT_CHARS: usize = 1200;
const CONTEXT_FILES: [&str; 12] = [
    "PROJECT_STATE.md",
    "CHANGELOG.md",
    "ENGINEERING_HANDOFF.md",
    "AGENTS.md",
    "ide/PROJECT_STATE.md",
    "ide/CHANGELOG.md",
    "ide/ENGINEERING_HANDOFF.md",
    "ide/AGENTS.md",
    "benchmarks/README.md",
    "benchmarks/manifest.json",
    "benchmarks/results.json",
    "benchmarks/benchmarks.json",
];

#[derive(Debug)]
pub enum ContinuityError {
    InvalidInput,
    Unavailable,
    Corrupt,
    Full,
    ProjectUnavailable,
}
impl ContinuityError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_CONTINUITY_REQUEST",
            Self::Unavailable => "CONTINUITY_UNAVAILABLE",
            Self::Corrupt => "CONTINUITY_RECORDS_CORRUPT",
            Self::Full => "CONTINUITY_LEDGER_FULL",
            Self::ProjectUnavailable => "PROJECT_NOT_OPEN",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevelopmentEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub observed_unix_seconds: u64,
    pub project_id: String,
    pub operation: String,
    pub outcome: String,
    pub source_revision: Option<String>,
    pub source_path: Option<String>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDocument {
    pub relative_path: &'static str,
    pub availability: &'static str,
    pub trust: &'static str,
    pub source_revision: Option<String>,
    pub byte_length: Option<u64>,
    pub excerpt: Option<String>,
    pub excerpt_truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub schema_version: u32,
    pub project_id: String,
    pub project_name: String,
    pub source: &'static str,
    pub remote_git_state: &'static str,
    pub local_artifacts: &'static str,
    pub current_toolchain_readiness: &'static str,
    pub events: Vec<DevelopmentEvent>,
    pub documents: Vec<ContextDocument>,
    pub inventory: EvidenceInventory,
    pub git_worktree: GitWorktreeInspection,
}

pub struct DevelopmentLedger {
    path: PathBuf,
    lock: Mutex<()>,
}

impl DevelopmentLedger {
    /// Application-profile path is selected by the trusted desktop, not by UI
    /// input or project data. Nothing is written into a user's source project.
    pub fn new(path: PathBuf) -> Result<Self, ContinuityError> {
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(ContinuityError::Corrupt);
            }
        }
        Ok(Self {
            path,
            lock: Mutex::new(()),
        })
    }

    fn events(&self) -> Result<Vec<DevelopmentEvent>, ContinuityError> {
        let _guard = self.lock.lock().map_err(|_| ContinuityError::Unavailable)?;
        let meta = match fs::symlink_metadata(&self.path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(ContinuityError::Unavailable),
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(ContinuityError::Corrupt);
        }
        if meta.len() > MAX_LEDGER_BYTES {
            return Err(ContinuityError::Full);
        }
        let mut data = Vec::new();
        File::open(&self.path)
            .map_err(|_| ContinuityError::Unavailable)?
            .take(MAX_LEDGER_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|_| ContinuityError::Unavailable)?;
        if data.len() as u64 > MAX_LEDGER_BYTES {
            return Err(ContinuityError::Full);
        }
        if !data.is_empty() && !data.ends_with(b"\n") {
            return Err(ContinuityError::Corrupt);
        }
        let mut result = Vec::new();
        for line in data.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let event: DevelopmentEvent =
                serde_json::from_slice(line).map_err(|_| ContinuityError::Corrupt)?;
            if event.schema_version != 1 {
                return Err(ContinuityError::Corrupt);
            }
            result.push(event);
        }
        Ok(result)
    }

    /// Trusted host call after an actual action; never offered as a Tauri
    /// command. A record of success is historical, not proof of present state.
    pub fn record(
        &self,
        project_id: &str,
        operation: &str,
        summary: &str,
        source_revision: Option<&str>,
        source_path: Option<&str>,
    ) -> Result<(), ContinuityError> {
        const ALLOWED: [&str; 5] = [
            "project.register",
            "project.open",
            "project.close",
            "project.unregister",
            "source.save",
        ];
        if project_id.len() > 128
            || project_id.is_empty()
            || !project_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || !ALLOWED.contains(&operation)
            || summary.chars().count() > MAX_NOTE_CHARS
            || summary.chars().any(char::is_control)
            || source_path.is_some_and(|p| validate_relative(p, false).is_err())
            || source_revision
                .is_some_and(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(ContinuityError::InvalidInput);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ContinuityError::Unavailable)?
            .as_secs();
        let event = DevelopmentEvent {
            schema_version: 1,
            event_id: format!("evt-{:032x}", rand::random::<u128>()),
            observed_unix_seconds: now,
            project_id: project_id.to_owned(),
            operation: operation.to_owned(),
            outcome: "observed_host_success".to_owned(),
            source_revision: source_revision.map(ToOwned::to_owned),
            source_path: source_path.map(ToOwned::to_owned),
            summary: summary.to_owned(),
        };
        let mut bytes = serde_json::to_vec(&event).map_err(|_| ContinuityError::Unavailable)?;
        bytes.push(b'\n');
        let _guard = self.lock.lock().map_err(|_| ContinuityError::Unavailable)?;
        if let Ok(meta) = fs::symlink_metadata(&self.path) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(ContinuityError::Corrupt);
            }
            if meta.len().saturating_add(bytes.len() as u64) > MAX_LEDGER_BYTES {
                return Err(ContinuityError::Full);
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| ContinuityError::Unavailable)?;
        file.write_all(&bytes)
            .map_err(|_| ContinuityError::Unavailable)?;
        file.sync_all().map_err(|_| ContinuityError::Unavailable)?;
        Ok(())
    }

    /// Return task orientation, not execution readiness. Project-authored
    /// documents are untrusted evidence with exact saved revisions.
    pub fn context(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
    ) -> Result<ProjectContext, ContinuityError> {
        let project = registry
            .projects()
            .map_err(|_| ContinuityError::Unavailable)?
            .into_iter()
            .find(|p| p.project_id == project_id && p.active)
            .ok_or(ContinuityError::ProjectUnavailable)?;
        let all = self.events()?;
        let events = all
            .into_iter()
            .rev()
            .filter(|event| event.project_id == project_id)
            .take(MAX_EVENTS)
            .collect();
        let mut documents = Vec::new();
        for name in CONTEXT_FILES {
            let read = registry.preview_source_file(project_id, name);
            let doc = match read {
                Ok(preview) => {
                    let excerpt: String = preview.text.chars().take(MAX_EXCERPT_CHARS).collect();
                    ContextDocument {
                        relative_path: name,
                        availability: "present_unverified",
                        trust: "untrusted_project_text",
                        source_revision: Some(preview.revision),
                        byte_length: Some(preview.byte_length),
                        excerpt_truncated: preview.text.chars().count() > MAX_EXCERPT_CHARS,
                        excerpt: Some(excerpt),
                    }
                }
                Err(BrowserError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                    ContextDocument {
                        relative_path: name,
                        availability: "missing",
                        trust: "untrusted_project_text",
                        source_revision: None,
                        byte_length: None,
                        excerpt: None,
                        excerpt_truncated: false,
                    }
                }
                Err(_) => ContextDocument {
                    relative_path: name,
                    availability: "unverifiable",
                    trust: "untrusted_project_text",
                    source_revision: None,
                    byte_length: None,
                    excerpt: None,
                    excerpt_truncated: false,
                },
            };
            documents.push(doc);
        }
        let git_worktree = registry.inspect_git_worktree(project_id);
        let inventory = registry
            .inspect_project_evidence(project_id)
            .map_err(|_| ContinuityError::Unavailable)?;
        Ok(ProjectContext {
            schema_version: 1,
            project_id: project_id.to_owned(),
            project_name: project.display_name,
            source: "local_standalone_host",
            remote_git_state: "not_queried",
            local_artifacts: "bounded_scan",
            current_toolchain_readiness: "not_verified",
            events,
            documents,
            inventory,
            git_worktree,
        })
    }
}
