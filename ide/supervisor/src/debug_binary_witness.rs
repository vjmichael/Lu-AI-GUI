//! DEV-04.2: independent readback of a unique native Cargo executable.
//! This does NOT launch the target, verify debug symbols, sign the bytes,
//! or grant authority to a debugger. An authenticated native worker must
//! already have terminated, yielded Completed and produced OutputWitness.
use crate::{
    wo6_output_evidence::{ArtifactEntry, OutputWitness},
    SupervisorError,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Component, Path},
};

const MAX_BINARY: u64 = 64 * 1024 * 1024;
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugBinaryWitness {
    pub relative_path: String,
    pub bytes: u64,
    pub binary_sha256: String,
    pub output_tree_sha256: String,
    pub format: &'static str,
    pub role: &'static str,
    pub debug_symbols_attested: bool,
    pub debugger_execution_authorized: bool,
    pub installed_app_accepted: bool,
}

fn candidate(entry: &ArtifactEntry) -> bool {
    let p = entry.relative_path.as_str();
    let Some(base) = p.strip_prefix("target/debug/") else {
        return false;
    };
    if base.is_empty()
        || base.contains('/')
        || base.starts_with('.')
        || base.ends_with(".d")
        || base.ends_with(".rlib")
        || base.ends_with(".rmeta")
        || base.ends_with(".pdb")
        || base.ends_with(".dll")
        || base.ends_with(".so")
        || base.ends_with(".a")
        || base.ends_with(".lib")
        || base.ends_with(".exp")
        || base.ends_with(".dSYM")
        || base.ends_with(".json")
    {
        return false;
    }
    #[cfg(windows)]
    {
        base.ends_with(".exe")
    }
    #[cfg(not(windows))]
    {
        !base.contains('.')
    }
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
fn checked_file(
    root: &Path,
    entry: &ArtifactEntry,
) -> Result<DebugBinaryWitnessFile, SupervisorError> {
    if entry.bytes == 0 || entry.bytes > MAX_BINARY {
        return Err(denied());
    }
    let relative = Path::new(&entry.relative_path);
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(denied());
    }
    let path = root.join(relative);
    let before = fs::symlink_metadata(&path).map_err(|_| denied())?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || is_reparse(&before)
        || before.len() != entry.bytes
    {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if before.nlink() != 1 || before.permissions().mode() & 0o111 == 0 {
            return Err(denied());
        }
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000); // O_NOFOLLOW Linux
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut file: File = options.open(&path).map_err(|_| denied())?;
    let opened = file.metadata().map_err(|_| denied())?;
    if !opened.is_file() || is_reparse(&opened) || opened.len() != entry.bytes {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != opened.dev() || before.ino() != opened.ino() || opened.nlink() != 1 {
            return Err(denied());
        }
    }
    let mut prefix = [0u8; 4];
    file.read_exact(&mut prefix).map_err(|_| denied())?;
    #[cfg(windows)]
    let format = if prefix[0..2] == *b"MZ" {
        "pe_candidate_mz_only"
    } else {
        return Err(denied());
    };
    #[cfg(target_os = "linux")]
    let format = if prefix == [0x7f, b'E', b'L', b'F'] {
        "elf_candidate_magic_only"
    } else {
        return Err(denied());
    };
    #[cfg(all(not(target_os = "linux"), not(windows)))]
    let format = "unrecognized_platform_candidate";
    let mut hash = Sha256::new();
    hash.update(prefix);
    let mut consumed = 4u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| denied())?;
        if count == 0 {
            break;
        }
        consumed = consumed.checked_add(count as u64).ok_or_else(denied)?;
        if consumed > entry.bytes {
            return Err(denied());
        }
        hash.update(&buffer[..count]);
    }
    if consumed != entry.bytes {
        return Err(denied());
    }
    let after = fs::symlink_metadata(&path).map_err(|_| denied())?;
    if !after.is_file()
        || is_reparse(&after)
        || after.len() != opened.len()
        || after.modified().ok() != opened.modified().ok()
    {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if after.dev() != opened.dev() || after.ino() != opened.ino() {
            return Err(denied());
        }
    }
    let digest = format!("{:x}", hash.finalize());
    if digest != entry.sha256 {
        return Err(denied());
    }
    Ok(DebugBinaryWitnessFile { format })
}
struct DebugBinaryWitnessFile {
    format: &'static str,
}

/// A library-only build, an empty output or an ambiguous multi-binary tree
/// is NOT treated as a verified debugger target.
pub fn audit_debug_binary_candidate(
    root: &Path,
    witness: &OutputWitness,
) -> Result<Option<DebugBinaryWitness>, SupervisorError> {
    if !root.is_absolute()
        || witness.truncated
        || witness.file_count != witness.files.len()
        || witness.role != "advisory_post_termination_output_metadata_not_os_acceptance"
    {
        return Ok(None);
    }
    let mut found = witness.files.iter().filter(|entry| candidate(entry));
    let Some(entry) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Ok(None);
    }
    let target = checked_file(root, entry)?;
    Ok(Some(DebugBinaryWitness {
        relative_path: entry.relative_path.clone(),
        bytes: entry.bytes,
        binary_sha256: entry.sha256.clone(),
        output_tree_sha256: witness.tree_sha256.clone(),
        format: target.format,
        role: "post_termination_unique_cargo_binary_readback_not_launch_authority",
        debug_symbols_attested: false,
        debugger_execution_authorized: false,
        installed_app_accepted: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_direct_compiled_binary_names() {
        assert!(!candidate(&ArtifactEntry {
            relative_path: "target/debug/deps/evil".into(),
            bytes: 4,
            sha256: "a".repeat(64)
        }));
        assert!(!candidate(&ArtifactEntry {
            relative_path: "target/debug/something.d".into(),
            bytes: 4,
            sha256: "a".repeat(64)
        }));
        assert!(!candidate(&ArtifactEntry {
            relative_path: "target/release/project".into(),
            bytes: 4,
            sha256: "a".repeat(64)
        }));
    }
    #[test]
    fn absent_or_ambiguous_output_is_never_promoted() {
        let entry = |name: &str| ArtifactEntry {
            relative_path: format!("target/debug/{name}"),
            bytes: 12,
            sha256: "a".repeat(64),
        };
        let mut witness = OutputWitness {
            tree_sha256: "b".repeat(64),
            file_count: 0,
            total_bytes: 0,
            files: Vec::new(),
            truncated: false,
            role: "advisory_post_termination_output_metadata_not_os_acceptance",
        };
        let root = std::env::temp_dir();
        assert!(audit_debug_binary_candidate(&root, &witness)
            .unwrap()
            .is_none());
        #[cfg(windows)]
        let entries = vec![entry("first.exe"), entry("second.exe")];
        #[cfg(not(windows))]
        let entries = vec![entry("first"), entry("second")];
        witness.files = entries;
        witness.file_count = 2;
        // Even when a hostile OS output tree has two plausible binaries,
        // no caller may select one by name.
        assert!(audit_debug_binary_candidate(&root, &witness)
            .unwrap()
            .is_none());
        witness.truncated = true;
        assert!(audit_debug_binary_candidate(&root, &witness)
            .unwrap()
            .is_none());
    }
    #[cfg(unix)]
    #[test]
    fn actual_elf_bytes_rehashed_and_modified_binary_denied() {
        use crate::wo6_output_evidence::audit_published_output;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("lu-dev042-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("target/debug")).unwrap();
        let path = root.join("target/debug/fixture");
        fs::write(&path, b"\x7fELFbinary source").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let witness = audit_published_output(&root).unwrap();
        let result = audit_debug_binary_candidate(&root, &witness)
            .unwrap()
            .unwrap();
        assert_eq!(result.binary_sha256, witness.files[0].sha256);
        assert!(!result.debugger_execution_authorized);
        fs::write(&path, b"\x7fELFbinary changed").unwrap();
        assert!(audit_debug_binary_candidate(&root, &witness).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn actual_pe_candidate_readback_does_not_grant_debugging() {
        use crate::wo6_output_evidence::audit_published_output;
        let root = std::env::temp_dir().join(format!("lu-dev042-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/fixture.exe"), b"MZ\0\0placeholder").unwrap();
        let witness = audit_published_output(&root).unwrap();
        let result = audit_debug_binary_candidate(&root, &witness)
            .unwrap()
            .unwrap();
        assert!(!result.debugger_execution_authorized);
        fs::remove_dir_all(root).unwrap();
    }
}
