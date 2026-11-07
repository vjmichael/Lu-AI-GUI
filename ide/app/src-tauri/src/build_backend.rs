//! WO6 opt-in native build/test dispatcher. No arbitrary commands, environment,
//! cwd, executable paths, script strings or grant tokens cross the WebView.
//! Only the platform's existing D3 restricted Cargo worker may run. Each real
//! run requires an exact reviewed release toolchain and FRESH native OS dialog.
#![cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use lu_ide_workspace::IdeWorkspaceRegistry;
use lu_ide_execution_supervisor::{
    snapshot::{publish_input, verify_input},
    wo6_output_evidence::{audit_published_output, ArtifactEntry},
    JobPlan, JobPhase, Supervisor,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[cfg(windows)]
use lu_ide_execution_supervisor::{
    windows_broker::TrustedBroker,
    windows_worker::{
        execute_windows_worker_after_staging_approval, trusted_offline_toolchain_digest,
        reconcile_windows_interrupted_via_service, WindowsCancellation as NativeCancel,
    },
};
#[cfg(target_os = "linux")]
use lu_ide_execution_supervisor::linux_worker::{
    execute_linux_worker, installed_toolchain_digest, reconcile_linux_interrupted,
    RunCancellation as NativeCancel,
};

const VALID_RECIPES: [&str; 3] = ["rust_check", "rust_tests", "lu_ide_host_tests"];
const MAX_HISTORY: usize = 32;
const MAX_RUNTIME_MS: u64 = 5 * 60_000;

#[cfg(windows)]
const PINNED_CARGO: Option<&str> = option_env!("LU_IDE_WO6_WINDOWS_CARGO_BUNDLE_SHA256");
#[cfg(windows)]
const PINNED_SERVICE: Option<&str> = option_env!("LU_IDE_WO6_WINDOWS_D7D_SERVICE_SHA256");
#[cfg(target_os = "linux")]
const PINNED_CARGO: Option<&str> = option_env!("LU_IDE_WO6_LINUX_TOOLCHAIN_SHA256");

fn approved_pin(value: Option<&str>) -> Result<&str, String> {
    let value = value.ok_or("BUILD_RELEASE_PINS_ABSENT")?;
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("BUILD_RELEASE_PINS_INVALID".into());
    }
    Ok(value)
}
#[cfg(windows)]
fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|_| "BUILD_SERVICE_MISSING")?;
    let mut sha = Sha256::new();
    let mut data = [0u8; 64 * 1024];
    loop {
        let size = file.read(&mut data).map_err(|_| "BUILD_SERVICE_UNREADABLE")?;
        if size == 0 {
            break;
        }
        sha.update(&data[..size]);
    }
    Ok(format!("{:x}", sha.finalize()))
}
struct Installation {
    toolchain_sha256: String,
    #[cfg(windows)]
    toolchain: PathBuf,
    #[cfg(windows)]
    broker: TrustedBroker,
}
fn installation(app: &AppHandle) -> Result<Installation, String> {
    let expected = approved_pin(PINNED_CARGO)?;
    #[cfg(windows)]
    {
        let resource = app.path().resource_dir().map_err(|_| "BUILD_RESOURCE_ROOT_MISSING")?;
        let toolchain = resource.join("build-tools").join("windows-rust-offline");
        let sha = trusted_offline_toolchain_digest(&toolchain)
            .map_err(|_| "BUILD_TOOLCHAIN_UNTRUSTED")?;
        if sha != expected {
            return Err("BUILD_TOOLCHAIN_PIN_MISMATCH".into());
        }
        let service = resource.join("recovery").join("windows_recovery_service.exe");
        let service_sha = approved_pin(PINNED_SERVICE)?;
        if digest_file(&service)? != service_sha {
            return Err("BUILD_SERVICE_PIN_MISMATCH".into());
        }
        let broker = TrustedBroker {
            executable: service,
            expected_sha256: service_sha.to_owned(),
            private_witness_directory: resource.join("recovery"),
        };
        Ok(Installation {
            toolchain_sha256: sha,
            toolchain,
            broker,
        })
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        let sha = installed_toolchain_digest().map_err(|_| "BUILD_TOOLCHAIN_UNTRUSTED")?;
        if sha != expected {
            return Err("BUILD_TOOLCHAIN_PIN_MISMATCH".into());
        }
        Ok(Installation {
            toolchain_sha256: sha,
        })
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildRunView {
    pub project_id: String,
    pub job_id: String,
    pub recipe_id: String,
    pub plan_sha256: String,
    pub source_head: String,
    pub input_snapshot_sha256: String,
    pub manifest_sha256: String,
    pub toolchain_sha256: String,
    pub phase: String,
    pub native_exit_code: Option<i64>,
    pub native_error_category: Option<String>,
    pub output_sha256: Option<String>,
    pub output_bytes: Option<u64>,
    pub artifact_manifest_sha256: Option<String>,
    pub artifact_evidence_state: String,
    pub artifact_files: Vec<ArtifactEntry>,
    pub artifact_count: usize,
    pub artifact_truncated: bool,
    pub timed_out: bool,
    pub requires_reconciliation: bool,
    pub installed_app_accepted: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveView {
    project_id: String,
    job_id: Option<String>,
    recipe_id: String,
    cancellation_requested: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatus {
    ready: bool,
    code: String,
    description: &'static str,
    current: Option<ActiveView>,
    recent: Vec<BuildRunView>,
    // Persisted supervisor journal; unlike recent, survives a host restart.
    journal: Vec<RecoveryView>,
    quarantined: Vec<RecoveryView>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryView {
    project_id: String,
    job_id: String,
    plan_sha256: String,
    phase: String,
    requires_reconciliation: bool,
}
struct Running {
    project_id: String,
    recipe_id: String,
    job_id: Option<String>,
    cancel: Arc<NativeCancel>,
    requested: Arc<AtomicBool>,
}
struct Store {
    current: Option<Running>,
    recent: BTreeMap<String, BuildRunView>,
}
pub struct NativeBuildRuntime {
    supervisor: Arc<Supervisor>,
    staged: PathBuf,
    outputs: PathBuf,
    store: Mutex<Store>,
}
impl NativeBuildRuntime {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let data = app.path().app_data_dir().map_err(|_| "BUILD_PROFILE_MISSING")?;
        fs::create_dir_all(&data).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        let staged = data.join("build-snapshots");
        let outputs = data.join("build-outputs");
        fs::create_dir_all(&staged).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        fs::create_dir_all(&outputs).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        let supervisor = Supervisor::open(data.join("build-job-receipts.jsonl"))
            .map_err(|e| e.code().to_owned())?;
        Ok(Self {
            supervisor: Arc::new(supervisor),
            staged,
            outputs,
            store: Mutex::new(Store {
                current: None,
                recent: BTreeMap::new(),
            }),
        })
    }
    pub fn status(&self, app: &AppHandle, project_id: &str) -> Result<BuildStatus, String> {
        let availability = installation(app).map(|_| ());
        let locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        let current = locked.current.as_ref().filter(|job| job.project_id == project_id)
            .map(|job| ActiveView {
                project_id: job.project_id.clone(),
                job_id: job.job_id.clone(),
                recipe_id: job.recipe_id.clone(),
                cancellation_requested: job.requested.load(Ordering::SeqCst),
            });
        let recent = locked.recent.values()
            .filter(|job| job.project_id == project_id).cloned().collect();
        drop(locked);
        let durable = self.supervisor.inspect().map_err(|e| e.code().to_owned())?;
        // Serialize only exact project-scoped immutable journal identifiers.
        // Do not reconstruct a BuildRunView or invent a successful native exit
        // code after restart: the original runtime receipt is authoritative.
        let journal: Vec<RecoveryView> = durable.into_iter()
            .filter(|job| job.project_id == project_id)
            .take(32)
            .map(|job| RecoveryView {
                project_id: job.project_id,
                job_id: job.job_id,
                plan_sha256: job.plan_sha256,
                phase: format!("{:?}", job.phase).to_lowercase(),
                requires_reconciliation: job.requires_reconciliation,
            })
            .collect();
        let quarantined: Vec<RecoveryView> = journal.iter()
            .filter(|job| job.requires_reconciliation)
            .map(|job| RecoveryView {
                project_id: job.project_id.clone(),
                job_id: job.job_id.clone(),
                plan_sha256: job.plan_sha256.clone(),
                phase: job.phase.clone(),
                requires_reconciliation: true,
            })
            .collect();
        Ok(BuildStatus {
            ready: availability.is_ok(),
            code: match availability {
                Ok(()) => "BUILD_RELEASE_RESOURCES_VERIFIED_ADMISSION_PER_JOB".into(),
                Err(code) => code,
            },
            description: "Pinned resources are not execution proof. Native Job/cgroup \
                admission, human approval and terminal OS cleanup are checked for each job.",
            current,
            recent,
            journal,
            quarantined,
        })
    }
    pub fn cancel(&self, project_id: &str, job_id: Option<&str>) -> Result<&'static str, String> {
        let locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        let active = locked.current.as_ref().ok_or("BUILD_NO_ACTIVE_JOB")?;
        if active.project_id != project_id
            || job_id.is_some_and(|id| active.job_id.as_deref() != Some(id))
        {
            return Err("BUILD_JOB_SCOPE_MISMATCH".into());
        }
        active.requested.store(true, Ordering::SeqCst);
        active.cancel.request_cancel();
        // Request is intentionally NOT mislabeled Cancelled. The worker must
        // first fsync the cancellation and prove native process-tree kill.
        Ok("BUILD_CANCELLATION_REQUESTED")
    }
    pub fn run(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<BuildRunView, String> {
        if !VALID_RECIPES.contains(&recipe_id) {
            return Err("BUILD_RECIPE_DENIED".into());
        }
        let cancel = Arc::new(NativeCancel::default());
        let requested = Arc::new(AtomicBool::new(false));
        {
            let mut locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
            if locked.current.is_some() {
                return Err("BUILD_ANOTHER_JOB_ACTIVE".into());
            }
            locked.current = Some(Running {
                project_id: project_id.to_owned(),
                recipe_id: recipe_id.to_owned(),
                job_id: None,
                cancel: Arc::clone(&cancel),
                requested: Arc::clone(&requested),
            });
        }
        let result = self.run_inner(
            app, registry, project_id, recipe_id, &cancel, &requested,
        );
        let mut locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        locked.current = None;
        if let Ok(ref view) = result {
            locked.recent.insert(view.job_id.clone(), view.clone());
            while locked.recent.len() > MAX_HISTORY {
                if let Some(old) = locked.recent.keys().next().cloned() {
                    locked.recent.remove(&old);
                }
            }
        }
        result
    }
    fn run_inner(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        recipe_id: &str,
        cancel: &Arc<NativeCancel>,
        requested: &Arc<AtomicBool>,
    ) -> Result<BuildRunView, String> {
        let installed = installation(app)?;
        let source_root = registry.trusted_native_worker_root(project_id)
            .map_err(|_| "BUILD_PROJECT_NOT_ACTIVE")?;
        let inspection = registry.inspect_git_worktree(project_id);
        if inspection.state != "local_repository_inspected"
            || inspection.worktree != "clean"
            || inspection.truncated
        {
            return Err("BUILD_REQUIRES_EXACT_CLEAN_GIT_HEAD".into());
        }
        let head = inspection.local_head.ok_or("BUILD_GIT_HEAD_UNAVAILABLE")?;
        if head.len() != 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("BUILD_GIT_HEAD_INVALID".into());
        }
        if requested.load(Ordering::SeqCst) {
            return Err("BUILD_CANCELLED_BEFORE_AUTHORIZATION".into());
        }
        let input = publish_input(&source_root, &self.staged, recipe_id)
            .map_err(|e| e.code().to_owned())?;
        verify_input(&input).map_err(|e| e.code().to_owned())?;
        let plan = JobPlan {
            project_id: project_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
            source_head: head.clone(),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: installed.toolchain_sha256.clone(),
            runtime_limit_ms: MAX_RUNTIME_MS,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 8,
            log_limit_bytes: 128 * 1024,
            effect_class: "test_build_isolated".into(),
        };
        let prepared = self.supervisor.prepare(plan.clone())
            .map_err(|e| e.code().to_owned())?;
        {
            let mut state = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
            let active = state.current.as_mut().ok_or("BUILD_HOST_UNAVAILABLE")?;
            if active.project_id != project_id {
                return Err("BUILD_JOB_SCOPE_MISMATCH".into());
            }
            active.job_id = Some(prepared.job_id.clone());
        }
        if requested.load(Ordering::SeqCst) {
            return Err("BUILD_CANCELLED_BEFORE_AUTHORIZATION".into());
        }
        let approve = || -> Result<(), lu_ide_execution_supervisor::SupervisorError> {
            use lu_ide_execution_supervisor::SupervisorError;
            if requested.load(Ordering::SeqCst) {
                return Err(SupervisorError::InvalidApproval);
            }
            let choice = app.dialog()
                .message(format!(
                    "Run a restricted, offline Rust job?\n\n\
                    Project: {project_id}\nRecipe: {recipe_id}\nSource HEAD: {head}\n\
                    Frozen source SHA-256: {}\nToolchain SHA-256: {}\n\n\
                    The OS worker will execute code from the sealed copy. No project files \
                    are writable from that worker. Cancellation requests require proven \
                    kernel process-tree termination.",
                    plan.input_snapshot_sha256, plan.toolchain_sha256,
                ))
                .title("Allow one protected build/test job")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Run this exact job".into(), "Cancel".into(),
                ))
                .blocking_show();
            if !choice || requested.load(Ordering::SeqCst) {
                return Err(SupervisorError::InvalidApproval);
            }
            let current_root = registry.trusted_native_worker_root(project_id)
                .map_err(|_| SupervisorError::WrongProjectOrSource)?;
            let fresh = registry.inspect_git_worktree(project_id);
            if current_root != source_root || fresh.local_head.as_deref() != Some(head.as_str())
                || fresh.worktree != "clean" || fresh.state != "local_repository_inspected"
                || fresh.truncated
            {
                return Err(SupervisorError::WrongProjectOrSource);
            }
            self.supervisor.record_human_approval(
                &prepared.job_id, &prepared.plan_sha256, &head, true,
            )?;
            Ok(())
        };
        #[cfg(windows)]
        let native = {
            execute_windows_worker_after_staging_approval(
                &self.supervisor, &prepared.job_id, &plan, &head, &input,
                &installed.toolchain, &self.outputs, &installed.broker, cancel, approve,
            ).map_err(|e| e.code().to_owned())?
        };
        #[cfg(target_os = "linux")]
        let native = {
            approve().map_err(|e| e.code().to_owned())?;
            execute_linux_worker(
                &self.supervisor, &prepared.job_id, &plan, &input,
                &self.outputs, cancel,
            ).map_err(|e| e.code().to_owned())?
        };
        let evidence = self.supervisor.review_build_test_evidence(
            &prepared.job_id, project_id, &prepared.plan_sha256, &head,
        ).map_err(|e| e.code().to_owned())?;
        #[cfg(windows)]
        let native_error_category: Option<String> = None;
        #[cfg(target_os = "linux")]
        let native_error_category = Some(native.stderr_category.to_owned());
        #[cfg(windows)]
        let (exit_code, output_sha256, output_bytes) = (
            native.exit_code.map(i64::from),
            if native.output_sha256.is_empty() { None } else { Some(native.output_sha256) },
            Some(native.output_bytes),
        );
        #[cfg(target_os = "linux")]
        let (exit_code, output_sha256, output_bytes) = (
            native.exit_code.map(i64::from),
            // Linux captures independent stdout/stderr hashes; a combined
            // artifact digest requires a separately reviewed output audit.
            None, None,
        );
        // Only attempt a read-only publication witness after the exact
        // native supervisor has recorded Completed. Failed/quarantined or
        // cancelled jobs never expose executable artifact paths to WebView.
        #[cfg(windows)]
        let published = if native.summary.phase == JobPhase::Completed
            && !native.output_root.as_os_str().is_empty()
        {
            audit_published_output(&native.output_root).ok()
        } else {
            None
        };
        #[cfg(target_os = "linux")]
        let published = if native.summary.phase == JobPhase::Completed {
            audit_published_output(&native.output_directory).ok()
        } else {
            None
        };
        let artifact_evidence_state = if published.is_some() {
            "read_only_post_termination_tree_hashed"
        } else {
            "no_trusted_artifact_manifest"
        };
        let (artifact_manifest_sha256, artifact_files, artifact_count, artifact_truncated) =
            match published {
                Some(manifest) => (
                    Some(manifest.tree_sha256),
                    manifest.files,
                    manifest.file_count,
                    manifest.truncated,
                ),
                None => (None, Vec::new(), 0, false),
            };
        Ok(BuildRunView {
            project_id: project_id.to_owned(),
            job_id: prepared.job_id,
            recipe_id: recipe_id.to_owned(),
            plan_sha256: prepared.plan_sha256,
            source_head: head,
            input_snapshot_sha256: plan.input_snapshot_sha256,
            manifest_sha256: plan.manifest_sha256,
            toolchain_sha256: plan.toolchain_sha256,
            phase: format!("{:?}", native.summary.phase).to_lowercase(),
            native_exit_code: exit_code,
            native_error_category,
            output_sha256,
            output_bytes,
            artifact_manifest_sha256,
            artifact_evidence_state: artifact_evidence_state.to_owned(),
            artifact_files,
            artifact_count,
            artifact_truncated,
            timed_out: native.timed_out,
            requires_reconciliation: evidence.requires_reconciliation,
            // A native worker reply is NOT independently installed-app
            // acceptance, especially on a WO4/WO5 stacked unaccepted branch.
            installed_app_accepted: false,
        })
    }
    /// Never automatically resumes a crashed job. The user must explicitly
    /// request kernel cleanup with an authenticated native service/cgroup.
    pub fn reconcile(
        &self,
        app: &AppHandle,
        project_id: &str,
        job_id: &str,
    ) -> Result<String, String> {
        if self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?.current.is_some() {
            return Err("BUILD_JOB_ACTIVE".into());
        }
        let entry = self.supervisor.inspect().map_err(|e| e.code().to_owned())?
            .into_iter().find(|job| job.job_id == job_id)
            .ok_or("BUILD_JOB_NOT_FOUND")?;
        if entry.project_id != project_id || !entry.requires_reconciliation {
            return Err("BUILD_RECOVERY_SCOPE_DENIED".into());
        }
        let install = installation(app)?;
        let choice = app.dialog()
            .message(format!(
                "Attempt OS-only cleanup of interrupted job {job_id}?\n\n\
                This will not rebuild or restart the project. It only attempts \
                to kill the exact kernel-controlled process tree and journal \
                a verified reconciliation receipt."
            ))
            .title("Reconcile interrupted protected job")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Verify and clean up".into(), "Leave quarantined".into(),
            )).blocking_show();
        if !choice {
            return Err("BUILD_RECOVERY_NOT_APPROVED".into());
        }
        #[cfg(windows)]
        let summary = reconcile_windows_interrupted_via_service(
            &self.supervisor, job_id, &install.broker,
        ).map_err(|e| e.code().to_owned())?;
        #[cfg(target_os = "linux")]
        let summary = {
            let _ = install;
            reconcile_linux_interrupted(&self.supervisor, job_id)
                .map_err(|e| e.code().to_owned())?
        };
        if summary.phase != JobPhase::Cancelled || summary.requires_reconciliation {
            return Err("BUILD_RECOVERY_REMAINS_QUARANTINED".into());
        }
        Ok("BUILD_KERNEL_TREE_RECONCILED_NO_REEXECUTION".into())
    }
}
