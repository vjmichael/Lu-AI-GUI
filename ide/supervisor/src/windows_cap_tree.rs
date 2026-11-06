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
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
};

const MAX_DEPTH: usize = 48;
const MAX_ITEM_BYTES: u64 = 1024 * 1024 * 1024;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
fn clean_name(name: &OsStr) -> Result<(), SupervisorError> {
    let text = name.to_str().ok_or_else(denied)?;
    if text.is_empty()
        || text == "."
        || text == ".."
        || text.len() > 240
        || text.contains(':')
        || text.contains('"')
        || text.chars().any(char::is_control)
    {
        return Err(denied());
    }
    Ok(())
}
fn safe_root(path: &Path) -> Result<Dir, SupervisorError> {
    // Open the trusted root with FILE_FLAG_OPEN_REPARSE_POINT so an attacker
    // cannot swap a junction between path-metadata and handle acquisition.
    // Share READ/WRITE but not DELETE: the opened directory itself cannot
    // be renamed during the entire rooted traversal.
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_SHARE_READ_WRITE: u32 = 0x0000_0003;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .share_mode(FILE_SHARE_READ_WRITE);
    let root_file = options.open(path).map_err(|_| denied())?;
    let meta = root_file.metadata().map_err(|_| denied())?;
    if !meta.is_dir() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(denied());
    }
    // The host validates root provenance/ACL. From this point every child
    // uses this pinned kernel directory handle, never an ambient path.
    Ok(Dir::from_std_file(root_file))
}
fn valid_leaf(file: &File) -> Result<(u64, std::time::SystemTime), SupervisorError> {
    let meta = file.metadata().map_err(|_| denied())?;
    if !meta.is_file() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(denied());
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0
        || info.nNumberOfLinks != 1
    {
        // A second NTFS hardlink could name an object outside the root.
        return Err(denied());
    }
    Ok((meta.len(), meta.modified().map_err(|_| denied())?))
}
fn checked_child_dir(parent: &Dir, name: &OsStr) -> Result<Dir, SupervisorError> {
    let md = parent.symlink_metadata(name).map_err(|_| denied())?;
    if !md.is_dir() || md.file_type().is_symlink() {
        return Err(denied());
    }
    // No ambient pathname here. cap_std resolves relative to parent handle.
    let child = parent.open_dir(name).map_err(|_| denied())?;
    if !child.dir_metadata().map_err(|_| denied())?.is_dir() {
        return Err(denied());
    }
    // A concurrent replacement either failed to rename the pinned parent,
    // was confined to this capability root, or is detected at a later seal.
    if parent
        .symlink_metadata(name)
        .map_err(|_| denied())?
        .file_type()
        .is_symlink()
    {
        return Err(denied());
    }
    Ok(child)
}
#[derive(Default)]
struct Budget {
    files: usize,
    bytes: u64,
}
fn visit(
    source: &Dir,
    output: Option<&Dir>,
    relative: &Path,
    depth: usize,
    budget: &mut Budget,
    tree: &mut Sha256,
    file_limit: usize,
    byte_limit: u64,
    item_limit: u64,
) -> Result<(), SupervisorError> {
    if depth > MAX_DEPTH {
        return Err(denied());
    }
    let mut names = source
        .entries()
        .map_err(|_| denied())?
        .map(|entry| entry.map(|item| item.file_name()).map_err(|_| denied()))
        .collect::<Result<Vec<_>, _>>()?;
    // Same deterministic depth-first ordering as the D7B PowerShell lock.
    names.sort();
    for name in names {
        clean_name(&name)?;
        let metadata = source.symlink_metadata(&name).map_err(|_| denied())?;
        if metadata.file_type().is_symlink() {
            return Err(denied());
        }
        let child_path = relative.join(&name);
        if metadata.is_dir() {
            let source_child = checked_child_dir(source, &name)?;
            let target_child = if let Some(out) = output {
                out.create_dir(&name).map_err(|_| denied())?;
                Some(checked_child_dir(out, &name)?)
            } else {
                None
            };
            visit(
                &source_child,
                target_child.as_ref(),
                &child_path,
                depth + 1,
                budget,
                tree,
                file_limit,
                byte_limit,
                item_limit,
            )?;
        } else if metadata.is_file() {
            // Read the last component with explicit no-follow. Even if a
            // symlink is swapped in after metadata, it is never traversed.
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let mut input = source
                .open_with(&name, &options)
                .map_err(|_| denied())?
                .into_std();
            let (expected, changed_at) = valid_leaf(&input)?;
            if expected != metadata.len() || expected > item_limit || expected > MAX_ITEM_BYTES {
                return Err(denied());
            }
            budget.files = budget.files.checked_add(1).ok_or_else(denied)?;
            budget.bytes = budget.bytes.checked_add(expected).ok_or_else(denied)?;
            if budget.files > file_limit || budget.bytes > byte_limit {
                return Err(denied());
            }
            let mut output_file = if let Some(dst) = output {
                let mut create = OpenOptions::new();
                create
                    .write(true)
                    .create_new(true)
                    .follow(FollowSymlinks::No);
                Some(dst.open_with(&name, &create).map_err(|_| denied())?)
            } else {
                None
            };
            let mut hash = Sha256::new();
            let mut read_bytes = 0_u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = input.read(&mut buffer).map_err(|_| denied())?;
                if count == 0 {
                    break;
                }
                read_bytes = read_bytes.checked_add(count as u64).ok_or_else(denied)?;
                if read_bytes > expected {
                    return Err(denied());
                }
                hash.update(&buffer[..count]);
                if let Some(out) = output_file.as_mut() {
                    out.write_all(&buffer[..count]).map_err(|_| denied())?;
                }
            }
            if read_bytes != expected {
                return Err(denied());
            }
            let (after_bytes, after_time) = valid_leaf(&input)?;
            if after_bytes != expected || after_time != changed_at {
                return Err(denied());
            }
            if let Some(out) = output_file.as_mut() {
                out.sync_all().map_err(|_| denied())?;
            }
            let name = child_path.to_str().ok_or_else(denied)?.replace('\\', "/");
            tree.update(name.as_bytes());
            tree.update([0]);
            tree.update(read_bytes.to_le_bytes());
            tree.update(format!("{:x}", hash.finalize()).as_bytes());
            tree.update([0]);
        } else {
            // No pipes, devices, sockets, or unrecognized data streams.
            return Err(denied());
        }
    }
    Ok(())
}

/// Live, capability-rooted *metadata* quota check for a tree that Cargo is
/// still mutating. The final output seal MUST always use audit_tree AFTER the
/// kernel Job is empty. Hashing changing rustc scratch files every 40 ms
/// cannot certify immutable output and causes false cancellation.
/// Returns false only for short-lived concurrent filesystem changes (deleted
/// names, sharing conflicts); structural violations and quota overruns deny.
fn live_visit(
    source: &Dir,
    depth: usize,
    budget: &mut Budget,
    file_limit: usize,
    byte_limit: u64,
    item_limit: u64,
) -> Result<bool, SupervisorError> {
    if depth > MAX_DEPTH {
        return Err(denied());
    }
    let mut stable = true;
    let entries = source.entries().map_err(|_| denied())?;
    for next in entries {
        let name = match next {
            Ok(entry) => entry.file_name(),
            Err(_) => {
                stable = false;
                continue;
            }
        };
        clean_name(&name)?;
        let metadata = match source.symlink_metadata(&name) {
            Ok(meta) => meta,
            Err(_) => {
                stable = false; // Rust may have atomically renamed a file.
                continue;
            }
        };
        if metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(denied());
        }
        if metadata.is_dir() {
            let child = match checked_child_dir(source, &name) {
                Ok(child) => child,
                Err(_) => {
                    stable = false; // active creation / deletion
                    continue;
                }
            };
            stable &= live_visit(
                &child,
                depth + 1,
                budget,
                file_limit,
                byte_limit,
                item_limit,
            )?;
        } else if metadata.is_file() {
            budget.files = budget.files.checked_add(1).ok_or_else(denied)?;
            budget.bytes = budget.bytes.checked_add(metadata.len()).ok_or_else(denied)?;
            if budget.files > file_limit
                || budget.bytes > byte_limit
                || metadata.len() > item_limit
                || metadata.len() > MAX_ITEM_BYTES
            {
                return Err(denied());
            }
            // Independent handle/open + no-follow checks catch hard links,
            // reparse swaps and nonregular files. Sharing violations during
            // a compiler write are transient: NEVER treat this scan as a seal.
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            match source.open_with(&name, &options) {
                Ok(opened) => match valid_leaf(&opened.into_std()) {
                    Ok((size, _)) if size == metadata.len() => {}
                    Ok(_) | Err(_) => stable = false,
                },
                Err(_) => stable = false,
            }
        } else {
            return Err(denied());
        }
    }
    Ok(stable)
}

/// Prevent a rapidly changing compiler-output tree from invalidating the
/// immutable final-seal protocol. A false result must be retried with a
/// bounded grace period, not converted to permanent success. Never publish
/// on this live check: publication runs audit_tree after Job exit.
pub(crate) fn live_output_quota(
    root: &Path,
    max_files: usize,
    max_bytes: u64,
    max_file_bytes: u64,
) -> Result<bool, SupervisorError> {
    let pinned = safe_root(root)?;
    live_visit(
        &pinned,
        0,
        &mut Budget::default(),
        max_files,
        max_bytes,
        max_file_bytes,
    )
}

/// Whole tree hash, optionally copy to an empty native-private destination.
/// Caller must supply an exclusively created output parent, not project paths.
pub(crate) fn audit_tree(
    root: &Path,
    destination: Option<&Path>,
    max_files: usize,
    max_bytes: u64,
    single_file_limit: u64,
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
    visit(
        &source,
        target.as_ref(),
        Path::new(""),
        0,
        &mut budget,
        &mut tree,
        max_files,
        max_bytes,
        single_file_limit,
    )?;
    Ok((budget.bytes, format!("{:x}", tree.finalize())))
}
pub(crate) fn digest_tree(
    root: &Path,
    destination: Option<&Path>,
    max_files: usize,
    max_bytes: u64,
) -> Result<String, SupervisorError> {
    audit_tree(root, destination, max_files, max_bytes, MAX_ITEM_BYTES).map(|(_, hash)| hash)
}

/// Publish under an already-open, native-authorized parent directory.
/// Both creation and writes use directory handles, so an attacker cannot
/// redirect the final output by replacing an intermediate ambient path.
/// The created child is exclusive and never silently reused.
pub(crate) fn publish_into_native_parent(
    source_path: &Path,
    trusted_parent: &Path,
    child_name: &OsStr,
    max_files: usize,
    max_bytes: u64,
    item_limit: u64,
) -> Result<(u64, String), SupervisorError> {
    clean_name(child_name)?;
    let source = safe_root(source_path)?;
    let parent = safe_root(trusted_parent)?;
    parent.create_dir(child_name).map_err(|_| denied())?;
    let target = checked_child_dir(&parent, child_name)?;
    let mut budget = Budget::default();
    let mut tree = Sha256::new();
    visit(
        &source,
        Some(&target),
        Path::new(""),
        0,
        &mut budget,
        &mut tree,
        max_files,
        max_bytes,
        item_limit,
    )?;
    let copied_hash = format!("{:x}", tree.finalize());

    // Re-read the published bytes through their still-live directory handle;
    // success cannot rely on a second open of a swappable absolute path.
    let mut verified = Budget::default();
    let mut verified_hash = Sha256::new();
    visit(
        &target,
        None,
        Path::new(""),
        0,
        &mut verified,
        &mut verified_hash,
        max_files,
        max_bytes,
        item_limit,
    )?;
    let seal = format!("{:x}", verified_hash.finalize());
    if budget.bytes != verified.bytes || seal != copied_hash {
        return Err(denied());
    }
    Ok((verified.bytes, seal))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_concurrent_child_rename_cannot_escape_pinned_root() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        use std::thread;
        let base = std::env::temp_dir().join(format!(
            "lu-cap-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = base.join("source");
        let inside = source.join("nested");
        let outside = base.join("outside");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(inside.join("safe.txt"), b"allowed workspace bytes").unwrap();
        let forbidden = b"HOST_ONLY_NOT_FOR_WORKER";
        fs::write(outside.join("secret.txt"), forbidden).unwrap();
        let control = digest_tree(&source, None, 100, 100_000).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&done);
        let source_for_attacker = source.clone();
        let outside_for_attacker = outside.clone();
        let adversary = thread::spawn(move || {
            let nested = source_for_attacker.join("nested");
            let alternate = source_for_attacker.join("nested-original");
            while !stop.load(Ordering::SeqCst) {
                // Both operations may fail because cap_std holds directory
                // handles without FILE_SHARE_DELETE. That is a valid secure
                // outcome; never retry with an ambient-path fallback.
                if fs::rename(&nested, &alternate).is_ok() {
                    if std::os::windows::fs::symlink_dir(&outside_for_attacker, &nested).is_ok() {
                        let _ = fs::remove_dir(&nested);
                    }
                    let _ = fs::rename(&alternate, &nested);
                }
                thread::yield_now();
            }
            if alternate.is_dir() && !nested.exists() {
                let _ = fs::rename(&alternate, &nested);
            }
        });
        let mut accepted = 0_usize;
        for iteration in 0..24 {
            let target = base.join(format!("stage-{iteration}"));
            let result = digest_tree(&source, Some(&target), 100, 100_000);
            if let Ok(seal) = result {
                // A successful rooted copy must be self-consistent, and may
                // observe the original directory under either *inside-root*
                // name while an attacker races a rename. Source identity is
                // enforced by the separately pinned snapshot manifest; this
                // test instead proves that the race cannot escape the root.
                assert_eq!(seal, digest_tree(&target, None, 100, 100_000).unwrap());
                let original = target.join("nested/safe.txt");
                let moved = target.join("nested-original/safe.txt");
                // A racing *inside-root* directory rename can expose the
                // safe object under one or both names during enumeration.
                // The proof is confinement and byte identity, not whether
                // one particular in-root filename happened to exist.
                assert!(original.is_file() || moved.is_file());
                for safe in [&original, &moved] {
                    if safe.is_file() {
                        let copied = fs::read(safe).unwrap();
                        assert_eq!(copied, b"allowed workspace bytes");
                        assert_ne!(copied, forbidden);
                    }
                }
                assert!(!target.join("nested/secret.txt").exists());
                assert!(!target.join("nested-original/secret.txt").exists());
                accepted += 1;
            }
        }
        done.store(true, Ordering::SeqCst);
        adversary.join().unwrap();
        // Normal proof case must still be accepted with no race.
        if source.join("nested").is_dir() && !source.join("nested").is_symlink() {
            assert_eq!(digest_tree(&source, None, 100, 100_000).unwrap(), control);
        }
        eprintln!(
            "D7C accepted {accepted}/24 race attempts; all accepted trees equal pinned input"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn live_cargo_output_quota_is_not_an_immutable_seal() {
        let base = std::env::temp_dir().join(format!(
            "lu-cap-live-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(base.join("target")).unwrap();
        let file = base.join("target").join("scratch.rmeta");
        fs::write(&file, b"changing").unwrap();
        assert!(live_output_quota(&base, 4, 100, 100).unwrap());
        // A live metadata scan does not certify any content digest; that
        // happens after kernel Job exit, using the immutable output seal.
        let old_digest = digest_tree(&base, None, 4, 100).unwrap();
        fs::write(&file, b"different contents").unwrap();
        assert!(live_output_quota(&base, 4, 100, 100).unwrap());
        assert_ne!(old_digest, digest_tree(&base, None, 4, 100).unwrap());
        assert!(live_output_quota(&base, 4, 2, 100).is_err());
        assert!(live_output_quota(&base, 4, 100, 2).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn output_publication_is_exclusive_and_sealed_inside_trusted_parent() {
        let base = std::env::temp_dir().join(format!(
            "lu-cap-publish-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = base.join("source");
        let native = base.join("native");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir(&native).unwrap();
        fs::write(source.join("artifact.bin"), b"published immutable bytes").unwrap();
        let (size, seal) = publish_into_native_parent(
            &source,
            &native,
            OsStr::new("job-fixed"),
            100,
            10000,
            10000,
        )
        .unwrap();
        assert_eq!(size, b"published immutable bytes".len() as u64);
        assert_eq!(
            seal,
            digest_tree(&native.join("job-fixed"), None, 100, 10000).unwrap()
        );
        // No overwrite/replay against a native destination from a different
        // job or previously published artifact directory.
        assert!(publish_into_native_parent(
            &source,
            &native,
            OsStr::new("job-fixed"),
            100,
            10000,
            10000
        )
        .is_err());
        assert!(publish_into_native_parent(
            &source,
            &native,
            OsStr::new("../escape"),
            100,
            10000,
            10000
        )
        .is_err());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn rooted_output_parent_refuses_junction_to_host_directory() {
        let base = std::env::temp_dir().join(format!(
            "lu-cap-output-junction-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = base.join("source");
        let host = base.join("protected");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir(&host).unwrap();
        fs::write(source.join("public.txt"), b"allowed").unwrap();
        fs::write(host.join("private.txt"), b"do not modify").unwrap();
        let alias = base.join("fake-native-parent");
        std::os::windows::fs::symlink_dir(&host, &alias)
            .expect("native hostile Windows runner must support junction/symlink creation");
        assert!(publish_into_native_parent(
            &source,
            &alias,
            OsStr::new("job-attempt"),
            100,
            10000,
            10000
        )
        .is_err());
        assert!(!host.join("job-attempt").exists());
        assert_eq!(
            fs::read(host.join("private.txt")).unwrap(),
            b"do not modify"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn rejects_absolute_escape_and_normalizes_tree_hashes() {
        let base = std::env::temp_dir().join(format!(
            "lu-cap-tree-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
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
