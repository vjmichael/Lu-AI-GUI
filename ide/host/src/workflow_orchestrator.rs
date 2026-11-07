//! WO9-E durable dependency DAG. The coordinator never executes programs.
//! Native WO6 remains the only worker launcher, with independent OS approval
//! and signed-resource admission on EVERY step. Crash => terminal quarantine.
use crate::IdeWorkspaceRegistry;
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, HashSet}, fs::{self, File, OpenOptions},
    io::Write, path::PathBuf, sync::Mutex};

const LIMIT_RUNS: usize = 16;
const LIMIT_STEPS: usize = 12;
const LIMIT_BYTES: u64 = 256 * 1024;
const ALLOWED: [&str; 3] = ["rust_check", "rust_tests", "lu_ide_host_tests"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowStep {
    pub project_id: String,
    pub recipe_id: String,
    pub depends_on: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowTask {
    pub spec: WorkflowStep,
    pub state: String,
    pub native_job_id: Option<String>,
    pub native_plan_sha256: Option<String>,
    pub native_output_sha256: Option<String>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowRun {
    pub workflow_id: String,
    pub project_ids: Vec<String>,
    pub source_fingerprint: String,
    pub tasks: Vec<WorkflowTask>,
    pub release_approved: bool,
    pub automatic_execution: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Store {
    version: u32,
    runs: BTreeMap<String, WorkflowRun>,
}
pub struct WorkflowCoordinator {
    file: PathBuf,
    inner: Mutex<Store>,
}
fn save(file: &PathBuf, store: &Store) -> Result<(), &'static str> {
    let bytes = serde_json::to_vec_pretty(store).map_err(|_| "WO9_GRAPH_ENCODING_FAILED")?;
    if bytes.len() as u64 > LIMIT_BYTES { return Err("WO9_GRAPH_CAPACITY"); }
    let parent = file.parent().ok_or("WO9_GRAPH_STORE_PATH_INVALID")?;
    fs::create_dir_all(parent).map_err(|_| "WO9_GRAPH_STORE_UNAVAILABLE")?;
    let temp = parent.join(format!(".wo9-{}.tmp", rand::random::<u64>()));
    let res = (|| {
        let mut writer = OpenOptions::new().write(true).create_new(true).open(&temp)
            .map_err(|_| "WO9_GRAPH_STORE_UNAVAILABLE")?;
        writer.write_all(&bytes).map_err(|_| "WO9_GRAPH_WRITE_FAILED")?;
        writer.sync_all().map_err(|_| "WO9_GRAPH_SYNC_FAILED")?;
        drop(writer);
        fs::rename(&temp, file).map_err(|_| "WO9_GRAPH_PUBLISH_FAILED")?;
        #[cfg(unix)]
        File::open(parent).and_then(|d| d.sync_all())
            .map_err(|_| "WO9_GRAPH_DIRECTORY_SYNC_FAILED")?;
        Ok::<(), &'static str>(())
    })();
    if res.is_err() { let _ = fs::remove_file(temp); }
    res
}
fn allowed_spec(specs: &[WorkflowStep], selected: &[String]) -> Result<(), &'static str> {
    if specs.is_empty() || specs.len() > LIMIT_STEPS { return Err("WO9_GRAPH_SIZE_DENIED"); }
    for (pos, step) in specs.iter().enumerate() {
        if !selected.contains(&step.project_id) || !ALLOWED.contains(&step.recipe_id.as_str())
            || step.depends_on.len() > LIMIT_STEPS {
            return Err("WO9_GRAPH_STEP_OUT_OF_SCOPE");
        }
        let mut seen = HashSet::new();
        for &dep in &step.depends_on {
            // Preceding step dependencies only: a cycle cannot be represented.
            if dep >= pos || !seen.insert(dep) { return Err("WO9_GRAPH_DEPENDENCY_CYCLE_OR_DUPLICATE"); }
        }
    }
    Ok(())
}
impl WorkflowCoordinator {
    pub fn new(file: PathBuf) -> Result<Self, &'static str> {
        let mut store: Store = if file.exists() {
            let meta = fs::symlink_metadata(&file).map_err(|_| "WO9_GRAPH_FILE_UNAVAILABLE")?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > LIMIT_BYTES {
                return Err("WO9_GRAPH_UNTRUSTED_FILE");
            }
            serde_json::from_slice(&fs::read(&file).map_err(|_| "WO9_GRAPH_FILE_UNAVAILABLE")?)
                .map_err(|_| "WO9_GRAPH_CORRUPT")?
        } else { Store { version: 1, runs: BTreeMap::new() } };
        if store.version != 1 || store.runs.len() > LIMIT_RUNS {
            return Err("WO9_GRAPH_UNSUPPORTED_VERSION");
        }
        let mut recovered = false;
        for (id, run) in store.runs.iter_mut() {
            if run.release_approved || run.automatic_execution
                || run.workflow_id != *id
                || run.project_ids.is_empty() || run.project_ids.len() > 8
                || run.source_fingerprint.len() != 64
                || !run.source_fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
                || allowed_spec(&run.tasks.iter().map(|t| t.spec.clone())
                    .collect::<Vec<_>>(), &run.project_ids).is_err() {
                return Err("WO9_GRAPH_UNTRUSTED_STATE");
            }
            for task in &mut run.tasks {
                if !matches!(task.state.as_str(),
                    "pending" | "running" | "interrupted" | "failed"
                    | "passed" | "cancelled" | "native_receipt_unverified") {
                    return Err("WO9_GRAPH_UNKNOWN_TASK_STATE");
                }
                if task.state == "running" {
                    task.state = "interrupted".into();
                    task.error_code = Some("WO9_HOST_CRASH_RECONCILIATION_REQUIRED".into());
                    recovered = true;
                } else if task.state == "passed" {
                    // User-modifiable profile state is never proof that the
                    // protected supervisor really completed a native job.
                    // On every host restart, independently re-check its
                    // fsynced OS journal before unlocking any dependent step.
                    task.state = "native_receipt_unverified".into();
                    task.error_code = Some("WO9_RESTART_NATIVE_RECEIPT_REQUIRED".into());
                    recovered = true;
                }
            }
        }
        if recovered { save(&file, &store)?; }
        Ok(Self { file, inner: Mutex::new(store) })
    }
    pub fn list(&self) -> Result<Vec<WorkflowRun>, &'static str> {
        let store = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        Ok(store.runs.values().cloned().collect())
    }
    pub fn create(&self, registry: &IdeWorkspaceRegistry, selected: &[String],
        fingerprint: &str, specs: Vec<WorkflowStep>) -> Result<WorkflowRun, &'static str> {
        allowed_spec(&specs, selected)?;
        registry.verify_multi_project_workflow(selected, fingerprint)?;
        let mut guard = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        if guard.runs.len() >= LIMIT_RUNS { return Err("WO9_GRAPH_CAPACITY"); }
        let id = format!("wo9-{:032x}", rand::random::<u128>());
        let tasks = specs.into_iter().map(|spec| WorkflowTask {
            spec, state: "pending".into(), native_job_id: None,
            native_plan_sha256: None, native_output_sha256: None, error_code: None,
        }).collect();
        let run = WorkflowRun {
            workflow_id: id.clone(), project_ids: selected.to_vec(),
            source_fingerprint: fingerprint.to_owned(), tasks,
            release_approved: false, automatic_execution: false,
        };
        let mut next = guard.runs.clone();
        next.insert(id, run.clone());
        save(&self.file, &Store { version: 1, runs: next.clone() })?;
        guard.runs = next;
        Ok(run)
    }
    pub fn begin_step(&self, registry: &IdeWorkspaceRegistry,
        workflow_id: &str, step: usize) -> Result<WorkflowStep, &'static str> {
        // Validate and recheck under same mutex so two identical UI clicks
        // cannot launch the same task concurrently or across projects.
        let mut guard = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        let mut next = guard.runs.clone();
        let run = next.get_mut(workflow_id).ok_or("WO9_GRAPH_UNKNOWN_RUN")?;
        if run.tasks.iter().any(|t| t.state == "running" || t.state == "interrupted") {
            return Err("WO9_GRAPH_RUNNING_OR_QUARANTINED");
        }
        let current = run.tasks.get(step).ok_or("WO9_GRAPH_UNKNOWN_STEP")?.clone();
        if current.state != "pending" { return Err("WO9_GRAPH_STEP_NOT_PENDING"); }
        for dep in &current.spec.depends_on {
            if run.tasks.get(*dep).map(|t| t.state.as_str()) != Some("passed") {
                return Err("WO9_GRAPH_DEPENDENCY_NOT_PASSED");
            }
        }
        registry.verify_multi_project_workflow(&run.project_ids, &run.source_fingerprint)?;
        run.tasks[step].state = "running".into();
        save(&self.file, &Store { version: 1, runs: next.clone() })?;
        guard.runs = next;
        Ok(current.spec)
    }
    pub fn pending_native_recheck(&self, workflow_id: &str, step: usize)
        -> Result<WorkflowTask, &'static str> {
        let guard = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        let run = guard.runs.get(workflow_id).ok_or("WO9_GRAPH_UNKNOWN_RUN")?;
        let task = run.tasks.get(step).ok_or("WO9_GRAPH_UNKNOWN_STEP")?;
        if task.state != "native_receipt_unverified"
            || task.native_job_id.is_none()
            || task.native_plan_sha256.is_none()
            || task.native_output_sha256.is_none() {
            return Err("WO9_NATIVE_RECEIPT_NOT_REVALIDATABLE");
        }
        Ok(task.clone())
    }

    /// The caller must first independently validate exact project/job/plan
    /// against the existing WO6 host-owned protected supervisor journal.
    /// No WebView-supplied phase, OS exit code or success is accepted.
    pub fn accept_rechecked_native_pass(
        &self, workflow_id: &str, step: usize, job_id: &str, plan_sha: &str,
    ) -> Result<WorkflowRun, &'static str> {
        let mut guard = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        let mut next = guard.runs.clone();
        let run = next.get_mut(workflow_id).ok_or("WO9_GRAPH_UNKNOWN_RUN")?;
        let task = run.tasks.get_mut(step).ok_or("WO9_GRAPH_UNKNOWN_STEP")?;
        if task.state != "native_receipt_unverified"
            || task.native_job_id.as_deref() != Some(job_id)
            || task.native_plan_sha256.as_deref() != Some(plan_sha) {
            return Err("WO9_NATIVE_RECEIPT_STALE_OR_WRONG_JOB");
        }
        task.state = "passed".into();
        task.error_code = None;
        let accepted = run.clone();
        save(&self.file, &Store { version: 1, runs: next.clone() })?;
        guard.runs = next;
        Ok(accepted)
    }

    /// Called ONLY by native Tauri host after WO6 supervisor returns its own
    /// authoritative receipt. No WebView command accepts outcome fields.
    pub fn finish_step(&self, workflow_id: &str, step: usize,
        native_job_id: Option<String>, plan_sha: Option<String>,
        output_sha: Option<String>, phase: &str, code: Option<i64>,
        needs_reconciliation: bool, error: Option<String>)
        -> Result<WorkflowRun, &'static str> {
        let mut guard = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        let mut next = guard.runs.clone();
        let run = next.get_mut(workflow_id).ok_or("WO9_GRAPH_UNKNOWN_RUN")?;
        let task = run.tasks.get_mut(step).ok_or("WO9_GRAPH_UNKNOWN_STEP")?;
        if task.state != "running" { return Err("WO9_GRAPH_STALE_TERMINAL_RECEIPT"); }
        task.state = if needs_reconciliation || native_job_id.is_none()
            || phase == "uncertain" || phase == "recoveryrequired" {
            "interrupted"
        } else if phase == "completed" && code == Some(0)
            && output_sha.as_deref().is_some_and(|sha| sha.len() == 64) {
            "passed"
        } else { "failed" }.into();
        task.native_job_id = native_job_id;
        task.native_plan_sha256 = plan_sha;
        task.native_output_sha256 = output_sha;
        task.error_code = error;
        let snapshot = run.clone();
        save(&self.file, &Store { version: 1, runs: next.clone() })?;
        guard.runs = next;
        Ok(snapshot)
    }
    /// Any failure blocks descendants. Cancellation marks the workflow
    /// permanently blocked unless independent existing OS cleanup proves
    /// safety; no silent retry or relaunch after desktop restart.
    pub fn cancel_pending(&self, workflow_id: &str, step: usize)
        -> Result<WorkflowRun, &'static str> {
        let mut guard = self.inner.lock().map_err(|_| "WO9_GRAPH_MUTEX_UNAVAILABLE")?;
        let mut next = guard.runs.clone();
        let run = next.get_mut(workflow_id).ok_or("WO9_GRAPH_UNKNOWN_RUN")?;
        let task = run.tasks.get_mut(step).ok_or("WO9_GRAPH_UNKNOWN_STEP")?;
        if task.state != "pending" { return Err("WO9_GRAPH_CANCEL_NEEDS_OS_SUPERVISOR"); }
        task.state = "cancelled".into();
        let snapshot = run.clone();
        save(&self.file, &Store { version: 1, runs: next.clone() })?;
        guard.runs = next;
        Ok(snapshot)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn spec(project: &str, deps: &[usize]) -> WorkflowStep {
        WorkflowStep { project_id: project.into(), recipe_id:"rust_tests".into(),
            depends_on:deps.to_vec() }
    }
    struct Temp(std::path::PathBuf);
    impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    struct NativePick(std::path::PathBuf);
    impl crate::NativeDirectoryPicker for NativePick {
        fn choose_project_directory(&mut self) -> Result<Option<std::path::PathBuf>, crate::IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    fn native_fixture() -> (Temp, IdeWorkspaceRegistry, String, String) {
        use git2::{Repository, Signature};
        let root = std::env::temp_dir().join(format!("wo9-dag-{}",rand::random::<u64>()));
        let project = root.join("picked-project");
        fs::create_dir_all(project.join("dist")).unwrap();
        fs::write(project.join("dist/data.tar"),b"fixed data").unwrap();
        let repo = Repository::init(&project).unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(std::path::Path::new("dist/data.tar")).unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let who = Signature::now("WO9", "wo9@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &who, &who, "fixed", &tree, &[]).unwrap();
        drop(tree); drop(idx); drop(repo);
        let registry = IdeWorkspaceRegistry::open(root.join("projects.json")).unwrap();
        let id = registry.register_from_native_picker(&mut NativePick(project))
            .unwrap().unwrap().project_id;
        let inspection = registry.inspect_multi_project_workflow(&[id.clone()]).unwrap();
        assert!(inspection.ready_for_review);
        (Temp(root), registry, id, inspection.fingerprint)
    }
    #[test]
    fn dependency_requires_native_completed_receipt_and_new_user_turn_per_step() {
        let (temp, registry, id, fingerprint) = native_fixture();
        let coordinator = WorkflowCoordinator::new(temp.0.join("workflow.json")).unwrap();
        let one = spec(&id, &[]);
        let two = spec(&id, &[0]);
        let run = coordinator.create(&registry,&[id.clone()],&fingerprint,
            vec![one,two]).unwrap();
        assert!(!run.automatic_execution && !run.release_approved);
        assert_eq!(coordinator.begin_step(&registry,&run.workflow_id,1).unwrap_err(),
            "WO9_GRAPH_DEPENDENCY_NOT_PASSED");
        assert!(coordinator.begin_step(&registry,&run.workflow_id,0).is_ok());
        assert_eq!(coordinator.begin_step(&registry,&run.workflow_id,0).unwrap_err(),
            "WO9_GRAPH_RUNNING_OR_QUARANTINED");
        let first = coordinator.finish_step(&run.workflow_id,0,
            Some("native-job-a".into()),Some("a".repeat(64)),
            Some("b".repeat(64)),"completed",Some(0),false,None).unwrap();
        assert_eq!(first.tasks[0].state,"passed");
        assert!(coordinator.begin_step(&registry,&run.workflow_id,1).is_ok());
        let second = coordinator.finish_step(&run.workflow_id,1,
            Some("native-job-b".into()),None,None,"failed",Some(1),false,
            Some("WO9_NATIVE_FAILED".into())).unwrap();
        assert_eq!(second.tasks[1].state,"failed");
        assert!(second.tasks.iter().all(|t| t.state != "running"));
        assert!(!second.release_approved);
        let reloaded = WorkflowCoordinator::new(temp.0.join("workflow.json")).unwrap();
        assert_eq!(reloaded.list().unwrap()[0].tasks[0].native_job_id.as_deref(),
            Some("native-job-a"));
    }
    #[test]
    fn denied_native_receipt_does_not_unblock_dependents_or_replay() {
        let (temp, registry, id, fingerprint) = native_fixture();
        let coordinator = WorkflowCoordinator::new(temp.0.join("workflow.json")).unwrap();
        let run = coordinator.create(&registry,&[id.clone()],&fingerprint,
            vec![spec(&id,&[]),spec(&id,&[0])]).unwrap();
        coordinator.begin_step(&registry,&run.workflow_id,0).unwrap();
        let uncertain = coordinator.finish_step(&run.workflow_id,0,None,None,None,
            "uncertain",None,true,Some("BUILD_ADMISSION_DENIED".into())).unwrap();
        assert_eq!(uncertain.tasks[0].state,"interrupted");
        assert_eq!(coordinator.begin_step(&registry,&run.workflow_id,1).unwrap_err(),
            "WO9_GRAPH_RUNNING_OR_QUARANTINED");
    }
    #[test]
    fn restart_invalidates_previous_pass_until_native_journal_rechecked() {
        let (temp, registry, project_id, digest) = native_fixture();
        let file = temp.0.join("safe-restart.json");
        let coordinator = WorkflowCoordinator::new(file.clone()).unwrap();
        let run = coordinator.create(&registry,&[project_id.clone()],&digest,
            vec![spec(&project_id,&[]),spec(&project_id,&[0])]).unwrap();
        coordinator.begin_step(&registry,&run.workflow_id,0).unwrap();
        coordinator.finish_step(&run.workflow_id,0,
            Some("native-job-identity".into()),Some("b".repeat(64)),
            Some("c".repeat(64)),"completed",Some(0),false,None).unwrap();
        drop(coordinator);
        let rebooted = WorkflowCoordinator::new(file.clone()).unwrap();
        assert_eq!(rebooted.list().unwrap()[0].tasks[0].state,
            "native_receipt_unverified");
        assert_eq!(rebooted.begin_step(&registry,&run.workflow_id,1).unwrap_err(),
            "WO9_GRAPH_DEPENDENCY_NOT_PASSED");
        let task = rebooted.pending_native_recheck(&run.workflow_id,0).unwrap();
        assert_eq!(task.native_job_id.as_deref(),Some("native-job-identity"));
        assert_eq!(rebooted.accept_rechecked_native_pass(&run.workflow_id,0,
            "wrong-native-job",&"b".repeat(64)).unwrap_err(),
            "WO9_NATIVE_RECEIPT_STALE_OR_WRONG_JOB");
        rebooted.accept_rechecked_native_pass(&run.workflow_id,0,
            "native-job-identity",&"b".repeat(64)).unwrap();
        assert_eq!(rebooted.begin_step(&registry,&run.workflow_id,1).unwrap().recipe_id,
            "rust_tests");
        drop(rebooted);
        let rebooted_again = WorkflowCoordinator::new(file).unwrap();
        assert_eq!(rebooted_again.list().unwrap()[0].tasks[0].state,
            "native_receipt_unverified");
    }

    #[test]
    fn cycle_and_unknown_recipe_fail() {
        assert!(allowed_spec(&[spec("a",&[0])], &["a".into()]).is_err());
        assert!(allowed_spec(&[spec("a",&[]),spec("b",&[1])],
            &["a".into(),"b".into()]).is_err());
        assert!(allowed_spec(&[spec("a",&[]),spec("b",&[0])],
            &["a".into(),"b".into()]).is_ok());
    }
    #[test]
    fn terminal_os_cancel_stays_failed_after_restart_and_blocks_dependents() {
        let (temp, registry, project_id, digest) = native_fixture();
        let file = temp.0.join("cancelled-graph.json");
        let coordinator = WorkflowCoordinator::new(file.clone()).unwrap();
        let run = coordinator.create(&registry, &[project_id.clone()], &digest,
            vec![spec(&project_id, &[]), spec(&project_id, &[0])]).unwrap();
        coordinator.begin_step(&registry, &run.workflow_id, 0).unwrap();
        let cancelled = coordinator.finish_step(&run.workflow_id, 0,
            Some("job-cancelled-by-native-os".into()), Some("a".repeat(64)),
            None, "cancelled", Some(130), false,
            Some("WO9_KERNEL_TREE_REAPED".into())).unwrap();
        assert_eq!(cancelled.tasks[0].state, "failed");
        assert_eq!(coordinator.begin_step(&registry, &run.workflow_id, 1).unwrap_err(),
            "WO9_GRAPH_DEPENDENCY_NOT_PASSED");
        drop(coordinator);
        let after_restart = WorkflowCoordinator::new(file).unwrap();
        let stored = after_restart.list().unwrap();
        assert_eq!(stored[0].tasks[0].state, "failed");
        assert_eq!(stored[0].tasks[1].state, "pending");
        assert!(!stored[0].automatic_execution);
        assert_eq!(after_restart.begin_step(&registry, &run.workflow_id, 1).unwrap_err(),
            "WO9_GRAPH_DEPENDENCY_NOT_PASSED");
    }

    #[test]
    fn host_crash_fsyncs_interrupted_and_blocks_silent_resume() {
        let path = std::env::temp_dir().join(format!("wo9-graph-{}.json",rand::random::<u64>()));
        let coordinator = WorkflowCoordinator::new(path.clone()).unwrap();
        let run = WorkflowRun {
            workflow_id:"wo9-test".into(),project_ids:vec!["p".into()],
            source_fingerprint:"a".repeat(64),
            tasks:vec![WorkflowTask {spec:spec("p",&[]),state:"running".into(),
                native_job_id:None,native_plan_sha256:None,native_output_sha256:None,
                error_code:None}],
            release_approved:false,automatic_execution:false,
        };
        let mut entries = BTreeMap::new();entries.insert(run.workflow_id.clone(),run);
        save(&path,&Store {version:1,runs:entries}).unwrap();
        drop(coordinator);
        let reopened = WorkflowCoordinator::new(path.clone()).unwrap();
        assert_eq!(reopened.list().unwrap()[0].tasks[0].state, "interrupted");
        assert_eq!(reopened.list().unwrap()[0].tasks[0].error_code.as_deref(),
            Some("WO9_HOST_CRASH_RECONCILIATION_REQUIRED"));
        assert!(!reopened.list().unwrap()[0].automatic_execution);
        let _ = fs::remove_file(path);
    }
}
