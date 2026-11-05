//! Host-owned history of verified byte observations from a bounded project scan.
//! Capturing a baseline is an explicit local UI action. It does not run files,
//! grant permissions, query live GitHub or claim installers were launched.
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::IdeWorkspaceRegistry;

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_BASELINES: usize = 64;

#[derive(Debug)]
pub enum SnapshotError {
    Unavailable,
    Corrupt,
    TooLarge,
    Incomplete,
    ProjectUnavailable,
}
impl SnapshotError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable => "SNAPSHOT_UNAVAILABLE",
            Self::Corrupt => "SNAPSHOT_CORRUPT",
            Self::TooLarge => "SNAPSHOT_TOO_LARGE",
            Self::Incomplete => "INVENTORY_INCOMPLETE",
            Self::ProjectUnavailable => "PROJECT_NOT_OPEN",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredArtifact {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub digest_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InventoryBaseline {
    pub project_id: String,
    pub observed_unix_seconds: u64,
    pub scope: String,
    pub artifacts: Vec<StoredArtifact>,
    pub local_head: Option<String>,
    pub local_tracking_head: Option<String>,
    #[serde(default)]
    pub git_worktree_state: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactComparison {
    pub relative_path: String,
    pub baseline_sha256: Option<String>,
    pub current_sha256: Option<String>,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryComparison {
    pub schema_version: u32,
    pub project_id: String,
    pub baseline_present: bool,
    pub baseline_count: usize,
    pub baseline_unix_seconds: Option<u64>,
    pub discovery_complete: bool,
    pub entries: Vec<ArtifactComparison>,
    pub local_git_head_change: &'static str,
    pub git_worktree_change: &'static str,
    pub live_remote: &'static str,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotFile {
    schema_version: u32,
    baselines: Vec<InventoryBaseline>,
}

pub struct InventorySnapshots {
    path: PathBuf,
    lock: Mutex<()>,
}
impl InventorySnapshots {
    pub fn new(path: PathBuf) -> Result<Self, SnapshotError> {
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(SnapshotError::Corrupt);
            }
        }
        Ok(Self { path, lock: Mutex::new(()) })
    }

    fn read(&self) -> Result<Vec<InventoryBaseline>, SnapshotError> {
        let meta = match fs::symlink_metadata(&self.path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(SnapshotError::Unavailable),
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(SnapshotError::Corrupt);
        }
        if meta.len() > MAX_BYTES {
            return Err(SnapshotError::TooLarge);
        }
        let mut bytes = Vec::new();
        File::open(&self.path).map_err(|_| SnapshotError::Unavailable)?
            .take(MAX_BYTES + 1).read_to_end(&mut bytes)
            .map_err(|_| SnapshotError::Unavailable)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(SnapshotError::TooLarge);
        }
        let doc: SnapshotFile =
            serde_json::from_slice(&bytes).map_err(|_| SnapshotError::Corrupt)?;
        if doc.schema_version != 1 || doc.baselines.len() > MAX_BASELINES
            || doc.baselines.iter().any(|b| b.artifacts.len() > 80)
        {
            return Err(SnapshotError::Corrupt);
        }
        Ok(doc.baselines)
    }

    fn persist(&self, baselines: Vec<InventoryBaseline>) -> Result<(), SnapshotError> {
        let data = serde_json::to_vec(&SnapshotFile {
            schema_version: 1, baselines,
        }).map_err(|_| SnapshotError::Unavailable)?;
        if data.len() as u64 > MAX_BYTES {
            return Err(SnapshotError::TooLarge);
        }
        let parent = self.path.parent().ok_or(SnapshotError::Unavailable)?;
        fs::create_dir_all(parent).map_err(|_| SnapshotError::Unavailable)?;
        let tmp = parent.join(format!(".lu-ide-evidence-{:032x}.tmp", rand::random::<u128>()));
        let result = (|| {
            let mut file = OpenOptions::new().write(true).create_new(true)
                .open(&tmp).map_err(|_| SnapshotError::Unavailable)?;
            file.write_all(&data).map_err(|_| SnapshotError::Unavailable)?;
            file.sync_all().map_err(|_| SnapshotError::Unavailable)?;
            fs::rename(&tmp, &self.path).map_err(|_| SnapshotError::Unavailable)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    /// Store exact observed hashes and locations; never claim usability.
    pub fn capture(&self, registry: &IdeWorkspaceRegistry, project_id: &str)
        -> Result<InventoryBaseline, SnapshotError>
    {
        let inventory = registry.inspect_project_evidence(project_id)
            .map_err(|_| SnapshotError::ProjectUnavailable)?;
        if !inventory.discovery_complete {
            return Err(SnapshotError::Incomplete);
        }
        let git = registry.inspect_git_worktree(project_id);
        let observed_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH).map_err(|_| SnapshotError::Unavailable)?
            .as_secs();
        let baseline = InventoryBaseline {
            project_id: project_id.to_owned(),
            observed_unix_seconds,
            scope: inventory.scope.to_owned(),
            artifacts: inventory.files.into_iter().map(|f| StoredArtifact {
                relative_path: f.relative_path,
                size_bytes: f.size_bytes,
                sha256: f.sha256,
                digest_status: f.digest_status.to_owned(),
            }).collect(),
            local_head: git.local_head,
            local_tracking_head: git.upstream_head,
            git_worktree_state: if git.worktree == "not_verified" { None }
                else { Some(git.worktree.to_owned()) },
        };
        let _guard = self.lock.lock().map_err(|_| SnapshotError::Unavailable)?;
        let mut baselines = self.read()?;
        // Preserve previous observations; never silently replace history.
        if baselines.len() >= MAX_BASELINES {
            return Err(SnapshotError::TooLarge);
        }
        baselines.push(baseline.clone());
        self.persist(baselines)?;
        Ok(baseline)
    }

    /// Compare an earlier host snapshot with currently inspected bytes. An
    /// incomplete scan never permits a false MISSING claim.
    pub fn compare(&self, registry: &IdeWorkspaceRegistry, project_id: &str)
        -> Result<InventoryComparison, SnapshotError>
    {
        let current = registry.inspect_project_evidence(project_id)
            .map_err(|_| SnapshotError::ProjectUnavailable)?;
        let git = registry.inspect_git_worktree(project_id);
        let _guard = self.lock.lock().map_err(|_| SnapshotError::Unavailable)?;
        let recorded = self.read()?;
        let baseline_count = recorded.iter().filter(|b| b.project_id == project_id).count();
        let prior = recorded.into_iter().rev().find(|b| b.project_id == project_id);
        let mut entries = Vec::new();
        let mut git_state = "baseline_missing";
        let mut worktree_state = "baseline_missing";
        let mut baseline_time = None;
        if let Some(ref baseline) = prior {
            baseline_time = Some(baseline.observed_unix_seconds);
            git_state = match (&baseline.local_head, &git.local_head) {
                (Some(a), Some(b)) if a == b => "same_observed_head",
                (Some(_), Some(_)) => "different_observed_head",
                _ => "unverifiable",
            };
            worktree_state = match (baseline.git_worktree_state.as_deref(), git.worktree) {
                (Some("clean"), "clean") => "remained_clean",
                (Some("clean"), "dirty") => "became_dirty",
                (Some("dirty"), "clean") => "became_clean",
                (Some("dirty"), "dirty") => "both_dirty_not_equivalent",
                _ => "unverifiable",
            };
            let current_files: BTreeMap<_, _> = current.files.iter()
                .map(|f| (f.relative_path.as_str(), f)).collect();
            for old in &baseline.artifacts {
                let found = current_files.get(old.relative_path.as_str());
                let status = match found {
                    None if current.discovery_complete => "missing_in_scanned_scope",
                    None => "unverifiable",
                    Some(file) if old.sha256.is_some()
                        && old.sha256 == file.sha256 => "matched_bytes",
                    Some(file) if old.sha256.is_some()
                        && file.sha256.is_some() => "changed_bytes",
                    Some(file) if old.size_bytes != file.size_bytes => "changed_size_only",
                    _ => "present_unverifiable",
                };
                entries.push(ArtifactComparison {
                    relative_path: old.relative_path.clone(),
                    baseline_sha256: old.sha256.clone(),
                    current_sha256: found.and_then(|f| f.sha256.clone()),
                    status,
                });
            }
            for file in &current.files {
                if !baseline.artifacts.iter().any(|old|
                    old.relative_path == file.relative_path)
                {
                    entries.push(ArtifactComparison {
                        relative_path: file.relative_path.clone(),
                        baseline_sha256: None,
                        current_sha256: file.sha256.clone(),
                        status: "new_in_scanned_scope",
                    });
                }
            }
        }
        Ok(InventoryComparison {
            schema_version: 1,
            project_id: project_id.to_owned(),
            baseline_present: prior.is_some(),
            baseline_count,
            baseline_unix_seconds: baseline_time,
            discovery_complete: current.discovery_complete,
            entries,
            local_git_head_change: git_state,
            git_worktree_change: worktree_state,
            live_remote: "not_queried",
        })
    }
}
