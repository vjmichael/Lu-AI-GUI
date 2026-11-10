//! IDE-05.4 append-only post-worker audit / private-output disposal journal.
//! Independent of D3 authority: no receipt from here can authorize a worker.
//! Replay rejects altered entries, incomplete trailing writes and cross-job
//! identity changes. A prepared discard is *pending* after restart, never done.
#![cfg(feature = "package-uv-worker")]
use crate::SupervisorError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_EVENTS: usize = 1024;
const MAX_BYTES: usize = 4 * 1024 * 1024;
fn denied() -> SupervisorError {
    SupervisorError::JournalCorrupt
}
fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn native_path_ok(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let meta = match fs::symlink_metadata(path) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageLifecycleBody {
    pub version: u32,
    pub sequence: u64,
    pub project_id: String,
    pub job_id: String,
    pub plan_sha256: String,
    pub terminal_receipt_sha256: String,
    pub output_tree_sha256: String,
    pub audit_sha256: Option<String>,
    pub stage: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageLifecycleEvent {
    body: PackageLifecycleBody,
    prior_sha256: String,
    entry_sha256: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageLifecycleStatus {
    pub project_id: String,
    pub job_id: String,
    pub plan_sha256: String,
    pub terminal_receipt_sha256: String,
    pub output_tree_sha256: String,
    pub audit_sha256: Option<String>,
    pub state: &'static str,
    pub requires_explicit_private_cleanup: bool,
    pub may_execute_or_retry_worker: bool,
    pub installed_into_project: bool,
}
fn identity(body: &PackageLifecycleBody) -> bool {
    body.version == 1
        && body.project_id.starts_with("ide-")
        && body.project_id.len() == 36
        && hex(&body.project_id[4..], 32)
        && body.job_id.starts_with("job-")
        && body.job_id.len() == 36
        && hex(&body.job_id[4..], 32)
        && hex(&body.plan_sha256, 64)
        && hex(&body.terminal_receipt_sha256, 64)
        && hex(&body.output_tree_sha256, 64)
        && body.audit_sha256.as_ref().is_none_or(|s| hex(s, 64))
        && matches!(
            body.stage.as_str(),
            "package_prepared"
                | "audit_local_complete"
                | "audit_blocked"
                | "discard_prepared"
                | "discard_verified"
        )
        && (body.stage != "audit_local_complete" || body.audit_sha256.is_some())
        && (body.stage != "package_prepared"
            || (body.terminal_receipt_sha256 == GENESIS
                && body.output_tree_sha256 == GENESIS
                && body.audit_sha256.is_none()))
        && (body.stage == "package_prepared" || body.terminal_receipt_sha256 != GENESIS)
}
fn digest(prior: &str, body: &PackageLifecycleBody) -> Result<String, SupervisorError> {
    let bytes = serde_json::to_vec(body).map_err(|_| denied())?;
    let mut hash = Sha256::new();
    hash.update(b"lu-ide-private-package-lifecycle-v1\0");
    hash.update(prior.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    Ok(format!("{:x}", hash.finalize()))
}
fn read_events(path: &Path) -> Result<Vec<PackageLifecycleEvent>, SupervisorError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(SupervisorError::JournalUnavailable),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() as usize > MAX_BYTES {
        return Err(denied());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(denied());
        }
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000); // Linux O_NOFOLLOW
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut file = options
        .open(path)
        .map_err(|_| SupervisorError::JournalUnavailable)?;
    let opened = file.metadata().map_err(|_| denied())?;
    if !opened.is_file() || opened.len() != meta.len() {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| SupervisorError::JournalUnavailable)?;
    let after = fs::symlink_metadata(path).map_err(|_| denied())?;
    if after.len() != meta.len()
        || after.modified().ok() != meta.modified().ok()
        || after.file_type().is_symlink()
        || !after.is_file()
    {
        return Err(denied());
    }
    if bytes.len() > MAX_BYTES || (!bytes.is_empty() && *bytes.last().unwrap() != b'\n') {
        return Err(denied());
    }
    let mut previous = GENESIS.to_owned();
    let mut state = BTreeMap::<String, PackageLifecycleBody>::new();
    let mut events = Vec::new();
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        if events.len() >= MAX_EVENTS {
            return Err(denied());
        }
        let e: PackageLifecycleEvent = serde_json::from_slice(line).map_err(|_| denied())?;
        if !identity(&e.body)
            || e.body.sequence != events.len() as u64
            || e.prior_sha256 != previous
            || e.entry_sha256 != digest(&previous, &e.body)?
        {
            return Err(denied());
        }
        let old = state.get(&e.body.job_id);
        if let Some(old) = old {
            let preparing = old.stage == "package_prepared"
                && matches!(
                    e.body.stage.as_str(),
                    "audit_local_complete" | "audit_blocked" | "discard_prepared"
                );
            if old.project_id != e.body.project_id
                || old.plan_sha256 != e.body.plan_sha256
                || (!preparing
                    && (old.terminal_receipt_sha256 != e.body.terminal_receipt_sha256
                        || old.output_tree_sha256 != e.body.output_tree_sha256
                        || old.audit_sha256 != e.body.audit_sha256))
            {
                return Err(denied());
            }
            match (old.stage.as_str(), e.body.stage.as_str()) {
                ("package_prepared", "audit_local_complete") => {}
                ("package_prepared", "audit_blocked") => {}
                ("package_prepared", "discard_prepared") => {}
                ("audit_local_complete", "discard_prepared") => {}
                ("discard_prepared", "discard_verified") => {}
                _ => return Err(denied()),
            }
        } else if e.body.stage != "package_prepared" {
            return Err(denied());
        }
        state.insert(e.body.job_id.clone(), e.body.clone());
        previous = e.entry_sha256.clone();
        events.push(e);
    }
    Ok(events)
}
pub struct PackageLifecycleLedger {
    path: PathBuf,
    gate: Mutex<()>,
}
impl PackageLifecycleLedger {
    pub fn open(private_parent: PathBuf) -> Result<Self, SupervisorError> {
        if !native_path_ok(&private_parent) {
            return Err(SupervisorError::JournalUnavailable);
        }
        let path = private_parent.join("package-output-lifecycle-v1.jsonl");
        read_events(&path)?;
        Ok(Self {
            path,
            gate: Mutex::new(()),
        })
    }
    pub fn statuses(&self) -> Result<Vec<PackageLifecycleStatus>, SupervisorError> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let events = read_events(&self.path)?;
        let mut latest = BTreeMap::new();
        for e in events {
            latest.insert(e.body.job_id.clone(), e.body);
        }
        Ok(latest
            .into_values()
            .map(|body| {
                let state = match body.stage.as_str() {
                    "package_prepared" => "original_package_job_requires_native_reconciliation",
                    "audit_local_complete" => "verified_local_bytes_private_not_promoted",
                    "audit_blocked" => "private_output_audit_blocked",
                    "discard_prepared" => "private_output_cleanup_pending",
                    "discard_verified" => "private_output_destroyed",
                    _ => "journal_corrupt",
                };
                PackageLifecycleStatus {
                    project_id: body.project_id,
                    job_id: body.job_id,
                    plan_sha256: body.plan_sha256,
                    terminal_receipt_sha256: body.terminal_receipt_sha256,
                    output_tree_sha256: body.output_tree_sha256,
                    audit_sha256: body.audit_sha256,
                    state,
                    requires_explicit_private_cleanup: body.stage == "discard_prepared"
                        || body.stage == "package_prepared",
                    may_execute_or_retry_worker: false,
                    installed_into_project: false,
                }
            })
            .collect())
    }
    fn append(&self, mut body: PackageLifecycleBody) -> Result<(), SupervisorError> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let events = read_events(&self.path)?;
        if events.len() >= MAX_EVENTS {
            return Err(SupervisorError::Capacity);
        }
        body.sequence = events.len() as u64;
        if !identity(&body) {
            return Err(SupervisorError::InvalidPlan);
        }
        if let Some(old) = events.iter().rev().find(|x| x.body.job_id == body.job_id) {
            let preparing = old.body.stage == "package_prepared"
                && matches!(
                    body.stage.as_str(),
                    "audit_local_complete" | "audit_blocked" | "discard_prepared"
                );
            if old.body.project_id != body.project_id
                || old.body.plan_sha256 != body.plan_sha256
                || (!preparing
                    && (old.body.terminal_receipt_sha256 != body.terminal_receipt_sha256
                        || old.body.output_tree_sha256 != body.output_tree_sha256
                        || old.body.audit_sha256 != body.audit_sha256))
            {
                return Err(denied());
            }
            if !matches!(
                (old.body.stage.as_str(), body.stage.as_str()),
                ("package_prepared", "audit_local_complete")
                    | ("package_prepared", "audit_blocked")
                    | ("package_prepared", "discard_prepared")
                    | ("audit_local_complete", "discard_prepared")
                    | ("discard_prepared", "discard_verified")
            ) {
                return Err(SupervisorError::InvalidTransition);
            }
        } else if body.stage != "package_prepared" {
            return Err(SupervisorError::InvalidTransition);
        }
        let prior = events
            .last()
            .map(|e| e.entry_sha256.as_str())
            .unwrap_or(GENESIS);
        let e = PackageLifecycleEvent {
            entry_sha256: digest(prior, &body)?,
            prior_sha256: prior.to_owned(),
            body,
        };
        let mut bytes = serde_json::to_vec(&e).map_err(|_| denied())?;
        bytes.push(b'\n');
        if bytes.len()
            + fs::metadata(&self.path)
                .map(|m| m.len() as usize)
                .unwrap_or(0)
            > MAX_BYTES
        {
            return Err(SupervisorError::Capacity);
        }
        let mut options = OpenOptions::new();
        options.append(true).create(true);
        match fs::symlink_metadata(&self.path) {
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() {
                    return Err(denied());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(denied());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(SupervisorError::JournalUnavailable),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(0x20000);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000);
        }
        let mut file = options
            .open(&self.path)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        file.write_all(&bytes)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        file.sync_all()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        Ok(())
    }
    /// Fsync BEFORE any native worker or human approval. A crash before the
    /// terminal audit leaves an explicit unresolved package-job breadcrumb
    /// with no terminal result and no authority to retry.
    pub fn record_prepared(
        &self,
        project_id: &str,
        job_id: &str,
        plan_sha256: &str,
    ) -> Result<(), SupervisorError> {
        self.append(PackageLifecycleBody {
            version: 1,
            sequence: 0,
            project_id: project_id.to_owned(),
            job_id: job_id.to_owned(),
            plan_sha256: plan_sha256.to_owned(),
            terminal_receipt_sha256: GENESIS.to_owned(),
            output_tree_sha256: GENESIS.to_owned(),
            audit_sha256: None,
            stage: "package_prepared".into(),
        })
    }
    pub fn record_audit(
        &self,
        project_id: &str,
        job_id: &str,
        plan_sha256: &str,
        terminal_receipt_sha256: &str,
        output_tree_sha256: &str,
        audit_sha256: Option<&str>,
    ) -> Result<(), SupervisorError> {
        self.append(PackageLifecycleBody {
            version: 1,
            sequence: 0,
            project_id: project_id.to_owned(),
            job_id: job_id.to_owned(),
            plan_sha256: plan_sha256.to_owned(),
            terminal_receipt_sha256: terminal_receipt_sha256.to_owned(),
            output_tree_sha256: output_tree_sha256.to_owned(),
            audit_sha256: audit_sha256.map(str::to_owned),
            stage: if audit_sha256.is_some() {
                "audit_local_complete"
            } else {
                "audit_blocked"
            }
            .into(),
        })
    }
    /// After the original OS Job reaches a trusted terminal state, an
    /// interrupted package job with no postrun audit may only be DISCARDED,
    /// never accepted. The host supplies the independently observed terminal
    /// D3 hash and current private output tree (no UI-selected paths).
    pub fn prepare_unverified_discard(
        &self,
        status: &PackageLifecycleStatus,
        terminal_receipt_sha256: &str,
        output_tree_sha256: &str,
    ) -> Result<(), SupervisorError> {
        if status.state != "original_package_job_requires_native_reconciliation"
            || !hex(terminal_receipt_sha256, 64)
            || terminal_receipt_sha256 == GENESIS
            || !hex(output_tree_sha256, 64)
        {
            return Err(SupervisorError::InvalidTransition);
        }
        let body = PackageLifecycleBody {
            version: 1,
            sequence: 0,
            project_id: status.project_id.clone(),
            job_id: status.job_id.clone(),
            plan_sha256: status.plan_sha256.clone(),
            terminal_receipt_sha256: terminal_receipt_sha256.to_owned(),
            output_tree_sha256: output_tree_sha256.to_owned(),
            audit_sha256: None,
            stage: "discard_prepared".into(),
        };
        self.append(body)
    }
    pub fn prepare_discard(&self, status: &PackageLifecycleStatus) -> Result<(), SupervisorError> {
        let body = PackageLifecycleBody {
            version: 1,
            sequence: 0,
            project_id: status.project_id.clone(),
            job_id: status.job_id.clone(),
            plan_sha256: status.plan_sha256.clone(),
            terminal_receipt_sha256: status.terminal_receipt_sha256.clone(),
            output_tree_sha256: status.output_tree_sha256.clone(),
            audit_sha256: status.audit_sha256.clone(),
            stage: "discard_prepared".into(),
        };
        self.append(body)
    }
    pub fn verify_discard(&self, status: &PackageLifecycleStatus) -> Result<(), SupervisorError> {
        // A stale pre-cleanup observation cannot be used to advance the
        // journal to destroyed. This remains only a journal transition;
        // callers must independently verify OS child cleanup and bytes.
        if status.state != "private_output_cleanup_pending"
            || !status.requires_explicit_private_cleanup
        {
            return Err(SupervisorError::InvalidTransition);
        }
        let body = PackageLifecycleBody {
            version: 1,
            sequence: 0,
            project_id: status.project_id.clone(),
            job_id: status.job_id.clone(),
            plan_sha256: status.plan_sha256.clone(),
            terminal_receipt_sha256: status.terminal_receipt_sha256.clone(),
            output_tree_sha256: status.output_tree_sha256.clone(),
            audit_sha256: status.audit_sha256.clone(),
            stage: "discard_verified".into(),
        };
        self.append(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Fixture, PackageLifecycleLedger) {
        let root =
            std::env::temp_dir().join(format!("ide054-ledger-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&root).unwrap();
        let ledger = PackageLifecycleLedger::open(root.clone()).unwrap();
        (Fixture(root), ledger)
    }
    fn ids() -> (String, String, String, String, String, String) {
        (
            format!("ide-{}", "a".repeat(32)),
            format!("job-{}", "b".repeat(32)),
            "c".repeat(64),
            "d".repeat(64),
            "e".repeat(64),
            "f".repeat(64),
        )
    }
    #[test]
    fn local_audit_survives_restart_without_becoming_execution_authority() {
        let (temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .unwrap();
        let restarted = PackageLifecycleLedger::open(temp.0.clone()).unwrap();
        let rows = restarted.statuses().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, "verified_local_bytes_private_not_promoted");
        assert!(!rows[0].may_execute_or_retry_worker);
        assert!(!rows[0].installed_into_project);
        assert_eq!(rows[0].audit_sha256.as_deref(), Some(audit.as_str()));
    }
    #[test]
    fn discard_prepare_survives_crash_and_remains_pending_until_verified() {
        let (temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .unwrap();
        let status = ledger.statuses().unwrap().pop().unwrap();
        ledger.prepare_discard(&status).unwrap();
        let reopened = PackageLifecycleLedger::open(temp.0.clone()).unwrap();
        let pending = reopened.statuses().unwrap().pop().unwrap();
        assert_eq!(pending.state, "private_output_cleanup_pending");
        assert!(pending.requires_explicit_private_cleanup);
        assert!(!pending.may_execute_or_retry_worker);
        reopened.verify_discard(&pending).unwrap();
        let final_reopen = PackageLifecycleLedger::open(temp.0.clone()).unwrap();
        let done = final_reopen.statuses().unwrap().pop().unwrap();
        assert_eq!(done.state, "private_output_destroyed");
        assert!(!done.requires_explicit_private_cleanup);
    }
    #[test]
    fn stale_pre_discard_status_cannot_claim_verified_cleanup() {
        let (_temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .unwrap();
        let stale = ledger.statuses().unwrap().pop().unwrap();
        assert!(matches!(
            ledger.verify_discard(&stale),
            Err(SupervisorError::InvalidTransition)
        ));
        ledger.prepare_discard(&stale).unwrap();
        assert!(matches!(
            ledger.verify_discard(&stale),
            Err(SupervisorError::InvalidTransition)
        ));
        let pending = ledger.statuses().unwrap().pop().unwrap();
        assert_eq!(pending.state, "private_output_cleanup_pending");
        assert!(!pending.may_execute_or_retry_worker);
    }
    #[test]
    fn tampered_or_partial_journal_refuses_replay() {
        let (temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .unwrap();
        let path = temp.0.join("package-output-lifecycle-v1.jsonl");
        let mut contents = fs::read_to_string(&path).unwrap();
        contents = contents.replace("audit_local_complete", "discard_verified");
        fs::write(&path, contents).unwrap();
        assert!(PackageLifecycleLedger::open(temp.0.clone()).is_err());
        let (temp, ledger) = fixture();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .unwrap();
        let path = temp.0.join("package-output-lifecycle-v1.jsonl");
        let mut bytes = fs::read(&path).unwrap();
        bytes.pop();
        fs::write(&path, bytes).unwrap();
        assert!(PackageLifecycleLedger::open(temp.0.clone()).is_err());
    }
    #[test]
    fn crash_between_supervisor_prepare_and_audit_stays_unresolved() {
        let (temp, ledger) = fixture();
        let (project, job, plan, _terminal, _tree, _audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        let reopened = PackageLifecycleLedger::open(temp.0.clone()).unwrap();
        let pending = reopened.statuses().unwrap().pop().unwrap();
        assert_eq!(
            pending.state,
            "original_package_job_requires_native_reconciliation"
        );
        assert!(pending.requires_explicit_private_cleanup);
        assert!(!pending.may_execute_or_retry_worker);
        assert!(reopened.prepare_discard(&pending).is_err());
        assert!(reopened.verify_discard(&pending).is_err());
    }
    #[test]
    fn same_job_cannot_claim_new_plan_or_replay_audit() {
        let (_temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .unwrap();
        assert!(ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, Some(&audit))
            .is_err());
        assert!(ledger
            .record_audit(
                &project,
                &job,
                &"8".repeat(64),
                &terminal,
                &tree,
                Some(&audit)
            )
            .is_err());
        assert!(ledger
            .record_audit(&project, &job, &plan, &"9".repeat(64), &tree, Some(&audit))
            .is_err());
    }
    #[test]
    fn unknown_outcome_can_only_transition_to_explicit_unverified_discard() {
        let (temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, _audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        let pending = ledger.statuses().unwrap().pop().unwrap();
        assert!(ledger.prepare_discard(&pending).is_err());
        ledger
            .prepare_unverified_discard(&pending, &terminal, &tree)
            .unwrap();
        let reopened = PackageLifecycleLedger::open(temp.0.clone()).unwrap();
        let discard = reopened.statuses().unwrap().pop().unwrap();
        assert_eq!(discard.state, "private_output_cleanup_pending");
        assert_eq!(discard.terminal_receipt_sha256, terminal);
        assert_eq!(discard.output_tree_sha256, tree);
        assert!(discard.audit_sha256.is_none());
        reopened.verify_discard(&discard).unwrap();
        assert_eq!(
            reopened.statuses().unwrap().pop().unwrap().state,
            "private_output_destroyed"
        );
    }
    #[test]
    fn blocked_audit_never_becomes_discard_verified_or_approval() {
        let (_temp, ledger) = fixture();
        let (project, job, plan, terminal, tree, _audit) = ids();
        ledger.record_prepared(&project, &job, &plan).unwrap();
        ledger
            .record_audit(&project, &job, &plan, &terminal, &tree, None)
            .unwrap();
        let blocked = ledger.statuses().unwrap().pop().unwrap();
        assert_eq!(blocked.state, "private_output_audit_blocked");
        assert!(ledger.prepare_discard(&blocked).is_err());
        assert!(ledger.verify_discard(&blocked).is_err());
        assert!(!blocked.may_execute_or_retry_worker);
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_journal_is_never_read_or_reused() {
        use std::os::unix::fs::symlink;
        let (temp, _ledger) = fixture();
        let path = temp.0.join("package-output-lifecycle-v1.jsonl");
        symlink("/etc/passwd", &path).unwrap();
        assert!(PackageLifecycleLedger::open(temp.0.clone()).is_err());
    }
}
