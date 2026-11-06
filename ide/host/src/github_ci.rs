//! C5: explicit, bounded public GitHub Actions evidence for the exact local commit.
//! No repository writes, Git CLI, credential use, arbitrary URLs, or job execution.
use crate::{github_live::github_repo, IdeWorkspaceRegistry};
use git2::Repository;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_RUN_RESPONSE_BYTES: u64 = 512 * 1024;
const MAX_JOBS_RESPONSE_BYTES: u64 = 512 * 1024;
const MAX_JOBS: usize = 80;
const MAX_RUNS: usize = 30;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiJobEvidence {
    pub name: &'static str,
    pub status: &'static str,
    pub conclusion: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiWorkflowEvidence {
    pub workflow: &'static str,
    pub run_id: Option<u64>,
    pub run_url: Option<String>,
    pub head_sha: Option<String>,
    pub run_status: &'static str,
    pub conclusion: &'static str,
    pub jobs: Vec<CiJobEvidence>,
    pub verdict: &'static str,
    pub jobs_truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactHeadCiEvidence {
    pub schema_version: u32,
    pub project_id: String,
    pub source: &'static str,
    pub repository: Option<String>,
    pub local_head: Option<String>,
    pub worktree: &'static str,
    pub checked_unix_seconds: Option<u64>,
    pub verdict: &'static str,
    pub error_code: Option<&'static str>,
    pub workflows: Vec<CiWorkflowEvidence>,
    pub runs_listing_partial: bool,
    pub local_head_changed_during_check: bool,
    pub installation_acceptance: &'static str,
    pub task_completion_acceptance: &'static str,
    pub credentials_used: bool,
}

const REQUIRED: [(&str, &str, &[&str]); 2] = [
    ("standalone_ide", ".github/workflows/ide-standalone.yml",
     &["host-linux", "frontend", "desktop-windows"]),
    ("continuity", ".github/workflows/ide-continuity.yml", &["continuity"]),
];

fn valid_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|c| c.is_ascii_hexdigit())
}

fn status(s: Option<&str>) -> &'static str {
    match s {
        Some("queued") | Some("waiting") | Some("pending") | Some("requested") => "queued",
        Some("in_progress") => "in_progress",
        Some("completed") => "completed",
        _ => "unknown",
    }
}

fn conclusion(s: Option<&str>) -> &'static str {
    match s {
        Some("success") => "success",
        Some("failure") => "failure",
        Some("cancelled") => "cancelled",
        Some("timed_out") => "timed_out",
        Some("skipped") => "skipped",
        Some("neutral") => "neutral",
        Some("action_required") => "action_required",
        _ => "unknown",
    }
}

fn empty_workflow(name: &'static str, jobs: &'static [&'static str]) -> CiWorkflowEvidence {
    CiWorkflowEvidence {
        workflow: name,
        run_id: None,
        run_url: None,
        head_sha: None,
        run_status: "not_observed",
        conclusion: "unknown",
        jobs: jobs.iter().map(|name| CiJobEvidence {
            name, status: "not_observed", conclusion: "unknown",
        }).collect(),
        verdict: "not_observed",
        jobs_truncated: false,
    }
}

/// Strictly match a workflow definition, not just arbitrary run/job labels.
fn matches_workflow(run: &Value, path: &str, head: &str) -> bool {
    run.get("head_sha").and_then(Value::as_str) == Some(head)
        && run.get("path").and_then(Value::as_str)
            .and_then(|s| s.split('@').next()) == Some(path)
        && run.get("id").and_then(Value::as_u64).is_some_and(|id| id > 0)
}

/// Pure classifier: successful run metadata without all required exact-head jobs is not proof.
fn assess_workflow(
    name: &'static str,
    expected: &'static [&'static str],
    run: &Value,
    jobs_payload: Option<&Value>,
    head: &str,
    repository: &str,
) -> CiWorkflowEvidence {
    let mut out = empty_workflow(name, expected);
    let Some(run_id) = run.get("id").and_then(Value::as_u64) else { return out };
    out.run_id = Some(run_id);
    out.run_url = Some(format!("https://github.com/{repository}/actions/runs/{run_id}"));
    out.head_sha = Some(head.to_owned());
    out.run_status = status(run.get("status").and_then(Value::as_str));
    out.conclusion = conclusion(run.get("conclusion").and_then(Value::as_str));

    let Some(payload) = jobs_payload else {
        out.verdict = "jobs_unavailable";
        return out;
    };
    let Some(all_jobs) = payload.get("jobs").and_then(Value::as_array) else {
        out.verdict = "jobs_unavailable";
        return out;
    };
    out.jobs_truncated = all_jobs.len() > MAX_JOBS
        || payload.get("total_count").and_then(Value::as_u64)
            .is_none_or(|total| total > MAX_JOBS as u64 || total > all_jobs.len() as u64);
    if out.jobs_truncated {
        out.verdict = "jobs_partial";
        return out;
    }

    let mut complete = true;
    let mut failed = false;
    for job in &mut out.jobs {
        let matches: Vec<&Value> = all_jobs.iter().filter(|item|
            item.get("name").and_then(Value::as_str) == Some(job.name)
        ).collect();
        if matches.len() != 1 {
            complete = false;
            job.status = if matches.is_empty() { "not_observed" } else { "ambiguous" };
            continue;
        }
        let item = matches[0];
        if item.get("head_sha").and_then(Value::as_str)
            .is_some_and(|sha| sha != head) {
            complete = false;
            job.status = "head_mismatch";
            continue;
        }
        job.status = status(item.get("status").and_then(Value::as_str));
        job.conclusion = conclusion(item.get("conclusion").and_then(Value::as_str));
        if job.status != "completed" || job.conclusion != "success" { complete = false; }
        if matches!(job.conclusion, "failure" | "timed_out" | "cancelled") { failed = true; }
    }
    out.verdict = if out.run_status != "completed" { "pending" }
        else if out.conclusion != "success" || failed { "failed_exact_head" }
        else if complete { "passed_exact_head" }
        else { "incomplete_job_evidence" };
    out
}

fn blank(project_id: &str, git: &crate::git_status::GitWorktreeInspection) -> ExactHeadCiEvidence {
    ExactHeadCiEvidence {
        schema_version: 1,
        project_id: project_id.to_owned(),
        source: "public_github_actions_explicit_read",
        repository: None,
        local_head: git.local_head.clone(),
        worktree: git.worktree,
        checked_unix_seconds: None,
        verdict: "unverified",
        error_code: None,
        workflows: REQUIRED.iter().map(|(name, _, jobs)| empty_workflow(name, jobs)).collect(),
        runs_listing_partial: false,
        local_head_changed_during_check: false,
        installation_acceptance: "not_tested",
        task_completion_acceptance: "not_verified",
        credentials_used: false,
    }
}

fn get_json(agent: &ureq::Agent, uri: &str, limit: u64) -> Result<Value, &'static str> {
    let response = agent.get(uri)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "Lu-IDE-exact-head-CI/0.1")
        .call().map_err(|e| match e {
            ureq::Error::Status(401 | 403, _) => "GITHUB_ACCESS_LIMITED",
            ureq::Error::Status(404, _) => "GITHUB_NOT_FOUND_OR_PRIVATE",
            _ => "GITHUB_REQUEST_FAILED",
        })?;
    let mut bytes = Vec::new();
    response.into_reader().take(limit + 1).read_to_end(&mut bytes)
        .map_err(|_| "GITHUB_RESPONSE_UNAVAILABLE")?;
    if bytes.len() as u64 > limit { return Err("GITHUB_RESPONSE_TOO_LARGE"); }
    serde_json::from_slice(&bytes).map_err(|_| "GITHUB_RESPONSE_INVALID")
}

impl IdeWorkspaceRegistry {
    /// Human-triggered remote observation only. Never infers CI for uncommitted files.
    pub fn inspect_exact_head_ci(&self, project_id: &str) -> ExactHeadCiEvidence {
        let git = self.inspect_git_worktree(project_id);
        let mut out = blank(project_id, &git);
        let Some(head) = git.local_head.as_deref().filter(|s| valid_sha(s)) else {
            out.error_code = Some("LOCAL_HEAD_UNAVAILABLE");
            return out;
        };
        if git.state != "local_repository_inspected" {
            out.error_code = Some("LOCAL_REPOSITORY_UNVERIFIED");
            return out;
        }
        let Ok(root) = self.registered_git_root(project_id) else {
            out.error_code = Some("PROJECT_UNAVAILABLE");
            return out;
        };
        let Ok(repo) = Repository::open(&root) else {
            out.error_code = Some("GIT_REPOSITORY_UNAVAILABLE");
            return out;
        };
        if repo.workdir().and_then(|w| fs::canonicalize(w).ok()).as_deref()
            != Some(root.as_path()) {
            out.error_code = Some("GIT_REPOSITORY_OUTSIDE_PROJECT");
            return out;
        }
        let Some((owner, name)) = repo.find_remote("origin").ok()
            .and_then(|r| r.url().and_then(github_repo)) else {
            out.error_code = Some("GITHUB_ORIGIN_UNAVAILABLE");
            return out;
        };
        let repository = format!("{owner}/{name}");
        out.repository = Some(repository.clone());
        // Fixed HTTPS origin, validated owner/repository and 40-digit local SHA.
        let base = format!("https://api.github.com/repos/{repository}");
        let agent = ureq::AgentBuilder::new().redirects(0)
            .timeout_connect(Duration::from_secs(4))
            .timeout_read(Duration::from_secs(6))
            .timeout_write(Duration::from_secs(4)).build();
        let uri = format!("{base}/actions/runs?head_sha={head}&per_page={MAX_RUNS}");
        let runs_response = match get_json(&agent, &uri, MAX_RUN_RESPONSE_BYTES) {
            Ok(value) => value,
            Err(code) => { out.error_code = Some(code); return out; }
        };
        let Some(runs) = runs_response.get("workflow_runs").and_then(Value::as_array) else {
            out.error_code = Some("GITHUB_RESPONSE_INVALID");
            return out;
        };
        out.runs_listing_partial = runs.len() > MAX_RUNS
            || runs_response.get("total_count").and_then(Value::as_u64)
                .is_none_or(|total| total > MAX_RUNS as u64);
        for (i, (kind, path, expected_jobs)) in REQUIRED.iter().enumerate() {
            let selected = runs.iter()
                .filter(|r| matches_workflow(r, path, head))
                .max_by_key(|r| r.get("id").and_then(Value::as_u64).unwrap_or(0));
            let Some(run) = selected else { continue };
            let id = run.get("id").and_then(Value::as_u64).unwrap();
            let jobs_uri = format!("{base}/actions/runs/{id}/jobs?per_page={MAX_JOBS}");
            let jobs = get_json(&agent, &jobs_uri, MAX_JOBS_RESPONSE_BYTES);
            out.workflows[i] = assess_workflow(
                kind, expected_jobs, run, jobs.as_ref().ok(), head, &repository,
            );
            if let Err(code) = jobs {
                out.error_code = Some(code);
            }
        }

        let after = self.inspect_git_worktree(project_id);
        out.local_head_changed_during_check = after.local_head != out.local_head
            || after.state != "local_repository_inspected";
        out.checked_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
        out.verdict = if out.local_head_changed_during_check {
            "local_head_changed_during_check"
        } else if out.workflows.iter().any(|w| w.verdict == "failed_exact_head") {
            "exact_head_ci_failed"
        } else if out.workflows.iter().all(|w| w.verdict == "passed_exact_head")
            && !out.runs_listing_partial && out.error_code.is_none() {
            "exact_commit_ci_passed"
        } else if out.workflows.iter().any(|w| w.run_id.is_some()) {
            "partial_or_pending_evidence"
        } else {
            "no_matching_workflow_runs"
        };
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn require_exact_workflow_path_and_sha() {
        let run = serde_json::json!({"id": 10,"head_sha": "a".repeat(40),
            "path": ".github/workflows/ide-standalone.yml@refs/pull/4/merge"});
        let sha = "a".repeat(40);
        assert!(matches_workflow(&run, ".github/workflows/ide-standalone.yml", &sha));
        assert!(!matches_workflow(&run, ".github/workflows/other.yml", &sha));
        assert!(!matches_workflow(&run, ".github/workflows/ide-standalone.yml", &"b".repeat(40)));
    }
    #[test]
    fn exact_head_pass_requires_all_expected_jobs() {
        let sha = "a".repeat(40);
        let run = serde_json::json!({"id": 41,"status": "completed","conclusion": "success"});
        let jobs = serde_json::json!({"total_count": 3,"jobs": [
            {"name":"host-linux","status":"completed","conclusion":"success","head_sha":sha},
            {"name":"frontend","status":"completed","conclusion":"success","head_sha":sha},
            {"name":"desktop-windows","status":"completed","conclusion":"success","head_sha":sha}
        ]});
        assert_eq!(assess_workflow("standalone_ide", REQUIRED[0].2, &run, Some(&jobs), &sha,
            "owner/repo").verdict, "passed_exact_head");
        let partial = serde_json::json!({"total_count": 2,"jobs":[jobs["jobs"][0],jobs["jobs"][1]]});
        assert_ne!(assess_workflow("standalone_ide",REQUIRED[0].2,&run,Some(&partial),&sha,
            "owner/repo").verdict, "passed_exact_head");
    }
    #[test]
    fn never_promote_ambiguous_duplicate_or_other_head_jobs() {
        let sha = "a".repeat(40);
        let run = serde_json::json!({"id": 55,"status":"completed","conclusion":"success"});
        let dup = serde_json::json!({"total_count": 2,"jobs": [
            {"name":"continuity","status":"completed","conclusion":"success"},
            {"name":"continuity","status":"completed","conclusion":"success"}
        ]});
        let wrong = serde_json::json!({"total_count": 1,"jobs": [
            {"name":"continuity","status":"completed","conclusion":"success","head_sha":"b".repeat(40)}
        ]});
        assert_ne!(assess_workflow("continuity",REQUIRED[1].2,&run,Some(&dup),&sha,
            "owner/repo").verdict,"passed_exact_head");
        assert_ne!(assess_workflow("continuity",REQUIRED[1].2,&run,Some(&wrong),&sha,
            "owner/repo").verdict,"passed_exact_head");
        assert!(!valid_sha("not-a-commit"));
    }
}
