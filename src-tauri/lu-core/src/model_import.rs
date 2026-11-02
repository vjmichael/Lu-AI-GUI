//! Explicit local GGUF import into Lu's own model store (text-only alpha).
//!
//! The OS file picker belongs to a trusted desktop command; models and webviews
//! cannot supply the source path as an authorized filesystem tool request.
//! Bytes are staged within the approved model directory, validated before
//! publication, and published with an atomic, no-replace hard link. A failed
//! import never exposes a partial GGUF in the model catalog.
use crate::gguf_preflight::inspect_gguf_header;
use crate::projector_identity::projector_filename_for_model;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_MODEL_BYTES: u64 = 128 * 1024 * 1024 * 1024;
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    InvalidSource,
    UnsupportedProjector,
    UnsafeModelDirectory,
    AlreadyPresent,
    InvalidGguf,
    SourceChanged,
    TooLarge,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedModel {
    pub filename: String,
    pub bytes: u64,
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

fn safe_name(source: &Path, projector: bool) -> Result<&str, ImportError> {
    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(ImportError::InvalidSource)?;
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':'])
        || name.len() > 200
        || !name.to_ascii_lowercase().ends_with(".gguf")
    {
        return Err(ImportError::InvalidSource);
    }
    let is_projector = name.to_ascii_lowercase().starts_with("mmproj");
    if is_projector != projector {
        return Err(ImportError::UnsupportedProjector);
    }
    Ok(name)
}

/// `approved_root` is supplied by Lu's private runtime directory resolver,
/// not by the UI or a model. `source` must be selected in a native OS dialog.
/// Return only a filename/size; never return the user's full source path.
pub fn import_selected_gguf(
    approved_root: &Path,
    source: &Path,
) -> Result<ImportedModel, ImportError> {
    let name = safe_name(source, false)?;
    import_named_gguf(approved_root, source, name, false)
}

/// A projector can only be associated with a previously admitted host model ID.
/// The host-selected file must itself be named mmproj*.gguf; its destination
/// derives from the canonical opaque model identity, never user-provided paths.
pub fn import_selected_projector(
    approved_root: &Path,
    source: &Path,
    model_ref: &str,
) -> Result<ImportedModel, ImportError> {
    safe_name(source, true)?;
    let name = projector_filename_for_model(model_ref).map_err(|_| ImportError::InvalidSource)?;
    import_named_gguf(approved_root, source, &name, true)
}

fn import_named_gguf(
    approved_root: &Path,
    source: &Path,
    name: &str,
    projector: bool,
) -> Result<ImportedModel, ImportError> {
    let root_meta =
        fs::symlink_metadata(approved_root).map_err(|_| ImportError::UnsafeModelDirectory)?;
    if !root_meta.file_type().is_dir() || is_reparse(&root_meta) {
        return Err(ImportError::UnsafeModelDirectory);
    }
    let root = approved_root
        .canonicalize()
        .map_err(|_| ImportError::UnsafeModelDirectory)?;
    let src_meta = fs::symlink_metadata(source).map_err(|_| ImportError::InvalidSource)?;
    if !src_meta.file_type().is_file() || is_reparse(&src_meta) {
        return Err(ImportError::InvalidSource);
    }
    let mut input = File::open(source).map_err(|_| ImportError::InvalidSource)?;
    let open_meta = input.metadata().map_err(|_| ImportError::InvalidSource)?;
    if !open_meta.is_file() || is_reparse(&open_meta) {
        return Err(ImportError::InvalidSource);
    }
    let size = open_meta.len();
    if size < 16 {
        return Err(ImportError::InvalidGguf);
    }
    if size > MAX_MODEL_BYTES || (projector && size > 32 * 1024 * 1024 * 1024) {
        return Err(ImportError::TooLarge);
    }
    let target = root.join(name);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(ImportError::AlreadyPresent);
    }
    let staged = (0..8)
        .find_map(|_| {
            let tick = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_nanos();
            let serial = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(
                ".lu-import-{}-{tick}-{serial}.partial",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(out) => Some(Ok((Stage(path), out))),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => None,
                Err(_) => Some(Err(ImportError::Io)),
            }
        })
        .ok_or(ImportError::Io)??;
    let (stage, mut output) = staged;
    // Limit the transfer even if another program grows the selected file.
    let copied =
        io::copy(&mut (&mut input).take(size + 1), &mut output).map_err(|_| ImportError::Io)?;
    if copied != size {
        return Err(ImportError::SourceChanged);
    }
    output.sync_all().map_err(|_| ImportError::Io)?;
    drop(output);
    let after = input.metadata().map_err(|_| ImportError::SourceChanged)?;
    if after.len() != size || after.modified().ok() != open_meta.modified().ok() {
        return Err(ImportError::SourceChanged);
    }
    inspect_gguf_header(&stage.0).map_err(|_| ImportError::InvalidGguf)?;
    // Linking a fully fsynced staging file is atomic on same-volume local
    // filesystems and refuses to replace an existing name on Windows/Linux.
    fs::hard_link(&stage.0, &target).map_err(|e| match e.kind() {
        io::ErrorKind::AlreadyExists => ImportError::AlreadyPresent,
        _ => ImportError::Io,
    })?;
    Ok(ImportedModel {
        filename: name.to_owned(),
        bytes: size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn dirs() -> (PathBuf, PathBuf) {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("lu-model-import-{}-{tick}", std::process::id()));
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();
        (root, models)
    }
    fn valid_file(path: &Path) {
        let mut f = File::create(path).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
    }
    #[test]
    fn copies_valid_model_atomically_and_never_overwrites() {
        let (root, models) = dirs();
        let source = root.join("my-model.GGUF");
        valid_file(&source);
        let imported = import_selected_gguf(&models, &source).unwrap();
        assert_eq!(imported.filename, "my-model.GGUF");
        assert_eq!(imported.bytes, fs::metadata(&source).unwrap().len());
        assert_eq!(
            fs::read(&source).unwrap(),
            fs::read(models.join(&imported.filename)).unwrap()
        );
        assert_eq!(
            import_selected_gguf(&models, &source),
            Err(ImportError::AlreadyPresent)
        );
        assert_eq!(
            fs::read_dir(&models).unwrap().count(),
            1,
            "no partial files remain"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn never_publishes_non_gguf_or_invalid_projector_or_partial_file() {
        let (root, models) = dirs();
        let bad = root.join("garbage.gguf");
        fs::write(&bad, b"not-a-gguf-header-1234567890").unwrap();
        assert_eq!(
            import_selected_gguf(&models, &bad),
            Err(ImportError::InvalidGguf)
        );
        assert_eq!(fs::read_dir(&models).unwrap().count(), 0);
        let project = root.join("mmproj-text.gguf");
        valid_file(&project);
        assert_eq!(
            import_selected_gguf(&models, &project),
            Err(ImportError::UnsupportedProjector)
        );
        let paired =
            import_selected_projector(&models, &project, "m-00112233445566778899aabbccddeeff")
                .unwrap();
        assert_eq!(
            paired.filename,
            "mmproj-for-m-00112233445566778899aabbccddeeff.gguf"
        );
        assert_eq!(
            import_selected_projector(&models, &project, "m-00112233445566778899aabbccddeeff"),
            Err(ImportError::AlreadyPresent)
        );
        assert_eq!(
            import_selected_projector(&models, &project, "../unsafe"),
            Err(ImportError::InvalidSource)
        );
        let other = root.join("model.bin");
        valid_file(&other);
        assert_eq!(
            import_selected_gguf(&models, &other),
            Err(ImportError::InvalidSource)
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn refuses_redirected_source_and_model_directory() {
        use std::os::unix::fs::symlink;
        let (root, models) = dirs();
        let source = root.join("valid.gguf");
        valid_file(&source);
        let redirected_source = root.join("alias.gguf");
        symlink(&source, &redirected_source).unwrap();
        assert_eq!(
            import_selected_gguf(&models, &redirected_source),
            Err(ImportError::InvalidSource)
        );
        let redirected_root = root.join("redirect-model-root");
        symlink(&models, &redirected_root).unwrap();
        assert_eq!(
            import_selected_gguf(&redirected_root, &source),
            Err(ImportError::UnsafeModelDirectory)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
