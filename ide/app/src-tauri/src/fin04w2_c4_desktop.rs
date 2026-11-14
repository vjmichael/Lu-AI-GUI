//! FIN-04W2-C4: default-OFF, Windows DEBUG ONLY, native IDE owner menu.
//! The menu is registered in the trusted Tauri host, NOT invoke_handler,
//! WebView, the Lu model capability registry or a remote instruction.
//! C4 cannot install an SCM service, edit protected ProgramData, run LLDB,
//! authorize a debuggee or invent a native human click.
//! It prepares one genuine sealed Cargo build and waits for an independently
//! administrator-pinned C2 service configuration for that exact Job+plan.
#![cfg(all(windows, debug_assertions, feature = "fin04w2-c4-desktop"))]
use crate::build_backend::NativeBuildRuntime;
use lu_ide_execution_supervisor::{
    project_debug_target::SealedProjectDebugTarget,
    snapshot::{publish_input, retire_input, verify_input},
    windows_fin04w2_c2_host,
    JobPhase, JobPlan, Supervisor,
};
use lu_ide_workspace::{wire::proto::ProjectView, IdeWorkspaceRegistry};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{
    menu::{Menu, MenuItem, Submenu}, App, AppHandle, Manager,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

const PREPARE_ID: &str = "lu.ide.fin04w2.c4.prepare.native";
const APPROVE_ID: &str = "lu.ide.fin04w2.c4.approve.native";
const CANCEL_ID: &str = "lu.ide.fin04w2.c4.cancel.native";
const C4_SERVICE_NAME: &str = "windows_fin04w2_debugger_service.exe";
const COMPILED_SERVICE_PIN: Option<&str> = option_env!("LU_IDE_FIN04W2_C4_SCM_SHA256");

fn denied(code: &'static str) -> String { code.into() }
fn sha(v: &str, n: usize) -> bool {
    v.len() == n && v.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn selected_native_project(rows: &[ProjectView]) -> Result<String, String> {
    // No WebView-supplied project ID or path. Multiple open projects require
    // a separate reviewed native project chooser; never silently pick one.
    let mut active = rows.iter().filter(|p| p.active && p.available);
    let only = active.next().ok_or("FIN04C4_NO_SINGLE_ACTIVE_PROJECT")?;
    if active.next().is_some() || rows.iter().any(|p| p.active && !p.available) {
        return Err(denied("FIN04C4_AMBIGUOUS_OR_UNAVAILABLE_PROJECT"));
    }
    if !only.project_id.starts_with("ide-") || !sha(&only.project_id[4..], 32) {
        return Err(denied("FIN04C4_UNTRUSTED_PROJECT_ID"));
    }
    Ok(only.project_id.clone())
}
fn forbidden_automated_session() -> Result<(), String> {
    if std::env::var_os("CI").is_some()
        || std::env::var_os("GITHUB_ACTIONS").is_some()
        || std::env::var_os("LU_FIN04W2_NATIVE_FIXTURE_ROOT").is_some()
    { Err(denied("FIN04C4_AUTOMATED_APPROVAL_FORBIDDEN")) }
    else { Ok(()) }
}
fn hash_regular_file(path: &Path) -> Result<String, String> {
    use std::os::windows::fs::MetadataExt;
    let meta = fs::symlink_metadata(path).map_err(|_| denied("FIN04C4_PINNED_IMAGE_MISSING"))?;
    if !path.is_absolute() || !meta.is_file() || meta.file_type().is_symlink()
        || meta.file_attributes() & 0x400 != 0 || meta.len() == 0
        || meta.len() > 256 * 1024 * 1024 {
        return Err(denied("FIN04C4_PINNED_IMAGE_UNTRUSTED"));
    }
    let mut f = File::open(path).map_err(|_| denied("FIN04C4_PINNED_IMAGE_UNREADABLE"))?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|_| denied("FIN04C4_PINNED_IMAGE_UNREADABLE"))?;
        if n == 0 { break; }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn pinned_service(app: &AppHandle) -> Result<(PathBuf, String), String> {
    let pinned = COMPILED_SERVICE_PIN.ok_or("FIN04C4_SERVICE_BUILD_PIN_ABSENT")?;
    if !sha(pinned, 64) { return Err(denied("FIN04C4_SERVICE_BUILD_PIN_INVALID")); }
    let path = app.path().resource_dir()
        .map_err(|_| denied("FIN04C4_SERVICE_RESOURCE_UNAVAILABLE"))?
        .join("recovery").join(C4_SERVICE_NAME);
    if hash_regular_file(&path)? != pinned {
        return Err(denied("FIN04C4_SERVICE_IMAGE_HASH_MISMATCH"));
    }
    Ok((path, pinned.to_owned()))
}
fn original_plan(
    app: &AppHandle,
    registry: &IdeWorkspaceRegistry,
    builds: &NativeBuildRuntime,
    project_id: &str,
    target: &Arc<SealedProjectDebugTarget>,
    stage: &Path,
) -> Result<JobPlan, String> {
    forbidden_automated_session()?;
    if selected_native_project(&registry.projects().map_err(|_| denied("FIN04C4_PROJECT_LIST_UNAVAILABLE"))?)?
        != project_id { return Err(denied("FIN04C4_ACTIVE_PROJECT_CHANGED")); }
    let current = builds.native_project_debug_target(project_id)?;
    if !Arc::ptr_eq(&current, target) {
        return Err(denied("FIN04C4_ORIGINAL_BUILD_TARGET_REPLACED"));
    }
    target.verify().map_err(|_| denied("FIN04C4_ORIGINAL_BINARY_OR_SYMBOLS_CHANGED"))?;
    let root = registry.trusted_native_worker_root(project_id)
        .map_err(|_| denied("FIN04C4_REGISTERED_ROOT_UNAVAILABLE"))?;
    let git = registry.inspect_git_worktree(project_id);
    if git.state != "local_repository_inspected" || git.worktree != "clean"
        || git.truncated {
        return Err(denied("FIN04C4_GIT_STATE_UNTRUSTED"));
    }
    let head = git.local_head.ok_or("FIN04C4_SOURCE_HEAD_MISSING")?;
    if !sha(&head, 40) || target.lineage.project_id != project_id
        || target.lineage.source_head != head
        || !target.lineage.build_successfully_completed
        || target.lineage.requires_reconciliation
        || !sha(&target.lineage.build_toolchain_sha256, 64) {
        return Err(denied("FIN04C4_COMPLETED_BUILD_OR_SOURCE_MISMATCH"));
    }
    // A live Git HEAD is not an input snapshot. Re-copy and independently
    // rehash the actual registered project before AND after the OS dialog.
    let snapshot = publish_input(&root, stage, "rust_check")
        .map_err(|e| e.code().to_owned())?;
    let seal_result = verify_input(&snapshot)
        .map_err(|e| e.code().to_owned())
        .and_then(|()| {
            if snapshot.seal.source_sha256 != target.lineage.source_snapshot_sha256
                || snapshot.seal.manifest_sha256 != target.lineage.manifest_sha256 {
                return Err(denied("FIN04C4_SOURCE_SNAPSHOT_CHANGED_AFTER_BUILD"));
            }
            Ok((snapshot.seal.source_sha256.clone(), snapshot.seal.manifest_sha256.clone()))
        });
    // The snapshot is evidence only; the C2 worker never runs this source.
    // Retire even if its content check failed. No private path leaves Tauri.
    let retire = retire_input(snapshot).map_err(|e| e.code().to_owned());
    let (source_sha, manifest_sha) = seal_result?;
    retire?;
    let _ = app;
    Ok(JobPlan {
        project_id: project_id.to_owned(),
        recipe_id: "dap_lldb_project".into(), // C2 protocol's fixed REVIEW identity; NO LLDB.
        source_head: head,
        input_snapshot_sha256: source_sha,
        manifest_sha256: manifest_sha,
        toolchain_sha256: target.lineage.build_toolchain_sha256.clone(),
        debug_target_binding_sha256: Some(target.binding_sha256.clone()),
        runtime_limit_ms: 90_000,
        memory_limit_bytes: 512 * 1024 * 1024,
        process_limit: 16,
        log_limit_bytes: 64 * 1024,
        effect_class: "debug_inspection_isolated".into(),
    })
}

struct Pending {
    project_id: String,
    job_id: String,
    plan_sha256: String,
    plan: JobPlan,
    sealed_build: Arc<SealedProjectDebugTarget>,
}
enum LocalState { Empty, Working, Prepared(Pending) }
struct C4Gate {
    original: Supervisor,
    journal: PathBuf,
    stage: PathBuf,
    state: Mutex<LocalState>,
}
impl C4Gate {
    fn open(app: &AppHandle) -> Result<Self, String> {
        let base = app.path().app_data_dir()
            .map_err(|_| denied("FIN04C4_PRIVATE_PROFILE_MISSING"))?;
        let stage = base.join("fin04w2-c4-staged");
        fs::create_dir_all(&stage).map_err(|_| denied("FIN04C4_PRIVATE_STAGE_UNAVAILABLE"))?;
        let journal = base.join("fin04w2-c4-original.jsonl");
        Ok(Self {
            original: Supervisor::open(journal.clone())
                .map_err(|e| e.code().to_owned())?,
            journal, stage, state: Mutex::new(LocalState::Empty),
        })
    }
    fn begin(&self) -> Result<(), String> {
        let mut locked = self.state.lock().map_err(|_| denied("FIN04C4_NATIVE_GATE_POISONED"))?;
        if !matches!(*locked, LocalState::Empty) {
            return Err(denied("FIN04C4_PREVIOUS_JOB_STILL_OWNED"));
        }
        *locked = LocalState::Working;
        Ok(())
    }
    fn prepare(
        &self, app: &AppHandle, registry: &IdeWorkspaceRegistry,
        builds: &NativeBuildRuntime,
    ) -> Result<String, String> {
        self.begin()?;
        let mut prepared_id: Option<String> = None;
        let candidate = (|| {
            forbidden_automated_session()?;
            // A recovered interrupted C2 admission must not silently issue
            // another job. Manual independent native recovery is required.
            if self.original.inspect().map_err(|e| e.code().to_owned())?
                .iter().any(|x| x.requires_reconciliation
                    || !matches!(x.phase, JobPhase::Completed | JobPhase::Cancelled | JobPhase::Denied))
            {
                return Err(denied("FIN04C4_PREVIOUS_NATIVE_JOB_UNRECONCILED"));
            }
            let project = selected_native_project(
                &registry.projects().map_err(|_| denied("FIN04C4_PROJECT_LIST_UNAVAILABLE"))?
            )?;
            let target = builds.native_project_debug_target(&project)?;
            let plan = original_plan(app, registry, builds, &project, &target, &self.stage)?;
            let prepared = self.original.prepare(plan.clone())
                .map_err(|e| e.code().to_owned())?;
            prepared_id = Some(prepared.job_id.clone());
            // This proposal is NOT the protected SCM config or creator consent.
            // An independent administrator must recheck its original evidence,
            // pin host/service SHA and exact job/plan and establish ACL/WFP.
            let host_exe = std::env::current_exe()
                .map_err(|_| denied("FIN04C4_TRUSTED_HOST_IMAGE_MISSING"))?;
            let host_sha = hash_regular_file(&host_exe)?;
            let manifest = json!({
                "schema":"lu.ide.fin04w2.c4.prepared-not-authorized.v1",
                "approvalKind":"NO_APPROVAL_PREPARED_ONLY",
                "projectId":project,
                "jobId":prepared.job_id,
                "planSha256":prepared.plan_sha256,
                "plan":plan,
                "originalJournal":self.journal,
                "hostExecutable":host_exe,
                "hostSha256":host_sha,
                "originalBuildJobId":target.lineage.build_job_id,
                "originalBuildReceiptSha256":target.lineage.terminal_receipt_sha256,
                "binarySha256":target.binary.binary_sha256,
                "symbolsSha256":target.symbols.symbols_sha256,
            });
            let path = self.stage.join(format!("{}.prepared.json", prepared.job_id));
            let mut f = OpenOptions::new().write(true).create_new(true)
                .open(&path).map_err(|_| denied("FIN04C4_PRIVATE_PROPOSAL_WRITE_DENIED"))?;
            f.write_all(&serde_json::to_vec_pretty(&manifest)
                .map_err(|_| denied("FIN04C4_PRIVATE_PROPOSAL_SERIALIZE_DENIED"))?)
                .map_err(|_| denied("FIN04C4_PRIVATE_PROPOSAL_WRITE_DENIED"))?;
            f.sync_all().map_err(|_| denied("FIN04C4_PRIVATE_PROPOSAL_FSYNC_DENIED"))?;
            Ok(Pending {project_id:project,job_id:prepared.job_id,
                plan_sha256:prepared.plan_sha256,plan,sealed_build:target})
        })();
        let mut state = self.state.lock().map_err(|_| denied("FIN04C4_NATIVE_GATE_POISONED"))?;
        match candidate {
            Ok(pending) => {
                let display = format!(
                    "Prepared only: {}\nPlan SHA-256: {}\n\nNo approval and no process execution occurred.\nAn independent administrator must install the exact restricted service/Job/WFP configuration before selecting Approve.",
                    pending.job_id, pending.plan_sha256);
                *state = LocalState::Prepared(pending);
                Ok(display)
            }
            Err(code) => {
                if let Some(id) = prepared_id {
                    let _ = self.original.cancel_before_start(&id);
                }
                *state = LocalState::Empty;
                Err(code)
            }
        }
    }
    fn approve(
        &self, app: &AppHandle, registry: &IdeWorkspaceRegistry,
        builds: &NativeBuildRuntime, window: &tauri::WebviewWindow,
    ) -> Result<String, String> {
        forbidden_automated_session()?;
        let pending = {
            let mut state = self.state.lock().map_err(|_| denied("FIN04C4_NATIVE_GATE_POISONED"))?;
            match std::mem::replace(&mut *state, LocalState::Working) {
                LocalState::Prepared(p) => p,
                previous => {
                    *state = previous;
                    return Err(denied("FIN04C4_NO_PREPARED_NATIVE_JOB"));
                }
            }
        };
        let result = (|| {
            if window.label() != "main" {
                return Err(denied("FIN04C4_FOREIGN_IDE_WINDOW"));
            }
            // Obtain the genuine Tauri-owned native Win32 handle in trusted
            // Rust. No HWND or execution path is accepted from JavaScript.
            let owner = window.hwnd().map_err(|_| denied("FIN04C4_NATIVE_HWND_UNAVAILABLE"))?;
            let (service, service_sha) = pinned_service(app)?;
            let proposed = original_plan(app,registry,builds,
                &pending.project_id,&pending.sealed_build,&self.stage)?;
            if proposed != pending.plan {
                return Err(denied("FIN04C4_PRE_DIALOG_PLAN_MISMATCH"));
            }
            windows_fin04w2_c2_host::run_fixed_after_visible_native_consent(
                &self.original,&pending.job_id,&pending.plan,
                owner.0.cast(),
                || original_plan(app,registry,builds,
                    &pending.project_id,&pending.sealed_build,&self.stage)
                    .map_err(|_| lu_ide_execution_supervisor::SupervisorError::WrongProjectOrSource),
                &service,&service_sha,
            ).map_err(|e| e.code().to_owned())?;
            let rows = self.original.receipt_history(&pending.job_id)
                .map_err(|e| e.code().to_owned())?;
            let finish = self.original.inspect().map_err(|e| e.code().to_owned())?;
            if rows.len()!=6 || !finish.iter().any(|j|j.job_id==pending.job_id
                && j.phase==JobPhase::Completed && !j.requires_reconciliation) {
                return Err(denied("FIN04C4_ORIGINAL_SIX_DURABLE_PHASES_UNVERIFIED"));
            }
            Ok(format!("Harmless C2 native job {} completed under original SCM/Job receipts. This is NOT LLDB, W2-D or installed-release acceptance.", pending.job_id))
        })();
        // A NO, failed preflight or missing admin service config does not
        // manufacture an Authorized receipt. After admission uncertainty,
        // C2's original recovery guard quarantines and refuses replay.
        if result.is_err() {
            if self.original.inspect().ok().is_some_and(|rows| rows.iter().any(
                |j| j.job_id == pending.job_id && j.phase == JobPhase::Prepared
            )) {
                let _ = self.original.cancel_before_start(&pending.job_id);
            }
        }
        if let Ok(mut guard) = self.state.lock() { *guard = LocalState::Empty; }
        result
    }
    fn cancel(&self) -> Result<String,String> {
        let mut state = self.state.lock().map_err(|_| denied("FIN04C4_NATIVE_GATE_POISONED"))?;
        let pending = match std::mem::replace(&mut *state, LocalState::Working) {
            LocalState::Prepared(p) => p,
            previous => {
                *state = previous;
                return Err(denied("FIN04C4_NO_PREPARED_NATIVE_JOB"));
            }
        };
        let result = self.original.cancel_before_start(&pending.job_id)
            .map_err(|e| e.code().to_owned())
            .map(|_| format!("Prepared native job {} cancelled. It can never be approved or replayed.", pending.job_id));
        *state = LocalState::Empty;
        result
    }
}
pub fn install(app: &App) -> tauri::Result<()> {
    let prepare = MenuItem::with_id(app,PREPARE_ID,
        "Prepare sealed harmless SCM test (no execution)",true,None::<&str>)?;
    let approve = MenuItem::with_id(app,APPROVE_ID,
        "Approve prepared harmless SCM test (native OS dialog)",true,None::<&str>)?;
    let cancel = MenuItem::with_id(app,CANCEL_ID,
        "Cancel prepared harmless SCM test",true,None::<&str>)?;
    let submenu = Submenu::with_items(app,"Native safety test (developer only)",true,
        &[&prepare,&approve,&cancel])?;
    let menu = Menu::with_items(app,&[&submenu])?;
    app.set_menu(menu)?;
    let gate = Arc::new(C4Gate::open(app.handle())
        .map_err(std::io::Error::other)?);
    app.manage(gate);
    app.on_menu_event(|app,event| {
        let action = if event.id() == PREPARE_ID {0}
            else if event.id() == APPROVE_ID {1}
            else if event.id() == CANCEL_ID {2}
            else {return};
        let Some(gate) = app.try_state::<Arc<C4Gate>>() else {return;};
        let Some(registry) = app.try_state::<Arc<IdeWorkspaceRegistry>>() else {return;};
        let Some(builds) = app.try_state::<Arc<NativeBuildRuntime>>() else {return;};
        let gate=Arc::clone(gate.inner());
        let registry=Arc::clone(registry.inner());
        let builds=Arc::clone(builds.inner());
        let app=app.clone();
        std::thread::spawn(move || {
            let result = match action {
                0=>gate.prepare(&app,&registry,&builds),
                1=>match app.get_webview_window("main") {
                    Some(window)=>gate.approve(&app,&registry,&builds,&window),
                    None=>Err(denied("FIN04C4_NATIVE_MAIN_WINDOW_MISSING")),
                },
                _=>gate.cancel(),
            };
            // Informational native UI only. This is never a second approval
            // channel and NEVER converts an error into a human grant.
            let info = result.unwrap_or_else(|code|format!(
                "Native operation denied: {code}\nNo fallback worker or debugger route is available."));
            app.dialog().message(info).title("Lu IDE — native safety test")
                .buttons(MessageDialogButtons::Ok).blocking_show();
        });
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn project(active:bool,available:bool,id:&str)->ProjectView {
        ProjectView { project_id:id.into(),display_name:"Local".into(),
            active,available }
    }
    #[test] fn no_renderer_selected_project_and_no_ambiguous_active_project() {
        let p="ide-0123456789abcdef0123456789abcdef";
        let q="ide-fedcba9876543210fedcba9876543210";
        assert!(selected_native_project(&[]).is_err());
        assert_eq!(selected_native_project(&[project(true,true,p)]).unwrap(),p);
        assert!(selected_native_project(&[project(true,true,p),project(true,true,q)]).is_err());
        assert!(selected_native_project(&[project(true,false,p)]).is_err());
        assert!(selected_native_project(&[project(true,true,p),project(true,false,q)]).is_err());
        assert!(selected_native_project(&[project(true,true,"../fake")]).is_err());
    }
    #[test] fn service_pin_requires_real_compiled_sha_and_known_fixed_filename() {
        assert!(!sha("not-a-hash",64));
        assert!(!sha(&"A".repeat(64),64));
        assert!(sha(&"a".repeat(64),64));
        assert_eq!(C4_SERVICE_NAME,"windows_fin04w2_debugger_service.exe");
        assert_ne!(PREPARE_ID,APPROVE_ID);
    }
}
