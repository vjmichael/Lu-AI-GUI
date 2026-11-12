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
mod terminal_broker;
#[cfg(all(any(windows, target_os = "linux"), any(feature = "native-build", feature = "native-debug", feature = "native-lsp")))]
mod global_native_custody;
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
use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};

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


#[derive(Default)]
struct Fin05ManifestMutationGate(Mutex<()>);
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

/// G06C-01: registered-project-only read-only local branch operation review.
/// The enum is typed; branch names are validated in the native host. No
/// operation executor, OS approval, Git CLI, remote or ref-write capability.
#[tauri::command]
async fn ide_git_review_branch_operation(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    operation: lu_ide_workspace::git_branch_operations::BranchOperationKind,
    selected_branch: Option<String>,
) -> Result<lu_ide_workspace::git_branch_operations::BranchOperationReview, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .review_local_branch_operation(&project_id, operation, selected_branch.as_deref())
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06C_REVIEW_HOST_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_read_local_stash_inventory(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<lu_ide_workspace::git_stash_inventory::StashInventory,String>{
 let registry=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
  registry.read_local_stash_metadata(&project_id))
  .await.map_err(|_|"FIN03_STASH_READ_THREAD_UNAVAILABLE".to_owned())?
  .map_err(str::to_owned)
}

/// FIN-03G: immutable native Git-object review. This does not grant
/// OS authority, apply or drop a stash, revert a commit, or alter local refs.
#[tauri::command]
async fn ide_git_preview_local_mutation_impact(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,
 project_id:String,
 kind:lu_ide_workspace::git_fin03_operation_preview::Fin03ImpactKind,
 commit_sha:Option<String>,stash_index:Option<usize>,
)->Result<lu_ide_workspace::git_fin03_operation_preview::Fin03ImpactReview,String>{
 let registry=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
  registry.review_fin03_local_impact(
   &project_id,kind,commit_sha.as_deref(),stash_index))
  .await.map_err(|_|"FIN03G_REVIEW_TASK_INTERRUPTED".to_owned())?
  .map_err(str::to_owned)
}

/// FIN-03: one reviewed staged file -> unstaged source, Git index ONLY.
/// No caller-provided Git commands, paths, OS approval tokens or ref writes.
#[tauri::command]
async fn ide_git_review_single_unstage(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<lu_ide_workspace::git_reviewed_unstage::ReviewedUnstage,String>{
 let registry=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
    registry.review_single_staged_unstage(&project_id))
    .await.map_err(|_|"FIN03_UNSTAGE_REVIEW_THREAD_INTERRUPTED".to_owned())?
    .map_err(str::to_owned)
}
#[tauri::command]
async fn ide_git_execute_single_unstage(
 app:AppHandle,state:State<'_,Arc<IdeWorkspaceRegistry>>,
 sessions:State<'_,Arc<PartialStageSessions>>,
 expected:lu_ide_workspace::git_reviewed_unstage::ReviewedUnstage,
)->Result<lu_ide_workspace::git_reviewed_unstage::UnstageReceipt,String>{
 let registry=Arc::clone(state.inner());
 let sessions=Arc::clone(sessions.inner());
 let project_id=expected.project_id.clone();
 let cancel=sessions.begin(&project_id)?;
 let result=tauri::async_runtime::spawn_blocking(move||{
    let fresh=registry.review_single_staged_unstage(&project_id)
       .map_err(str::to_owned)?;
    if fresh.review_sha256!=expected.review_sha256 ||
        fresh.index_sha256!=expected.index_sha256 ||
        fresh.relative_path!=expected.relative_path ||
        fresh.head_sha!=expected.head_sha ||
        expected.operation_authorized || !expected.source_rechecked ||
        expected.refs_modified || expected.source_files_modified {
        return Err("FIN03_UNSTAGE_REVIEW_CHANGED".into());
    }
    if cancel.load(Ordering::SeqCst)!=0{
        return Err("FIN03_UNSTAGE_CANCELLED_PRE_CONSENT".into());
    }
    let yes=app.dialog().message(format!(
       "Unstage exactly ONE reviewed file?\n\nProject: {}\nFile: {}\nHEAD: {}\nIndex SHA: {}\nReview SHA: {}\n\nOnly Git INDEX changes. Worktree/source and branch refs will not be altered.",
       project_id,fresh.relative_path,fresh.head_sha,
       fresh.index_sha256,fresh.review_sha256))
       .title("Lu IDE · One-use native unstage")
       .buttons(MessageDialogButtons::OkCancelCustom(
           "Unstage reviewed file".into(),"Deny".into()))
       .blocking_show();
    if !yes || cancel.load(Ordering::SeqCst)!=0{
        return Err("FIN03_UNSTAGE_HUMAN_DENIED".into());
    }
    registry.execute_approved_single_unstage(&fresh,&cancel)
        .map_err(str::to_owned)
 }).await.map_err(|_|"FIN03_UNSTAGE_HOST_INTERRUPTED".to_owned());
 sessions.end(&project_id);
 result?
}

#[tauri::command]
async fn ide_git_review_one_file_stash(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<lu_ide_workspace::git_reviewed_stash_save::ReviewedStashSave,String>{
 let reg=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
  reg.review_one_file_stash_save(&project_id))
  .await.map_err(|_|"FIN03_STASH_REVIEW_TASK_INTERRUPTED".to_owned())?
  .map_err(str::to_owned)
}
#[tauri::command]
async fn ide_git_execute_one_file_stash(
 app:AppHandle,state:State<'_,Arc<IdeWorkspaceRegistry>>,
 sessions:State<'_,Arc<PartialStageSessions>>,
 project_id:String,expected_review_sha256:String,
)->Result<lu_ide_workspace::git_reviewed_stash_save::ReviewedStashReceipt,String>{
 if !cfg!(feature="fin03-reviewed-stash"){
  return Err("FIN03_STASH_NATIVE_ACCEPTANCE_REQUIRED".into());
 }
 let registry=Arc::clone(state.inner());
 let sessions=Arc::clone(sessions.inner());
 let cancel=sessions.begin(&project_id)?;
 let project_for_session=project_id.clone();
 let result=tauri::async_runtime::spawn_blocking(move||{
  let reviewed=registry.review_one_file_stash_save(&project_id)
   .map_err(str::to_owned)?;
  if reviewed.review_sha256!=expected_review_sha256 ||
     cancel.load(Ordering::SeqCst)!=0{
   return Err("FIN03_STASH_REVIEW_CHANGED".into());
  }
  let confirmed=app.dialog().message(format!(
   "Save exactly ONE reviewed tracked modification in the local Git stash?\n\nProject: {}\nPath: {}\nReview SHA-256: {}\n\nThis changes refs/stash and resets the reviewed file in the working tree. Original bytes are preserved privately. No remote action or automatic recovery is permitted.",
   project_id,reviewed.relative_path,reviewed.review_sha256))
   .title("Lu IDE · One-use protected stash save")
   .buttons(MessageDialogButtons::OkCancelCustom(
     "Save reviewed stash".into(),"Decline".into())).blocking_show();
  if !confirmed || cancel.load(Ordering::SeqCst)!=0{
   return Err("FIN03_STASH_HUMAN_DENIED".into());
  }
  registry.execute_reviewed_one_file_stash_save(&reviewed,&cancel)
   .map_err(str::to_owned)
 }).await.map_err(|_|"FIN03_STASH_NATIVE_TASK_INTERRUPTED".to_owned());
 sessions.end(&project_for_session);
 result?
}
#[tauri::command]
async fn ide_git_one_file_stash_recovery(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<bool,String>{
 let reg=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
  reg.one_file_stash_recovery_required(&project_id))
  .await.map_err(|_|"FIN03_STASH_RECOVERY_TASK_INTERRUPTED".to_owned())?
  .map_err(str::to_owned)
}

#[tauri::command]
async fn ide_git_review_single_file_restore(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<lu_ide_workspace::git_reviewed_restore::SingleFileRestoreReview,String>{
 let registry=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
  registry.review_single_file_restore(&project_id))
  .await.map_err(|_|"FIN03_RESTORE_REVIEW_THREAD_INTERRUPTED".to_owned())?
  .map_err(str::to_owned)
}
#[tauri::command]
async fn ide_git_execute_single_file_restore(
 app:AppHandle,state:State<'_,Arc<IdeWorkspaceRegistry>>,
 sessions:State<'_,Arc<PartialStageSessions>>,
 expected:lu_ide_workspace::git_reviewed_restore::SingleFileRestoreReview,
)->Result<lu_ide_workspace::git_reviewed_restore::SingleFileRestoreReceipt,String>{
 if !cfg!(feature="fin03-reviewed-restore") {
  return Err("FIN03_RESTORE_OPT_IN_AND_NATIVE_ACCEPTANCE_REQUIRED".into());
 }
 let registry=Arc::clone(state.inner());
 let sessions=Arc::clone(sessions.inner());
 let id=expected.project_id.clone();
 let cancel=sessions.begin(&id)?;
 let result=tauri::async_runtime::spawn_blocking(move||{
  let fresh=registry.review_single_file_restore(&id).map_err(str::to_owned)?;
  if fresh.review_sha256!=expected.review_sha256 ||
     expected.operation_authorized || expected.source_files_modified ||
     expected.refs_modified || cancel.load(Ordering::SeqCst)!=0 {
    return Err("FIN03_RESTORE_REVIEW_CHANGED".into());
  }
  let yes=app.dialog().message(format!(
   "Restore the original HEAD contents of ONE file?\\n\\nProject: {}\\nFile: {}\\nOriginal SHA: {}\\nHEAD SHA: {}\\nReview SHA: {}\\n\\nYour modified original bytes will be preserved in the local .git directory. This changes the worktree; no Git ref or remote is changed.",
   id,fresh.relative_path,fresh.original_sha256,fresh.head_sha,fresh.review_sha256))
   .title("Lu IDE · One-use native file restore")
   .buttons(MessageDialogButtons::OkCancelCustom(
    "Restore reviewed file".into(),"Deny".into())).blocking_show();
  if !yes || cancel.load(Ordering::SeqCst)!=0{
    return Err("FIN03_RESTORE_HUMAN_DENIED".into());
  }
  registry.execute_approved_single_file_restore(&fresh,&cancel).map_err(str::to_owned)
 }).await.map_err(|_|"FIN03_RESTORE_HOST_INTERRUPTED".to_owned());
 sessions.end(&id);
 result?
}
#[tauri::command]
async fn ide_git_single_file_restore_recovery(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<bool,String>{
 let registry=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
  registry.single_file_restore_recovery_required(&project_id))
  .await.map_err(|_|"FIN03_RESTORE_RECOVERY_READ_INTERRUPTED".to_owned())?
  .map_err(str::to_owned)
}

#[tauri::command]
async fn ide_git_inspect_unstage_recovery(
 state:State<'_,Arc<IdeWorkspaceRegistry>>,project_id:String,
)->Result<Option<&'static str>,String>{
 let registry=Arc::clone(state.inner());
 tauri::async_runtime::spawn_blocking(move||
   registry.inspect_unstage_interrupted(&project_id))
   .await.map_err(|_|"FIN03_UNSTAGE_RECOVERY_READ_INTERRUPTED".to_owned())?
   .map_err(str::to_owned)
}
#[tauri::command]
async fn ide_git_reconcile_unstage_recovery(
 app:AppHandle,state:State<'_,Arc<IdeWorkspaceRegistry>>,
 sessions:State<'_,Arc<PartialStageSessions>>,project_id:String,
)->Result<&'static str,String>{
 let registry=Arc::clone(state.inner());
 let sessions=Arc::clone(sessions.inner());
 let token=sessions.begin(&project_id)?;
 let result=tauri::async_runtime::spawn_blocking(move||{
   let state=registry.inspect_unstage_interrupted(&project_id)
       .map_err(str::to_owned)?;
   if state!=Some("manual_reconciliation_required") ||
      token.load(Ordering::SeqCst)!=0{
      return Err("FIN03_UNSTAGE_RECOVERY_NOT_REQUIRED".into());
   }
   let approved=app.dialog().message(format!(
      "Inspect the interrupted one-file Git unstage for project {}?\n\nNo Git index write, replay or rollback is permitted. Only existing on-disk evidence can be classified.",project_id))
      .title("Lu IDE · Recover interrupted unstage")
      .buttons(MessageDialogButtons::OkCancelCustom(
        "Classify original Git evidence".into(),"Decline".into()))
      .blocking_show();
   if !approved || token.load(Ordering::SeqCst)!=0{
      return Err("FIN03_UNSTAGE_RECOVERY_HUMAN_DENIED".into());
   }
   registry.reconcile_unstage_interrupted(&project_id)
      .map_err(str::to_owned)
 }).await.map_err(|_|"FIN03_UNSTAGE_RECOVERY_THREAD_INTERRUPTED".to_owned());
 sessions.end(&project_id);
 result?
}
/// G06C-02: read-only merge plan. No ref writes or user-approved grant.
#[tauri::command]
async fn ide_git_preview_local_merge(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    selected_branch: String,
) -> Result<lu_ide_workspace::git_merge_plan::ReviewedMergePlan, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .preview_reviewed_local_merge(&project_id, &selected_branch)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06C02_MERGE_PREVIEW_HOST_UNAVAILABLE".to_owned())?
}

/// 6D-01: inspect REAL unresolved stage 1/2/3 entries in the Git index.
/// Read-only; no stage, checkout, resolution, commit or authorization route.
#[tauri::command]
async fn ide_git_inspect_native_conflicts(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_conflict_inventory::NativeConflictInventory, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_native_git_conflicts(&project_id).map_err(str::to_owned)
    }).await.map_err(|_| "G06D01_NATIVE_INSPECTION_HOST_UNAVAILABLE".to_owned())?
}

/// 6D-03: source-bound native proposal, explicitly NO execution authority.
#[tauri::command]
async fn ide_git_review_conflict_resolution(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_path: String,
    inventory_sha256: String,
    index_sha256: String,
    candidate_text: String,
) -> Result<lu_ide_workspace::git_conflict_resolution::ConflictResolutionReview, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || registry
        .review_native_conflict_resolution(&project_id, &relative_path,
            &inventory_sha256, &index_sha256, &candidate_text)
        .map_err(str::to_owned))
        .await.map_err(|_| "G06D03_REVIEW_HOST_INTERRUPTED".to_owned())?
}

/// 6D-04: protected application requires a FRESH separate native OS dialog.
/// A WebView review digest is only a stale-data claim, not authorization.
#[tauri::command]
async fn ide_git_apply_conflict_resolution(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
    relative_path: String,
    inventory_sha256: String,
    index_sha256: String,
    candidate_text: String,
    review_sha256: String,
) -> Result<lu_ide_workspace::git_conflict_resolution::ProtectedResolutionReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let cancel = sessions.begin(&project_id)?;
    let id_for_task = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let reviewed = registry.review_native_conflict_resolution(
            &id_for_task, &relative_path, &inventory_sha256,
            &index_sha256, &candidate_text).map_err(str::to_owned)?;
        if reviewed.review_sha256 != review_sha256 {
            return Err("G06D04_REVIEW_STALE_BEFORE_DIALOG".to_owned());
        }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06D04_CANCELLED_BEFORE_DIALOG".to_owned());
        }
        let approved = app.dialog().message(format!(
            "APPLY EXACT CONFLICT RESOLUTION TO ONE LOCAL FILE?\n\nProject: {}\nFile: {}\nCurrent Git HEAD: {}\nOriginal index: {}\nProposed content SHA-256: {}\nCandidate bytes: {}\nReviewed evidence: {}\n\nThis separately authorized, one-use action replaces this tracked worktree file and stages its resolved blob into the Git index. It does NOT create a commit, merge, update a branch ref, run Git hooks or contact a remote. A crash between file replacement and index publication requires manual repair. Cancellation is possible only before mutation. Inspect the proposed text before accepting.",
            id_for_task, reviewed.relative_path, reviewed.current_head,
            reviewed.original_index_sha256, reviewed.proposed_content_sha256,
            reviewed.proposed_bytes, reviewed.review_sha256,
        )).title("Lu IDE · Separate native one-use conflict resolution consent")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Apply this reviewed file resolution".into(), "Deny / Cancel".into(),
          )).blocking_show();
        if !approved { return Err("G06D04_NATIVE_HUMAN_REFUSED".to_owned()); }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06D04_CANCELLED_AFTER_DIALOG".to_owned());
        }
        registry.apply_os_approved_conflict_resolution(
            &id_for_task, &relative_path, &inventory_sha256,
            &index_sha256, &candidate_text, &review_sha256, &cancel)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06D04_NATIVE_APPLY_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

#[tauri::command]
async fn ide_git_inspect_conflict_recovery(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_conflict_resolution::ProtectedResolutionRecovery, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move ||
        registry.inspect_protected_conflict_recovery(&project_id).map_err(str::to_owned))
        .await.map_err(|_| "G06D04_RECOVERY_HOST_INTERRUPTED".to_owned())?
}

/// Receipt-only reconciliation: separate OS consent, no Git writes or retry.
#[tauri::command]
async fn ide_git_reconcile_conflict_receipt(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_conflict_resolution::ProtectedResolutionRecovery, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let _single_use = sessions.begin(&project_id)?;
    let id_for_task = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let review = registry.inspect_protected_conflict_recovery(&id_for_task)
            .map_err(str::to_owned)?;
        if !matches!(review.state, "resolved_requires_independent_receipt" |
            "unpublished_original_source_preserved") {
            return Err("G06D04_RECOVERY_MANUAL_REPAIR_REQUIRED".to_owned());
        }
        let approved = app.dialog().message(format!(
            "INDEPENDENT CONFLICT RESOLUTION RECOVERY RECEIPT\n\nProject: {}\nPath: {}\nOperation: {}\nObserved state: {}\n\nRecord a receipt only after rereading original Git HEAD, index and worktree? This does NOT apply the resolution, retry the stage, delete Git locks, replace a file, or change any branch.",
            id_for_task, review.relative_path, review.operation_id, review.state,
        )).title("Lu IDE · Separate native receipt-only reconciliation")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Independently verify and record receipt".into(), "Deny / Cancel".into(),
          )).blocking_show();
        if !approved { return Err("G06D04_RECOVERY_HUMAN_REFUSED".to_owned()); }
        registry.reconcile_protected_conflict_receipt(&id_for_task)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06D04_RECOVERY_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

/// 6C-03B: actual native three-way merge preview, with new blobs confined
/// to a temporary isolated ODB. This deliberately has NO publication route.
#[tauri::command]
async fn ide_git_verify_divergent_merge(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    selected_branch: String,
    evidence_sha256: String,
) -> Result<lu_ide_workspace::git_divergent_verify::VerifiedDivergentMerge, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.verify_divergent_merge_result(
            &project_id, &selected_branch, &evidence_sha256,
        ).map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06C03B_PREVIEW_HOST_UNAVAILABLE".to_owned())?
}

/// 6C-03B: distinct one-use native OS authorization to publish a verified
/// two-parent local merge. The WebView supplies ONLY source-bound review data.
#[tauri::command]
async fn ide_git_execute_divergent_merge(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
    selected_branch: String,
    evidence_sha256: String,
    result_sha256: String,
) -> Result<lu_ide_workspace::git_protected_divergent_merge::ApprovedDivergentMergeReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let cancel = sessions.begin(&project_id)?;
    let selected_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let verified = registry.revalidate_protected_divergent_merge(
            &selected_id, &selected_branch, &evidence_sha256, &result_sha256,
        ).map_err(str::to_owned)?;
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C03B_CANCELLED_BEFORE_DIALOG".into());
        }
        let approved = app.dialog().message(format!(
            "PUBLISH A NEW TWO-PARENT LOCAL MERGE COMMIT?\\n\\nProject: {}\\nSelected local branch: {}\\nFirst parent/current HEAD: {}\\nSecond parent/selected tip: {}\\nUnique base: {}\\nVerified merge tree: {}\\nMerge result SHA-256: {}\\nSource plan SHA-256: {}\\n\\nThis operation writes local Git objects, changes tracked work files and the index, then advances the current local branch using exact old-HEAD compare-and-swap. A crash before the ref update can require manual Git repair. A crash after publication needs separate receipt-only approval. This approval is ONE USE, specific to these exact parents and verified tree. No hooks, Git CLI, remote push or network access. Deny if another Git process is active.",
            selected_id, selected_branch, verified.first_parent,
            verified.second_parent, verified.merge_base,
            verified.prospective_tree.as_deref().unwrap_or("NONE"),
            verified.result_sha256, verified.source_plan_sha256,
        )).title("Lu IDE · Separate native two-parent divergent merge consent")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Publish this exact two-parent merge".into(), "Deny / Cancel".into(),
          )).blocking_show();
        if !approved { return Err("G06C03B_HUMAN_REFUSED".into()); }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C03B_CANCELLED_AFTER_DIALOG".into());
        }
        registry.execute_os_approved_divergent_merge(
            &selected_id, &selected_branch, &evidence_sha256, &result_sha256, &cancel,
        ).map_err(str::to_owned)
    }).await.map_err(|_| "G06C03B_NATIVE_EXECUTION_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

#[tauri::command]
async fn ide_git_inspect_divergent_recovery(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_protected_divergent_merge::DivergentMergeRecovery, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_protected_divergent_recovery(&project_id).map_err(str::to_owned)
    }).await.map_err(|_| "G06C03B_RECOVERY_HOST_UNAVAILABLE".to_owned())?
}

#[tauri::command]
async fn ide_git_reconcile_divergent_receipt(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_protected_divergent_merge::DivergentMergeRecovery, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let _lock = sessions.begin(&project_id)?;
    let selected_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let observed = registry.inspect_protected_divergent_recovery(&selected_id)
            .map_err(str::to_owned)?;
        if !observed.operator_reconciliation_required { return Ok(observed); }
        if observed.foreign_index_lock_present || !matches!(observed.state,
            "published_requires_independent_receipt" |
            "unpublished_no_index_change_needs_review") {
            return Err("G06C03B_PHYSICAL_MANUAL_REPAIR_REQUIRED".into());
        }
        let approved = app.dialog().message(format!(
            "INDEPENDENT DIVERGENT MERGE RECOVERY RECEIPT\\n\\nProject: {}\\nObserved state: {}\\nJournal phase: {}\\n\\nRe-read the original and candidate Git objects, both parents, branch ref, index and worktree, and append ONLY a receipt? This will never replay the merge, modify files, write refs, delete Git locks or silently repair an interrupted checkout.",
            selected_id, observed.state, observed.phase,
        )).title("Lu IDE · Separate native receipt-only reconciliation")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Verify and record receipt only".into(), "Deny / Cancel".into(),
          )).blocking_show();
        if !approved { return Err("G06C03B_RECOVERY_HUMAN_REFUSED".into()); }
        registry.reconcile_protected_divergent_receipt(&selected_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06C03B_RECOVERY_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

/// 6C-03A native one-use OS-confirmed local fast-forward. The frontend
/// supplies only selection and a claim to exact source evidence, NEVER a grant.
/// A divergent merge is intentionally unavailable until its output is verified.
#[tauri::command]
async fn ide_git_execute_fast_forward(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
    selected_branch: String,
    evidence_sha256: String,
) -> Result<lu_ide_workspace::git_protected_merge::ProtectedFastForwardReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let cancel = sessions.begin(&project_id)?;
    let selected_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let fresh = registry.revalidate_protected_fast_forward(
            &selected_id, &selected_branch, &evidence_sha256,
        ).map_err(str::to_owned)?;
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C03_CANCELLED_BEFORE_CONSENT".into());
        }
        let approved = app.dialog().message(format!(
            "FAST-FORWARD LOCAL BRANCH '{}' to the exact tip of '{}'?\n\nProject ID: {}\nPrevious HEAD: {}\nNew HEAD: {}\nCurrent tree: {}\nSelected tree: {}\nOriginal index SHA-256: {}\nMerge review SHA-256: {}\nChanged paths: {}\n\nThis one-use operation will safely change the local Git INDEX and working files BEFORE atomically advancing the branch ref. A crash between checkout and ref update can require manual repair. Cancellation is available only BEFORE checkout begins. No hooks, command, push or network operation. Deny if another Git process is running.",
            fresh.current_branch, selected_branch, selected_id,
            fresh.current_head, fresh.selected_tip, fresh.current_tree,
            fresh.selected_tree, fresh.index_sha256, fresh.evidence_sha256,
            fresh.changed_path_count
        )).title("Lu IDE · One-use protected local fast-forward authorization")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Safely fast-forward this exact local branch".into(),
            "Deny / Cancel".into(),
          )).blocking_show();
        if !approved { return Err("G06C03_HUMAN_DENIED".into()); }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C03_CANCELLED_AFTER_DIALOG".into());
        }
        registry.execute_os_approved_fast_forward(
            &selected_id, &selected_branch, &fresh.evidence_sha256, &cancel,
        ).map_err(str::to_owned)
    }).await.map_err(|_| "G06C03_EXECUTION_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

/// Cancels only while the native operation is still before its initial
/// index/worktree checkout intent. An already started checkout is irreversible
/// from this endpoint; interrupted effects require separately reviewed recovery.
#[tauri::command]
async fn ide_git_cancel_protected_merge(
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<&'static str, String> {
    sessions.cancel(&project_id)
}

#[tauri::command]
async fn ide_git_inspect_protected_merge_recovery(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_protected_merge::ProtectedMergeRecovery, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_protected_merge_recovery(&project_id).map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06C03_RECOVERY_HOST_UNAVAILABLE".to_owned())?
}

/// Separate native OS consent to record an independently verified historical
/// receipt. This never retries a merge, deletes Git locks or edits work files.
#[tauri::command]
async fn ide_git_reconcile_protected_merge_receipt(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_protected_merge::ProtectedMergeRecovery, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let _token = sessions.begin(&project_id)?;
    let selected_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let observed = registry.inspect_protected_merge_recovery(&selected_id)
            .map_err(str::to_owned)?;
        if !observed.operator_reconciliation_required {
            return Ok(observed);
        }
        if observed.git_index_lock_present {
            return Err("G06C03_RECOVERY_LOCK_REQUIRES_MANUAL_REVIEW".into());
        }
        if !matches!(observed.state,
            "published_ref_requires_operator_receipt"
            | "prepublication_no_index_change_pending_review") {
            return Err("G06C03_RECOVERY_REQUIRES_MANUAL_GIT_REPAIR".into());
        }
        let approved = app.dialog().message(format!(
            "REVIEW interrupted local merge for project {}?\n\nObserved: {}\nJournal phase: {}\n\nLu will re-open the exact native Git repository and verify its ref, index, worktree and saved journal before recording a terminal receipt. This will NOT rerun checkout, rewrite a branch, delete any Git lock, restore files or contact a remote.",
            selected_id, observed.state, observed.last_phase
        )).title("Lu IDE · Independent protected merge receipt reconciliation")
          .buttons(MessageDialogButtons::OkCancelCustom(
            "Verify and record receipt only".into(),
            "Deny / Cancel".into(),
          )).blocking_show();
        if !approved {
            return Err("G06C03_RECOVERY_HUMAN_DENIED".into());
        }
        registry.reconcile_protected_merge_receipt(&selected_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06C03_RECOVERY_HOST_INTERRUPTED".to_owned());
    sessions.end(&project_id);
    result?
}

/// 6C-04A: strictly read-only commit graph and original change identity.
/// No replay, no OS approval, no local index or ref change, and no remote.
#[tauri::command]
async fn ide_git_review_rebase_cherry_pick(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    operation: lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind,
    selected_branch: String,
) -> Result<lu_ide_workspace::git_rebase_cherry_plan::ReviewedReplayPlan, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.review_local_rebase_cherry_pick(&project_id, operation, &selected_branch)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06C04_REPLAY_REVIEW_HOST_INTERRUPTED".to_owned())?
}

/// G06C-04B: build prospective commits ONLY in disposable bare Git ODB.
#[tauri::command]
async fn ide_git_preview_isolated_replay(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    operation: lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind,
    selected_branch: String,
) -> Result<lu_ide_workspace::git_replay_quarantine::ProspectiveReplayResult, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.preview_prospective_replay(&project_id, operation, &selected_branch)
            .map_err(str::to_owned)
    })
    .await
    .map_err(|_| "G06C04B_QUARANTINE_REVIEW_HOST_INTERRUPTED".to_owned())?
}

/// No page or AI model can issue these one-use grants: an independent
/// blocking operating-system dialog re-validates exact Git refs/results.
async fn g06c04_execute_with_native_consent(
    app: AppHandle,
    registry: Arc<IdeWorkspaceRegistry>,
    sessions: Arc<PartialStageSessions>,
    project_id: String,
    selected_branch: String,
    evidence_sha256: String,
    operation: lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind,
) -> Result<lu_ide_workspace::git_protected_replay::ApprovedReplayReceipt, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let cancel = sessions.begin(&project_id)?;
    let session_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let plan = registry.revalidate_protected_replay(
            &project_id, operation, &selected_branch, &evidence_sha256,
        ).map_err(str::to_owned)?;
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C04_CANCELLED_BEFORE_OS_CONSENT".into());
        }
        let new_head = plan.prospective_head.as_deref().ok_or("G06C04_MISSING_HEAD")?;
        let new_tree = plan.prospective_tree.as_deref().ok_or("G06C04_MISSING_TREE")?;
        let (caption, warning, action) = match operation {
            lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind::CherryPickLocalTip => (
                "Lu IDE · One-use local cherry-pick consent",
                "CHERRY-PICK will append a NEW local commit with original HEAD as parent. This is NOT a rebase grant. No remote push.",
                "Authorize this exact cherry-pick only",
            ),
            lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind::RebaseCurrentOntoLocal => (
                "Lu IDE · EXTRA history-rewrite consent for local rebase",
                "WARNING: REBASE REWRITES CURRENT LOCAL BRANCH HISTORY. Its previous tip will be preserved under a dedicated refs/lu-ide/g06c04/original/* ref before checkout. This does NOT authorize force-push or any remote rewrite.",
                "Authorize local history rewrite only",
            ),
        };
        let approved = app.dialog().message(format!(
            "{warning}\n\nProject ID: {project_id}\nOriginal current HEAD: {}\nSelected local branch: {selected_branch}\nExact selected tip: {}\nNew resulting HEAD: {new_head}\nNew tree: {new_tree}\nResult SHA-256: {}\nChanged paths: {}\nOriginal commits: {}\n\nThe Git working tree/index changes before the branch-ref CAS. Interruption may require manual recovery; cancellation is possible only BEFORE object publication. OS consent is one use and valid only for this reviewed evidence.",
            plan.current_head, plan.selected_tip, plan.evidence_sha256,
            plan.candidate_change_paths, plan.result_steps.len(),
        )).title(caption)
          .buttons(MessageDialogButtons::OkCancelCustom(
            action.into(), "Deny / Cancel".into()
          )).blocking_show();
        if !approved { return Err("G06C04_HUMAN_DENIED".into()); }
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C04_CANCELLED_AFTER_OS_CONSENT".into());
        }
        registry.execute_os_approved_replay(
            &project_id, operation, &selected_branch, &plan.evidence_sha256, &cancel,
        ).map_err(str::to_owned)
    }).await.map_err(|_| "G06C04_OS_APPROVED_HOST_INTERRUPTED".to_owned());
    sessions.end(&session_id);
    result?
}

/// 6C-04C: append exactly the selected local branch-tip original commit.
#[tauri::command]
async fn ide_git_execute_cherry_pick(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
    selected_branch: String,
    evidence_sha256: String,
) -> Result<lu_ide_workspace::git_protected_replay::ApprovedReplayReceipt, String> {
    g06c04_execute_with_native_consent(
        app, Arc::clone(state.inner()), Arc::clone(sessions.inner()),
        project_id, selected_branch, evidence_sha256,
        lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind::CherryPickLocalTip,
    ).await
}
/// 6C-04D: separate native OS consent explicitly warns about history rewrite.
#[tauri::command]
async fn ide_git_execute_rebase(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
    selected_branch: String,
    evidence_sha256: String,
) -> Result<lu_ide_workspace::git_protected_replay::ApprovedReplayReceipt, String> {
    g06c04_execute_with_native_consent(
        app, Arc::clone(state.inner()), Arc::clone(sessions.inner()),
        project_id, selected_branch, evidence_sha256,
        lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind::RebaseCurrentOntoLocal,
    ).await
}

#[tauri::command]
async fn ide_git_cancel_protected_replay(
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<&'static str, String> {
    sessions.cancel(&project_id)
}

#[tauri::command]
async fn ide_git_inspect_replay_recovery(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_protected_replay::ReplayRecovery, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.inspect_protected_replay_recovery(&project_id)
            .map_err(str::to_owned)
    }).await.map_err(|_| "G06C04_REPLAY_RECOVERY_HOST_INTERRUPTED".to_owned())?
}

#[tauri::command]
async fn ide_git_reconcile_replay_receipt(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<PartialStageSessions>>,
    project_id: String,
) -> Result<lu_ide_workspace::git_protected_replay::ReplayRecovery, String> {
    use tauri_plugin_dialog::MessageDialogButtons;
    let registry = Arc::clone(state.inner());
    let sessions = Arc::clone(sessions.inner());
    let _cancel = sessions.begin(&project_id)?;
    let session_id = project_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let observed = registry.inspect_protected_replay_recovery(&project_id)
            .map_err(str::to_owned)?;
        if observed.state != "published_ref_requires_separate_receipt"
            || !observed.operator_reconciliation_required
            || observed.index_lock_present || !observed.backup_ref_verified {
            return Err("G06C04_RECONCILIATION_UNSAFE_STATE".into());
        }
        let approved = app.dialog().message(format!(
            "Independently verify historical {} replay receipt for project {}?\n\nOriginal HEAD: {}\nPublished HEAD: {}\nJournal phase: {}\n\nThis operation ONLY appends a verified receipt after native reread of exact branch, output tree/index, source branch and preserved original history. It NEVER reruns a replay, changes refs, deletes locks, checks out or rolls back files.",
            match observed.operation {
                Some(lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind::RebaseCurrentOntoLocal)
                    => "REBASE history rewrite",
                _ => "cherry-pick",
            },
            project_id, observed.original_head, observed.candidate_head, observed.last_phase,
        )).title("Lu IDE · Independent one-use replay recovery receipt consent")
          .buttons(MessageDialogButtons::OkCancelCustom(
              "Verify historical receipt only".into(), "Deny / Cancel".into(),
          )).blocking_show();
        if !approved { return Err("G06C04_RECONCILIATION_HUMAN_DENIED".into()); }
        registry.reconcile_protected_replay_receipt(&project_id).map_err(str::to_owned)
    }).await.map_err(|_| "G06C04_RECONCILIATION_HOST_INTERRUPTED".to_owned());
    sessions.end(&session_id);
    result?
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

/// 6E-01: observe only open public PR metadata for the exact local
/// GitHub origin and branch. A PR review never grants Git authority.
#[tauri::command]
async fn ide_read_github_pull_requests(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<lu_ide_workspace::github_pull_review::GitHubPullInspection, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move ||
        registry.inspect_github_pull_requests(&project_id))
        .await.map_err(|_| "G06E01_PR_REVIEW_HOST_UNAVAILABLE".to_owned())
}

/// FIN-03: one explicit, read-only, same-HEAD public review inspection.
#[tauri::command]
async fn ide_read_github_review_state(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    number: u64,
) -> Result<lu_ide_workspace::github_review_state::ReviewStateEvidence, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move ||
        registry.inspect_github_review_state(&project_id,number))
        .await.map_err(|_| "G06E02_REVIEW_HOST_UNAVAILABLE".to_owned())
}
/// FIN-03: public branch rule metadata is not complete policy or authorization.
#[tauri::command]
async fn ide_read_github_branch_rules(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,number:u64,
)->Result<lu_ide_workspace::github_branch_rules::BranchRulesEvidence,String>{
    let native=Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move||
        native.inspect_github_branch_rules(&project_id,number))
        .await.map_err(|_|"FIN03_RULE_INSPECTION_UNAVAILABLE".to_owned())
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
            "globalRecoveryRequired":false,"activeInAnotherProject":false,
            "orphanedDebugActionRequired":false,
            "operationControlSupported":false,
            "operationControlCode":"DEV046D_NATIVE_HUMAN_ACTION_APPROVAL_NOT_ACCEPTED",
            "code":"DEBUG_PROTECTED_ADAPTER_NOT_INSTALLED",
            "description":"Default IDE does not ship a native debugger.",
            "current":null, "last":null, "journal":[], "quarantined":[]
        }))
    }
}
/// FIN-04R: one explicit native OS operator decision, original on-disk
/// source/action/supervisor journal replay and independently re-queried
/// OS kernel tree. ONLY writes audit evidence of unknown interrupted
/// debugger outcome. No job restart, action dispatch, marker clearance
/// or WebView-provided process ID, command, witness, path or consent.
#[tauri::command]
async fn ide_debug_action_os_recovery(
    app:AppHandle,
    state:State<'_,Arc<IdeWorkspaceRegistry>>,
    project_id:String,
)->Result<serde_json::Value,String>{
    #[cfg(all(feature="native-debug",any(windows,target_os="linux")))]
    {
        state.trusted_native_worker_root(&project_id)
            .map_err(|_|"FIN04REC_PROJECT_NOT_REGISTERED")?;
        let runtime=app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
            .ok_or("FIN04REC_NATIVE_SUPERVISOR_UNAVAILABLE")?;
        let runtime=Arc::clone(runtime.inner());
        let result=tauri::async_runtime::spawn_blocking(move||{
            let original=runtime.inspect_control_evidence(&project_id)?;
            if original.original_action_files_checked==0 ||
                !original.global_unresolved_action_required {
                return Err("FIN04REC_NO_SELECTED_PROJECT_ORIGINAL_ACTION".into());
            }
            let approved=app.dialog()
                .message(format!(
                    "Independently inspect interrupted native debugger action OS custody?\n\n\
                    Registered project: {project_id}\n\
                    Original action files: {}\n\
                    Debug journal SHA-256: {}\n\
                    Action manifest SHA-256: {}\n\n\
                    This queries original Windows Job Objects or Linux cgroups,\
                    requires the old process owner to have exited, and writes\
                    one audit-only receipt if the original evidence is stable.\n\
                    No debug action will replay, no DAP command will be sent,\
                    no previous step will be declared successful, and the\
                    global admission marker will NOT be cleared.",
                    original.original_action_files_checked,
                    original.original_debug_journal_sha256,
                    original.action_manifest_sha256,
                ))
                .title("Lu IDE · OS-only debugger action recovery evidence")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Verify original OS custody only".into(),
                    "Decline".into(),
                ))
                .blocking_show();
            if !approved {
                return Err("FIN04REC_OPERATOR_DECLINED".into());
            }
            runtime.record_action_os_recovery(&project_id)
        }).await.map_err(|_|"FIN04REC_NATIVE_TASK_INTERRUPTED".to_owned())??;
        return serde_json::to_value(result)
            .map_err(|_|"FIN04REC_RECEIPT_SERIALIZATION_DENIED".into());
    }
    #[cfg(not(all(feature="native-debug",any(windows,target_os="linux"))))]
    {
        let _=(app,state,project_id);
        Err("FIN04REC_NATIVE_OS_SUPERVISOR_NOT_INSTALLED".into())
    }
}
/// FIN-04: read-only original debugger action evidence. This must never
/// accept an action token, OS witness boolean, PID, DAP command or path.
#[tauri::command]
async fn ide_debug_control_evidence(
 app:AppHandle,
 state:State<'_,Arc<IdeWorkspaceRegistry>>,
 project_id:String,
)->Result<serde_json::Value,String>{
 #[cfg(all(feature="native-debug",any(windows,target_os="linux")))]
 {
  state.trusted_native_worker_root(&project_id)
   .map_err(|_|"FIN04_PROJECT_NOT_REGISTERED_OR_CLOSED")?;
  let backend=app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
   .ok_or("FIN04_NATIVE_DEBUG_SUPERVISOR_UNAVAILABLE")?;
  let backend=Arc::clone(backend.inner());
  let inventory=tauri::async_runtime::spawn_blocking(move||
   backend.inspect_control_evidence(&project_id))
   .await.map_err(|_|"FIN04_ACTION_EVIDENCE_TASK_INTERRUPTED".to_owned())??;
  return serde_json::to_value(inventory)
   .map_err(|_|"FIN04_ACTION_EVIDENCE_SERIALIZATION_FAILED".into());
 }
 #[cfg(not(all(feature="native-debug",any(windows,target_os="linux"))))]
 {
  let _=(app,state,project_id);
  Err("FIN04_NATIVE_DEBUG_EVIDENCE_NOT_INSTALLED".into())
 }
}
/// FIN-04: read-only control candidate bound to an existing original
/// stopped-source review, never a live owner or executable adapter command.
#[tauri::command]
async fn ide_debug_review_control_candidate(
 app:AppHandle,state:State<'_,Arc<IdeWorkspaceRegistry>>,
 project_id:String,job_id:String,plan_sha256:String,
 kind:String,
)->Result<serde_json::Value,String>{
 #[cfg(all(feature="native-debug",any(windows,target_os="linux")))]
 {
  let kind=match kind.as_str(){
   "step_over"=>debug_backend::DebugControlReviewKind::StepOver,
   "step_into"=>debug_backend::DebugControlReviewKind::StepInto,
   "step_out"=>debug_backend::DebugControlReviewKind::StepOut,
   "breakpoint_at_reviewed_line"=>
     debug_backend::DebugControlReviewKind::BreakpointAtReviewedLine,
   "inspect_reviewed_frame"=>
     debug_backend::DebugControlReviewKind::InspectReviewedFrame,
   _=>return Err("FIN04_UNKNOWN_READ_ONLY_CONTROL_KIND".into()),
  };
  let registry=Arc::clone(state.inner());
  let runtime=app.try_state::<Arc<debug_backend::NativeDebugRuntime>>()
   .ok_or("FIN04_NATIVE_DEBUG_SUPERVISOR_UNAVAILABLE")?;
  let runtime=Arc::clone(runtime.inner());
  let review=tauri::async_runtime::spawn_blocking(move||
   runtime.review_control_candidate(
    &registry,&project_id,&job_id,&plan_sha256,kind))
    .await.map_err(|_|"FIN04_CONTROL_REVIEW_INTERRUPTED".to_owned())??;
  return serde_json::to_value(review)
   .map_err(|_|"FIN04_CONTROL_REVIEW_SERIALIZATION_FAILED".into());
 }
 #[cfg(not(all(feature="native-debug",any(windows,target_os="linux"))))]
 {
  let _=(app,state,project_id,job_id,plan_sha256,kind);
  Err("FIN04_NATIVE_DEBUG_CONTROL_REVIEW_UNAVAILABLE".into())
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
/// Read-only FIN01 cross-supervisor custody classification. Neither the
/// renderer nor this command can approve OS cleanup, remove a marker, or
/// replay an interrupted native job.
/// FIN-01D: separate one-use native OS consent. Never accept a renderer
/// authorization boolean, process ID, journal result or kernel witness.
/// This command only clears the exact stale admission marker after fresh
/// Windows Job Object or delegated Linux cgroup checks and durable receipts.
#[tauri::command]
async fn ide_fin01_clear_stale_marker(app:AppHandle)
 ->Result<serde_json::Value,String>{
 #[cfg(all(any(windows,target_os="linux"),
   any(feature="native-build",feature="native-debug",feature="native-lsp")))]
 {
  let profile=app.path().app_data_dir()
   .map_err(|_|"FIN01_PROFILE_UNAVAILABLE")?;
  return tauri::async_runtime::spawn_blocking(move||{
   let consent=app.dialog().message(
    "Reconcile the previous FIN-01 global admission marker?\n\nThis is NOT a new worker grant. The native host will re-read all original job journals, verify the previous owner exited, and query actual Windows Job Objects or the Linux delegated cgroup-v2 hierarchy. If anything is missing, busy or uncertain, the marker stays blocked. No project code will run or be retried.")
    .title("Lu IDE · Verify global OS custody")
    .buttons(MessageDialogButtons::OkCancelCustom(
      "Verify kernel and clear only if empty".into(),
      "Leave quarantined".into()))
    .blocking_show();
   if !consent {return Err("FIN01_GLOBAL_RECOVERY_NOT_APPROVED".to_owned());}
   let witness=crate::global_native_custody
    ::clear_stale_marker_with_kernel_proof(&profile)?;
   serde_json::to_value(witness)
    .map_err(|_|"FIN01_GLOBAL_KERNEL_RECEIPT_UNSERIALIZABLE".to_owned())
  }).await.map_err(|_|"FIN01_KERNEL_RECOVERY_TASK_INTERRUPTED".to_owned())?;
 }
 #[cfg(not(all(any(windows,target_os="linux"),
   any(feature="native-build",feature="native-debug",feature="native-lsp"))))]
 {
  let _=app;
  Err("FIN01_NATIVE_KERNEL_RECOVERY_UNAVAILABLE".into())
 }
}
#[tauri::command]
async fn ide_fin01_recovery_assessment(app:AppHandle)
 ->Result<serde_json::Value,String>{
 #[cfg(all(any(windows,target_os="linux"),any(feature="native-build",feature="native-debug",feature="native-lsp")))]
 {
  let profile=app.path().app_data_dir()
   .map_err(|_|"FIN01_PROFILE_UNAVAILABLE")?;
  return tauri::async_runtime::spawn_blocking(move||
    crate::global_native_custody::assess_global_recovery(&profile))
   .await.map_err(|_|"FIN01_RECOVERY_ASSESSMENT_INTERRUPTED".to_owned())?
   .and_then(|value|serde_json::to_value(value)
     .map_err(|_|"FIN01_RECOVERY_ASSESSMENT_SERIALIZATION_FAILED".to_owned()));
 }
 #[cfg(not(all(any(windows,target_os="linux"),any(feature="native-build",feature="native-debug",feature="native-lsp"))))]
 {
  let _=app;
  Err("FIN01_NATIVE_SUPERVISORS_NOT_INSTALLED".into())
 }
}

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
            "globalRecoveryRequired":false,
            "activeInAnotherProject":false,
            "globalWorkerBusy":false,
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

/// INT03: explicit project Build/Test profile execution. The renderer supplies
/// only a fixed recipe ID and the exact previously reviewed saved manifest
/// SHA-256; the native runner freezes and independently rechecks that identity,
/// then requires a NEW OS human confirmation for the original worker job.
/// INT03-E: read-only exact-job source review for a previously reported
/// failing test. No URI, source path, reporter text or navigation permission
/// comes from JavaScript. Only the native original in-memory result can
/// identify the source; OS receipts and current saved bytes are rechecked.
#[tauri::command]
async fn ide_build_review_test_target(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    job_id: String,
    plan_sha256: String,
    case_id: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("INT03E_NATIVE_EVIDENCE_UNAVAILABLE")?;
        let native = Arc::clone(runtime.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            native.review_test_case_target(
                &registry,&project_id,&job_id,&plan_sha256,&case_id)
        })
        .await
        .map_err(|_| "INT03E_REVIEW_THREAD_UNAVAILABLE")??;
        return serde_json::to_value(result)
            .map_err(|_| "INT03E_REVIEW_SERIALIZATION_UNAVAILABLE".into());
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _=(app,registry,project_id,job_id,plan_sha256,case_id);
        Err("INT03E_NATIVE_EVIDENCE_UNAVAILABLE".into())
    }
}

#[tauri::command]
async fn ide_build_run_profile(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    recipe_id: String,
    expected_manifest_sha256: String,
) -> Result<serde_json::Value, String> {
    #[cfg(all(feature = "native-build", any(windows, target_os = "linux")))]
    {
        let runtime = app
            .try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("INT03_PROTECTED_RUNTIME_UNAVAILABLE")?;
        let native = Arc::clone(runtime.inner());
        let registry = Arc::clone(registry.inner());
        let result = tauri::async_runtime::spawn_blocking(move || {
            native.run_profile(&app, &registry, &project_id,
                &recipe_id, &expected_manifest_sha256)
        })
        .await
        .map_err(|_| "INT03_NATIVE_EXECUTION_TASK_UNAVAILABLE")??;
        return serde_json::to_value(result).map_err(|_| "INT03_RECEIPT_SERIALIZATION_DENIED".into());
    }
    #[cfg(not(all(feature = "native-build", any(windows, target_os = "linux"))))]
    {
        let _ = (app, registry, project_id, recipe_id, expected_manifest_sha256);
        Err("INT03_PROTECTED_RUNTIME_UNAVAILABLE".into())
    }
}

/// FIN-04H: one exact saved Python/Node candidate, rechecked by native host
/// against the original frozen tree, release-pinned worker and a fresh OS grant.
/// CTest source macros do not grant executable exact-test authority.
#[tauri::command]
async fn ide_run_protected_framework_test_case(
    app:AppHandle,registry:State<'_,Arc<IdeWorkspaceRegistry>>,
    project_id:String,relative_path:String,case_name:String,
    expected_source_revision:String,expected_manifest_sha256:String
)->Result<serde_json::Value,String>{
    #[cfg(all(feature="native-build",feature="native-multilang",any(windows,target_os="linux")))]
    {
        let runtime=app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
            .ok_or("FIN04H_PROTECTED_BUILD_HOST_UNAVAILABLE")?;
        let runtime=Arc::clone(runtime.inner());
        let registry=Arc::clone(registry.inner());
        let result=tauri::async_runtime::spawn_blocking(move||
            runtime.run_selected_framework_case(&app,&registry,&project_id,
                &relative_path,&case_name,&expected_source_revision,
                &expected_manifest_sha256))
            .await.map_err(|_|"FIN04H_PROTECTED_TEST_THREAD_INTERRUPTED".to_owned())??;
        return serde_json::to_value(result)
            .map_err(|_|"FIN04H_TEST_RECEIPT_SERIALIZATION_DENIED".into());
    }
    #[cfg(not(all(feature="native-build",feature="native-multilang",any(windows,target_os="linux"))))]
    {
        let _=(app,registry,project_id,relative_path,case_name,
            expected_source_revision,expected_manifest_sha256);
        Err("FIN04H_OS_FRAMEWORK_WORKER_NOT_INSTALLED".into())
    }
}
/// FIN-04: verified saved test candidate, frozen source, genuine pinned Cargo
/// OS process, exact libtest name filter and a separate native user grant.
#[tauri::command]
async fn ide_run_protected_rust_test_case(
 app:AppHandle,registry:State<'_,Arc<IdeWorkspaceRegistry>>,
 project_id:String,relative_path:String,case_name:String,
 expected_source_revision:String,expected_manifest_sha256:String
)->Result<serde_json::Value,String>{
 #[cfg(all(feature="native-build",any(windows,target_os="linux")))]
 {
    let runtime=app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
        .ok_or("FIN04_PROTECTED_BUILD_HOST_UNAVAILABLE")?;
    let runtime=Arc::clone(runtime.inner());
    let registry=Arc::clone(registry.inner());
    let result=tauri::async_runtime::spawn_blocking(move||
        runtime.run_selected_rust_case(&app,&registry,&project_id,
          &relative_path,&case_name,&expected_source_revision,
          &expected_manifest_sha256))
        .await.map_err(|_|"FIN04_PROTECTED_TEST_THREAD_INTERRUPTED".to_owned())??;
    return serde_json::to_value(result)
        .map_err(|_|"FIN04_TEST_RECEIPT_SERIALIZATION_DENIED".into());
 }
 #[cfg(not(all(feature="native-build",any(windows,target_os="linux"))))]
 {
   let _=(app,registry,project_id,relative_path,case_name,
      expected_source_revision,expected_manifest_sha256);
   Err("FIN04_OS_TEST_WORKER_NOT_INSTALLED".into())
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

/// FIN-04: read bounded native saved test-symbol candidates; NEVER execute.
#[tauri::command]
async fn ide_inspect_saved_test_symbols(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id:String,relative_path:String,
)->Result<lu_ide_workspace::conventional_tests::TestSymbolInspection,String>{
    let registry=Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move||
        registry.inspect_test_symbol_candidates(&project_id,&relative_path))
        .await.map_err(|_|"FIN04_SYMBOL_READER_UNAVAILABLE".to_owned())?
        .map_err(str::to_owned)
}

/// FIN-05B: exact vcpkg declaration diff, no file write or package process.
#[tauri::command]
async fn ide_preview_exact_vcpkg_manifest_edit(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id:String,
    request:lu_ide_workspace::package_preview::PackagePreviewRequest,
    expected_preview_sha256:String,
    expected_evidence_fingerprint:String,
)->Result<lu_ide_workspace::package_manifest_edits::VcpkgManifestEdit,String>{
    let native=Arc::clone(registry.inner());
    tauri::async_runtime::spawn_blocking(move||
        native.prepare_exact_vcpkg_edit(&project_id,request,
            &expected_preview_sha256,&expected_evidence_fingerprint))
        .await.map_err(|_|"FIN05_VCPKG_REVIEW_THREAD_UNAVAILABLE".to_owned())?
        .map_err(str::to_owned)
}

/// FIN-05C: one user-confirmed source-manifest replacement. This is not
/// package installation. The worker cannot accept arbitrary file paths,
/// process arguments, source bytes, URLs or human approval from the WebView.
#[tauri::command]
async fn ide_apply_reviewed_vcpkg_manifest_edit(
    app:AppHandle,
    registry:State<'_,Arc<IdeWorkspaceRegistry>>,
    gate:State<'_,Arc<Fin05ManifestMutationGate>>,
    project_id:String,
    request:lu_ide_workspace::package_preview::PackagePreviewRequest,
    expected_preview_sha256:String,
    expected_evidence_fingerprint:String,
    expected_original_revision:String,
    expected_proposed_revision:String,
)->Result<serde_json::Value,String>{
    let registry=Arc::clone(registry.inner());
    let gate=Arc::clone(gate.inner());
    tauri::async_runtime::spawn_blocking(move||{
        let _one_use=gate.0.try_lock()
            .map_err(|_|"FIN05_MANIFEST_EDIT_ALREADY_ACTIVE".to_owned())?;
        let before=registry.prepare_exact_vcpkg_edit(
            &project_id,request.clone(),&expected_preview_sha256,
            &expected_evidence_fingerprint).map_err(str::to_owned)?;
        if before.expected_revision!=expected_original_revision ||
            before.proposed_revision!=expected_proposed_revision ||
            before.process_started || before.files_modified ||
            before.execution_authorized || before.packages_installed {
            return Err("FIN05_SAVED_MANIFEST_CHANGED_REVIEW_AGAIN".to_owned());
        }
        // Native dialog, no web-granted boolean, bounded 30-second refusal.
        let (sender,receiver)=std::sync::mpsc::sync_channel(1);
        app.dialog()
            .message(format!(
                "Approve ONE vcpkg.json dependency declaration edit?\n\
                 Operation: {}\nDependency: {}\n\
                 Source: {}\nProposed SHA-256: {}\n\
                 This changes ONLY vcpkg.json. No packages are installed.",
                 before.operation,before.dependency_name,
                 before.expected_revision,before.proposed_revision))
            .title("Lu IDE · review and save package declaration")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Approve this exact file edit".into(),"Decline".into()))
            .show(move|confirmed|{let _=sender.try_send(confirmed);});
        let allowed=receiver.recv_timeout(std::time::Duration::from_secs(30))
            .unwrap_or(false);
        if !allowed {
            return Err("FIN05_HUMAN_DECLINED_OR_MODAL_TIMED_OUT".to_owned());
        }
        // Fresh native package evidence AND original source revision after
        // dialog, immediately before the already revision-checked save.
        let fresh=registry.prepare_exact_vcpkg_edit(
            &project_id,request,&expected_preview_sha256,
            &expected_evidence_fingerprint).map_err(str::to_owned)?;
        if fresh.expected_revision!=before.expected_revision ||
            fresh.proposed_revision!=before.proposed_revision ||
            fresh.preview_sha256!=before.preview_sha256 ||
            fresh.evidence_fingerprint!=before.evidence_fingerprint {
            return Err("FIN05_EDIT_CHANGED_DURING_NATIVE_CONSENT".to_owned());
        }
        let saved=registry.save_source_file(&project_id,"vcpkg.json",
            &fresh.expected_revision,&fresh.proposed_text)
            .map_err(|e|e.code().to_owned())?;
        Ok(serde_json::json!({
            "projectId":project_id,"relativePath":"vcpkg.json",
            "previousRevision":fresh.expected_revision,
            "revision":saved.revision,
            "proposedRevision":fresh.proposed_revision,
            "exactSavedRevisionMatched":saved.revision==fresh.proposed_revision,
            "oneUseNativeHumanConfirmed":true,
            "processStarted":false,"packagesInstalled":false,
            "lockfileResolved":false,"installedAppAccepted":false,
            "executionAuthorizedByManifestEdit":false
        }))
    }).await.map_err(|_|"FIN05_MANIFEST_EDIT_THREAD_UNAVAILABLE".to_owned())?
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
    /// Exact release-pinned providers that also offer semantic operations.
    /// Python is absent here when the installed provider is Ruff-only.
    semantic_languages: Vec<String>,
    /// Release pins are verified byte-for-byte before a provider appears.
    installed_providers: Vec<String>,
    /// Idle, running, unavailable or durable recovery required.
    session_state: &'static str,
}

/// Native admission is deliberately NOT represented by the presence of an
/// LSP binary, an open project, or a catalog entry. D7D must first prove
/// independent service and worker confinement in native Windows acceptance.
/// A future E2 host broker will replace this fail-closed status only after
/// independently attested process + permission evidence.
#[tauri::command]
async fn ide_lsp_runtime_state(app: AppHandle) -> String {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        if let Some(state) = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>() {
            let native = Arc::clone(state.inner());
            return tauri::async_runtime::spawn_blocking(move ||
                native.poll_status().to_owned()).await
                .unwrap_or_else(|_| "reconciliation_required".into());
        }
    }
    let _ = app;
    "unavailable".into()
}
#[tauri::command]
async fn ide_lsp_worker_status(app: AppHandle) -> LanguageWorkerStatus {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let native = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .map(|state| Arc::clone(state.inner()));
        let (supported, semantic, providers, phase) =
            tauri::async_runtime::spawn_blocking(move || {
                if let Some(native) = native {
                    (lsp_backend::installed_languages(&app),
                     lsp_backend::installed_semantic_languages(&app),
                     lsp_backend::installed_provider_ids(&app),
                     native.poll_status())
                } else {
                    (Vec::new(), Vec::new(), Vec::new(), "unavailable")
                }
            }).await.unwrap_or_default();
        if phase == "reconciliation_required" {
            return LanguageWorkerStatus {
                ready: false,
                code: "LSP_NATIVE_RECOVERY_REQUIRED",
                detail: "An earlier protected worker was interrupted or its OS cleanup is unverified. No new session may start until independent native reconciliation.",
                supported_languages: Vec::new(),
                semantic_languages: Vec::new(),
                installed_providers: providers,
                session_state: phase,
            };
        }
        if !supported.is_empty() {
            return LanguageWorkerStatus {
                ready: true,
                code: if phase == "running" { "LSP_PROTECTED_SESSION_ACTIVE" }
                      else { "LSP_REVIEWED_INSTALLATION_AVAILABLE" },
                detail: "Local reviewed provider bytes verified. Actual protected language analysis still requires native OS approval and worker containment.",
                supported_languages: supported,
                semantic_languages: semantic,
                installed_providers: providers,
                session_state: phase,
            };
        }
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    let _ = app;
    LanguageWorkerStatus {
        ready: false,
        code: "LSP_PROTECTED_WORKER_UNVERIFIED",
        detail: "Reviewed offline language-server bundles are not installed or native LSP support is disabled in this build.",
        supported_languages: Vec::new(),
        semantic_languages: Vec::new(),
        installed_providers: Vec::new(),
        session_state: "unavailable",
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
async fn ide_lsp_open_document(
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
            native.open_document(&registry, typed)).await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, registry, request);
        Err("LSP_NATIVE_WORKER_UNAVAILABLE".into())
    }
}
#[tauri::command]
async fn ide_lsp_close_document(
    app: AppHandle,
    request: serde_json::Value,
) -> Result<(), String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let typed = serde_json::from_value::<lsp_backend::LspRequest>(request)
            .map_err(|_| "LSP_INVALID_REQUEST")?;
        return tauri::async_runtime::spawn_blocking(move ||
            native.close_document(typed)).await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, request);
        Err("LSP_NATIVE_WORKER_UNAVAILABLE".into())
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
async fn ide_lsp_project_diagnostics(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
) -> Result<serde_json::Value, String> {
    #[cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
    {
        let state = app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("LSP_NATIVE_WORKER_UNAVAILABLE")?;
        let native = Arc::clone(state.inner());
        let registry = Arc::clone(registry.inner());
        return tauri::async_runtime::spawn_blocking(move ||
            native.project_diagnostics(&registry)).await
            .map_err(|_| "LSP_NATIVE_THREAD_UNAVAILABLE".to_owned())?;
    }
    #[cfg(not(all(any(windows, target_os = "linux"), feature = "native-lsp")))]
    {
        let _ = (app, registry);
        Err("LSP_NATIVE_WORKER_UNAVAILABLE".into())
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

/// FIN-01D: discover ONLY this registered project's original interrupted
/// LSP job IDs. Never a worker handle, execution token, source or foreign job.
#[tauri::command]
async fn ide_lsp_recoverable_jobs(app:AppHandle,project_id:String)
    ->Result<Vec<String>,String>{
    #[cfg(all(any(windows,target_os="linux"),feature="native-lsp"))]
    {
        let native=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("FIN01_LSP_NATIVE_SUPERVISOR_MISSING")?;
        let native=Arc::clone(native.inner());
        return tauri::async_runtime::spawn_blocking(move||
            native.recoverable_jobs(&project_id))
            .await.map_err(|_|"FIN01_LSP_RECOVERY_READ_INTERRUPTED".to_owned())?;
    }
    #[cfg(not(all(any(windows,target_os="linux"),feature="native-lsp")))]
    {
        let _=(app,project_id);
        Err("FIN01_LSP_NATIVE_SUPERVISOR_NOT_INSTALLED".into())
    }
}
/// This requires its own physical native OS dialog and independently reaps
/// an exact original cgroup/Windows Job. It cannot clear the global fence.
#[tauri::command]
async fn ide_lsp_reconcile(app:AppHandle,project_id:String,job_id:String)
    ->Result<String,String>{
    #[cfg(all(any(windows,target_os="linux"),feature="native-lsp"))]
    {
        let native=app.try_state::<Arc<lsp_backend::NativeLspRuntime>>()
            .ok_or("FIN01_LSP_NATIVE_SUPERVISOR_MISSING")?;
        let native=Arc::clone(native.inner());
        return tauri::async_runtime::spawn_blocking(move||
            native.reconcile(&app,&project_id,&job_id).map(str::to_owned))
            .await.map_err(|_|"FIN01_LSP_RECOVERY_TASK_INTERRUPTED".to_owned())?;
    }
    #[cfg(not(all(any(windows,target_os="linux"),feature="native-lsp")))]
    {
        let _=(app,project_id,job_id);
        Err("FIN01_LSP_NATIVE_SUPERVISOR_NOT_INSTALLED".into())
    }
}

/// Read-only descriptions of known free/open-source LSP servers.
/// No PATH search, installation, process spawn, worker grant or IPC occurs.
#[tauri::command]
async fn ide_list_language_servers() -> Vec<lu_ide_workspace::lsp::LanguageServerDescriptor> {
    lu_ide_workspace::lsp::available_servers().to_vec()
}


/// FIN-02: compile-time feature inventory only; not runtime readiness,
/// worker authority, installer approval, or proof of installed resources.
#[tauri::command]
async fn ide_native_feature_inventory() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion":1,
        "profile":"fin02_compiled_features_observation",
        "protectedAlphaProfile":cfg!(feature="fin02-protected-alpha"),
        "nativeLsp":cfg!(feature="native-lsp"),
        "nativeBuild":cfg!(feature="native-build"),
        "nativeMultilang":cfg!(feature="native-multilang"),
        "nativeDebug":cfg!(feature="native-debug"),
        "nativeProjectDebug":cfg!(feature="native-project-debug"),
        "nativeOfflineUv":cfg!(feature="native-package-uv"),
        "developerOnlyDebugActions":cfg!(feature="dev046d-native-dialog"),
        "runtimeResourcesIndependentlyVerified":false,
        "installedAppAccepted":false,
        "authorizationGranted":false,
        "processStarted":false
    })
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

/// IDE-INT01E: on-demand read-only, bounded filename discovery.
/// No file contents, executable processes or arbitrary roots.
#[tauri::command]
async fn ide_quick_open_source_paths(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    query: String,
) -> Result<lu_ide_workspace::quick_open::QuickOpenResult, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.quick_open_source_paths(&project_id, &query)
            .map_err(|error| error.code().to_owned())
    }).await.map_err(|_| "INT01E_QUICK_OPEN_TASK_UNAVAILABLE".to_owned())?
}

/// IDE-INT01E: independently re-enumerate one directory for a sorted page.
/// Cursor is a filename only, not a filesystem authorization.
#[tauri::command]
async fn ide_list_source_directory_page(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
    relative_dir: String,
    after: Option<String>,
) -> Result<lu_ide_workspace::quick_open::DirectoryPage, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry.list_source_directory_page(&project_id, &relative_dir, after.as_deref())
            .map_err(|error| error.code().to_owned())
    }).await.map_err(|_| "INT01E_DIRECTORY_PAGE_TASK_UNAVAILABLE".to_owned())?
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


// IDE-INT05-D: optional typed interactive terminal, never a general shell.
// Every command is consumed once inside the native, project-bound session.
// Only the existing fixed D3 runner can launch code; it still requires fresh
// frozen manifest verification, independent OS human dialog and custody.
#[tauri::command]
async fn ide_terminal_open(
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<terminal_broker::TerminalSessions>>,
    project_id: String,
) -> Result<terminal_broker::TerminalSessionView, String> {
    registry.source_capability(&project_id)
        .map_err(|_| "INT05D_REGISTERED_PROJECT_REQUIRED")?;
    sessions.open(&project_id).map_err(str::to_owned)
}
#[tauri::command]
async fn ide_terminal_submit(
    app: AppHandle,
    registry: State<'_, Arc<IdeWorkspaceRegistry>>,
    sessions: State<'_, Arc<terminal_broker::TerminalSessions>>,
    project_id: String,
    session_id: String,
    sequence: u64,
    line: String,
    expected_manifest_sha256: Option<String>,
) -> Result<terminal_broker::TerminalReply, String> {
    registry.source_capability(&project_id)
        .map_err(|_| "INT05D_REGISTERED_PROJECT_REQUIRED")?;
    let action = sessions.consume(&project_id, &session_id, sequence, &line)
        .map_err(str::to_owned)?;
    use terminal_broker::{TerminalAction, TerminalReply};
    let (kind, message, value) = match action {
        TerminalAction::Help => {
            if expected_manifest_sha256.is_some() {
                return Err("INT05D_UNEXPECTED_MANIFEST_IDENTITY".into());
            }
            ("help", "Commands: help, status, toolchains, packages, run <fixed reviewed profile>, cancel <exact native job ID>. No shell, pipes, scripts, network, or arbitrary arguments.", None)
        }
        TerminalAction::Packages => {
            if expected_manifest_sha256.is_some() {
                return Err("INT05D_UNEXPECTED_MANIFEST_IDENTITY".into());
            }
            let registry = Arc::clone(registry.inner());
            let project = project_id.clone();
            let report = tauri::async_runtime::spawn_blocking(move ||
                registry.inspect_package_managers(&project).map_err(str::to_owned))
                .await.map_err(|_|"INT05D_PACKAGE_INSPECTION_UNAVAILABLE")??;
            ("packages", "Native read-only manifest inspection; no package manager executed.",
                Some(serde_json::to_value(report).map_err(|_|"INT05D_SERIALIZE_DENIED")?))
        }
        TerminalAction::Status | TerminalAction::Toolchains => {
            if expected_manifest_sha256.is_some() {
                return Err("INT05D_UNEXPECTED_MANIFEST_IDENTITY".into());
            }
            #[cfg(all(feature="native-build",any(windows,target_os="linux")))]
            {
                let native = app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
                    .ok_or("INT05D_NATIVE_SUPERVISOR_NOT_INSTALLED")?;
                let native = Arc::clone(native.inner());
                let app_copy=app.clone();
                let project=project_id.clone();
                let status=tauri::async_runtime::spawn_blocking(move ||
                    native.status(&app_copy,&project))
                    .await.map_err(|_|"INT05D_NATIVE_STATUS_UNAVAILABLE")??;
                ("status", "Native runtime and recovery status; readiness is not execution approval.",
                    Some(serde_json::to_value(status).map_err(|_|"INT05D_SERIALIZE_DENIED")?))
            }
            #[cfg(not(all(feature="native-build",any(windows,target_os="linux"))))]
            {
                ("status","Protected native toolchains are not installed; execution denied.",None)
            }
        }
        TerminalAction::Run(recipe) => {
            let sha=expected_manifest_sha256.ok_or("INT05D_REVIEWED_MANIFEST_REQUIRED")?;
            #[cfg(all(feature="native-build",any(windows,target_os="linux")))]
            {
                registry.trusted_native_worker_root(&project_id)
                    .map_err(|_|"INT05D_PROJECT_WORKER_SCOPE_DENIED")?;
                let native=app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
                    .ok_or("INT05D_NATIVE_SUPERVISOR_NOT_INSTALLED")?;
                let native=Arc::clone(native.inner());
                let registry=Arc::clone(registry.inner());
                let project=project_id.clone();
                let app_copy=app.clone();
                // The exact recipe comes ONLY from the enum above; the SHA is
                // rehashed natively and one-use consent happens in run_profile.
                let completed=tauri::async_runtime::spawn_blocking(move ||
                    native.run_profile(&app_copy,&registry,&project,recipe,&sha))
                    .await.map_err(|_|"INT05D_RUN_WORKER_UNAVAILABLE")??;
                ("native_job_result","Native worker returned; inspect its original terminal receipt and cleanup evidence.",
                    Some(serde_json::to_value(completed).map_err(|_|"INT05D_SERIALIZE_DENIED")?))
            }
            #[cfg(not(all(feature="native-build",any(windows,target_os="linux"))))]
            {
                let _=(app,registry,recipe,sha);
                return Err("INT05D_PROTECTED_NATIVE_RUNNER_NOT_INSTALLED".into())
            }
        }
        TerminalAction::Cancel(job) => {
            if expected_manifest_sha256.is_some() {
                return Err("INT05D_UNEXPECTED_MANIFEST_IDENTITY".into());
            }
            #[cfg(all(feature="native-build",any(windows,target_os="linux")))]
            {
                let native=app.try_state::<Arc<build_backend::NativeBuildRuntime>>()
                    .ok_or("INT05D_NATIVE_SUPERVISOR_NOT_INSTALLED")?;
                let receipt=native.cancel(&project_id,job.as_deref())?;
                ("cancellation_requested","Cancellation requested, NOT yet confirmed. Native OS-tree cleanup must be verified.",
                    Some(serde_json::json!({"nativeRequest":receipt})))
            }
            #[cfg(not(all(feature="native-build",any(windows,target_os="linux"))))]
            {
                let _=(app,job);
                return Err("INT05D_PROTECTED_NATIVE_RUNNER_NOT_INSTALLED".into())
            }
        }
    };
    Ok(TerminalReply::new(project_id,session_id,sequence,kind,message,value))
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
            app.manage(Arc::new(Fin05ManifestMutationGate::default()));
            app.manage(Arc::new(terminal_broker::TerminalSessions::default()));
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
            ide_native_feature_inventory,
            ide_list_language_servers,
            ide_lsp_worker_status,
            ide_lsp_runtime_state,
            ide_lsp_start_session,
            ide_lsp_open_document,
            ide_lsp_close_document,
            ide_lsp_change_document,
            ide_lsp_feature,
            ide_lsp_read_diagnostics,
            ide_lsp_project_diagnostics,
            ide_lsp_stop_session,
            ide_lsp_recoverable_jobs,
            ide_lsp_reconcile,
            ide_prepare_execution_intent,
            ide_decide_execution_intent,
            ide_list_execution_intents,
            ide_discover_project_run_profiles,
            ide_inspect_package_managers,
            ide_preview_package_operation,
            ide_revalidate_package_operation,
            ide_preview_exact_vcpkg_manifest_edit,
            ide_apply_reviewed_vcpkg_manifest_edit,
            ide_discover_conventional_tests,
            ide_inspect_saved_test_symbols,
            ide_review_project_debug_target,
            ide_review_multilang_adapter,
            ide_fin01_recovery_assessment,
            ide_fin01_clear_stale_marker,
            ide_build_status,
            ide_terminal_open,
            ide_terminal_submit,
            ide_build_read_output,
            ide_build_run_profile,
            ide_run_protected_rust_test_case,
            ide_run_protected_framework_test_case,
            ide_build_review_test_target,
            ide_build_start,
            ide_package_run_reviewed_uv,
            ide_package_discard_private_output,
            ide_build_cancel,
            ide_build_reconcile,
            ide_debug_status,
            ide_debug_control_evidence,
            ide_debug_action_os_recovery,
            ide_debug_review_control_candidate,
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
            ide_git_review_branch_operation,
            ide_read_local_stash_inventory,
            ide_git_preview_local_mutation_impact,
            ide_git_review_single_unstage,
            ide_git_review_one_file_stash,
            ide_git_execute_one_file_stash,
            ide_git_one_file_stash_recovery,
            ide_git_review_single_file_restore,
            ide_git_execute_single_file_restore,
            ide_git_single_file_restore_recovery,
            ide_git_execute_single_unstage,
            ide_git_inspect_unstage_recovery,
            ide_git_reconcile_unstage_recovery,
            ide_git_preview_local_merge,
            ide_git_verify_divergent_merge,
            ide_git_inspect_native_conflicts,
            ide_git_review_conflict_resolution,
            ide_git_apply_conflict_resolution,
            ide_git_inspect_conflict_recovery,
            ide_git_reconcile_conflict_receipt,
            ide_git_execute_divergent_merge,
            ide_git_inspect_divergent_recovery,
            ide_git_reconcile_divergent_receipt,
            ide_git_review_rebase_cherry_pick,
            ide_git_preview_isolated_replay,
            ide_git_execute_cherry_pick,
            ide_git_execute_rebase,
            ide_git_cancel_protected_replay,
            ide_git_inspect_replay_recovery,
            ide_git_reconcile_replay_receipt,
            ide_git_execute_fast_forward,
            ide_git_cancel_protected_merge,
            ide_git_inspect_protected_merge_recovery,
            ide_git_reconcile_protected_merge_receipt,
            ide_read_git_diff,
            ide_probe_project_resource,
            ide_read_git_history,
            ide_compare_live_github,
            ide_read_github_pull_requests,
            ide_read_github_review_state,
            ide_read_github_branch_rules,
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
            ide_list_source_directory_page,
            ide_quick_open_source_paths,
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
