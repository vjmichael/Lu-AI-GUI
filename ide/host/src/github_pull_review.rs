//! 6E-01: explicitly requested, bounded, read-only GitHub pull-request review.
//!
//! No Git CLI, checkout, credentials, cookie, token, arbitrary URL, remote
//! publication, reviewer submission or permissions are reachable from here.
//! Remote metadata is untrusted observation and NEVER an execution grant.
use crate::{git_branch_operations::{exact_repo, safe_local_name},
    github_live::github_repo, IdeWorkspaceRegistry};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, fs, io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH}};

const MAX_RESPONSE: u64 = 160 * 1024;
const MAX_PRS: usize = 20;
const MAX_TITLE_BYTES: usize = 300;
const MAX_BRANCH_BYTES: usize = 160;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPullEvidence {
    pub number: u64,
    pub title: String,
    pub draft: bool,
    pub state: &'static str,
    pub head_branch: String,
    pub head_sha: String,
    pub base_branch: String,
    pub base_sha: String,
    pub url: String,
    pub local_vs_pr_head: &'static str,
    pub review_approval_verified: bool,
    pub checks_verified: bool,
    pub operation_authorized: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPullInspection {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub repository: Option<String>,
    pub current_branch: Option<String>,
    pub local_head: Option<String>,
    pub checked_unix_seconds: Option<u64>,
    pub pulls: Vec<GitHubPullEvidence>,
    pub listing_partial: bool,
    pub source_rechecked: bool,
    pub error_code: Option<&'static str>,
    pub read_only: bool,
    pub credentials_used: bool,
    pub remote_ref_modified: bool,
    pub review_submitted: bool,
    pub git_process_started: bool,
    pub task_completion_verified: bool,
}

fn blank(project_id: &str) -> GitHubPullInspection {
    GitHubPullInspection {
        schema_version: 1, project_id: project_id.to_owned(),
        state: "unverifiable", repository: None, current_branch: None,
        local_head: None, checked_unix_seconds: None,
        pulls: vec![], listing_partial: false, source_rechecked: false,
        error_code: None, read_only: true, credentials_used: false,
        remote_ref_modified: false, review_submitted: false,
        git_process_started: false, task_completion_verified: false,
    }
}
fn sha40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn safe_branch(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_BRANCH_BYTES
        && safe_local_name(s)
}
fn path_encode(s: &str) -> String {
    let mut out = String::new();
    for c in s.bytes() {
        if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.') {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

fn field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path { current = current.get(*key)?; }
    Some(current)
}
fn string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    field(value, path)?.as_str()
}
fn parse_pulls(payload: &Value, repository: &str, owner: &str,
    branch: &str, local_head: &str) -> Result<Vec<GitHubPullEvidence>, &'static str> {
    let items = payload.as_array().ok_or("G06E01_RESPONSE_NOT_ARRAY")?;
    if items.len() > MAX_PRS { return Err("G06E01_RESPONSE_TOO_MANY_PRS"); }
    let mut nums = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for pr in items {
        let number = pr.get("number").and_then(Value::as_u64)
            .filter(|n| *n > 0).ok_or("G06E01_INVALID_PR_NUMBER")?;
        if !nums.insert(number) { return Err("G06E01_DUPLICATE_PR_NUMBER"); }
        let title = string(pr, &["title"]).ok_or("G06E01_TITLE_MISSING")?;
        if title.is_empty() || title.len() > MAX_TITLE_BYTES ||
            title.chars().any(char::is_control) {
            return Err("G06E01_UNTRUSTED_PR_TITLE");
        }
        if string(pr, &["state"]) != Some("open") ||
            string(pr, &["head", "repo", "full_name"]) != Some(repository) ||
            string(pr, &["base", "repo", "full_name"]) != Some(repository) ||
            string(pr, &["head", "ref"]) != Some(branch) {
            return Err("G06E01_FOREIGN_OR_STALE_PR");
        }
        let draft = pr.get("draft").and_then(Value::as_bool)
            .ok_or("G06E01_DRAFT_STATE_MISSING")?;
        let head_sha = string(pr, &["head", "sha"])
            .filter(|s| sha40(s)).ok_or("G06E01_HEAD_SHA_INVALID")?;
        let base_branch = string(pr, &["base", "ref"])
            .filter(|s| safe_branch(s)).ok_or("G06E01_BASE_BRANCH_UNSAFE")?;
        let base_sha = string(pr, &["base", "sha"])
            .filter(|s| sha40(s)).ok_or("G06E01_BASE_SHA_INVALID")?;
        let url = format!("https://github.com/{repository}/pull/{number}");
        let _ = owner; // Derived from the validated origin; never an arbitrary API host.
        out.push(GitHubPullEvidence {
            number, title: title.to_owned(), draft, state: "open",
            head_branch: branch.to_owned(), head_sha: head_sha.to_owned(),
            base_branch: base_branch.to_owned(), base_sha: base_sha.to_owned(),
            url, local_vs_pr_head: if local_head.eq_ignore_ascii_case(head_sha) {
                "same_commit"
            } else {
                "different_commit_ancestry_unknown"
            },
            review_approval_verified: false, checks_verified: false,
            operation_authorized: false,
        });
    }
    Ok(out)
}

/// Read local branch and validated GitHub origin only; never follow arbitrary
/// URI/redirect or trust Git config's optional remote commands or hooks.
fn scope(registry: &IdeWorkspaceRegistry, id: &str)
    -> Result<(String, String, String, String), &'static str> {
    let repo = exact_repo(registry, id)?;
    let root = registry.registered_git_root(id)
        .map_err(|_| "G06E01_PROJECT_CLOSED")?;
    if repo.workdir().and_then(|w| fs::canonicalize(w).ok()).as_deref()
        != Some(root.as_path()) {
        return Err("G06E01_PROJECT_SCOPE_CHANGED");
    }
    let head = repo.head().map_err(|_| "G06E01_HEAD_MISSING")?;
    let branch = head.name().and_then(|n| n.strip_prefix("refs/heads/"))
        .filter(|s| safe_branch(s)).ok_or("G06E01_NOT_SAFE_LOCAL_BRANCH")?;
    let commit = head.target().ok_or("G06E01_INDIRECT_HEAD")?;
    repo.find_commit(commit).map_err(|_| "G06E01_HEAD_NOT_COMMIT")?;
    let remote = repo.find_remote("origin").map_err(|_| "G06E01_ORIGIN_MISSING")?;
    let (owner, repository) = remote.url()
        .and_then(github_repo).ok_or("G06E01_ORIGIN_NOT_GITHUB")?;
    Ok((owner, repository, branch.to_owned(), commit.to_string()))
}

impl IdeWorkspaceRegistry {
    pub fn inspect_github_pull_requests(&self, project_id: &str) -> GitHubPullInspection {
        let mut out = blank(project_id);
        let (owner, name, branch, head) = match scope(self, project_id) {
            Ok(values) => values,
            Err(error) => { out.error_code = Some(error); return out; }
        };
        let repository = format!("{owner}/{name}");
        out.repository = Some(repository.clone());
        out.current_branch = Some(branch.clone());
        out.local_head = Some(head.clone());

        // Explicitly bounded, unauthenticated HTTPS to the only permitted host.
        // Filter same-owner PRs server-side, then independently verify all fields.
        let url = format!(
            "https://api.github.com/repos/{repository}/pulls?state=open&head={}%3A{}&per_page={MAX_PRS}",
            path_encode(&owner), path_encode(&branch)
        );
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(4))
            .timeout_read(Duration::from_secs(6))
            .timeout_write(Duration::from_secs(4))
            .build();
        let response = match agent.get(&url)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "Lu-IDE-PR-review/0.1")
            .call() {
            Ok(r) => r,
            Err(ureq::Error::Status(401 | 403, _)) => {
                out.state = "access_limited";
                out.error_code = Some("G06E01_PUBLIC_API_LIMITED");
                return out;
            },
            Err(ureq::Error::Status(404, _)) => {
                out.state = "repository_private_or_missing";
                out.error_code = Some("G06E01_PUBLIC_REPO_UNAVAILABLE");
                return out;
            },
            Err(_) => {
                out.state = "network_unavailable";
                out.error_code = Some("G06E01_API_REQUEST_FAILED");
                return out;
            },
        };
        let next_page = response.header("Link")
            .is_some_and(|v| v.contains("rel=\"next\""));
        let mut bytes = Vec::new();
        if response.into_reader().take(MAX_RESPONSE + 1)
            .read_to_end(&mut bytes).is_err() ||
            bytes.len() as u64 > MAX_RESPONSE {
            out.error_code = Some("G06E01_RESPONSE_TOO_LARGE_OR_UNAVAILABLE");
            return out;
        }
        let value: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => { out.error_code = Some("G06E01_RESPONSE_NOT_JSON"); return out; }
        };
        let pulls = match parse_pulls(&value, &repository, &owner, &branch, &head) {
            Ok(p) => p,
            Err(error) => { out.error_code = Some(error); return out; }
        };
        let after = scope(self, project_id);
        if !matches!(after, Ok((ref a, ref b, ref c, ref d))
            if a == &owner && b == &name && c == &branch && d == &head) {
            out.state = "source_changed_during_inspection";
            out.error_code = Some("G06E01_ORIGIN_OR_HEAD_CHANGED");
            return out;
        }
        out.listing_partial = next_page || pulls.len() == MAX_PRS;
        out.pulls = pulls;
        out.source_rechecked = true;
        out.checked_unix_seconds = SystemTime::now().duration_since(UNIX_EPOCH)
            .ok().map(|d| d.as_secs());
        out.state = if out.listing_partial {
            "partial_open_pr_listing"
        } else if out.pulls.is_empty() {
            "no_open_same_origin_pr_observed"
        } else {
            "open_same_origin_prs_observed"
        };
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pr(title: &str, head: &str) -> Value {
        serde_json::json!({
            "number": 12,
            "title": title,
            "state": "open",
            "draft": true,
            "head": {"sha": head, "ref":"feat/one",
                "repo":{"full_name":"acme/widgets"}},
            "base": {"sha": "b".repeat(40), "ref":"main",
                "repo":{"full_name":"acme/widgets"}},
            "html_url": "https://attacker.invalid/unsafe"
        })
    }
    #[test]
    fn parse_typed_same_origin_open_pr_without_trusting_response_url() {
        let head = "a".repeat(40);
        let list = parse_pulls(&serde_json::json!([pr("Fix PR", &head)]),
            "acme/widgets", "acme", "feat/one", &head).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].url, "https://github.com/acme/widgets/pull/12");
        assert_eq!(list[0].local_vs_pr_head, "same_commit");
        assert!(!list[0].review_approval_verified && !list[0].operation_authorized);
    }
    #[test]
    fn never_accept_foreign_pr_wrong_commit_as_approval_or_malicious_title() {
        let head = "a".repeat(40);
        let mut forged = pr("text", &head);
        forged["head"]["repo"]["full_name"] = serde_json::json!("outsider/fork");
        assert!(parse_pulls(&serde_json::json!([forged]), "acme/widgets",
            "acme", "feat/one", &head).is_err());
        let mut wrong = pr("Text", &head);
        wrong["head"]["sha"] = serde_json::json!("c".repeat(40));
        let value = parse_pulls(&serde_json::json!([wrong]),
            "acme/widgets", "acme", "feat/one", &head).unwrap();
        assert_eq!(value[0].local_vs_pr_head, "different_commit_ancestry_unknown");
        assert!(!value[0].checks_verified && !value[0].review_approval_verified);
        assert!(parse_pulls(&serde_json::json!([pr("line\nbreak", &head)]),
            "acme/widgets", "acme", "feat/one", &head).is_err());
        assert!(parse_pulls(&serde_json::json!([pr(&"x".repeat(301), &head)]),
            "acme/widgets", "acme", "feat/one", &head).is_err());
    }
    #[test]
    fn conservative_branch_and_query_canonicalization() {
        assert!(safe_branch("feat/review-123"));
        assert!(!safe_branch("../escape"));
        assert!(!safe_branch("bad?query"));
        assert_eq!(path_encode("feat/one"), "feat%2Fone");
        assert_eq!(path_encode("name:branch"), "name%3Abranch");
        assert!(parse_pulls(&serde_json::json!([]),
            "acme/widgets", "acme", "feat/one", &"a".repeat(40)).unwrap().is_empty());
    }
}
