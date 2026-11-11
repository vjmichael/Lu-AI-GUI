//! G06C-02: immutable evidence for a *potential* local merge. NO execution.
//!
//! Only reads local Git objects/refs, the ordinary index file, and status.
//! Never calls merge_trees, merge_commits, checkout, update_ref, or libgit2
//! mutation APIs on the user's repository. A three-way path-overlap scan is
//! conservative: "no overlap found" is NOT a tested clean merge, because
//! rename, mode, merge drivers, content and directory transitions can matter.
use crate::{
    git_branch_operations::{
        exact_repo, local_commit, safe_local_name, BranchOperationKind, BranchOperationReview,
    },
    IdeWorkspaceRegistry,
};
use git2::{
    ObjectType, Oid, Repository, RepositoryState, Status, StatusOptions, TreeWalkMode,
    TreeWalkResult,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TREE_ENTRIES: usize = 20_000;
const MAX_PATH_BYTES: usize = 1024;
const MAX_DISPLAY_PATHS: usize = 80;
const MAX_CHANGED_STATUSES: usize = 1024;

#[derive(Debug, Clone, Eq, PartialEq)]
struct ObjectStamp {
    oid: Oid,
    file_mode: i32,
    kind: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SourceFence {
    current_branch: String,
    current_oid: Oid,
    selected_oid: Oid,
    index_sha256: String,
    dirty_count: usize,
    dirty_truncated: bool,
    index_conflicts: bool,
    index_lock: bool,
    state: RepositoryState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePathReview {
    pub relative_path: String,
    pub classification: &'static str,
    pub base_oid: Option<String>,
    pub current_oid: Option<String>,
    pub selected_oid: Option<String>,
    pub current_changed: bool,
    pub selected_changed: bool,
    pub requires_manual_resolution: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedMergePlan {
    pub schema_version: u32,
    pub project_id: String,
    pub current_branch: String,
    pub current_head: String,
    pub selected_local_branch: String,
    pub selected_tip: String,
    pub current_tree: String,
    pub selected_tree: String,
    pub merge_base_commit: Option<String>,
    pub merge_base_tree: Option<String>,
    pub merge_base_count: usize,
    pub current_only_commits: Option<usize>,
    pub selected_only_commits: Option<usize>,
    pub classification: &'static str,
    pub conflict_review: &'static str,
    pub possible_conflict_count: usize,
    pub changed_path_count: usize,
    pub visible_paths: Vec<MergePathReview>,
    pub omitted_path_count: usize,
    pub index_sha256: String,
    pub upstream_review_sha256: String,
    pub evidence_sha256: String,
    pub blockers: Vec<&'static str>,
    pub source_rechecked: bool,
    pub three_way_content_merge_performed: bool,
    pub rename_analysis_performed: bool,
    pub authorization_granted: bool,
    pub branch_ref_updated: bool,
    pub index_or_worktree_modified: bool,
    pub hooks_executed: bool,
    pub remote_queried: bool,
    pub process_started: bool,
    pub execution_route_available: bool,
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fingerprint(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"lu-ide-g06c02-reviewed-merge-readonly-v1\n");
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn ordinary_index_sha(gitdir: &Path) -> Result<String, &'static str> {
    let path = gitdir.join("index");
    let meta = fs::symlink_metadata(&path).map_err(|_| "G06C02_INDEX_UNAVAILABLE")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX_BYTES {
        return Err("G06C02_INDEX_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06C02_INDEX_READ_FAILED")?;
    if bytes.len() as u64 != meta.len() {
        return Err("G06C02_INDEX_CHANGED_DURING_READ");
    }
    Ok(hex(&bytes))
}

fn source_fence(repo: &Repository, selected_branch: &str) -> Result<SourceFence, &'static str> {
    if repo.head_detached().unwrap_or(true) {
        return Err("G06C02_HEAD_DETACHED");
    }
    let head_ref = repo.head().map_err(|_| "G06C02_HEAD_UNAVAILABLE")?;
    let raw = head_ref.name().ok_or("G06C02_HEAD_NOT_BRANCH")?;
    let current_branch = raw
        .strip_prefix("refs/heads/")
        .ok_or("G06C02_HEAD_NOT_BRANCH")?;
    if !safe_local_name(current_branch) {
        return Err("G06C02_CURRENT_REF_UNTRUSTED");
    }
    let current_oid = head_ref.target().ok_or("G06C02_HEAD_NOT_DIRECT")?;
    let selected_oid = local_commit(repo, selected_branch)?;
    let gitdir = repo.path();
    let index_sha256 = ordinary_index_sha(gitdir)?;
    let index = repo.index().map_err(|_| "G06C02_INDEX_PARSE_FAILED")?;
    let index_conflicts = index.has_conflicts();
    let index_lock = fs::symlink_metadata(gitdir.join("index.lock")).is_ok();
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .renames_head_to_index(false)
        .renames_index_to_workdir(false);
    let entries = repo
        .statuses(Some(&mut opts))
        .map_err(|_| "G06C02_STATUS_UNAVAILABLE")?;
    let mut dirty = 0usize;
    for entry in entries.iter() {
        if entry.status() != Status::CURRENT {
            dirty = dirty.saturating_add(1);
        }
    }
    Ok(SourceFence {
        current_branch: current_branch.to_owned(),
        current_oid,
        selected_oid,
        index_sha256,
        dirty_count: dirty.min(MAX_CHANGED_STATUSES),
        dirty_truncated: dirty > MAX_CHANGED_STATUSES,
        index_conflicts,
        index_lock,
        state: repo.state(),
    })
}

/// Walk only Git tree *metadata*. Never read blob contents, run merge drivers,
/// materialize a tree, write a Git object, or follow a worktree symlink.
fn tree_manifest(
    repo: &Repository,
    commit_id: Oid,
) -> Result<(String, BTreeMap<String, ObjectStamp>), &'static str> {
    let commit = repo
        .find_commit(commit_id)
        .map_err(|_| "G06C02_COMMIT_UNAVAILABLE")?;
    let tree = commit.tree().map_err(|_| "G06C02_TREE_UNAVAILABLE")?;
    let tree_oid = tree.id().to_string();
    let mut entries = BTreeMap::new();
    let mut failure = None;
    let walked = tree.walk(TreeWalkMode::PreOrder, |prefix, item| {
        if entries.len() >= MAX_TREE_ENTRIES {
            failure = Some("G06C02_TREE_CAP_EXCEEDED");
            return TreeWalkResult::Abort;
        }
        let Some(name) = item.name() else {
            failure = Some("G06C02_NON_UTF8_TREE_PATH");
            return TreeWalkResult::Abort;
        };
        let path = format!("{prefix}{name}");
        if path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
            failure = Some("G06C02_TREE_PATH_UNSAFE");
            return TreeWalkResult::Abort;
        }
        let kind = match item.kind() {
            Some(ObjectType::Tree) => "tree",
            Some(ObjectType::Blob) => "blob",
            Some(ObjectType::Commit) => "gitlink",
            _ => "unsupported",
        };
        let old = entries.insert(
            path,
            ObjectStamp {
                oid: item.id(),
                file_mode: item.filemode(),
                kind,
            },
        );
        if old.is_some() {
            failure = Some("G06C02_DUPLICATE_TREE_PATH");
            return TreeWalkResult::Abort;
        }
        TreeWalkResult::Ok
    });
    if let Some(reason) = failure {
        return Err(reason);
    }
    walked.map_err(|_| "G06C02_TREE_WALK_FAILED")?;
    Ok((tree_oid, entries))
}

fn inspect_path(
    relative_path: String,
    base: Option<&ObjectStamp>,
    current: Option<&ObjectStamp>,
    selected: Option<&ObjectStamp>,
) -> Option<MergePathReview> {
    // Directory-to-directory changes can be entirely independent. Inspect
    // actual leaf paths; retain directory/file swaps for manual review.
    let only_trees = [base, current, selected]
        .iter()
        .flatten()
        .all(|entry| entry.kind == "tree");
    if only_trees {
        return None;
    }
    let current_changed = current != base;
    let selected_changed = selected != base;
    if !current_changed && !selected_changed {
        return None;
    }
    let unsupported_kind = [base, current, selected]
        .iter()
        .flatten()
        .any(|entry| entry.kind != "blob" || !matches!(entry.file_mode, 0o100644 | 0o100755));
    let (classification, manual) = if unsupported_kind {
        ("object_type_or_mode_requires_manual_review", true)
    } else if current_changed && selected_changed && current != selected {
        // Even independently mergeable line edits in the same path are
        // *potential* conflicts here: no content merge has been run.
        ("possible_same_path_conflict", true)
    } else if current_changed && selected_changed {
        ("both_sides_identical", false)
    } else if current_changed {
        ("retain_current_change", false)
    } else {
        ("selected_only_change", false)
    };
    Some(MergePathReview {
        relative_path,
        classification,
        base_oid: base.map(|s| s.oid.to_string()),
        current_oid: current.map(|s| s.oid.to_string()),
        selected_oid: selected.map(|s| s.oid.to_string()),
        current_changed,
        selected_changed,
        requires_manual_resolution: manual,
    })
}

fn add_blocker(blockers: &mut Vec<&'static str>, code: &'static str) {
    if !blockers.contains(&code) {
        blockers.push(code);
    }
}

impl IdeWorkspaceRegistry {
    /// G06C02. Exact committed trees plus original index/source fence.
    /// This is not a preview of a computed merged tree or an execution grant.
    pub fn preview_reviewed_local_merge(
        &self,
        project_id: &str,
        selected_branch: &str,
    ) -> Result<ReviewedMergePlan, &'static str> {
        if !safe_local_name(selected_branch) {
            return Err("G06C02_INVALID_LOCAL_BRANCH");
        }
        let upstream: BranchOperationReview = self.review_local_branch_operation(
            project_id,
            BranchOperationKind::Merge,
            Some(selected_branch),
        )?;
        let repo = exact_repo(self, project_id)?;
        let initial = source_fence(&repo, selected_branch)?;
        if initial.current_oid.to_string() != upstream.current_head
            || initial.selected_oid.to_string() != upstream.selected_tip.as_deref().unwrap_or("")
            || initial.current_branch != upstream.current_branch
        {
            return Err("G06C02_BASE_PREVIEW_STALE");
        }
        let (head_tree, current_entries) = tree_manifest(&repo, initial.current_oid)?;
        let (selected_tree, selected_entries) = tree_manifest(&repo, initial.selected_oid)?;
        let merge_bases = repo
            .merge_bases(initial.current_oid, initial.selected_oid)
            .ok();
        let base_count = merge_bases.as_ref().map_or(0, |array| array.len());
        let base_id = merge_bases
            .as_ref()
            .filter(|bases| bases.len() == 1)
            .and_then(|bases| bases.iter().next().copied());
        let (base_tree, base_entries) = match base_id {
            Some(oid) => {
                let (tree, entries) = tree_manifest(&repo, oid)?;
                (Some(tree), entries)
            }
            None => (None, BTreeMap::new()),
        };
        let mut blockers = upstream.blockers.clone();
        if initial.dirty_count != 0 {
            add_blocker(&mut blockers, "worktree_or_index_dirty");
        }
        if initial.dirty_truncated {
            add_blocker(&mut blockers, "dirty_status_truncated");
        }
        if initial.index_conflicts {
            add_blocker(&mut blockers, "index_conflicts");
        }
        if initial.index_lock {
            add_blocker(&mut blockers, "git_index_lock_requires_manual_review");
        }
        if initial.state != RepositoryState::Clean {
            add_blocker(&mut blockers, "git_operation_in_progress");
        }
        if base_count > 1 {
            add_blocker(&mut blockers, "multiple_merge_bases_unresolved");
        }
        if base_count == 0 {
            add_blocker(&mut blockers, "unrelated_histories");
        }
        let classification = if initial.current_branch == selected_branch
            || initial.current_oid == initial.selected_oid
        {
            "same_commit_no_op"
        } else if base_count != 1 {
            "unverifiable_ancestry"
        } else if base_id == Some(initial.current_oid) {
            "fast_forward_candidate"
        } else if base_id == Some(initial.selected_oid) {
            "already_contains_selected_no_op"
        } else {
            "divergent_merge_candidate"
        };
        let mut all_paths = BTreeSet::new();
        all_paths.extend(base_entries.keys().cloned());
        all_paths.extend(current_entries.keys().cloned());
        all_paths.extend(selected_entries.keys().cloned());
        if all_paths.len() > MAX_TREE_ENTRIES * 3 {
            return Err("G06C02_UNION_PATH_CAP_EXCEEDED");
        }
        let mut visible = Vec::new();
        let mut changed = 0usize;
        let mut potential_conflicts = 0usize;
        let mut path_digest = Sha256::new();
        for path in all_paths {
            let p = inspect_path(
                path.clone(),
                base_entries.get(&path),
                current_entries.get(&path),
                selected_entries.get(&path),
            );
            if let Some(p) = p {
                changed += 1;
                potential_conflicts += usize::from(p.requires_manual_resolution);
                // The path ledger identity covers ALL entries, including
                // those omitted from bounded UI output.
                for part in [
                    p.relative_path.as_str(),
                    p.classification,
                    p.base_oid.as_deref().unwrap_or(""),
                    p.current_oid.as_deref().unwrap_or(""),
                    p.selected_oid.as_deref().unwrap_or(""),
                ] {
                    path_digest.update((part.len() as u64).to_le_bytes());
                    path_digest.update(part.as_bytes());
                }
                if visible.len() < MAX_DISPLAY_PATHS {
                    visible.push(p);
                }
            }
        }
        if potential_conflicts != 0 {
            add_blocker(&mut blockers, "potential_conflicts_require_manual_review");
        }
        if classification == "divergent_merge_candidate" {
            // Path-level comparisons are not a complete rename-aware merge
            // analysis, even when this scan observed zero overlapping paths.
            add_blocker(&mut blockers, "divergent_result_tree_not_verified");
        }
        if matches!(
            classification,
            "same_commit_no_op" | "already_contains_selected_no_op"
        ) {
            add_blocker(&mut blockers, "merge_unnecessary");
        }
        let final_fence = source_fence(&repo, selected_branch)?;
        if initial != final_fence {
            return Err("G06C02_SOURCE_CHANGED_DURING_PREVIEW");
        }
        let path_sha = format!("{:x}", path_digest.finalize());
        let base_string = base_id.map(|oid| oid.to_string());
        let selected_string = initial.selected_oid.to_string();
        let head_string = initial.current_oid.to_string();
        let evidence = fingerprint(&[
            project_id,
            &initial.current_branch,
            selected_branch,
            &head_string,
            &selected_string,
            base_string.as_deref().unwrap_or(""),
            &head_tree,
            &selected_tree,
            base_tree.as_deref().unwrap_or(""),
            &initial.index_sha256,
            &upstream.evidence_sha256,
            classification,
            &path_sha,
            &format!("{:?}", blockers),
        ]);
        let conflict_review = if base_count != 1 {
            "not_computable_no_unique_merge_base"
        } else if potential_conflicts != 0 {
            "potential_overlap_manual_resolution_required"
        } else {
            "no_same_path_overlap_observed_not_a_clean_merge_proof"
        };
        Ok(ReviewedMergePlan {
            schema_version: 1,
            project_id: project_id.to_owned(),
            current_branch: initial.current_branch,
            current_head: head_string,
            selected_local_branch: selected_branch.to_owned(),
            selected_tip: selected_string,
            current_tree: head_tree,
            selected_tree,
            merge_base_commit: base_string,
            merge_base_tree: base_tree,
            merge_base_count: base_count,
            current_only_commits: upstream.current_only_commits,
            selected_only_commits: upstream.selected_only_commits,
            classification,
            conflict_review,
            possible_conflict_count: potential_conflicts,
            changed_path_count: changed,
            omitted_path_count: changed.saturating_sub(visible.len()),
            visible_paths: visible,
            index_sha256: initial.index_sha256,
            upstream_review_sha256: upstream.evidence_sha256,
            evidence_sha256: evidence,
            blockers,
            source_rechecked: true,
            three_way_content_merge_performed: false,
            rename_analysis_performed: false,
            authorization_granted: false,
            branch_ref_updated: false,
            index_or_worktree_modified: false,
            hooks_executed: false,
            remote_queried: false,
            process_started: false,
            execution_route_available: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::{Repository, Signature};
    use std::{fs, path::PathBuf};

    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        id: String,
        registry: IdeWorkspaceRegistry,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    fn fixture() -> Fixture {
        let base = std::env::temp_dir().join(format!("ide-g06c02-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("one.txt"), b"base one\n").unwrap();
        fs::write(root.join("two.txt"), b"base two\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let sig = Signature::now("Merge test", "merge@localhost.invalid").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("one.txt")).unwrap();
        index.add_path(Path::new("two.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let root_commit = repo
            .commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();
        drop(tree);
        let original = repo.find_commit(root_commit).unwrap();
        repo.branch("feature", &original, false).unwrap();
        let initial_tree = original.tree().unwrap();
        let mut builder = repo.treebuilder(Some(&initial_tree)).unwrap();
        let blob = repo.blob(b"feature one\n").unwrap();
        builder.insert("one.txt", blob, 0o100644).unwrap();
        let feature_tree = repo.find_tree(builder.write().unwrap()).unwrap();
        repo.commit(
            Some("refs/heads/feature"),
            &sig,
            &sig,
            "feature one",
            &feature_tree,
            &[&original],
        )
        .unwrap();
        drop(feature_tree);
        drop(builder);
        drop(initial_tree);
        drop(original);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Picker(root.clone()))
            .unwrap()
            .unwrap()
            .project_id;
        Fixture {
            base,
            root,
            registry,
            id,
        }
    }
    fn add_main_change(x: &Fixture, relative: &str, bytes: &[u8]) {
        let repo = Repository::open(&x.root).unwrap();
        let old = repo.head().unwrap().peel_to_commit().unwrap();
        fs::write(x.root.join(relative), bytes).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(relative)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Merge test", "merge@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "main changed", &tree, &[&old])
            .unwrap();
    }
    fn original_bytes(x: &Fixture) -> (Vec<u8>, Vec<u8>, Vec<u8>, String, String) {
        let repo = Repository::open(&x.root).unwrap();
        let head = repo.head().unwrap().target().unwrap().to_string();
        let feature = local_commit(&repo, "feature").unwrap().to_string();
        (
            fs::read(x.root.join("one.txt")).unwrap(),
            fs::read(x.root.join("two.txt")).unwrap(),
            fs::read(x.root.join(".git/index")).unwrap(),
            head,
            feature,
        )
    }
    #[test]
    fn fast_forward_candidate_has_exact_tree_and_no_repo_mutation() {
        let x = fixture();
        let before = original_bytes(&x);
        let p = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        assert_eq!(p.classification, "fast_forward_candidate");
        assert_eq!(
            p.merge_base_commit.as_deref(),
            Some(p.current_head.as_str())
        );
        assert_eq!(p.merge_base_count, 1);
        assert_eq!(p.current_only_commits, Some(0));
        assert_eq!(p.selected_only_commits, Some(1));
        assert_eq!(p.possible_conflict_count, 0);
        assert_eq!(p.changed_path_count, 1);
        assert_eq!(p.visible_paths[0].classification, "selected_only_change");
        assert_eq!(p.visible_paths[0].relative_path, "one.txt");
        assert!(p.blockers.is_empty());
        assert_eq!(p.evidence_sha256.len(), 64);
        assert!(p.source_rechecked);
        assert!(!p.three_way_content_merge_performed);
        assert!(!p.authorization_granted && !p.branch_ref_updated);
        assert!(!p.index_or_worktree_modified && !p.process_started && !p.remote_queried);
        assert_eq!(before, original_bytes(&x));
        assert!(!x.root.join(".git/index.lock").exists());
        let again = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        assert_eq!(p.evidence_sha256, again.evidence_sha256);
    }
    #[test]
    fn divergent_disjoint_changes_not_misrepresented_as_verified_merge() {
        let x = fixture();
        add_main_change(&x, "two.txt", b"main two\n");
        let before = original_bytes(&x);
        let p = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        assert_eq!(p.classification, "divergent_merge_candidate");
        assert_eq!(p.current_only_commits, Some(1));
        assert_eq!(p.selected_only_commits, Some(1));
        assert_eq!(p.changed_path_count, 2);
        assert_eq!(p.possible_conflict_count, 0);
        assert!(p.blockers.contains(&"divergent_result_tree_not_verified"));
        assert_eq!(
            p.conflict_review,
            "no_same_path_overlap_observed_not_a_clean_merge_proof"
        );
        assert_eq!(before, original_bytes(&x));
    }
    #[test]
    fn same_path_divergence_reports_potential_conflict_without_writing() {
        let x = fixture();
        add_main_change(&x, "one.txt", b"main one\n");
        let before = original_bytes(&x);
        let p = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        assert_eq!(p.classification, "divergent_merge_candidate");
        assert_eq!(p.possible_conflict_count, 1);
        assert_eq!(p.visible_paths[0].relative_path, "one.txt");
        assert_eq!(
            p.visible_paths[0].classification,
            "possible_same_path_conflict"
        );
        assert!(p.visible_paths[0].requires_manual_resolution);
        assert!(p
            .blockers
            .contains(&"potential_conflicts_require_manual_review"));
        assert_eq!(before, original_bytes(&x));
    }
    #[test]
    fn dirty_or_foreign_lock_never_authorizes_or_removes_it() {
        let x = fixture();
        fs::write(x.root.join("one.txt"), b"unsaved\n").unwrap();
        fs::write(x.root.join("extra.txt"), b"untracked\n").unwrap();
        let foreign_lock = x.root.join(".git/index.lock");
        fs::write(&foreign_lock, b"OTHER_OWNER").unwrap();
        let p = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        assert!(p.blockers.contains(&"worktree_or_index_dirty"));
        assert!(p
            .blockers
            .contains(&"git_index_lock_requires_manual_review"));
        assert!(!p.authorization_granted && !p.execution_route_available);
        assert_eq!(fs::read(&foreign_lock).unwrap(), b"OTHER_OWNER");
        assert_eq!(fs::read(x.root.join("one.txt")).unwrap(), b"unsaved\n");
    }
    #[test]
    fn staleness_and_untrusted_selectors_are_not_capabilities() {
        let x = fixture();
        let old = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        for name in [
            "",
            "refs/heads/feature",
            "../feature",
            "origin/feature",
            "feature;rm",
            "HEAD",
        ] {
            assert!(x
                .registry
                .preview_reviewed_local_merge(&x.id, name)
                .is_err());
        }
        assert!(x
            .registry
            .preview_reviewed_local_merge("nonexistent", "feature")
            .is_err());
        add_main_change(&x, "two.txt", b"main two\n");
        let next = x
            .registry
            .preview_reviewed_local_merge(&x.id, "feature")
            .unwrap();
        assert_ne!(old.evidence_sha256, next.evidence_sha256);
        assert_ne!(old.current_head, next.current_head);
    }
}
