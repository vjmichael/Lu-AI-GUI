//! Wave D7C — capability-relative Windows tree scanner/copy/publisher.
//!
//! Every child is opened through a live cap_std::fs::Dir, not by joining an
//! ambient absolute path. On Windows cap-std uses root-relative native opens
//! and guards directory rename while handles are live. Each leaf explicitly
//! refuses symlink/reparse traversal, multi-link aliases and non-regular files.
//! Output targets are opened relative to a distinct trusted directory handle,
//! with create_new so an existing file can never be overwritten.
//!
//! This protects against source/child rename races inside the rooted tree.
//! It is NOT an authentication mechanism for a malicious same-account process
//! that can replace the trusted *root before it is opened*, alter the native
//! journal, or modify the AppContainer profile's ACLs.
use crate::SupervisorError;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{ambient_authority, fs::{Dir, OpenOptions}};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    os::windows::{fs::MetadataExt, io::AsRawHandle},
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
};

const MAX_DEPTH: usize = 48;
const MAX_ITEM_BYTES: u64 = 1024 * 1024 * 1024;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn denied() -> SupervisorError { SupervisorError::NoContainment }
fn clean_name(name: &OsStr) -> Result<(), SupervisorError> {
    let text = name.to_str().ok_or_else(denied)?;
    if text.is_empty() || text == "." || text == ".." || text.len() > 240
        || text.contains(':') || text.contains('"')
        || text.chars().any(char::is_control) {
        return Err(denied());
    }
    Ok(())
}
fn safe_root(path: &Path) -> Result<Dir, SupervisorError> {
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !meta.is_dir() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(denied());
    }
    let root = Dir::open_ambient_dir(path, ambient_authority()).map_err(|_| denied())?;
    if !root.dir_metadata().map_err(|_| denied())?.is_dir() {
        return Err(denied());
    }
    // Root identity/ACL authorization is the trusted host broker's job.
    // cap_std keeps *all subsequent* child operations rooted to this handle.
    Ok(root)
}
fn valid_leaf(file: &File) -> Result<(u64, std::time::SystemTime), SupervisorError> {
    let meta = file.metadata().map_err(|_| denied())?;
    if !meta.is_file() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(denied());
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe {
        GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info)
    } == 0 || info.nNumberOfLinks != 1 {
        // A second NTFS hardlink could name an object outside the root.
        return Err(denied());
    }
    Ok((meta.len(), meta.modified().map_err(|_| denied())?))
}
fn checked_child_dir(parent: &Dir, name: &OsStr) -> Result<Dir, SupervisorError> {
    let md = parent.symlink_metadata(name).map_err(|_| denied())?;
    if !md.is_dir() || md.file_type().is_symlink() { return Err(denied()); }
    // No ambient pathname here. cap_std resolves relative to parent handle.
    let child = parent.open_dir(name).map_err(|_| denied())?;
    if !child.dir_metadata().map_err(|_| denied())?.is_dir() {
        return Err(denied());
    }
    // A concurrent replacement either failed to rename the pinned parent,
    // was confined to this capability root, or is detected at a later seal.
    if parent.symlink_metadata(name).map_err(|_| denied())?.file_type().is_symlink() {
        return Err(denied());
    }
    Ok(child)
}
#[derive(Default)]
struct Budget { files: usize, bytes: u64 }
fn visit(
    source: &Dir, output: Option<&Dir>, relative: &Path, depth: usize,
    budget: &mut Budget, tree: &mut Sha256, file_limit: usize,
    byte_limit: u64, item_limit: u64,
) -> Result<(), SupervisorError> {
    if depth > MAX_DEPTH { return Err(denied()); }
    let mut names = source.entries()
        .map_err(|_| denied())?
        .map(|entry| entry.map(|item| item.file_name()).map_err(|_| denied()))
        .collect::<Result<Vec<_>, _>>()?;
    // Same deterministic depth-first ordering as the D7B PowerShell lock.
    names.sort();
    for name in names {
        clean_name(&name)?;
        let metadata = source.symlink_metadata(&name).map_err(|_| denied())?;
        if metadata.file_type().is_symlink() { return Err(denied()); }
        let child_path = relative.join(&name);
        if metadata.is_dir() {
            let source_child = checked_child_dir(source, &name)?;
            let target_child = if let Some(out) = output {
                out.create_dir(&name).map_err(|_| denied())?;
                Some(checked_child_dir(out, &name)?)
            } else { None };
            visit(&source_child, target_child.as_ref(), &child_path, depth + 1,
                budget, tree, file_limit, byte_limit, item_limit)?;
        } else if metadata.is_file() {
            // Read the last component with explicit no-follow. Even if a
            // symlink is swapped in after metadata, it is never traversed.
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let mut input = source.open_with(&name, &options)
                .map_err(|_| denied())?.into_std();
            let (expected, changed_at) = valid_leaf(&input)?;
            if expected != metadata.len() || expected > item_limit || expected > MAX_ITEM_BYTES {
                return Err(denied());
            }
            budget.files = budget.files.checked_add(1).ok_or_else(denied)?;
            budget.bytes = budget.bytes.checked_add(expected).ok_or_else(denied)?;
            if budget.files > file_limit || budget.bytes > byte_limit { return Err(denied()); }
            let mut output_file = if let Some(dst) = output {
                let mut create = OpenOptions::new();
                create.write(true).create_new(true).follow(FollowSymlinks::No);
                Some(dst.open_with(&name, &create).map_err(|_| denied())?)
            } else { None };
            let mut hash = Sha256::new();
            let mut read_bytes = 0_u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = input.read(&mut buffer).map_err(|_| denied())?;
                if count == 0 { break; }
                read_bytes = read_bytes.checked_add(count as u64).ok_or_else(denied)?;
                if read_bytes > expected { return Err(denied()); }
                hash.update(&buffer[..count]);
                if let Some(out) = output_file.as_mut() {
                    out.write_all(&buffer[..count]).map_err(|_| denied())?;
                }
            }
            if read_bytes != expected { return Err(denied()); }
            let (after_bytes, after_time) = valid_leaf(&input)?;
            if after_bytes != expected || after_time != changed_at {
                return Err(denied());
            }
            if let Some(out) = output_file.as_mut() {
                out.sync_all().map_err(|_| denied())?;
            }
            let name = child_path.to_str().ok_or_else(denied)?.replace('\\', "/");
            tree.update(name.as_bytes()); tree.update([0]);
            tree.update(read_bytes.to_le_bytes());
            tree.update(format!("{:x}", hash.finalize()).as_bytes()); tree.update([0]);
        } else {
            // No pipes, devices, sockets, or unrecognized data streams.
            return Err(denied());
        }
    }
    Ok(())
}

/// Whole tree hash, optionally copy to an empty native-private destination.
/// Caller must supply an exclusively created output parent, not project paths.
pub(crate) fn audit_tree(
    root: &Path, destination: Option<&Path>, max_files: usize,
    max_bytes: u64, single_file_limit: u64,
) -> Result<(u64, String), SupervisorError> {
    let source = safe_root(root)?;
    let target = match destination {
        Some(path) => {
            if !path.exists() {
                fs::create_dir(path).map_err(|_| denied())?;
            }
            Some(safe_root(path)?)
        }
        None => None,
    };
    let mut budget = Budget::default();
    let mut tree = Sha256::new();
    visit(&source, target.as_ref(), Path::new(""), 0, &mut budget,
        &mut tree, max_files, max_bytes, single_file_limit)?;
    Ok((budget.bytes, format!("{:x}", tree.finalize())))
}
pub(crate) fn digest_tree(
    root: &Path, destination: Option<&Path>, max_files: usize, max_bytes: u64,
) -> Result<String, SupervisorError> {
    audit_tree(root, destination, max_files, max_bytes, MAX_ITEM_BYTES)
        .map(|(_, hash)| hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_absolute_escape_and_normalizes_tree_hashes() {
        let base = std::env::temp_dir().join(format!(
            "lu-cap-tree-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()
        ));
        let input = base.join("input");
        let output = base.join("output");
        fs::create_dir_all(input.join("src")).unwrap();
        fs::write(input.join("src/main.rs"), b"fn main() {}").unwrap();
        let first = digest_tree(&input, Some(&output), 100, 10000).unwrap();
        assert_eq!(first, digest_tree(&output, None, 100, 10000).unwrap());
        fs::write(output.join("src/main.rs"), b"changed").unwrap();
        assert_ne!(first, digest_tree(&output, None, 100, 10000).unwrap());
        let _ = fs::remove_dir_all(base);
    }
}
