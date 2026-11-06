//! C6: project-scoped task evidence in the existing private append-only
//! development journal. Human task notes are not test/installation receipts.
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    continuity::{ContinuityError, DevelopmentEvent, DevelopmentLedger},
    github_ci::ExactHeadCiEvidence,
    IdeWorkspaceRegistry,
};

pub const MAX_TASKS: usize = 64;
pub const MAX_GATES: usize = 4;
const GATES: [&str; 4] = [
    "source_review",
    "exact_head_ci",
    "installed_windows",
    "human_approval",
];
const STATES: [&str; 4] = ["planned", "in_progress", "blocked", "ready_for_review"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskCiWorkflow {
    pub workflow: String,
    pub run_id: Option<u64>,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskCiObservation {
    pub source: String,
    pub observed_unix_seconds: Option<u64>,
    pub repository: Option<String>,
    pub commit_sha: String,
    pub verdict: String,
    pub error_code: Option<String>,
    pub workflows: Vec<TaskCiWorkflow>,
    pub installation_acceptance: String,
    pub task_acceptance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskEvent {
    pub task_id: String,
    /// "human_update" or "ci_observation"; records never assert completion.
    pub kind: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub next_action: Option<String>,
    pub required_gates: Vec<String>,
    pub linked_commit: Option<String>,
    pub linked_worktree: String,
    pub bound_update_event_id: Option<String>,
    pub ci: Option<TaskCiObservation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGateEvidence {
    pub gate: String,
    pub state: &'static str,
    pub provenance: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub next_action: String,
    pub required_gates: Vec<String>,
    pub update_event_id: String,
    pub linked_commit: Option<String>,
    pub linked_worktree: String,
    pub updated_unix_seconds: u64,
    pub observed_ci: Option<TaskCiObservation>,
    pub ci_link_state: &'static str,
    pub acceptance_gates: Vec<TaskGateEvidence>,
    pub task_acceptance: &'static str,
}

fn valid_task_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

fn valid_note(s: &str, max: usize, nonempty: bool) -> bool {
    (!nonempty || !s.trim().is_empty()) && s.chars().count() <= max
        && !s.chars().any(char::is_control)
}

fn valid_gates(gates: &[String]) -> bool {
    !gates.is_empty() && gates.len() <= MAX_GATES
        && gates.iter().all(|gate| GATES.contains(&gate.as_str()))
        && gates.iter().enumerate().all(|(i, gate)|
            !gates[..i].iter().any(|prev| prev == gate))
}

fn valid_commit(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|c| c.is_ascii_hexdigit())
}

fn now() -> Result<u64, ContinuityError> {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs()).map_err(|_| ContinuityError::Unavailable)
}

fn event_id() -> String {
    format!("evt-{:032x}", rand::random::<u128>())
}

fn event(
    project_id: &str,
    operation: &str,
    outcome: &str,
    task: TaskEvent,
) -> Result<DevelopmentEvent, ContinuityError> {
    Ok(DevelopmentEvent {
        schema_version: 1,
        event_id: event_id(),
        observed_unix_seconds: now()?,
        project_id: project_id.to_owned(),
        operation: operation.to_owned(),
        outcome: outcome.to_owned(),
        source_revision: None,
        source_path: None,
        summary: if operation == "task.update" {
            "Human task note recorded; verification separate"
        } else {
            "Native public GitHub Actions observation linked; acceptance separate"
        }.to_owned(),
        task: Some(task),
    })
}

/// Reconstruct current task claims from the single ordered development journal.
/// CI observations only count while bound to the *latest* human update event.
/// A subsequent task update invalidates old attached CI, even for the same SHA.
pub fn project_tasks(
    events: &[DevelopmentEvent],
    project_id: &str,
    current_local_head: Option<&str>,
) -> Result<Vec<TaskSummary>, ContinuityError> {
    let mut tasks: BTreeMap<String, TaskSummary> = BTreeMap::new();
    for event in events.iter().filter(|e| e.project_id == project_id) {
        let Some(task) = event.task.as_ref() else { continue };
        if !valid_task_id(&task.task_id) { return Err(ContinuityError::Corrupt); }
        match (event.operation.as_str(), task.kind.as_str()) {
            ("task.update", "human_update") => {
                let (Some(title), Some(status), Some(next)) = (
                    task.title.as_ref(), task.status.as_ref(), task.next_action.as_ref()
                ) else { return Err(ContinuityError::Corrupt) };
                if !valid_note(title, 120, true)
                    || !valid_note(next, 240, true)
                    || !STATES.contains(&status.as_str())
                    || !valid_gates(&task.required_gates)
                    || task.bound_update_event_id.is_some() || task.ci.is_some()
                    || task.linked_commit.as_deref().is_some_and(|s| !valid_commit(s))
                    || task.linked_worktree.len() > 64 {
                    return Err(ContinuityError::Corrupt);
                }
                tasks.insert(task.task_id.clone(), TaskSummary {
                    task_id: task.task_id.clone(),
                    title: title.clone(), status: status.clone(),
                    next_action: next.clone(),
                    required_gates: task.required_gates.clone(),
                    update_event_id: event.event_id.clone(),
                    linked_commit: task.linked_commit.clone(),
                    linked_worktree: task.linked_worktree.clone(),
                    updated_unix_seconds: event.observed_unix_seconds,
                    observed_ci: None, ci_link_state: "not_observed",
                    acceptance_gates: Vec::new(), task_acceptance: "not_verified",
                });
                if tasks.len() > MAX_TASKS { return Err(ContinuityError::Full); }
            }
            ("task.ci.observe", "ci_observation") => {
                let Some(existing) = tasks.get_mut(&task.task_id) else {
                    return Err(ContinuityError::Corrupt);
                };
                let Some(ci) = task.ci.as_ref() else { return Err(ContinuityError::Corrupt) };
                if task.title.is_some() || task.status.is_some()
                    || task.next_action.is_some() || !task.required_gates.is_empty()
                    || task.bound_update_event_id.as_deref() != Some(existing.update_event_id.as_str())
                    || existing.linked_commit.as_deref() != Some(ci.commit_sha.as_str())
                    || task.linked_commit.as_deref() != Some(ci.commit_sha.as_str())
                    || ci.source != "public_github_actions_explicit_read"
                    || ci.workflows.len() > 2
                    || ci.installation_acceptance != "not_tested"
                    || ci.task_acceptance != "not_verified"
                    || !valid_commit(&ci.commit_sha)
                    || ci.repository.as_ref().is_some_and(|s| s.len() > 205)
                    || ci.error_code.as_ref().is_some_and(|s| s.len() > 80) {
                    return Err(ContinuityError::Corrupt);
                }
                existing.observed_ci = Some(ci.clone());
            }
            _ => return Err(ContinuityError::Corrupt),
        }
    }
    let mut out: Vec<TaskSummary> = tasks.into_values().collect();
    for task in &mut out {
        let source_matches = task.linked_commit.as_deref() == current_local_head
            && task.linked_commit.is_some();
        task.ci_link_state = if task.observed_ci.is_none() {
            "not_observed"
        } else if !source_matches {
            "linked_commit_not_current"
        } else {
            "current_commit_observation"
        };
        task.acceptance_gates = task.required_gates.iter().map(|gate| {
            let (state, provenance) = match gate.as_str() {
                "exact_head_ci" if source_matches => {
                    let verdict = task.observed_ci.as_ref().map(|ci| ci.verdict.as_str());
                    (match verdict {
                        Some("exact_commit_ci_passed") => "commit_ci_passed_not_task_accepted",
                        Some("exact_head_ci_failed") => "commit_ci_failed",
                        Some(_) => "ci_incomplete",
                        None => "not_observed",
                    }, "native_exact_sha_github_actions_observation")
                }
                "exact_head_ci" => ("commit_not_current_or_unknown", "recorded_source_commit"),
                "source_review" => ("not_verified", "no_source_review_receipt"),
                "installed_windows" => ("not_tested", "no_installed_desktop_receipt"),
                "human_approval" => ("not_recorded", "no_explicit_acceptance_record"),
                _ => ("unverifiable", "unknown_gate"),
            };
            TaskGateEvidence { gate: gate.clone(), state, provenance }
        }).collect();
    }
    out.sort_by(|a, b| b.updated_unix_seconds.cmp(&a.updated_unix_seconds)
        .then_with(|| a.task_id.cmp(&b.task_id)));
    Ok(out)
}

fn snapshot_ci(observed: &ExactHeadCiEvidence) -> Result<TaskCiObservation, ContinuityError> {
    let head = observed.local_head.as_deref()
        .filter(|s| valid_commit(s)).ok_or(ContinuityError::TaskStale)?;
    if observed.workflows.len() > 2 { return Err(ContinuityError::Corrupt); }
    Ok(TaskCiObservation {
        source: "public_github_actions_explicit_read".to_owned(),
        observed_unix_seconds: observed.checked_unix_seconds,
        repository: observed.repository.clone(),
        commit_sha: head.to_owned(),
        verdict: observed.verdict.to_owned(),
        error_code: observed.error_code.map(ToOwned::to_owned),
        workflows: observed.workflows.iter().map(|w| TaskCiWorkflow {
            workflow: w.workflow.to_owned(),
            run_id: w.run_id,
            verdict: w.verdict.to_owned(),
        }).collect(),
        installation_acceptance: "not_tested".to_owned(),
        task_acceptance: "not_verified".to_owned(),
    })
}

impl DevelopmentLedger {
    /// Human UI only: append a task claim; host captures actual Git HEAD.
    /// No "completed"/"passed" input is accepted.
    pub fn record_task_update(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        task_id: &str,
        title: &str,
        status: &str,
        next_action: &str,
        required_gates: Vec<String>,
    ) -> Result<TaskSummary, ContinuityError> {
        if !valid_task_id(task_id)
            || !valid_note(title, 120, true)
            || !valid_note(next_action, 240, true)
            || !STATES.contains(&status)
            || !valid_gates(&required_gates) {
            return Err(ContinuityError::InvalidInput);
        }
        registry.registered_git_root(project_id)
            .map_err(|_| ContinuityError::ProjectUnavailable)?;
        let git = registry.inspect_git_worktree(project_id);
        let current_head = git.local_head.as_deref().filter(|s| valid_commit(s));
        let task = TaskEvent {
            task_id: task_id.to_owned(),
            kind: "human_update".to_owned(),
            title: Some(title.to_owned()),
            status: Some(status.to_owned()),
            next_action: Some(next_action.to_owned()),
            required_gates, linked_commit: current_head.map(ToOwned::to_owned),
            linked_worktree: git.worktree.to_owned(),
            bound_update_event_id: None, ci: None,
        };
        let record = event(project_id, "task.update", "human_task_note_recorded", task)?;
        self.append_transaction(|prior| {
            let mut combined = prior.to_vec();
            combined.push(record.clone());
            let view = project_tasks(&combined, project_id, current_head)?
                .into_iter().find(|t| t.task_id == task_id)
                .ok_or(ContinuityError::Corrupt)?;
            Ok((record, view))
        })
    }

    /// Human UI only: native host independently queries the same exact-head
    /// CI operation as C5, then links an observed result to a specific task
    /// revision. The browser never supplies verdicts, job IDs or SHAs.
    pub fn record_task_ci(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
        task_id: &str,
    ) -> Result<TaskSummary, ContinuityError> {
        if !valid_task_id(task_id) { return Err(ContinuityError::InvalidInput); }
        registry.registered_git_root(project_id)
            .map_err(|_| ContinuityError::ProjectUnavailable)?;
        let before = registry.inspect_git_worktree(project_id);
        let summaries = project_tasks(&self.events()?, project_id, before.local_head.as_deref())?;
        let task = summaries.into_iter().find(|t| t.task_id == task_id)
            .ok_or(ContinuityError::TaskNotFound)?;
        let Some(bound_head) = task.linked_commit.as_deref() else {
            return Err(ContinuityError::TaskStale);
        };
        if before.local_head.as_deref() != Some(bound_head) {
            return Err(ContinuityError::TaskStale);
        }
        let observed = registry.inspect_exact_head_ci(project_id);
        let after = registry.inspect_git_worktree(project_id);
        if observed.local_head.as_deref() != Some(bound_head)
            || observed.local_head_changed_during_check
            || after.local_head.as_deref() != Some(bound_head) {
            return Err(ContinuityError::TaskStale);
        }
        let task_event = TaskEvent {
            task_id: task_id.to_owned(),
            kind: "ci_observation".to_owned(),
            title: None, status: None, next_action: None, required_gates: Vec::new(),
            linked_commit: Some(bound_head.to_owned()),
            linked_worktree: after.worktree.to_owned(),
            bound_update_event_id: Some(task.update_event_id.clone()),
            ci: Some(snapshot_ci(&observed)?),
        };
        let record = event(project_id, "task.ci.observe",
            "host_ci_observation_recorded_not_acceptance", task_event)?;
        self.append_transaction(|prior| {
            let latest = project_tasks(prior, project_id, after.local_head.as_deref())?
                .into_iter().find(|t| t.task_id == task_id)
                .ok_or(ContinuityError::TaskNotFound)?;
            if latest.update_event_id != task.update_event_id
                || latest.linked_commit != task.linked_commit {
                return Err(ContinuityError::TaskStale);
            }
            let mut combined = prior.to_vec();
            combined.push(record.clone());
            let view = project_tasks(&combined, project_id, after.local_head.as_deref())?
                .into_iter().find(|t| t.task_id == task_id)
                .ok_or(ContinuityError::Corrupt)?;
            Ok((record, view))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_event(id: &str, sha: &str) -> DevelopmentEvent {
        event("test-project", "task.update", "human_task_note_recorded", TaskEvent {
            task_id: id.to_owned(),
            kind: "human_update".into(),
            title: Some("Release gate".into()),
            status: Some("ready_for_review".into()),
            next_action: Some("Run native CI".into()),
            required_gates: GATES.iter().map(|s| s.to_string()).collect(),
            linked_commit: Some(sha.into()),
            linked_worktree: "dirty".into(),
            bound_update_event_id: None,
            ci: None,
        }).unwrap()
    }

    #[test]
    fn task_updates_are_claims_not_acceptance() {
        let sha = "a".repeat(40);
        let input = vec![task_event("C6", &sha)];
        let tasks = project_tasks(&input, "test-project", Some(&sha)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, "ready_for_review");
        assert_eq!(tasks[0].task_acceptance, "not_verified");
        assert_eq!(tasks[0].acceptance_gates[1].state, "not_observed");
        assert_eq!(tasks[0].acceptance_gates[2].state, "not_tested");
        assert_eq!(tasks[0].acceptance_gates[3].state, "not_recorded");
        assert!(project_tasks(&input, "another-project", Some(&sha)).unwrap().is_empty());
    }

    #[test]
    fn ci_is_linked_to_exact_task_update_and_never_completes_task() {
        let sha = "a".repeat(40);
        let update = task_event("C6", &sha);
        let receipt = TaskCiObservation {
            source: "public_github_actions_explicit_read".into(),
            observed_unix_seconds: Some(10),
            repository: Some("owner/repo".into()),
            commit_sha: sha.clone(),
            verdict: "exact_commit_ci_passed".into(),
            error_code: None,
            workflows: vec![],
            installation_acceptance: "not_tested".into(),
            task_acceptance: "not_verified".into(),
        };
        let observation = event("test-project", "task.ci.observe",
            "host_ci_observation_recorded_not_acceptance", TaskEvent {
                task_id: "C6".into(), kind: "ci_observation".into(), title: None,
                status: None, next_action: None, required_gates: vec![],
                linked_commit: Some(sha.clone()), linked_worktree: "dirty".into(),
                bound_update_event_id: Some(update.event_id.clone()), ci: Some(receipt),
            }).unwrap();
        let view = project_tasks(&[update.clone(),observation.clone()],
            "test-project",Some(&sha)).unwrap();
        assert_eq!(view[0].acceptance_gates[1].state, "commit_ci_passed_not_task_accepted");
        assert_eq!(view[0].task_acceptance, "not_verified");
        let stale = project_tasks(&[update.clone(),observation.clone()],
            "test-project",Some(&"b".repeat(40))).unwrap();
        assert_eq!(stale[0].acceptance_gates[1].state, "commit_not_current_or_unknown");
        let revised = task_event("C6", &sha);
        let reset = project_tasks(&[update, observation, revised],
            "test-project",Some(&sha)).unwrap();
        assert!(reset[0].observed_ci.is_none());
        assert_eq!(reset[0].ci_link_state, "not_observed");
    }

    #[test]
    fn reject_invalid_task_fields_and_gates() {
        assert!(!valid_task_id("../escape"));
        assert!(!valid_task_id(""));
        assert!(!valid_note("hi\nraw", 120, true));
        assert!(!valid_note("", 120, true));
        assert!(!valid_gates(&[]));
        assert!(!valid_gates(&["exact_head_ci".into(),"exact_head_ci".into()]));
        assert!(!valid_gates(&["build.run".into()]));
        assert!(valid_gates(&["source_review".into(),"exact_head_ci".into()]));
    }
    #[test]
    fn single_ledger_reopens_legacy_events_and_task_updates() {
        let path = std::env::temp_dir().join(format!(
            "ide-c6-test-{}-{}.jsonl",
            std::process::id(),
            rand::random::<u128>()
        ));
        let legacy = DevelopmentEvent {
            schema_version: 1,
            event_id: event_id(),
            observed_unix_seconds: now().unwrap(),
            project_id: "test-project".into(),
            operation: "project.open".into(),
            outcome: "observed_host_success".into(),
            source_revision: None,
            source_path: None,
            summary: "Opened".into(),
            task: None,
        };
        let ledger = DevelopmentLedger::new(path.clone()).unwrap();
        ledger.append_transaction(|prior| {
            assert!(prior.is_empty());
            Ok((legacy.clone(), ()))
        }).unwrap();
        let task = task_event("C6-restart", &"a".repeat(40));
        ledger.append_transaction(|prior| {
            assert_eq!(prior.len(), 1);
            Ok((task.clone(), ()))
        }).unwrap();
        drop(ledger);
        let reopened = DevelopmentLedger::new(path.clone()).unwrap();
        let persisted = reopened.events().unwrap();
        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[0].operation, "project.open");
        assert!(persisted[0].task.is_none());
        let state = project_tasks(&persisted, "test-project",
            Some(&"a".repeat(40))).unwrap();
        assert_eq!(state.len(), 1);
        assert_eq!(state[0].task_id, "C6-restart");
        assert_eq!(state[0].task_acceptance, "not_verified");
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn truncated_journal_fails_closed_on_reopen() {
        let path = std::env::temp_dir().join(format!(
            "ide-c6-corrupt-{}-{}.jsonl",
            std::process::id(),
            rand::random::<u128>()
        ));
        std::fs::write(&path, b"{\"schemaVersion\":1").unwrap();
        let ledger = DevelopmentLedger::new(path.clone()).unwrap();
        assert!(matches!(ledger.events(), Err(ContinuityError::Corrupt)));
        drop(ledger);
        std::fs::remove_file(path).unwrap();
    }

}
