//! Standalone IDE desktop. All project directories originate in the OS picker,
//! not from JavaScript, model output, or a Lu IPC call.
#![forbid(unsafe_code)]
#[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
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

/// WO9-E durable workflow creation from a native Git/artifact fingerprint.
#[tauri::command]
async fn ide_workflow_create(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    coordinator: State<'_, Arc<lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator>>,
    project_ids: Vec<String>,
    expected_fingerprint: String,
    steps: Vec<lu_ide_workspace::workflow_orchestrator::WorkflowStep>,
) -> Result<lu_ide_workspace::workflow_orchestrator::WorkflowRun, String> {
    let registry = Arc::clone(registry.inner());
    let coordinator = Arc::clone(coordinator.inner());
    tauri::async_runtime::spawn_blocking(move || {
        coordinator.create(&registry, &project_ids, &expected_fingerprint, steps)
            .map_err(str::to_owned)
    }).await.map_err(|_| "WO9_GRAPH_CREATE_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_workflow_list_runs(
    coordinator: State<'_, Arc<lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator>>,
) -> Result<Vec<lu_ide_workspace::workflow_orchestrator::WorkflowRun>, String> {
    coordinator.list().map_err(str::to_owned)
}

#[tauri::command]
async fn ide_workflow_cancel_pending(
    coordinator: State<'_, Arc<lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator>>,
    workflow_id: String,
    step: usize,
) -> Result<lu_ide_workspace::workflow_orchestrator::WorkflowRun, String> {
    coordinator.cancel_pending(&workflow_id, step).map_err(str::to_owned)
}

/// WO9-E restart recovery: revalidate a previously completed DAG step
/// against the protected WO6 journal. Read-only; never executes/replays code.
#[tauri::command]
async fn ide_workflow_verify_native_receipt(
    app: AppHandle,
    coordinator: State<'_, Arc<lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator>>,
    workflow_id: String,
    step: usize,
) -> Result<lu_ide_workspace::workflow_orchestrator::WorkflowRun, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("WO9_GRAPH_PROTECTED_JOURNAL_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        let coordinator = Arc::clone(coordinator.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            let original = coordinator.pending_native_recheck(&workflow_id, step)
                .map_err(str::to_owned)?;
            let job_id = original.native_job_id
                .ok_or("WO9_NATIVE_JOURNAL_JOB_UNAVAILABLE")?;
            let plan_sha = original.native_plan_sha256
                .ok_or("WO9_NATIVE_JOURNAL_PLAN_UNAVAILABLE")?;
            runtime.verify_replayed_completed_job(
                &original.spec.project_id, &job_id, &plan_sha)?;
            coordinator.accept_rechecked_native_pass(
                &workflow_id, step, &job_id, &plan_sha).map_err(str::to_owned)
        }).await.map_err(|_| "WO9_GRAPH_REPLAY_INSPECTION_TASK_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, coordinator, workflow_id, step);
        Err("WO9_GRAPH_PROTECTED_JOURNAL_UNAVAILABLE".into())
    }
}

/// This method does NOT implement a second worker. It invokes exactly the
/// existing WO6 native signed-resource supervisor (including its independent
/// human OS approval), and records only the resulting native receipt.
#[tauri::command]
async fn ide_workflow_run_step(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    coordinator: State<'_, Arc<lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator>>,
    workflow_id: String,
    step: usize,
) -> Result<lu_ide_workspace::workflow_orchestrator::WorkflowRun, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("WO9_GRAPH_PROTECTED_WORKER_NOT_INSTALLED")?;
        let runtime = Arc::clone(runtime.inner());
        let registry = Arc::clone(registry.inner());
        let coordinator = Arc::clone(coordinator.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            let spec = coordinator.begin_step(&registry, &workflow_id, step)
                .map_err(str::to_owned)?;
            let result = runtime.run(&app, &registry, &spec.project_id, &spec.recipe_id);
            match result {
                Ok(receipt) => {
                    coordinator.finish_step(&workflow_id, step,
                        Some(receipt.job_id), Some(receipt.plan_sha256),
                        receipt.output_sha256, &receipt.phase, receipt.native_exit_code,
                        receipt.requires_reconciliation || receipt.timed_out,
                        receipt.native_error_category).map_err(str::to_owned)
                }
                Err(code) => coordinator.finish_step(&workflow_id, step,
                    None, None, None, "uncertain", None, true, Some(code))
                    .map_err(str::to_owned),
            }
        }).await.map_err(|_| "WO9_GRAPH_NATIVE_WORKER_TASK_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, coordinator, workflow_id, step);
        Err("WO9_GRAPH_PROTECTED_WORKER_NOT_INSTALLED".into())
    }
}

/// WO9-D: native read-only preview for one tracked file; stage/commit
/// require separate exact-source revalidation and an explicit OS confirmation.
#[tauri::command]
async fn ide_git_review_file(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
) -> Result<lu_ide_workspace::reviewed_git::ReviewedGitFile, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.preview_reviewed_git_file(&project_id, &relative_path).map_err(str::to_owned)
    }).await.map_err(|_| "WO9_GIT_REVIEW_HOST_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_git_stage_reviewed(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    expected: lu_ide_workspace::reviewed_git::ReviewedGitFile,
) -> Result<lu_ide_workspace::reviewed_git::ReviewedStageReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let current = registry.preview_reviewed_git_file(
            &expected.project_id, &expected.relative_path).map_err(str::to_owned)?;
        if current.local_head != expected.local_head
            || current.index_fingerprint != expected.index_fingerprint
            || current.worktree_sha256 != expected.worktree_sha256
            || current.file_size != expected.file_size {
            return Err("WO9_GIT_REVIEW_CHANGED".into());
        }
        let allow = app.dialog().message(format!(
            "Stage only this reviewed file?\n{}\nSHA-256 {}\nNo hooks, Git CLI, or remote operations.",
            current.relative_path, current.worktree_sha256
        )).title("Lu IDE · One-file Git stage")
          .buttons(MessageDialogButtons::OkCancelCustom(
              "Stage reviewed bytes".into(), "Cancel".into())).blocking_show();
        if !allow { return Err("WO9_GIT_STAGE_NOT_AUTHORIZED".into()); }
        registry.stage_reviewed_git_file(&current).map_err(str::to_owned)
    }).await.map_err(|_| "WO9_GIT_STAGE_HOST_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_git_commit_reviewed(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    stage: lu_ide_workspace::reviewed_git::ReviewedStageReceipt,
    message: String,
    author_name: String,
    author_email: String,
) -> Result<lu_ide_workspace::reviewed_git::ReviewedCommitReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let allow = app.dialog().message(format!(
            "Create one local Git commit?\nFile: {}\nStaged SHA-256: {}\nMessage: {}\nNo Git hooks or push.",
            stage.relative_path, stage.source_sha256, message
        )).title("Lu IDE · Confirm local Git commit")
          .buttons(MessageDialogButtons::OkCancelCustom(
              "Commit exact staged file".into(), "Cancel".into())).blocking_show();
        if !allow { return Err("WO9_GIT_COMMIT_NOT_AUTHORIZED".into()); }
        registry.commit_reviewed_git_file(&stage, &message, &author_name, &author_email)
            .map_err(str::to_owned)
    }).await.map_err(|_| "WO9_GIT_COMMIT_HOST_UNAVAILABLE".to_owned())?
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

/// C7: user-selected task/file association is verified against actual
/// native registered project source bytes and the latest C6 task revision.
/// No source text, draft, CI verdict or worker authorization can be supplied.
#[tauri::command]
async fn ide_link_task_source(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    ledger: State<'_, Arc<DevelopmentLedger>>,
    project_id: String,
    task_id: String,
    relative_path: String,
    expected_saved_revision: String,
    expected_task_update_event_id: String,
) -> Result<lu_ide_workspace::task_evidence::TaskSummary, String> {
    let registry = Arc::clone(registry.inner());
    let ledger = Arc::clone(ledger.inner());
    tauri::async_runtime::spawn_blocking(move || {
        ledger.record_task_source(&registry, &project_id, &task_id,
            &relative_path, &expected_saved_revision, &expected_task_update_event_id)
            .map_err(|error| error.code().to_owned())
    })
    .await.map_err(|_| "TASK_SOURCE_UNAVAILABLE".to_owned())?
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
/// DEV-04.4: only the native host picks its most recently completed,
/// independently sealed Cargo build. No binary path, build receipt,
/// LLDB arguments, process ID or permission come from WebView.
#[tauri::command]
async fn ide_debug_run_project(
    app:AppHandle,
    registry:State<'_,Arc<IdeWorkspaceRegistry>>,
    project_id:String,
)->Result<serde_json::Value,String>{
    #[cfg(all(feature="native-project-debug",any(windows,target_os="linux")))]
    {
        let debug=app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEV044_PROTECTED_DEBUGGER_MISSING")?;
        let builds=app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("DEV044_PROTECTED_BUILD_HOST_MISSING")?;
        let debug=Arc::clone(debug.inner());
        let builds=Arc::clone(builds.inner());
        let registry=Arc::clone(registry.inner());
        let result=tauri::async_runtime::spawn_blocking(move||{
            let target=builds.native_project_debug_target(&project_id)?;
            debug.run_project(&app,&registry,&project_id,target)
        }).await.map_err(|_|"DEV044_PROJECT_DEBUG_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result)
            .map_err(|_|"DEV044_DEBUG_RESULT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature="native-project-debug",any(windows,target_os="linux"))))]
    {
        let _=(app,registry,project_id);
        Err("DEV044_PROJECT_DEBUG_FEATURE_NOT_INSTALLED".into())
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
async fn ide_build_read_output(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    job_id: String,
    after: u64,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        // The host revalidates project registration. Never accept a caller
        // supplied output path, process ID, stream name or grant token.
        registry.trusted_native_worker_root(&project_id)
            .map_err(|_| "BUILD_PROJECT_NOT_ACTIVE")?;
        let runtime = app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        let value = runtime.read_output(&project_id, &job_id, after)?;
        return serde_json::to_value(value)
            .map_err(|_| "BUILD_OUTPUT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, project_id, job_id, after);
        Err("BUILD_PROTECTED_WORKER_NOT_INSTALLED".into())
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

/// DEV-03.2B: native read-only review of one fixed language adapter
/// contract. This does not install toolchains, authorize a job or execute.
#[tauri::command]
async fn ide_review_multilang_adapter(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    profile_id: String,
    expected_saved_manifest_sha256: String,
) -> Result<lu_ide_workspace::reviewed_adapters::ReviewedAdapterContract, String> {
    let registry = Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.review_multilang_adapter(
            &project_id, &profile_id, &expected_saved_manifest_sha256,
        ).map_err(str::to_owned)
    })
    .await.map_err(|_| "DEV03_ADAPTER_REVIEW_UNAVAILABLE".to_owned())?
}

/// DEV-03.1: enumerate fixed registered-project build/test profiles and
/// conventional test-name hints. This read-only command grants no execution.
#[tauri::command]
async fn ide_review_project_debug_target(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    expected_manifest_sha256: String,
) -> Result<lu_ide_workspace::debug_target_review::DebugTargetReview, String> {
    registry.review_project_debug_target(&project_id, &expected_manifest_sha256)
        .map_err(str::to_owned)
}

#[tauri::command]
async fn ide_discover_conventional_tests(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::conventional_tests::TestDiscovery, String> {
    registry.discover_conventional_test_files(&project_id)
        .map_err(str::to_owned)
}

#[tauri::command]
async fn ide_discover_project_run_profiles(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::project_run_profiles::ProjectRunProfileDiscovery, String> {
    let registry = Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.discover_project_run_profiles(&project_id)
            .map_err(str::to_owned)
    })
    .await.map_err(|_| "DEV03_DISCOVERY_UNAVAILABLE".to_owned())?
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
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let installed = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>().is_some();
        let supported = tauri::async_runtime::spawn_blocking(move || {
            if !installed { return Vec::new(); }
            lsp_backend::installed_languages(&app)
        }).await.unwrap_or_default();
        if !supported.is_empty() {
            return LanguageWorkerStatus {
                ready: true, code: "LSP_REVIEWED_INSTALLATION_AVAILABLE",
                detail: "Reviewed native language server installed; human consent and actual OS worker containment are required for every session.",
                supported_languages: supported,
            };
        }
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
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
/// Only the reviewed, opt-in Windows/Linux native-lsp build can execute,
/// after an OS approval dialog and independent kernel/Job containment.
#[tauri::command]
async fn ide_lsp_start_session(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
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
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_change_document(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
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
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_feature(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
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
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_read_diagnostics(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
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
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {let _=(app,registry,request);Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())}
}
#[tauri::command]
async fn ide_lsp_stop_session(app: AppHandle) -> Result<(), String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native=Arc::clone(state.inner());
        return tauri::async_runtime::spawn_blocking(move ||
            native.stop()
        ).await.map_err(|_|"LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
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

/// B9. Navigation metadata is not a source snapshot or permission grant.
#[tauri::command]
async fn ide_workspace_layout_read(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    layouts: State<'_, Arc<lu_ide_workspace::workspace_layout::WorkspaceLayoutStore>>,
    project_id: String,
) -> Result<Option<lu_ide_workspace::workspace_layout::WorkspaceLayout>, String> {
    let registry = Arc::clone(registry.inner());
    let layouts = Arc::clone(layouts.inner());
    tauri::async_runtime::spawn_blocking(move || {
        layouts.read(&registry, &project_id).map_err(str::to_owned)
    }).await.map_err(|_| "B9_LAYOUT_TASK_UNAVAILABLE".to_owned())?
}

/// The human explicitly records only relative paths and hashes. Before an
/// atomic app-data write, the existing registered capability independently
/// re-opens every saved source and compares its revision. No project write.
#[tauri::command]
async fn ide_workspace_layout_save(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    layouts: State<'_, Arc<lu_ide_workspace::workspace_layout::WorkspaceLayoutStore>>,
    layout: lu_ide_workspace::workspace_layout::WorkspaceLayout,
) -> Result<lu_ide_workspace::workspace_layout::WorkspaceLayout, String> {
    let registry = Arc::clone(registry.inner());
    let layouts = Arc::clone(layouts.inner());
    tauri::async_runtime::spawn_blocking(move || {
        layouts.save(&registry, layout).map_err(str::to_owned)
    }).await.map_err(|_| "B9_LAYOUT_TASK_UNAVAILABLE".to_owned())?
}

/// Explicit removal of saved navigation only. Private B2 draft checkpoints
/// and registered project source files are never deleted by this command.
#[tauri::command]
async fn ide_workspace_layout_clear(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    layouts: State<'_, Arc<lu_ide_workspace::workspace_layout::WorkspaceLayoutStore>>,
    project_id: String,
) -> Result<bool, String> {
    let registry = Arc::clone(registry.inner());
    let layouts = Arc::clone(layouts.inner());
    tauri::async_runtime::spawn_blocking(move || {
        layouts.clear(&registry, &project_id).map_err(str::to_owned)
    }).await.map_err(|_| "B9_LAYOUT_TASK_UNAVAILABLE".to_owned())?
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
    layouts: State<'_, Arc<lu_ide_workspace::workspace_layout::WorkspaceLayoutStore>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    let ledger = Arc::clone(ledger.inner());
    let layouts = Arc::clone(layouts.inner());
    tauri::async_runtime::spawn_blocking(move || {
        // Remove the private navigation hint before dropping the catalog ID,
        // including for a currently closed but still registered project.
        // B2 draft records and project source bytes remain untouched.
        layouts.purge_for_unregister(&registry, &project_id)
            .map_err(str::to_owned)?;
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

/// B3: registered project, bounded literal saved-source search.
/// Reads only under the native picker capability. No scripts or writes.
#[tauri::command]
async fn ide_find_in_project(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    query: String,
    case_sensitive: bool,
) -> Result<lu_ide_workspace::project_search::ProjectSearchResult, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.find_in_project(&project_id, &query, case_sensitive)
            .map_err(|error| error.code().to_owned())
    }).await.map_err(|_| "PROJECT_SEARCH_UNAVAILABLE".to_owned())?
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

/// B2 recovery is in IDE private app data, NEVER in the user's project.
/// All four calls first validate an open native-picker project-relative file.
#[tauri::command]
async fn ide_draft_checkpoint(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    input: lu_ide_workspace::draft_vault::DraftCheckpointInput,
) -> Result<lu_ide_workspace::draft_vault::DraftSnapshot,String> {
    let registry=Arc::clone(registry.inner());
    let vault=Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move||{
        registry.preview_source_file(&input.project_id,&input.relative_path)
            .map_err(|e|e.code().to_owned())?;
        vault.checkpoint(input).map_err(str::to_owned)
    }).await.map_err(|_|"DRAFT_CHECKPOINT_TASK_UNAVAILABLE".to_owned())?
}
#[tauri::command]
async fn ide_draft_latest(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    project_id: String,
    relative_path: String,
) -> Result<Option<lu_ide_workspace::draft_vault::DraftSnapshot>,String> {
    let registry=Arc::clone(registry.inner());
    let vault=Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move||{
        registry.preview_source_file(&project_id,&relative_path)
            .map_err(|e|e.code().to_owned())?;
        vault.latest(&project_id,&relative_path).map_err(str::to_owned)
    }).await.map_err(|_|"DRAFT_RECOVERY_TASK_UNAVAILABLE".to_owned())?
}
#[tauri::command]
async fn ide_draft_load(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    project_id: String,
    relative_path: String,
    checkpoint_id: String,
) -> Result<lu_ide_workspace::draft_vault::DraftSnapshot,String> {
    let registry=Arc::clone(registry.inner());
    let vault=Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move||{
        registry.preview_source_file(&project_id,&relative_path)
            .map_err(|e|e.code().to_owned())?;
        vault.load(&project_id,&relative_path,&checkpoint_id).map_err(str::to_owned)
    }).await.map_err(|_|"DRAFT_LOAD_TASK_UNAVAILABLE".to_owned())?
}
#[tauri::command]
async fn ide_draft_discard(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    project_id: String,
    relative_path: String,
    expected_head: String,
) -> Result<(),String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry=Arc::clone(registry.inner());
    let vault=Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move||{
        registry.preview_source_file(&project_id,&relative_path)
            .map_err(|e|e.code().to_owned())?;
        let allowed=app.dialog().message(format!(
            "Delete the local, privately preserved draft history for this file?\n{}\nCheckpoint: {}\nThis does NOT delete the project file.",
            relative_path,expected_head
        )).title("Lu IDE · Confirm private draft history deletion")
          .buttons(MessageDialogButtons::OkCancelCustom(
              "Delete private drafts".into(),"Keep recovery".into())).blocking_show();
        if !allowed {return Err("DRAFT_DISCARD_NOT_APPROVED".into());}
        vault.discard(&project_id,&relative_path,&expected_head)
            .map_err(str::to_owned)
    }).await.map_err(|_|"DRAFT_DISCARD_TASK_UNAVAILABLE".to_owned())?
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
            let drafts=lu_ide_workspace::draft_vault::DraftVault::open(
                data.join("private-draft-recovery"))
                .map_err(|_|std::io::Error::other("IDE private draft vault unavailable"))?;
            app.manage(Arc::new(drafts));
            let navigation=lu_ide_workspace::workspace_layout::WorkspaceLayoutStore::open(
                data.join("private-workspace-navigation"))
                .map_err(|_|std::io::Error::other("IDE private navigation unavailable"))?;
            app.manage(Arc::new(navigation));
            app.manage(Arc::new(registry));
            let snapshots = InventorySnapshots::new(data.join("evidence-baselines.json"))
                .map_err(|_| std::io::Error::other("IDE evidence history failed to open"))?;
            let workflows = lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator::new(
                data.join("workflow-dependencies.json"))
                .map_err(|_| std::io::Error::other("IDE durable workflow journal failed to open"))?;
            app.manage(Arc::new(workflows));
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
            #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
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
            ide_discover_project_run_profiles,
            ide_discover_conventional_tests,
            ide_review_project_debug_target,
            ide_review_multilang_adapter,
            ide_build_status,
            ide_build_read_output,
            ide_build_start,
            ide_build_cancel,
            ide_build_reconcile,
            ide_debug_status,
            ide_debug_run_fixture,
            ide_debug_run_project,
            ide_debug_cancel,
            ide_debug_reconcile,
            ide_read_project_context,
            ide_record_task_update,
            ide_link_task_source,
            ide_observe_task_ci,
            ide_read_git_status,
            ide_read_git_diff,
            ide_probe_project_resource,
            ide_read_git_history,
            ide_compare_live_github,
            ide_read_exact_head_ci,
            ide_capture_project_inventory,
            ide_compare_project_inventory,
            ide_workflow_create,
            ide_workflow_list_runs,
            ide_workflow_run_step,
            ide_workflow_verify_native_receipt,
            ide_workflow_cancel_pending,
            ide_git_review_file,
            ide_git_stage_reviewed,
            ide_git_commit_reviewed,
            ide_workflow_preview,
            ide_workflow_verify,
            ide_workflow_export_manifest,
            ide_workflow_export_bundle,
            ide_list_projects,
            ide_workspace_layout_read,
            ide_workspace_layout_save,
            ide_workspace_layout_clear,
            ide_choose_project,
            ide_open_project,
            ide_close_project,
            ide_unregister_project,
            ide_list_source_directory,
            ide_find_in_project,
            ide_draft_checkpoint,
            ide_draft_latest,
            ide_draft_load,
            ide_draft_discard,
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
