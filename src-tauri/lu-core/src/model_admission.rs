//! Host-owned model admission and single-model lifecycle for Lu inference P2.
//! No inference is claimed: a verified backend must attach in a later phase.
//! Callers supply a host-approved model directory, not an arbitrary model path.
use crate::gguf_preflight::{inspect_gguf_header, GgufHeader, GgufPreflightError};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug)]
pub enum ModelAdmissionError {
    Io(std::io::Error),
    OutsideApprovedDirectory,
    NotRegularFile,
    InvalidName,
    InvalidGguf(GgufPreflightError),
    AlreadyAdmitted,
    NoModelAdmitted,
    ChangedSinceAdmission,
}
impl From<std::io::Error> for ModelAdmissionError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

#[derive(Debug, Clone)]
pub struct AdmittedModel {
    canonical_path: PathBuf,
    header: GgufHeader,
    modified: Option<SystemTime>,
}
impl AdmittedModel {
    pub fn path(&self) -> &Path {
        &self.canonical_path
    }
    pub fn header(&self) -> &GgufHeader {
        &self.header
    }
    /// Recheck before backend open. This detects size/timestamp changes; it does
    /// not prevent replacement races. The backend must open a pinned handle
    /// and validate identity or consume an immutable verified model copy.
    pub fn recheck(&self) -> Result<(), ModelAdmissionError> {
        let meta = fs::symlink_metadata(&self.canonical_path)?;
        if !meta.file_type().is_file()
            || meta.len() != self.header.file_bytes
            || meta.modified().ok() != self.modified
        {
            return Err(ModelAdmissionError::ChangedSinceAdmission);
        }
        let fresh =
            inspect_gguf_header(&self.canonical_path).map_err(ModelAdmissionError::InvalidGguf)?;
        if fresh != self.header {
            return Err(ModelAdmissionError::ChangedSinceAdmission);
        }
        Ok(())
    }
}
/// A host-created manager holds one selected model. It does not hand a model
/// produced path to external tools or execute model-controlled commands.
#[derive(Default)]
pub struct ModelAdmissionManager {
    current: Option<AdmittedModel>,
}
impl ModelAdmissionManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn current(&self) -> Option<&AdmittedModel> {
        self.current.as_ref()
    }
    pub fn unload(&mut self) -> bool {
        self.current.take().is_some()
    }
    pub fn admit(
        &mut self,
        approved_directory: &Path,
        filename: &str,
    ) -> Result<&AdmittedModel, ModelAdmissionError> {
        if self.current.is_some() {
            return Err(ModelAdmissionError::AlreadyAdmitted);
        }
        if filename.is_empty()
            || filename == "."
            || filename == ".."
            || Path::new(filename).file_name().and_then(|s| s.to_str()) != Some(filename)
            || !filename.to_ascii_lowercase().ends_with(".gguf")
            || filename.contains('\\')
            || filename.contains('/')
            || filename.contains(':')
        {
            return Err(ModelAdmissionError::InvalidName);
        }
        let root = approved_directory.canonicalize()?;
        if !fs::symlink_metadata(&root)?.file_type().is_dir() {
            return Err(ModelAdmissionError::OutsideApprovedDirectory);
        }
        let target = root.join(filename);
        // Reject symbolic links and Windows reparse-point redirects at the
        // model entry. Parent is canonicalized and the filename has one part.
        if !fs::symlink_metadata(&target)?.file_type().is_file() {
            return Err(ModelAdmissionError::NotRegularFile);
        }
        let canonical_path = target.canonicalize()?;
        if canonical_path.parent() != Some(root.as_path()) {
            return Err(ModelAdmissionError::OutsideApprovedDirectory);
        }
        let meta = fs::symlink_metadata(&canonical_path)?;
        if !meta.file_type().is_file() {
            return Err(ModelAdmissionError::NotRegularFile);
        }
        let header =
            inspect_gguf_header(&canonical_path).map_err(ModelAdmissionError::InvalidGguf)?;
        self.current = Some(AdmittedModel {
            canonical_path,
            header,
            modified: meta.modified().ok(),
        });
        Ok(self.current.as_ref().expect("model was just admitted"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lu-admission-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let mut f = fs::File::create(dir.join("tiny.gguf")).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        dir
    }
    #[test]
    fn admits_only_host_scoped_file_and_supports_unload() {
        let dir = fixture();
        let mut m = ModelAdmissionManager::new();
        assert!(m.admit(&dir, "../tiny.gguf").is_err());
        assert!(m.admit(&dir, "tiny.gguf").is_ok());
        assert!(matches!(
            m.admit(&dir, "tiny.gguf"),
            Err(ModelAdmissionError::AlreadyAdmitted)
        ));
        assert!(m.current().unwrap().recheck().is_ok());
        assert!(m.unload());
        assert!(!m.unload());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_model_mutation_after_admission() {
        let dir = fixture();
        let mut m = ModelAdmissionManager::new();
        m.admit(&dir, "tiny.gguf").unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(dir.join("tiny.gguf"))
            .unwrap()
            .write_all(b"changed")
            .unwrap();
        assert!(matches!(
            m.current().unwrap().recheck(),
            Err(ModelAdmissionError::ChangedSinceAdmission)
        ));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_bad_names() {
        let dir = fixture();
        let mut m = ModelAdmissionManager::new();
        for name in ["", "..", "../tiny.gguf", "other.bin", "sub\\tiny.gguf"] {
            assert!(m.admit(&dir, name).is_err());
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
