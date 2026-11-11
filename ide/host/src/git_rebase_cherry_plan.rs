//! IDE 6C-04A: conservative read-only replay review for rebase/cherry-pick.
 //!
 //! No model/WebView authority to rewrite history is created here. Native
 //! commit enumeration, original parent/tree identity, and path evidence are
 //! observations only. The resulting trees are NOT computed or verified.
use crate::{
    git_branch_operations::{exact_repo, local_commit, safe_local_name, BranchOperationKind},
    IdeWorkspaceRegistry,
};
use git2::{Delta, Oid, Repository};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

const MAX_REPLAY_COMMITS: usize = 16;
const MAX_PATHS_PER_COMMIT: usize = 256;
const MAX_VISIBLE_PATHS: usize = 40;
const MAX_PATH_BYTES: usize = 240;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplayReviewKind {
    RebaseCurrentOntoLocal,
    CherryPickLocalTip,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayCommitEvidence {
    pub ordinal: usize,
    pub original_commit: String,
    pub original_parent: String,
    pub original_tree: String,
    pub parent_tree: String,
    pub path_count: usize,
    pub visible_paths: Vec<String>,
    pub omitted_path_count: usize,
    pub unsupported_delta_count: usize,
    pub path_ledger_sha256: String,
    pub commit_message_preview: String,
    pub original_object_preserved: bool,
    pub content_merge_performed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedReplayPlan {
    pub schema_version: u32,
    pub project_id: String,
    pub operation: ReplayReviewKind,
    pub current_branch: String,
    pub current_head: String,
    pub current_tree: String,
    pub selected_local_branch: String,
    pub selected_tip: String,
    pub selected_tree: String,
    pub merge_base: String,
    pub merge_base_tree: String,
    pub original_index_sha256: String,
    pub upstream_review_sha256: String,
    pub merge_review_sha256: String,
    pub evidence_sha256: String,
    pub replay_commits: Vec<ReplayCommitEvidence>,
    pub changed_path_count: usize,
    pub possible_overlap_count: usize,
    pub visible_overlapping_paths: Vec<String>,
    pub blockers: Vec<&'static str>,
    pub history_rewrite: bool,
    pub original_commits_retained: bool,
    pub branch_ref_updated: bool,
    pub worktree_or_index_modified: bool,
    pub git_hook_executed: bool,
    pub network_queried: bool,
    pub process_started: bool,
    pub execution_route_available: bool,
    pub one_use_os_approval_granted: bool,
    pub resulting_tree_verified: bool,
    pub conflict_resolution_performed: bool,
    pub rename_analysis_performed: bool,
    pub source_rechecked: bool,
}

fn digest(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    h.update(b"lu-ide-g06c04-original-replay-review-v1\n");
    for item in parts {
        h.update((item.len() as u64).to_le_bytes());
        h.update(item.as_bytes());
    }
    format!("{:x}", h.finalize())
}

fn add(blockers: &mut Vec<&'static str>, code: &'static str) {
    if !blockers.contains(&code) { blockers.push(code); }
}

fn valid_path(p: &Path) -> Result<String, &'static str> {
    let text = p.to_str().ok_or("G06C04_NON_UTF8_PATH")?;
    if text.is_empty() || text.len() > MAX_PATH_BYTES
        || text.chars().any(char::is_control)
        || text.starts_with('/') || text.contains('\\')
        || text.split('/').any(|s| s.is_empty() || s == "." || s == "..")
    {
        return Err("G06C04_UNSAFE_DIFF_PATH");
    }
    Ok(text.to_owned())
}

fn single_parent_commit(
    repo: &Repository, id: Oid, ordinal: usize,
) -> Result<(ReplayCommitEvidence, BTreeSet<String>), &'static str> {
    let commit = repo.find_commit(id).map_err(|_| "G06C04_COMMIT_NOT_FOUND")?;
    if commit.parent_count() != 1 {
        return Err("G06C04_ROOT_OR_MERGE_COMMIT_REVIEW_DENIED");
    }
    let parent_id = commit.parent_id(0).map_err(|_| "G06C04_PARENT_UNAVAILABLE")?;
    let parent = repo.find_commit(parent_id).map_err(|_| "G06C04_PARENT_MISSING")?;
    let before = parent.tree().map_err(|_| "G06C04_PARENT_TREE_UNAVAILABLE")?;
    let after = commit.tree().map_err(|_| "G06C04_COMMIT_TREE_UNAVAILABLE")?;
    let diff = repo.diff_tree_to_tree(Some(&before), Some(&after), None)
        .map_err(|_| "G06C04_COMMIT_DIFF_UNAVAILABLE")?;
    if diff.deltas().len() > MAX_PATHS_PER_COMMIT {
        return Err("G06C04_COMMIT_PATH_LIMIT");
    }
    let mut paths = BTreeSet::new();
    let mut unsupported = 0usize;
    let mut path_hash = Sha256::new();
    path_hash.update(b"lu-ide-g06c04-commit-path-ledger-v1\n");
    for delta in diff.deltas() {
        let status = delta.status();
        // Even default libgit2 diff does not prove rename/copy resolution.
        if matches!(status, Delta::Renamed | Delta::Copied | Delta::Typechange
                    | Delta::Unreadable | Delta::Conflicted) {
            unsupported += 1;
        }
        let before_path = delta.old_file().path().map(valid_path).transpose()?;
        let after_path = delta.new_file().path().map(valid_path).transpose()?;
        if before_path.is_none() && after_path.is_none() {
            return Err("G06C04_DIFF_PATH_MISSING");
        }
        for value in [
            format!("{:?}", status),
            before_path.clone().unwrap_or_default(),
            after_path.clone().unwrap_or_default(),
            delta.old_file().id().to_string(),
            delta.new_file().id().to_string(),
        ] {
            path_hash.update((value.len() as u64).to_le_bytes());
            path_hash.update(value.as_bytes());
        }
        if let Some(before) = before_path { paths.insert(before); }
        if let Some(after) = after_path { paths.insert(after); }
        if paths.len() > MAX_PATHS_PER_COMMIT {
            return Err("G06C04_COMMIT_PATH_LIMIT");
        }
    }
    let message = commit.summary().unwrap_or("");
    let message = if message.len() > 120 || message.chars().any(char::is_control) {
        "[message hidden: unsafe or oversized]".to_owned()
    } else {
        message.to_owned()
    };
    let visible = paths.iter().take(MAX_VISIBLE_PATHS).cloned().collect::<Vec<_>>();
    let path_count = paths.len();
    Ok((ReplayCommitEvidence {
        ordinal,
        original_commit: id.to_string(),
        original_parent: parent_id.to_string(),
        original_tree: after.id().to_string(),
        parent_tree: before.id().to_string(),
        path_count,
        omitted_path_count: path_count.saturating_sub(visible.len()),
        visible_paths: visible,
        unsupported_delta_count: unsupported,
        path_ledger_sha256: format!("{:x}", path_hash.finalize()),
        commit_message_preview: message,
        original_object_preserved: true,
        content_merge_performed: false,
    }, paths))
}

fn first_parent_range(
    repo: &Repository, head: Oid, base: Oid,
) -> Result<Vec<Oid>, &'static str> {
    let mut out = Vec::new();
    let mut at = head;
    while at != base {
        if out.len() >= MAX_REPLAY_COMMITS {
            return Err("G06C04_REBASE_COMMIT_LIMIT");
        }
        let commit = repo.find_commit(at).map_err(|_| "G06C04_HISTORY_UNAVAILABLE")?;
        if commit.parent_count() != 1 {
            return Err("G06C04_REBASE_MERGE_OR_ROOT_IN_RANGE");
        }
        out.push(at);
        at = commit.parent_id(0).map_err(|_| "G06C04_HISTORY_PARENT_MISSING")?;
    }
    out.reverse();
    Ok(out)
}

fn changed_since_base(
    repo: &Repository, base: Oid, tip: Oid,
) -> Result<BTreeSet<String>, &'static str> {
    let before = repo.find_commit(base).map_err(|_| "G06C04_BASE_COMMIT_MISSING")?;
    let after = repo.find_commit(tip).map_err(|_| "G06C04_TIP_COMMIT_MISSING")?;
    let base_tree = before.tree().map_err(|_| "G06C04_BASE_TREE_MISSING")?;
    let tip_tree = after.tree().map_err(|_| "G06C04_TIP_TREE_MISSING")?;
    let diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tip_tree), None)
        .map_err(|_| "G06C04_COMPARE_DIFF_FAILED")?;
    if diff.deltas().len() > MAX_PATHS_PER_COMMIT * MAX_REPLAY_COMMITS {
        return Err("G06C04_TARGET_DIFF_TOO_LARGE");
    }
    let mut paths = BTreeSet::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.old_file().path() {
            paths.insert(valid_path(path)?);
        }
        if let Some(path) = delta.new_file().path() {
            paths.insert(valid_path(path)?);
        }
        if paths.len() > MAX_PATHS_PER_COMMIT * MAX_REPLAY_COMMITS {
            return Err("G06C04_TARGET_DIFF_TOO_LARGE");
        }
    }
    Ok(paths)
}

impl IdeWorkspaceRegistry {
    /// Strict read-only review of original commit objects, never a replay.
    /// Cherry-pick reviews ONLY the selected local branch's tip commit.
    /// Rebase reviews ONLY the current branch's first-parent commits since
    /// the unique base, replayed atop the selected local branch's exact tip.
    pub fn review_local_rebase_cherry_pick(
        &self, project_id: &str, kind: ReplayReviewKind, selected_branch: &str,
    ) -> Result<ReviewedReplayPlan, &'static str> {
        if !safe_local_name(selected_branch) {
            return Err("G06C04_BAD_LOCAL_BRANCH");
        }
        let upstream_kind = match kind {
            ReplayReviewKind::RebaseCurrentOntoLocal => BranchOperationKind::Rebase,
            ReplayReviewKind::CherryPickLocalTip => BranchOperationKind::CherryPick,
        };
        let upstream = self.review_local_branch_operation(
            project_id, upstream_kind, Some(selected_branch))?;
        let merge = self.preview_reviewed_local_merge(project_id, selected_branch)?;
        if upstream.current_head != merge.current_head
            || upstream.selected_tip.as_deref() != Some(merge.selected_tip.as_str())
            || upstream.current_branch != merge.current_branch {
            return Err("G06C04_SOURCE_CHANGED_BEFORE_ENUMERATION");
        }
        let repo = exact_repo(self, project_id)?;
        let current_id = Oid::from_str(&merge.current_head).map_err(|_| "G06C04_HEAD_INVALID")?;
        let selected_id = local_commit(&repo, selected_branch)?;
        if selected_id.to_string() != merge.selected_tip {
            return Err("G06C04_SELECTED_REF_CHANGED");
        }
        let bases = repo.merge_bases(current_id, selected_id)
            .map_err(|_| "G06C04_MERGE_BASE_UNVERIFIABLE")?;
        if bases.len() != 1 {
            return Err("G06C04_NOT_UNIQUE_MERGE_BASE");
        }
        let base = bases.iter().next().copied().ok_or("G06C04_MERGE_BASE_MISSING")?;
        if merge.merge_base_commit.as_deref() != Some(base.to_string().as_str()) {
            return Err("G06C04_BASE_CHANGED");
        }
        let commits = match kind {
            ReplayReviewKind::RebaseCurrentOntoLocal => first_parent_range(&repo, current_id, base)?,
            ReplayReviewKind::CherryPickLocalTip => vec![selected_id],
        };
        let mut blockers = upstream.blockers.clone();
        if merge.blockers.contains(&"index_conflicts") {
            add(&mut blockers, "index_conflicts");
        }
        if merge.blockers.contains(&"git_index_lock_requires_manual_review") {
            add(&mut blockers, "foreign_git_index_lock");
        }
        if !merge.blockers.is_empty() {
            // Divergent result tree is expected to be unverified here; never
            // accidentally turn that into a ready-to-execute declaration.
            add(&mut blockers, "merge_result_unverified");
        }
        if current_id == selected_id {
            add(&mut blockers, "same_commit");
        }
        if kind == ReplayReviewKind::RebaseCurrentOntoLocal && commits.is_empty() {
            add(&mut blockers, "nothing_to_rebase");
        }
        if kind == ReplayReviewKind::CherryPickLocalTip {
            if current_id == selected_id
                || repo.graph_descendant_of(current_id, selected_id).unwrap_or(false) {
                add(&mut blockers, "selected_commit_already_in_current_history");
            }
        }
        let reference_target = match kind {
            ReplayReviewKind::RebaseCurrentOntoLocal => selected_id,
            ReplayReviewKind::CherryPickLocalTip => current_id,
        };
        let already_changed = changed_since_base(&repo, base, reference_target)?;
        let mut replay_paths = BTreeSet::new();
        let mut records = Vec::new();
        for (ordinal, id) in commits.iter().enumerate() {
            let (entry, paths) = single_parent_commit(&repo, *id, ordinal + 1)?;
            if entry.unsupported_delta_count > 0 {
                add(&mut blockers, "unsupported_original_delta");
            }
            if entry.path_count == 0 {
                add(&mut blockers, "empty_original_commit");
            }
            replay_paths.extend(paths);
            records.push(entry);
        }
        let overlaps = replay_paths.intersection(&already_changed).cloned().collect::<Vec<_>>();
        if !overlaps.is_empty() {
            add(&mut blockers, "possible_path_overlap_requires_resolution");
        }
        // A rebase may reapply overlapping edits even if path sets are
        // disjoint. Commit messages, merges, renames, and drivers can matter.
        add(&mut blockers, "replay_resulting_trees_not_verified");
        add(&mut blockers, "one_use_native_os_authorization_not_issued");
        let fresh_upstream = self.review_local_branch_operation(
            project_id, upstream_kind, Some(selected_branch))?;
        let fresh_merge = self.preview_reviewed_local_merge(project_id, selected_branch)?;
        if fresh_upstream.evidence_sha256 != upstream.evidence_sha256
            || fresh_merge.evidence_sha256 != merge.evidence_sha256
            || local_commit(&repo, selected_branch)? != selected_id {
            return Err("G06C04_SOURCE_CHANGED_DURING_REVIEW");
        }
        let mut ledger = Vec::new();
        for record in &records {
            ledger.push(format!(
                "{}:{}:{}:{}:{}:{}",
                record.ordinal, record.original_commit, record.original_parent,
                record.original_tree, record.parent_tree, record.path_ledger_sha256
            ));
        }
        let ledger_sha = digest(&ledger.iter().map(String::as_str).collect::<Vec<_>>());
        let overlap_sha = digest(&overlaps.iter().map(String::as_str).collect::<Vec<_>>());
        let base_string = base.to_string();
        let kind_name = match kind {
            ReplayReviewKind::RebaseCurrentOntoLocal => "rebase_current_onto_local",
            ReplayReviewKind::CherryPickLocalTip => "cherry_pick_local_tip",
        };
        let evidence_sha256 = digest(&[
            project_id, kind_name, &merge.current_branch, selected_branch,
            &merge.current_head, &merge.current_tree, &merge.selected_tip,
            &merge.selected_tree, &base_string, &merge.merge_base_tree.clone().unwrap_or_default(),
            &merge.index_sha256, &upstream.evidence_sha256, &merge.evidence_sha256,
            &ledger_sha, &overlap_sha, &format!("{:?}", blockers)
        ]);
        Ok(ReviewedReplayPlan {
            schema_version: 1,
            project_id: project_id.to_owned(),
            operation: kind,
            current_branch: merge.current_branch,
            current_head: merge.current_head,
            current_tree: merge.current_tree,
            selected_local_branch: selected_branch.to_owned(),
            selected_tip: merge.selected_tip,
            selected_tree: merge.selected_tree,
            merge_base: base_string,
            merge_base_tree: merge.merge_base_tree.ok_or("G06C04_BASE_TREE_UNAVAILABLE")?,
            original_index_sha256: merge.index_sha256,
            upstream_review_sha256: upstream.evidence_sha256,
            merge_review_sha256: merge.evidence_sha256,
            evidence_sha256,
            replay_commits: records,
            changed_path_count: replay_paths.len(),
            possible_overlap_count: overlaps.len(),
            visible_overlapping_paths: overlaps.iter().take(MAX_VISIBLE_PATHS).cloned().collect(),
            blockers,
            history_rewrite: kind == ReplayReviewKind::RebaseCurrentOntoLocal,
            original_commits_retained: true,
            branch_ref_updated: false,
            worktree_or_index_modified: false,
            git_hook_executed: false,
            network_queried: false,
            process_started: false,
            execution_route_available: false,
            one_use_os_approval_granted: false,
            resulting_tree_verified: false,
            conflict_resolution_performed: false,
            rename_analysis_performed: false,
            source_rechecked: true,
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
        registry: IdeWorkspaceRegistry,
        project_id: String,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    fn fixture() -> Fixture {
        let base = std::env::temp_dir().join(
            format!("ide-g06c04-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("original.keep"), b"stable\n").unwrap();
        fs::write(root.join("main.txt"), b"main baseline\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        repo.config().unwrap().set_bool("core.autocrlf", false).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("original.keep")).unwrap();
        index.add_path(Path::new("main.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let base_oid = repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[]).unwrap();
        drop(tree);
        let original = repo.find_commit(base_oid).unwrap();
        repo.branch("feature", &original, false).unwrap();
        // Move the selected local branch two commits beyond the root.
        let mut previous = base_oid;
        for (name, content) in [
            ("selected.txt", b"feature selected\n".as_slice()),
            ("selected2.txt", b"feature second\n".as_slice()),
        ] {
            let before = repo.find_commit(previous).unwrap();
            let old = before.tree().unwrap();
            let mut builder = repo.treebuilder(Some(&old)).unwrap();
            let blob = repo.blob(content).unwrap();
            builder.insert(name, blob, 0o100644).unwrap();
            let new_tree = repo.find_tree(builder.write().unwrap()).unwrap();
            previous = repo.commit(Some("refs/heads/feature"), &sig, &sig, name,
                &new_tree, &[&before]).unwrap();
        }
        // Move HEAD one commit independently; synchronize worktree/index.
        fs::write(root.join("main.txt"), b"main changed\n").unwrap();
        index.add_path(Path::new("main.txt")).unwrap();
        index.write().unwrap();
        let new_tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "main changed", &new_tree, &[&original]).unwrap();
        drop(new_tree);
        drop(index);
        drop(original);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let project_id = registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture { base, root, registry, project_id }
    }
    fn snapshot(x: &Fixture) -> (String, String, Vec<u8>, Vec<u8>) {
        let repo = Repository::open(&x.root).unwrap();
        let head_sha = repo.head().unwrap().target().unwrap().to_string();
        (
            head_sha,
            local_commit(&repo, "feature").unwrap().to_string(),
            fs::read(x.root.join(".git/index")).unwrap(),
            fs::read(x.root.join("original.keep")).unwrap(),
        )
    }
    #[test]
    fn rebase_lists_exact_current_only_original_commit_in_order_without_writing() {
        let x = fixture();
        let before = snapshot(&x);
        let plan = x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::RebaseCurrentOntoLocal, "feature").unwrap();
        assert_eq!(plan.replay_commits.len(), 1);
        assert_eq!(plan.replay_commits[0].original_commit, plan.current_head);
        assert_eq!(plan.replay_commits[0].visible_paths, vec!["main.txt"]);
        assert_eq!(plan.changed_path_count, 1);
        assert_eq!(plan.possible_overlap_count, 0);
        assert!(plan.history_rewrite);
        assert!(plan.blockers.contains(&"replay_resulting_trees_not_verified"));
        assert!(!plan.execution_route_available && !plan.branch_ref_updated);
        assert!(!plan.resulting_tree_verified && !plan.one_use_os_approval_granted);
        assert!(plan.source_rechecked);
        assert_eq!(plan.evidence_sha256.len(), 64);
        assert_eq!(before, snapshot(&x));
        assert_eq!(plan.evidence_sha256, x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::RebaseCurrentOntoLocal, "feature")
            .unwrap().evidence_sha256);
    }
    #[test]
    fn cherry_pick_only_selected_branch_tip_with_parent_and_tree_provenance() {
        let x = fixture();
        let before = snapshot(&x);
        let plan = x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::CherryPickLocalTip, "feature").unwrap();
        assert_eq!(plan.replay_commits.len(), 1);
        assert_eq!(plan.replay_commits[0].original_commit, plan.selected_tip);
        assert_eq!(plan.replay_commits[0].visible_paths, vec!["selected2.txt"]);
        assert_eq!(plan.replay_commits[0].ordinal, 1);
        assert!(!plan.history_rewrite);
        assert!(plan.original_commits_retained);
        assert!(!plan.git_hook_executed && !plan.network_queried && !plan.process_started);
        assert!(!plan.conflict_resolution_performed && !plan.rename_analysis_performed);
        assert_eq!(before, snapshot(&x));
    }
    #[test]
    fn stale_selected_ref_dirty_index_foreign_lock_and_unsafe_name_refuse_authority() {
        let x = fixture();
        let old = x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::CherryPickLocalTip, "feature").unwrap();
        for bad in ["", "../feature", "refs/heads/feature", "origin/feature", "feature;rm", "HEAD"] {
            assert!(x.registry.review_local_rebase_cherry_pick(
                &x.project_id, ReplayReviewKind::CherryPickLocalTip, bad).is_err());
        }
        assert!(x.registry.review_local_rebase_cherry_pick(
            "not-registered", ReplayReviewKind::RebaseCurrentOntoLocal, "feature").is_err());
        let repo = Repository::open(&x.root).unwrap();
        let old_feature = repo.find_commit(Oid::from_str(&old.selected_tip).unwrap()).unwrap();
        let tree = old_feature.tree().unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("refs/heads/feature"), &sig, &sig, "tip moved", &tree,
            &[&old_feature]).unwrap();
        let fresh = x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::CherryPickLocalTip, "feature").unwrap();
        assert_ne!(old.selected_tip, fresh.selected_tip);
        assert_ne!(old.evidence_sha256, fresh.evidence_sha256);
        fs::write(x.root.join("main.txt"), b"dirty unsaved\n").unwrap();
        let dirty = x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::CherryPickLocalTip, "feature").unwrap();
        assert!(dirty.blockers.contains(&"worktree_or_index_dirty"));
        fs::write(x.root.join(".git/index.lock"), b"FOREIGN_OWNER").unwrap();
        let locked = x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::CherryPickLocalTip, "feature").unwrap();
        assert!(locked.blockers.contains(&"git_index_lock_requires_manual_review"));
        assert_eq!(fs::read(x.root.join(".git/index.lock")).unwrap(), b"FOREIGN_OWNER");
        assert!(!locked.execution_route_available);
    }
    #[test]
    fn merge_commit_in_original_replay_range_is_refused_not_flattened() {
        let x = fixture();
        let repo = Repository::open(&x.root).unwrap();
        let main = repo.head().unwrap().peel_to_commit().unwrap();
        let feature = repo.find_commit(local_commit(&repo, "feature").unwrap()).unwrap();
        let tree = main.tree().unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "test merge", &tree,
            &[&main, &feature]).unwrap();
        assert!(x.registry.review_local_rebase_cherry_pick(
            &x.project_id, ReplayReviewKind::RebaseCurrentOntoLocal, "feature").is_err());
        assert!(!x.root.join(".git/index.lock").exists());
    }
}
