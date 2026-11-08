//! Immutable-input preparation for a future isolated worker.
//! A Git SHA is not a snapshot. This publisher copies bytes into an exclusive
//! application-private directory, hashes source and staged bytes, and rejects
//! symlink/reparse paths, non-files, and resource exhaustion.
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_FILES: usize = 12_000;
const MAX_DEPTH: usize = 48;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 255;
const BUFFER_BYTES: usize = 64 * 1024;
const SKIP: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".venv",
    "__pycache__",
    ".next",
    ".cache",
    "dist",
    "build",
    ".idea",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    InvalidRoot,
    UntrustedSource,
    InvalidName,
    LimitExceeded,
    Io,
    RaceDetected,
    InvalidSnapshot,
}
impl SnapshotError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "SNAPSHOT_INVALID_ROOT",
            Self::UntrustedSource => "SNAPSHOT_SOURCE_TYPE_DENIED",
            Self::InvalidName => "SNAPSHOT_INVALID_PATH",
            Self::LimitExceeded => "SNAPSHOT_INPUT_BUDGET_EXCEEDED",
            Self::Io => "SNAPSHOT_IO_UNAVAILABLE",
            Self::RaceDetected => "SNAPSHOT_SOURCE_CHANGED_DURING_COPY",
            Self::InvalidSnapshot => "SNAPSHOT_SEAL_BROKEN",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputSeal {
    pub version: u32,
    pub snapshot_id: String,
    pub source_sha256: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub manifest_sha256: String,
    pub manifest_relative_path: String,
}

pub struct PublishedInput {
    pub root: PathBuf,
    pub seal: InputSeal,
}
fn uid() -> String {
    let mut key = [0_u8; 16];
    OsRng.fill_bytes(&mut key);
    let mut out = "snap-".to_string();
    for byte in key {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
fn check_dir(path: &Path) -> Result<(), SnapshotError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| SnapshotError::Io)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(SnapshotError::UntrustedSource);
    }
    Ok(())
}
#[cfg(windows)]
fn is_reparse(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_meta: &fs::Metadata) -> bool {
    false
}

fn check_name(value: &OsStr) -> Result<String, SnapshotError> {
    let name = value.to_str().ok_or(SnapshotError::InvalidName)?;
    if name.is_empty()
        || name.as_bytes().len() > MAX_NAME_BYTES
        || name == "."
        || name == ".."
        || name.contains(':')
        || name.chars().any(char::is_control)
    {
        return Err(SnapshotError::InvalidName);
    }
    Ok(name.to_string())
}

#[derive(Default)]
struct Scan {
    entries: Vec<(String, u64)>,
    files: usize,
    total_bytes: u64,
}
fn is_generated(name: &str) -> bool {
    SKIP.contains(&name)
}
fn scan_directory(
    root: &Path,
    relative: &Path,
    depth: usize,
    scan: &mut Scan,
) -> Result<(), SnapshotError> {
    if depth > MAX_DEPTH {
        return Err(SnapshotError::LimitExceeded);
    }
    let current = root.join(relative);
    check_dir(&current)?;
    let mut children = fs::read_dir(&current)
        .map_err(|_| SnapshotError::Io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SnapshotError::Io)?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        let name = check_name(&entry.file_name())?;
        if is_generated(&name) {
            continue;
        }
        let child_relative = relative.join(&name);
        let child = root.join(&child_relative);
        let metadata = fs::symlink_metadata(&child).map_err(|_| SnapshotError::Io)?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err(SnapshotError::UntrustedSource);
        }
        if metadata.is_dir() {
            scan_directory(root, &child_relative, depth + 1, scan)?;
        } else if metadata.is_file() {
            if metadata.len() > MAX_FILE_BYTES || scan.files >= MAX_FILES {
                return Err(SnapshotError::LimitExceeded);
            }
            scan.total_bytes = scan
                .total_bytes
                .checked_add(metadata.len())
                .ok_or(SnapshotError::LimitExceeded)?;
            if scan.total_bytes > MAX_TOTAL_BYTES {
                return Err(SnapshotError::LimitExceeded);
            }
            let relative_string = child_relative
                .iter()
                .map(|component| check_name(component))
                .collect::<Result<Vec<_>, _>>()?
                .join("/");
            scan.entries.push((relative_string, metadata.len()));
            scan.files += 1;
        } else {
            // No sockets, devices, FIFOs, or future unknown filesystem types.
            return Err(SnapshotError::UntrustedSource);
        }
    }
    Ok(())
}

fn secure_file_read(path: &Path) -> Result<File, SnapshotError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| SnapshotError::Io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(SnapshotError::UntrustedSource);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NOFOLLOW: opening a swapped symlink must fail.
        options.custom_flags(0x20000);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: never transparently traverse a junction.
        options.custom_flags(0x00200000);
    }
    options.open(path).map_err(|_| SnapshotError::Io)
}

fn fingerprint_copy(
    root: &Path,
    staged_root: &Path,
    scan: &Scan,
    manifest_relative_path: &str,
) -> Result<(String, String), SnapshotError> {
    let mut tree_hash = Sha256::new();
    let mut manifest_hash = None;
    let mut buffer = [0_u8; BUFFER_BYTES];
    for (relative, expected_size) in &scan.entries {
        let local = Path::new(relative);
        if local
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(SnapshotError::InvalidName);
        }
        let source_path = root.join(local);
        let target_path = staged_root.join(local);
        let mut src = secure_file_read(&source_path)?;
        let initial = src.metadata().map_err(|_| SnapshotError::Io)?;
        if !initial.is_file() || initial.len() != *expected_size || is_reparse(&initial) {
            return Err(SnapshotError::RaceDetected);
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|_| SnapshotError::Io)?;
        }
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target_path)
            .map_err(|_| SnapshotError::Io)?;
        let mut bytes_read = 0_u64;
        let mut hasher = Sha256::new();
        loop {
            let count = src.read(&mut buffer).map_err(|_| SnapshotError::Io)?;
            if count == 0 {
                break;
            }
            bytes_read = bytes_read
                .checked_add(count as u64)
                .ok_or(SnapshotError::LimitExceeded)?;
            if bytes_read > *expected_size {
                return Err(SnapshotError::RaceDetected);
            }
            hasher.update(&buffer[..count]);
            out.write_all(&buffer[..count])
                .map_err(|_| SnapshotError::Io)?;
        }
        if bytes_read != *expected_size {
            return Err(SnapshotError::RaceDetected);
        }
        let after = src.metadata().map_err(|_| SnapshotError::Io)?;
        if after.len() != initial.len() || after.modified().ok() != initial.modified().ok() {
            return Err(SnapshotError::RaceDetected);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if after.dev() != initial.dev() || after.ino() != initial.ino() {
                return Err(SnapshotError::RaceDetected);
            }
        }
        out.sync_all().map_err(|_| SnapshotError::Io)?;
        let digest = format!("{:x}", hasher.finalize());
        tree_hash.update(relative.as_bytes());
        tree_hash.update([0]);
        tree_hash.update(bytes_read.to_le_bytes());
        tree_hash.update(digest.as_bytes());
        tree_hash.update([0]);
        if relative == manifest_relative_path {
            manifest_hash = Some(digest);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target_path, fs::Permissions::from_mode(0o400))
                .map_err(|_| SnapshotError::Io)?;
        }
    }
    let manifest_hash = manifest_hash.ok_or(SnapshotError::InvalidSnapshot)?;
    Ok((format!("{:x}", tree_hash.finalize()), manifest_hash))
}

/// Trusted profile root and registered source project must be provided by
/// native host. Caller cannot select stage filenames; symlinks are rejected.
/// This function does not attest an external toolchain or Git HEAD.
pub fn publish_input(
    source_root: &Path,
    private_stage_parent: &Path,
    recipe_id: &str,
) -> Result<PublishedInput, SnapshotError> {
    let manifest_relative_path = match recipe_id {
        "rust_check" | "rust_build" | "rust_tests" => "Cargo.toml",
        "lu_ide_host_tests" => "ide/host/Cargo.toml",
        _ => return Err(SnapshotError::InvalidName),
    };
    publish_with_manifest(source_root, private_stage_parent, manifest_relative_path)
}

/// E2: a language server analyses one exact native-verified file while the
/// entire bounded workspace is sealed. The active file's content digest
/// supplies the manifest binding; no Cargo project is required for C/Python.
/// This path is an identity checked by the host, NOT execution authority.
pub fn publish_lsp_input(
    source_root: &Path,
    private_stage_parent: &Path,
    active_relative_file: &str,
    server_id: &str,
) -> Result<PublishedInput, SnapshotError> {
    if active_relative_file.is_empty()
        || active_relative_file.len() > 1024
        || active_relative_file.contains('\\')
        || active_relative_file.contains(':')
        || active_relative_file.chars().any(char::is_control)
    {
        return Err(SnapshotError::InvalidName);
    }
    let relative = Path::new(active_relative_file);
    if !relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(SnapshotError::InvalidName);
    }
    let extension = relative
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let supported = match server_id {
        "rust-analyzer" => matches!(extension.as_str(), "rs"),
        "clangd" => matches!(
            extension.as_str(),
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hxx"
        ),
        "ruff" | "pyright" => matches!(extension.as_str(), "py" | "pyi"),
        "typescript-language-server" => matches!(
            extension.as_str(),
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs"
        ),
        _ => false,
    };
    if !supported {
        return Err(SnapshotError::InvalidName);
    }
    // Host must resolve this file against its active registered project and
    // unsaved buffer version before calling; snapshot takes only disk bytes.
    publish_with_manifest(source_root, private_stage_parent, active_relative_file)
}
fn publish_with_manifest(
    source_root: &Path,
    private_stage_parent: &Path,
    manifest_relative_path: &str,
) -> Result<PublishedInput, SnapshotError> {
    check_dir(source_root)?;
    check_dir(private_stage_parent)?;
    let canonical_root = fs::canonicalize(source_root).map_err(|_| SnapshotError::InvalidRoot)?;
    let canonical_parent =
        fs::canonicalize(private_stage_parent).map_err(|_| SnapshotError::InvalidRoot)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let owner = fs::metadata(&canonical_parent).map_err(|_| SnapshotError::InvalidRoot)?;
        if owner.uid() != unsafe { libc::geteuid() } || owner.permissions().mode() & 0o077 != 0 {
            return Err(SnapshotError::InvalidRoot);
        }
    }
    if canonical_parent.starts_with(&canonical_root)
        || canonical_root.starts_with(&canonical_parent)
    {
        return Err(SnapshotError::InvalidRoot);
    }
    let mut scan = Scan::default();
    scan_directory(&canonical_root, Path::new(""), 0, &mut scan)?;
    scan.entries.sort_by(|a, b| a.0.cmp(&b.0));
    let snapshot_id = uid();
    let dest = canonical_parent.join(&snapshot_id);
    fs::create_dir(&dest).map_err(|_| SnapshotError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o700))
            .map_err(|_| SnapshotError::Io)?;
    }
    let result = fingerprint_copy(&canonical_root, &dest, &scan, manifest_relative_path);
    let (source_sha256, manifest_sha256) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&dest);
            return Err(error);
        }
    };
    let published = PublishedInput {
        root: dest,
        seal: InputSeal {
            version: 1,
            snapshot_id,
            source_sha256,
            file_count: scan.files,
            total_bytes: scan.total_bytes,
            manifest_sha256,
            manifest_relative_path: manifest_relative_path.to_owned(),
        },
    };
    if verify_input(&published).is_err() {
        let _ = fs::remove_dir_all(&published.root);
        return Err(SnapshotError::InvalidSnapshot);
    }
    Ok(published)
}
/// Seal verification operates exclusively on staged inputs and returns a
/// deterministic fingerprint. Any file replacement, new file, or deletion
/// must fail before grant consumption and OS worker admission.
pub fn verify_input(input: &PublishedInput) -> Result<(), SnapshotError> {
    let mut scan = Scan::default();
    scan_directory(&input.root, Path::new(""), 0, &mut scan)?;
    scan.entries.sort_by(|a, b| a.0.cmp(&b.0));
    if scan.files != input.seal.file_count || scan.total_bytes != input.seal.total_bytes {
        return Err(SnapshotError::InvalidSnapshot);
    }
    let mut tree_hash = Sha256::new();
    let mut manifest_hash = None;
    let mut buffer = [0_u8; BUFFER_BYTES];
    for (relative, size) in scan.entries {
        let mut file = secure_file_read(&input.root.join(&relative))?;
        let mut hasher = Sha256::new();
        let mut length = 0_u64;
        loop {
            let count = file.read(&mut buffer).map_err(|_| SnapshotError::Io)?;
            if count == 0 {
                break;
            }
            length += count as u64;
            if length > size {
                return Err(SnapshotError::InvalidSnapshot);
            }
            hasher.update(&buffer[..count]);
        }
        if length != size {
            return Err(SnapshotError::InvalidSnapshot);
        }
        let digest = format!("{:x}", hasher.finalize());
        tree_hash.update(relative.as_bytes());
        tree_hash.update([0]);
        tree_hash.update(size.to_le_bytes());
        tree_hash.update(digest.as_bytes());
        tree_hash.update([0]);
        if relative == input.seal.manifest_relative_path {
            manifest_hash = Some(digest);
        }
    }
    let current = format!("{:x}", tree_hash.finalize());
    if current != input.seal.source_sha256
        || manifest_hash.as_deref() != Some(input.seal.manifest_sha256.as_str())
    {
        return Err(SnapshotError::InvalidSnapshot);
    }
    Ok(())
}
pub fn retire_input(input: PublishedInput) -> Result<(), SnapshotError> {
    let id = input
        .root
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(SnapshotError::InvalidName)?;
    if id != input.seal.snapshot_id || !id.starts_with("snap-") {
        return Err(SnapshotError::InvalidSnapshot);
    }
    // Trusted profile-owned paths only. Do not accept caller-provided paths.
    fs::remove_dir_all(input.root).map_err(|_| SnapshotError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(uid());
        let project = base.join("project");
        let staging = base.join("staging");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(&staging).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staging, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(
            project.join("Cargo.toml"),
            b"[package]\nname=\"test\"\nversion=\"0.1.0\"\n",
        )
        .unwrap();
        fs::write(project.join("Cargo.lock"), b"# pinned").unwrap();
        fs::create_dir(project.join("src")).unwrap();
        fs::write(project.join("src").join("main.rs"), b"fn main() {}").unwrap();
        (project, staging)
    }
    #[test]
    fn snapshot_copies_hashes_and_revalidates_seal() {
        let (project, stage) = fixture();
        let copied = publish_input(&project, &stage, "rust_tests").unwrap();
        assert_eq!(copied.seal.file_count, 3);
        assert_eq!(copied.seal.source_sha256.len(), 64);
        verify_input(&copied).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                copied.root.join("Cargo.lock"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        fs::write(copied.root.join("Cargo.lock"), b"# changed").unwrap();
        assert!(verify_input(&copied).is_err());
        retire_input(copied).unwrap();
        fs::remove_dir_all(project.parent().unwrap()).unwrap();
    }
    #[test]
    fn no_new_ignored_generated_files_enter_input() {
        let (project, stage) = fixture();
        fs::create_dir(project.join("target")).unwrap();
        fs::write(project.join("target").join("cache"), b"untrusted").unwrap();
        let copied = publish_input(&project, &stage, "rust_tests").unwrap();
        assert!(!copied.root.join("target").exists());
        retire_input(copied).unwrap();
        fs::remove_dir_all(project.parent().unwrap()).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn source_symlink_fails_closed() {
        use std::os::unix::fs::symlink;
        let (project, stage) = fixture();
        symlink("/etc/passwd", project.join("outside")).unwrap();
        assert!(matches!(
            publish_input(&project, &stage, "rust_tests"),
            Err(SnapshotError::UntrustedSource)
        ));
        fs::remove_dir_all(project.parent().unwrap()).unwrap();
    }
    #[test]
    fn private_stage_must_not_be_inside_project() {
        let (project, _stage) = fixture();
        let inside = project.join("stage");
        fs::create_dir(&inside).unwrap();
        assert!(matches!(
            publish_input(&project, &inside, "rust_tests"),
            Err(SnapshotError::InvalidRoot)
        ));
        fs::remove_dir_all(project.parent().unwrap()).unwrap();
    }
    #[test]
    fn lsp_snapshot_binds_an_active_c_file_not_an_arbitrary_project_manifest() {
        let root = std::env::temp_dir().join(format!("lu-e2-source-{}", uid()));
        let project = root.join("project");
        let stage = root.join("stage");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&stage).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(project.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
        let input = publish_lsp_input(&project, &stage, "main.c", "clangd").unwrap();
        assert_eq!(input.seal.manifest_relative_path, "main.c");
        assert!(verify_input(&input).is_ok());
        // The publisher intentionally marks sealed Unix snapshots 0o400.
        // Recreate a hostile owner chmod before mutating, as in the existing
        // Cargo snapshot seal test. Never weaken production permissions.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(input.root.join("main.c"), fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        fs::write(input.root.join("main.c"), b"changed").unwrap();
        assert!(verify_input(&input).is_err());
        assert!(matches!(
            publish_lsp_input(&project, &stage, "../main.c", "clangd"),
            Err(SnapshotError::InvalidName)
        ));
        assert!(matches!(
            publish_lsp_input(&project, &stage, "main.c", "rust-analyzer"),
            Err(SnapshotError::InvalidName)
        ));
        let _ = fs::remove_dir_all(&root);
    }
}
