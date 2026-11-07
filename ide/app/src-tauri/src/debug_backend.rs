//! WO6 feature-gated, OS-restricted LLDB-DAP diagnostic coordinator.
//!
//! It can run only the fixed, independently pinned installer-packaged debuggee
//! inside the adapter's AppContainer Job or delegated cgroup/Bubblewrap tree.
//! No project executable, PID, path, arguments, expression, conditional
//! breakpoint, adapter reverse request, or terminal command reaches this API.
#![cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use lu_ide_workspace::{
    dap_session::{DapEvent, DapPhase},
    IdeWorkspaceRegistry,
};
use lu_ide_execution_supervisor::{
    snapshot::publish_input, JobPhase, JobPlan, Supervisor,
};
#[cfg(windows)]
use lu_ide_execution_supervisor::{
    windows_broker::TrustedBroker,
    windows_worker::lsp_native::{
        native_lsp_bundle_digest, start_windows_dap, WindowsDapSession as NativeDapSession,
    },
};
#[cfg(target_os = "linux")]
use lu_ide_execution_supervisor::linux_worker::lsp_native::{
    native_lsp_bundle_digest, start_linux_dap, LinuxDapSession as NativeDapSession,
};

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
    use std::{fs::File, io::Read};
    use sha2::{Digest, Sha256};
    let mut reader = File::open(path).map_err(|_| "DEBUG_SERVICE_MISSING")?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|_| "DEBUG_SERVICE_UNREADABLE")?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn installation(app: &AppHandle) -> Result<Installation, String> {
    let approved = pin(BUNDLE_PIN)?;
    #[cfg(windows)]
    {
        let root = app.path().resource_dir().map_err(|_| "DEBUG_RESOURCE_MISSING")?;
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
            bundle, digest: observed,
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
        Ok(Installation { bundle, digest: observed })
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugWitness {
    pub job_id: String,
    pub project_id: String,
    pub source_head: String,
    pub plan_sha256: String,
    pub bundle_sha256: String,
    pub stopped_thread: Option<u64>,
    pub threads_response_observed: bool,
    pub stack_response_observed: bool,
    pub terminal_phase: String,
    pub verified_os_tree_release_by_worker: bool,
    pub installed_app_accepted: bool,
    pub target: &'static str,
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
    code: String,
    current: Option<DebugActive>,
    last: Option<DebugWitness>,
    description: &'static str,
}
struct Active {
    project_id: String,
    job_id: Option<String>,
    cancellation: Arc<AtomicBool>,
}
pub struct NativeDebugRuntime {
    supervisor: Arc<Supervisor>,
    stage: PathBuf,
    active: Mutex<Option<Active>>,
    last: Mutex<Option<DebugWitness>>,
}
impl NativeDebugRuntime {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let data = app.path().app_data_dir().map_err(|_| "DEBUG_PROFILE_MISSING")?;
        fs::create_dir_all(&data).map_err(|_| "DEBUG_PROFILE_UNAVAILABLE")?;
        let stage = data.join("debug-fixture-snapshots");
        fs::create_dir_all(&stage).map_err(|_| "DEBUG_PROFILE_UNAVAILABLE")?;
        let supervisor = Supervisor::open(data.join("debug-fixture-receipts.jsonl"))
            .map_err(|e| e.code().to_owned())?;
        Ok(Self {
            supervisor: Arc::new(supervisor),
            stage,
            active: Mutex::new(None),
            last: Mutex::new(None),
        })
    }
    pub fn status(&self, app: &AppHandle, project_id: &str) -> Result<DebugStatus, String> {
        let available = installation(app).map(|_| ());
        let current = self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?
            .as_ref().filter(|v| v.project_id == project_id)
            .map(|v| DebugActive {
                project_id: v.project_id.clone(),
                job_id: v.job_id.clone(),
                cancellation_requested: v.cancellation.load(Ordering::SeqCst),
            });
        let last = self.last.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?
            .as_ref().filter(|v| v.project_id == project_id).cloned();
        Ok(DebugStatus {
            ready: available.is_ok(),
            code: match available {
                Ok(()) => "DEBUG_FIXTURE_BUNDLE_VERIFIED_OS_ADMISSION_PER_RUN".into(),
                Err(code) => code,
            },
            current,
            last,
            description: "Only fixed LLDB-DAP self-test debuggee. Project debug launches remain disabled.",
        })
    }
    pub fn cancel(&self, project_id: &str, job_id: Option<&str>) -> Result<&'static str, String> {
        let locked = self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?;
        let active = locked.as_ref().ok_or("DEBUG_NO_ACTIVE_SESSION")?;
        if active.project_id != project_id
            || job_id.is_some_and(|id| active.job_id.as_deref() != Some(id))
        {
            return Err("DEBUG_SESSION_SCOPE_MISMATCH".into());
        }
        active.cancellation.store(true, Ordering::SeqCst);
        Ok("DEBUG_CANCEL_REQUESTED_OS_TERMINATION_PENDING")
    }
    pub fn run_fixture(
        &self, app: &AppHandle, registry: &IdeWorkspaceRegistry, project_id: &str,
    ) -> Result<DebugWitness, String> {
        let cancellation = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")?;
            if active.is_some() {
                return Err("DEBUG_SESSION_ALREADY_ACTIVE".into());
            }
            *active = Some(Active {
                project_id: project_id.to_owned(),
                job_id: None,
                cancellation: Arc::clone(&cancellation),
            });
        }
        let result = self.run_inner(app, registry, project_id, &cancellation);
        *self.active.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")? = None;
        if let Ok(ref witness) = result {
            *self.last.lock().map_err(|_| "DEBUG_HOST_UNAVAILABLE")? = Some(witness.clone());
        }
        result
    }
    fn run_inner(
        &self, app: &AppHandle, registry: &IdeWorkspaceRegistry,
        project_id: &str, cancel: &AtomicBool,
    ) -> Result<DebugWitness, String> {
        let install = installation(app)?;
        let root = registry.trusted_native_worker_root(project_id)
            .map_err(|_| "DEBUG_PROJECT_NOT_ACTIVE")?;
        let git = registry.inspect_git_worktree(project_id);
        if git.state != "local_repository_inspected" || git.worktree != "clean" || git.truncated {
            return Err("DEBUG_REQUIRES_CLEAN_PROJECT".into());
        }
        let head = git.local_head.ok_or("DEBUG_SOURCE_HEAD_UNAVAILABLE")?;
        if head.len() != 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("DEBUG_SOURCE_HEAD_INVALID".into());
        }
        let input = publish_input(&root, &self.stage, "rust_check")
            .map_err(|e| e.code().to_owned())?;
        if cancel.load(Ordering::SeqCst) {
            return Err("DEBUG_CANCELLED_BEFORE_NATIVE_AUTHORIZATION".into());
        }
        let plan = JobPlan {
            project_id: project_id.to_owned(), recipe_id: "dap_lldb_fixed".into(),
            source_head: head.clone(),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: install.digest.clone(),
            runtime_limit_ms: 3 * 60_000, memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 4, log_limit_bytes: 64 * 1024,
            effect_class: "debug_inspection_isolated".into(),
        };
        let prepared = self.supervisor.prepare(plan.clone()).map_err(|e| e.code().to_owned())?;
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
        let decision = app.dialog()
            .message(format!(
                "Run the reviewed, fixed LLDB-DAP debugger fixture INSIDE the protected OS worker?\n\n\
                Project ID: {project_id}\nSource HEAD: {head}\n\
                Frozen input SHA-256: {}\nAdapter+debuggee SHA-256: {}\n\n\
                Only the installer-packaged self-test binary can be launched. \
                No project executable, evaluated expression or user-supplied terminal command is permitted.",
                plan.input_snapshot_sha256, install.digest,
            ))
            .title("Approve one protected native debugger self-test")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Run fixed debugger self-test".into(), "Cancel".into(),
            )).blocking_show();
        if !decision || cancel.load(Ordering::SeqCst) {
            return Err("DEBUG_HUMAN_APPROVAL_DENIED".into());
        }
        let fresh = registry.inspect_git_worktree(project_id);
        if registry.trusted_native_worker_root(project_id).ok().as_ref() != Some(&root)
            || fresh.state != "local_repository_inspected"
            || fresh.worktree != "clean"
            || fresh.local_head.as_deref() != Some(head.as_str())
            || fresh.truncated
        {
            return Err("DEBUG_SOURCE_CHANGED_DURING_APPROVAL".into());
        }
        self.supervisor.record_human_approval(
            &prepared.job_id, &prepared.plan_sha256, &head, true
        ).map_err(|e| e.code().to_owned())?;
        #[cfg(windows)]
        let mut session: NativeDapSession = start_windows_dap(
            Arc::clone(&self.supervisor), &prepared.job_id, &plan, &head,
            &input, &install.bundle, &install.broker,
        ).map_err(|e| e.code().to_owned())?;
        #[cfg(target_os = "linux")]
        let mut session: NativeDapSession = start_linux_dap(
            Arc::clone(&self.supervisor), &prepared.job_id, &plan, &head,
            &input, &install.bundle,
        ).map_err(|e| e.code().to_owned())?;
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
        session.launch_fixed_fixture().map_err(|e| e.code().to_owned())?;
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
                    DapEvent::Stopped { thread_id } => stopped_thread = Some(thread_id),
                    DapEvent::ThreadsReceived => threads_response = true,
                    DapEvent::StackTraceReceived => stack_response = true,
                    DapEvent::Terminated => return Err("DEBUG_FIXED_DEBUGGEE_EXITED_BEFORE_INSPECTION".into()),
                    _ => {}
                }
            }
            if stopped_thread.is_some() && !queried_thread
                && session.phase() == DapPhase::Stopped
            {
                session.threads().map_err(|e| e.code().to_owned())?;
                queried_thread = true;
            }
            if threads_response && !queried_stack {
                session.stack_trace(stopped_thread.ok_or("DEBUG_NO_STOPPED_THREAD")?)
                    .map_err(|e| e.code().to_owned())?;
                queried_stack = true;
            }
            if stack_response { break; }
            thread::sleep(Duration::from_millis(30));
        }
        if !stack_response {
            return Err("DEBUG_INSPECTION_TIMEOUT".into());
        }
        // The session is never exposed to the browser. Close demands Job/
        // cgroup emptiness, independent service release and a terminal receipt.
        session.close().map_err(|e| e.code().to_owned())?;
        let state = self.supervisor.inspect().map_err(|e| e.code().to_owned())?
            .into_iter().find(|job| job.job_id == prepared.job_id)
            .ok_or("DEBUG_JOURNAL_RECEIPT_MISSING")?;
        if state.phase != JobPhase::Completed || state.requires_reconciliation {
            return Err("DEBUG_TERMINAL_OS_CLEANUP_NOT_VERIFIED".into());
        }
        Ok(DebugWitness {
            job_id: prepared.job_id, project_id: project_id.to_owned(),
            source_head: head, plan_sha256: prepared.plan_sha256,
            bundle_sha256: plan.toolchain_sha256, stopped_thread,
            threads_response_observed: threads_response,
            stack_response_observed: stack_response,
            terminal_phase: "completed".into(),
            verified_os_tree_release_by_worker: true,
            installed_app_accepted: false,
            target: "fixed_installer_selftest_only_not_project_debugger",
        })
    }
}
