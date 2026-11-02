//! Host-authorized companion projector admission for multimodal GGUF models.
//! An mmproj is an optional *separate* GGUF file; its presence never implies
//! that the selected inference backend supports vision or audio.
use crate::gguf_preflight::{inspect_gguf_header, GgufHeader, GgufPreflightError};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
#[derive(Debug)]
pub enum ProjectorError {
    InvalidName,
    NotRegularFile,
    OutsideApprovedDirectory,
    InvalidHeader(GgufPreflightError),
    Io(std::io::Error),
    Changed,
}
#[derive(Debug, Clone)]
pub struct AdmittedProjector {
    path: PathBuf,
    header: GgufHeader,
    modified: Option<SystemTime>,
}
impl AdmittedProjector {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn header(&self) -> &GgufHeader {
        &self.header
    }
    pub fn recheck(&self) -> Result<(), ProjectorError> {
        let meta = fs::symlink_metadata(&self.path).map_err(ProjectorError::Io)?;
        if !meta.file_type().is_file()
            || meta.len() != self.header.file_bytes
            || meta.modified().ok() != self.modified
        {
            return Err(ProjectorError::Changed);
        }
        let h = inspect_gguf_header(&self.path).map_err(ProjectorError::InvalidHeader)?;
        if h != self.header {
            return Err(ProjectorError::Changed);
        }
        Ok(())
    }
}
/// Caller must choose the projector explicitly; do not auto-guess based on
/// neighboring files. Projector/model architecture compatibility is established
/// later by the actual backend, not by the GGUF magic alone.
pub fn admit_projector(root: &Path, filename: &str) -> Result<AdmittedProjector, ProjectorError> {
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || Path::new(filename).file_name().and_then(|n| n.to_str()) != Some(filename)
        || !filename.to_ascii_lowercase().ends_with(".gguf")
    {
        return Err(ProjectorError::InvalidName);
    }
    // Windows separators must be rejected even if tested on a non-Windows host.
    if filename.contains('\\') || filename.contains('/') || filename.contains(':') {
        return Err(ProjectorError::InvalidName);
    }
    let root = root.canonicalize().map_err(ProjectorError::Io)?;
    if !fs::symlink_metadata(&root)
        .map_err(ProjectorError::Io)?
        .file_type()
        .is_dir()
    {
        return Err(ProjectorError::OutsideApprovedDirectory);
    }
    let path = root.join(filename);
    if !fs::symlink_metadata(&path)
        .map_err(ProjectorError::Io)?
        .file_type()
        .is_file()
    {
        return Err(ProjectorError::NotRegularFile);
    }
    let path = path.canonicalize().map_err(ProjectorError::Io)?;
    if path.parent() != Some(root.as_path()) {
        return Err(ProjectorError::OutsideApprovedDirectory);
    }
    let meta = fs::symlink_metadata(&path).map_err(ProjectorError::Io)?;
    let header = inspect_gguf_header(&path).map_err(ProjectorError::InvalidHeader)?;
    Ok(AdmittedProjector {
        path,
        header,
        modified: meta.modified().ok(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn projector_requires_explicit_safe_file() {
        let root = std::env::temp_dir().join(format!("lu-projector-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("mmproj-model-f16.gguf");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        drop(f);
        for name in [
            "../mmproj-model-f16.gguf",
            "..\\mmproj-model-f16.gguf",
            "X:evil.gguf",
            "",
        ] {
            assert!(admit_projector(&root, name).is_err());
        }
        let p = admit_projector(&root, "mmproj-model-f16.gguf").unwrap();
        assert!(p.recheck().is_ok());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"x")
            .unwrap();
        assert!(matches!(p.recheck(), Err(ProjectorError::Changed)));
        fs::remove_dir_all(root).unwrap();
    }
}
