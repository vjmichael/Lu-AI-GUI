//! D1: non-executing, host-owned job intent protocol.
//! PREPARATION AND HUMAN REVIEW ARE NOT EXECUTION AUTHORITY.
//! No executable spawn path or reusable grant exists in this module.
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::IdeWorkspaceRegistry;

const JOURNAL_LIMIT_BYTES: u64 = 256 * 1024;
const MAX_JOURNAL_EVENTS: usize = 128;
const MAX_INTENTS_PER_PROJECT: usize = 16;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const INTENT_TTL_SECS: u64 = 120;

#[derive(Debug)]
pub enum IntentError {
    Invalid,
    ProjectUnavailable,
    SourceDirty,
    ManifestUnavailable,
    JournalUnavailable,
    JournalCorrupt,
    Capacity,
    NotFound,
    Stale,
}
impl IntentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "INTENT_INVALID_REQUEST",
            Self::ProjectUnavailable => "INTENT_PROJECT_UNAVAILABLE",
            Self::SourceDirty => "INTENT_REQUIRES_CLEAN_GIT_HEAD",
            Self::ManifestUnavailable => "INTENT_MANIFEST_UNAVAILABLE",
            Self::JournalUnavailable => "INTENT_JOURNAL_UNAVAILABLE",
            Self::JournalCorrupt => "INTENT_JOURNAL_CORRUPT",
            Self::Capacity => "INTENT_HISTORY_FULL",
            Self::NotFound => "INTENT_NOT_FOUND",
            Self::Stale => "INTENT_SOURCE_OR_SESSION_STALE",
        }
    }
}

#[derive(Clone, Debug)]
struct Recipe {
    id: &'static str,
    manifest: &'static str,
    display: &'static str,
    proposed_action: &'static str,
}
const RECIPES: [Recipe; 3] = [
    Recipe {
        id: "rust_check",
        manifest: "Cargo.toml",
        display: "Rust project type-check",
        proposed_action: "cargo check --locked --manifest-path Cargo.toml",
    },
    Recipe {
        id: "rust_tests",
        manifest: "Cargo.toml",
        display: "Rust project tests",
        proposed_action: "cargo test --locked --manifest-path Cargo.toml",
    },
    Recipe {
        id: "lu_ide_host_tests",
        manifest: "ide/host/Cargo.toml",
        display: "Lu IDE host tests",
        proposed_action: "cargo test --locked --manifest-path ide/host/Cargo.toml",
    },
];

fn recipe(id: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|value| value.id == id)
}
fn valid_sha(value: &str, digits: usize) -> bool {
    value.len() == digits && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn valid_project(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'-')
}
fn now() -> Result<u64, IntentError> {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| IntentError::JournalUnavailable)
}
fn new_id() -> String {
    format!("intent-{:032x}", rand::random::<u128>())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IntentRecord {
    intent_id: String,
    project_id: String,
    recipe_id: String,
    manifest_relative_path: String,
    manifest_sha256: String,
    source_head: String,
    worktree: String,
    created_unix_seconds: u64,
    expires_unix_seconds: u64,
    session_id: String,
    status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IntentEvent {
    schema_version: u32,
    event_id: String,
    observed_unix_seconds: u64,
    intent: IntentRecord,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentSummary {
    pub intent_id: String,
    pub project_id: String,
    pub recipe_id: String,
    pub display_name: &'static str,
    pub proposed_action: &'static str,
    pub manifest_relative_path: String,
    pub manifest_sha256: String,
    pub source_head: String,
    pub worktree: String,
    pub created_unix_seconds: u64,
    pub expires_unix_seconds: u64,
    pub state: &'static str,
    pub can_record_review: bool,
    pub execution: &'static str,
    pub execution_authority: &'static str,
    pub worker_containment: &'static str,
    pub grant_token_issued: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentList {
    pub schema_version: u32,
    pub project_id: String,
    pub source: &'static str,
    pub entries: Vec<IntentSummary>,
    pub worker_execution_enabled: bool,
    pub installation_acceptance: &'static str,
}

fn safe_manifest(root: &Path, relative: &str) -> Result<String, IntentError> {
    // Fixed relative recipe paths only, not caller supplied paths.
    if !RECIPES.iter().any(|candidate| candidate.manifest == relative) {
        return Err(IntentError::Invalid);
    }
    let mut file_path = root.to_owned();
    for part in relative.split('/') {
        file_path.push(part);
        let meta = fs::symlink_metadata(&file_path)
            .map_err(|_| IntentError::ManifestUnavailable)?;
        if meta.file_type().is_symlink() {
            return Err(IntentError::ManifestUnavailable);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(IntentError::ManifestUnavailable);
            }
        }
    }
    let meta = fs::symlink_metadata(&file_path)
        .map_err(|_| IntentError::ManifestUnavailable)?;
    if !meta.is_file() || meta.len() > MAX_MANIFEST_BYTES {
        return Err(IntentError::ManifestUnavailable);
    }
    if fs::canonicalize(&file_path).ok().as_ref()
        != Some(&root.join(relative)) {
        return Err(IntentError::ManifestUnavailable);
    }
    let mut bytes = Vec::new();
    File::open(&file_path)
        .map_err(|_| IntentError::ManifestUnavailable)?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| IntentError::ManifestUnavailable)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(IntentError::ManifestUnavailable);
    }
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn check_source(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    candidate: &Recipe,
) -> Result<(String, String), IntentError> {
    let root = registry.registered_git_root(project_id)
        .map_err(|_| IntentError::ProjectUnavailable)?;
    let git = registry.inspect_git_worktree(project_id);
    if git.state != "local_repository_inspected"
        || git.worktree != "clean"
        || git.modified_or_staged_count != 0
        || git.untracked_count != 0
        || git.truncated
        || !git.local_head.as_deref().is_some_and(|sha| valid_sha(sha, 40)) {
        return Err(IntentError::SourceDirty);
    }
    let hash = safe_manifest(&root, candidate.manifest)?;
    Ok((git.local_head.unwrap(), hash))
}

fn valid_record(record: &IntentRecord) -> bool {
    let Some(candidate) = recipe(&record.recipe_id) else { return false };
    record.intent_id.starts_with("intent-")
        && valid_sha(record.intent_id.trim_start_matches("intent-"), 32)
        && valid_project(&record.project_id)
        && record.manifest_relative_path == candidate.manifest
        && valid_sha(&record.manifest_sha256, 64)
        && valid_sha(&record.source_head, 40)
        && record.worktree == "clean"
        && record.session_id.starts_with("session-")
        && valid_sha(record.session_id.trim_start_matches("session-"), 32)
        && record.expires_unix_seconds >= record.created_unix_seconds
        && record.expires_unix_seconds - record.created_unix_seconds == INTENT_TTL_SECS
        && matches!(
            record.status.as_str(),
            "prepared" | "review_recorded_no_execution" | "cancelled" | "review_withdrawn"
        )
}
fn allowed_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("prepared", "review_recorded_no_execution")
            | ("prepared", "cancelled")
            | ("review_recorded_no_execution", "review_withdrawn")
    )
}
fn snapshot(
    latest: &IntentRecord,
    session: &str,
    observed_now: u64,
) -> Result<IntentSummary, IntentError> {
    let Some(candidate) = recipe(&latest.recipe_id) else {
        return Err(IntentError::JournalCorrupt);
    };
    let presentation = if latest.status == "prepared" && latest.session_id != session {
        "requires_new_session_review"
    } else if latest.status == "prepared" && observed_now > latest.expires_unix_seconds {
        "expired_without_execution"
    } else if latest.status == "prepared" {
        "prepared_not_authorized"
    } else if latest.status == "review_recorded_no_execution" {
        "human_review_recorded_no_execution"
    } else if latest.status == "review_withdrawn" {
        "human_review_withdrawn"
    } else {
        "cancelled_before_execution"
    };
    Ok(IntentSummary {
        intent_id: latest.intent_id.clone(),
        project_id: latest.project_id.clone(),
        recipe_id: latest.recipe_id.clone(),
        display_name: candidate.display,
        proposed_action: candidate.proposed_action,
        manifest_relative_path: latest.manifest_relative_path.clone(),
        manifest_sha256: latest.manifest_sha256.clone(),
        source_head: latest.source_head.clone(),
        worktree: latest.worktree.clone(),
        created_unix_seconds: latest.created_unix_seconds,
        expires_unix_seconds: latest.expires_unix_seconds,
        state: presentation,
        can_record_review: presentation == "prepared_not_authorized",
        execution: "never_started",
        execution_authority: "not_issued",
        worker_containment: "not_implemented",
        grant_token_issued: false,
    })
}

pub struct ExecutionIntents {
    path: PathBuf,
    session_id: String,
    lock: Mutex<()>,
}

impl ExecutionIntents {
    pub fn new(path: PathBuf) -> Result<Self, IntentError> {
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(IntentError::JournalCorrupt);
            }
        }
        Ok(Self {
            path, session_id: format!("session-{:032x}", rand::random::<u128>()),
            lock: Mutex::new(()),
        })
    }

    fn events_locked(&self) -> Result<Vec<IntentEvent>, IntentError> {
        let meta = match fs::symlink_metadata(&self.path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(IntentError::JournalUnavailable),
        };
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > JOURNAL_LIMIT_BYTES {
            return Err(IntentError::JournalCorrupt);
        }
        let mut bytes = Vec::new();
        File::open(&self.path)
            .map_err(|_| IntentError::JournalUnavailable)?
            .take(JOURNAL_LIMIT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| IntentError::JournalUnavailable)?;
        if bytes.len() as u64 > JOURNAL_LIMIT_BYTES
            || (!bytes.is_empty() && !bytes.ends_with(b"\n")) {
            return Err(IntentError::JournalCorrupt);
        }
        let mut result = Vec::new();
        for line in bytes.split(|ch| *ch == b'\n').filter(|line| !line.is_empty()) {
            let event: IntentEvent = serde_json::from_slice(line)
                .map_err(|_| IntentError::JournalCorrupt)?;
            if event.schema_version != 1
                || !valid_record(&event.intent)
                || event.event_id.len() != 36
                || !event.event_id.starts_with("evt-")
                || !valid_sha(&event.event_id[4..], 32) {
                return Err(IntentError::JournalCorrupt);
            }
            result.push(event);
        }
        if result.len() > MAX_JOURNAL_EVENTS { return Err(IntentError::JournalCorrupt); }
        // Audit all transitions before returning any apparent current state.
        self.project_state(&result)?;
        Ok(result)
    }

    fn project_state(
        &self,
        events: &[IntentEvent],
    ) -> Result<BTreeMap<String, IntentRecord>, IntentError> {
        let mut current = BTreeMap::new();
        let mut ids = HashSet::new();
        for event in events {
            if !ids.insert(event.event_id.as_str()) {
                return Err(IntentError::JournalCorrupt);
            }
            let next = &event.intent;
            if let Some(old) = current.get(&next.intent_id) {
                let old: &IntentRecord = old;
                let mut expected = old.clone();
                expected.status = next.status.clone();
                if expected != *next || !allowed_transition(&old.status, &next.status) {
                    return Err(IntentError::JournalCorrupt);
                }
            } else if next.status != "prepared" {
                return Err(IntentError::JournalCorrupt);
            }
            current.insert(next.intent_id.clone(), next.clone());
        }
        Ok(current)
    }

    fn append_locked(
        &self,
        events: &[IntentEvent],
        next: IntentRecord,
    ) -> Result<IntentSummary, IntentError> {
        if events.len() >= MAX_JOURNAL_EVENTS { return Err(IntentError::Capacity); }
        let at = now()?;
        let current = self.project_state(events)?;
        let record = IntentEvent {
            schema_version: 1,
            event_id: format!("evt-{:032x}", rand::random::<u128>()),
            observed_unix_seconds: at,
            intent: next.clone(),
        };
        let mut all = events.to_vec();
        all.push(record.clone());
        self.project_state(&all)?;
        let mut bytes = serde_json::to_vec(&record)
            .map_err(|_| IntentError::JournalUnavailable)?;
        bytes.push(b'\n');
        let len = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if len.saturating_add(bytes.len() as u64) > JOURNAL_LIMIT_BYTES {
            return Err(IntentError::Capacity);
        }
        let mut file = OpenOptions::new().append(true).create(true)
            .open(&self.path).map_err(|_| IntentError::JournalUnavailable)?;
        file.write_all(&bytes).map_err(|_| IntentError::JournalUnavailable)?;
        file.sync_all().map_err(|_| IntentError::JournalUnavailable)?;
        let _ = current;
        snapshot(&next, &self.session_id, at)
    }

    pub fn list(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
    ) -> Result<IntentList, IntentError> {
        registry.registered_git_root(project_id)
            .map_err(|_| IntentError::ProjectUnavailable)?;
        let _guard = self.lock.lock().map_err(|_| IntentError::JournalUnavailable)?;
        let events = self.events_locked()?;
        let current = self.project_state(&events)?;
        let at = now()?;
        let mut entries = current.values()
            .filter(|record| record.project_id == project_id)
            .map(|record| snapshot(record, &self.session_id, at))
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort_by(|a, b| b.created_unix_seconds.cmp(&a.created_unix_seconds)
            .then_with(|| a.intent_id.cmp(&b.intent_id)));
        Ok(IntentList {
            schema_version: 1,
            project_id: project_id.to_owned(),
            source: "private_host_execution_intent_journal",
            entries,
            worker_execution_enabled: false,
            installation_acceptance: "not_tested",
        })
    }

    pub fn prepare(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<IntentSummary, IntentError> {
        let candidate = recipe(recipe_id).ok_or(IntentError::Invalid)?;
        let (source_head, manifest_sha256) = check_source(registry, project_id, candidate)?;
        let at = now()?;
        let next = IntentRecord {
            intent_id: new_id(),
            project_id: project_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
            manifest_relative_path: candidate.manifest.to_owned(),
            manifest_sha256,
            source_head,
            worktree: "clean".to_owned(),
            created_unix_seconds: at,
            expires_unix_seconds: at + INTENT_TTL_SECS,
            session_id: self.session_id.clone(),
            status: "prepared".to_owned(),
        };
        let _guard = self.lock.lock().map_err(|_| IntentError::JournalUnavailable)?;
        let events = self.events_locked()?;
        let count = self.project_state(&events)?.values()
            .filter(|intent| intent.project_id == project_id).count();
        if count >= MAX_INTENTS_PER_PROJECT { return Err(IntentError::Capacity); }
        self.append_locked(&events, next)
    }

    pub fn decide(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        intent_id: &str,
        decision: &str,
    ) -> Result<IntentSummary, IntentError> {
        if !valid_project(project_id)
            || !intent_id.starts_with("intent-")
            || !valid_sha(&intent_id[7..], 32)
            || !matches!(decision, "record_review" | "cancel") {
            return Err(IntentError::Invalid);
        }
        registry.registered_git_root(project_id)
            .map_err(|_| IntentError::ProjectUnavailable)?;
        let _guard = self.lock.lock().map_err(|_| IntentError::JournalUnavailable)?;
        let events = self.events_locked()?;
        let current = self.project_state(&events)?;
        let prior = current.get(intent_id).ok_or(IntentError::NotFound)?;
        if prior.project_id != project_id { return Err(IntentError::NotFound); }
        let mut next = prior.clone();
        if decision == "record_review" {
            if prior.status != "prepared"
                || prior.session_id != self.session_id
                || now()? > prior.expires_unix_seconds {
                return Err(IntentError::Stale);
            }
            let candidate = recipe(&prior.recipe_id).ok_or(IntentError::JournalCorrupt)?;
            let (head, digest) = check_source(registry, project_id, candidate)?;
            if head != prior.source_head || digest != prior.manifest_sha256 {
                return Err(IntentError::Stale);
            }
            next.status = "review_recorded_no_execution".to_owned();
        } else {
            next.status = match prior.status.as_str() {
                "prepared" => "cancelled",
                "review_recorded_no_execution" => "review_withdrawn",
                _ => return Err(IntentError::Stale),
            }.to_owned();
        }
        self.append_locked(&events, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> IntentRecord {
        IntentRecord {
            intent_id: id.to_owned(), project_id: "project-1".into(),
            recipe_id: "rust_tests".into(), manifest_relative_path: "Cargo.toml".into(),
            manifest_sha256: "b".repeat(64), source_head: "a".repeat(40),
            worktree: "clean".into(), created_unix_seconds: 100,
            expires_unix_seconds: 100 + INTENT_TTL_SECS,
            session_id: "session-0123456789abcdef0123456789abcdef".into(),
            status: "prepared".into(),
        }
    }
    fn sample_event(intent: IntentRecord, id: &str) -> IntentEvent {
        IntentEvent { schema_version: 1, event_id: id.into(),
            observed_unix_seconds: 100, intent }
    }
    #[test]
    fn schema_refuses_arbitrary_recipes_and_paths() {
        assert!(recipe("rust_tests").is_some());
        assert!(recipe("sh -c evil").is_none());
        assert!(recipe("node_install").is_none());
        assert!(!valid_record(&IntentRecord {
            manifest_relative_path: "../secret".into(),
            ..sample("intent-0123456789abcdef0123456789abcdef")
        }));
    }
    #[test]
    fn one_shot_review_and_cancel_have_no_execution_authority() {
        let path = std::env::temp_dir().join(format!("d1-events-{}.jsonl", rand::random::<u128>()));
        let store = ExecutionIntents::new(path).unwrap();
        let id = "intent-0123456789abcdef0123456789abcdef";
        let first = sample(id);
        let second = IntentRecord { status: "review_recorded_no_execution".into(), ..first.clone() };
        let third = IntentRecord { status: "review_withdrawn".into(), ..second.clone() };
        let rows = vec![
            sample_event(first, "evt-0123456789abcdef0123456789abcdef"),
            sample_event(second, "evt-11111111111111111111111111111111"),
            sample_event(third, "evt-22222222222222222222222222222222"),
        ];
        assert_eq!(store.project_state(&rows).unwrap().get(id).unwrap().status, "review_withdrawn");
        assert!(store.project_state(&[rows[0].clone(), rows[1].clone(), rows[1].clone()]).is_err());
        let snap = snapshot(&rows[1].intent, &store.session_id, 110).unwrap();
        assert!(!snap.grant_token_issued);
        assert_eq!(snap.execution, "never_started");
        assert_eq!(snap.execution_authority, "not_issued");
    }
    #[test]
    fn session_restart_and_timeout_force_fresh_preparation() {
        let prepared = sample("intent-0123456789abcdef0123456789abcdef");
        let live = snapshot(&prepared, &prepared.session_id, 110).unwrap();
        assert_eq!(live.state, "prepared_not_authorized");
        let restarted = snapshot(&prepared, "session-ffffffffffffffffffffffffffffffff", 110).unwrap();
        assert_eq!(restarted.state, "requires_new_session_review");
        let expired = snapshot(&prepared, &prepared.session_id, 300).unwrap();
        assert_eq!(expired.state, "expired_without_execution");
        assert!(!expired.can_record_review);
    }
    #[test]
    fn corrupt_and_incomplete_journals_are_not_silently_reset() {
        let path = std::env::temp_dir().join(format!("d1-corrupt-{}.jsonl", rand::random::<u128>()));
        fs::write(&path, b"{\"schemaVersion\":1").unwrap();
        let store = ExecutionIntents::new(path.clone()).unwrap();
        assert!(matches!(store.events_locked(), Err(IntentError::JournalCorrupt)));
        drop(store);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn durable_preparation_reopens_without_reusable_review() {
        let path = std::env::temp_dir().join(format!(
            "d1-restart-{}.jsonl", rand::random::<u128>()
        ));
        let store = ExecutionIntents::new(path.clone()).unwrap();
        let at = now().unwrap();
        let record = IntentRecord {
            session_id: store.session_id.clone(),
            created_unix_seconds: at,
            expires_unix_seconds: at + INTENT_TTL_SECS,
            ..sample("intent-abcdefabcdefabcdefabcdefabcdefab")
        };
        assert!(valid_record(&record));
        let first = store.append_locked(&[], record.clone()).unwrap();
        assert_eq!(first.state, "prepared_not_authorized");
        assert!(first.can_record_review);
        drop(store);
        let reopened = ExecutionIntents::new(path.clone()).unwrap();
        let persisted = reopened.events_locked().unwrap();
        assert_eq!(persisted.len(), 1);
        let latest = reopened.project_state(&persisted).unwrap();
        let state = snapshot(
            latest.get(&record.intent_id).unwrap(),
            &reopened.session_id,
            at
        ).unwrap();
        assert_eq!(state.state, "requires_new_session_review");
        assert!(!state.can_record_review);
        assert_eq!(state.execution, "never_started");
        assert!(!state.grant_token_issued);
        drop(reopened);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn corrupt_transition_or_cross_project_replay_is_rejected() {
        let first = sample("intent-0123456789abcdef0123456789abcdef");
        let change_project = IntentRecord {
            project_id: "project-2".into(),
            status: "review_recorded_no_execution".into(),
            ..first.clone()
        };
        let store = ExecutionIntents::new(std::env::temp_dir().join(format!(
            "d1-proof-{}.jsonl", rand::random::<u128>()
        ))).unwrap();
        let rows = vec![
            sample_event(first, "evt-0123456789abcdef0123456789abcdef"),
            sample_event(change_project, "evt-11111111111111111111111111111111"),
        ];
        assert!(matches!(store.project_state(&rows), Err(IntentError::JournalCorrupt)));
    }

}
