//! Content-addressed staging for verified single-file native CE executables.
//! Never extract a user-supplied archive or execute from an untrusted path.
//! Publish atomically without replacing an existing executable and pin the
//! verified file's identity for subsequent prelaunch checks.
//!
//! IMPORTANT: a pinned handle and prelaunch identity check do not by
//! themselves eliminate a pathname race in a subsequent OS spawn. The Linux
//! sandbox must eventually bind/execute directly from a held immutable handle.

use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use crate::ce_signing::VerifiedCePackage;
use sha2::{Digest, Sha256};

static NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeStageError {
    InvalidRoot,
    InvalidExecutable,
    ChangedOnDisk,
    Io(String),
}

impl From<std::io::Error> for CeStageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

fn matching_file_identity(expected: &Metadata, actual: &Metadata) -> bool {
    if !expected.file_type().is_file()
        || !actual.file_type().is_file()
        || expected.len() != actual.len()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        expected.dev() == actual.dev() && expected.ino() == actual.ino()
    }
    #[cfg(not(unix))]
    {
        // External CE worker execution is disabled on Windows/macOS. This is
        // only a size check there, not a complete platform file-ID attestation.
        true
    }
}

fn private_directory(dir: &Path) -> Result<(), CeStageError> {
    let metadata = fs::symlink_metadata(dir).map_err(|_| CeStageError::InvalidRoot)?;
    if !metadata.file_type().is_dir() {
        return Err(CeStageError::InvalidRoot);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(CeStageError::InvalidRoot);
        }
    }
    Ok(())
}

/// A same-content hardlink can preserve the worker inode while an attacker
/// substitutes the directory containing it. Pin both directory inodes too.
#[cfg(unix)]
fn verify_directory_identity(path: &Path, pinned: &File) -> Result<(), CeStageError> {
    use std::os::unix::fs::MetadataExt;

    private_directory(path)?;
    let current = fs::symlink_metadata(path)?;
    let original = pinned.metadata()?;
    if !current.is_dir()
        || !original.is_dir()
        || current.dev() != original.dev()
        || current.ino() != original.ino()
        || current.uid() != original.uid()
    {
        return Err(CeStageError::ChangedOnDisk);
    }
    Ok(())
}

fn matching_worker_identity(expected: &Metadata, actual: &Metadata, dir: &Metadata) -> bool {
    if !matching_file_identity(expected, actual) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // An existing executable is not trusted merely because its bytes
        // hash correctly. Require the host directory's owner and a
        // non-writable, private, owner-executable worker.
        expected.uid() == dir.uid()
            && actual.uid() == dir.uid()
            && expected.permissions().mode() & 0o777 == 0o500
            && actual.permissions().mode() & 0o777 == 0o500
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        true
    }
}

#[derive(Clone)]
pub struct StagedCeExecutable {
    package: VerifiedCePackage,
    path: PathBuf,
    // This handle pins the *original verified inode* across runtime clones.
    // Opening the same pathname again must still resolve to that file.
    pinned: Arc<File>,
    #[cfg(unix)]
    pinned_root: Arc<File>,
    #[cfg(unix)]
    pinned_directory: Arc<File>,
}

impl StagedCeExecutable {
    pub fn package(&self) -> &VerifiedCePackage {
        &self.package
    }

    /// Private host path; do not expose as authority to models or workers.
    pub fn host_path(&self) -> &Path {
        &self.path
    }

    /// Compare the path to the held file identity before AND after bounded
    /// hashing. A newly swapped-in file with identical bytes is still denied
    /// on Unix (different dev/inode). Reject symlinks, missing files, writes,
    /// and directory substitutions. The final Linux bwrap path consumption is
    /// still a separate race until handle-based mount/exec is implemented.
    pub fn verify_on_disk(&self) -> Result<(), CeStageError> {
        let dir = self.path.parent().ok_or(CeStageError::InvalidRoot)?;
        let root = dir.parent().ok_or(CeStageError::InvalidRoot)?;
        private_directory(root)?;
        private_directory(dir)?;
        #[cfg(unix)]
        {
            verify_directory_identity(root, &self.pinned_root)?;
            verify_directory_identity(dir, &self.pinned_directory)?;
        }
        let dir_meta = fs::symlink_metadata(dir)?;
        let pinned_meta = self.pinned.metadata()?;
        if pinned_meta.len() as usize != self.package.artifact().len() {
            return Err(CeStageError::ChangedOnDisk);
        }
        let before = fs::symlink_metadata(&self.path)?;
        if !matching_worker_identity(&pinned_meta, &before, &dir_meta) {
            return Err(CeStageError::ChangedOnDisk);
        }
        let mut file = File::open(&self.path)?;
        if !matching_worker_identity(&pinned_meta, &file.metadata()?, &dir_meta) {
            return Err(CeStageError::ChangedOnDisk);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take((self.package.artifact().len() + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() != self.package.artifact().len()
            || format!("{:x}", Sha256::digest(&bytes)) != self.package.artifact_sha256()
        {
            return Err(CeStageError::ChangedOnDisk);
        }
        let after = fs::symlink_metadata(&self.path)?;
        if !matching_worker_identity(&pinned_meta, &after, &dir_meta)
            || !matching_worker_identity(&pinned_meta, &file.metadata()?, &dir_meta)
        {
            return Err(CeStageError::ChangedOnDisk);
        }
        #[cfg(unix)]
        {
            // Recheck both parents after hashing, not only at entry.
            verify_directory_identity(root, &self.pinned_root)?;
            verify_directory_identity(dir, &self.pinned_directory)?;
        }
        Ok(())
    }
}

pub struct CeExecutableStager {
    root: PathBuf,
    #[cfg(unix)]
    pinned_root: Arc<File>,
}

impl CeExecutableStager {
    /// Trusted host must pass a private, pre-created, non-symlink Lu directory.
    pub fn new(root: &Path) -> Result<Self, CeStageError> {
        private_directory(root)?;
        let root = root.canonicalize().map_err(|_| CeStageError::InvalidRoot)?;
        #[cfg(unix)]
        let pinned_root = Arc::new(File::open(&root)?);
        #[cfg(unix)]
        verify_directory_identity(&root, &pinned_root)?;
        Ok(Self {
            root,
            #[cfg(unix)]
            pinned_root,
        })
    }

    /// Only a single signed executable file is supported, not zip/tar,
    /// scripts, installers, shared libraries, or executable directories.
    pub fn stage(&self, package: VerifiedCePackage) -> Result<StagedCeExecutable, CeStageError> {
        let bytes = package.artifact();
        #[cfg(target_os = "linux")]
        if !bytes.starts_with(b"\x7fELF") {
            return Err(CeStageError::InvalidExecutable);
        }
        #[cfg(target_os = "windows")]
        if !bytes.starts_with(b"MZ") {
            return Err(CeStageError::InvalidExecutable);
        }
        #[cfg(target_os = "macos")]
        if ![
            b"\xfe\xed\xfa\xcf",
            b"\xcf\xfa\xed\xfe",
            b"\xca\xfe\xba\xbe",
        ]
        .iter()
        .any(|magic| bytes.starts_with(*magic))
        {
            return Err(CeStageError::InvalidExecutable);
        }

        // Never chmod/chown an existing directory supplied by an adversary.
        // Atomic 0700 creation avoids a world-readable pre-chmod interval.
        private_directory(&self.root)?;
        #[cfg(unix)]
        verify_directory_identity(&self.root, &self.pinned_root)?;
        let dir = self.root.join(package.artifact_sha256());
        match fs::symlink_metadata(&dir) {
            Ok(_) => private_directory(&dir)?,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    let mut builder = fs::DirBuilder::new();
                    builder.mode(0o700);
                    match builder.create(&dir) {
                        Ok(()) => (),
                        Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
                        Err(e) => return Err(e.into()),
                    }
                }
                #[cfg(not(unix))]
                match fs::create_dir(&dir) {
                    Ok(()) => (),
                    Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e.into()),
                }
                private_directory(&dir)?;
            }
            Err(e) => return Err(e.into()),
        }
        #[cfg(unix)]
        let pinned_directory = Arc::new(File::open(&dir)?);
        #[cfg(unix)]
        verify_directory_identity(&dir, &pinned_directory)?;
        let path = dir.join(if cfg!(windows) {
            "worker.exe"
        } else {
            "worker"
        });
        match fs::symlink_metadata(&path) {
            Ok(meta) if !meta.file_type().is_file() => return Err(CeStageError::ChangedOnDisk),
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                let temp = dir.join(format!(
                    ".pending-{}-{}",
                    std::process::id(),
                    NONCE.fetch_add(1, Ordering::Relaxed)
                ));
                let result = (|| -> Result<(), CeStageError> {
                    let mut options = OpenOptions::new();
                    options.write(true).create_new(true);
                    let mut file = options.open(&temp)?;
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(&temp, fs::Permissions::from_mode(0o500))?;
                    }
                    // hard_link is atomic and fails if the target exists.
                    // Unlike rename it never replaces an executable created
                    // by another thread/process while we staged our temp file.
                    match fs::hard_link(&temp, &path) {
                        Ok(()) => (),
                        Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
                        Err(e) => return Err(e.into()),
                    }
                    #[cfg(unix)]
                    File::open(&dir)?.sync_all()?;
                    Ok(())
                })();
                let _ = fs::remove_file(&temp);
                result?;
            }
            Err(e) => return Err(e.into()),
        }
        private_directory(&dir)?;
        // Opening immediately pins the inode for the entire lifetime of the
        // staged object and its clones. Every later check compares to it.
        let pinned = Arc::new(File::open(&path)?);
        let staged = StagedCeExecutable {
            package,
            path,
            pinned,
            #[cfg(unix)]
            pinned_root: Arc::clone(&self.pinned_root),
            #[cfg(unix)]
            pinned_directory,
        };
        staged.verify_on_disk()?;
        Ok(staged)
    }
}
