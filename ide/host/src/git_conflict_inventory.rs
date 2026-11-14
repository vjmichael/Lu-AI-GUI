//! IDE 6D-01: read-only inventory of REAL unresolved Git index conflicts.
//!
//! This is not a conflict-resolution executor. Source selection, stage OIDs,
//! blob previews and the exact index bytes are bound to one observation digest.
//! It never calls index.write, checkout, merge_trees, commit, or Git CLI.
use crate::{git_branch_operations::exact_repo, git_protected_merge::changed_path_is_plain_safe, IdeWorkspaceRegistry};
use git2::{IndexEntry, Repository};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MAX_INDEX: u64 = 8 * 1024 * 1024;
const MAX_CONFLICTS: usize = 128;
const MAX_BLOB: usize = 1024 * 1024;
const MAX_PREVIEW: usize = 2048;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictStageEvidence {
    pub oid: String,
    pub mode: u32,
    pub object_size: usize,
    pub preview: Option<String>,
    pub preview_truncated: bool,
    pub binary_or_non_utf8: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeConflictEntry {
    pub relative_path: String,
    pub ancestor: Option<ConflictStageEvidence>,
    pub ours: Option<ConflictStageEvidence>,
    pub theirs: Option<ConflictStageEvidence>,
    pub classification: &'static str,
    pub human_resolution_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeConflictInventory {
    pub schema_version: u32,
    pub project_id: String,
    pub current_branch: String,
    pub current_head: String,
    pub git_state: String,
    pub index_sha256: String,
    pub conflict_count: usize,
    pub conflicts: Vec<NativeConflictEntry>,
    pub evidence_sha256: String,
    pub source_rechecked: bool,
    pub authority_granted: bool,
    pub resolution_written: bool,
    pub index_or_worktree_modified: bool,
    pub branch_ref_modified: bool,
    pub remote_contacted: bool,
    pub git_process_started: bool,
}

fn hash(data: &[u8]) -> String { format!("{:x}", Sha256::digest(data)) }

fn raw_index(gitdir: &Path) -> Result<String, &'static str> {
    let file = gitdir.join("index");
    let meta = fs::symlink_metadata(&file).map_err(|_| "G06D01_INDEX_MISSING")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX {
        return Err("G06D01_INDEX_UNTRUSTED");
    }
    let data = fs::read(file).map_err(|_| "G06D01_INDEX_READ_DENIED")?;
    if data.len() as u64 != meta.len() { return Err("G06D01_INDEX_RACED"); }
    Ok(hash(&data))
}

fn path_of(entry: &IndexEntry) -> Result<&str, &'static str> {
    std::str::from_utf8(&entry.path).map_err(|_| "G06D01_PATH_NOT_UTF8")
}
fn stage(repo: &Repository, entry: Option<&IndexEntry>)
    -> Result<Option<ConflictStageEvidence>, &'static str> {
    let Some(entry) = entry else { return Ok(None); };
    if !matches!(entry.mode, 0o100644 | 0o100755) {
        return Err("G06D01_UNSUPPORTED_CONFLICT_MODE");
    }
    let blob = repo.find_blob(entry.id).map_err(|_| "G06D01_BLOB_UNAVAILABLE")?;
    if blob.size() > MAX_BLOB { return Err("G06D01_BLOB_TOO_LARGE"); }
    let bytes = blob.content();
    let clean = !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok();
    let view = if clean {
        let chars = std::str::from_utf8(bytes).map_err(|_| "G06D01_BLOB_ENCODING")?;
        Some(chars.chars().take(MAX_PREVIEW).collect())
    } else { None };
    let truncated = clean && std::str::from_utf8(bytes)
        .map_err(|_| "G06D01_BLOB_ENCODING")?.chars().count() > MAX_PREVIEW;
    Ok(Some(ConflictStageEvidence {
        oid: entry.id.to_string(), mode: entry.mode,
        object_size: bytes.len(), preview: view, preview_truncated: truncated,
        binary_or_non_utf8: !clean,
    }))
}
fn fence(repo: &Repository) -> Result<(String, String, String), &'static str> {
    let lock = match fs::symlink_metadata(repo.path().join("index.lock")) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("G06D01_LOCK_STATUS_UNVERIFIABLE"),
    };
    if repo.is_shallow() || repo.head_detached().unwrap_or(true) || lock {
        return Err("G06D01_UNSAFE_REPOSITORY");
    }
    let head = repo.head().map_err(|_| "G06D01_HEAD_UNAVAILABLE")?;
    let branch = head.name().and_then(|x| x.strip_prefix("refs/heads/"))
        .ok_or("G06D01_NOT_LOCAL_HEAD")?;
    let oid = head.target().ok_or("G06D01_NOT_DIRECT_HEAD")?;
    repo.find_commit(oid).map_err(|_| "G06D01_INVALID_HEAD")?;
    Ok((branch.to_owned(), oid.to_string(), format!("{:?}", repo.state())))
}

impl IdeWorkspaceRegistry {
    /// Read actual stage 1/2/3 index entries, not speculative 6C02 path overlap.
    pub fn inspect_native_git_conflicts(&self, project_id: &str)
        -> Result<NativeConflictInventory, &'static str> {
        let repo = exact_repo(self, project_id)?;
        let root = self.registered_git_root(project_id).map_err(|_| "G06D01_PROJECT_CLOSED")?;
        let initial = fence(&repo)?;
        let before_index = raw_index(repo.path())?;
        let index = repo.index().map_err(|_| "G06D01_INDEX_OPEN_DENIED")?;
        let mut entries = Vec::new();
        let mut count = 0usize;
        if index.has_conflicts() {
            for record in index.conflicts().map_err(|_| "G06D01_CONFLICT_SCAN_FAILED")? {
                let c = record.map_err(|_| "G06D01_CONFLICT_CORRUPTED")?;
                count += 1;
                if count > MAX_CONFLICTS { return Err("G06D01_CONFLICT_LIMIT"); }
                let paths = [c.ancestor.as_ref(), c.our.as_ref(), c.their.as_ref()];
                let names: Vec<&str> = paths.iter().flatten().map(|e| path_of(e))
                    .collect::<Result<_,_>>()?;
                let first = names.first().ok_or("G06D01_CONFLICT_WITHOUT_PATH")?;
                if names.iter().any(|s| s != first) {
                    // Renames require their own manual planning; avoid falsely
                    // presenting different paths as one text conflict.
                    return Err("G06D01_RENAME_CONFLICT_REQUIRES_SPECIALIST");
                }
                changed_path_is_plain_safe(&root, first)?;
                let ancestor = stage(&repo, c.ancestor.as_ref())?;
                let ours = stage(&repo, c.our.as_ref())?;
                let theirs = stage(&repo, c.their.as_ref())?;
                let classification = if ancestor.is_none() {
                    "add_add_or_no_base"
                } else if ours.is_none() || theirs.is_none() {
                    "modify_delete_or_missing_side"
                } else if [ancestor.as_ref(), ours.as_ref(), theirs.as_ref()]
                    .iter().flatten().any(|s| s.binary_or_non_utf8) {
                    "binary_or_non_utf8"
                } else { "three_way_text_conflict" };
                entries.push(NativeConflictEntry {
                    relative_path: (*first).to_owned(), ancestor, ours, theirs,
                    classification, human_resolution_required: true,
                });
            }
        }
        if fence(&repo)? != initial || raw_index(repo.path())? != before_index {
            return Err("G06D01_SOURCE_CHANGED_DURING_INSPECTION");
        }
        let mut sha = Sha256::new();
        sha.update(b"lu-ide-g06d01-readonly-conflict-inventory-v1\n");
        for s in [project_id, &initial.0, &initial.1, &initial.2, &before_index] {
            sha.update((s.len() as u64).to_le_bytes()); sha.update(s.as_bytes());
        }
        for e in &entries {
            sha.update((e.relative_path.len() as u64).to_le_bytes());
            sha.update(e.relative_path.as_bytes());
            for record in [&e.ancestor, &e.ours, &e.theirs] {
                match record {
                    None => sha.update(b"NONE"),
                    Some(v) => {
                        sha.update(v.oid.as_bytes());
                        sha.update(v.mode.to_le_bytes());
                    }
                }
            }
        }
        Ok(NativeConflictInventory {
            schema_version: 1, project_id: project_id.to_owned(),
            current_branch: initial.0, current_head: initial.1, git_state: initial.2,
            index_sha256: before_index, conflict_count: count, conflicts: entries,
            evidence_sha256: format!("{:x}", sha.finalize()),
            source_rechecked: true, authority_granted: false, resolution_written: false,
            index_or_worktree_modified: false, branch_ref_modified: false,
            remote_contacted: false, git_process_started: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::path::PathBuf;

    #[test]
    fn content_preview_is_bounded_and_blob_identity_is_original() {
        let root = std::env::temp_dir().join(
            format!("ide-g06d01-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        fs::write(root.join("a.txt"), b"source line\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let entry = index.get_path(Path::new("a.txt"), 0).unwrap();
        let before = raw_index(repo.path()).unwrap();
        let proof = stage(&repo, Some(&entry)).unwrap().unwrap();
        assert_eq!(proof.preview.as_deref(), Some("source line\n"));
        assert_eq!(proof.oid, entry.id.to_string());
        assert_eq!(proof.mode, 0o100644);
        assert!(!proof.binary_or_non_utf8 && !proof.preview_truncated);
        assert_eq!(before, raw_index(repo.path()).unwrap());
        drop(index);
        drop(repo);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_stage_is_not_exposed_as_utf8_text() {
        let root = std::env::temp_dir().join(
            format!("ide-g06d01-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        fs::write(root.join("binary.bin"), [0, 159, 255, 0]).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("binary.bin")).unwrap();
        let entry = index.get_path(Path::new("binary.bin"), 0).unwrap();
        let proof = stage(&repo, Some(&entry)).unwrap().unwrap();
        assert!(proof.binary_or_non_utf8);
        assert!(proof.preview.is_none());
        drop(index);
        drop(repo);
        fs::remove_dir_all(root).unwrap();
    }
    struct Picker(PathBuf);
    impl crate::NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, crate::IdeError> {
            Ok(Some(self.0.clone()))
        }
    }

    #[test]
    fn real_unresolved_stage_one_two_three_is_inspected_without_mutation() {
        let base = std::env::temp_dir().join(
            format!("ide-g06d01-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("conflict.txt"), b"original\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let sig = git2::Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("conflict.txt")).unwrap();
        index.write().unwrap();
        let mut ancestor = index.get_path(Path::new("conflict.txt"), 0).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "baseline", &tree, &[]).unwrap();
        drop(tree);
        index.clear().unwrap();
        for (stage, bytes) in [
            (1u16, b"original\n".as_slice()),
            (2u16, b"ours\n".as_slice()),
            (3u16, b"theirs\n".as_slice()),
        ] {
            ancestor.id = repo.blob(bytes).unwrap();
            ancestor.file_size = bytes.len() as u32;
            ancestor.flags = (ancestor.flags & !0x3000) | (stage << 12);
            index.add(&ancestor).unwrap();
        }
        index.write().unwrap();
        assert!(index.has_conflicts());
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        let original_index = fs::read(root.join(".git/index")).unwrap();
        let original_worktree = fs::read(root.join("conflict.txt")).unwrap();
        let inspection = registry.inspect_native_git_conflicts(&id).unwrap();
        assert_eq!(inspection.conflict_count, 1);
        assert_eq!(inspection.conflicts[0].relative_path, "conflict.txt");
        assert_eq!(inspection.conflicts[0].classification, "three_way_text_conflict");
        assert_eq!(inspection.conflicts[0].ancestor.as_ref().unwrap().preview.as_deref(),
            Some("original\n"));
        assert_eq!(inspection.conflicts[0].ours.as_ref().unwrap().preview.as_deref(),
            Some("ours\n"));
        assert_eq!(inspection.conflicts[0].theirs.as_ref().unwrap().preview.as_deref(),
            Some("theirs\n"));
        assert!(inspection.source_rechecked);
        assert!(!inspection.authority_granted && !inspection.resolution_written);
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), original_index);
        assert_eq!(fs::read(root.join("conflict.txt")).unwrap(), original_worktree);
        assert_eq!(
            registry.inspect_native_git_conflicts(&id).unwrap().evidence_sha256,
            inspection.evidence_sha256
        );
        drop(registry);
        fs::remove_dir_all(base).unwrap();
    }

}
