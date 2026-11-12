//! WO6 opt-in native build/test dispatcher. No arbitrary commands, environment,
//! cwd, executable paths, script strings or grant tokens cross the WebView.
//! Only the platform's existing D3 restricted Cargo worker may run. Each real
//! run requires an exact reviewed release toolchain and FRESH native OS dialog.
#![cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
use lu_ide_execution_supervisor::{
    debug_binary_witness::{audit_debug_binary_candidate, DebugBinaryWitness},
    debug_symbol_verification::{verify_build_debug_symbols, DebugSymbolWitness},
    live_output::{LiveOutput, OutputPage},
    project_debug_target::{
        seal_project_debug_target, CompletedBuildLineage, SealedProjectDebugTarget,
    },
    snapshot::{publish_input, verify_input},
    wo6_output_evidence::{audit_published_output, ArtifactEntry},
    JobPhase, JobPlan, Supervisor,
};
#[cfg(feature = "native-package-uv")]
use lu_ide_execution_supervisor::{
    package_lifecycle::{PackageLifecycleLedger, PackageLifecycleStatus},
    package_output_evidence::{
        audit_uv_private_output, verify_offline_registry_lock, PackageOutputAudit,
    },
};
use lu_ide_workspace::IdeWorkspaceRegistry;
use crate::test_case_evidence::{reported_cases, TestCaseReport};
use serde::Serialize;
use sha2::{Digest, Sha256};
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
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[cfg(target_os = "linux")]
use lu_ide_execution_supervisor::linux_worker::{
    execute_linux_worker_with_output, installed_toolchain_digest, reconcile_linux_interrupted,
    RunCancellation as NativeCancel,
};
#[cfg(windows)]
use lu_ide_execution_supervisor::{
    windows_broker::TrustedBroker,
    windows_worker::{
        execute_windows_worker_after_staging_approval_with_output,
        reconcile_windows_interrupted_via_service, trusted_offline_toolchain_digest,
        WindowsCancellation as NativeCancel,
    },
};

const VALID_RECIPES: [&str; 4] = [
    "rust_check",
    "rust_build",
    "rust_tests",
    "lu_ide_host_tests",
];
const MULTILANG_RECIPES: [&str; 4] = [
    "python_pytest",
    "js_tests",
    "cpp_cmake_build",
    "uv_sync_offline",
];
/// INT03: An explicit reviewed profile binds exactly one saved manifest,
/// never a caller-submitted path, script, shell argument or toolchain.
fn profile_manifest(recipe: &str) -> Option<&'static str> {
    match recipe {
        "rust_check" | "rust_build" | "rust_tests" => Some("Cargo.toml"),
        id if lu_ide_execution_supervisor::rust_exact_case_name(id).is_some()=>
            Some("Cargo.toml"),
        "lu_ide_host_tests" => Some("ide/host/Cargo.toml"),
        "python_pytest" => Some("pyproject.toml"),
        "js_tests" => Some("package.json"),
        "cpp_cmake_build" => Some("CMakeLists.txt"),
        _ => None,
    }
}

/// Audit the exact frozen copy, not just live-source SHA evidence.
/// The worker may not receive a manifest or lock swapped between the
/// preview and publish_input. Snapshot bounds each file at 64 MiB.
#[cfg(feature = "native-package-uv")]
fn sha256_private_staged_file(root: &Path, name: &'static str) -> Result<String, String> {
    let file = root.join(name);
    let before = fs::symlink_metadata(&file).map_err(|_| "IDE053_STAGED_PACKAGE_FILE_MISSING")?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > 64 * 1024 * 1024 {
        return Err("IDE053_STAGED_PACKAGE_FILE_UNTRUSTED".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if before.file_attributes() & 0x400 != 0 {
            return Err("IDE053_STAGED_PACKAGE_FILE_UNTRUSTED".into());
        }
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000); // Linux O_NOFOLLOW.
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT.
    }
    let mut input = options
        .open(&file)
        .map_err(|_| "IDE053_STAGED_PACKAGE_FILE_UNAVAILABLE")?;
    let opened = input
        .metadata()
        .map_err(|_| "IDE053_STAGED_PACKAGE_FILE_UNAVAILABLE")?;
    if !opened.is_file() || opened.len() != before.len() {
        return Err("IDE053_STAGED_PACKAGE_CHANGED_BEFORE_READ".into());
    }
    let mut sha = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    let mut count = 0u64;
    loop {
        let read = input
            .read(&mut chunk)
            .map_err(|_| "IDE053_STAGED_PACKAGE_READ_FAILED")?;
        if read == 0 {
            break;
        }
        count = count.saturating_add(read as u64);
        if count > before.len() || count > 64 * 1024 * 1024 {
            return Err("IDE053_STAGED_PACKAGE_CHANGED_DURING_READ".into());
        }
        sha.update(&chunk[..read]);
    }
    let after =
        fs::symlink_metadata(&file).map_err(|_| "IDE053_STAGED_PACKAGE_RECHECK_UNAVAILABLE")?;
    if count != before.len()
        || after.len() != before.len()
        || !after.is_file()
        || after.file_type().is_symlink()
        || before.modified().ok() != after.modified().ok()
    {
        return Err("IDE053_STAGED_PACKAGE_CHANGED_DURING_READ".into());
    }
    Ok(format!("{:x}", sha.finalize()))
}
#[derive(Clone)]
struct ReviewedPackageSync {
    request: lu_ide_workspace::package_preview::PackagePreviewRequest,
    preview_sha256: String,
    evidence_fingerprint: String,
}

#[cfg(feature = "native-package-uv")]
fn native_package_consent(app: &AppHandle, message: String, requested: &AtomicBool) -> bool {
    use std::{
        sync::mpsc::{sync_channel, RecvTimeoutError},
        time::{Duration, Instant},
    };
    let (send, receive) = sync_channel(1);
    if requested.load(Ordering::SeqCst) {
        return false;
    }
    app.dialog()
        .message(message)
        .title("Lu IDE · approve one protected offline uv sync")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Approve this one sealed uv sync".into(),
            "Decline".into(),
        ))
        .show(move |confirmed| {
            let _ = send.try_send(confirmed);
        });
    // The OS dialog is not an authority while the host is cancelled or timed
    // out. A late click only talks to a disconnected private channel.
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if requested.load(Ordering::SeqCst) {
            return false;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match receive.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(confirmed) => return confirmed && !requested.load(Ordering::SeqCst),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

#[cfg(feature = "native-multilang")]
fn multilang_release_pin(recipe_id: &str) -> Option<&'static str> {
    #[cfg(windows)]
    {
        match recipe_id {
            "python_pytest" => option_env!("LU_IDE_DEV03_WINDOWS_PYTHON_SHA256"),
            "js_tests" => option_env!("LU_IDE_DEV03_WINDOWS_NODE_SHA256"),
            "cpp_cmake_build" => option_env!("LU_IDE_DEV03_WINDOWS_CMAKE_SHA256"),
            "uv_sync_offline" if cfg!(feature = "native-package-uv") => {
                option_env!("LU_IDE05_WINDOWS_UV_SHA256")
            }
            _ => None,
        }
    }
    #[cfg(target_os = "linux")]
    {
        match recipe_id {
            "python_pytest" => option_env!("LU_IDE_DEV03_LINUX_PYTHON_SHA256"),
            "js_tests" => option_env!("LU_IDE_DEV03_LINUX_NODE_SHA256"),
            "cpp_cmake_build" => option_env!("LU_IDE_DEV03_LINUX_CMAKE_SHA256"),
            "uv_sync_offline" if cfg!(feature = "native-package-uv") => {
                option_env!("LU_IDE05_LINUX_UV_SHA256")
            }
            _ => None,
        }
    }
}

// The supervisor serializes JobPhase with snake_case. Debug-lowercasing
// would turn CancelRequested into "cancelrequested", silently breaking
// recovery/progress guards and case-sensitive UI admission.
fn native_phase_label(phase: JobPhase) -> &'static str {
    match phase {
        JobPhase::Prepared => "prepared",
        JobPhase::Authorized => "authorized",
        JobPhase::AdmissionPending => "admission_pending",
        JobPhase::Running => "running",
        JobPhase::ReleasePending => "release_pending",
        JobPhase::CancelRequested => "cancel_requested",
        JobPhase::Cancelled => "cancelled",
        JobPhase::Completed => "completed",
        JobPhase::Failed => "failed",
        JobPhase::Denied => "denied",
        JobPhase::RecoveryRequired => "recovery_required",
        JobPhase::Uncertain => "uncertain",
    }
}

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
        let size = file
            .read(&mut data)
            .map_err(|_| "BUILD_SERVICE_UNREADABLE")?;
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
        let resource = app
            .path()
            .resource_dir()
            .map_err(|_| "BUILD_RESOURCE_ROOT_MISSING")?;
        let toolchain = resource.join("build-tools").join("windows-rust-offline");
        let sha = trusted_offline_toolchain_digest(&toolchain)
            .map_err(|_| "BUILD_TOOLCHAIN_UNTRUSTED")?;
        if sha != expected {
            return Err("BUILD_TOOLCHAIN_PIN_MISMATCH".into());
        }
        let service = resource
            .join("recovery")
            .join("windows_recovery_service.exe");
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


/// INT05-E: provenance of the *actually rehashed installed bytes*. Neither
/// a compile-time digest alone nor project metadata is installation proof.
/// Publisher, package version and license closure are intentionally unknown.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvenance {
    profile_id: &'static str,
    observed_sha256: Option<String>,
    release_pin_sha256: Option<String>,
    provenance_state: &'static str,
    version_evidence: &'static str,
    publisher_evidence: &'static str,
    license_evidence: &'static str,
    execution_authorized: bool,
    installed_app_accepted: bool,
}
fn runtime_provenance(
    profile_id: &'static str,
    observed: Option<String>,
    declared_pin: Option<&str>,
) -> RuntimeProvenance {
    let pin = approved_pin(declared_pin).ok().map(str::to_owned);
    let matched = observed.as_ref().zip(pin.as_ref())
        .is_some_and(|(bytes, expected)| bytes == expected &&
            bytes.len() == 64 && bytes.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    RuntimeProvenance {
        profile_id,
        observed_sha256: if matched { observed } else { None },
        release_pin_sha256: pin,
        provenance_state: if matched {
            "installed_bytes_match_compile_time_release_pin"
        } else {
            "unavailable_or_not_independently_verified"
        },
        version_evidence: "not_independently_verified",
        publisher_evidence: "not_independently_verified",
        license_evidence: "not_independently_verified",
        execution_authorized: false,
        installed_app_accepted: false,
    }
}

/// Native runtime readiness only; this is NEVER execution permission.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultilangProfileReadiness {
    profile_id: &'static str,
    ready: bool,
    code: &'static str,
    provenance: RuntimeProvenance,
}

#[cfg(feature = "native-multilang")]
fn installation_multilang(app: &AppHandle, recipe_id: &str) -> Result<Installation, String> {
    if !MULTILANG_RECIPES.contains(&recipe_id) {
        return Err("DEV03_RECIPE_DENIED".into());
    }
    let expected = approved_pin(multilang_release_pin(recipe_id))?;
    #[cfg(windows)]
    {
        let resource = app
            .path()
            .resource_dir()
            .map_err(|_| "DEV03_RESOURCE_ROOT_MISSING")?;
        let toolchain = resource
            .join("build-tools")
            .join("windows-runtimes")
            .join(recipe_id);
        let attested = lu_ide_execution_supervisor::runtime_bundles::verify_runtime_bundle(
            recipe_id, &toolchain, expected,
        )
        .map_err(|_| "DEV03_RUNTIME_UNTRUSTED")?;
        let service = resource
            .join("recovery")
            .join("windows_recovery_service.exe");
        let service_sha = approved_pin(PINNED_SERVICE)?;
        if digest_file(&service)? != service_sha {
            return Err("DEV03_RECOVERY_SERVICE_PIN_MISMATCH".into());
        }
        let broker = TrustedBroker {
            executable: service,
            expected_sha256: service_sha.to_owned(),
            private_witness_directory: resource.join("recovery"),
        };
        Ok(Installation {
            toolchain_sha256: attested.tree_sha256,
            toolchain,
            broker,
        })
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        let actual =
            lu_ide_execution_supervisor::linux_worker::installed_multilang_toolchain_digest(
                recipe_id,
            )
            .map_err(|_| "DEV03_RUNTIME_UNTRUSTED")?;
        if actual != expected {
            return Err("DEV03_RUNTIME_PIN_MISMATCH".into());
        }
        Ok(Installation {
            toolchain_sha256: actual,
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
    pub terminal_receipt_sha256: String,
    pub debug_binary_candidate: Option<DebugBinaryWitness>,
    pub debug_symbol_witness: Option<DebugSymbolWitness>,
    pub artifact_manifest_sha256: Option<String>,
    pub artifact_evidence_state: String,
    pub artifact_files: Vec<ArtifactEntry>,
    pub artifact_count: usize,
    pub artifact_truncated: bool,
    /// INT03-E: bounded framework-reported rows, never test assertion proof.
    pub test_case_evidence: Option<TestCaseReport>,
    pub timed_out: bool,
    pub requires_reconciliation: bool,
    pub installed_app_accepted: bool,
    #[cfg(feature = "native-package-uv")]
    pub package_output_audit: Option<PackageOutputAudit>,
    #[cfg(feature = "native-package-uv")]
    pub package_audit_state: &'static str,
    #[cfg(feature = "native-package-uv")]
    pub postrun_original_manifest_lock_unchanged: bool,
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
    /// Separate native Cargo proof. A pinned Python/Node/CMake bundle
    /// must never make the uninstalled Cargo recipe appear runnable.
    cargo_ready: bool,
    // Global OS worker custody has process scope, not selected project
    // scope. Expose only booleans; foreign job IDs and output stay private.
    global_recovery_required: bool,
    active_in_another_project: bool,
    global_worker_busy: bool,
    cargo_provenance: RuntimeProvenance,
    multilang_ready: Vec<MultilangProfileReadiness>,
    code: String,
    description: &'static str,
    current: Option<ActiveView>,
    // Ephemeral identity only; never an output path or replayable receipt.
    last_output_job_id: Option<String>,
    recent: Vec<BuildRunView>,
    // Persisted supervisor journal; unlike recent, survives a host restart.
    journal: Vec<RecoveryView>,
    quarantined: Vec<RecoveryView>,
    #[cfg(feature = "native-package-uv")]
    package_audits: Vec<PackageLifecycleStatus>,
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
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestTargetReview {
    pub project_id: String,
    pub job_id: String,
    pub plan_sha256: String,
    pub terminal_receipt_sha256: String,
    pub case_id: String,
    pub relative_path: String,
    pub disk_sha256: String,
    pub line: u32,
    pub character_utf16: u32,
    /// Fresh host source hash only; location still originates from reporter.
    pub saved_source_verified: bool,
    pub test_assertion_independently_verified: bool,
    pub task_accepted: bool,
}
struct Running {
    project_id: String,
    recipe_id: String,
    job_id: Option<String>,
    cancel: Arc<NativeCancel>,
    requested: Arc<AtomicBool>,
    output: Arc<LiveOutput>,
}
struct Store {
    current: Option<Running>,
    recent: BTreeMap<String, BuildRunView>,
    last_output: Option<(String, String, Arc<LiveOutput>)>,
    debug_target: Option<Arc<SealedProjectDebugTarget>>,
}
pub struct NativeBuildRuntime {
    supervisor: Arc<Supervisor>,
    global_custody_profile: PathBuf,
    staged: PathBuf,
    outputs: PathBuf,
    debug_targets: PathBuf,
    #[cfg(feature = "native-package-uv")]
    package_lifecycle: PackageLifecycleLedger,
    store: Mutex<Store>,
}
impl NativeBuildRuntime {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let data = app
            .path()
            .app_data_dir()
            .map_err(|_| "BUILD_PROFILE_MISSING")?;
        fs::create_dir_all(&data).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        let staged = data.join("build-snapshots");
        let outputs = data.join("build-outputs");
        let debug_targets = data.join("sealed-debug-targets");
        fs::create_dir_all(&staged).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        fs::create_dir_all(&outputs).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        fs::create_dir_all(&debug_targets).map_err(|_| "BUILD_PROFILE_UNAVAILABLE")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&debug_targets, fs::Permissions::from_mode(0o700))
                .map_err(|_| "BUILD_PRIVATE_DEBUG_STORAGE_DENIED")?;
        }
        let supervisor = Supervisor::open(data.join("build-job-receipts.jsonl"))
            .map_err(|e| e.code().to_owned())?;
        #[cfg(feature = "native-package-uv")]
        let package_lifecycle =
            PackageLifecycleLedger::open(data.clone()).map_err(|e| e.code().to_owned())?;
        Ok(Self {
            supervisor: Arc::new(supervisor),
            global_custody_profile: data.clone(),
            staged,
            outputs,
            debug_targets,
            #[cfg(feature = "native-package-uv")]
            package_lifecycle,
            store: Mutex::new(Store {
                current: None,
                recent: BTreeMap::new(),
                last_output: None,
                debug_target: None,
            }),
        })
    }
    pub fn status(&self, app: &AppHandle, project_id: &str) -> Result<BuildStatus, String> {
        let installed_cargo=installation(app);
        // Host OS admission is a different fact from the resource hash.
        // A normal Linux WebView process is not automatically a delegated
        // systemd service: it must not advertise a launchable worker.
        #[cfg(target_os="linux")]
        let native_os_ready=lu_ide_execution_supervisor::linux_worker
            ::native_delegated_host_readiness().is_ok();
        #[cfg(windows)]
        let native_os_ready=true; // Windows service + Job validated per job.
        let cargo_provenance = runtime_provenance("rust_cargo",
            installed_cargo.as_ref().ok()
                .map(|bundle|bundle.toolchain_sha256.clone()),PINNED_CARGO);
        let availability=if native_os_ready{installed_cargo}
            else{Err("FIN02_LINUX_DELEGATED_HOST_NOT_ADMITTED".into())};
        let multilang_ready: Vec<MultilangProfileReadiness> = MULTILANG_RECIPES
            .iter()
            .map(|&profile_id| {
                #[cfg(feature = "native-multilang")]
                let result: Result<Installation, String> =
                    if !native_os_ready {
                        Err("FIN02_LINUX_DELEGATED_HOST_NOT_ADMITTED".into())
                    } else if profile_id == "uv_sync_offline" && cfg!(windows) {
                        // A valid hash cannot substitute for Windows source
                        // AppContainer ACL acceptance: fail closed.
                        Err("IDE053_WINDOWS_WORKSPACE_ACL_NOT_ACCEPTED".into())
                    } else {
                        installation_multilang(app, profile_id)
                    };
                #[cfg(not(feature = "native-multilang"))]
                let result: Result<Installation, String> =
                    Err("DEV03_OPTIN_FEATURE_DISABLED".into());
                #[cfg(feature = "native-multilang")]
                let pin = multilang_release_pin(profile_id);
                #[cfg(not(feature = "native-multilang"))]
                let pin = None;
                let provenance = runtime_provenance(profile_id,
                    result.as_ref().ok().map(|bundle|bundle.toolchain_sha256.clone()),pin);
                MultilangProfileReadiness {
                    provenance,
                    profile_id,
                    ready: result.is_ok(),
                    code: if !native_os_ready {
                        "FIN02_LINUX_DELEGATED_HOST_NOT_ADMITTED"
                    } else if profile_id == "uv_sync_offline" && cfg!(windows) {
                        "IDE053_WINDOWS_WORKSPACE_ACL_NOT_ACCEPTED"
                    } else if result.is_ok() {
                        "DEV03_RUNTIME_PIN_VERIFIED_NATIVE_ADMISSION_PER_JOB"
                    } else {
                        "DEV03_RUNTIME_NOT_READY"
                    },
                }
            })
            .collect();
        let locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        let global_worker_busy = locked.current.is_some();
        let active_in_another_project = locked.current
            .as_ref().is_some_and(|v| v.project_id != project_id);
        let current = locked
            .current
            .as_ref()
            .filter(|job| job.project_id == project_id)
            .map(|job| ActiveView {
                project_id: job.project_id.clone(),
                job_id: job.job_id.clone(),
                recipe_id: job.recipe_id.clone(),
                cancellation_requested: job.requested.load(Ordering::SeqCst),
            });
        let last_output_job_id = locked
            .last_output
            .as_ref()
            .filter(|(project, _, _)| project == project_id)
            .map(|(_, job_id, _)| job_id.clone());
        let recent = locked
            .recent
            .values()
            .filter(|job| job.project_id == project_id)
            .cloned()
            .collect();
        drop(locked);
        let durable = self.supervisor.inspect().map_err(|e| e.code().to_owned())?;
        // This uses the full original durable journal, not the truncated
        // selected-project presentation. No foreign job ID is published.
        let global_custody =
            lu_ide_execution_supervisor::observe_global_custody(&durable,project_id);
        let unresolved_global_marker=crate::global_native_custody
            ::global_custody_unresolved(&self.global_custody_profile)?;
        let other=crate::global_native_custody::sibling_rows(
            &self.global_custody_profile,"build")?;
        let other_custody=lu_ide_execution_supervisor
            ::observe_global_custody(&other,project_id);
        let global_recovery_required = global_custody.recovery_required ||
            other_custody.recovery_required || unresolved_global_marker;
        // A durable nonterminal job is busy even after a lost desktop
        // process handle; never imply global idleness from local store only.
        let global_worker_busy = global_worker_busy ||
            unresolved_global_marker ||
            global_custody.worker_busy_or_unreconciled ||
            other_custody.worker_busy_or_unreconciled;
        let active_in_another_project = active_in_another_project ||
            global_custody.active_in_another_project ||
            other_custody.active_in_another_project;
        // Serialize only exact project-scoped immutable journal identifiers.
        // Do not reconstruct a BuildRunView or invent a successful native exit
        // code after restart: the original runtime receipt is authoritative.
        // Never truncate quarantine state. Every blocked job is still visible
        // and eligible for exact native OS-only reconciliation.
        let quarantined: Vec<RecoveryView> = durable
            .iter()
            .filter(|job| job.project_id == project_id && job.requires_reconciliation)
            .map(|job| RecoveryView {
                project_id: job.project_id.clone(),
                job_id: job.job_id.clone(),
                plan_sha256: job.plan_sha256.clone(),
                phase: native_phase_label(job.phase).to_owned(),
                requires_reconciliation: true,
            })
            .collect();
        // The UI displays a bounded most-recent subset, not all secrets or
        // unbounded durable history. The security quarantine above is full.
        let mut journal: Vec<RecoveryView> = durable
            .into_iter()
            .rev()
            .filter(|job| job.project_id == project_id)
            .take(32)
            .map(|job| RecoveryView {
                project_id: job.project_id,
                job_id: job.job_id,
                plan_sha256: job.plan_sha256,
                phase: native_phase_label(job.phase).to_owned(),
                requires_reconciliation: job.requires_reconciliation,
            })
            .collect();
        journal.reverse();
        #[cfg(feature = "native-package-uv")]
        let package_audits: Vec<PackageLifecycleStatus> = self
            .package_lifecycle
            .statuses()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .filter(|row| row.project_id == project_id)
            .collect();
        #[cfg(feature = "native-package-uv")]
        for row in &package_audits {
            // Never project a separate audit log as native OS success by
            // itself. Join to the independently replayed D3 supervisor
            // journal using exact project, job, plan and terminal receipt.
            let original = self
                .supervisor
                .inspect()
                .map_err(|e| e.code().to_owned())?
                .into_iter()
                .find(|job| job.job_id == row.job_id)
                .ok_or("IDE054_ORIGINAL_D3_JOB_MISSING")?;
            if original.project_id != row.project_id || original.plan_sha256 != row.plan_sha256 {
                return Err("IDE054_AUDIT_JOURNAL_SCOPE_MISMATCH".into());
            }
            let history = self
                .supervisor
                .receipt_history(&row.job_id)
                .map_err(|e| e.code().to_owned())?;
            if row.state != "original_package_job_requires_native_reconciliation"
                && !history.iter().any(|entry| {
                    entry.entry_hash == row.terminal_receipt_sha256
                        && entry.body.project_id == row.project_id
                        && entry.body.plan_sha256 == row.plan_sha256
                })
            {
                return Err("IDE054_AUDIT_TERMINAL_RECEIPT_UNVERIFIED".into());
            }
        }
        let cargo_ready = availability.is_ok();
        let any_native_profile_ready = cargo_ready ||
            multilang_ready.iter().any(|profile| {
                profile.profile_id != "uv_sync_offline" && profile.ready
            });
        Ok(BuildStatus {
            ready: any_native_profile_ready,
            cargo_ready,
            global_recovery_required,
            active_in_another_project,
            global_worker_busy,
            cargo_provenance,
            multilang_ready,
            code: if cargo_ready {
                "BUILD_RELEASE_RESOURCES_VERIFIED_ADMISSION_PER_JOB".into()
            } else if any_native_profile_ready {
                "BUILD_MULTILANG_RELEASE_PINS_VERIFIED_ADMISSION_PER_JOB".into()
            } else {
                availability.err().unwrap_or_else(|| "BUILD_NATIVE_RUNTIME_ABSENT".into())
            },
            description: "Pinned resources are not execution proof. Native Job/cgroup \
                admission, human approval and terminal OS cleanup are checked for each job.",
            current,
            last_output_job_id,
            recent,
            journal,
            quarantined,
            #[cfg(feature = "native-package-uv")]
            package_audits,
        })
    }
    /// Read only the existing protected WO6 supervisor's durable OS journal.
    /// A user-writable WO9 graph file can never self-certify a successful
    /// native step after host restart. This does not launch or re-run code.
    pub fn verify_replayed_completed_job(
        &self,
        project_id: &str,
        job_id: &str,
        plan_sha: &str,
    ) -> Result<(), String> {
        if job_id.is_empty()
            || plan_sha.len() != 64
            || !plan_sha.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("WO9_NATIVE_REPLAY_IDENTITY_INVALID".into());
        }
        let job = self
            .supervisor
            .inspect()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or("WO9_NATIVE_REPLAY_JOB_ABSENT")?;
        if job.project_id != project_id || job.plan_sha256 != plan_sha {
            return Err("WO9_NATIVE_REPLAY_PROJECT_OR_PLAN_MISMATCH".into());
        }
        if job.phase != JobPhase::Completed || job.requires_reconciliation {
            return Err("WO9_NATIVE_REPLAY_NOT_TERMINALLY_VERIFIED".into());
        }
        Ok(())
    }

    /// No WebView path or artifact selector. This returns only the newest
    /// same-process, complete, exact-job sealed native Cargo output.
    pub fn native_project_debug_target(
        &self,
        project_id: &str,
    ) -> Result<Arc<SealedProjectDebugTarget>, String> {
        let target = {
            let locked = self
                .store
                .lock()
                .map_err(|_| "DEV044_BUILD_STATE_UNAVAILABLE")?;
            if locked.current.is_some() {
                return Err("DEV044_BUILD_IN_PROGRESS".into());
            }
            locked
                .debug_target
                .as_ref()
                .filter(|x| x.lineage.project_id == project_id)
                .cloned()
                .ok_or("DEV044_NO_SEALED_BUILD_TARGET")?
        };
        self.supervisor
            .verify_original_completed_cargo_build_for_debug(&target.lineage)
            .map_err(|e| e.code().to_owned())?;
        target
            .verify()
            .map_err(|_| "DEV044_PRIVATE_TARGET_CHANGED".to_owned())?;
        Ok(target)
    }
    pub fn cancel(&self, project_id: &str, job_id: Option<&str>) -> Result<&'static str, String> {
        let locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        let active = locked.current.as_ref().ok_or("BUILD_NO_ACTIVE_JOB")?;
        if active.project_id != project_id {
            return Err("BUILD_JOB_SCOPE_MISMATCH".into());
        }
        // A known native job must be identified exactly: omitting its ID
        // cannot cancel a replacement job admitted between UI polls.
        match (active.job_id.as_deref(), job_id) {
            (Some(current), Some(expected)) if current == expected => {}
            (None, None) => {} // Only preflight, before a native ID exists.
            _ => return Err("BUILD_CANCEL_EXACT_JOB_ID_REQUIRED".into()),
        }
        if active.requested.swap(true, Ordering::SeqCst) {
            return Err("BUILD_CANCEL_ALREADY_REQUESTED".into());
        }
        active.cancel.request_cancel();
        // Request is intentionally NOT mislabeled Cancelled. The worker must
        // first fsync the cancellation and prove native process-tree kill.
        Ok("BUILD_CANCELLATION_REQUESTED")
    }
    /// Only current native job or most recently finished in this process.
    /// No disk path is accepted, no journal is upgraded to log authority.
    pub fn read_output(
        &self,
        project_id: &str,
        job_id: &str,
        after: u64,
    ) -> Result<OutputPage, String> {
        if after > 1_000_000 {
            return Err("BUILD_OUTPUT_CURSOR_INVALID".into());
        }
        let state = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        if let Some(current) = state.current.as_ref() {
            if current.project_id == project_id && current.job_id.as_deref() == Some(job_id) {
                return Ok(current.output.page(after));
            }
        }
        if let Some((project, job, output)) = state.last_output.as_ref() {
            if project == project_id && job == job_id {
                return Ok(output.page(after));
            }
        }
        Err("BUILD_OUTPUT_NOT_ACTIVE_OR_RECENT".into())
    }
    pub fn run(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<BuildRunView, String> {
        if !VALID_RECIPES.contains(&recipe_id)
            && !(cfg!(feature = "native-multilang")
                && MULTILANG_RECIPES.contains(&recipe_id)
                && recipe_id != "uv_sync_offline")
        {
            return Err("BUILD_RECIPE_DENIED".into());
        }
        self.run_with_review(app, registry, project_id, recipe_id, None, None, None)
    }

    /// INT03: the user explicitly selects an already-discovered profile
    /// whose saved manifest SHA is carried through native freezing and fresh
    /// OS approval. No old review is a substitute for worker permission.
    pub fn run_profile(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        recipe_id: &str,
        expected_manifest_sha256: &str,
    ) -> Result<BuildRunView, String> {
        let manifest = profile_manifest(recipe_id).ok_or("INT03_PROFILE_NOT_ALLOWLISTED")?;
        if !VALID_RECIPES.contains(&recipe_id) &&
            !(cfg!(feature = "native-multilang") &&
              MULTILANG_RECIPES.contains(&recipe_id)) {
            return Err("INT03_NATIVE_ADAPTER_UNAVAILABLE".into());
        }
        if expected_manifest_sha256.len() != 64 ||
            !expected_manifest_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("INT03_MANIFEST_SHA_INVALID".into());
        }
        let saved = registry.preview_source_file(project_id, manifest)
            .map_err(|_| "INT03_MANIFEST_NOT_REGISTERED")?;
        if saved.project_id != project_id ||
            saved.relative_path != manifest ||
            saved.revision != expected_manifest_sha256 {
            return Err("INT03_DISCOVERED_MANIFEST_STALE".into());
        }
        self.run_with_review(app, registry, project_id, recipe_id, None,
            Some(expected_manifest_sha256),None)
    }

    /// Only the separately named native package entrypoint can choose the
    /// uv recipe. A generic build request may NEVER inherit package writes.
    #[cfg(feature = "native-package-uv")]
    pub fn run_reviewed_uv_sync(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        expected_preview_sha256: &str,
        expected_evidence_fingerprint: &str,
    ) -> Result<BuildRunView, String> {
        use lu_ide_workspace::package_preview::{PackageOperation, PackagePreviewRequest};
        let request = PackagePreviewRequest {
            manager_id: "uv".into(),
            operation: PackageOperation::Sync,
            package_name: None,
            exact_version: None,
        };
        registry
            .revalidate_package_operation(
                project_id,
                request.clone(),
                expected_preview_sha256,
                expected_evidence_fingerprint,
            )
            .map_err(str::to_owned)?;
        self.run_with_review(
            app,
            registry,
            project_id,
            "uv_sync_offline",
            Some(ReviewedPackageSync {
                request,
                preview_sha256: expected_preview_sha256.to_owned(),
                evidence_fingerprint: expected_evidence_fingerprint.to_owned(),
            }),
            None,
            None,
        )
    }

    /// FIN04: one native-discovered saved Rust test name, sealed with Cargo
    /// and original test source SHA, executed only under a fresh OS approval.
    pub fn run_selected_rust_case(
        &self,app:&AppHandle,registry:&IdeWorkspaceRegistry,
        project_id:&str,relative_path:&str,case_name:&str,
        expected_source_revision:&str,expected_manifest_sha256:&str
    )->Result<BuildRunView,String>{
        let source=registry.inspect_test_symbol_candidates(project_id,relative_path)
            .map_err(str::to_owned)?;
        if source.project_id!=project_id ||
           source.language!="rust" ||
           source.source_revision!=expected_source_revision ||
           relative_path.starts_with("ide/host/") ||
           !source.symbols.iter().any(|row|row.name==case_name &&
               row.candidate_only && !row.executable_test_verified){
            return Err("FIN04_SELECTED_SAVED_TEST_STALE_OR_UNKNOWN".into());
        }
        let recipe_id=format!("rust_exact_case:{case_name}");
        if lu_ide_execution_supervisor::rust_exact_case_name(&recipe_id)
            !=Some(case_name){
            return Err("FIN04_SELECTED_TEST_NAME_INVALID".into());
        }
        let manifest=registry.preview_source_file(project_id,"Cargo.toml")
            .map_err(|_|"FIN04_TEST_CARGO_MANIFEST_MISSING")?;
        if manifest.project_id!=project_id ||
           manifest.revision!=expected_manifest_sha256{
            return Err("FIN04_TEST_MANIFEST_STALE".into());
        }
        self.run_with_review(app,registry,project_id,&recipe_id,None,
            Some(expected_manifest_sha256),
            Some((relative_path.to_owned(),expected_source_revision.to_owned())))
    }

    fn run_with_review(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        recipe_id: &str,
        reviewed: Option<ReviewedPackageSync>,
        expected_profile_sha: Option<&str>,
        selected_case_source:Option<(String,String)>,
    ) -> Result<BuildRunView, String> {
        if lu_ide_execution_supervisor::rust_exact_case_name(recipe_id).is_some()
           && selected_case_source.is_none(){
           return Err("FIN04_RUST_EXACT_CASE_NOT_NATIVE_REVIEWED".into());
        }
        if recipe_id == "uv_sync_offline" {
            #[cfg(feature = "native-package-uv")]
            if reviewed.is_none() {
                return Err("IDE053_PACKAGE_REVIEW_REQUIRED".into());
            }
            #[cfg(not(feature = "native-package-uv"))]
            return Err("IDE053_PACKAGE_WORKER_NOT_COMPILED".into());
        }
        // Acquire before the in-memory current-job slot is set. A denied
        // global lease cannot leave a phantom active build.
        // Refuse known busy journals BEFORE creating the profile-wide marker.
        // The exclusive create_new still serializes the final admission, and
        // the original journals are rechecked while it is held.
        crate::global_native_custody::preflight_global_idle(
            &self.global_custody_profile,"build")?;
        let mut global_fence=crate::global_native_custody::GlobalAdmissionFence
            ::acquire(&self.global_custody_profile,"build")?;
        let own_before=self.supervisor.inspect()
            .map_err(|e|e.code().to_owned())?;
        if crate::global_native_custody::durable_busy(&own_before) {
            return Err("FIN01_BUILD_JOURNAL_STILL_OWNS_CUSTODY".into());
        }
        let sibling=crate::global_native_custody::sibling_rows(
            &self.global_custody_profile,"build")?;
        if crate::global_native_custody::durable_busy(&sibling) {
            return Err("FIN01_DEBUG_JOURNAL_STILL_OWNS_CUSTODY".into());
        }
        let cancel = Arc::new(NativeCancel::default());
        let requested = Arc::new(AtomicBool::new(false));
        let output = Arc::new(LiveOutput::default());
        {
            let mut locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
            if locked.current.is_some() {
                return Err("BUILD_ANOTHER_JOB_ACTIVE".into());
            }
            locked.last_output = None;
            locked.current = Some(Running {
                project_id: project_id.to_owned(),
                recipe_id: recipe_id.to_owned(),
                job_id: None,
                cancel: Arc::clone(&cancel),
                requested: Arc::clone(&requested),
                output: Arc::clone(&output),
            });
        }
        {
            // Never allow a superseded project's build to be used by a
            // fresh debugger request, even if the new build is denied.
            let mut store = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
            store.debug_target = None;
        }
        let result = self.run_inner(
            app,
            registry,
            project_id,
            recipe_id,
            &cancel,
            &requested,
            &output,
            reviewed.as_ref(),
            expected_profile_sha,
            selected_case_source.as_ref(),
        );
        let mut locked = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
        let finished_output = locked.current.as_ref().and_then(|active| {
            active.job_id.as_ref().map(|job_id| {
                (
                    active.project_id.clone(),
                    job_id.clone(),
                    Arc::clone(&active.output),
                )
            })
        });
        locked.last_output = finished_output;
        locked.current = None;
        if let Ok(ref view) = result {
            locked.recent.insert(view.job_id.clone(), view.clone());
            while locked.recent.len() > MAX_HISTORY {
                if let Some(old) = locked.recent.keys().next().cloned() {
                    locked.recent.remove(&old);
                }
            }
        }
        // Do not release admission on merely returning from a worker.
        // Both durable journals must have terminal, reconciled custody.
        let own=self.supervisor.inspect().map_err(|e|e.code().to_owned());
        let sibling=crate::global_native_custody::sibling_rows(
            &self.global_custody_profile,"build");
        let release=match (own,sibling){
            (Ok(own),Ok(sibling))=>
                global_fence.release_if_terminal(&own,&sibling),
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
        recipe_id: &str,
        cancel: &Arc<NativeCancel>,
        requested: &Arc<AtomicBool>,
        output: &Arc<LiveOutput>,
        reviewed: Option<&ReviewedPackageSync>,
        expected_profile_sha: Option<&str>,
        selected_case_source:Option<&(String,String)>,
    ) -> Result<BuildRunView, String> {
        // No Build/Test start may overlap an unresolved OS process tree.
        // A green recipe listing, cached editor plan or new approval dialog
        // is not proof that a previous native worker was reaped.
        if self.supervisor.inspect().map_err(|e| e.code().to_owned())?
            .iter().any(|job| job.requires_reconciliation) {
            return Err("INT03_PRIOR_KERNEL_RECONCILIATION_REQUIRED".into());
        }
        #[cfg(feature = "native-multilang")]
        let installed = if MULTILANG_RECIPES.contains(&recipe_id) {
            installation_multilang(app, recipe_id)?
        } else {
            installation(app)?
        };
        #[cfg(not(feature = "native-multilang"))]
        let installed = installation(app)?;
        let source_root = registry
            .trusted_native_worker_root(project_id)
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
        #[cfg(feature = "native-package-uv")]
        let native_package_review = if recipe_id == "uv_sync_offline" {
            let reviewed = reviewed.ok_or("IDE053_PACKAGE_REVIEW_REQUIRED")?;
            let current = registry
                .preview_package_operation(project_id, reviewed.request.clone())
                .map_err(str::to_owned)?;
            if current.preview_sha256 != reviewed.preview_sha256
                || current.evidence_fingerprint != reviewed.evidence_fingerprint
            {
                return Err("IDE053_SOURCE_CHANGED_BEFORE_FREEZE".into());
            }
            Some(current)
        } else {
            None
        };
        let input = publish_input(&source_root, &self.staged, recipe_id)
            .map_err(|e| e.code().to_owned())?;
        verify_input(&input).map_err(|e| e.code().to_owned())?;
        if let Some(expected) = expected_profile_sha {
            // Rebind the user-selected saved source revision to bytes
            // independently rehashed in the immutable native worker input.
            if input.seal.manifest_relative_path != profile_manifest(recipe_id)
                    .ok_or("INT03_PROFILE_NOT_ALLOWLISTED")? ||
                input.seal.manifest_sha256 != expected {
                return Err("INT03_FROZEN_MANIFEST_MISMATCH".into());
            }
        }
        if let Some((relative,revision))=selected_case_source {
            if lu_ide_execution_supervisor::rust_exact_case_name(recipe_id).is_none()
                || relative.len()>1024
                || relative.starts_with('/')
                || relative.contains(':') || relative.contains('\\')
                || relative.split('/').any(|p|p.is_empty()||p=="."||p=="..")
                || revision.len()!=64
                || !revision.bytes().all(|c|c.is_ascii_hexdigit()){
                return Err("FIN04_SELECTED_CASE_SOURCE_DENIED".into());
            }
            let path=input.root.join(relative);
            let info=fs::symlink_metadata(&path)
                .map_err(|_|"FIN04_FROZEN_TEST_SOURCE_MISSING")?;
            if !info.is_file() || info.file_type().is_symlink() ||
                info.len()>512*1024 {
                return Err("FIN04_FROZEN_TEST_SOURCE_UNTRUSTED".into());
            }
            let data=fs::read(&path)
                .map_err(|_|"FIN04_FROZEN_TEST_SOURCE_READ_FAILED")?;
            if data.len() as u64!=info.len() ||
               format!("{:x}",Sha256::digest(&data))!=*revision{
                return Err("FIN04_FROZEN_SOURCE_DIFFERS_FROM_TEST_REVIEW".into());
            }
        }
        #[cfg(feature = "native-package-uv")]
        if let Some(review) = native_package_review.as_ref() {
            let py = review
                .evidence
                .iter()
                .find(|e| e.relative_path == "pyproject.toml")
                .and_then(|e| e.sha256.as_deref())
                .ok_or("IDE053_REVIEWED_MANIFEST_SHA_MISSING")?;
            let lock = review
                .evidence
                .iter()
                .find(|e| e.relative_path == "uv.lock")
                .and_then(|e| e.sha256.as_deref())
                .ok_or("IDE053_REVIEWED_LOCK_SHA_MISSING")?;
            if input.seal.manifest_sha256 != py
                || sha256_private_staged_file(&input.root, "uv.lock")? != lock
            {
                return Err("IDE053_FROZEN_SOURCE_DIFFERS_FROM_APPROVED_REVIEW".into());
            }
            // IDE-05.4: preflight all frozen uv.lock package entries BEFORE
            // worker admission. A postrun provenance audit is not enough
            // if editable/Git/path sources can run their build backend.
            verify_offline_registry_lock(&input.root, lock)
                .map_err(|_| "IDE054_FROZEN_LOCK_SOURCE_POLICY_DENIED")?;
        }
        let plan = JobPlan {
            project_id: project_id.to_owned(),
            recipe_id: recipe_id.to_owned(),
            source_head: head.clone(),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: installed.toolchain_sha256.clone(),
            debug_target_binding_sha256: None,
            runtime_limit_ms: if recipe_id == "uv_sync_offline" {
                3 * 60_000
            } else {
                MAX_RUNTIME_MS
            },
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 8,
            log_limit_bytes: 128 * 1024,
            effect_class: {
                #[cfg(feature = "native-package-uv")]
                if recipe_id == "uv_sync_offline" {
                    format!(
                        "package_uv_sync_offline:{}",
                        reviewed
                            .ok_or("IDE053_PACKAGE_REVIEW_REQUIRED")?
                            .preview_sha256
                    )
                } else {
                    "test_build_isolated".into()
                }
                #[cfg(not(feature = "native-package-uv"))]
                {
                    "test_build_isolated".into()
                }
            },
        };
        let prepared = self
            .supervisor
            .prepare(plan.clone())
            .map_err(|e| e.code().to_owned())?;
        #[cfg(feature = "native-package-uv")]
        if recipe_id == "uv_sync_offline" {
            // Persist this identity before possible human approval, OS job
            // creation or worker launch. Restart shows unresolved prep if
            // postrun audit was interrupted, never auto-retry.
            self.package_lifecycle
                .record_prepared(project_id, &prepared.job_id, &prepared.plan_sha256)
                .map_err(|e| e.code().to_owned())?;
        }
        {
            let mut state = self.store.lock().map_err(|_| "BUILD_HOST_UNAVAILABLE")?;
            let active = state.current.as_mut().ok_or("BUILD_HOST_UNAVAILABLE")?;
            if active.project_id != project_id {
                return Err("BUILD_JOB_SCOPE_MISMATCH".into());
            }
            active.job_id = Some(prepared.job_id.clone());
        }
        if requested.load(Ordering::SeqCst) {
            #[cfg(feature = "native-package-uv")]
            if recipe_id == "uv_sync_offline" {
                self.supervisor
                    .cancel_before_start(&prepared.job_id)
                    .map_err(|e| e.code().to_owned())?;
            }
            return Err("BUILD_CANCELLED_BEFORE_AUTHORIZATION".into());
        }
        let approve = || -> Result<(), lu_ide_execution_supervisor::SupervisorError> {
            use lu_ide_execution_supervisor::SupervisorError;
            if requested.load(Ordering::SeqCst) {
                return Err(SupervisorError::InvalidApproval);
            }
            #[cfg(feature = "native-package-uv")]
            let choice = if recipe_id == "uv_sync_offline" {
                let reviewed = reviewed.ok_or(SupervisorError::InvalidApproval)?;
                native_package_consent(
                    app,
                    format!(
                        "Approve one OFFLINE LOCKED uv Sync in an OS-protected worker?\n\n\
                        Project: {project_id}\nExact source HEAD: {head}\n\
                        One-use native Job ID: {}\nFull sealed JobPlan SHA-256: {}\n\
                        Manifest + lock evidence: {}\nReviewed operation: {}\n\
                        Frozen source tree: {}\nPinned uv runtime: {}\n\n\
                        Source /workspace is read only; writable environment is in \
                        disposable private /output. No network namespace or AppContainer \
                        network capabilities; builds, editable workspace installs, and \
                        arbitrary package manager arguments are disabled.\n\
                        Dependency output will be independently audited. Deny if source changed.",
                        prepared.job_id,
                        prepared.plan_sha256,
                        reviewed.evidence_fingerprint,
                        reviewed.preview_sha256,
                        plan.input_snapshot_sha256,
                        plan.toolchain_sha256,
                    ),
                    requested,
                )
            } else {
                app.dialog()
                    .message(format!(
                        "Run an exact restricted offline project build/test job?\n\n\
                        Project: {project_id}\nRecipe: {recipe_id}\nSource HEAD: {head}\n\
                        Frozen source SHA-256: {}\nToolchain SHA-256: {}\n\n\
                        The OS worker will execute code from the sealed copy. No project files \
                        are writable from that worker. Cancellation requests require proven \
                        kernel process-tree termination.",
                        plan.input_snapshot_sha256, plan.toolchain_sha256,
                    ))
                    .title("Allow one protected build/test job")
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Run this exact job".into(),
                        "Cancel".into(),
                    ))
                    .blocking_show()
            };
            #[cfg(not(feature = "native-package-uv"))]
            let choice = app
                .dialog()
                .message(format!(
                    "Run an exact restricted offline project build/test job?\n\n\
                    Project: {project_id}\nRecipe: {recipe_id}\nSource HEAD: {head}\n\
                    Frozen source SHA-256: {}\nToolchain SHA-256: {}\n\n\
                    The OS worker will execute code from the sealed copy. No project files \
                    are writable from that worker. Cancellation requests require proven \
                    kernel process-tree termination.",
                    plan.input_snapshot_sha256, plan.toolchain_sha256,
                ))
                .title("Allow one protected build/test job")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Run this exact job".into(),
                    "Cancel".into(),
                ))
                .blocking_show();
            if !choice || requested.load(Ordering::SeqCst) {
                return Err(SupervisorError::InvalidApproval);
            }
            let current_root = registry
                .trusted_native_worker_root(project_id)
                .map_err(|_| SupervisorError::WrongProjectOrSource)?;
            let fresh = registry.inspect_git_worktree(project_id);
            if current_root != source_root
                || fresh.local_head.as_deref() != Some(head.as_str())
                || fresh.worktree != "clean"
                || fresh.state != "local_repository_inspected"
                || fresh.truncated
            {
                return Err(SupervisorError::WrongProjectOrSource);
            }
            // Last native refresh after the real human dialog and after
            // sealed worker staging on Windows. No cached WebView approval.
            #[cfg(feature = "native-package-uv")]
            if recipe_id == "uv_sync_offline" {
                let reviewed = reviewed.ok_or(SupervisorError::InvalidApproval)?;
                registry
                    .revalidate_package_operation(
                        project_id,
                        reviewed.request.clone(),
                        &reviewed.preview_sha256,
                        &reviewed.evidence_fingerprint,
                    )
                    .map_err(|_| SupervisorError::WrongProjectOrSource)?;
            }
            if let Some(expected) = expected_profile_sha {
                let manifest = profile_manifest(recipe_id)
                    .ok_or(SupervisorError::InvalidApproval)?;
                let fresh = registry.preview_source_file(project_id, manifest)
                    .map_err(|_| SupervisorError::WrongProjectOrSource)?;
                if fresh.project_id != project_id ||
                   fresh.relative_path != manifest || fresh.revision != expected ||
                   input.seal.manifest_sha256 != expected {
                    return Err(SupervisorError::WrongProjectOrSource);
                }
            }
            if requested.load(Ordering::SeqCst) {
                return Err(SupervisorError::InvalidApproval);
            }
            self.supervisor.record_human_approval(
                &prepared.job_id,
                &prepared.plan_sha256,
                &head,
                true,
            )?;
            Ok(())
        };
        #[cfg(windows)]
        let native_attempt = execute_windows_worker_after_staging_approval_with_output(
            &self.supervisor,
            &prepared.job_id,
            &plan,
            &head,
            &input,
            &installed.toolchain,
            &self.outputs,
            &installed.broker,
            cancel,
            approve,
            Some(Arc::clone(output)),
        );
        #[cfg(target_os = "linux")]
        let native_attempt = (|| {
            approve()?;
            execute_linux_worker_with_output(
                &self.supervisor,
                &prepared.job_id,
                &plan,
                &input,
                &self.outputs,
                cancel,
                Some(Arc::clone(output)),
            )
        })();
        let native = match native_attempt {
            Ok(receipt) => receipt,
            Err(reason) => {
                // Only a Job that NEVER reached OS admission can receive a
                // before-start terminal receipt. Running or uncertain jobs
                // MUST remain under OS cleanup/quarantine, never be labelled
                // harmlessly cancelled by a rejected dialog.
                #[cfg(feature = "native-package-uv")]
                if recipe_id == "uv_sync_offline" {
                    let phase = self
                        .supervisor
                        .inspect()
                        .map_err(|e| e.code().to_owned())?
                        .into_iter()
                        .find(|job| job.job_id == prepared.job_id)
                        .ok_or("IDE053_PACKAGE_JOB_RECEIPT_MISSING")?
                        .phase;
                    if matches!(phase, JobPhase::Prepared | JobPhase::Authorized) {
                        self.supervisor
                            .cancel_before_start(&prepared.job_id)
                            .map_err(|e| e.code().to_owned())?;
                    }
                }
                return Err(reason.code().to_owned());
            }
        };
        // A package job requires a distinct original receipt check; the
        // Cargo build reviewer must NEVER silently accept its effect class.
        #[cfg(feature = "native-package-uv")]
        let evidence = if recipe_id == "uv_sync_offline" {
            self.supervisor.review_protected_package_evidence(
                &prepared.job_id,
                project_id,
                &prepared.plan_sha256,
                &head,
            )
        } else {
            self.supervisor.review_build_test_evidence(
                &prepared.job_id,
                project_id,
                &prepared.plan_sha256,
                &head,
            )
        }
        .map_err(|e| e.code().to_owned())?;
        #[cfg(not(feature = "native-package-uv"))]
        let evidence = self
            .supervisor
            .review_build_test_evidence(&prepared.job_id, project_id, &prepared.plan_sha256, &head)
            .map_err(|e| e.code().to_owned())?;
        #[cfg(windows)]
        let native_error_category: Option<String> = None;
        #[cfg(target_os = "linux")]
        let native_error_category = Some(native.stderr_category.to_owned());
        #[cfg(windows)]
        let (exit_code, output_sha256, output_bytes) = (
            native.exit_code.map(i64::from),
            if native.output_sha256.is_empty() {
                None
            } else {
                Some(native.output_sha256)
            },
            Some(native.output_bytes),
        );
        #[cfg(target_os = "linux")]
        let (exit_code, output_sha256, output_bytes) = (
            native.exit_code.map(i64::from),
            // Linux captures independent stdout/stderr hashes; a combined
            // artifact digest requires a separately reviewed output audit.
            None,
            None,
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
        // IDE-05.4: a separate local-artifact audit is performed only
        // AFTER the original supervisor has a terminal receipt and its OS
        // process tree has been verified gone. A signed-off wheel origin
        // is intentionally NOT inferred from metadata/lock declarations.
        #[cfg(feature = "native-package-uv")]
        let (package_output_audit, package_audit_state, postrun_original_manifest_lock_unchanged) =
            if recipe_id == "uv_sync_offline" {
                let reviewed = native_package_review.as_ref();
                let lock_sha = reviewed
                    .and_then(|review| {
                        review
                            .evidence
                            .iter()
                            .find(|row| row.relative_path == "uv.lock")
                    })
                    .and_then(|row| row.sha256.as_deref());
                #[cfg(windows)]
                let package_output_root = &native.output_root;
                #[cfg(target_os = "linux")]
                let package_output_root = &native.output_directory;
                let original_manifests_unchanged = reviewed.is_some_and(|value| {
                    registry
                        .revalidate_package_operation(
                            project_id,
                            value.request.clone(),
                            &value.preview_sha256,
                            &value.evidence_fingerprint,
                        )
                        .is_ok()
                }) && verify_input(&input).is_ok();
                let trusted_terminal = native.summary.phase == JobPhase::Completed
                    && !evidence.requires_reconciliation
                    && exit_code == Some(0)
                    && published.is_some();
                let audit = if trusted_terminal && original_manifests_unchanged {
                    lock_sha
                        .and_then(|lock| {
                            audit_uv_private_output(
                                package_output_root,
                                &input.root,
                                &input.seal.manifest_sha256,
                                lock,
                            )
                            .ok()
                        })
                        .filter(|proof| {
                            published.as_ref().is_some_and(|output| {
                                output.tree_sha256 == proof.output_tree_sha256
                            })
                        })
                } else {
                    None
                };
                let lifecycle_tree = published
                    .as_ref()
                    .map(|value| value.tree_sha256.as_str())
                    .unwrap_or("0000000000000000000000000000000000000000000000000000000000000000");
                // The append-only audit event is fsynced before returning a
                // successful host result. A corrupt or unavailable journal
                // can never silently certify private package contents.
                self.package_lifecycle
                    .record_audit(
                        project_id,
                        &prepared.job_id,
                        &prepared.plan_sha256,
                        &evidence.terminal_receipt_hash,
                        lifecycle_tree,
                        audit.as_ref().map(|proof| proof.digest_sha256.as_str()),
                    )
                    .map_err(|e| e.code().to_owned())?;
                let state = if audit.is_some() {
                    "verified_local_bytes_origin_unproven_private_only"
                } else {
                    "private_output_blocked_no_promotion"
                };
                (audit, state, original_manifests_unchanged)
            } else {
                (None, "not_a_package_job", false)
            };
        // DEV-04.2: host-only exact worker output root, never a WebView path.
        // Post-completion candidate requires native zero exit, unquarantined
        // exact plan receipt and the independently hashed output witness.
        let completed_build = recipe_id == "rust_build"
            && native.summary.phase == JobPhase::Completed
            && !evidence.requires_reconciliation
            && exit_code == Some(0);
        #[cfg(windows)]
        let debug_binary_candidate = if completed_build {
            published.as_ref().and_then(|witness| {
                audit_debug_binary_candidate(&native.output_root, witness)
                    .ok()
                    .flatten()
            })
        } else {
            None
        };
        #[cfg(target_os = "linux")]
        let debug_binary_candidate = if completed_build {
            published.as_ref().and_then(|witness| {
                audit_debug_binary_candidate(&native.output_directory, witness)
                    .ok()
                    .flatten()
            })
        } else {
            None
        };
        // DEV-04.3: native-only bounded readback of *the same* protected
        // completed Cargo output and its independently rehashed executable.
        // No symbol path, PDB path, executable identity or approval is taken
        // from the WebView. Missing/invalid/ambiguous debug data is None.
        #[cfg(windows)]
        let debug_symbol_witness = if completed_build {
            published
                .as_ref()
                .zip(debug_binary_candidate.as_ref())
                .and_then(|(output, binary)| {
                    verify_build_debug_symbols(&native.output_root, output, binary)
                        .ok()
                        .flatten()
                })
        } else {
            None
        };
        #[cfg(target_os = "linux")]
        let debug_symbol_witness = if completed_build {
            published
                .as_ref()
                .zip(debug_binary_candidate.as_ref())
                .and_then(|(output, binary)| {
                    verify_build_debug_symbols(&native.output_directory, output, binary)
                        .ok()
                        .flatten()
                })
        } else {
            None
        };
        let binding_lineage = CompletedBuildLineage {
            project_id: project_id.to_owned(),
            build_job_id: prepared.job_id.clone(),
            build_plan_sha256: prepared.plan_sha256.clone(),
            terminal_receipt_sha256: evidence.terminal_receipt_hash.clone(),
            source_head: head.clone(),
            source_snapshot_sha256: plan.input_snapshot_sha256.clone(),
            manifest_sha256: plan.manifest_sha256.clone(),
            build_toolchain_sha256: plan.toolchain_sha256.clone(),
            output_tree_sha256: published
                .as_ref()
                .map(|w| w.tree_sha256.clone())
                .unwrap_or_default(),
            build_successfully_completed: completed_build,
            requires_reconciliation: evidence.requires_reconciliation,
        };
        #[cfg(windows)]
        let native_artifacts_root = &native.output_root;
        #[cfg(target_os = "linux")]
        let native_artifacts_root = &native.output_directory;
        let private_debug_target = if completed_build {
            debug_binary_candidate
                .as_ref()
                .zip(debug_symbol_witness.as_ref())
                .and_then(|(binary, symbols)| {
                    seal_project_debug_target(
                        native_artifacts_root,
                        &self.debug_targets,
                        binding_lineage,
                        binary,
                        symbols,
                    )
                    .ok()
                })
                .map(Arc::new)
        } else {
            None
        };
        self.store
            .lock()
            .map_err(|_| "BUILD_HOST_UNAVAILABLE")?
            .debug_target = private_debug_target;
        let test_case_evidence = if matches!(
            recipe_id, "rust_tests" | "lu_ide_host_tests" |
                "python_pytest" | "js_tests" | "cpp_cmake_build"
        ) || lu_ide_execution_supervisor::rust_exact_case_name(recipe_id).is_some() {
            // The ORIGINAL native worker returned its terminal result. Output
            // is still project-controlled, so these are reported observations.
            let native_terminated = matches!(
                native.summary.phase, JobPhase::Completed | JobPhase::Failed
            ) && !evidence.requires_reconciliation &&
                exit_code.is_some() && !native.timed_out;
            Some(reported_cases(
                registry, &input, &output.bounded_native_snapshot(),
                project_id, &prepared.job_id, recipe_id,
                &prepared.plan_sha256, &evidence.terminal_receipt_hash,
                &head, &plan.toolchain_sha256, native_terminated,
            ))
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
            phase: native_phase_label(native.summary.phase).to_owned(),
            native_exit_code: exit_code,
            native_error_category,
            output_sha256,
            output_bytes,
            terminal_receipt_sha256: evidence.terminal_receipt_hash,
            debug_binary_candidate,
            debug_symbol_witness,
            artifact_manifest_sha256,
            artifact_evidence_state: artifact_evidence_state.to_owned(),
            artifact_files,
            artifact_count,
            artifact_truncated,
            test_case_evidence,
            timed_out: native.timed_out,
            requires_reconciliation: evidence.requires_reconciliation,
            // A native worker reply is NOT independently installed-app
            // acceptance, especially on a WO4/WO5 stacked unaccepted branch.
            installed_app_accepted: false,
            #[cfg(feature = "native-package-uv")]
            package_output_audit,
            #[cfg(feature = "native-package-uv")]
            package_audit_state,
            #[cfg(feature = "native-package-uv")]
            postrun_original_manifest_lock_unchanged,
        })
    }
    /// IDE-05.4: rollback means destroying ONLY the private, quarantinable
    /// package output, never changing the original project. Both a terminal
    /// D3 OS receipt and a durable local audit record must match the exact
    /// Job/plan. A crash after discard_prepared leaves pending cleanup.
    /// Reopening never retries work or erases files automatically.
    #[cfg(feature = "native-package-uv")]
    pub fn discard_package_private_output(
        &self,
        app: &AppHandle,
        project_id: &str,
        job_id: &str,
    ) -> Result<String, String> {
        if self
            .store
            .lock()
            .map_err(|_| "IDE054_HOST_UNAVAILABLE")?
            .current
            .is_some()
        {
            return Err("IDE054_JOB_ACTIVE_CANNOT_DISCARD".into());
        }
        if job_id.len() != 36
            || !job_id.starts_with("job-")
            || !job_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("IDE054_JOB_ID_INVALID".into());
        }
        let statuses = self
            .package_lifecycle
            .statuses()
            .map_err(|e| e.code().to_owned())?;
        let previous = statuses
            .iter()
            .find(|row| row.job_id == job_id && row.project_id == project_id)
            .ok_or("IDE054_PACKAGE_RECEIPT_MISSING")?;
        if !matches!(
            previous.state,
            "verified_local_bytes_private_not_promoted"
                | "original_package_job_requires_native_reconciliation"
                | "private_output_cleanup_pending"
        ) {
            return Err("IDE054_CLEANUP_NOT_VERIFIABLE".into());
        }
        let expected_plan = previous.plan_sha256.clone();
        let verify_terminal = || -> Result<(), String> {
            let receipt = self
                .supervisor
                .inspect()
                .map_err(|e| e.code().to_owned())?
                .into_iter()
                .find(|job| job.job_id == job_id)
                .ok_or("IDE054_ORIGINAL_SUPERVISOR_JOB_MISSING")?;
            if receipt.project_id != project_id
                || receipt.plan_sha256 != expected_plan
                || receipt.requires_reconciliation
                || !(if previous.audit_sha256.is_some() {
                    receipt.phase == JobPhase::Completed
                } else {
                    matches!(
                        receipt.phase,
                        JobPhase::Completed | JobPhase::Failed | JobPhase::Cancelled
                    )
                })
            {
                return Err("IDE054_OS_CLEANUP_NOT_VERIFIED".into());
            }
            Ok(())
        };
        verify_terminal()?;
        let confirmed = app
            .dialog()
            .message(format!(
                "Permanently discard ONLY the private offline uv environment \
                for job {job_id}?\n\nOriginal project: {project_id}\n\
                Plan SHA-256: {expected_plan}\n\
                This does not remove or install packages in the project. \
                No package manager is relaunched.\n\
                A durable cleanup-intent record is written first; incomplete \
                removal remains pending after restart."
            ))
            .title("Discard one protected package output")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Discard this private output".into(),
                "Leave private output".into(),
            ))
            .blocking_show();
        if !confirmed {
            return Err("IDE054_PRIVATE_DISCARD_DECLINED".into());
        }
        verify_terminal()?;
        let fresh = self
            .package_lifecycle
            .statuses()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .find(|row| {
                row.job_id == job_id
                    && row.project_id == project_id
                    && row.plan_sha256 == expected_plan
            })
            .ok_or("IDE054_PRIVATE_DISCARD_RECEIPT_CHANGED")?;
        let pending = fresh.state == "private_output_cleanup_pending";
        let unresolved = fresh.state == "original_package_job_requires_native_reconciliation";
        if !pending && !unresolved && fresh.state != "verified_local_bytes_private_not_promoted" {
            return Err("IDE054_PRIVATE_DISCARD_TRANSITION_DENIED".into());
        }
        // Do not canonicalize an attacker-swapped output-parent symlink
        // and then accidentally delete under its external destination.
        let original_parent = fs::symlink_metadata(&self.outputs)
            .map_err(|_| "IDE054_PRIVATE_OUTPUT_PARENT_UNAVAILABLE")?;
        if !original_parent.is_dir() || original_parent.file_type().is_symlink() {
            return Err("IDE054_PRIVATE_OUTPUT_PARENT_UNSAFE".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if original_parent.file_attributes() & 0x400 != 0 {
                return Err("IDE054_PRIVATE_OUTPUT_PARENT_UNSAFE".into());
            }
        }
        let parent = fs::canonicalize(&self.outputs)
            .map_err(|_| "IDE054_PRIVATE_OUTPUT_PARENT_UNAVAILABLE")?;
        let meta = fs::symlink_metadata(&parent)
            .map_err(|_| "IDE054_PRIVATE_OUTPUT_PARENT_UNAVAILABLE")?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("IDE054_PRIVATE_OUTPUT_PARENT_UNSAFE".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err("IDE054_PRIVATE_OUTPUT_PARENT_UNSAFE".into());
            }
        }
        let output = parent.join(job_id);
        let present = match fs::symlink_metadata(&output) {
            Ok(meta) => {
                if !meta.is_dir() || meta.file_type().is_symlink() {
                    return Err("IDE054_PRIVATE_OUTPUT_NOT_DIRECTORY".into());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err("IDE054_PRIVATE_OUTPUT_REPARSE_DENIED".into());
                    }
                }
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => return Err("IDE054_PRIVATE_OUTPUT_UNREADABLE".into()),
        };
        if !present && !pending && !unresolved {
            return Err("IDE054_OUTPUT_MISSING_BEFORE_PREPARE".into());
        }
        let mut observed_tree =
            "0000000000000000000000000000000000000000000000000000000000000000".to_owned();
        if present {
            let canonical =
                fs::canonicalize(&output).map_err(|_| "IDE054_PRIVATE_OUTPUT_UNREADABLE")?;
            if canonical.parent() != Some(parent.as_path()) {
                return Err("IDE054_PRIVATE_OUTPUT_SCOPE_MISMATCH".into());
            }
            let manifest = audit_published_output(&output)
                .map_err(|_| "IDE054_UNSAFE_PRIVATE_TREE_NOT_REMOVED")?;
            observed_tree = manifest.tree_sha256;
            if !unresolved && observed_tree != fresh.output_tree_sha256 {
                return Err("IDE054_OUTPUT_CHANGED_NOT_REMOVED".into());
            }
        }
        if unresolved {
            // A host crash before postrun audit cannot promote the output.
            // It MAY be cleaned after independent D3 terminal verification,
            // a fresh human consent, and bounded native private-tree scan.
            let history = self
                .supervisor
                .receipt_history(job_id)
                .map_err(|e| e.code().to_owned())?;
            let terminal = history.last().ok_or("IDE054_D3_TERMINAL_MISSING")?;
            if terminal.body.project_id != project_id
                || terminal.body.plan_sha256 != expected_plan
                || !matches!(
                    terminal.body.phase,
                    JobPhase::Completed | JobPhase::Failed | JobPhase::Cancelled
                )
            {
                return Err("IDE054_D3_TERMINAL_NOT_VERIFIED".into());
            }
            self.package_lifecycle
                .prepare_unverified_discard(&fresh, &terminal.entry_hash, &observed_tree)
                .map_err(|e| e.code().to_owned())?;
        } else if !pending {
            self.package_lifecycle
                .prepare_discard(&fresh)
                .map_err(|e| e.code().to_owned())?;
        }
        // If a crash occurs here, a new human-approved attempt checks the
        // *same* OS job, journal and remaining private directory. A partially
        // removed directory with changed hash remains blocked: no success
        // claim and no untrusted recursive traversal workaround.
        if present {
            fs::remove_dir_all(&output).map_err(|_| "IDE054_REMOVE_FAILED_CLEANUP_PENDING")?;
        }
        if fs::symlink_metadata(&output).is_ok() {
            return Err("IDE054_OUTPUT_STILL_PRESENT_CLEANUP_PENDING".into());
        }
        #[cfg(unix)]
        {
            File::open(&parent)
                .and_then(|file| file.sync_all())
                .map_err(|_| "IDE054_PARENT_FSYNC_FAILED_CLEANUP_PENDING")?;
        }
        let pending_identity = self
            .package_lifecycle
            .statuses()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .find(|row| {
                row.job_id == job_id
                    && row.project_id == project_id
                    && row.state == "private_output_cleanup_pending"
            })
            .ok_or("IDE054_PENDING_DISCARD_RECEIPT_MISSING")?;
        self.package_lifecycle
            .verify_discard(&pending_identity)
            .map_err(|e| e.code().to_owned())?;
        Ok("IDE054_PRIVATE_PACKAGE_OUTPUT_DISCARDED_NO_PROJECT_MUTATION".into())
    }

    /// Read-only review of one ORIGINAL current-process native test-case
    /// source candidate. The WebView supplies only an opaque case ID; never
    /// the reported path or source hash. Native verifies exact journal and
    /// current registered saved source before returning a navigation hint.
    pub fn review_test_case_target(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        job_id: &str,
        plan_sha256: &str,
        case_id: &str,
    ) -> Result<TestTargetReview, String> {
        if !job_id.starts_with("job-") || job_id.len()!=36 ||
           !job_id[4..].bytes().all(|b| b.is_ascii_hexdigit()) ||
           plan_sha256.len()!=64 ||
           !plan_sha256.bytes().all(|b| b.is_ascii_hexdigit()) ||
           !case_id.starts_with("case_") || case_id.len()!=69 ||
           !case_id[5..].bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("INT03E_TARGET_IDENTITY_DENIED".into());
        }
        // There is deliberately NO path in this request. The only source
        // identity comes from the original receipt-linked in-memory report.
        let locked=self.store.lock().map_err(|_| "INT03E_JOB_STATE_UNAVAILABLE")?;
        let observed=locked.recent.get(job_id)
            .cloned().ok_or("INT03E_ORIGINAL_CURRENT_PROCESS_RESULT_MISSING")?;
        drop(locked); // Native journal and disk rechecks cannot hold the UI job lock.
        if observed.project_id!=project_id || observed.plan_sha256!=plan_sha256 ||
           observed.requires_reconciliation ||
           !matches!(observed.phase.as_str(),"completed"|"failed") {
            return Err("INT03E_JOB_NOT_RECONCILED_OR_SCOPE_CHANGED".into());
        }
        let report=observed.test_case_evidence.as_ref()
            .ok_or("INT03E_NO_STRUCTURED_TEST_CASES")?;
        if report.project_id!=project_id ||
           report.job_id!=job_id || report.plan_sha256!=plan_sha256 ||
           report.terminal_receipt_sha256!=observed.terminal_receipt_sha256 ||
           report.input_snapshot_sha256!=observed.input_snapshot_sha256 ||
           report.toolchain_sha256!=observed.toolchain_sha256 ||
           report.manifest_sha256!=observed.manifest_sha256 ||
           report.evidence_state!="native_terminal_framework_reported_not_assertion_verified" {
            return Err("INT03E_ORIGINAL_RECEIPT_SCOPE_DENIED".into());
        }
        let case=report.cases.iter().find(|row|row.case_id==case_id)
            .ok_or("INT03E_CASE_ID_NOT_RECORDED")?;
        if case.status!="reported_fail" {
            return Err("INT03E_ONLY_FAILING_CASES_HAVE_TARGETS".into());
        }
        let source=case.source.as_ref().ok_or("INT03E_NO_SOURCE_ANCHOR")?;
        let durable=self.supervisor.inspect().map_err(|e|e.code().to_owned())?
            .into_iter().find(|row|row.job_id==job_id)
            .ok_or("INT03E_ORIGINAL_SUPERVISOR_JOB_MISSING")?;
        if durable.project_id!=project_id ||
           durable.plan_sha256!=plan_sha256 ||
           durable.requires_reconciliation ||
           !matches!(durable.phase,JobPhase::Completed|JobPhase::Failed) {
            return Err("INT03E_DURABLE_NATIVE_JOB_CHANGED".into());
        }
        let history=self.supervisor.receipt_history(job_id)
            .map_err(|e|e.code().to_owned())?;
        let last=history.last().ok_or("INT03E_TERMINAL_RECEIPT_MISSING")?;
        if last.entry_hash!=report.terminal_receipt_sha256 ||
           last.body.project_id!=project_id ||
           last.body.plan_sha256!=plan_sha256 ||
           !matches!(last.body.phase,JobPhase::Completed|JobPhase::Failed) {
            return Err("INT03E_TERMINAL_RECEIPT_MISMATCH".into());
        }
        // Verify an exact native-registered saved-file digest again *now*.
        // If disk drifted or registry switched projects, no navigation.
        let saved=registry.preview_source_file(project_id,&source.relative_path)
            .map_err(|_| "INT03E_SAVED_SOURCE_NOT_AVAILABLE")?;
        if saved.project_id!=project_id ||
           saved.relative_path!=source.relative_path ||
           saved.revision!=source.disk_sha256 {
            return Err("INT03E_SAVED_SOURCE_CHANGED".into());
        }
        if source.line==0 || source.line as usize > saved.text.lines().count().max(1) {
            return Err("INT03E_SOURCE_POSITION_STALE".into());
        }
        Ok(TestTargetReview {
            project_id:project_id.to_owned(),job_id:job_id.to_owned(),
            plan_sha256:plan_sha256.to_owned(),
            terminal_receipt_sha256:report.terminal_receipt_sha256.clone(),
            case_id:case_id.to_owned(),relative_path:source.relative_path.clone(),
            disk_sha256:source.disk_sha256.clone(),line:source.line,
            character_utf16:source.character_utf16,
            saved_source_verified:true,
            test_assertion_independently_verified:false,task_accepted:false,
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
        if self
            .store
            .lock()
            .map_err(|_| "BUILD_HOST_UNAVAILABLE")?
            .current
            .is_some()
        {
            return Err("BUILD_JOB_ACTIVE".into());
        }
        let entry = self
            .supervisor
            .inspect()
            .map_err(|e| e.code().to_owned())?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or("BUILD_JOB_NOT_FOUND")?;
        if entry.project_id != project_id || !entry.requires_reconciliation {
            return Err("BUILD_RECOVERY_SCOPE_DENIED".into());
        }
        // Recovery must not depend on Cargo being installed. The Windows
        // broker is the SAME original independently pinned SCM service for
        // all fixed native recipes; the native installer is still measured
        // against its reviewed binary hash. Linux uses original cgroup
        // custody and does not require any executable toolchain at cleanup.
        #[cfg(windows)]
        let install = installation(app).or_else(|_| {
            #[cfg(feature = "native-multilang")]
            {
                ["python_pytest", "js_tests", "cpp_cmake_build"]
                    .iter()
                    .find_map(|&recipe| installation_multilang(app, recipe).ok())
                    .ok_or_else(|| "INT03_PINNED_RECOVERY_BROKER_UNAVAILABLE".into())
            }
            #[cfg(not(feature = "native-multilang"))]
            {
                Err("INT03_PINNED_RECOVERY_BROKER_UNAVAILABLE".into())
            }
        })?;
        let choice = app
            .dialog()
            .message(format!(
                "Attempt OS-only cleanup of interrupted job {job_id}?\n\n\
                This will not rebuild or restart the project. It only attempts \
                to kill the exact kernel-controlled process tree and journal \
                a verified reconciliation receipt."
            ))
            .title("Reconcile interrupted protected job")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Verify and clean up".into(),
                "Leave quarantined".into(),
            ))
            .blocking_show();
        if !choice {
            return Err("BUILD_RECOVERY_NOT_APPROVED".into());
        }
        #[cfg(windows)]
        let summary =
            reconcile_windows_interrupted_via_service(&self.supervisor, job_id, &install.broker)
                .map_err(|e| e.code().to_owned())?;
        #[cfg(target_os = "linux")]
        let summary = reconcile_linux_interrupted(&self.supervisor, job_id)
            .map_err(|e| e.code().to_owned())?;
        if summary.phase != JobPhase::Cancelled || summary.requires_reconciliation {
            return Err("BUILD_RECOVERY_REMAINS_QUARANTINED".into());
        }
        Ok("BUILD_KERNEL_TREE_RECONCILED_NO_REEXECUTION".into())
    }
}

#[cfg(test)]
mod dev035_native_lifecycle_tests {
    use super::*;

    #[test]
    fn ide053_package_worker_requires_separate_fixed_entrypoint() {
        assert!(MULTILANG_RECIPES.contains(&"uv_sync_offline"));
        assert!(!VALID_RECIPES.contains(&"uv_sync_offline"));
        assert!(VALID_RECIPES.contains(&"rust_check"));
        // Generic ide_build_start still cannot reach the new recipe:
        // run() explicitly refuses uv_sync_offline.
    }
    #[test]
    fn int03_run_profiles_never_accept_webview_executable_or_package_sync() {
        for (recipe, manifest) in [
            ("rust_check", "Cargo.toml"),
            ("rust_build", "Cargo.toml"),
            ("rust_tests", "Cargo.toml"),
            ("lu_ide_host_tests", "ide/host/Cargo.toml"),
            ("python_pytest", "pyproject.toml"),
            ("js_tests", "package.json"),
            ("cpp_cmake_build", "CMakeLists.txt"),
        ] {
            assert_eq!(profile_manifest(recipe), Some(manifest));
        }
        for hostile in [
            "uv_sync_offline",
            "../Cargo.toml",
            "sh -c echo bad",
            "npm test",
            "pytest -q",
            "python_pytest;id",
            "",
        ] {
            assert_eq!(profile_manifest(hostile), None);
        }
    }
    #[test]
    fn provenance_never_promotes_an_unverified_bundle_or_publisher() {
        let digest = "a".repeat(64);
        let valid = runtime_provenance("rust_cargo", Some(digest.clone()),Some(&digest));
        assert_eq!(valid.provenance_state,
            "installed_bytes_match_compile_time_release_pin");
        assert_eq!(valid.observed_sha256.as_deref(),Some(digest.as_str()));
        assert_eq!(valid.license_evidence,"not_independently_verified");
        assert!(!valid.execution_authorized);
        let forged = runtime_provenance("rust_cargo",Some("b".repeat(64)),
            Some(&digest));
        assert!(forged.observed_sha256.is_none());
        assert_eq!(forged.provenance_state,
            "unavailable_or_not_independently_verified");
        let unpinned = runtime_provenance("python_pytest",Some(digest),None);
        assert!(unpinned.observed_sha256.is_none());
    }
    #[test]
    fn cancellation_and_recovery_phase_names_match_durable_supervisor_schema() {
        assert_eq!(
            native_phase_label(JobPhase::CancelRequested),
            "cancel_requested"
        );
        assert_eq!(
            native_phase_label(JobPhase::ReleasePending),
            "release_pending"
        );
        assert_eq!(
            native_phase_label(JobPhase::RecoveryRequired),
            "recovery_required"
        );
        assert_eq!(native_phase_label(JobPhase::Cancelled), "cancelled");
        assert_eq!(native_phase_label(JobPhase::Completed), "completed");
    }
}
