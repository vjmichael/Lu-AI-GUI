//! Group 6B-02: immutable, non-executing review of selected Git diff hunks.
//! A user-selected hunk is data for a future one-use OS approval, never an
//! authorization to apply patches, stage an index entry or alter refs.
use crate::IdeWorkspaceRegistry;
use git2::{DiffOptions, Patch};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

const MAX_HUNKS: usize = 48;
const MAX_LINES_PER_HUNK: usize = 256;
const MAX_PATCH_BYTES: usize = 96 * 1024;
const MAX_LINE_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedHunkLine {
    pub origin: char,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedHunk {
    pub id: String,
    pub position: usize,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub lines: Vec<ReviewedHunkLine>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedHunkPlan {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub head_sha: String,
    pub index_fingerprint: String,
    pub index_blob_oid: String,
    pub source_sha256: String,
    pub patch_sha256: String,
    pub hunks: Vec<ReviewedHunk>,
    pub ready_for_selection: bool,
    pub stage_authorized: bool,
    pub commit_authorized: bool,
    pub git_hooks_executed: bool,
    pub network_queried: bool,
    pub git_process_started: bool,
    pub operation_performed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedHunkSelection {
    pub schema_version: u32,
    pub plan: ReviewedHunkPlan,
    pub selected_ids: Vec<String>,
    pub selection_sha256: String,
    pub ready_for_operator_review: bool,
    pub stage_authorized: bool,
    pub commit_authorized: bool,
    pub operation_performed: bool,
}

fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn hash_selection(plan: &ReviewedHunkPlan, selected: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(b"ide-g06b02-selected-hunks-v1\n");
    h.update(plan.project_id.as_bytes());
    h.update(b"\0");
    h.update(plan.relative_path.as_bytes());
    h.update(b"\0");
    h.update(plan.patch_sha256.as_bytes());
    for id in selected {
        h.update(b"\0");
        h.update(id.as_bytes());
    }
    format!("{:x}", h.finalize())
}

impl IdeWorkspaceRegistry {
    pub fn preview_reviewed_git_hunks(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<ReviewedHunkPlan, &'static str> {
        let pinned = self.read_reviewed_git_hunk_buffers(project_id, relative_path)?;
        let old = pinned.index_bytes.as_slice();
        let new = pinned.source_bytes.as_slice();
        // libgit2 can otherwise classify arbitrary non-text blobs, rename
        // candidates and huge hunks. Hunk review is text-only and must be
        // byte-exact with the original tracked regular file.
        if old.contains(&0) || new.contains(&0)
            || std::str::from_utf8(old).is_err()
            || std::str::from_utf8(new).is_err()
        {
            return Err("G06B_HUNK_BINARY_OR_NON_UTF8_DENIED");
        }
        let mut patch_hash = Sha256::new();
        patch_hash.update(b"ide-g06b02-index-to-capability-source-v1\n");
        for field in [
            pinned.review.project_id.as_bytes(),
            pinned.review.relative_path.as_bytes(),
            pinned.review.local_head.as_bytes(),
            pinned.review.index_fingerprint.as_bytes(),
            pinned.index_blob_oid.as_bytes(),
            pinned.review.worktree_sha256.as_bytes(),
        ] {
            patch_hash.update((field.len() as u64).to_le_bytes());
            patch_hash.update(field);
        }
        patch_hash.update(digest(old).as_bytes());
        patch_hash.update(digest(new).as_bytes());
        let patch_sha256 = format!("{:x}", patch_hash.finalize());
        let mut opts = DiffOptions::new();
        opts.context_lines(3).interhunk_lines(0).force_text(true);
        let path = Path::new(relative_path);
        let patch = Patch::from_buffers(old, Some(path), new, Some(path), Some(&mut opts))
            .map_err(|_| "G06B_HUNK_DIFF_UNAVAILABLE")?;
        let mut hunks = Vec::new();
        if let Some(patch) = patch {
            if patch.num_hunks() > MAX_HUNKS || patch.size(true, true, false) > MAX_PATCH_BYTES {
                return Err("G06B_HUNK_DIFF_OVER_LIMIT");
            }
            let mut displayed = 0_usize;
            for position in 0..patch.num_hunks() {
                let (hunk, line_count) = patch.hunk(position)
                    .map_err(|_| "G06B_HUNK_UNAVAILABLE")?;
                if line_count > MAX_LINES_PER_HUNK || hunk.header().len() > 320 {
                    return Err("G06B_HUNK_LINES_OVER_LIMIT");
                }
                let header = std::str::from_utf8(hunk.header())
                    .map_err(|_| "G06B_HUNK_HEADER_ENCODING_DENIED")?
                    .to_owned();
                let mut lines = Vec::with_capacity(line_count);
                let mut identity = Sha256::new();
                identity.update(b"ide-g06b02-exact-hunk-v1\n");
                identity.update(patch_sha256.as_bytes());
                identity.update((position as u64).to_le_bytes());
                identity.update((header.len() as u64).to_le_bytes());
                identity.update(header.as_bytes());
                for idx in 0..line_count {
                    let line = patch.line_in_hunk(position, idx)
                        .map_err(|_| "G06B_HUNK_LINE_UNAVAILABLE")?;
                    if !matches!(line.origin(), ' ' | '+' | '-' | '=' | '<' | '>') {
                        return Err("G06B_HUNK_UNSUPPORTED_LINE_ORIGIN");
                    }
                    if line.content().len() > MAX_LINE_BYTES {
                        return Err("G06B_HUNK_LINE_OVER_LIMIT");
                    }
                    displayed = displayed.checked_add(line.content().len())
                        .ok_or("G06B_HUNK_DIFF_OVER_LIMIT")?;
                    if displayed > MAX_PATCH_BYTES {
                        return Err("G06B_HUNK_DIFF_OVER_LIMIT");
                    }
                    let text = std::str::from_utf8(line.content())
                        .map_err(|_| "G06B_HUNK_LINE_ENCODING_DENIED")?
                        .to_owned();
                    identity.update([line.origin() as u8]);
                    identity.update(line.old_lineno().unwrap_or_default().to_le_bytes());
                    identity.update(line.new_lineno().unwrap_or_default().to_le_bytes());
                    identity.update((text.len() as u64).to_le_bytes());
                    identity.update(text.as_bytes());
                    lines.push(ReviewedHunkLine {
                        origin: line.origin(),
                        old_lineno: line.old_lineno(),
                        new_lineno: line.new_lineno(),
                        text,
                    });
                }
                hunks.push(ReviewedHunk {
                    id: format!("{:x}", identity.finalize()),
                    position,
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    header,
                    lines,
                });
            }
        }
        // The source was capability-read and verified again after libgit2's
        // index snapshot. No patch content has been applied to the index.
        Ok(ReviewedHunkPlan {
            schema_version: 1,
            project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            head_sha: pinned.review.local_head,
            index_fingerprint: pinned.review.index_fingerprint,
            index_blob_oid: pinned.index_blob_oid,
            source_sha256: pinned.review.worktree_sha256,
            patch_sha256,
            hunks,
            ready_for_selection: true,
            stage_authorized: false,
            commit_authorized: false,
            git_hooks_executed: false,
            network_queried: false,
            git_process_started: false,
            operation_performed: false,
        })
    }

    pub fn select_reviewed_git_hunks(
        &self,
        expected: &ReviewedHunkPlan,
        selected_ids: &[String],
    ) -> Result<ReviewedHunkSelection, &'static str> {
        if expected.schema_version != 1 || !expected.ready_for_selection
            || expected.stage_authorized || expected.commit_authorized
            || expected.operation_performed || expected.git_hooks_executed
            || expected.network_queried || expected.git_process_started
        {
            return Err("G06B_HUNK_PLAN_NOT_REVIEWABLE");
        }
        if selected_ids.is_empty() || selected_ids.len() > MAX_HUNKS {
            return Err("G06B_HUNK_SELECTION_COUNT_DENIED");
        }
        let fresh = self.preview_reviewed_git_hunks(&expected.project_id, &expected.relative_path)?;
        if &fresh != expected {
            return Err("G06B_HUNK_PLAN_STALE_OR_TAMPERED");
        }
        let mut selected = BTreeSet::new();
        for id in selected_ids {
            if !fresh.hunks.iter().any(|h| &h.id == id) {
                return Err("G06B_HUNK_SELECTION_UNKNOWN_ID");
            }
            if !selected.insert(id.as_str()) {
                return Err("G06B_HUNK_SELECTION_DUPLICATE_ID");
            }
        }
        // Always serialize in the native, verified hunk order, not UI order.
        let ordered = fresh.hunks.iter()
            .filter(|h| selected.contains(h.id.as_str()))
            .map(|h| h.id.clone())
            .collect::<Vec<_>>();
        let selection_sha256 = hash_selection(&fresh, &ordered);
        Ok(ReviewedHunkSelection {
            schema_version: 1, plan: fresh, selected_ids: ordered,
            selection_sha256, ready_for_operator_review: true,
            stage_authorized: false, commit_authorized: false,
            operation_performed: false,
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
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    fn fixture() -> (Fixture, IdeWorkspaceRegistry, String, PathBuf) {
        let base = std::env::temp_dir().join(format!("ide-g06b02-{:032x}", rand::random::<u128>()));
        let root = base.join("project");
        fs::create_dir_all(&root).unwrap();
        let original = (1..=44)
            .map(|n| format!("line {n:02}\n")).collect::<String>();
        fs::write(root.join("tracked.txt"), original).unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "original", &tree, &[]).unwrap();
        drop(tree); drop(index); drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        (Fixture(base), registry, id, root)
    }
    fn change_two_hunks(root: &Path) {
        let mut content = fs::read_to_string(root.join("tracked.txt")).unwrap();
        content = content.replace("line 03\n", "edited 03\n")
            .replace("line 39\n", "edited 39\n");
        fs::write(root.join("tracked.txt"), content).unwrap();
    }

    #[test]
    fn two_separate_exact_hunks_are_selectable_without_index_mutation() {
        let (_f, registry, id, root) = fixture();
        change_two_hunks(&root);
        let before = fs::read(root.join(".git/index")).unwrap();
        let head_before = Repository::open(&root).unwrap().head().unwrap().target();
        let plan = registry.preview_reviewed_git_hunks(&id, "tracked.txt").unwrap();
        assert_eq!(plan.hunks.len(), 2);
        assert_ne!(plan.hunks[0].id, plan.hunks[1].id);
        assert!(plan.hunks[0].lines.iter().any(|line| line.text.contains("edited 03")));
        assert!(plan.hunks[1].lines.iter().any(|line| line.text.contains("edited 39")));
        let chosen = registry.select_reviewed_git_hunks(&plan, &[plan.hunks[1].id.clone()])
            .unwrap();
        assert_eq!(chosen.selected_ids, vec![plan.hunks[1].id.clone()]);
        assert!(!chosen.stage_authorized && !chosen.commit_authorized && !chosen.operation_performed);
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), before);
        assert_eq!(Repository::open(&root).unwrap().head().unwrap().target(), head_before);
    }

    #[test]
    fn stale_source_forged_hunk_or_duplicate_selection_is_denied() {
        let (_f, registry, id, root) = fixture();
        change_two_hunks(&root);
        let plan = registry.preview_reviewed_git_hunks(&id, "tracked.txt").unwrap();
        assert_eq!(
            registry.select_reviewed_git_hunks(&plan, &[plan.hunks[0].id.clone(), plan.hunks[0].id.clone()])
                .unwrap_err(), "G06B_HUNK_SELECTION_DUPLICATE_ID"
        );
        assert!(registry.select_reviewed_git_hunks(&plan, &["0".repeat(64)]).is_err());
        let mut forged = plan.clone();
        forged.hunks[0].header.push('X');
        assert_eq!(registry.select_reviewed_git_hunks(&forged, &[plan.hunks[0].id.clone()])
            .unwrap_err(), "G06B_HUNK_PLAN_STALE_OR_TAMPERED");
        fs::write(root.join("tracked.txt"), b"changed again\n").unwrap();
        assert_eq!(registry.select_reviewed_git_hunks(&plan, &[plan.hunks[0].id.clone()])
            .unwrap_err(), "G06B_HUNK_PLAN_STALE_OR_TAMPERED");
    }

    #[test]
    fn binary_and_unregistered_or_untracked_paths_fail_closed() {
        let (_f, registry, id, root) = fixture();
        fs::write(root.join("tracked.txt"), b"hello\0world").unwrap();
        assert_eq!(registry.preview_reviewed_git_hunks(&id, "tracked.txt")
            .unwrap_err(), "G06B_HUNK_BINARY_OR_NON_UTF8_DENIED");
        assert!(registry.preview_reviewed_git_hunks(&id, "../outside").is_err());
        assert!(registry.preview_reviewed_git_hunks(&id, "untracked.txt").is_err());
        assert!(registry.preview_reviewed_git_hunks("not-registered", "tracked.txt").is_err());
    }
}
