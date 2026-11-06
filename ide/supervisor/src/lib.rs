//! Lu IDE Wave D3: native execution-supervisor CONTROL PLANE.
//!
//! This crate has no spawn/exec APIs or Tauri connection. Its release build is
//! deliberately incapable of launching project code. Native OS launch adapters
//! must separately prove restricted rights, pre-exec containment, process-tree
//! cancellation and crash recovery before they can be connected.
//!
//! Grants are short-lived, in-memory, one-shot, project/source/recipe scoped.
//! Historic D1 review notes, journal lines, WebView text and task claims are
//! never executable authorizations.
// Unsafe is restricted to the Linux worker's pre-exec cgroup admission.
// The portable supervisor state machine itself performs no unsafe operations.
#[cfg(all(target_os = "linux", feature = "linux-worker"))]
pub mod linux_worker;
pub mod snapshot;

use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_JOURNAL_BYTES: u64 = 2 * 1024 * 1024;
const MAX_EVENTS: usize = 512;
const MAX_JOBS: usize = 64;
const MAX_LOG_BYTES: u64 = 256 * 1024;
const MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PIDS: u32 = 16;
const MAX_RUNTIME_MS: u64 = 10 * 60 * 1000;
const GRANT_TTL_MS: u64 = 60_000;
const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorError {
    InvalidPlan,
    InvalidApproval,
    NoSuchJob,
    WrongProjectOrSource,
    GrantExpired,
    GrantConsumed,
    NoContainment,
    InvalidTransition,
    Quarantined,
    JournalCorrupt,
    JournalUnavailable,
    Capacity,
}
impl SupervisorError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidPlan => "SUPERVISOR_INVALID_PLAN",
            Self::InvalidApproval => "SUPERVISOR_APPROVAL_REQUIRED",
            Self::NoSuchJob => "SUPERVISOR_JOB_NOT_FOUND",
            Self::WrongProjectOrSource => "SUPERVISOR_SCOPE_MISMATCH",
            Self::GrantExpired => "SUPERVISOR_GRANT_EXPIRED",
            Self::GrantConsumed => "SUPERVISOR_GRANT_CONSUMED",
            Self::NoContainment => "SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED",
            Self::InvalidTransition => "SUPERVISOR_TRANSITION_DENIED",
            Self::Quarantined => "SUPERVISOR_RECONCILIATION_REQUIRED",
            Self::JournalCorrupt => "SUPERVISOR_RECEIPTS_CORRUPT",
            Self::JournalUnavailable => "SUPERVISOR_JOURNAL_UNAVAILABLE",
            Self::Capacity => "SUPERVISOR_CAPACITY_REACHED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobPlan {
    pub project_id: String,
    pub recipe_id: String,
    pub source_head: String,
    /// Must be a host-observed digest of the actual input tree before execution.
    /// A Git HEAD or clean libgit2 status alone is NOT sufficient.
    pub input_snapshot_sha256: String,
    pub manifest_sha256: String,
    pub toolchain_sha256: String,
    pub runtime_limit_ms: u64,
    pub memory_limit_bytes: u64,
    pub process_limit: u32,
    pub log_limit_bytes: u64,
    pub effect_class: String,
}
fn hex_sha(value: &str, n: usize) -> bool {
    value.len() == n && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn validate_plan(value: &JobPlan) -> bool {
    value.project_id.starts_with("ide-")
        && value.project_id.len() == 36
        && hex_sha(&value.project_id[4..], 32)
        && matches!(
            value.recipe_id.as_str(),
            "rust_check" | "rust_tests" | "lu_ide_host_tests"
        )
        && hex_sha(&value.source_head, 40)
        && hex_sha(&value.input_snapshot_sha256, 64)
        && hex_sha(&value.manifest_sha256, 64)
        && hex_sha(&value.toolchain_sha256, 64)
        && value.runtime_limit_ms > 0
        && value.runtime_limit_ms <= MAX_RUNTIME_MS
        && value.memory_limit_bytes >= 32 * 1024 * 1024
        && value.memory_limit_bytes <= MAX_MEMORY_BYTES
        && value.process_limit > 0
        && value.process_limit <= MAX_PIDS
        && value.log_limit_bytes > 0
        && value.log_limit_bytes <= MAX_LOG_BYTES
        && value.effect_class == "test_build_isolated"
}
fn fingerprint(value: &JobPlan) -> Result<String, SupervisorError> {
    let bytes = serde_json::to_vec(value).map_err(|_| SupervisorError::InvalidPlan)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}
fn random_id(prefix: &str) -> String {
    let mut buffer = [0_u8; 16];
    OsRng.fill_bytes(&mut buffer);
    let mut id = prefix.to_owned();
    for byte in buffer {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}
fn now_ms() -> Result<u64, SupervisorError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SupervisorError::JournalUnavailable)?;
    u64::try_from(duration.as_millis()).map_err(|_| SupervisorError::JournalUnavailable)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Prepared,
    Authorized,
    AdmissionPending,
    Running,
    CancelRequested,
    Cancelled,
    Completed,
    Failed,
    Denied,
    RecoveryRequired,
    Uncertain,
}
fn valid_change(previous: Option<JobPhase>, next: JobPhase) -> bool {
    matches!(
        (previous, next),
        (None, JobPhase::Prepared)
            | (Some(JobPhase::Prepared), JobPhase::Authorized)
            | (Some(JobPhase::Prepared), JobPhase::Cancelled)
            | (Some(JobPhase::Authorized), JobPhase::AdmissionPending)
            | (Some(JobPhase::Authorized), JobPhase::Cancelled)
            | (Some(JobPhase::AdmissionPending), JobPhase::Running)
            | (Some(JobPhase::AdmissionPending), JobPhase::Denied)
            | (Some(JobPhase::AdmissionPending), JobPhase::RecoveryRequired)
            | (Some(JobPhase::Running), JobPhase::CancelRequested)
            | (Some(JobPhase::Running), JobPhase::Completed)
            | (Some(JobPhase::Running), JobPhase::Failed)
            | (Some(JobPhase::Running), JobPhase::RecoveryRequired)
            | (Some(JobPhase::CancelRequested), JobPhase::Cancelled)
            | (Some(JobPhase::CancelRequested), JobPhase::Uncertain)
            | (Some(JobPhase::CancelRequested), JobPhase::RecoveryRequired)
            | (Some(JobPhase::RecoveryRequired), JobPhase::Cancelled)
            | (Some(JobPhase::RecoveryRequired), JobPhase::Uncertain)
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptBody {
    pub version: u32,
    pub sequence: u64,
    pub event_id: String,
    pub job_id: String,
    pub project_id: String,
    pub plan_sha256: String,
    pub phase: JobPhase,
    pub observed_unix_ms: u64,
    pub session_id: String,
    pub outcome_code: String,
    /// No raw logs, OS credentials, local absolute paths, tokens or source text.
    pub evidence_code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub body: ReceiptBody,
    pub prior_hash: String,
    pub entry_hash: String,
}
fn receipt_digest(body: &ReceiptBody, prior: &str) -> Result<String, SupervisorError> {
    let mut hash = Sha256::new();
    hash.update(prior.as_bytes());
    hash.update(serde_json::to_vec(body).map_err(|_| SupervisorError::JournalCorrupt)?);
    Ok(format!("{:x}", hash.finalize()))
}
#[derive(Debug, Clone)]
struct JobState {
    plan: JobPlan,
    phase: JobPhase,
    plan_hash: String,
}
#[derive(Debug, Clone)]
struct Grant {
    job_id: String,
    project_id: String,
    plan_hash: String,
    source_head: String,
    expires_unix_ms: u64,
    session_id: String,
}
#[derive(Debug, Clone)]
struct Store {
    receipts: Vec<Receipt>,
    jobs: BTreeMap<String, JobState>,
    grants: HashMap<String, Grant>,
    quarantine: HashSet<String>,
}
impl Store {
    fn empty() -> Self {
        Self {
            receipts: vec![],
            jobs: BTreeMap::new(),
            grants: HashMap::new(),
            quarantine: HashSet::new(),
        }
    }
    fn replay(receipts: Vec<Receipt>) -> Result<Self, SupervisorError> {
        if receipts.len() > MAX_EVENTS {
            return Err(SupervisorError::JournalCorrupt);
        }
        let mut state = Self::empty();
        let mut seen_events = HashSet::new();
        let mut expected_previous = GENESIS.to_owned();
        for (i, receipt) in receipts.iter().enumerate() {
            let body = &receipt.body;
            if body.version != 1
                || body.sequence != i as u64
                || !seen_events.insert(&body.event_id)
                || !body.event_id.starts_with("evt-")
                || !hex_sha(&body.event_id[4..], 32)
                || !body.job_id.starts_with("job-")
                || !hex_sha(&body.job_id[4..], 32)
                || !hex_sha(&body.plan_sha256, 64)
                || receipt.prior_hash != expected_previous
                || receipt.entry_hash != receipt_digest(body, &receipt.prior_hash)?
                || body.outcome_code.len() > 80
                || body.evidence_code.len() > 100
            {
                return Err(SupervisorError::JournalCorrupt);
            }
            let prev_phase = state.jobs.get(&body.job_id).map(|job| job.phase);
            if !valid_change(prev_phase, body.phase) {
                return Err(SupervisorError::JournalCorrupt);
            }
            if let Some(old) = state.jobs.get_mut(&body.job_id) {
                if old.plan_hash != body.plan_sha256 || old.plan.project_id != body.project_id {
                    return Err(SupervisorError::JournalCorrupt);
                }
                old.phase = body.phase;
            } else {
                // The full plan is intentionally not persisted. On reopening,
                // only the signed-by-nobody receipt coordinates are retained:
                // no prepared/authorized grant can survive a process restart.
                let blank = JobPlan {
                    project_id: body.project_id.clone(),
                    recipe_id: String::new(),
                    source_head: String::new(),
                    input_snapshot_sha256: String::new(),
                    manifest_sha256: String::new(),
                    toolchain_sha256: String::new(),
                    runtime_limit_ms: 0,
                    memory_limit_bytes: 0,
                    process_limit: 0,
                    log_limit_bytes: 0,
                    effect_class: String::new(),
                };
                state.jobs.insert(
                    body.job_id.clone(),
                    JobState {
                        plan: blank,
                        plan_hash: body.plan_sha256.clone(),
                        phase: body.phase,
                    },
                );
            }
            expected_previous = receipt.entry_hash.clone();
        }
        state.receipts = receipts;
        for (id, job) in &state.jobs {
            if matches!(
                job.phase,
                JobPhase::AdmissionPending
                    | JobPhase::Running
                    | JobPhase::CancelRequested
                    | JobPhase::RecoveryRequired
                    | JobPhase::Uncertain
            ) {
                state.quarantine.insert(id.clone());
            }
        }
        if state.jobs.len() > MAX_JOBS {
            return Err(SupervisorError::JournalCorrupt);
        }
        Ok(state)
    }
}

#[derive(Debug, Clone)]
pub struct JobSummary {
    pub job_id: String,
    pub project_id: String,
    pub plan_sha256: String,
    pub phase: JobPhase,
    pub requires_reconciliation: bool,
    pub execution_verified: bool,
}

#[derive(Debug, Clone)]
pub struct GrantSummary {
    /// Not an executor capability. Only usable by private native supervisor methods.
    pub job_id: String,
    pub plan_sha256: String,
    pub expires_unix_ms: u64,
    pub executable_from_desktop: bool,
}

pub struct Supervisor {
    journal_path: PathBuf,
    session_id: String,
    state: Mutex<Store>,
}
impl Supervisor {
    /// Path must be supplied by trusted application profile management.
    /// Do not point this at a repository or user-selected project file.
    pub fn open(journal_path: PathBuf) -> Result<Self, SupervisorError> {
        if let Ok(meta) = fs::symlink_metadata(&journal_path) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(SupervisorError::JournalCorrupt);
            }
        }
        let receipts = read_journal(&journal_path)?;
        let state = Store::replay(receipts)?;
        Ok(Self {
            journal_path,
            session_id: random_id("session-"),
            state: Mutex::new(state),
        })
    }

    pub fn inspect(&self) -> Result<Vec<JobSummary>, SupervisorError> {
        let state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let mut out: Vec<JobSummary> = state
            .jobs
            .iter()
            .map(|(id, job)| JobSummary {
                job_id: id.clone(),
                project_id: job.plan.project_id.clone(),
                plan_sha256: job.plan_hash.clone(),
                phase: job.phase,
                requires_reconciliation: state.quarantine.contains(id),
                execution_verified: false,
            })
            .collect();
        out.sort_by(|a, b| a.job_id.cmp(&b.job_id));
        Ok(out)
    }

    /// Host-only verified input coordinates; no actual process execution.
    pub fn prepare(&self, plan: JobPlan) -> Result<JobSummary, SupervisorError> {
        if !validate_plan(&plan) {
            return Err(SupervisorError::InvalidPlan);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        if !state.quarantine.is_empty() {
            return Err(SupervisorError::Quarantined);
        }
        if state.jobs.len() >= MAX_JOBS {
            return Err(SupervisorError::Capacity);
        }
        let id = random_id("job-");
        let plan_hash = fingerprint(&plan)?;
        self.append(
            &mut state,
            &id,
            &plan.project_id,
            &plan_hash,
            JobPhase::Prepared,
            "host_plan_prepared",
            "inputs_not_executed",
        )?;
        state.jobs.insert(
            id.clone(),
            JobState {
                plan: plan.clone(),
                phase: JobPhase::Prepared,
                plan_hash: plan_hash.clone(),
            },
        );
        Ok(JobSummary {
            job_id: id,
            project_id: plan.project_id,
            plan_sha256: plan_hash,
            phase: JobPhase::Prepared,
            requires_reconciliation: false,
            execution_verified: false,
        })
    }

    /// A fresh explicit native human action, not D1 historical review.
    /// Returns only display coordinates. The in-memory one-shot grant ID is
    /// intentionally NEVER exported or serialized in this API.
    pub fn record_human_approval(
        &self,
        job_id: &str,
        expected_plan_sha256: &str,
        current_source_head: &str,
        confirmed_in_active_desktop: bool,
    ) -> Result<GrantSummary, SupervisorError> {
        if !confirmed_in_active_desktop {
            return Err(SupervisorError::InvalidApproval);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        if !state.quarantine.is_empty() {
            return Err(SupervisorError::Quarantined);
        }
        let plan = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if plan.phase != JobPhase::Prepared {
            return Err(SupervisorError::InvalidTransition);
        }
        if plan.plan_hash != expected_plan_sha256
            || plan.plan.source_head != current_source_head
            || !hex_sha(current_source_head, 40)
        {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        let expiration = now_ms()?.saturating_add(GRANT_TTL_MS);
        self.append(
            &mut state,
            job_id,
            &plan.plan.project_id,
            &plan.plan_hash,
            JobPhase::Authorized,
            "fresh_human_approval",
            "no_process_started",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = JobPhase::Authorized;
        let grant_id = random_id("grant-");
        state.grants.insert(
            grant_id,
            Grant {
                job_id: job_id.to_owned(),
                project_id: plan.plan.project_id,
                plan_hash: plan.plan_hash.clone(),
                source_head: current_source_head.to_owned(),
                expires_unix_ms: expiration,
                session_id: self.session_id.clone(),
            },
        );
        Ok(GrantSummary {
            job_id: job_id.to_owned(),
            plan_sha256: plan.plan_hash,
            expires_unix_ms: expiration,
            executable_from_desktop: false,
        })
    }

    /// Private broker admission. Native OS attestation must come from
    /// production_worker_admission, which currently denies on both platforms.
    /// No public or Tauri route calls this method.
    #[allow(dead_code)]
    fn arm_for_native_worker(
        &self,
        job_id: &str,
        refreshed_plan: &JobPlan,
    ) -> Result<(), SupervisorError> {
        let proof = production_worker_admission()?;
        self.consume_grant_after_attestation(job_id, refreshed_plan, &proof)
    }

    /// One-shot grant is consumed only after durable admission-pending receipt.
    /// Future OS worker must create a contained/suspended process AFTER this,
    /// then record running only after pre-exec containment is proven.
    fn consume_grant_after_attestation(
        &self,
        job_id: &str,
        refreshed_plan: &JobPlan,
        proof: &OsContainmentProof,
    ) -> Result<(), SupervisorError> {
        if !proof.complete() {
            return Err(SupervisorError::NoContainment);
        }
        if !validate_plan(refreshed_plan) {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        if !state.quarantine.is_empty() {
            return Err(SupervisorError::Quarantined);
        }
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if job.phase != JobPhase::Authorized {
            return Err(SupervisorError::InvalidTransition);
        }
        let observed_hash = fingerprint(refreshed_plan)?;
        if observed_hash != job.plan_hash || *refreshed_plan != job.plan {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        let grant_id = state
            .grants
            .iter()
            .find(|(_, value)| value.job_id == job_id)
            .map(|(key, _)| key.clone())
            .ok_or(SupervisorError::GrantConsumed)?;
        let grant = state.grants.get(&grant_id).unwrap().clone();
        if grant.session_id != self.session_id || now_ms()? > grant.expires_unix_ms {
            state.grants.remove(&grant_id);
            return Err(SupervisorError::GrantExpired);
        }
        if grant.plan_hash != job.plan_hash
            || grant.project_id != job.plan.project_id
            || grant.source_head != refreshed_plan.source_head
        {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            JobPhase::AdmissionPending,
            "grant_consumed_pending_os_admission",
            "no_worker_yet_started",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = JobPhase::AdmissionPending;
        state.grants.remove(&grant_id);
        Ok(())
    }

    /// Native-only state transition: successful OS admission and suspended
    /// process placement must precede recording a running worker.
    fn native_mark_running(&self, job_id: &str) -> Result<(), SupervisorError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if job.phase != JobPhase::AdmissionPending {
            return Err(SupervisorError::InvalidTransition);
        }
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            JobPhase::Running,
            "native_contained_worker_started",
            "os_admission_and_input_seal_checked",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = JobPhase::Running;
        Ok(())
    }

    /// A worker failure before child execution may be recorded as denied
    /// only after the native broker establishes no live process was started.
    fn native_deny_before_start(
        &self,
        job_id: &str,
        no_process_started: bool,
    ) -> Result<(), SupervisorError> {
        if !no_process_started {
            return Err(SupervisorError::NoContainment);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if job.phase != JobPhase::AdmissionPending {
            return Err(SupervisorError::InvalidTransition);
        }
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            JobPhase::Denied,
            "native_process_admission_denied",
            "no_child_executed",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = JobPhase::Denied;
        Ok(())
    }

    /// Final outcome requires the OS process tree to have fully exited
    /// and the output broker to have verified bounded captured artifacts.
    fn native_finish(
        &self,
        job_id: &str,
        succeeded: bool,
        tree_reaped: bool,
        outputs_bounded: bool,
    ) -> Result<JobSummary, SupervisorError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if job.phase != JobPhase::Running {
            return Err(SupervisorError::InvalidTransition);
        }
        if !tree_reaped || !outputs_bounded {
            self.append(
                &mut state,
                job_id,
                &job.plan.project_id,
                &job.plan_hash,
                JobPhase::RecoveryRequired,
                "incomplete_os_worker_reap",
                "do_not_restart_or_reuse_outputs",
            )?;
            state.jobs.get_mut(job_id).unwrap().phase = JobPhase::RecoveryRequired;
            state.quarantine.insert(job_id.to_owned());
            return Err(SupervisorError::Quarantined);
        }
        let (phase, outcome) = if succeeded {
            (JobPhase::Completed, "native_worker_completed")
        } else {
            (JobPhase::Failed, "native_worker_failed")
        };
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            phase,
            outcome,
            "reaped_process_tree_bounded_outputs",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = phase;
        Ok(JobSummary {
            job_id: job_id.to_owned(),
            project_id: job.plan.project_id,
            plan_sha256: job.plan_hash,
            phase,
            requires_reconciliation: false,
            execution_verified: tree_reaped && outputs_bounded,
        })
    }

    pub fn cancel_before_start(&self, job_id: &str) -> Result<JobSummary, SupervisorError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if !matches!(job.phase, JobPhase::Prepared | JobPhase::Authorized) {
            return Err(SupervisorError::InvalidTransition);
        }
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            JobPhase::Cancelled,
            "human_cancelled_before_start",
            "no_worker_was_started",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = JobPhase::Cancelled;
        state.grants.retain(|_, grant| grant.job_id != job_id);
        Ok(JobSummary {
            job_id: job_id.to_owned(),
            project_id: job.plan.project_id,
            plan_sha256: job.plan_hash,
            phase: JobPhase::Cancelled,
            requires_reconciliation: false,
            execution_verified: false,
        })
    }

    /// The backend MUST verify process-tree termination (including descendants)
    /// before returning true. The broker is private to the native supervisor:
    /// no WebView/LLM command can fabricate a terminal receipt.
    /// The absence of an OS-backed broker keeps this API unreachable from Tauri.
    fn cancel_running_with(
        &self,
        job_id: &str,
        broker: &mut impl NativeTreeTerminator,
    ) -> Result<JobSummary, SupervisorError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if job.phase != JobPhase::Running {
            return Err(SupervisorError::InvalidTransition);
        }
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            JobPhase::CancelRequested,
            "cancel_requested_before_os_kill",
            "tree_termination_pending",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = JobPhase::CancelRequested;
        let killed = broker.terminate_and_verify_tree(job_id, &job.plan_hash);
        let (phase, code) = if matches!(killed, Ok(true)) {
            (JobPhase::Cancelled, "os_tree_termination_verified")
        } else {
            (JobPhase::Uncertain, "os_termination_not_verified")
        };
        // Even on an OS failure, the uncertain receipt is durable. No retry
        // assumes that the interrupted worker is gone.
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            phase,
            code,
            "native_supervisor_tree_kill_evidence",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = phase;
        if phase == JobPhase::Uncertain {
            state.quarantine.insert(job_id.to_owned());
        }
        Ok(JobSummary {
            job_id: job_id.to_owned(),
            project_id: job.plan.project_id,
            plan_sha256: job.plan_hash,
            phase,
            requires_reconciliation: phase == JobPhase::Uncertain,
            execution_verified: false,
        })
    }

    /// After process or host crash, no worker is relaunched. A durable
    /// recovery-required receipt is written before contacting an OS broker.
    /// If the broker cannot prove descendant cleanup, remain quarantined.
    fn reconcile_interrupted_with(
        &self,
        job_id: &str,
        broker: &mut impl NativeTreeTerminator,
    ) -> Result<JobSummary, SupervisorError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state
            .jobs
            .get(job_id)
            .ok_or(SupervisorError::NoSuchJob)?
            .clone();
        if !state.quarantine.contains(job_id) {
            return Err(SupervisorError::InvalidTransition);
        }
        if matches!(
            job.phase,
            JobPhase::AdmissionPending | JobPhase::Running | JobPhase::CancelRequested
        ) {
            self.append(
                &mut state,
                job_id,
                &job.plan.project_id,
                &job.plan_hash,
                JobPhase::RecoveryRequired,
                "recovery_before_os_reconcile",
                "unknown_descendant_state",
            )?;
            state.jobs.get_mut(job_id).unwrap().phase = JobPhase::RecoveryRequired;
        } else if job.phase != JobPhase::RecoveryRequired {
            return Err(SupervisorError::InvalidTransition);
        }
        let killed = broker.terminate_and_verify_tree(job_id, &job.plan_hash);
        let (phase, code) = if matches!(killed, Ok(true)) {
            (JobPhase::Cancelled, "os_orphan_cleanup_verified")
        } else {
            (JobPhase::Uncertain, "os_orphan_cleanup_unverified")
        };
        self.append(
            &mut state,
            job_id,
            &job.plan.project_id,
            &job.plan_hash,
            phase,
            code,
            "explicit_host_reconciliation",
        )?;
        state.jobs.get_mut(job_id).unwrap().phase = phase;
        if phase == JobPhase::Cancelled {
            state.quarantine.remove(job_id);
        }
        Ok(JobSummary {
            job_id: job_id.to_owned(),
            project_id: job.plan.project_id,
            plan_sha256: job.plan_hash,
            phase,
            requires_reconciliation: phase == JobPhase::Uncertain,
            execution_verified: false,
        })
    }

    pub fn receipt_history(&self, job_id: &str) -> Result<Vec<Receipt>, SupervisorError> {
        let state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        if !state.jobs.contains_key(job_id) {
            return Err(SupervisorError::NoSuchJob);
        }
        Ok(state
            .receipts
            .iter()
            .filter(|r| r.body.job_id == job_id)
            .cloned()
            .collect())
    }

    fn append(
        &self,
        state: &mut Store,
        job_id: &str,
        project: &str,
        plan_sha256: &str,
        phase: JobPhase,
        outcome_code: &str,
        evidence_code: &str,
    ) -> Result<(), SupervisorError> {
        if state.receipts.len() >= MAX_EVENTS {
            return Err(SupervisorError::Capacity);
        }
        let previous_phase = state.jobs.get(job_id).map(|job| job.phase);
        if !valid_change(previous_phase, phase) {
            return Err(SupervisorError::InvalidTransition);
        }
        if outcome_code.len() > 80 || evidence_code.len() > 100 {
            return Err(SupervisorError::InvalidPlan);
        }
        let prior_hash = state
            .receipts
            .last()
            .map(|r| r.entry_hash.clone())
            .unwrap_or_else(|| GENESIS.to_owned());
        let body = ReceiptBody {
            version: 1,
            sequence: state.receipts.len() as u64,
            event_id: random_id("evt-"),
            job_id: job_id.to_owned(),
            project_id: project.to_owned(),
            plan_sha256: plan_sha256.to_owned(),
            phase,
            observed_unix_ms: now_ms()?,
            session_id: self.session_id.clone(),
            outcome_code: outcome_code.to_owned(),
            evidence_code: evidence_code.to_owned(),
        };
        let entry_hash = receipt_digest(&body, &prior_hash)?;
        let receipt = Receipt {
            body,
            prior_hash,
            entry_hash,
        };
        let mut bytes =
            serde_json::to_vec(&receipt).map_err(|_| SupervisorError::JournalUnavailable)?;
        bytes.push(b'\n');
        let length = fs::symlink_metadata(&self.journal_path)
            .map(|meta| {
                if !meta.is_file() || meta.file_type().is_symlink() {
                    Err(SupervisorError::JournalCorrupt)
                } else {
                    Ok(meta.len())
                }
            })
            .unwrap_or_else(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Ok(0)
                } else {
                    Err(SupervisorError::JournalUnavailable)
                }
            })?;
        if length.saturating_add(bytes.len() as u64) > MAX_JOURNAL_BYTES {
            return Err(SupervisorError::Capacity);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        file.write_all(&bytes)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        file.sync_all()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        state.receipts.push(receipt);
        Ok(())
    }
}

fn read_journal(path: &PathBuf) -> Result<Vec<Receipt>, SupervisorError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err(SupervisorError::JournalUnavailable),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_JOURNAL_BYTES {
        return Err(SupervisorError::JournalCorrupt);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| SupervisorError::JournalUnavailable)?
        .take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SupervisorError::JournalUnavailable)?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES || (!bytes.is_empty() && !bytes.ends_with(b"\n")) {
        return Err(SupervisorError::JournalCorrupt);
    }
    let mut result = vec![];
    for line in bytes
        .split(|value| *value == b'\n')
        .filter(|line| !line.is_empty())
    {
        let receipt: Receipt =
            serde_json::from_slice(line).map_err(|_| SupervisorError::JournalCorrupt)?;
        result.push(receipt);
        if result.len() > MAX_EVENTS {
            return Err(SupervisorError::JournalCorrupt);
        }
    }
    Ok(result)
}

// Kept private: a user-supplied status or persisted journal line can never
// masquerade as a process-tree termination receipt.
trait NativeTreeTerminator {
    fn terminate_and_verify_tree(
        &mut self,
        job_id: &str,
        plan_sha256: &str,
    ) -> Result<bool, SupervisorError>;
}

/// Native OS admission must pass EVERY required right/containment assertion.
/// There is no release-build adapter capable of returning a verified admission:
/// restricted-token and cgroup-v2 acceptance are still under development.
#[derive(Debug, Clone, Default)]
pub struct OsContainmentProof {
    pub platform: String,
    pub restricted_identity: bool,
    pub filesystem_access_limited: bool,
    pub network_access_limited: bool,
    pub child_contained_before_resume: bool,
    pub process_tree_kill_verified: bool,
    pub memory_quota: bool,
    pub cpu_quota: bool,
    pub pid_quota: bool,
    pub protected_journal_broker: bool,
}
impl OsContainmentProof {
    pub fn complete(&self) -> bool {
        matches!(self.platform.as_str(), "windows" | "linux")
            && self.restricted_identity
            && self.filesystem_access_limited
            && self.network_access_limited
            && self.child_contained_before_resume
            && self.process_tree_kill_verified
            && self.memory_quota
            && self.cpu_quota
            && self.pid_quota
            && self.protected_journal_broker
    }
}
pub fn production_worker_admission() -> Result<OsContainmentProof, SupervisorError> {
    // D2 fixed Windows Job Object probes and Linux Bubblewrap probes do NOT
    // establish the complete rights + quotas + crash-reconcile contract.
    Err(SupervisorError::NoContainment)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!("ide-d3-{}.jsonl", random_id("test-")))
    }
    fn plan() -> JobPlan {
        JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "lu_ide_host_tests".into(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            runtime_limit_ms: 30_000,
            memory_limit_bytes: 128 * 1024 * 1024,
            process_limit: 4,
            log_limit_bytes: 32 * 1024,
            effect_class: "test_build_isolated".into(),
        }
    }
    #[test]
    fn rejects_unpinned_or_unbounded_plans() {
        let mut bad = plan();
        bad.recipe_id = "powershell -Command evil".into();
        assert!(!validate_plan(&bad));
        let mut bad = plan();
        bad.input_snapshot_sha256 = String::new();
        assert!(!validate_plan(&bad));
        let mut bad = plan();
        bad.runtime_limit_ms = u64::MAX;
        assert!(!validate_plan(&bad));
        let mut bad = plan();
        bad.effect_class = "write_anywhere".into();
        assert!(!validate_plan(&bad));
    }
    #[test]
    fn only_fresh_matching_human_grant_and_never_desktop_execution() {
        let p = path();
        let sup = Supervisor::open(p.clone()).unwrap();
        let task = sup.prepare(plan()).unwrap();
        assert_eq!(
            sup.record_human_approval(&task.job_id, &task.plan_sha256, &"f".repeat(40), true)
                .unwrap_err(),
            SupervisorError::WrongProjectOrSource
        );
        assert_eq!(
            sup.record_human_approval(&task.job_id, &task.plan_sha256, &"b".repeat(40), false)
                .unwrap_err(),
            SupervisorError::InvalidApproval
        );
        let auth = sup
            .record_human_approval(&task.job_id, &task.plan_sha256, &"b".repeat(40), true)
            .unwrap();
        assert!(!auth.executable_from_desktop);
        assert_eq!(
            sup.record_human_approval(&task.job_id, &task.plan_sha256, &"b".repeat(40), true)
                .unwrap_err(),
            SupervisorError::InvalidTransition
        );
        let cancelled = sup.cancel_before_start(&task.job_id).unwrap();
        assert_eq!(cancelled.phase, JobPhase::Cancelled);
        assert_eq!(sup.receipt_history(&task.job_id).unwrap().len(), 3);
        drop(sup);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn persisted_receipts_survive_restart_but_grants_do_not() {
        let p = path();
        let sup = Supervisor::open(p.clone()).unwrap();
        let task = sup.prepare(plan()).unwrap();
        sup.record_human_approval(&task.job_id, &task.plan_sha256, &"b".repeat(40), true)
            .unwrap();
        drop(sup);
        let reopened = Supervisor::open(p.clone()).unwrap();
        let status = reopened.inspect().unwrap();
        assert_eq!(status[0].phase, JobPhase::Authorized);
        assert!(!status[0].execution_verified);
        assert!(reopened.state.lock().unwrap().grants.is_empty());
        assert_eq!(
            reopened.cancel_before_start(&task.job_id).unwrap().phase,
            JobPhase::Cancelled
        );
        drop(reopened);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn receipt_tampering_and_truncation_fail_closed() {
        let p = path();
        let sup = Supervisor::open(p.clone()).unwrap();
        let _ = sup.prepare(plan()).unwrap();
        drop(sup);
        let bytes = fs::read_to_string(&p).unwrap();
        fs::write(
            &p,
            bytes.replace("host_plan_prepared", "fake_plan_accepted"),
        )
        .unwrap();
        assert!(matches!(
            Supervisor::open(p.clone()),
            Err(SupervisorError::JournalCorrupt)
        ));
        fs::write(&p, b"{\"body\":{").unwrap();
        assert!(matches!(
            Supervisor::open(p.clone()),
            Err(SupervisorError::JournalCorrupt)
        ));
        fs::remove_file(p).unwrap();
    }
    struct TestTerminator {
        verified: bool,
        requests: usize,
    }
    impl NativeTreeTerminator for TestTerminator {
        fn terminate_and_verify_tree(
            &mut self,
            _job_id: &str,
            _plan_sha256: &str,
        ) -> Result<bool, SupervisorError> {
            self.requests += 1;
            Ok(self.verified)
        }
    }
    fn inject_running(sup: &Supervisor, job: &JobSummary) {
        let mut state = sup.state.lock().unwrap();
        sup.append(
            &mut state,
            &job.job_id,
            &job.project_id,
            &job.plan_sha256,
            JobPhase::AdmissionPending,
            "test_simulated_admission",
            "no_real_process",
        )
        .unwrap();
        state.jobs.get_mut(&job.job_id).unwrap().phase = JobPhase::AdmissionPending;
        sup.append(
            &mut state,
            &job.job_id,
            &job.project_id,
            &job.plan_sha256,
            JobPhase::Running,
            "test_simulated_running",
            "no_real_process",
        )
        .unwrap();
        state.jobs.get_mut(&job.job_id).unwrap().phase = JobPhase::Running;
    }
    #[test]
    fn cancellation_logs_request_before_verified_tree_kill() {
        let path = path();
        let sup = Supervisor::open(path.clone()).unwrap();
        let job = sup.prepare(plan()).unwrap();
        sup.record_human_approval(&job.job_id, &job.plan_sha256, &"b".repeat(40), true)
            .unwrap();
        inject_running(&sup, &job);
        let mut broker = TestTerminator {
            verified: true,
            requests: 0,
        };
        let result = sup.cancel_running_with(&job.job_id, &mut broker).unwrap();
        assert_eq!(broker.requests, 1);
        assert_eq!(result.phase, JobPhase::Cancelled);
        let phases: Vec<_> = sup
            .receipt_history(&job.job_id)
            .unwrap()
            .iter()
            .map(|r| r.body.phase)
            .collect();
        assert_eq!(
            phases,
            vec![
                JobPhase::Prepared,
                JobPhase::Authorized,
                JobPhase::AdmissionPending,
                JobPhase::Running,
                JobPhase::CancelRequested,
                JobPhase::Cancelled,
            ]
        );
        drop(sup);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_os_kill_is_uncertain_and_blocks_new_work() {
        let path = path();
        let sup = Supervisor::open(path.clone()).unwrap();
        let job = sup.prepare(plan()).unwrap();
        sup.record_human_approval(&job.job_id, &job.plan_sha256, &"b".repeat(40), true)
            .unwrap();
        inject_running(&sup, &job);
        let mut broker = TestTerminator {
            verified: false,
            requests: 0,
        };
        let result = sup.cancel_running_with(&job.job_id, &mut broker).unwrap();
        assert_eq!(result.phase, JobPhase::Uncertain);
        assert_eq!(
            sup.prepare(plan()).unwrap_err(),
            SupervisorError::Quarantined
        );
        drop(sup);
        let reopened = Supervisor::open(path.clone()).unwrap();
        assert!(reopened.inspect().unwrap()[0].requires_reconciliation);
        drop(reopened);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn crash_recovery_never_retries_and_requires_os_cleanup_receipt() {
        let path = path();
        let sup = Supervisor::open(path.clone()).unwrap();
        let job = sup.prepare(plan()).unwrap();
        sup.record_human_approval(&job.job_id, &job.plan_sha256, &"b".repeat(40), true)
            .unwrap();
        inject_running(&sup, &job);
        drop(sup);
        let recovered = Supervisor::open(path.clone()).unwrap();
        assert!(recovered.inspect().unwrap()[0].requires_reconciliation);
        let mut broker = TestTerminator {
            verified: true,
            requests: 0,
        };
        let result = recovered
            .reconcile_interrupted_with(&job.job_id, &mut broker)
            .unwrap();
        assert_eq!(result.phase, JobPhase::Cancelled);
        assert_eq!(broker.requests, 1);
        assert!(!recovered.inspect().unwrap()[0].requires_reconciliation);
        let phases: Vec<_> = recovered
            .receipt_history(&job.job_id)
            .unwrap()
            .iter()
            .map(|r| r.body.phase)
            .collect();
        assert_eq!(phases[4], JobPhase::RecoveryRequired);
        assert_eq!(phases[5], JobPhase::Cancelled);
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fresh_one_shot_grant_is_bound_to_verified_plan_and_cannot_replay() {
        let path = path();
        let sup = Supervisor::open(path.clone()).unwrap();
        let plan = plan();
        let job = sup.prepare(plan.clone()).unwrap();
        sup.record_human_approval(&job.job_id, &job.plan_sha256, &plan.source_head, true)
            .unwrap();
        // The real production path must deny; no trusted OS adapter exists.
        assert_eq!(
            sup.arm_for_native_worker(&job.job_id, &plan).unwrap_err(),
            SupervisorError::NoContainment
        );
        let fake_proof = OsContainmentProof {
            platform: "windows".into(),
            restricted_identity: true,
            filesystem_access_limited: true,
            network_access_limited: true,
            child_contained_before_resume: true,
            process_tree_kill_verified: true,
            memory_quota: true,
            cpu_quota: true,
            pid_quota: true,
            protected_journal_broker: true,
        };
        let mut different = plan.clone();
        different.source_head = "f".repeat(40);
        assert_eq!(
            sup.consume_grant_after_attestation(&job.job_id, &different, &fake_proof)
                .unwrap_err(),
            SupervisorError::WrongProjectOrSource
        );
        sup.consume_grant_after_attestation(&job.job_id, &plan, &fake_proof)
            .unwrap();
        assert_eq!(
            sup.consume_grant_after_attestation(&job.job_id, &plan, &fake_proof)
                .unwrap_err(),
            SupervisorError::InvalidTransition
        );
        assert!(sup.state.lock().unwrap().grants.is_empty());
        drop(sup);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn os_containment_is_permanently_fail_closed_until_real_native_acceptance() {
        assert_eq!(
            production_worker_admission().unwrap_err(),
            SupervisorError::NoContainment
        );
        let denied = OsContainmentProof::default();
        assert!(!denied.complete());
        let partial = OsContainmentProof {
            platform: "windows".into(),
            child_contained_before_resume: true,
            process_tree_kill_verified: true,
            memory_quota: true,
            pid_quota: true,
            ..Default::default()
        };
        assert!(!partial.complete());
    }
    #[test]
    fn replay_flags_interrupted_execution_as_reconciliation_required() {
        let p = path();
        let sup = Supervisor::open(p.clone()).unwrap();
        let j = sup.prepare(plan()).unwrap();
        sup.record_human_approval(&j.job_id, &j.plan_sha256, &"b".repeat(40), true)
            .unwrap();
        {
            let mut lock = sup.state.lock().unwrap();
            sup.append(
                &mut lock,
                &j.job_id,
                &j.project_id,
                &j.plan_sha256,
                JobPhase::AdmissionPending,
                "worker_admission_started",
                "test_fault_injection",
            )
            .unwrap();
            lock.jobs.get_mut(&j.job_id).unwrap().phase = JobPhase::AdmissionPending;
        }
        drop(sup);
        let reopened = Supervisor::open(p.clone()).unwrap();
        let state = reopened.inspect().unwrap();
        assert!(state[0].requires_reconciliation);
        assert_eq!(
            reopened.prepare(plan()).unwrap_err(),
            SupervisorError::Quarantined
        );
        // No automatic retry, no assumption that a process died on host restart.
        drop(reopened);
        fs::remove_file(p).unwrap();
    }
}
