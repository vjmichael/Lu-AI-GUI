//! WO6 feature-gated, OS-restricted LLDB-DAP diagnostic coordinator.
//!
//! It can run only the fixed, independently pinned installer-packaged debuggee
//! inside the adapter's AppContainer Job or delegated cgroup/Bubblewrap tree.
//! No project executable, PID, path, arguments, expression, conditional
//! breakpoint, adapter reverse request, or terminal command reaches this API.
#![cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
#[cfg(target_os = "linux")]
use lu_ide_execution_supervisor::linux_worker::lsp_native::{
    native_lsp_bundle_digest, start_linux_dap, start_linux_project_dap,
    LinuxDapSession as NativeDapSession,
};
#[cfg(target_os = "linux")]
use lu_ide_execution_supervisor::linux_worker::reconcile_linux_interrupted;
use lu_ide_execution_supervisor::{
    project_debug_target::SealedProjectDebugTarget,
    snapshot::{publish_input, verify_input, PublishedInput}, JobPhase, JobPlan,
    Supervisor,
};
#[cfg(windows)]
use lu_ide_execution_supervisor::{
    windows_broker::TrustedBroker,
    windows_worker::lsp_native::{
        native_lsp_bundle_digest, start_windows_dap, start_windows_project_dap,
        WindowsDapSession as NativeDapSession,
    },
    windows_worker::reconcile_windows_interrupted_via_service,
};
use lu_ide_workspace::{
    dap_session::{DapEvent, DapPhase},
    dap_source_lines::SourceLineCandidate,
    IdeWorkspaceRegistry,
};
use serde::{Deserialize,Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[cfg(windows)]
const BUNDLE_PIN: Option<&str> = option_env!("LU_IDE_WO6_WINDOWS_LLDB_DAP_SHA256");
#[cfg(windows)]
const SERVICE_PIN: Option<&str> = option_env!("LU_IDE_WO6_WINDOWS_D7D_SERVICE_SHA256");
#[cfg(target_os = "linux")]
const BUNDLE_PIN: Option<&str> = option_env!("LU_IDE_WO6_LINUX_LLDB_DAP_SHA256");
fn pin(source: Option<&str>) -> Result<&str, String> {
    let sha = source.ok_or("DEBUG_RELEASE_PINS_ABSENT")?;
    if sha.len() != 64 || !sha.bytes().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("DEBUG_RELEASE_PIN_INVALID".into());
    }
    Ok(sha)
}
struct Installation {
    bundle: PathBuf,
    digest: String,
    #[cfg(windows)]
    broker: TrustedBroker,
}
#[cfg(windows)]
fn digest_file(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::Read};
    let mut reader = File::open(path).map_err(|_| "DEBUG_SERVICE_MISSING")?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|_| "DEBUG_SERVICE_UNREADABLE")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn installation(app: &AppHandle) -> Result<Installation, String> {
    let approved = pin(BUNDLE_PIN)?;
    #[cfg(windows)]
    {
        let root = app
            .path()
            .resource_dir()
            .map_err(|_| "DEBUG_RESOURCE_MISSING")?;
        let bundle = root.join("debug-adapters").join("lldb-dap");
        let observed = native_lsp_bundle_digest(&bundle, "dap_lldb_fixed")
            .map_err(|_| "DEBUG_BUNDLE_UNTRUSTED")?;
        if observed != approved {
            return Err("DEBUG_BUNDLE_PIN_MISMATCH".into());
        }
        let service = root.join("recovery/windows_recovery_service.exe");
        let sha = pin(SERVICE_PIN)?;
        if digest_file(&service)? != sha {
            return Err("DEBUG_SERVICE_PIN_MISMATCH".into());
        }
        Ok(Installation {
            bundle,
            digest: observed,
            broker: TrustedBroker {
                executable: service,
                expected_sha256: sha.to_owned(),
                private_witness_directory: root.join("recovery"),
            },
        })
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        let bundle = PathBuf::from("/opt/lu-ide/debug-adapters/lldb-dap");
        let observed = native_lsp_bundle_digest(&bundle, "dap_lldb_fixed")
            .map_err(|_| "DEBUG_BUNDLE_UNTRUSTED")?;
        if observed != approved {
            return Err("DEBUG_BUNDLE_PIN_MISMATCH".into());
        }
        Ok(Installation {
            bundle,
            digest: observed,
        })
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedVariableSummary {
    pub name: String,
    pub declared_type: Option<String>,
    pub value_redacted: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugStoppedSource {
    /// No adapter-origin absolute path ever reaches WebView.
    pub relative_path: String,
    pub disk_sha256: String,
    pub line: u32,
    /// We intentionally navigate to column zero: DAP columns have not been
    /// independently proven UTF-16 or cross-checked to a compiler line map.
    pub character_utf16: u32,
    pub frame_id: u64,
    pub source_bytes_verified: bool,
    pub os_instruction_source_line_verified: bool,
    pub installed_app_accepted: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSourceReview {
    pub project_id: String,
    pub job_id: String,
    pub plan_sha256: String,
    pub terminal_receipt_sha256: String,
    pub source: DebugStoppedSource,
    pub navigation_automatic: bool,
    pub debugger_execution_authorized: bool,
    pub human_task_accepted: bool,
}
fn reviewed_initial_stop_source(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    input: &PublishedInput,
    candidate: &SourceLineCandidate,
) -> Option<DebugStoppedSource> {
    // OS process translation is fixed by the native worker. The adapter is
    // NOT allowed to choose a host absolute path or access a foreign project.
    #[cfg(target_os = "linux")]
    let relative = candidate.adapter_path.strip_prefix("/workspace/")?;
    #[cfg(windows)]
    let relative = Path::new(&candidate.adapter_path)
        .strip_prefix(&input.root).ok()?.to_str()?;
    if relative.is_empty() || relative.len() > 512 || relative.contains('\\') ||
        relative.contains(':') || relative.chars().any(char::is_control) ||
        !Path::new(relative).components()
            .all(|part| matches!(part, Component::Normal(_))) ||
        relative.split('/').any(|part| part == "." || part == ".." || part.is_empty()) ||
        candidate.line == 0 || candidate.line > 250_000 ||
        candidate.frame_id == 0 || candidate.frame_id > 1_000_000 ||
        verify_input(input).is_err() {
        return None;
    }
    let path = input.root.join(relative);
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.file_type().is_symlink() ||
        meta.len() == 0 || meta.len() > 512 * 1024 {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {return None;}
    }
    let bytes = fs::read(&path).ok()?;
    if bytes.len() as u64 != meta.len() {return None;}
    let text = std::str::from_utf8(&bytes).ok()?;
    if candidate.line as usize > text.split('\n').count() {return None;}
    let digest = format!("{:x}",Sha256::digest(&bytes));
    let saved = registry.preview_source_file(project_id,relative).ok()?;
    if saved.project_id != project_id || saved.relative_path != relative ||
       saved.revision != digest || saved.text != text {
        return None;
    }
    Some(DebugStoppedSource {
        relative_path: relative.to_owned(),disk_sha256:digest,
        line:candidate.line,character_utf16:0,frame_id:candidate.frame_id,
        source_bytes_verified:true,
        // A DAP stack line checked against saved bytes is NOT independent
        // stopped OS RIP -> DWARF/PDB line-map proof. This stays false.
        os_instruction_source_line_verified:false,
        installed_app_accepted:false,
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugWitness {
    pub job_id: String,
    pub project_id: String,
    pub source_head: String,
    pub plan_sha256: String,
    pub bundle_sha256: String,
    pub terminal_receipt_sha256: String,
    pub stopped_source: Option<DebugStoppedSource>,
    pub stopped_thread: Option<u64>,
    pub threads_response_observed: bool,
    pub stack_response_observed: bool,
    pub terminal_phase: String,
    pub verified_os_tree_release_by_worker: bool,
    pub installed_app_accepted: bool,
    pub target: &'static str,
    pub origin_build_job_id: Option<String>,
    pub origin_build_plan_sha256: Option<String>,
    pub origin_build_receipt_sha256: Option<String>,
    pub binary_sha256: Option<String>,
    pub symbol_sha256: Option<String>,
    pub target_binding_sha256: Option<String>,
    pub operation_outcome_code: Option<&'static str>,
    pub operation_receipt_sha256: Option<String>,
    /// Only developer-only, physically consented one-shot DAP names/types.
    /// No adapter values, memory references, handles or expressions.
    pub inspected_frame_id: Option<u64>,
    pub redacted_variables: Option<Vec<RedactedVariableSummary>>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugActive {
    project_id: String,
    job_id: Option<String>,
    cancellation_requested: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugStatus {
    ready: bool,
    /// All protected debugger jobs share OS cleanup custody, even when the
    /// Jobs pane is currently viewing a different project.
    global_recovery_required: bool,
    active_in_another_project: bool,
    orphaned_debug_action_required: bool,
    project_debug_supported: bool,
    operation_control_supported: bool,
    operation_control_code: &'static str,
    code: String,
    current: Option<DebugActive>,
    last: Option<DebugWitness>,
    journal: Vec<DebugInterrupted>,
    quarantined: Vec<DebugInterrupted>,
    description: &'static str,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugInterrupted {
    project_id: String,
    job_id: String,
    plan_sha256: String,
    phase: String,
    requires_reconciliation: bool,
}
/// FIN-04: candidate control names, NOT an executable DAP enum.
#[derive(Clone,Copy,Debug,Deserialize,Serialize)]
#[serde(rename_all="snake_case")]
pub enum DebugControlReviewKind {
    StepOver,StepInto,StepOut,BreakpointAtReviewedLine,InspectReviewedFrame,
}
impl DebugControlReviewKind {
    fn label(self)->&'static str {
        match self{
            Self::StepOver=>"step_over",
            Self::StepInto=>"step_into",
            Self::StepOut=>"step_out",
            Self::BreakpointAtReviewedLine=>"breakpoint_at_reviewed_line",
            Self::InspectReviewedFrame=>"inspect_reviewed_frame",
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct DebugControlCandidateReview {
    project_id:String,
    original_debug_job_id:String,
    original_plan_sha256:String,
    original_terminal_receipt_sha256:String,
    kind:DebugControlReviewKind,
    reviewed_source_relative_path:String,
    reviewed_source_sha256:String,
    reviewed_source_line:u32,
    adapter_reported_frame_id:u64,
    original_action_rows_visible:usize,
    original_action_cleanup_required:bool,
    original_debug_journal_sha256:String,
    action_manifest_sha256:String,
    global_action_cleanup_required:bool,
    review_sha256:String,
    blockers:Vec<&'static str>,
    source_bytes_rechecked:bool,
    live_native_owner_event_verified:bool,
    kernel_instruction_source_line_verified:bool,
    reviewed_frame_independently_verified:bool,
    installed_debugger_controls_accepted:bool,
    execution_route_available:bool,
    debugger_execution_authorized:bool,
    replay_authorized:bool,
}
#[derive(Clone, Copy, Debug)]
pub enum ReviewedNativeAction {
    StepOver,
    SameLineBreakpoint,
    InspectVariables,
}
struct Active {
    project_id: String,
    job_id: Option<String>,
    cancellation: Arc<AtomicBool>,
}
/// The native OS dialog must not hold a protected stopped Job forever.
/// A late callback has no receiver and can never produce a consent marker.
/// Used for developer-only debugger actions; this is not a CI-simulated yes.
#[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
fn bounded_native_debugger_consent(app: &AppHandle, message: &str, cancel: &AtomicBool) -> bool {
    use std::sync::mpsc::{sync_channel, RecvTimeoutError};
    if cancel.load(Ordering::SeqCst) {
        return false;
    }
    let (sender, receiver) = sync_channel(1);
    app.dialog()
        .message(message.to_owned())
        .title("Lu IDE · one debugger operation")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Approve this single operation".into(),
            "Decline".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.try_send(confirmed);
        });
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(confirmed) => return confirmed && !cancel.load(Ordering::SeqCst),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

pub struct NativeDebugRuntime {
    supervisor: Arc<Supervisor>,
    global_custody_profile: PathBuf,
    stage: PathBuf,
    active: Mutex<Option<Active>>,
    last: Mutex<Option<DebugWitness>>,
}
impl NativeDebugRuntime {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let data = app
            .path()
            .app_data_dir()
            .map_err(|_| "DEBUG_PROFILE_MISSING")?;
        fs::create_dir_all(&data).map_err(|_| "DEBUG_PROFILE_UNAVAILABLE")?;
        let stage = data.join("debug-fixture-snapshots");
        fs::create_dir_all(&stage).map_err(|_| "DEBUG_PROFILE_UNAVAILABLE")?;
        let supervisor = Supervisor::open(data.join("debug-fixture-receipts.jsonl"))
            .map_err(|e| e.code().to_owned())?;
        Ok(Self {
            supervisor: Arc::new(supervisor),
            global_custody_profile: data.clone(),
            stage,
            active: Mutex::new(None),
            last: Mutex::new(None),
        })
    }
    pub fn status(&self, app: &AppHandle, project_id: &str) -> Result<DebugStatus, String> {
        let available = installation(app).map(|_| ());
        let current = self
            .active
            .lock()
            .map_err(|_| "DEBUG_HOST_UNAVAILABLE")?
            .as_ref()
            .filter(|v| v.project_id == project_id)
            .map(|v| DebugActive {
                project_id: v.project_id.clone(),
                job_id: v.job_id.clone(),
                cancellation_requested: v.cancellation.load(Ordering::SeqCst),
            });
        let last = self
            .last
            .lock()
            .map_err(|_| "DEBUG_HOST_UNAVAILABLE")?
            .as_ref()
            .filter(|v| v.project_id == project_id)
            .cloned();
        let durable = self.supervisor.inspect().map_err(|e| e.code().to_owned())?;
        let global_custody =
            lu_ide_execution_supervisor::observe_global_custody(&durable,project_id);
        let unresolved_global_marker=crate::global_native_custody
            ::global_custody_unresolved(&self.global_custody_profile)?;
        let other=crate::global_native_custody::sibling_rows(
            &self.global_custody_profile,"debug")?;
        let other_custody=lu_ide_execution_supervisor
            ::observe_global_custody(&other,project_id);
        let global_recovery_required=global_custody.recovery_required ||
            other_custody.recovery_required || unresolved_global_marker;
        let active_in_another_project = global_custody.active_in_another_project ||
            other_custody.active_in_another_project ||
            self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?
            .as_ref().is_some_and(|value| value.project_id != project_id);
        // FIN-04: *every* native debugger build replays all original native
        // action journals, including foreign-project records, before ready
        // can become true. A terminal Job alone cannot clear an in-flight
        // debugger action. The inspector discloses only a global blocker.
        let action_evidence=self.inspect_control_evidence(project_id)?;
        let original_action_cleanup_required=
            action_evidence.global_os_cleanup_required;
        let global_recovery_required =
            global_recovery_required || original_action_cleanup_required;
        // Security decision uses every outstanding job, not the bounded UI list.
        let quarantined: Vec<DebugInterrupted> = durable
            .iter()
            .filter(|job| job.project_id == project_id && job.requires_reconciliation)
            .map(|job| DebugInterrupted {
                project_id: job.project_id.clone(),
                job_id: job.job_id.clone(),
                plan_sha256: job.plan_sha256.clone(),
                phase: match job.phase {
                    JobPhase::CancelRequested => "cancel_requested",
                    JobPhase::RecoveryRequired => "recovery_required",
                    JobPhase::AdmissionPending => "admission_pending",
                    JobPhase::ReleasePending => "release_pending",
                    JobPhase::Uncertain => "uncertain",
                    JobPhase::Completed => "completed",
                    JobPhase::Cancelled => "cancelled",
                    JobPhase::Failed => "failed",
                    JobPhase::Denied => "denied",
                    JobPhase::Running => "running",
                    JobPhase::Prepared => "prepared",
                    JobPhase::Authorized => "authorized",
                }
                .to_owned(),
                requires_reconciliation: true,
            })
            .collect();
        // Last successful DAP result is session-only; never fabricate it after
        // restart. Display the newest persisted job IDs as journal metadata.
        let mut journal: Vec<DebugInterrupted> = durable
            .into_iter()
            .rev()
            .filter(|job| job.project_id == project_id)
            .take(32)
            .map(|job| DebugInterrupted {
                project_id: job.project_id,
                job_id: job.job_id,
                plan_sha256: job.plan_sha256,
                phase: match job.phase {
                    JobPhase::CancelRequested => "cancel_requested",
                    JobPhase::RecoveryRequired => "recovery_required",
                    JobPhase::AdmissionPending => "admission_pending",
                    JobPhase::ReleasePending => "release_pending",
                    JobPhase::Uncertain => "uncertain",
                    JobPhase::Completed => "completed",
                    JobPhase::Cancelled => "cancelled",
                    JobPhase::Failed => "failed",
                    JobPhase::Denied => "denied",
                    JobPhase::Running => "running",
                    JobPhase::Prepared => "prepared",
                    JobPhase::Authorized => "authorized",
                }
                .to_owned(),
                requires_reconciliation: job.requires_reconciliation,
            })
            .collect();
        journal.reverse();
        let native_admission_available = available.is_ok()
            && !global_recovery_required
            && !global_custody.worker_busy_or_unreconciled
            && !other_custody.worker_busy_or_unreconciled
            && !unresolved_global_marker
            && current.is_none()
            && !active_in_another_project
            && !original_action_cleanup_required;
        Ok(DebugStatus {
            ready: native_admission_available,
            global_recovery_required,
            active_in_another_project,
            orphaned_debug_action_required: original_action_cleanup_required,
            project_debug_supported: cfg!(feature = "native-project-debug")
                && native_admission_available,
            // Test-only D6C permission never becomes installed action authority.
            operation_control_supported: cfg!(all(
                windows,
                debug_assertions,
                feature = "dev046d-native-dialog"
            )) && native_admission_available,
            operation_control_code: if cfg!(all(
                windows,
                debug_assertions,
                feature = "dev046d-native-dialog"
            )) {
                "DEV046D_NATIVE_DIALOG_DEV_ONLY_NOT_RELEASE_ACCEPTED"
            } else {
                "DEV046D_NATIVE_HUMAN_ACTION_APPROVAL_NOT_ACCEPTED"
            },
            code: if original_action_cleanup_required {
                "DEV046D_INTERRUPTED_ACTION_OS_RECONCILIATION_REQUIRED".into()
            } else if global_recovery_required {
                "IDE_INT04_GLOBAL_KERNEL_RECONCILIATION_REQUIRED".into()
            } else if active_in_another_project {
                "IDE_INT04_OTHER_PROJECT_DEBUGGER_ACTIVE".into()
            } else {
                match available {
                    Ok(()) => "DEBUG_FIXTURE_BUNDLE_VERIFIED_OS_ADMISSION_PER_RUN".into(),
                    Err(code) => code,
                }
            },
            current,
            last,
            journal,
            quarantined,
            description: if cfg!(all(
                windows,
                debug_assertions,
                feature = "dev046d-native-dialog"
            )) {
                "Developer-only native modal for a verified LLDB stop. A fresh build, launch approval and per-action consent are mandatory; this is NOT signed installed acceptance."
            } else {
                "Fixed LLDB self-test and separately approved sealed project launch are feature-gated. Installed step/breakpoint control remains disabled."
            },
        })
    }
    /// FIN-04: original action-journal evidence inventory, never a debugger
    /// operation grant. This does not reconstruct native owner event handles.
    pub fn inspect_control_evidence(
        &self, project_id:&str
    )->Result<lu_ide_execution_supervisor::debug_control_evidence::DebugControlEvidenceInventory,String>{
        lu_ide_execution_supervisor::debug_control_evidence
            ::inspect_debug_control_evidence(
                &self.global_custody_profile,&self.supervisor,project_id)
            .map_err(str::to_owned)
    }
    /// Read-only control *candidate* from the earlier exact terminal LLDB
    /// source observation, original terminal job receipt, and inspected
    /// action journal. No stopped process exists for this candidate: even
    /// a complete SHA-256 does not authorize a fresh DAP request.
    pub fn review_control_candidate(
        &self,registry:&IdeWorkspaceRegistry,project_id:&str,
        job_id:&str,plan_sha256:&str,kind:DebugControlReviewKind,
    )->Result<DebugControlCandidateReview,String>{
        let source=self.review_stopped_source(
            registry,project_id,job_id,plan_sha256)?;
        let audit=self.inspect_control_evidence(project_id)?;
        let observed=&source.source;
        let mut hash=Sha256::new();
        hash.update(b"lu-ide-fin04-read-only-control-candidate-v1\n");
        for part in [
            project_id,job_id,plan_sha256,
            source.terminal_receipt_sha256.as_str(),kind.label(),
            observed.relative_path.as_str(),observed.disk_sha256.as_str(),
            audit.original_debug_journal_sha256.as_str(),
            audit.action_manifest_sha256.as_str(),
        ]{
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part.as_bytes());
        }
        hash.update(observed.line.to_le_bytes());
        hash.update(observed.frame_id.to_le_bytes());
        let mut blockers=vec![
            "no_live_native_owner_event",
            "debugger_job_already_terminal",
            "fin01d_native_kernel_acceptance_not_proven",
            "installed_debugger_control_not_accepted",
            "independent_instruction_source_mapping_missing",
            "one_use_execution_consent_not_granted",
        ];
        if audit.requires_os_cleanup{
            blockers.push("original_debug_action_os_recovery_required");
        }
        if audit.global_os_cleanup_required{
            blockers.push("global_debugger_action_or_job_recovery_required");
        }
        if matches!(kind,DebugControlReviewKind::InspectReviewedFrame){
            blockers.push("adapter_frame_id_is_not_kernel_frame_proof");
        }
        Ok(DebugControlCandidateReview{
            project_id:project_id.to_owned(),
            original_debug_job_id:job_id.to_owned(),
            original_plan_sha256:plan_sha256.to_owned(),
            original_terminal_receipt_sha256:source.terminal_receipt_sha256,
            kind,
            reviewed_source_relative_path:observed.relative_path.clone(),
            reviewed_source_sha256:observed.disk_sha256.clone(),
            reviewed_source_line:observed.line,
            adapter_reported_frame_id:observed.frame_id,
            original_action_rows_visible:audit.rows.len(),
            original_action_cleanup_required:audit.requires_os_cleanup,
            original_debug_journal_sha256:audit.original_debug_journal_sha256,
            action_manifest_sha256:audit.action_manifest_sha256,
            global_action_cleanup_required:audit.global_os_cleanup_required,
            review_sha256:format!("{:x}",hash.finalize()),
            blockers,source_bytes_rechecked:true,
            live_native_owner_event_verified:false,
            kernel_instruction_source_line_verified:false,
            reviewed_frame_independently_verified:false,
            installed_debugger_controls_accepted:false,
            execution_route_available:false,
            debugger_execution_authorized:false,
            replay_authorized:false,
        })
    }
    /// INT04: one read-only source review for an earlier, successfully
    /// terminated protected LLDB session. No debuggee, DAP request, source
    /// path, frame ID, or authorization token is accepted from the WebView.
    /// A debugger stack coordinate remains adapter-reported: OS source
    /// mapping and independent instruction ownership are NOT inferred.
    pub fn review_stopped_source(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        job_id: &str,
        plan_sha256: &str,
    ) -> Result<DebugSourceReview, String> {
        if !job_id.starts_with("job-") || job_id.len() != 36 ||
            !job_id[4..].bytes().all(|byte| byte.is_ascii_hexdigit()) ||
            plan_sha256.len()!=64 ||
            !plan_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("INT04_INVALID_DEBUG_SESSION_IDENTITY".into());
        }
        if self.active.lock().map_err(|_| "INT04_DEBUG_HOST_BUSY")?.is_some() {
            return Err("INT04_DEBUG_SESSION_STILL_ACTIVE".into());
        }
        let witnessed = self.last.lock()
            .map_err(|_| "INT04_LAST_DEBUG_RECEIPT_UNAVAILABLE")?
            .as_ref().cloned().ok_or("INT04_NO_SAME_PROCESS_DEBUG_EVIDENCE")?;
        if witnessed.project_id != project_id || witnessed.job_id != job_id ||
            witnessed.plan_sha256 != plan_sha256 ||
            witnessed.terminal_phase != "completed" ||
            witnessed.target != "one_exact_private_project_build_in_protected_job" ||
            !witnessed.verified_os_tree_release_by_worker ||
            witnessed.installed_app_accepted {
            return Err("INT04_DEBUG_ORIGINAL_SESSION_MISMATCH".into());
        }
        let source = witnessed.stopped_source.as_ref()
            .ok_or("INT04_NO_SAVED_STOPPED_SOURCE_ANCHOR")?;
        if !source.source_bytes_verified ||
            source.os_instruction_source_line_verified ||
            source.installed_app_accepted {
            return Err("INT04_DEBUG_SOURCE_AUTHORITY_MISMATCH".into());
        }
        let current = self.supervisor.inspect().map_err(|e| e.code().to_owned())?;
        let original = current.iter().find(|entry| entry.job_id == job_id)
            .ok_or("INT04_ORIGINAL_DEBUG_JOB_NOT_FOUND")?;
        if original.project_id != project_id ||
            original.plan_sha256 != plan_sha256 ||
            original.phase != JobPhase::Completed ||
            original.requires_reconciliation ||
            current.iter().any(|entry| entry.requires_reconciliation) {
            return Err("INT04_NATIVE_DEBUG_KERNEL_RECOVERY_REQUIRED".into());
        }
        let receipts = self.supervisor.receipt_history(job_id)
            .map_err(|e| e.code().to_owned())?;
        let last = receipts.last().ok_or("INT04_DEBUG_JOURNAL_EMPTY")?;
        if last.entry_hash != witnessed.terminal_receipt_sha256 ||
            last.body.project_id != project_id ||
            last.body.plan_sha256 != plan_sha256 ||
            last.body.phase != JobPhase::Completed {
            return Err("INT04_DEBUG_JOURNAL_RECEIPT_CHANGED".into());
        }
        let saved = registry.preview_source_file(project_id,&source.relative_path)
            .map_err(|_| "INT04_STOPPED_SOURCE_NOT_REGISTERED")?;
        if saved.project_id != project_id ||
            saved.relative_path != source.relative_path ||
            saved.revision != source.disk_sha256 ||
            source.line == 0 ||
            source.line as usize > saved.text.split('\n').count() {
            return Err("INT04_STOPPED_SOURCE_CHANGED".into());
        }
        Ok(DebugSourceReview {
            project_id: project_id.to_owned(),job_id:job_id.to_owned(),
            plan_sha256:plan_sha256.to_owned(),
            terminal_receipt_sha256:witnessed.terminal_receipt_sha256,
            source:source.clone(),
            navigation_automatic:false,
            debugger_execution_authorized:false,
            human_task_accepted:false,
        })
    }
    /// A separately approved OS-only cleanup, never a debugger resume.
    /// Missing/stale project receipts remain quarantined across restarts.
    pub fn reconcile(
        &self,
        app: &AppHandle,
        project_id: &str,
        job_id: &str,
    ) -> Result<&'static str, String> {
        if self
            .active
            .lock()
            .map_err(|_| "DEBUG_HOST_UNAVAILABLE")?
            .is_some()
        {
            return Err("DEBUG_ACTIVE_SESSION_PREVENTS_RECOVERY".into());
        }
        let interrupted = self
            .supervisor
            .inspect()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or("DEBUG_RECOVERY_JOB_NOT_FOUND")?;
        if interrupted.project_id != project_id || !interrupted.requires_reconciliation {
            return Err("DEBUG_RECOVERY_SCOPE_DENIED".into());
        }
        let install = installation(app)?;
        let confirmed = app
            .dialog()
            .message(format!(
                "Verify native OS-only termination of interrupted debugger job {job_id}?\n\n\
                 No debugger or project executable will be restarted. The protected broker must \
                 verify zero descendants and fsync its terminal receipt."
            ))
            .title("Reconcile interrupted protected debugger")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Verify OS cleanup".into(),
                "Leave quarantined".into(),
            ))
            .blocking_show();
        if !confirmed {
            return Err("DEBUG_RECOVERY_NOT_APPROVED".into());
        }
        #[cfg(windows)]
        let summary =
            reconcile_windows_interrupted_via_service(&self.supervisor, job_id, &install.broker)
                .map_err(|e| e.code().to_owned())?;
        #[cfg(target_os = "linux")]
        let summary = {
            let _ = install;
            reconcile_linux_interrupted(&self.supervisor, job_id)
                .map_err(|e| e.code().to_owned())?
        };
        if summary.phase != JobPhase::Cancelled || summary.requires_reconciliation {
            return Err("DEBUG_RECOVERY_STILL_QUARANTINED".into());
        }
        Ok("DEBUG_OS_TREE_RECONCILED_NO_REEXECUTION")
    }

    pub fn cancel(&self, project_id: &str, job_id: Option<&str>) -> Result<&'static str, String> {
        let locked = self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?;
        let active = locked.as_ref().ok_or("DEBUG_NO_ACTIVE_SESSION")?;
        if active.project_id != project_id {
            return Err("DEBUG_SESSION_SCOPE_MISMATCH".into());
        }
        match (active.job_id.as_deref(), job_id) {
            (Some(current), Some(expected)) if current == expected => {}
            (None, None) => {}
            _ => return Err("DEBUG_EXACT_JOB_ID_REQUIRED".into()),
        }
        if active.cancellation.swap(true, Ordering::SeqCst) {
            return Err("DEBUG_CANCEL_ALREADY_REQUESTED".into());
        }
        Ok("DEBUG_CANCEL_REQUESTED_OS_TERMINATION_PENDING")
    }
    pub fn run_fixture(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
    ) -> Result<DebugWitness, String> {
        self.run_bound(app, registry, project_id, None, None)
    }
    #[cfg(feature = "native-project-debug")]
    pub fn run_project(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        target: Arc<SealedProjectDebugTarget>,
    ) -> Result<DebugWitness, String> {
        self.run_bound(app, registry, project_id, Some(target), None)
    }
    #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
    pub fn run_project_native_action(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        target: Arc<SealedProjectDebugTarget>,
        operation: ReviewedNativeAction,
    ) -> Result<DebugWitness, String> {
        self.run_bound(app, registry, project_id, Some(target), Some(operation))
    }
    fn run_bound(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        target: Option<Arc<SealedProjectDebugTarget>>,
        operation: Option<ReviewedNativeAction>,
    ) -> Result<DebugWitness, String> {
        if self
            .supervisor
            .inspect()
            .map_err(|e| e.code().to_owned())?
            .iter()
            .any(|job| job.requires_reconciliation)
        {
            return Err("DEBUG_KERNEL_RECOVERY_REQUIRED_BEFORE_ANY_NEW_RUN".into());
        }
        // A profile-wide native action audit is mandatory even when the
        // current project has no visible interrupted Job. No release build
        // may infer control safety from a terminal debugger journal alone.
        if self.inspect_control_evidence(project_id)?
            .global_os_cleanup_required {
            return Err("FIN04_ORIGINAL_ACTION_OS_RECOVERY_REQUIRED".into());
        }
        // Cross-supervisor, cross-process admission before reserving the
        // debugger slot or generating fresh native worker authority.
        // Refuse known busy journals BEFORE creating the profile-wide marker.
        // The exclusive create_new still serializes the final admission, and
        // the original journals are rechecked while it is held.
        crate::global_native_custody::preflight_global_idle(
            &self.global_custody_profile,"debug")?;
        let mut global_fence=crate::global_native_custody::GlobalAdmissionFence
            ::acquire(&self.global_custody_profile,"debug")?;
        let own_before=self.supervisor.inspect()
            .map_err(|e|e.code().to_owned())?;
        if crate::global_native_custody::durable_busy(&own_before) {
            let after=self.supervisor.inspect().map_err(|e|e.code().to_owned())?;
            global_fence.release_before_prepare_if_unchanged(&own_before,&after)?;
            return Err("FIN01_DEBUG_JOURNAL_STILL_OWNS_CUSTODY".into());
        }
        // Repeat the original action audit under the new global admission
        // fence: source-level preflight alone is not enough across processes.
        // If an action changed since preflight, leave the marker quarantined
        // rather than dispatch a worker with uncertain authority.
        if self.inspect_control_evidence(project_id)?
            .global_os_cleanup_required {
            let after=self.supervisor.inspect().map_err(|e|e.code().to_owned())?;
            global_fence.release_before_prepare_if_unchanged(&own_before,&after)?;
            return Err("FIN04_ACTION_RECOVERY_REQUIRED_INSIDE_GLOBAL_FENCE".into());
        }
        let sibling=crate::global_native_custody::sibling_rows(
            &self.global_custody_profile,"debug")?;
        if crate::global_native_custody::durable_busy(&sibling){
            let after=self.supervisor.inspect().map_err(|e|e.code().to_owned())?;
            global_fence.release_before_prepare_if_unchanged(&own_before,&after)?;
            return Err("FIN01_BUILD_OR_LSP_JOURNAL_STILL_OWNS_CUSTODY".into());
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?;
            if active.is_some() {
                drop(active);
                let after=self.supervisor.inspect().map_err(|e|e.code().to_owned())?;
                global_fence.release_before_prepare_if_unchanged(&own_before,&after)?;
                return Err("DEBUG_SESSION_ALREADY_ACTIVE".into());
            }
            // Serialize launch and read-only result invalidation together.
            // A poisoned evidence lock must not leave a phantom live worker.
            *self.last.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")? = None;
            *active = Some(Active {
                project_id: project_id.to_owned(),
                job_id: None,
                cancellation: Arc::clone(&cancellation),
            });
        }
        let result = self.run_inner(
            app,
            registry,
            project_id,
            &cancellation,
            target.as_deref(),
            operation,
        );
        // A refused debugger consent before OS admission may append only
        // the original supervisor's strictly Prepared/Authorized terminal
        // cancellation. Never reclassify a launched or uncertain Job as idle.
        if result.is_err() {
            let current_id=self.active.lock()
                .map_err(|_|"DEBUG_HOST_UNAVAILABLE")?
                .as_ref().and_then(|a|a.job_id.clone());
            if let Some(id)=current_id {
                let state=self.supervisor.inspect().map_err(|e|e.code().to_owned())?
                    .into_iter().find(|j|j.job_id==id)
                    .ok_or("FIN01_DEBUG_ORIGINAL_JOB_MISSING")?;
                if matches!(state.phase,JobPhase::Prepared|JobPhase::Authorized){
                    self.supervisor.cancel_before_start(&id)
                        .map_err(|e|e.code().to_owned())?;
                }
            }
        }
        *self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")? = None;
        if let Ok(ref witness) = result {
            *self.last.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")? = Some(witness.clone());
        }
        let own=self.supervisor.inspect().map_err(|e|e.code().to_owned());
        let sibling=crate::global_native_custody::sibling_rows(
            &self.global_custody_profile,"debug");
        let release=match (own,sibling){
            (Ok(own),Ok(sibling))=>global_fence.release_if_terminal(&own,&sibling),
            _=>Err("FIN01_GLOBAL_TERMINAL_PROOF_UNAVAILABLE".into()),
        };
        if let Err(reason)=release {
            if result.is_ok(){return Err(reason);}
        }
        result
    }
    fn run_inner(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        cancel: &AtomicBool,
        target: Option<&SealedProjectDebugTarget>,
        operation: Option<ReviewedNativeAction>,
    ) -> Result<DebugWitness, String> {
        if operation.is_some()
            && !cfg!(all(
                windows,
                debug_assertions,
                feature = "dev046d-native-dialog"
            ))
        {
            return Err("DEV046D_NATIVE_DIALOG_FEATURE_DISABLED".into());
        }
        if operation.is_some() && target.is_none() {
            return Err("DEV046D_REQUIRES_VERIFIED_PROJECT_TARGET".into());
        }
        let install = installation(app)?;
        let root = registry
            .trusted_native_worker_root(project_id)
            .map_err(|_| "DEBUG_PROJECT_NOT_ACTIVE")?;
        let git = registry.inspect_git_worktree(project_id);
        if git.state != "local_repository_inspected" || git.worktree != "clean" || git.truncated {
            return Err("DEBUG_REQUIRES_CLEAN_PROJECT".into());
        }
        let head = git.local_head.ok_or("DEBUG_SOURCE_HEAD_UNAVAILABLE")?;
        if head.len() != 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("DEBUG_SOURCE_HEAD_INVALID".into());
        }
        let input =
            publish_input(&root, &self.stage, "rust_check").map_err(|e| e.code().to_owned())?;
        if let Some(target) = target {
            if target.lineage.project_id != project_id
                || target.lineage.source_head != head
                || target.lineage.source_snapshot_sha256 != input.seal.source_sha256
                || target.lineage.manifest_sha256 != input.seal.manifest_sha256
                || target.lineage.requires_reconciliation
            {
                return Err("DEV044_BUILD_SOURCE_OR_RECEIPT_MISMATCH".into());
            }
            target
                .verify()
                .map_err(|_| "DEV044_PROJECT_BINARY_OR_SYMBOLS_CHANGED")?;
        }
        if cancel.load(Ordering::SeqCst) {
            return Err("DEBUG_CANCELLED_BEFORE_NATIVE_AUTHORIZATION".into());
        }
        let plan = JobPlan {
            project_id: project_id.to_owned(),
            recipe_id: if target.is_some() {
                "dap_lldb_project"
            } else {
                "dap_lldb_fixed"
            }
            .into(),
            source_head: head.clone(),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: install.digest.clone(),
            debug_target_binding_sha256: target.map(|target| target.binding_sha256.clone()),
            runtime_limit_ms: 3 * 60_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 16,
            log_limit_bytes: 64 * 1024,
            effect_class: "debug_inspection_isolated".into(),
        };
        let prepared = self
            .supervisor
            .prepare(plan.clone())
            .map_err(|e| e.code().to_owned())?;
        {
            let mut active = self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?;
            let current = active.as_mut().ok_or("DEBUG_HOST_UNAVAILABLE")?;
            if current.project_id != project_id {
                return Err("DEBUG_SCOPE_CHANGED".into());
            }
            current.job_id = Some(prepared.job_id.clone());
        }
        if cancel.load(Ordering::SeqCst) {
            return Err("DEBUG_CANCELLED_BEFORE_NATIVE_AUTHORIZATION".into());
        }
        let (dialog_title, dialog_message, button) = if let Some(target) = target {
            target
                .verify()
                .map_err(|_| "DEV044_PRIVATE_BINARY_CHANGED_BEFORE_APPROVAL")?;
            (
                "Approve one protected project debugger",
                format!(
                    "Debug ONLY this independently sealed project Cargo executable under a new protected OS job?\n\n\
                    Project: {project_id}\nBuild job: {}\nBuild receipt SHA-256: {}\n\
                    Source snapshot SHA-256: {}\nBinary SHA-256: {}\n\
                    Symbols SHA-256: {}\nNew D3 target binding: {}\n\n\
                    No arbitrary attach, shell, terminal, path, arguments, expressions or credentials.\
                    Cancellation requires verified kernel process-tree termination.",
                    target.lineage.build_job_id,
                    target.lineage.terminal_receipt_sha256,
                    target.lineage.source_snapshot_sha256,
                    target.binary.binary_sha256,
                    target.symbols.symbols_sha256,target.binding_sha256
                ),
                "Debug this exact project build"
            )
        } else {
            (
                "Approve one protected native debugger self-test",
                format!(
                    "Run the reviewed fixed LLDB-DAP debugger self-test?\nProject: {project_id}\n\
                     Source HEAD: {head}\nFrozen input SHA-256: {}\nAdapter SHA-256: {}",
                    plan.input_snapshot_sha256, install.digest
                ),
                "Run fixed debugger self-test",
            )
        };
        let decision = app.dialog().message(dialog_message);
        let decision = decision
            .title(dialog_title)
            .buttons(MessageDialogButtons::OkCancelCustom(
                button.into(),
                "Cancel".into(),
            ))
            .blocking_show();
        if !decision || cancel.load(Ordering::SeqCst) {
            return Err("DEBUG_HUMAN_APPROVAL_DENIED".into());
        }
        let fresh = registry.inspect_git_worktree(project_id);
        if registry
            .trusted_native_worker_root(project_id)
            .ok()
            .as_ref()
            != Some(&root)
            || fresh.state != "local_repository_inspected"
            || fresh.worktree != "clean"
            || fresh.local_head.as_deref() != Some(head.as_str())
            || fresh.truncated
        {
            return Err("DEBUG_SOURCE_CHANGED_DURING_APPROVAL".into());
        }
        if let Some(target) = target {
            target
                .verify()
                .map_err(|_| "DEV044_PRIVATE_TARGET_CHANGED_DURING_APPROVAL")?;
        }
        self.supervisor
            .record_human_approval(&prepared.job_id, &prepared.plan_sha256, &head, true)
            .map_err(|e| e.code().to_owned())?;
        #[cfg(windows)]
        let mut session: NativeDapSession = if let Some(target) = target {
            start_windows_project_dap(
                Arc::clone(&self.supervisor),
                &prepared.job_id,
                &plan,
                &head,
                &input,
                &install.bundle,
                &install.broker,
                target,
            )
        } else {
            start_windows_dap(
                Arc::clone(&self.supervisor),
                &prepared.job_id,
                &plan,
                &head,
                &input,
                &install.bundle,
                &install.broker,
            )
        }
        .map_err(|e| e.code().to_owned())?;
        #[cfg(target_os = "linux")]
        let mut session: NativeDapSession = if let Some(target) = target {
            start_linux_project_dap(
                Arc::clone(&self.supervisor),
                &prepared.job_id,
                &plan,
                &head,
                &input,
                &install.bundle,
                target,
            )
        } else {
            start_linux_dap(
                Arc::clone(&self.supervisor),
                &prepared.job_id,
                &plan,
                &head,
                &input,
                &install.bundle,
            )
        }
        .map_err(|e| e.code().to_owned())?;
        session.initialize().map_err(|e| e.code().to_owned())?;
        let init_at = Instant::now();
        while session.phase() != DapPhase::Ready && init_at.elapsed() < Duration::from_secs(15) {
            if cancel.load(Ordering::SeqCst) {
                return Err("DEBUG_CANCELLED_OS_TREE_CLEANUP_REQUIRED".into());
            }
            session.poll().map_err(|e| e.code().to_owned())?;
            thread::sleep(Duration::from_millis(30));
        }
        if session.phase() != DapPhase::Ready {
            return Err("DEBUG_INITIALIZE_TIMEOUT".into());
        }
        if let Some(target) = target {
            target
                .verify()
                .map_err(|_| "DEV044_BINARY_CHANGED_BEFORE_LAUNCH")?;
            session
                .launch_project_target()
                .map_err(|e| e.code().to_owned())?;
        } else {
            session
                .launch_fixed_fixture()
                .map_err(|e| e.code().to_owned())?;
        }
        // Handle a valid initialized event already consumed during the
        // initialize response read. Never issue configuration before launch.
        if session.configuration_needed() {
            session
                .configuration_done()
                .map_err(|e| e.code().to_owned())?;
        }
        let began = Instant::now();
        let mut stopped_thread = None;
        let mut threads_response = false;
        let mut stack_response = false;
        let mut queried_thread = false;
        let mut queried_stack = false;
        while began.elapsed() < Duration::from_secs(30) {
            if cancel.load(Ordering::SeqCst) {
                return Err("DEBUG_CANCELLED_OS_TREE_CLEANUP_REQUIRED".into());
            }
            for event in session.poll().map_err(|e| e.code().to_owned())? {
                match event {
                    DapEvent::ConfigurationRequested => {
                        // LLDB may send initialized AFTER the pinned native launch.
                        // Complete only empty configuration; do not accept an
                        // arbitrary source, terminal or debugger command.
                        session
                            .configuration_done()
                            .map_err(|e| e.code().to_owned())?;
                    }
                    DapEvent::Stopped { thread_id } => stopped_thread = Some(thread_id),
                    DapEvent::ThreadsReceived => threads_response = true,
                    DapEvent::StackTraceReceived => stack_response = true,
                    DapEvent::Terminated => {
                        return Err("DEBUG_FIXED_DEBUGGEE_EXITED_BEFORE_INSPECTION".into())
                    }
                    _ => {}
                }
            }
            if stopped_thread.is_some() && !queried_thread && session.phase() == DapPhase::Stopped {
                session.threads().map_err(|e| e.code().to_owned())?;
                queried_thread = true;
            }
            if threads_response && !queried_stack {
                session
                    .stack_trace(stopped_thread.ok_or("DEBUG_NO_STOPPED_THREAD")?)
                    .map_err(|e| e.code().to_owned())?;
                queried_stack = true;
            }
            if stack_response {
                break;
            }
            thread::sleep(Duration::from_millis(30));
        }
        if !stack_response {
            return Err("DEBUG_INSPECTION_TIMEOUT".into());
        }
        // DAP's source file and line are merely a candidate. Bind them to
        // the *original* sealed project and independently current saved
        // bytes. This is read-only and is not a trusted PDB/DWARF PC map.
        let stopped_source = if target.is_some() {
            session.source_candidate().as_ref()
                .and_then(|source| reviewed_initial_stop_source(
                    registry,project_id,&input,source))
        } else {
            None
        };
        // The session is never exposed to the browser. Close demands Job/
        // cgroup emptiness, independent service release and a terminal receipt.
        // D6D: developer-only Windows native approval after an actual
        // first-party LLDB kernel event, Job/RIP, PDB and immutable Cargo seal.
        // No WebView DAP command, PID, path or approval token is accepted.
        #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
        let mut native_action_receipt: Option<(u64, String, &'static str)> = None;
        #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
        let mut inspected_variables: Option<(u64, Vec<RedactedVariableSummary>)> = None;
        #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
        if let (Some(target), Some(operation)) = (target, operation) {
            if cancel.load(Ordering::SeqCst) {
                return Err("DEV046D_CANCELLED_BEFORE_STOP_APPROVAL".into());
            }
            let original = session
                .verify_real_lldb_event_owner_for_ci(
                    &self.supervisor,
                    &prepared.job_id,
                    &plan,
                    target,
                )
                .map_err(|e| e.code().to_owned())?;
            let mut dialog = |trusted_message: &str| {
                bounded_native_debugger_consent(app, trusted_message, cancel)
            };
            let (owner_seq, receipt, outcome) = match operation {
                ReviewedNativeAction::StepOver => {
                    let (after, audit) = session
                        .execute_one_protected_native_step(
                            &self.supervisor,
                            &prepared.job_id,
                            &plan,
                            target,
                            &original,
                            &mut dialog,
                            cancel,
                        )
                        .map_err(|e| e.code().to_owned())?;
                    if audit.3 != "ResultVerified"
                        || after.owner_sequence != original.owner_sequence + 1
                    {
                        return Err("DEV046D_STEP_MISSING_INDEPENDENT_STOP".into());
                    }
                    (after.owner_sequence, audit.2, "native_dialog_step_verified")
                }
                ReviewedNativeAction::InspectVariables => {
                    let (read, audit) = session
                        .inspect_one_protected_native_variables(
                            &self.supervisor,
                            &prepared.job_id,
                            &plan,
                            target,
                            &original,
                            &mut dialog,
                            cancel,
                        )
                        .map_err(|e| e.code().to_owned())?;
                    if audit.3 != "ResultUncertain"
                        || read.installed_app_accepted
                        || read.frame_id != original.verified_frame_id
                        || read.variables.len() > 16
                        || read.variables.iter().any(|v| !v.value_redacted)
                    {
                        return Err("IDE_G04D_VARIABLES_OWNER_OR_REDACTION_FAILED".into());
                    }
                    inspected_variables = Some((
                        read.frame_id,
                        read.variables
                            .into_iter()
                            .map(|variable| RedactedVariableSummary {
                                name: variable.name,
                                declared_type: variable.declared_type,
                                value_redacted: true,
                            })
                            .collect(),
                    ));
                    (
                        original.owner_sequence,
                        audit.2,
                        "native_dialog_variables_redacted_uncertain",
                    )
                }
                ReviewedNativeAction::SameLineBreakpoint => {
                    let audit = session
                        .attempt_one_protected_native_breakpoint(
                            &self.supervisor,
                            &prepared.job_id,
                            &plan,
                            target,
                            &original,
                            &mut dialog,
                            cancel,
                        )
                        .map_err(|e| e.code().to_owned())?;
                    if audit.3 != "ResultUncertain" {
                        return Err("DEV046D_BP_ACK_NOT_INDEPENDENT_PROOF".into());
                    }
                    (
                        original.owner_sequence,
                        audit.2,
                        "native_dialog_bp_uncertain",
                    )
                }
            };
            native_action_receipt = Some((owner_seq, receipt, outcome));
        }
        #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
        let (operation_outcome_code, operation_receipt_sha256) =
            if let Some((owner_seq, receipt, outcome)) = native_action_receipt {
                session
                    .close_with_real_owner_continue_for_ci(owner_seq)
                    .map_err(|e| e.code().to_owned())?;
                (Some(outcome), Some(receipt))
            } else {
                session.close().map_err(|e| e.code().to_owned())?;
                (None, None)
            };
        #[cfg(not(all(windows, debug_assertions, feature = "dev046d-native-dialog")))]
        let (operation_outcome_code, operation_receipt_sha256) = {
            session.close().map_err(|e| e.code().to_owned())?;
            (None, None)
        };

        #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
        let (inspected_frame_id, redacted_variables) = match inspected_variables {
            Some((frame, variables)) => (Some(frame), Some(variables)),
            None => (None, None),
        };
        #[cfg(not(all(windows, debug_assertions, feature = "dev046d-native-dialog")))]
        let (inspected_frame_id, redacted_variables): (
            Option<u64>,
            Option<Vec<RedactedVariableSummary>>,
        ) = (None, None);

        let state = self
            .supervisor
            .inspect()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .find(|job| job.job_id == prepared.job_id)
            .ok_or("DEBUG_JOURNAL_RECEIPT_MISSING")?;
        if state.phase != JobPhase::Completed || state.requires_reconciliation ||
            state.project_id != project_id || state.plan_sha256 != prepared.plan_sha256 {
            return Err("DEBUG_TERMINAL_OS_CLEANUP_NOT_VERIFIED".into());
        }
        let history = self.supervisor.receipt_history(&prepared.job_id)
            .map_err(|e| e.code().to_owned())?;
        let terminal = history.last().ok_or("IDE_INT04_DEBUG_TERMINAL_RECEIPT_MISSING")?;
        if terminal.body.project_id != project_id ||
            terminal.body.plan_sha256 != prepared.plan_sha256 ||
            terminal.body.phase != JobPhase::Completed {
            return Err("IDE_INT04_DEBUG_TERMINAL_RECEIPT_MISMATCH".into());
        }
        Ok(DebugWitness {
            job_id: prepared.job_id,
            project_id: project_id.to_owned(),
            source_head: head,
            plan_sha256: prepared.plan_sha256,
            bundle_sha256: plan.toolchain_sha256,
            terminal_receipt_sha256: terminal.entry_hash.clone(),
            stopped_source,
            stopped_thread,
            threads_response_observed: threads_response,
            stack_response_observed: stack_response,
            terminal_phase: "completed".into(),
            verified_os_tree_release_by_worker: true,
            installed_app_accepted: false,
            target: if target.is_some() {
                "one_exact_private_project_build_in_protected_job"
            } else {
                "fixed_installer_selftest_only_not_project_debugger"
            },
            origin_build_job_id: target.map(|target| target.lineage.build_job_id.clone()),
            origin_build_plan_sha256: target.map(|target| target.lineage.build_plan_sha256.clone()),
            origin_build_receipt_sha256: target
                .map(|target| target.lineage.terminal_receipt_sha256.clone()),
            binary_sha256: target.map(|target| target.binary.binary_sha256.clone()),
            symbol_sha256: target.map(|target| target.symbols.symbols_sha256.clone()),
            target_binding_sha256: target.map(|target| target.binding_sha256.clone()),
            operation_outcome_code,
            operation_receipt_sha256,
            inspected_frame_id,
            redacted_variables,
        })
    }
}
