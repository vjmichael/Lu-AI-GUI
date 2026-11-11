//! Standalone IDE desktop. All project directories originate in the OS picker,
//! not from JavaScript, model output, or a Lu IPC call.
#![forbid(unsafe_code)]
// D6D is deliberately impossible to package as a release or on Linux.
// The first-party LLDB owner/PE/PDB route still requires native OS acceptance.
#[cfg(all(feature = "dev046d-native-dialog", not(debug_assertions)))]
compile_error!("DEV-04.6D native dialog must not ship in release or signed NSIS builds");
#[cfg(all(feature = "dev046d-native-dialog", not(windows)))]
compile_error!("DEV-04.6D native dialog is Windows only");

#[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
mod build_backend;
#[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
mod debug_backend;
#[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
mod lsp_backend;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::{AtomicU8, Ordering}},
};

use lu_ide_workspace::{
    continuity::DevelopmentLedger,
    execution_intents::ExecutionIntents,
    inbox::{ApprovedStaging, IdeProposalInbox, ProposalDetail, ProposalSummary},
    inventory_history::InventorySnapshots,
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// In-memory one-use lifetime of an OS-confirmed partial stage.
#[derive(Default)]
struct PartialStageSessions(Mutex<HashMap<String, Arc<AtomicU8>>>);
impl PartialStageSessions {
    fn begin(&self, project_id: &str) -> Result<Arc<AtomicU8>, String> {
        let mut sessions = self.0.lock().map_err(|_| "G06B_STAGE_SESSION_UNAVAILABLE")?;
        if sessions.contains_key(project_id) {
            return Err("G06B_STAGE_ANOTHER_OPERATION_ACTIVE".into());
        }
        let token = Arc::new(AtomicU8::new(0));
        sessions.insert(project_id.to_owned(), Arc::clone(&token));
        Ok(token)
    }
    fn end(&self, project_id: &str) {
        if let Ok(mut sessions) = self.0.lock() { sessions.remove(project_id); }
    }
    fn cancel(&self, project_id: &str) -> Result<&'static str, String> {
        let sessions = self.0.lock().map_err(|_| "G06B_STAGE_SESSION_UNAVAILABLE")?;
        let control = sessions.get(project_id).ok_or("G06B_STAGE_NOT_IN_PROGRESS")?;
        if lu_ide_workspace::git_partial_stage::request_partial_stage_cancel(control) {
            Ok("cancel_requested_before_index_publication")
        } else {
            Err("G06B_STAGE_ALREADY_CANCELLED_OR_COMMIT_POINT".into())
        }
    }
}

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
fn observe(
    ledger: &DevelopmentLedger,
    project_id: &str,
    operation: &str,
    summary: &str,
    revision: Option<&str>,
    source_path: Option<&str>,
) {
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
        coordinator
            .create(&registry, &project_ids, &expected_fingerprint, steps)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "WO9_GRAPH_CREATE_TASK_UNAVAILABLE".to_owned())?
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
    coordinator
        .cancel_pending(&workflow_id, step)
        .map_err(str::to_owned)
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
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("WO9_GRAPH_PROTECTED_JOURNAL_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        let coordinator = Arc::clone(coordinator.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            let original = coordinator
                .pending_native_recheck(&workflow_id, step)
                .map_err(str::to_owned)?;
            let job_id = original
                .native_job_id
                .ok_or("WO9_NATIVE_JOURNAL_JOB_UNAVAILABLE")?;
            let plan_sha = original
                .native_plan_sha256
                .ok_or("WO9_NATIVE_JOURNAL_PLAN_UNAVAILABLE")?;
            runtime.verify_replayed_completed_job(&original.spec.project_id, &job_id, &plan_sha)?;
            coordinator
                .accept_rechecked_native_pass(&workflow_id, step, &job_id, &plan_sha)
                .map_err(str::to_owned)
        })
        .await
        .map_err(|_| "WO9_GRAPH_REPLAY_INSPECTION_TASK_UNAVAILABLE".to_owned())?;
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
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("WO9_GRAPH_PROTECTED_WORKER_NOT_INSTALLED")?;
        let runtime = Arc::clone(runtime.inner());
        let registry = Arc::clone(registry.inner());
        let coordinator = Arc::clone(coordinator.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            let spec = coordinator
                .begin_step(&registry, &workflow_id, step)
                .map_err(str::to_owned)?;
            let result = runtime.run(&app, &registry, &spec.project_id, &spec.recipe_id);
            match result {
                Ok(receipt) => coordinator
                    .finish_step(
                        &workflow_id,
                        step,
                        Some(receipt.job_id),
                        Some(receipt.plan_sha256),
                        receipt.output_sha256,
                        &receipt.phase,
                        receipt.native_exit_code,
                        receipt.requires_reconciliation || receipt.timed_out,
                        receipt.native_error_category,
                    )
                    .map_err(str::to_owned),
                Err(code) => coordinator
                    .finish_step(
                        &workflow_id,
                        step,
                        None,
                        None,
                        None,
                        "uncertain",
                        None,
                        true,
                        Some(code),
                    )
                    .map_err(str::to_owned),
            }
        })
        .await
        .map_err(|_| "WO9_GRAPH_NATIVE_WORKER_TASK_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, coordinator, workflow_id, step);
        Err("WO9_GRAPH_PROTECTED_WORKER_NOT_INSTALLED".into())
    }
}

/// WO9-D: native read-only preview for one tracked file; stage/commit
/// require separate exact-source revalidation and an explicit OS confirmation.
/// Group 6B batch review is metadata-only. These commands cannot stage,
/// commit, execute Git hooks or grant a future operator approval.
#[tauri::command]
async fn ide_git_preview_batch(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_paths: Vec<String>,
) -> Result<lu_ide_workspace::git_batch_review::ReviewedGitBatch, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.preview_reviewed_git_batch(&project_id, &relative_paths)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06B_BATCH_PREVIEW_HOST_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_git_verify_batch_review(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    expected: lu_ide_workspace::git_batch_review::ReviewedGitBatch,
) -> Result<lu_ide_workspace::git_batch_review::ReviewedGitBatch, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.revalidate_reviewed_git_batch(&expected)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06B_BATCH_REVERIFY_HOST_UNAVAILABLE".to_owned())?
}

/// Group 6B-02: exact reviewed hunks from a registered project. The
/// WebView cannot supply a patch or ask this command to stage anything.
#[tauri::command]
async fn ide_git_preview_hunks(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
) -> Result<lu_ide_workspace::git_hunk_review::ReviewedHunkPlan, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.preview_reviewed_git_hunks(&project_id, &relative_path)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06B_HUNK_HOST_UNAVAILABLE".to_owned())?
}

/// A selection is still review data. The entire supplied plan is
/// independently rebuilt from native Git and capability-held file bytes.
#[tauri::command]
async fn ide_git_select_reviewed_hunks(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    expected: lu_ide_workspace::git_hunk_review::ReviewedHunkPlan,
    selected_ids: Vec<String>,
) -> Result<lu_ide_workspace::git_hunk_review::ReviewedHunkSelection, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.select_reviewed_git_hunks(&expected, &selected_ids)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06B_HUNK_SELECTION_HOST_UNAVAILABLE".to_owned())?
}

/// Native OS confirmation supplies authorization. Neither plan nor
/// selected IDs are an approval token. Consent is deliberately one-shot.
#[tauri::command]
async fn ide_git_stage_selected_hunks(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    expected: lu_ide_workspace::git_hunk_review::ReviewedHunkSelection,
) -> Result<lu_ide_workspace::git_partial_stage::PartialStageReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let coordinator = Arc::clone(sessions.inner());
    let project_id = expected.plan.project_id.clone();
    let cancel = coordinator.begin(&project_id)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        // Independent verification precedes the dialog. Approval is never
        // derived from serialized WebView selection fields.
        let refreshed = registry.select_reviewed_git_hunks(
            &expected.plan, &expected.selected_ids).map_err(str::to_owned)?;
        if refreshed.selection_sha256 != expected.selection_sha256
            || refreshed.selected_ids != expected.selected_ids
            || refreshed.plan != expected.plan
            || !expected.ready_for_operator_review
            || expected.stage_authorized || expected.commit_authorized
            || expected.operation_performed
        {
            return Err("G06B_STAGE_REVIEW_STALE_OR_FORGED".into());
        }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06B_STAGE_CANCELLED_BEFORE_APPROVAL".into());
        }
        let approved = app.dialog()
            .message(format!(
                "Stage ONLY {} selected Git hunk(s) in one tracked file?\n\nFile: {}\nProject: {}\nHEAD: {}\nSelection SHA-256: {}\n\nThis changes the Git INDEX, not the source file. No commit, hooks, checkout, Git CLI or remote operations. Review does not authorize future changes.",
                refreshed.selected_ids.len(),
                refreshed.plan.relative_path,
                refreshed.plan.project_id,
                refreshed.plan.head_sha,
                refreshed.selection_sha256,
            ))
            .title("Lu IDE · One-use protected partial Git stage")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Stage only reviewed hunks".into(), "Deny / Cancel".into(),
            ))
            .blocking_show();
        if !approved {
            return Err("G06B_STAGE_HUMAN_DENIED_OR_CANCELLED".into());
        }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06B_STAGE_CANCELLED_AFTER_DIALOG".into());
        }
        registry.apply_approved_partial_stage(&refreshed, &cancel)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06B_STAGE_HOST_TASK_INTERRUPTED".to_owned());
    coordinator.end(&project_id);
    result?
}

/// Safe to request while an approval dialog or native preparation is active.
/// Cancellation never rolls back an already-published Git index.
#[tauri::command]
async fn ide_git_cancel_partial_stage(
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<&'static str, String> {
    sessions.cancel(&project_id)
}

/// Read-only durable journal reconciliation across native process restart.
/// Never guess an interrupted index write can be rolled back automatically.
/// On fresh native OS confirmation, recheck durable Git evidence and
/// append only a terminal receipt. Never delete index.lock or roll back data.
#[tauri::command]
async fn ide_git_reconcile_partial_stage(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_partial_stage::PartialStageRecovery, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let status = registry.inspect_partial_stage_recovery(&project_id)
            .map_err(str::to_owned)?;
        if !status.operator_reconciliation_required {
            return Ok(status);
        }
        if status.pending_git_lock {
            return Err("G06B_STAGE_STALE_LOCK_REQUIRES_MANUAL_GIT_REVIEW".into());
        }
        let approved = app.dialog()
            .message(format!(
                "Review interrupted partial Git stage for registered project {}?\n\nState: {}\nLast journal phase: {}\n\nLu will independently verify HEAD, Git index bytes, original tracked source, and the intended staged Git blob before appending a terminal receipt. It will NOT change or delete the Git index, remove a lock, or authorize another stage.",
                project_id, status.state, status.last_phase
            ))
            .title("Lu IDE · Review interrupted partial Git stage")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Verify and reconcile receipt".into(),
                "Deny / Cancel".into(),
            ))
            .blocking_show();
        if !approved { return Err("G06B_STAGE_RECOVERY_HUMAN_DENIED".into()); }
        registry.reconcile_partial_stage_journal(&project_id)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06B_STAGE_RECOVERY_HOST_INTERRUPTED".to_owned())?
}

#[tauri::command]
async fn ide_git_partial_stage_recovery(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_partial_stage::PartialStageRecovery, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_partial_stage_recovery(&project_id).map_err(str::to_owned)
    }).await.map_err(|_| "G06B_STAGE_RECOVERY_HOST_UNAVAILABLE".to_owned())?
}

/// Read-only reactivation of an independently verified staged-hunk
/// receipt after installed IDE restart. This does NOT grant a commit.
#[tauri::command]
async fn ide_git_restore_reviewed_stage_receipt(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_partial_stage::PartialStageReceipt, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.restore_verified_partial_stage_receipt(&project_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06B_STAGE_RESTORE_HOST_UNAVAILABLE".to_owned())?
}

/// Separate from staging: a reviewed staged-hunk index alone cannot
/// authorize either a new commit or a history rewrite.
#[tauri::command]
async fn ide_git_preview_reviewed_commit(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    stage: lu_ide_workspace::git_partial_stage::PartialStageReceipt,
    mode: lu_ide_workspace::git_reviewed_commit::ReviewedCommitMode,
    message: String,
    author_name: String,
    author_email: String,
) -> Result<lu_ide_workspace::git_reviewed_commit::ReviewedCommitPlan, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.preview_reviewed_partial_commit(
            &stage, mode, &message, &author_name, &author_email)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06B_COMMIT_PREVIEW_HOST_UNAVAILABLE".to_owned())?
}

fn perform_reviewed_commit_with_os_approval(
    app: AppHandle,
    registry: Arc<IdeWorkspaceRegistry>,
    expected: lu_ide_workspace::git_reviewed_commit::ReviewedCommitPlan,
    required_mode: lu_ide_workspace::git_reviewed_commit::ReviewedCommitMode,
    cancel: Arc<AtomicU8>,
) -> Result<lu_ide_workspace::git_reviewed_commit::ReviewedCommitReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    use lu_ide_workspace::git_reviewed_commit::ReviewedCommitMode;
    if expected.mode != required_mode {
        return Err("G06B_COMMIT_MODE_NOT_AUTHORIZED".into());
    }
    let fresh = registry.preview_reviewed_partial_commit(
        &expected.stage, expected.mode.clone(), &expected.message,
        &expected.author_name, &expected.author_email).map_err(str::to_owned)?;
    if fresh != expected {
        return Err("G06B_COMMIT_REVIEW_CHANGED".into());
    }
    if cancel.load(Ordering::SeqCst) != 0 {
        return Err("G06B_COMMIT_CANCELLED_BEFORE_CONSENT".into());
    }
    let mode_label = if required_mode == ReviewedCommitMode::Amend {
        "REWRITE LAST LOCAL COMMIT" } else { "CREATE NEW LOCAL COMMIT" };
    let allowed = app.dialog().message(format!(
        "{}?\n\nProject: {}\nBranch: {}\nCurrent HEAD: {}\nNext parent: {}\nFile: {}\nStaged tree: {}\nIndex SHA-256: {}\nPlan SHA-256: {}\nAuthor: {} <{}>\nMessage: {}\n\nThis approval is one-use. No hook, Git CLI, network, push, or source edit.",
        mode_label, fresh.stage.project_id, fresh.branch_ref,
        fresh.previous_head, fresh.resulting_parent, fresh.stage.relative_path,
        fresh.staged_tree_oid, fresh.index_sha256, fresh.plan_sha256,
        fresh.author_name, fresh.author_email, fresh.message,
    )).title("Lu IDE · One-use local Git commit authorization")
      .buttons(MessageDialogButtons::OkCancelCustom(
        if required_mode == ReviewedCommitMode::Amend {
            "Review history rewrite".into()
        } else { "Create this exact commit".into() },
        "Deny / Cancel".into(),
      )).blocking_show();
    if !allowed { return Err("G06B_COMMIT_HUMAN_DENIED".into()); }
    if required_mode == ReviewedCommitMode::Amend {
        // History rewrite is an independently acknowledged destructive act,
        // not an implied side effect of the ordinary commit approval.
        let rewrite = app.dialog().message(format!(
            "FINAL HISTORY-REWRITE CONSENT\n\nReplace local commit {} on {} with a NEW commit whose parent is {}?\n\nThe previous commit will no longer be branch HEAD. This may invalidate clones, tags, approvals, published links, or collaborators. Lu cannot verify remote publication offline. No force-push is performed. Deny unless you intend to replace this exact local commit.",
            fresh.previous_head, fresh.branch_ref, fresh.resulting_parent
        )).title("Lu IDE · Explicit amend/rewrite authorization")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Rewrite exact local HEAD".into(), "Deny / Cancel".into()
          )).blocking_show();
        if !rewrite { return Err("G06B_AMEND_HUMAN_DENIED".into()); }
    }
    if cancel.load(Ordering::SeqCst) != 0 {
        return Err("G06B_COMMIT_CANCELLED_AFTER_CONSENT".into());
    }
    registry.execute_approved_reviewed_commit(&fresh, &cancel)
        .map_err(str::to_owned)
}

#[tauri::command]
async fn ide_git_commit_selected_hunks(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    expected: lu_ide_workspace::git_reviewed_commit::ReviewedCommitPlan,
) -> Result<lu_ide_workspace::git_reviewed_commit::ReviewedCommitReceipt, String> {
    use lu_ide_workspace::git_reviewed_commit::ReviewedCommitMode;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let project_id = expected.stage.project_id.clone();
    let cancel = sessions.begin(&project_id)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        perform_reviewed_commit_with_os_approval(
            app, registry, expected, ReviewedCommitMode::Create, cancel)
    }).await.map_err(|_| "G06B_COMMIT_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

#[tauri::command]
async fn ide_git_amend_reviewed_commit(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    expected: lu_ide_workspace::git_reviewed_commit::ReviewedCommitPlan,
) -> Result<lu_ide_workspace::git_reviewed_commit::ReviewedCommitReceipt, String> {
    use lu_ide_workspace::git_reviewed_commit::ReviewedCommitMode;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let project_id = expected.stage.project_id.clone();
    let cancel = sessions.begin(&project_id)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        perform_reviewed_commit_with_os_approval(
            app, registry, expected, ReviewedCommitMode::Amend, cancel)
    }).await.map_err(|_| "G06B_AMEND_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

/// Safe before the native commit's branch-ref compare-and-swap fence.
#[tauri::command]
async fn ide_git_cancel_reviewed_commit(
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<&'static str, String> {
    sessions.cancel(&project_id)
}

#[tauri::command]
async fn ide_git_inspect_commit_recovery(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_reviewed_commit::ReviewedCommitRecovery, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_reviewed_commit_recovery(&project_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06B_COMMIT_RECOVERY_HOST_UNAVAILABLE".to_owned())?
}

/// Separate physical OS approval to reconcile a journal, never a hidden
/// automatic retry, ref update, force push, index change, or lock deletion.
#[tauri::command]
async fn ide_git_reconcile_commit_receipt(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_reviewed_commit::ReviewedCommitRecovery, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let _token = sessions.begin(&project_id)?;
    let selected_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let observed = registry.inspect_reviewed_commit_recovery(&selected_id)
            .map_err(str::to_owned)?;
        if !observed.operator_reconciliation_required {
            return Ok(observed);
        }
        if observed.git_index_lock_present {
            return Err("G06B_COMMIT_LOCK_REQUIRES_MANUAL_REVIEW".into());
        }
        let approved = app.dialog().message(format!(
            "Reconcile original local Git commit journal for registered project {}?\n\nObserved: {}\nPhase: {}\n\nLu will independently inspect the Git index, branch HEAD, parent, tree and original file before recording a terminal receipt. It will NOT retry a commit, rewrite a ref, delete a Git lock, or push.",
            selected_id, observed.state, observed.last_phase,
        )).title("Lu IDE · Approve commit receipt reconciliation")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Verify journal only".into(), "Deny / Cancel".into()
          )).blocking_show();
        if !approved { return Err("G06B_COMMIT_RECOVERY_DENIED".into()); }
        registry.reconcile_reviewed_commit_journal(&selected_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06B_COMMIT_RECOVERY_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

#[tauri::command]
async fn ide_git_review_file(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
) -> Result<lu_ide_workspace::reviewed_git::ReviewedGitFile, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .preview_reviewed_git_file(&project_id, &relative_path)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "WO9_GIT_REVIEW_HOST_UNAVAILABLE".to_owned())?
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
        registry
            .inspect_multi_project_workflow(&project_ids)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "WORKFLOW_NATIVE_TASK_UNAVAILABLE".to_owned())?
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
        registry
            .verify_multi_project_workflow(&project_ids, &expected_fingerprint)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "WORKFLOW_REVALIDATE_TASK_UNAVAILABLE".to_owned())?
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
        let folder = NativePicker(app)
            .choose_project_directory()
            .map_err(|_| "WORKFLOW_EXPORT_PICKER_UNAVAILABLE")?
            .ok_or("WORKFLOW_EXPORT_CANCELLED")?;
        registry
            .export_workflow_manifest(&project_ids, &expected_fingerprint, &folder)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "WORKFLOW_EXPORT_HOST_TASK_UNAVAILABLE".to_owned())?
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
        let selected = NativePicker(app)
            .choose_project_directory()
            .map_err(|_| "WORKFLOW_BUNDLE_PICKER_UNAVAILABLE")?
            .ok_or("WORKFLOW_BUNDLE_CANCELLED")?;
        registry
            .export_workflow_bundle(&project_ids, &expected_fingerprint, &selected)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "WORKFLOW_BUNDLE_TASK_UNAVAILABLE".to_owned())?
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
        registry
            .probe_project_resource(&project_id, &relative_path)
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
        registry
            .inspect_git_diff(&project_id, &scope)
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
        registry
            .inspect_git_history(&project_id)
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
    let status =
        tauri::async_runtime::spawn_blocking(move || registry.inspect_git_worktree(&project_id))
            .await
            .map_err(|_| "GIT_STATUS_UNAVAILABLE".to_owned())?;
    Ok(status)
}

/// Read-only branch and linked worktree metadata from the exact selected
/// native project root. A linked location is never authorized or exposed.
#[tauri::command]
async fn ide_read_git_repository_inventory(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_inventory::GitRepositoryInventory, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_git_repository_inventory(&project_id)
    })
    .await
    .map_err(|_| "GIT_INVENTORY_HOST_UNAVAILABLE".to_owned())
}

/// Local committed-file history and blame only. Untrusted file path is
/// checked inside the native registered-project capability.
#[tauri::command]
async fn ide_read_git_file_history(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
) -> Result<lu_ide_workspace::git_file_history::GitFileHistoryEvidence, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_git_file_history(&project_id, &relative_path)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06A_HISTORY_HOST_UNAVAILABLE".to_owned())?
}

/// Read-only local refs, safe remote aliases and Git health; no URLs,
/// credential access, Git process or branch mutation.
#[tauri::command]
async fn ide_read_git_refs_health(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_refs_health::GitRefsAndHealthEvidence, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_git_refs_health(&project_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06A_REFS_HOST_UNAVAILABLE".to_owned())?
}

/// On explicit human request, compare local work with current public GitHub
/// branch evidence. HTTPS to fixed api.github.com only; no token or Git fetch.
#[tauri::command]
async fn ide_compare_live_github(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::github_live::LiveGitHubComparison, String> {
    let registry = Arc::clone(state.inner());
    let result =
        tauri::async_runtime::spawn_blocking(move || registry.compare_live_github(&project_id))
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
        .await
        .map_err(|_| "GITHUB_CI_TASK_UNAVAILABLE".to_owned())
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
        ledger
            .context(&registry, &project_id)
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
        snapshots
            .capture(&registry, &project_id)
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
        snapshots
            .compare(&registry, &project_id)
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
        ledger
            .record_task_update(
                &registry,
                &project_id,
                &task_id,
                &title,
                &status,
                &next_action,
                required_gates,
            )
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
        ledger
            .record_task_source(
                &registry,
                &project_id,
                &task_id,
                &relative_path,
                &expected_saved_revision,
                &expected_task_update_event_id,
            )
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "TASK_SOURCE_UNAVAILABLE".to_owned())?
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
        ledger
            .record_task_ci(&registry, &project_id, &task_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "TASK_CI_UNAVAILABLE".to_owned())?
}

// WO6 native debugger fixture. The WebView cannot submit an executable,
// adapter path, PID, expression, breakpoint condition, arguments or token.
// All execution is restricted to the independently pinned self-test bundle.
#[tauri::command]
async fn ide_debug_status(app: AppHandle, project_id: String) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
    {
        let backend = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        let backend = Arc::clone(backend.inner());
        let result =
            tauri::async_runtime::spawn_blocking(move || backend.status(&app, &project_id))
                .await
                .map_err(|_| "DEBUG_STATUS_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "DEBUG_STATUS_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-debug", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id);
        Ok(serde_json::json!({
            "ready":false,"projectDebugSupported":false,
            "operationControlSupported":false,
            "operationControlCode":"DEV046D_NATIVE_HUMAN_ACTION_APPROVAL_NOT_ACCEPTED",
            "code":"DEBUG_PROTECTED_ADAPTER_NOT_INSTALLED",
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
        let backend = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        let backend = Arc::clone(backend.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            backend.run_fixture(&app, &registry, &project_id)
        })
        .await
        .map_err(|_| "DEBUG_EXECUTION_TASK_UNAVAILABLE")??;
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
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-project-debug", any(windows, target_os = "linux")))]
    {
        let debug = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEV044_PROTECTED_DEBUGGER_MISSING")?;
        let builds = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("DEV044_PROTECTED_BUILD_HOST_MISSING")?;
        let debug = Arc::clone(debug.inner());
        let builds = Arc::clone(builds.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            let target = builds.native_project_debug_target(&project_id)?;
            debug.run_project(&app, &registry, &project_id, target)
        })
        .await
        .map_err(|_| "DEV044_PROJECT_DEBUG_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result)
            .map_err(|_| "DEV044_DEBUG_RESULT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-project-debug", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, project_id);
        Err("DEV044_PROJECT_DEBUG_FEATURE_NOT_INSTALLED".into())
    }
}

/// Only fixed operations can be proposed by the renderer; no DAP request,
/// source path, thread ID or approval can be supplied to the native host.
#[tauri::command]
async fn ide_debug_step_project(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
    {
        let debug = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEV046D_PROTECTED_DEBUGGER_MISSING")?;
        let builds = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("DEV046D_PROTECTED_BUILD_HOST_MISSING")?;
        let debug = Arc::clone(debug.inner());
        let builds = Arc::clone(builds.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            let target = builds.native_project_debug_target(&project_id)?;
            debug.run_project_native_action(
                &app,
                &registry,
                &project_id,
                target,
                debug_backend::ReviewedNativeAction::StepOver,
            )
        })
        .await
        .map_err(|_| "DEV046D_STEP_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "DEV046D_STEP_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(windows, debug_assertions, feature = "dev046d-native-dialog")))]
    {
        let _ = (app, registry, project_id);
        Err("DEV046D_NATIVE_ACTIONS_NOT_INSTALLED".into())
    }
}
#[tauri::command]
async fn ide_debug_breakpoint_project(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
    {
        let debug = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEV046D_PROTECTED_DEBUGGER_MISSING")?;
        let builds = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("DEV046D_PROTECTED_BUILD_HOST_MISSING")?;
        let debug = Arc::clone(debug.inner());
        let builds = Arc::clone(builds.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            let target = builds.native_project_debug_target(&project_id)?;
            debug.run_project_native_action(
                &app,
                &registry,
                &project_id,
                target,
                debug_backend::ReviewedNativeAction::SameLineBreakpoint,
            )
        })
        .await
        .map_err(|_| "DEV046D_BREAKPOINT_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result)
            .map_err(|_| "DEV046D_BREAKPOINT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(windows, debug_assertions, feature = "dev046d-native-dialog")))]
    {
        let _ = (app, registry, project_id);
        Err("DEV046D_NATIVE_ACTIONS_NOT_INSTALLED".into())
    }
}

/// G04D: opt-in developer only. The native host derives the exact
/// previously verified frame and obtains a NEW physical OS modal for this
/// inspection. No path, PID, frame ID, DAP payload or consent crosses JS.
#[tauri::command]
async fn ide_debug_inspect_project_variables(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
    {
        let debug = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("IDE_G04D_PROTECTED_DEBUGGER_MISSING")?;
        let builds = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("IDE_G04D_PROTECTED_BUILD_HOST_MISSING")?;
        let debug = Arc::clone(debug.inner());
        let builds = Arc::clone(builds.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            let target = builds.native_project_debug_target(&project_id)?;
            debug.run_project_native_action(
                &app,
                &registry,
                &project_id,
                target,
                debug_backend::ReviewedNativeAction::InspectVariables,
            )
        })
        .await
        .map_err(|_| "IDE_G04D_VARIABLES_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result)
            .map_err(|_| "IDE_G04D_VARIABLES_SERIALIZATION_FAILED".into());
    }
    #[cfg(not(all(windows, debug_assertions, feature = "dev046d-native-dialog")))]
    {
        let _ = (app, registry, project_id);
        Err("IDE_G04D_VARIABLE_INSPECTION_NOT_INSTALLED".into())
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
        let backend = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        return backend
            .cancel(&project_id, job_id.as_deref())
            .map(str::to_owned);
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
        let backend = app
            .try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("DEBUG_PROTECTED_HOST_UNAVAILABLE")?;
        let backend = Arc::clone(backend.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            backend
                .reconcile(&app, &project_id, &job_id)
                .map(str::to_owned)
        })
        .await
        .map_err(|_| "DEBUG_RECOVERY_TASK_UNAVAILABLE")?;
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
async fn ide_build_status(app: AppHandle, project_id: String) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        let result =
            tauri::async_runtime::spawn_blocking(move || runtime.status(&app, &project_id))
                .await
                .map_err(|_| "BUILD_STATUS_TASK_UNAVAILABLE")??;
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
        registry
            .trusted_native_worker_root(&project_id)
            .map_err(|_| "BUILD_PROJECT_NOT_ACTIVE")?;
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("BUILD_PROTECTED_HOST_UNAVAILABLE")?;
        let value = runtime.read_output(&project_id, &job_id, after)?;
        return serde_json::to_value(value).map_err(|_| "BUILD_OUTPUT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, project_id, job_id, after);
        Err("BUILD_PROTECTED_WORKER_NOT_INSTALLED".into())
    }
}

/// IDE-05.3: fixed offline uv Sync ONLY. The renderer can submit only
/// opaque project identity and two prior review digests. Native code derives
/// operation=Sync, manager=uv, exact worktree/manifest, pinned runtime, OS
/// containment, per-job human modal and private output location. This never
/// uses historical preview revalidation as an execution grant.
#[tauri::command]
async fn ide_package_run_reviewed_uv(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    expected_preview_sha256: String,
    expected_evidence_fingerprint: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-package-uv", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("IDE053_PROTECTED_PACKAGE_HOST_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            runtime.run_reviewed_uv_sync(
                &app,
                &registry,
                &project_id,
                &expected_preview_sha256,
                &expected_evidence_fingerprint,
            )
        })
        .await
        .map_err(|_| "IDE053_PACKAGE_WORKER_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result)
            .map_err(|_| "IDE053_PACKAGE_RESULT_SERIALIZE_FAILED".into());
    }
    #[cfg(not(all(feature = "native-package-uv", any(windows, target_os = "linux"))))]
    {
        let _ = (
            app,
            registry,
            project_id,
            expected_preview_sha256,
            expected_evidence_fingerprint,
        );
        Err("IDE053_PROTECTED_PACKAGE_RUNTIME_NOT_APPROVED".into())
    }
}

/// IDE-05.4: user-confirmed discard ONLY of private, completed package
/// output under the native fixed app-profile root. No project paths,
/// deletion flags, or worker grants are accepted. Prepared-only cleanup
/// receipts remain pending on host crash, never auto-replayed.
#[tauri::command]
async fn ide_package_discard_private_output(
    app: AppHandle,
    project_id: String,
    job_id: String,
) -> Result<String, String> {
    #[cfg(all(feature = "native-package-uv", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("IDE054_PACKAGE_LIFECYCLE_HOST_UNAVAILABLE")?;
        let runtime = Arc::clone(runtime.inner());
        return tauri::async_runtime::spawn_blocking(move || {
            runtime.discard_package_private_output(&app, &project_id, &job_id)
        })
        .await
        .map_err(|_| "IDE054_PRIVATE_DISCARD_TASK_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(feature = "native-package-uv", any(windows, target_os = "linux"))))]
    {
        let _ = (app, project_id, job_id);
        Err("IDE054_PRIVATE_PACKAGE_DISCARD_NOT_INSTALLED".into())
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
        })
        .await
        .map_err(|_| "BUILD_EXECUTION_TASK_UNAVAILABLE")??;
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
        return runtime
            .cancel(&project_id, job_id.as_deref())
            .map(str::to_owned);
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
        })
        .await
        .map_err(|_| "BUILD_RECOVERY_TASK_UNAVAILABLE")?;
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
        registry
            .review_multilang_adapter(&project_id, &profile_id, &expected_saved_manifest_sha256)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "DEV03_ADAPTER_REVIEW_UNAVAILABLE".to_owned())?
}

/// DEV-03.1: enumerate fixed registered-project build/test profiles and
/// conventional test-name hints. This read-only command grants no execution.
#[tauri::command]
async fn ide_review_project_debug_target(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    expected_manifest_sha256: String,
) -> Result<lu_ide_workspace::debug_target_review::DebugTargetReview, String> {
    registry
        .review_project_debug_target(&project_id, &expected_manifest_sha256)
        .map_err(str::to_owned)
}

#[tauri::command]
async fn ide_discover_conventional_tests(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::conventional_tests::TestDiscovery, String> {
    registry
        .discover_conventional_test_files(&project_id)
        .map_err(str::to_owned)
}

/// IDE-05.1 manager-of-managers read-only detection. The WebView supplies
/// an opaque registered project ID ONLY; manager/tool/path selection is fixed
/// inside the native host. No subprocess or network access is possible here.
#[tauri::command]
async fn ide_inspect_package_managers(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::package_manager::PackageManagerInspection, String> {
    let registry = Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .inspect_package_managers(&project_id)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "IDE05_DISCOVERY_TASK_UNAVAILABLE".to_owned())?
}

/// IDE-05.2: statically review a *fixed typed* package intent bound to
/// registered project manifest SHA evidence. Does NOT resolve or install.
#[tauri::command]
async fn ide_preview_package_operation(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    request: lu_ide_workspace::package_preview::PackagePreviewRequest,
) -> Result<lu_ide_workspace::package_preview::PackageOperationPreview, String> {
    let registry = Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .preview_package_operation(&project_id, request)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "IDE052_PREVIEW_TASK_UNAVAILABLE".to_owned())?
}

/// The host ALWAYS independently reopens every native manifest and lock.
/// A matching digest means only "safe to review this exact intent again",
/// not authorization. No Tauri command can apply the proposal.
#[tauri::command]
async fn ide_revalidate_package_operation(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    request: lu_ide_workspace::package_preview::PackagePreviewRequest,
    expected_preview_sha256: String,
    expected_evidence_fingerprint: String,
) -> Result<lu_ide_workspace::package_preview::PackagePreviewRevalidation, String> {
    let registry = Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .revalidate_package_operation(
                &project_id,
                request,
                &expected_preview_sha256,
                &expected_evidence_fingerprint,
            )
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "IDE052_REVALIDATION_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_discover_project_run_profiles(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::project_run_profiles::ProjectRunProfileDiscovery, String> {
    let registry = Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .discover_project_run_profiles(&project_id)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "DEV03_DISCOVERY_UNAVAILABLE".to_owned())?
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
    tauri::async_runtime::spawn_blocking(move || {
        intents
            .prepare(&registry, &project_id, &recipe_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "EXECUTION_PREPARE_UNAVAILABLE".to_owned())?
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
    tauri::async_runtime::spawn_blocking(move || {
        intents
            .decide(&registry, &project_id, &intent_id, &decision)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "EXECUTION_REVIEW_UNAVAILABLE".to_owned())?
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
    tauri::async_runtime::spawn_blocking(move || {
        intents
            .list(&registry, &project_id)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "EXECUTION_LIST_UNAVAILABLE".to_owned())?
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
        let installed = app
            .try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .is_some();
        let supported = tauri::async_runtime::spawn_blocking(move || {
            if !installed {
                return Vec::new();
            }
            lsp_backend::installed_languages(&app)
        })
        .await
        .unwrap_or_default();
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
        let state = app
            .try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let registry = Arc::clone(registry.inner());
        let typed = serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_| "LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move || native.start(&app, &registry, typed))
            .await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, registry, request);
        Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())
    }
}
#[tauri::command]
async fn ide_lsp_change_document(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state = app
            .try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let registry = Arc::clone(registry.inner());
        let typed = serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_| "LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move || native.change(&registry, typed))
            .await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, registry, request);
        Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())
    }
}
#[tauri::command]
async fn ide_lsp_feature(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state = app
            .try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let registry = Arc::clone(registry.inner());
        let typed = serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_| "LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move || native.feature(&registry, typed))
            .await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, registry, request);
        Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())
    }
}
#[tauri::command]
async fn ide_lsp_read_diagnostics(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state = app
            .try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let registry = Arc::clone(registry.inner());
        let typed = serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_| "LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move || native.diagnostics(&registry, typed))
            .await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, registry, request);
        Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())
    }
}
#[tauri::command]
async fn ide_lsp_stop_session(app: AppHandle) -> Result<(), String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state = app
            .try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        return tauri::async_runtime::spawn_blocking(move || native.stop())
            .await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = app;
        Err("LSP_PROTECTED_WORKER_UNVERIFIED".into())
    }
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
    })
    .await
    .map_err(|_| "B9_LAYOUT_TASK_UNAVAILABLE".to_owned())?
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
    })
    .await
    .map_err(|_| "B9_LAYOUT_TASK_UNAVAILABLE".to_owned())?
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
    })
    .await
    .map_err(|_| "B9_LAYOUT_TASK_UNAVAILABLE".to_owned())?
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
        let attempted = registry.register_from_native_picker(&mut NativePicker(app));
        // Fixed-vocabulary diagnostic; never print selected filesystem paths,
        // project names, user content or window text. Native selection is not
        // a worker authorization or an assertion that UI has rendered.
        match &attempted {
            Ok(Some(_)) => eprintln!("IDE056_NATIVE_PICKER_RESULT=registered"),
            Ok(None) => eprintln!("IDE056_NATIVE_PICKER_RESULT=cancelled"),
            Err(error) => {
                let category = match stable_error(error) {
                    "INVALID_DIRECTORY" => "invalid_directory",
                    "PROJECT_CAPACITY" => "project_capacity",
                    "PROJECT_ALREADY_OPEN" => "already_open",
                    "IO_FAILURE" => "io_failure",
                    "CATALOG_INVALID" => "catalog_invalid",
                    "HOST_UNAVAILABLE" => "host_unavailable",
                    _ => "unexpected_error",
                };
                eprintln!("IDE056_NATIVE_PICKER_RESULT={category}");
            }
        }
        let selected = attempted.map_err(|error| stable_error(&error).to_string())?;
        if let Some(ref project) = selected {
            observe(
                &ledger,
                &project.project_id,
                "project.register",
                "Project registered through native picker",
                None,
                None,
            );
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
        observe(
            &ledger,
            &project_id,
            "project.open",
            "Project reopened",
            None,
            None,
        );
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
        observe(
            &ledger,
            &project_id,
            "project.close",
            "Project closed",
            None,
            None,
        );
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
        layouts
            .purge_for_unregister(&registry, &project_id)
            .map_err(str::to_owned)?;
        registry
            .unregister(&project_id)
            .map_err(|error| stable_error(&error).to_string())?;
        observe(
            &ledger,
            &project_id,
            "project.unregister",
            "Project removed from IDE catalog",
            None,
            None,
        );
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
        registry
            .list_source_directory(&project_id, &relative_dir)
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
        registry
            .find_in_project(&project_id, &query, case_sensitive)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "PROJECT_SEARCH_UNAVAILABLE".to_owned())?
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
        registry
            .read_source_span(
                &project_id,
                &relative_path,
                &expected_revision,
                first_line,
                last_line,
            )
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
        registry
            .preview_source_file(&project_id, &relative_path)
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
            .review_source_change(
                &project_id,
                &relative_path,
                &expected_revision,
                &proposed_text,
            )
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
) -> Result<lu_ide_workspace::draft_vault::DraftSnapshot, String> {
    let registry = Arc::clone(registry.inner());
    let vault = Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .preview_source_file(&input.project_id, &input.relative_path)
            .map_err(|e| e.code().to_owned())?;
        vault.checkpoint(input).map_err(str::to_owned)
    })
    .await
    .map_err(|_| "DRAFT_CHECKPOINT_TASK_UNAVAILABLE".to_owned())?
}
#[tauri::command]
async fn ide_draft_latest(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    project_id: String,
    relative_path: String,
) -> Result<Option<lu_ide_workspace::draft_vault::DraftSnapshot>, String> {
    let registry = Arc::clone(registry.inner());
    let vault = Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .preview_source_file(&project_id, &relative_path)
            .map_err(|e| e.code().to_owned())?;
        vault
            .latest(&project_id, &relative_path)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "DRAFT_RECOVERY_TASK_UNAVAILABLE".to_owned())?
}
#[tauri::command]
async fn ide_draft_load(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    project_id: String,
    relative_path: String,
    checkpoint_id: String,
) -> Result<lu_ide_workspace::draft_vault::DraftSnapshot, String> {
    let registry = Arc::clone(registry.inner());
    let vault = Arc::clone(vault.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .preview_source_file(&project_id, &relative_path)
            .map_err(|e| e.code().to_owned())?;
        vault
            .load(&project_id, &relative_path, &checkpoint_id)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "DRAFT_LOAD_TASK_UNAVAILABLE".to_owned())?
}
#[tauri::command]
async fn ide_draft_discard(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    vault: State<'_, Arc<lu_ide_workspace::draft_vault::DraftVault>>,
    project_id: String,
    relative_path: String,
    expected_head: String,
) -> Result<(), String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(registry.inner());
    let vault = Arc::clone(vault.inner());
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
        observe(
            &ledger,
            &project_id,
            "source.save",
            "Source saved with revision check",
            Some(&saved.revision),
            Some(&relative_path),
        );
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
        inbox
            .import(&registry, &proposal_json)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_list_proposals(
    inbox: State<'_, Arc<IdeProposalInbox>>,
) -> Result<Vec<ProposalSummary>, String> {
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox.list().map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
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
        inbox
            .detail(&registry, &inbox_id)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
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
        inbox
            .decide(&registry, &inbox_id, &expected_digest, approve_for_staging)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
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
        inbox
            .staging(&registry, &inbox_id, &expected_digest)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_discard_proposal(
    inbox: State<'_, Arc<IdeProposalInbox>>,
    inbox_id: String,
) -> Result<Vec<ProposalSummary>, String> {
    let inbox = Arc::clone(inbox.inner());
    tauri::async_runtime::spawn_blocking(move || {
        inbox
            .discard(&inbox_id)
            .map_err(|error| error.code().to_string())?;
        inbox.list().map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "INBOX_TASK_UNAVAILABLE".to_owned())?
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
            let drafts = lu_ide_workspace::draft_vault::DraftVault::open(
                data.join("private-draft-recovery"),
            )
            .map_err(|_| std::io::Error::other("IDE private draft vault unavailable"))?;
            app.manage(Arc::new(drafts));
            let navigation = lu_ide_workspace::workspace_layout::WorkspaceLayoutStore::open(
                data.join("private-workspace-navigation"),
            )
            .map_err(|_| std::io::Error::other("IDE private navigation unavailable"))?;
            app.manage(Arc::new(navigation));
            app.manage(Arc::new(PartialStageSessions::default()));
            app.manage(Arc::new(registry));
            let snapshots = InventorySnapshots::new(data.join("evidence-baselines.json"))
                .map_err(|_| std::io::Error::other("IDE evidence history failed to open"))?;
            let workflows = lu_ide_workspace::workflow_orchestrator::WorkflowCoordinator::new(
                data.join("workflow-dependencies.json"),
            )
            .map_err(|_| std::io::Error::other("IDE durable workflow journal failed to open"))?;
            app.manage(Arc::new(workflows));
            let intents =
                ExecutionIntents::new(data.join("execution-intents.jsonl")).map_err(|_| {
                    std::io::Error::other("IDE execution-intent journal failed to open")
                })?;
            app.manage(Arc::new(intents));
            app.manage(Arc::new(ledger));
            app.manage(Arc::new(snapshots));
            app.manage(Arc::new(IdeProposalInbox::default()));
            #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
            match build_backend::NativeBuildRuntime::new(app.handle()) {
                Ok(native) => {
                    app.manage(Arc::new(native));
                }
                Err(code) => eprintln!("IDE protected build/test unavailable: {code}"),
            }
            #[cfg(all(feature = "native-debug", any(windows, target_os = "linux")))]
            match debug_backend::NativeDebugRuntime::new(app.handle()) {
                Ok(native) => {
                    app.manage(Arc::new(native));
                }
                Err(code) => eprintln!("IDE protected debugger unavailable: {code}"),
            }
            #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
            match lsp_backend::NativeLspRuntime::new(app.handle()) {
                Ok(native) => {
                    app.manage(Arc::new(native));
                }
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
            ide_inspect_package_managers,
            ide_preview_package_operation,
            ide_revalidate_package_operation,
            ide_discover_conventional_tests,
            ide_review_project_debug_target,
            ide_review_multilang_adapter,
            ide_build_status,
            ide_build_read_output,
            ide_build_start,
            ide_package_run_reviewed_uv,
            ide_package_discard_private_output,
            ide_build_cancel,
            ide_build_reconcile,
            ide_debug_status,
            ide_debug_run_fixture,
            ide_debug_run_project,
            ide_debug_step_project,
            ide_debug_breakpoint_project,
            ide_debug_inspect_project_variables,
            ide_debug_cancel,
            ide_debug_reconcile,
            ide_read_project_context,
            ide_record_task_update,
            ide_link_task_source,
            ide_observe_task_ci,
            ide_read_git_status,
            ide_read_git_repository_inventory,
            ide_read_git_file_history,
            ide_read_git_refs_health,
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
            ide_git_preview_batch,
            ide_git_verify_batch_review,
            ide_git_preview_hunks,
            ide_git_select_reviewed_hunks,
            ide_git_stage_selected_hunks,
            ide_git_cancel_partial_stage,
            ide_git_partial_stage_recovery,
            ide_git_reconcile_partial_stage,
            ide_git_restore_reviewed_stage_receipt,
            ide_git_preview_reviewed_commit,
            ide_git_commit_selected_hunks,
            ide_git_amend_reviewed_commit,
            ide_git_cancel_reviewed_commit,
            ide_git_inspect_commit_recovery,
            ide_git_reconcile_commit_receipt,
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
