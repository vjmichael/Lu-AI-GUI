//! Group 6A-06/07 read-only refs, remote aliases and repository health.
//! Never expose remote URLs (which may contain credentials), config values,
//! absolute paths, object payloads or credentials. Local metadata only.
use crate::IdeWorkspaceRegistry;
use git2::Repository;
use serde::Serialize;
use std::fs;

const MAX_REFS: usize = 120;
const MAX_SCANNED_REFS: usize = 3000;
const MAX_REMOTES: usize = 32;
const MAX_NAME_BYTES: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefEvidence {
    pub name: String,
    pub kind: &'static str,
    pub direct_sha: Option<String>,
    pub resolved_sha: Option<String>,
    pub symbolic: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryHealth {
    pub repository_state: String,
    pub index_conflicts: Option<bool>,
    pub shallow: bool,
    pub detached_head: bool,
    pub unborn_head: bool,
    pub metadata_clean_only: bool,
    pub operation_ready: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefsAndHealthEvidence {
    pub schema_version: u32,
    pub project_id: String,
    pub head_sha: Option<String>,
    pub local_refs: Vec<GitRefEvidence>,
    pub refs_truncated: bool,
    pub remote_aliases: Vec<String>,
    pub remotes_truncated: bool,
    pub health: GitRepositoryHealth,
    pub remote_url_exposed: bool,
    pub network_queried: bool,
    pub git_process_started: bool,
    pub refs_or_files_modified: bool,
}

fn safe_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_NAME_BYTES && !s.chars().any(char::is_control)
}
fn selected_repo(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<Repository, &'static str> {
    let root = registry
        .registered_git_root(project_id)
        .map_err(|_| "G06A_PROJECT_NOT_OPEN")?;
    let repo = Repository::open(&root).map_err(|_| "G06A_REPOSITORY_UNAVAILABLE")?;
    let actual = fs::canonicalize(repo.workdir().ok_or("G06A_BARE_UNSUPPORTED")?)
        .map_err(|_| "G06A_WORKTREE_UNVERIFIABLE")?;
    if actual != root {
        return Err("G06A_REPOSITORY_OUTSIDE_PROJECT");
    }
    Ok(repo)
}

impl IdeWorkspaceRegistry {
    pub fn inspect_git_refs_health(
        &self,
        project_id: &str,
    ) -> Result<GitRefsAndHealthEvidence, &'static str> {
        let repo = selected_repo(self, project_id)?;
        let mut refs = Vec::new();
        let mut refs_truncated = false;
        let iterator = repo.references().map_err(|_| "G06A_REFS_UNAVAILABLE")?;
        for (scanned, item) in iterator.enumerate() {
            if refs.len() >= MAX_REFS || scanned >= MAX_SCANNED_REFS {
                refs_truncated = true;
                break;
            }
            let reference = item.map_err(|_| "G06A_REF_UNVERIFIABLE")?;
            let Some(raw) = reference.name() else {
                continue;
            };
            let (kind, name) = if let Some(rest) = raw.strip_prefix("refs/heads/") {
                ("local_branch", rest)
            } else if let Some(rest) = raw.strip_prefix("refs/remotes/") {
                ("stored_remote_tracking", rest)
            } else if let Some(rest) = raw.strip_prefix("refs/tags/") {
                ("tag", rest)
            } else {
                // Internal refs, HEAD and notes are intentionally excluded.
                continue;
            };
            if !safe_name(name) {
                continue;
            }
            let direct = reference.target();
            let resolved = reference.resolve().ok().and_then(|r| r.target());
            refs.push(GitRefEvidence {
                name: name.to_owned(),
                kind,
                direct_sha: direct.map(|o| o.to_string()),
                resolved_sha: resolved.map(|o| o.to_string()),
                symbolic: reference.symbolic_target().is_some(),
            });
        }
        refs.sort_by(|a, b| a.kind.cmp(b.kind).then(a.name.cmp(&b.name)));
        // Remote names are aliases only. Do NOT fetch remote URLs or
        // expose git config, including credential-bearing origin URLs.
        let names = repo
            .remotes()
            .map_err(|_| "G06A_REMOTE_ALIASES_UNAVAILABLE")?;
        let mut aliases = Vec::new();
        let mut remotes_truncated = false;
        for name in names.iter().flatten() {
            if aliases.len() >= MAX_REMOTES {
                remotes_truncated = true;
                break;
            }
            if safe_name(name) {
                aliases.push(name.to_owned());
            }
        }
        aliases.sort();
        let state = format!("{:?}", repo.state());
        let conflicts = repo.index().ok().map(|index| index.has_conflicts());
        let unborn = repo.head().is_err() && repo.is_empty().unwrap_or(false);
        let detached = repo.head_detached().unwrap_or(false);
        let healthy = matches!(repo.state(), git2::RepositoryState::Clean)
            && conflicts == Some(false)
            && !unborn;
        Ok(GitRefsAndHealthEvidence {
            schema_version: 1,
            project_id: project_id.to_owned(),
            head_sha: repo
                .head()
                .ok()
                .and_then(|head| head.target())
                .map(|id| id.to_string()),
            local_refs: refs,
            refs_truncated,
            remote_aliases: aliases,
            remotes_truncated,
            health: GitRepositoryHealth {
                repository_state: state,
                index_conflicts: conflicts,
                shallow: repo.is_shallow(),
                detached_head: detached,
                unborn_head: unborn,
                metadata_clean_only: healthy,
                operation_ready: false,
            },
            remote_url_exposed: false,
            network_queried: false,
            git_process_started: false,
            refs_or_files_modified: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::Signature;
    use std::path::PathBuf;
    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn local_refs_and_remote_aliases_never_return_credential_url() {
        let path =
            std::env::temp_dir().join(format!("ide-g06a-refs-{:032x}", rand::random::<u128>()));
        let _fixture = Fixture(path.clone());
        let root = path.join("root");
        fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        fs::write(root.join("source.rs"), b"hello\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("source.rs")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
        repo.reference("refs/tags/v1", oid, false, "test tag")
            .unwrap();
        repo.remote(
            "origin",
            "https://secret-user:secret-token@example.invalid/org/repo.git",
        )
        .unwrap();
        drop(tree);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(path.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Picker(root))
            .unwrap()
            .unwrap()
            .project_id;
        let result = registry.inspect_git_refs_health(&id).unwrap();
        assert!(result
            .local_refs
            .iter()
            .any(|r| r.name == "v1" && r.kind == "tag"));
        assert_eq!(result.remote_aliases, vec!["origin"]);
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(!encoded.contains("secret-token"));
        assert!(!encoded.contains("example.invalid"));
        assert!(
            !result.remote_url_exposed && !result.network_queried && !result.git_process_started
        );
        assert_eq!(result.health.index_conflicts, Some(false));
        assert!(result.health.metadata_clean_only);
        assert!(!result.health.operation_ready);
    }
}
