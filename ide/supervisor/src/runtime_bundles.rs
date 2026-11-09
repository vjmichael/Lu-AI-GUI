//! DEV-03.2B.2: exact-tree attestation of offline, installer-provisioned
//! non-Rust runtime bundles. No new worker launch or caller-supplied commands.
//! Only the native host can supply an expected *approved build-time* pin.
use crate::SupervisorError;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

const MAX_RUNTIME_FILES: usize = 40_000;
const MAX_RUNTIME_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_SINGLE_FILE: u64 = 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_PATH_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProfile {
    PythonPytest,
    NodeTests,
    CmakeCTest,
    UvSyncOffline,
}
impl RuntimeProfile {
    pub fn for_id(id: &str) -> Option<Self> {
        match id {
            "python_pytest" => Some(Self::PythonPytest),
            "js_tests" => Some(Self::NodeTests),
            "cpp_cmake_build" => Some(Self::CmakeCTest),
            "uv_sync_offline" if cfg!(feature = "package-uv-worker") =>
                Some(Self::UvSyncOffline),
            _ => None,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::PythonPytest => "python_pytest",
            Self::NodeTests => "js_tests",
            Self::CmakeCTest => "cpp_cmake_build",
            Self::UvSyncOffline => "uv_sync_offline",
        }
    }
    /// Paths are fixed at build time. A candidate tree that lacks these
    /// prerequisites is denied even when its hash is syntactically valid.
    pub fn required(self, windows: bool) -> &'static [&'static str] {
        match (self, windows) {
            (Self::UvSyncOffline, true) => &[
                "bin/uv.exe", "bin/python.exe", "LICENSES/uv.txt",
                "LICENSES/python.txt",
            ],
            (Self::UvSyncOffline, false) => &[
                "bin/uv", "bin/python3", "LICENSES/uv.txt",
                "LICENSES/python.txt",
            ],
            (Self::PythonPytest, true) => &[
                "bin/python.exe",
                "Lib/site-packages/pytest/__init__.py",
                "LICENSES/python.txt",
                "LICENSES/pytest.txt",
            ],
            (Self::PythonPytest, false) => &[
                "bin/python3",
                "lib/python3.13/site-packages/pytest/__init__.py",
                "LICENSES/python.txt",
                "LICENSES/pytest.txt",
            ],
            (Self::NodeTests, true) => &["bin/node.exe", "LICENSES/node.txt"],
            (Self::NodeTests, false) => &["bin/node", "LICENSES/node.txt"],
            (Self::CmakeCTest, true) => &[
                "bin/cmake.exe",
                "bin/ctest.exe",
                "bin/ninja.exe",
                "bin/clang.exe",
                "bin/clang++.exe",
                "LICENSES/cmake.txt",
                "LICENSES/ninja.txt",
                "LICENSES/llvm.txt",
                "drivers/dev03-cmake.cmake",
            ],
            (Self::CmakeCTest, false) => &[
                "bin/cmake",
                "bin/ctest",
                "bin/ninja",
                "bin/clang",
                "bin/clang++",
                "LICENSES/cmake.txt",
                "LICENSES/ninja.txt",
                "LICENSES/llvm.txt",
                "drivers/dev03-cmake.cmake",
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedRuntimeBundle {
    pub profile: RuntimeProfile,
    pub root: PathBuf,
    pub tree_sha256: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

/// Native-only staging identity; does NOT grant process authority.
/// The OS executor must copy/ro-bind the tree under its own custody and
/// independently call this again immediately before grant consumption.
pub fn verify_runtime_bundle(
    profile_id: &str,
    root: &Path,
    expected_sha256: &str,
) -> Result<VerifiedRuntimeBundle, SupervisorError> {
    let profile = RuntimeProfile::for_id(profile_id).ok_or(SupervisorError::InvalidPlan)?;
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || expected_sha256.bytes().all(|b| b == b'0')
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let root_meta = fs::symlink_metadata(root).map_err(|_| denied())?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() || untrusted_metadata(&root_meta) {
        return Err(denied());
    }
    let canonical = fs::canonicalize(root).map_err(|_| denied())?;
    // On Windows canonical paths legitimately acquire the \\?\ device prefix.
    // Native registration still rejects reparse points at every traversed
    // child. On Unix, no alias to a different root-owned path is permitted.
    #[cfg(unix)]
    if canonical != root {
        return Err(denied());
    }
    let mut files = Vec::new();
    let mut count = 0usize;
    let mut bytes = 0u64;
    scan(
        &canonical, &canonical, 0, &mut files, &mut count, &mut bytes,
    )?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for needed in profile.required(cfg!(windows)) {
        if files
            .binary_search_by(|item| item.0.as_str().cmp(needed))
            .is_err()
        {
            return Err(denied());
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(b"lu-ide-runtime-tree-v1\0");
    for (relative, size, digest) in &files {
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(size.to_le_bytes());
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    Ok(VerifiedRuntimeBundle {
        profile,
        root: canonical,
        tree_sha256: actual,
        file_count: count,
        total_bytes: bytes,
    })
}

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
#[cfg(windows)]
fn untrusted_metadata(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(unix)]
fn untrusted_metadata(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // Immutable resource trees installed by a privileged package manager.
    // The platform's AppContainer path has its own ACL/installer pin checks.
    // Unit tests run in an unprivileged ephemeral fixture. Production code
    // accepts root-owned installer resources ONLY; no user-owned fallback.
    let owner_ok = if cfg!(test) {
        meta.uid() == 0 || meta.uid() == unsafe { libc::geteuid() }
    } else {
        meta.uid() == 0
    };
    !owner_ok || meta.mode() & 0o0022 != 0 || meta.nlink() == 0
}
#[cfg(not(any(windows, unix)))]
fn untrusted_metadata(_: &fs::Metadata) -> bool {
    true
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':' | '/' | '|'))
        && !name.ends_with('.')
        && !name.ends_with(' ')
}

fn scan(
    root: &Path,
    dir: &Path,
    depth: usize,
    entries: &mut Vec<(String, u64, String)>,
    count: &mut usize,
    total: &mut u64,
) -> Result<(), SupervisorError> {
    if depth > MAX_DEPTH {
        return Err(denied());
    }
    let mut children = fs::read_dir(dir)
        .map_err(|_| denied())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| denied())?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let name = child.file_name();
        let name = name.to_str().ok_or_else(denied)?;
        if !safe_name(name) {
            return Err(denied());
        }
        let path = child.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| denied())?;
        if meta.file_type().is_symlink() || untrusted_metadata(&meta) {
            return Err(denied());
        }
        if meta.is_dir() {
            scan(root, &path, depth + 1, entries, count, total)?;
            continue;
        }
        if !meta.is_file() || meta.len() > MAX_SINGLE_FILE {
            return Err(denied());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() != 1 {
                return Err(denied());
            }
        }
        *count = count.checked_add(1).ok_or_else(denied)?;
        *total = total.checked_add(meta.len()).ok_or_else(denied)?;
        if *count > MAX_RUNTIME_FILES || *total > MAX_RUNTIME_BYTES {
            return Err(denied());
        }
        let rel = path.strip_prefix(root).map_err(|_| denied())?;
        let rel = rel
            .iter()
            .map(|component| component.to_str().ok_or_else(denied))
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        if rel.as_bytes().len() > MAX_PATH_BYTES {
            return Err(denied());
        }
        let mut file = File::open(&path).map_err(|_| denied())?;
        let observed = file.metadata().map_err(|_| denied())?;
        if !observed.is_file() || observed.len() != meta.len() {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        let mut sha = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        let mut size = 0u64;
        loop {
            let n = file.read(&mut buf).map_err(|_| denied())?;
            if n == 0 {
                break;
            }
            size = size.checked_add(n as u64).ok_or_else(denied)?;
            if size > meta.len() {
                return Err(SupervisorError::WrongProjectOrSource);
            }
            sha.update(&buf[..n]);
        }
        let after = fs::symlink_metadata(&path).map_err(|_| denied())?;
        if size != meta.len()
            || after.len() != meta.len()
            || !after.is_file()
            || untrusted_metadata(&after)
        {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        entries.push((rel, size, format!("{:x}", sha.finalize())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn fixture() -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base =
            std::env::temp_dir().join(format!("dev03-runtime-{}-{tick}", std::process::id()));
        fs::create_dir_all(base.join("bin")).unwrap();
        fs::create_dir(base.join("LICENSES")).unwrap();
        fs::write(
            base.join("bin/node"),
            b"fixed fake Node; never executable in tests",
        )
        .unwrap();
        fs::write(base.join("LICENSES/node.txt"), b"fixture license").unwrap();
        base
    }
    fn digest(root: &Path) -> String {
        let mut files = Vec::new();
        let (mut count, mut total) = (0usize, 0u64);
        scan(root, root, 0, &mut files, &mut count, &mut total).unwrap();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        let mut sha = Sha256::new();
        sha.update(b"lu-ide-runtime-tree-v1\0");
        for (name, size, digest) in files {
            sha.update(name.as_bytes());
            sha.update([0]);
            sha.update(size.to_le_bytes());
            sha.update(digest.as_bytes());
            sha.update([0]);
        }
        format!("{:x}", sha.finalize())
    }
    #[cfg(unix)]
    #[test]
    fn node_bundle_exact_pin_and_post_modify_refusal() {
        let root = fixture();
        let sha = digest(&root);
        let observed = verify_runtime_bundle("js_tests", &root, &sha).unwrap();
        assert_eq!(observed.tree_sha256, sha);
        assert_eq!(observed.file_count, 2);
        assert_eq!(
            observed.total_bytes,
            b"fixed fake Node; never executable in tests".len() as u64
                + b"fixture license".len() as u64
        );
        assert_eq!(
            verify_runtime_bundle("js_tests", &root, &"0".repeat(64)).unwrap_err(),
            SupervisorError::WrongProjectOrSource
        );
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(root.join("bin/node"))
            .unwrap();
        file.write_all(b"changed").unwrap();
        assert_eq!(
            verify_runtime_bundle("js_tests", &root, &sha).unwrap_err(),
            SupervisorError::WrongProjectOrSource
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn reject_missing_license_and_untrusted_runtime_id() {
        let root = fixture();
        let sha = digest(&root);
        assert_eq!(
            verify_runtime_bundle("npm run test", &root, &sha).unwrap_err(),
            SupervisorError::InvalidPlan
        );
        assert_eq!(
            verify_runtime_bundle("python_pytest", &root, &sha).unwrap_err(),
            SupervisorError::NoContainment
        );
        fs::remove_file(root.join("LICENSES/node.txt")).unwrap();
        let current = digest(&root);
        assert_eq!(
            verify_runtime_bundle("js_tests", &root, &current).unwrap_err(),
            SupervisorError::NoContainment
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_hardlink_and_world_writable_runtime_denied() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = fixture();
        symlink("/etc/passwd", root.join("bin/extra")).unwrap();
        let dummy = "a".repeat(64);
        assert_eq!(
            verify_runtime_bundle("js_tests", &root, &dummy).unwrap_err(),
            SupervisorError::NoContainment
        );
        fs::remove_file(root.join("bin/extra")).unwrap();
        fs::hard_link(root.join("bin/node"), root.join("bin/other")).unwrap();
        assert_eq!(
            verify_runtime_bundle("js_tests", &root, &dummy).unwrap_err(),
            SupervisorError::NoContainment
        );
        fs::remove_file(root.join("bin/other")).unwrap();
        fs::set_permissions(root.join("bin/node"), fs::Permissions::from_mode(0o666)).unwrap();
        assert_eq!(
            verify_runtime_bundle("js_tests", &root, &dummy).unwrap_err(),
            SupervisorError::NoContainment
        );
        fs::remove_dir_all(root).unwrap();
    }
}
