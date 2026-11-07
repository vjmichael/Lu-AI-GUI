//! Standalone IDE desktop. All project directories originate in the OS picker,
//! not from JavaScript, model output, or a Lu IPC call.
#![forbid(unsafe_code)]
#[cfg(all(windows, feature = "native-lsp"))]
mod lsp_backend;
#[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
mod build_backend;
#[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
mod debug_backend;
use std::{path::PathBuf, sync::Arc};

use lu_ide_workspace::{execution_intents::ExecutionIntents, continuity::DevelopmentLedger, inventory_history::InventorySnapshots, inbox::{ApprovedStaging, IdeProposalInbox, ProposalDetail, ProposalSummary}, IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

struct NativePicker(AppHandle);
impl NativeDirectoryPicker for NativePicker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        self.0
            .dialog()
            .file()
            .blocking_pick_folder()
            .map(|folder| folder.into_path().map_err(|_| IdeError::InvalidSelection))
            .transpose()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary {
    project_id: String,
    display_name: String,
    active: bool,
    available: bool,
}

impl From<lu_ide_workspace::wire::proto::ProjectView> for ProjectSummary {
    fn from(project: lu_ide_workspace::wire::proto::ProjectView) -> Self {
        Self {
            project_id: project.project_id,
            display_name: project.display_name,
            active: project.active,
            available: project.available,
        }
    }
}

fn summarize(registry: &IdeWorkspaceRegistry) -> Result<Vec<ProjectSummary>, String> {
    registry
        .projects()
        .map(|list| list.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Project registry unavailable: {}", stable_error(&error)))
}

// Continuity records are best-effort operational breadcrumbs, not security
// receipts. A failed ledger append never turns a successful file save into a
// retryable failed write (which could lead to duplicate effects).
fn observe(ledger: &DevelopmentLedger, project_id: &str, operation: &str,
    summary: &str, revision: Option<&str>, source_path: Option<&str>) {
    if let Err(err) = ledger.record(project_id, operation, summary, revision, source_path) {
        eprintln!("IDE continuity observation unavailable: {}", err.code());
    }
}

fn stable_error(error: &IdeError) -> &'static str {
    match error {
        IdeError::Io(_) => "IO_FAILURE",
        IdeError::Format(_) | IdeError::InvalidCatalog => "CATALOG_INVALID",
        IdeError::Poisoned => "HOST_UNAVAILABLE",
        IdeError::InvalidSelection => "INVALID_DIRECTORY",
        IdeError::UnknownProject => "PROJECT_NOT_FOUND",
        IdeError::Capacity => "PROJECT_CAPACITY",
        IdeError::AlreadyActive => "PROJECT_ALREADY_OPEN",
        IdeError::NotActive => "PROJECT_NOT_OPEN",
    }
}

/// WO9-A/B: inspect several explicitly registered projects in one native,
/// bounded, non-executing release workflow. No paths or commands from WebView.
#[tauri::command]
async fn ide_workflow_preview(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_ids: Vec<String>,
) -> Result<lu_ide_workspace::advanced_workflows::WorkflowInspection, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_multi_project_workflow(&project_ids).map_err(str::to_owned)
    }).await.map_err(|_| "WORKFLOW_NATIVE_TASK_UNAVAILABLE".to_owned())?
}

/// WO9-B: reject stale Git HEAD, dirty worktree or changed staged artifacts.
/// Verifying a plan does not approve packaging or request OS-worker admission.
#[tauri::command]
async fn ide_workflow_verify(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_ids: Vec<String>,
    expected_fingerprint: String,
) -> Result<lu_ide_workspace::advanced_workflows::WorkflowInspection, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.verify_multi_project_workflow(&project_ids, &expected_fingerprint)
            .map_err(str::to_owned)
    }).await.map_err(|_| "WORKFLOW_REVALIDATE_TASK_UNAVAILABLE".to_owned())?
}

/// Explicit human-triggered export of a non-executing, immutable manifest
/// to a fresh OS-picker-selected folder, never a JavaScript-selected path.
#[tauri::command]
async fn ide_workflow_export_manifest(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_ids: Vec<String>,
    expected_fingerprint: String,
) -> Result<lu_ide_workspace::advanced_workflows::WorkflowManifestExport, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let folder = NativePicker(app).choose_project_directory()
            .map_err(|_| "WORKFLOW_EXPORT_PICKER_UNAVAILABLE")?
            .ok_or("WORKFLOW_EXPORT_CANCELLED")?;
        registry.export_workflow_manifest(&project_ids, &expected_fingerprint, &folder)
            .map_err(str::to_owned)
    }).await.map_err(|_| "WORKFLOW_EXPORT_HOST_TASK_UNAVAILABLE".to_owned())?
}

/// WO9-C: real portable ZIP of independently hashed predetermined project
/// artifacts only. Native OS folder chooser supplies the destination;
/// WebView cannot submit filesystem paths, commands, Git write or release flags.
#[tauri::command]
async fn ide_workflow_export_bundle(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_ids: Vec<String>,
    expected_fingerprint: String,
) -> Result<lu_ide_workspace::advanced_workflows::WorkflowBundleExport, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let selected = NativePicker(app).choose_project_directory()
            .map_err(|_| "WORKFLOW_BUNDLE_PICKER_UNAVAILABLE")?
            .ok_or("WORKFLOW_BUNDLE_CANCELLED")?;
        registry.export_workflow_bundle(&project_ids, &expected_fingerprint, &selected)
            .map_err(str::to_owned)
    }).await.map_err(|_| "WORKFLOW_BUNDLE_TASK_UNAVAILABLE".to_owned())?
}

/// Read file metadata and bounded SHA-256 under an active registered project.
/// The result NEVER proves an executable is safe or operable.
#[tauri::command]
async fn ide_probe_project_resource(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
) -> Result<lu_ide_workspace::resource_probe::ProjectResourceProbe, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.probe_project_resource(&project_id, &relative_path)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "RESOURCE_PROBE_TASK_UNAVAILABLE".to_owned())?
}

/// Read local Git diff hunks only, in the exact registered project root.
/// Scope is strictly "staged" or "worktree"; there are no mutation actions.
#[tauri::command]
async fn ide_read_git_diff(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    scope: String,
) -> Result<lu_ide_workspace::git_evidence::GitDiffEvidence, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_git_diff(&project_id, &scope)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "GIT_DIFF_TASK_UNAVAILABLE".to_owned())?
}

/// Read recent local commits. No remote fetch, arbitrary rev expressions,
/// hooks, shell commands or repository writes.
#[tauri::command]
async fn ide_read_git_history(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_evidence::GitHistoryEvidence, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_git_history(&project_id)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "GIT_HISTORY_TASK_UNAVAILABLE".to_owned())?
}

/// Read-only Git source control status of an already selected workspace.
/// No subprocess, model bridge, network operation, or repository mutation.
#[tauri::command]
async fn ide_read_git_status(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_status::GitWorktreeInspection, String> {
    let registry = Arc::clone(state.inner());
    let status = tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_git_worktree(&project_id)
    })
    .await
    .map_err(|_| "GIT_STATUS_UNAVAILABLE".to_owned())?;
    Ok(status)
}

/// On explicit human request, compare local work with current public GitHub
/// branch evidence. HTTPS to fixed api.github.com only; no token or Git fetch.
#[tauri::command]
async fn ide_compare_live_github(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::github_live::LiveGitHubComparison, String> {
    let registry = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.compare_live_github(&project_id)
    })
    .await
    .map_err(|_| "GITHUB_COMPARE_TASK_UNAVAILABLE".to_owned())?;
    Ok(result)
}

/// Explicit read-only public GitHub Actions receipts for the exact local HEAD.
#[tauri::command]
async fn ide_read_exact_head_ci(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::github_ci::ExactHeadCiEvidence, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || registry.inspect_exact_head_ci(&project_id))
        .await.map_err(|_| "GITHUB_CI_TASK_UNAVAILABLE".to_owned())
}

/// Bounded, source-revisioned continuity from the trusted local IDE profile.
#[tauri::command]
async fn ide_read_project_context(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    ledger: State<'_, Arc<DevelopmentLedger>>,
    project_id: String,
) -> Result<lu_ide_workspace::continuity::ProjectContext, String> {
    let registry = Arc::clone(registry.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        ledger.context(&registry, &project_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "CONTINUITY_TASK_UNAVAILABLE".to_owned())?
}

/// Explicitly capture read-only observed artifact and local reference bytes
/// into the IDE's private app-profile evidence history. No executable worker.
#[tauri::command]
async fn ide_capture_project_inventory(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    snapshots: State<'_, Arc<InventorySnapshots>>,
    project_id: String,
) -> Result<lu_ide_workspace::inventory_history::InventoryBaseline, String> {
    let registry = Arc::clone(registry.inner());
    let snapshots = Arc::clone(snapshots.inner());
    tauri::async_runtime::spawn_blocking(move || {
        snapshots.capture(&registry, &project_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "SNAPSHOT_TASK_UNAVAILABLE".to_owned())?
}

/// Compare a saved local baseline against a fresh bounded read-only scan.
#[tauri::command]
async fn ide_compare_project_inventory(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    snapshots: State<'_, Arc<InventorySnapshots>>,
    project_id: String,
) -> Result<lu_ide_workspace::inventory_history::InventoryComparison, String> {
    let registry = Arc::clone(registry.inner());
    let snapshots = Arc::clone(snapshots.inner());
    tauri::async_runtime::spawn_blocking(move || {
        snapshots.compare(&registry, &project_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "INVENTORY_COMPARE_UNAVAILABLE".to_owned())?
}

/// C6: user-authored task progress is an append-only note, not acceptance.
#[tauri::command]
async fn ide_record_task_update(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    ledger: State<'_, Arc<DevelopmentLedger>>,
    project_id: String,
    task_id: String,
    title: String,
    status: String,
    next_action: String,
    required_gates: Vec<String>,
) -> Result<lu_ide_workspace::task_evidence::TaskSummary, String> {
    let registry = Arc::clone(registry.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        ledger.record_task_update(&registry, &project_id, &task_id, &title,
            &status, &next_action, required_gates)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "TASK_UPDATE_UNAVAILABLE".to_owned())?
}

/// C6: native host independently checks exact-head public GitHub Actions and
/// appends the bounded observation to a specific task revision. The UI cannot
/// submit a verdict, commit SHA, or supposed CI receipt.
#[tauri::command]
async fn ide_observe_task_ci(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    ledger: State<'_, Arc<DevelopmentLedger>>,
    project_id: String,
    task_id: String,
) -> Result<lu_ide_workspace::task_evidence::TaskSummary, String> {
    let registry = Arc::clone(registry.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        ledger.record_task_ci(&registry, &project_id, &task_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "TASK_CI_UNAVAILABLE".to_owned())?
}

// WO6 native debugger fixture. The WebView cannot submit an executable,
// adapter path, PID, expression, breakpoint condition, arguments or token.
// All execution is restricted to the independently pinned self-test bundle.
#[tauri::command]
async fn ide_debug_status(
    app: AppHandle,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
    {
        let backend = app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        let backend = Arc::clone(backend.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            backend.status(&app, &project_id)
        }).await.map_err(|_| "DEBUG_STATUS_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "DEBUG_STATUS_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-debug", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id);
        Ok(serde_json::json!({
            "ready":false, "code":"DEBUG_PROTECTED_ADAPTER_NOT_INSTALLED",
            "description":"Default IDE does not ship a native debugger.",
            "current":null, "last":null, "journal":[], "quarantined":[]
        }))
    }
}
#[tauri::command]
async fn ide_debug_run_fixture(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
    {
        let backend = app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        let backend = Arc::clone(backend.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            backend.run_fixture(&app, &registry, &project_id)
        }).await.map_err(|_| "DEBUG_EXECUTION_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "DEBUG_RESULT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-debug", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, project_id);
        Err("DEBUG_PROTECTED_ADAPTER_NOT_INSTALLED".into())
    }
}
#[tauri::command]
async fn ide_debug_cancel(
    app: AppHandle,
    project_id: String,
    job_id: Option<String>,
) -> Result<String, String> {
    #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
    {
        let backend = app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        return backend.cancel(&project_id, job_id.as_deref()).map(str::to_owned);
    }
    #[cfg(not(all(feature = "native-debug", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id, job_id);
        Err("DEBUG_PROTECTED_ADAPTER_NOT_INSTALLED".into())
    }
}

#[tauri::command]
async fn ide_debug_reconcile(
    app: AppHandle,
    project_id: String,
    job_id: String,
) -> Result<String, String> {
    #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
    {
        let backend = app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        let backend = Arc::clone(backend.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            backend.reconcile(&app, &project_id, &job_id).map(str::to_owned)
        }).await.map_err(|_| "DEBUG_RECOVERY_TASK_UNAVAILABLE")?;
    }
    #[cfg(not(all(feature = "native-debug", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id, job_id);
        Err("DEBUG_PROTECTED_ADAPTER_NOT_INSTALLED".into())
    }
}

// WO6 native protected Cargo: explicit feature-only host authority.
// The default desktop exposes read-only "unavailable" status; none of these
// commands can run an unpinned executable, accept a path, grant token, source
// payload, shell flags or debugger command from the WebView.
#[tauri::command]
async fn ide_build_status(
    app: AppHandle,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            runtime.status(&app, &project_id)
        }).await.map_err(|_| "BUILD_STATUS_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "BUILD_STATUS_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id);
        Ok(serde_json::json!({
            "ready":false,
            "code":"BUILD_PROTECTED_WORKER_NOT_INSTALLED",
            "description":"Default IDE does not ship an authorized native build/test runtime.",
            "current":null, "recent":[], "journal":[], "quarantined":[]
        }))
    }
}
#[tauri::command]
async fn ide_build_start(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    recipe_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            runtime.run(&app, &registry, &project_id, &recipe_id)
        }).await.map_err(|_| "BUILD_EXECUTION_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "BUILD_RESULT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, project_id, recipe_id);
        Err("BUILD_PROTECTED_WORKER_NOT_INSTALLED".into())
    }
}
#[tauri::command]
async fn ide_build_cancel(
    app: AppHandle,
    project_id: String,
    job_id: Option<String>,
) -> Result<String, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        return runtime.cancel(&project_id, job_id.as_deref()).map(str::to_owned);
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id, job_id);
        Err("BUILD_PROTECTED_WORKER_NOT_INSTALLED".into())
    }
}
#[tauri::command]
async fn ide_build_reconcile(
    app: AppHandle,
    project_id: String,
    job_id: String,
) -> Result<String, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            runtime.reconcile(&app, &project_id, &job_id)
        }).await.map_err(|_| "BUILD_RECOVERY_TASK_UNAVAILABLE")?;
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id, job_id);
        Err("BUILD_PROTECTED_WORKER_NOT_INSTALLED".into())
    }
}

/// D1 records a bounded user-selected job *intent*, never a process launch.
#[tauri::command]
async fn ide_prepare_execution_intent(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    intents: State<'_, Arc<ExecutionIntents>>,
    project_id: String,
    recipe_id: String,
) -> Result<lu_ide_workspace::execution_intents::IntentSummary, String> {
    let registry = Arc::clone(registry.inner());
    let intents = Arc::clone(intents.inner());
    tauri::async_runtime::spawn_blocking(move ||
        intents.prepare(&registry, &project_id, &recipe_id)
            .map_err(|error| error.code().to_owned()))
        .await.map_err(|_| "EXECUTION_PREPARE_UNAVAILABLE".to_owned())?
}

/// Human review/cancellation is logged but never produces a worker grant.
#[tauri::command]
async fn ide_decide_execution_intent(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    intents: State<'_, Arc<ExecutionIntents>>,
    project_id: String,
    intent_id: String,
    decision: String,
) -> Result<lu_ide_workspace::execution_intents::IntentSummary, String> {
    let registry = Arc::clone(registry.inner());
    let intents = Arc::clone(intents.inner());
    tauri::async_runtime::spawn_blocking(move ||
        intents.decide(&registry, &project_id, &intent_id, &decision)
            .map_err(|error| error.code().to_owned()))
        .await.map_err(|_| "EXECUTION_REVIEW_UNAVAILABLE".to_owned())?
}

/// Project-scoped, read-only host journal; no job can be resumed from it.
#[tauri::command]
async fn ide_list_execution_intents(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    intents: State<'_, Arc<ExecutionIntents>>,
    project_id: String,
) -> Result<lu_ide_workspace::execution_intents::IntentList, String> {
    let registry = Arc::clone(registry.inner());
    let intents = Arc::clone(intents.inner());
    tauri::async_runtime::spawn_blocking(move ||
        intents.list(&registry, &project_id)
            .map_err(|error| error.code().to_owned()))
        .await.map_err(|_| "EXECUTION_LIST_UNAVAILABLE".to_owned())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LanguageWorkerStatus {
    ready: bool,
    code: &'static str,
    detail: &'static str,
    /// Only languages whose native executor AND exact release pins were verified.
    /// The general LSP catalog is not a runnable language list.
    supported_languages: Vec<String>,
}

/// Native admission is deliberately NOT represented by the presence of an
/// LSP binary, an open project, or a catalog entry. D7D must first prove
/// independent service and worker confinement in native Windows acceptance.
/// A future E2 host broker will replace this fail-closed status only after
/// independently attested process + permission evidence.
#[tauri::command]
async fn ide_lsp_worker_status(app: AppHandle) -> LanguageWorkerStatus {
    #[cfg(all(windows, feature = "native-lsp"))]
    {
        let installed = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>().is_some();
        let supported = tauri::async_runtime::spawn_blocking(move || {
            if !installed { return Vec::new(); }
            lsp_backend::installed_languages(&app)
        }).await.unwrap_or_default();
        if !supported.is_empty() {
            return LanguageWorkerStatus {
                ready: true, code: "LSP_REVIEWED_INSTALLATION_AVAILABLE",
                detail: "Pinned native language server available. Starting a session still requires human approval and protected worker admission.",
                supported_languages: supported,
            };
        }
    }
    #[cfg(not(all(windows, feature = "native-lsp")))]
    let _ = app;
    LanguageWorkerStatus {
        ready: false,
        code: "LSP_PROTECTED_WORKER_UNVERIFIED",
        detail: "Protected language servers are unavailable in this build or have unverified release evidence.",
        supported_languages: Vec::new(),
    }
}

/// E2 native feature commands are *always fail-closed by default*. A named
/// WebView operation never grants an executable right merely by existing.
/// Only the reviewed native-lsp build can execute, after an OS approval dialog.
#[tauri::command]
async fn ide_lsp_start_session(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, feature = "native-lsp"))]
    {
        let state = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let registry = Arc::clone(registry.inner());
        let typed = serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_| "LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move ||
            native.start(&app, &registry, typed)
        ).await.map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(windows, feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_change_document(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, feature = "native-lsp"))]
    {
        let state=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native=Arc::clone(state.inner());
        let registry=Arc::clone(registry.inner());
        let typed=serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_|"LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move ||
            native.change(&registry,typed)
        ).await.map_err(|_|"LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(windows, feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_feature(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, feature = "native-lsp"))]
    {
        let state=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native=Arc::clone(state.inner());
        let registry=Arc::clone(registry.inner());
        let typed=serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_|"LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move ||
            native.feature(&registry,typed)
        ).await.map_err(|_|"LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(windows, feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_read_diagnostics(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, feature = "native-lsp"))]
    {
        let state=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native=Arc::clone(state.inner());
        let registry=Arc::clone(registry.inner());
        let typed=serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_|"LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move ||
            native.diagnostics(&registry,typed)
        ).await.map_err(|_|"LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(windows, feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_stop_session(app: AppHandle) -> Result<(), String> {
    #[cfg(all(windows, feature = "native-lsp"))]
    {
        let state=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native=Arc::clone(state.inner());
        return tauri::async_runtime::spawn_blocking(move ||
            native.stop()
        ).await.map_err(|_|"LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(windows, feature = "native-lsp")))]
    {let _=app;Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}

/// Read-only descriptions of known free/open-source LSP servers.
/// No PATH search, installation, process spawn, worker grant or IPC occurs.
#[tauri::command]
async fn ide_list_language_servers() -> Vec<lu_ide_workspace::lsp::LanguageServerDescriptor> {
    lu_ide_workspace::lsp::available_servers().to_vec()
}

/// List actual host-supported operations. Discovery is not authorization.
#[tauri::command]
async fn ide_list_capabilities() -> lu_ide_workspace::capabilities::CapabilityCatalog {
    lu_ide_workspace::capabilities::list_capabilities()
}

#[tauri::command]
async fn ide_list_projects(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || summarize(&registry))
        .await
        .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

#[tauri::command]
async fn ide_choose_project(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    ledger: State<'_, Arc<DevelopmentLedger>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let selected = registry
            .register_from_native_picker(&mut NativePicker(app))
            .map_err(|error| stable_error(&error).to_string())?;
        if let Some(ref project) = selected {
            observe(&ledger, &project.project_id, "project.register",
                "Project registered through native picker", None, None);
        }
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_PICKER_UNAVAILABLE".to_string())?
}

#[tauri::command]
async fn ide_open_project(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    ledger: State<'_, Arc<DevelopmentLedger>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .reopen(&project_id)
            .map_err(|error| stable_error(&error).to_string())?;
        observe(&ledger, &project_id, "project.open", "Project reopened", None, None);
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

#[tauri::command]
async fn ide_close_project(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    ledger: State<'_, Arc<DevelopmentLedger>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .close(&project_id)
            .map_err(|error| stable_error(&error).to_string())?;
        observe(&ledger, &project_id, "project.close", "Project closed", None, None);
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

/// Delist a project from the standalone IDE. This modifies only the private
/// project catalog, never the selected directory or any of its files.
#[tauri::command]
async fn ide_unregister_project(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    ledger: State<'_, Arc<DevelopmentLedger>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .unregister(&project_id)
            .map_err(|error| stable_error(&error).to_string())?;
        observe(&ledger, &project_id, "project.unregister", "Project removed from IDE catalog", None, None);
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

/// These read-only requests require a registered, currently open project ID.
/// Paths are *relative within* a cap-std capability; never absolute WebView paths.
#[tauri::command]
async fn ide_list_source_directory(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_dir: String,
) -> Result<lu_ide_workspace::browser::DirectoryListing, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.list_source_directory(&project_id, &relative_dir)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "BROWSER_TASK_UNAVAILABLE".to_string())?
}

/// Read a precisely identified span from a previously observed saved revision.
/// No AI bridge, shell execution, file mutation or implicit authority.
#[tauri::command]
async fn ide_read_source_span(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
    expected_revision: String,
    first_line: usize,
    last_line: usize,
) -> Result<lu_ide_workspace::source_span::SourceSpan, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.read_source_span(&project_id, &relative_path,
            &expected_revision, first_line, last_line)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "SOURCE_SPAN_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_preview_source_file(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
) -> Result<lu_ide_workspace::browser::TextPreview, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.preview_source_file(&project_id, &relative_path)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "PREVIEW_TASK_UNAVAILABLE".to_string())?
}

/// Validate and preview a proposed source edit, without applying it.
/// Future Lu-proposed edits must use an independently authenticated bridge;
/// this desktop command does not grant write or execution authority.
#[tauri::command]
async fn ide_review_source_change(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
    expected_revision: String,
    proposed_text: String,
) -> Result<lu_ide_workspace::review::ChangeReview, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .review_source_change(&project_id, &relative_path, &expected_revision, &proposed_text)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "REVIEW_TASK_UNAVAILABLE".to_string())?
}

/// An explicit user-initiated save with optimistic file-revision protection.
/// There is no model-origin tool path or unrestricted filesystem write command.
#[tauri::command]
async fn ide_save_source_file(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
    expected_revision: String,
    text: String,
    ledger: State<'_, Arc<DevelopmentLedger>>,
) -> Result<lu_ide_workspace::browser::TextPreview, String> {
    let registry = Arc::clone(state.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let saved = registry
            .save_source_file(&project_id, &relative_path, &expected_revision, &text)
            .map_err(|error| error.code().to_owned())?;
        observe(&ledger, &project_id, "source.save",
            "Source saved with revision check", Some(&saved.revision), Some(&relative_path));
        Ok(saved)
    })
    .await
    .map_err(|_| "SAVE_TASK_UNAVAILABLE".to_owned())?
}

/// Data-only import to the IDE's volatile proposal inbox. No model bridge,
/// filesystem changes or execution authority are available to the payload.
#[tauri::command]
async fn ide_import_proposal(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    inbox: State<'_, Arc<IdeProposalInbox>>,
    proposal_json: String,
) -> Result<ProposalSummary, String> {
    let registry = Arc::clone(registry.inner());
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.import(&registry, &proposal_json).map_err(|error| error.code().to_string())
    }).await.map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_list_proposals(
    inbox: State<'_, Arc<IdeProposalInbox>>,
) -> Result<Vec<ProposalSummary>, String> {
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.list().map_err(|error| error.code().to_string())
    }).await.map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_review_proposal(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    inbox: State<'_, Arc<IdeProposalInbox>>,
    inbox_id: String,
) -> Result<ProposalDetail, String> {
    let registry = Arc::clone(registry.inner());
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.detail(&registry, &inbox_id).map_err(|error| error.code().to_string())
    }).await.map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

/// Records only an IDE-owned human decision. Approve-for-staging does not
/// apply edits, persist credentials, grant write rights, or run workers.
#[tauri::command]
async fn ide_decide_proposal(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    inbox: State<'_, Arc<IdeProposalInbox>>,
    inbox_id: String,
    expected_digest: String,
    approve_for_staging: bool,
) -> Result<ProposalSummary, String> {
    let registry = Arc::clone(registry.inner());
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.decide(&registry, &inbox_id, &expected_digest, approve_for_staging)
            .map_err(|error| error.code().to_string())
    }).await.map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

/// A reviewed proposal enters editor buffers only; never a native file save.
#[tauri::command]
async fn ide_stage_proposal(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    inbox: State<'_, Arc<IdeProposalInbox>>,
    inbox_id: String,
    expected_digest: String,
) -> Result<ApprovedStaging, String> {
    let registry = Arc::clone(registry.inner());
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.staging(&registry, &inbox_id, &expected_digest)
            .map_err(|error| error.code().to_string())
    }).await.map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_discard_proposal(
    inbox: State<'_, Arc<IdeProposalInbox>>,
    inbox_id: String,
) -> Result<Vec<ProposalSummary>, String> {
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.discard(&inbox_id).map_err(|error| error.code().to_string())?;
        inbox.list().map_err(|error| error.code().to_string())
    }).await.map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let registry = IdeWorkspaceRegistry::open(data.join("projects.json"))
                .map_err(|_| std::io::Error::other("standalone IDE registry failed to open"))?;
            let ledger = DevelopmentLedger::new(data.join("development-ledger.jsonl"))
                .map_err(|_| std::io::Error::other("standalone IDE ledger failed to open"))?;
            app.manage(Arc::new(registry));
            let snapshots = InventorySnapshots::new(data.join("evidence-baselines.json"))
                .map_err(|_| std::io::Error::other("IDE evidence history failed to open"))?;
            let intents = ExecutionIntents::new(data.join("execution-intents.jsonl"))
                .map_err(|_| std::io::Error::other("IDE execution-intent journal failed to open"))?;
            app.manage(Arc::new(intents));
            app.manage(Arc::new(ledger));
            app.manage(Arc::new(snapshots));
            app.manage(Arc::new(IdeProposalInbox::default()));
            #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
            match build_backend::NativeBuildRuntime::new(app.handle()) {
                Ok(native) => { app.manage(Arc::new(native)); },
                Err(code) => eprintln!("IDE protected build/test unavailable: {code}"),
            }
            #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
            match debug_backend::NativeDebugRuntime::new(app.handle()) {
                Ok(native) => { app.manage(Arc::new(native)); },
                Err(code) => eprintln!("IDE protected debugger unavailable: {code}"),
            }
            #[cfg(all(windows, feature = "native-lsp"))]
            match lsp_backend::NativeLspRuntime::new(app.handle()) {
                Ok(native) => {app.manage(Arc::new(native));},
                Err(code) => eprintln!("IDE LSP remains disabled: {code}"),
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ide_list_capabilities,
            ide_list_language_servers,
            ide_lsp_worker_status,
            ide_lsp_start_session,
            ide_lsp_change_document,
            ide_lsp_feature,
            ide_lsp_read_diagnostics,
            ide_lsp_stop_session,
            ide_prepare_execution_intent,
            ide_decide_execution_intent,
            ide_list_execution_intents,
            ide_build_status,
            ide_build_start,
            ide_build_cancel,
            ide_build_reconcile,
            ide_debug_status,
            ide_debug_run_fixture,
            ide_debug_cancel,
            ide_debug_reconcile,
            ide_read_project_context,
            ide_record_task_update,
            ide_observe_task_ci,
            ide_read_git_status,
            ide_read_git_diff,
            ide_probe_project_resource,
            ide_read_git_history,
            ide_compare_live_github,
            ide_read_exact_head_ci,
            ide_capture_project_inventory,
            ide_compare_project_inventory,
            ide_workflow_preview,
            ide_workflow_verify,
            ide_workflow_export_manifest,
            ide_workflow_export_bundle,
            ide_list_projects,
            ide_choose_project,
            ide_open_project,
            ide_close_project,
            ide_unregister_project,
            ide_list_source_directory,
            ide_preview_source_file,
            ide_read_source_span,
            ide_review_source_change,
            ide_save_source_file,
            ide_import_proposal,
            ide_list_proposals,
            ide_review_proposal,
            ide_decide_proposal,
            ide_stage_proposal,
            ide_discard_proposal
        ])
        .run(tauri::generate_context!())
        .expect("standalone Lu IDE failed to initialize");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_summary_exposes_no_host_path() {
        let entry = lu_ide_workspace::wire::proto::ProjectView {
            project_id: "ide-0123456789abcdef0123456789abcdef".into(),
            display_name: "example".into(),
            active: false,
            available: true,
        };
        let summary: ProjectSummary = entry.into();
        assert_eq!(summary.display_name, "example");
        assert_eq!(summary.project_id.len(), 36);
        // The public type has no PathBuf or path fields by construction.
    }
}
