//! Explicit, read-only public GitHub branch evidence for an IDE-registered
//! Git worktree. This never fetches Git objects or writes repository state.
//! No model adapter, credentials, arbitrary remote URL or shell execution.
use std::{
    fs,
    io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use git2::Repository;
use serde::{Deserialize, Serialize};

use crate::IdeWorkspaceRegistry;

const MAX_RESPONSE_BYTES: u64 = 48 * 1024;
const MAX_BRANCH_BYTES: usize = 200;
const MAX_REPO_COMPONENT_BYTES: usize = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGitHubComparison {
    pub schema_version: u32,
    pub state: &'static str,
    pub source: &'static str,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub checked_unix_seconds: Option<u64>,
    pub local_head: Option<String>,
    pub stored_tracking_head: Option<String>,
    pub github_head: Option<String>,
    pub local_vs_github: &'static str,
    pub tracking_vs_github: &'static str,
    pub worktree: &'static str,
    pub dirty_file_count: Option<usize>,
    pub error_code: Option<&'static str>,
    pub response_authenticated: bool,
    pub github_data_verified_as_current: bool,
}

fn base(state: &'static str, error: Option<&'static str>) -> LiveGitHubComparison {
    LiveGitHubComparison {
        schema_version: 1,
        state,
        source: "github_public_rest_explicit_check",
        repository: None,
        branch: None,
        checked_unix_seconds: None,
        local_head: None,
        stored_tracking_head: None,
        github_head: None,
        local_vs_github: "unverifiable",
        tracking_vs_github: "unverifiable",
        worktree: "not_verified",
        dirty_file_count: None,
        error_code: error,
        response_authenticated: false,
        github_data_verified_as_current: false,
    }
}

fn safe_component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_REPO_COMPONENT_BYTES
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        && s != "."
        && s != ".."
}

fn safe_branch(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_BRANCH_BYTES
        && !s.starts_with('/')
        && !s.ends_with('/')
        && !s.contains("..")
        && !s.contains("//")
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'-' | b'_' | b'.'))
}

fn github_repo(remote: &str) -> Option<(String, String)> {
    let path = if let Some(p) = remote.strip_prefix("https://github.com/") {
        p
    } else if let Some(p) = remote.strip_prefix("git@github.com:") {
        p
    } else if let Some(p) = remote.strip_prefix("ssh://git@github.com/") {
        p
    } else {
        return None; // Never derive an API host from arbitrary Git remotes.
    };
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    let owner = parts.next()?;
    let repository = parts.next()?;
    if parts.next().is_some() || !safe_component(owner) || !safe_component(repository) {
        return None;
    }
    Some((owner.to_owned(), repository.to_owned()))
}

fn branch_api_path(branch: &str) -> String {
    // Encode slashes to keep a single GitHub branch path parameter; never
    // interpolate unvalidated model-supplied or project-file URLs.
    let mut result = String::new();
    for byte in branch.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            result.push(char::from(byte));
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

fn compare_hash(a: Option<&str>, b: Option<&str>) -> &'static str {
    match (a, b) {
        (Some(x), Some(y)) if x == y => "same_commit",
        (Some(_), Some(_)) => "different_commit_ancestry_unknown",
        _ => "unverifiable",
    }
}

#[derive(Deserialize)]
struct GitHubBranch {
    name: String,
    commit: GitHubCommit,
}
#[derive(Deserialize)]
struct GitHubCommit {
    sha: String,
}

impl IdeWorkspaceRegistry {
    /// An explicit host operation, intended for a human-triggered button.
    /// Public metadata only. No token, cookie, OAuth context or Git mutation.
    pub fn compare_live_github(&self, project_id: &str) -> LiveGitHubComparison {
        let local = self.inspect_git_worktree(project_id);
        let mut out = base("unverifiable", None);
        out.local_head = local.local_head.clone();
        out.stored_tracking_head = local.upstream_head.clone();
        out.worktree = local.worktree;
        if local.worktree == "clean" || local.worktree == "dirty" {
            out.dirty_file_count = Some(local.modified_or_staged_count + local.untracked_count);
        }

        let Ok(root) = self.registered_git_root(project_id) else {
            out.error_code = Some("PROJECT_UNAVAILABLE");
            return out;
        };
        let Ok(repo) = Repository::open(&root) else {
            out.error_code = Some("GIT_REPOSITORY_UNAVAILABLE");
            return out;
        };
        let Some(workdir) = repo.workdir() else {
            out.error_code = Some("BARE_REPOSITORY_UNSUPPORTED");
            return out;
        };
        if fs::canonicalize(workdir).ok().as_deref() != Some(root.as_path()) {
            out.error_code = Some("REPOSITORY_SCOPE_MISMATCH");
            return out;
        }
        if repo.head_detached().unwrap_or(true) {
            out.error_code = Some("DETACHED_HEAD_UNSUPPORTED");
            return out;
        }
        let Some(branch) = local.branch.as_deref().filter(|s| safe_branch(s)) else {
            out.error_code = Some("BRANCH_UNSUPPORTED");
            return out;
        };
        let remote = match repo.find_remote("origin") {
            Ok(remote) => remote,
            Err(_) => {
                out.error_code = Some("ORIGIN_UNAVAILABLE");
                return out;
            }
        };
        let Some((owner, repository)) = remote.url().and_then(github_repo) else {
            out.error_code = Some("NOT_A_SUPPORTED_GITHUB_REMOTE");
            return out;
        };
        out.repository = Some(format!("{owner}/{repository}"));
        out.branch = Some(branch.to_owned());

        let uri = format!(
            "https://api.github.com/repos/{owner}/{repository}/branches/{}",
            branch_api_path(branch)
        );
        // Explicit, fixed-origin TLS request, bounded size and time. Redirects
        // disabled so the server cannot redirect a privileged host elsewhere.
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(4))
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(4))
            .build();
        let response = match agent
            .get(&uri)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "Lu-IDE-continuity/0.1")
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) => {
                out.state = "branch_not_found_or_private";
                out.error_code = Some("GITHUB_BRANCH_UNAVAILABLE");
                return out;
            }
            Err(ureq::Error::Status(403, _)) => {
                out.state = "rate_limited_or_forbidden";
                out.error_code = Some("GITHUB_ACCESS_LIMITED");
                return out;
            }
            Err(_) => {
                out.state = "network_unavailable";
                out.error_code = Some("GITHUB_REQUEST_FAILED");
                return out;
            }
        };
        let mut bytes = Vec::new();
        if response
            .into_reader()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 > MAX_RESPONSE_BYTES
        {
            out.error_code = Some("GITHUB_RESPONSE_UNVERIFIABLE");
            return out;
        }
        let Ok(payload) = serde_json::from_slice::<GitHubBranch>(&bytes) else {
            out.error_code = Some("GITHUB_RESPONSE_INVALID");
            return out;
        };
        if payload.name != branch
            || payload.commit.sha.len() != 40
            || !payload.commit.sha.bytes().all(|c| c.is_ascii_hexdigit())
        {
            out.error_code = Some("GITHUB_RESPONSE_INVALID");
            return out;
        }
        out.github_head = Some(payload.commit.sha);
        out.local_vs_github = compare_hash(out.local_head.as_deref(), out.github_head.as_deref());
        out.tracking_vs_github = compare_hash(
            out.stored_tracking_head.as_deref(),
            out.github_head.as_deref(),
        );
        out.checked_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        out.github_data_verified_as_current = true;
        out.state = "public_branch_observed";
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_untrusted_git_hosts_and_credential_urls() {
        assert!(github_repo("https://github.com/owner/repo.git").is_some());
        assert!(github_repo("git@github.com:owner/repo.git").is_some());
        assert!(github_repo("ssh://git@github.com/owner/repo.git").is_some());
        for bad in [
            "https://github.com.evil.com/owner/repo",
            "https://token@github.com/owner/repo",
            "http://github.com/owner/repo",
            "https://github.com/owner/repo/extra",
            "https://github.com/owner/../repo",
            "file:///tmp/repo",
        ] {
            assert!(github_repo(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn branch_and_hash_parity_are_not_guesses_about_ancestry() {
        assert!(safe_branch("feature/safe-name"));
        assert!(!safe_branch("../bad"));
        assert!(!safe_branch("foo?bar"));
        assert_eq!(branch_api_path("feature/safe"), "feature%2Fsafe");
        assert_eq!(compare_hash(Some("a"), Some("a")), "same_commit");
        assert_eq!(
            compare_hash(Some("a"), Some("b")),
            "different_commit_ancestry_unknown"
        );
        assert_eq!(compare_hash(None, Some("b")), "unverifiable");
    }
}
