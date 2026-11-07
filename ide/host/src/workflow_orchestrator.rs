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
        for run in store.runs.values_mut() {
            if run.release_approved || run.automatic_execution {
                return Err("WO9_GRAPH_UNTRUSTED_STATE");
            }
            for task in &mut run.tasks {
                if task.state == "running" {
                    task.state = "interrupted".into();
                    task.error_code = Some("WO9_HOST_CRASH_RECONCILIATION_REQUIRED".into());
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
    #[test]
    fn cycle_and_unknown_recipe_fail() {
        assert!(allowed_spec(&[spec("a",&[0])], &["a".into()]).is_err());
        assert!(allowed_spec(&[spec("a",&[]),spec("b",&[1])],
            &["a".into(),"b".into()]).is_err());
        assert!(allowed_spec(&[spec("a",&[]),spec("b",&[0])],
            &["a".into(),"b".into()]).is_ok());
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
