//! WO6 post-execution artifact evidence. Read-only metadata/hashes from a
//! native worker's already-published output root. Never opens an arbitrary UI
//! path, grants execution, loads artifact bytes into WebView, or represents a
//! receipt/journal authentication boundary. Child must be terminated first.
use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use crate::SupervisorError;

const MAX_ENTRIES: usize = 96;
const MAX_TREE_DEPTH: usize = 24;
const MAX_ONE_FILE: u64 = 64 * 1024 * 1024;
const MAX_TREE_BYTES: u64 = 256 * 1024 * 1024;
const READ_CHUNK: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEntry {
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputWitness {
    pub tree_sha256: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub files: Vec<ArtifactEntry>,
    pub truncated: bool,
    pub role: &'static str,
}
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
fn is_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}
fn checked_meta(path: &Path, directory: bool) -> Result<fs::Metadata, SupervisorError> {
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if meta.file_type().is_symlink()
        || is_reparse(&meta)
        || if directory { !meta.is_dir() } else { !meta.is_file() }
    {
        return Err(denied());
    }
    Ok(meta)
}
fn valid_component(name: &str) -> bool {
    !name.is_empty() && name.len() <= 200 &&
        name.bytes().all(|b| b.is_ascii_alphanumeric()
            || matches!(b, b'.' | b'-' | b'_' | b'+'))
}
fn hash_regular_file(path: &Path, expected_size: u64) -> Result<String, SupervisorError> {
    if expected_size > MAX_ONE_FILE {
        return Err(denied());
    }
    let before = checked_meta(path, false)?;
    if before.len() != expected_size {
        return Err(denied());
    }
    let mut reader = File::open(path).map_err(|_| denied())?;
    let opened = reader.metadata().map_err(|_| denied())?;
    if !opened.is_file() || opened.len() != expected_size {
        return Err(denied());
    }
    let mut hasher = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0u8; READ_CHUNK];
    loop {
        let n = reader.read(&mut buf).map_err(|_| denied())?;
        if n == 0 { break; }
        count = count.checked_add(n as u64).ok_or_else(denied)?;
        if count > expected_size || count > MAX_ONE_FILE {
            return Err(denied());
        }
        hasher.update(&buf[..n]);
    }
    if count != expected_size ||
        checked_meta(path, false)?.len() != expected_size
    {
        return Err(denied());
    }
    Ok(format!("{:x}", hasher.finalize()))
}
/// Only invoke with a native-worker produced, independently sealed output
/// directory AFTER verified kernel descendant cleanup and terminal receipt.
/// This is a bounded, advisory host readback; not an OS sandbox witness.
pub fn audit_published_output(root: &Path) -> Result<OutputWitness, SupervisorError> {
    if !root.is_absolute() {
        return Err(denied());
    }
    checked_meta(root, true)?;
    let mut files = Vec::new();
    let mut total = 0u64;
    fn walk(
        base: &Path,
        relative: &Path,
        depth: usize,
        files: &mut Vec<ArtifactEntry>,
        total: &mut u64,
    ) -> Result<(), SupervisorError> {
        if depth > MAX_TREE_DEPTH { return Err(denied()); }
        let dir = base.join(relative);
        checked_meta(&dir, true)?;
        let mut entries = Vec::new();
        for item in fs::read_dir(&dir).map_err(|_| denied())? {
            let item = item.map_err(|_| denied())?;
            let name = item.file_name().into_string().map_err(|_| denied())?;
            if !valid_component(&name) { return Err(denied()); }
            entries.push(name);
            if entries.len() > MAX_ENTRIES { return Err(denied()); }
        }
        entries.sort();
        for name in entries {
            let path = relative.join(name);
            if !path.components().all(|part| matches!(part, Component::Normal(_))) {
                return Err(denied());
            }
            let abs = base.join(&path);
            let meta = fs::symlink_metadata(&abs).map_err(|_| denied())?;
            if meta.is_dir() {
                checked_meta(&abs, true)?;
                walk(base, &path, depth + 1, files, total)?;
            } else {
                let file = checked_meta(&abs, false)?;
                *total = total.checked_add(file.len()).ok_or_else(denied)?;
                if *total > MAX_TREE_BYTES || files.len() >= MAX_ENTRIES {
                    return Err(denied());
                }
                let digest = hash_regular_file(&abs, file.len())?;
                let relative_path = path.to_str().ok_or_else(denied)?.replace('\\', "/");
                files.push(ArtifactEntry {
                    relative_path, bytes: file.len(), sha256: digest,
                });
            }
        }
        Ok(())
    }
    walk(root, Path::new(""), 0, &mut files, &mut total)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut hasher = Sha256::new();
    for file in &files {
        hasher.update(file.relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(file.bytes.to_le_bytes());
        hasher.update(file.sha256.as_bytes());
        hasher.update([0]);
    }
    Ok(OutputWitness {
        tree_sha256: format!("{:x}", hasher.finalize()),
        file_count: files.len(),
        total_bytes: total,
        files,
        truncated: false,
        role: "advisory_post_termination_output_metadata_not_os_acceptance",
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lu-wo6-out-{:032x}", rand::random::<u128>()))
    }
    #[test]
    fn sealed_relative_artifact_manifest_uses_exact_bytes() {
        let root = temp();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/fixture.bin"), b"hello test").unwrap();
        let witness = audit_published_output(&root).unwrap();
        assert_eq!(witness.file_count, 1);
        assert_eq!(witness.total_bytes, 10);
        assert_eq!(witness.files[0].relative_path, "target/debug/fixture.bin");
        assert_eq!(witness.files[0].sha256,
            format!("{:x}", Sha256::digest(b"hello test")));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_oversize_or_untrusted_names() {
        let root = temp();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("bad name.txt"), b"data").unwrap();
        assert!(audit_published_output(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_without_following_them() {
        use std::os::unix::fs::symlink;
        let root = temp();
        std::fs::create_dir_all(&root).unwrap();
        symlink("/etc/passwd", root.join("shortcut")).unwrap();
        assert!(audit_published_output(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
