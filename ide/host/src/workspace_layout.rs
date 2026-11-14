//! B9: opt-in, host-private editor navigation metadata. This is NOT a draft
//! store, a source snapshot, a file-picker grant, or a startup executor.
//! Each persisted relative file was verified through the existing cap-std
//! registered source browser before the metadata was committed.
use crate::{browser::validate_relative, wire, IdeWorkspaceRegistry};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const SCHEMA: u32 = 1;
const MAX_RECORD_BYTES: u64 = 96 * 1024;
const MAX_FILES: usize = 48;
const MAX_PATH: usize = 1024;
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RememberedSource {
    pub relative_path: String,
    pub saved_revision: String,
    pub was_dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceLayout {
    pub schema_version: u32,
    pub project_id: String,
    pub files: Vec<RememberedSource>,
    pub primary_path: Option<String>,
    pub secondary_path: Option<String>,
    pub split_open: bool,
    pub tab_window_start: usize,
}

fn checked_layout(layout: &WorkspaceLayout) -> Result<()> {
    if layout.schema_version != SCHEMA
        || !wire::valid_project_id(&layout.project_id)
        || layout.files.is_empty()
        || layout.files.len() > MAX_FILES
        || layout.tab_window_start >= MAX_FILES
    {
        return Err("B9_INVALID_LAYOUT");
    }
    let mut paths = BTreeSet::new();
    for file in &layout.files {
        if file.relative_path.len() > MAX_PATH
            || validate_relative(&file.relative_path, false).is_err()
            || file.saved_revision.len() != 64
            || !file
                .saved_revision
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !paths.insert(file.relative_path.as_str())
        {
            return Err("B9_INVALID_FILE_IDENTITY");
        }
    }
    for selected in [&layout.primary_path, &layout.secondary_path] {
        if let Some(path) = selected {
            if !paths.contains(path.as_str()) {
                return Err("B9_SELECTION_NOT_IN_LAYOUT");
            }
        }
    }
    if layout.primary_path.is_none() && (layout.secondary_path.is_some() || layout.split_open) {
        return Err("B9_INVALID_LAYOUT");
    }
    if layout.primary_path.is_some() && layout.primary_path == layout.secondary_path {
        return Err("B9_DUPLICATE_EDITOR_PANE");
    }
    let bytes = serde_json::to_vec(layout).map_err(|_| "B9_INVALID_LAYOUT")?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("B9_LAYOUT_TOO_LARGE");
    }
    Ok(())
}

fn private_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err("B9_UNSAFE_STORAGE");
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o077 != 0 {
                    return Err("B9_UNSAFE_STORAGE");
                }
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("B9_UNSAFE_STORAGE");
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700);
                builder.create(path).map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
            }
            #[cfg(not(unix))]
            fs::create_dir(path).map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
            return private_dir(path);
        }
        Err(_) => return Err("B9_STORAGE_UNAVAILABLE"),
    }
    Ok(())
}

fn safe_record(path: &Path) -> Result<Option<u64>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_RECORD_BYTES {
                return Err("B9_UNTRUSTED_RECORD");
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("B9_UNTRUSTED_RECORD");
                }
            }
            Ok(Some(meta.len()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("B9_STORAGE_UNAVAILABLE"),
    }
}

pub struct WorkspaceLayoutStore {
    root: PathBuf,
    lock: Mutex<()>,
}

impl WorkspaceLayoutStore {
    pub fn open(root: PathBuf) -> Result<Self> {
        private_dir(&root)?;
        Ok(Self {
            root,
            lock: Mutex::new(()),
        })
    }

    fn file(&self, project_id: &str) -> Result<PathBuf> {
        if !wire::valid_project_id(project_id) {
            return Err("B9_INVALID_PROJECT");
        }
        private_dir(&self.root)?;
        Ok(self.root.join(format!("{project_id}.json")))
    }

    pub fn read(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
    ) -> Result<Option<WorkspaceLayout>> {
        // A stored project ID is merely a hint: the native registered catalog
        // must still show it as actively open before we reveal navigation.
        registry
            .source_capability(project_id)
            .map_err(|_| "B9_PROJECT_NOT_OPEN")?;
        let _gate = self.lock.lock().map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        let path = self.file(project_id)?;
        if safe_record(&path)?.is_none() {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|_| "B9_STORAGE_UNAVAILABLE")?
            .take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        let layout: WorkspaceLayout =
            serde_json::from_slice(&bytes).map_err(|_| "B9_UNTRUSTED_RECORD")?;
        checked_layout(&layout).map_err(|_| "B9_UNTRUSTED_RECORD")?;
        if layout.project_id != project_id {
            return Err("B9_UNTRUSTED_RECORD");
        }
        Ok(Some(layout))
    }

    pub fn save(
        &self,
        registry: &IdeWorkspaceRegistry,
        layout: WorkspaceLayout,
    ) -> Result<WorkspaceLayout> {
        checked_layout(&layout)?;
        // Verify each saved revision using the host's existing cap-std source
        // browser. Paths never become arbitrary OS paths or new permissions.
        for file in &layout.files {
            let source = registry
                .preview_source_file(&layout.project_id, &file.relative_path)
                .map_err(|_| "B9_SOURCE_UNAVAILABLE")?;
            if source.revision != file.saved_revision {
                return Err("B9_SOURCE_CHANGED");
            }
        }
        let _gate = self.lock.lock().map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        let path = self.file(&layout.project_id)?;
        let bytes = serde_json::to_vec(&layout).map_err(|_| "B9_INVALID_LAYOUT")?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err("B9_LAYOUT_TOO_LARGE");
        }
        safe_record(&path)?;
        let mut random = OsRng;
        let nonce = random.next_u64();
        let temp = self
            .root
            .join(format!(".b9-{}-{nonce:016x}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&temp).map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        let result = (|| -> Result<()> {
            output.write_all(&bytes).map_err(|_| "B9_WRITE_FAILED")?;
            output.sync_all().map_err(|_| "B9_WRITE_FAILED")?;
            drop(output);
            // Recheck destination after writing the temp: never overwrite a
            // newly substituted link or unexpectedly large stored record.
            safe_record(&path)?;
            fs::rename(&temp, &path).map_err(|_| "B9_WRITE_FAILED")?;
            #[cfg(unix)]
            File::open(&self.root)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| "B9_WRITE_FAILED")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        Ok(layout)
    }

    fn remove_record(&self, project_id: &str) -> Result<bool> {
        let _gate = self.lock.lock().map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        let path = self.file(project_id)?;
        if safe_record(&path)?.is_none() {
            return Ok(false);
        }
        fs::remove_file(&path).map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        #[cfg(unix)]
        File::open(&self.root)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| "B9_STORAGE_UNAVAILABLE")?;
        Ok(true)
    }

    pub fn clear(&self, registry: &IdeWorkspaceRegistry, project_id: &str) -> Result<bool> {
        registry
            .source_capability(project_id)
            .map_err(|_| "B9_PROJECT_NOT_OPEN")?;
        self.remove_record(project_id)
    }

    /// Called only by the native unregister command BEFORE removing a
    /// known catalog entry. Closed registered projects are eligible too.
    /// Never touch their source folders or the separate B2 draft vault.
    pub fn purge_for_unregister(
        &self,
        registry: &IdeWorkspaceRegistry,
        project_id: &str,
    ) -> Result<bool> {
        if !wire::valid_project_id(project_id)
            || !registry
                .projects()
                .map_err(|_| "B9_PROJECT_NOT_REGISTERED")?
                .iter()
                .any(|item| item.project_id == project_id)
        {
            return Err("B9_PROJECT_NOT_REGISTERED");
        }
        self.remove_record(project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> WorkspaceLayout {
        WorkspaceLayout {
            schema_version: 1,
            project_id: format!("ide-{}", "a".repeat(32)),
            files: vec![RememberedSource {
                relative_path: "src/main.rs".into(),
                saved_revision: "a".repeat(64),
                was_dirty: true,
            }],
            primary_path: Some("src/main.rs".into()),
            secondary_path: None,
            split_open: false,
            tab_window_start: 0,
        }
    }
    #[test]
    fn bounded_relative_navigation_never_carries_source_text() {
        let layout = sample();
        checked_layout(&layout).unwrap();
        let json = serde_json::to_string(&layout).unwrap();
        assert!(!json.contains("draft"));
        assert!(!json.contains("savedText"));
        assert!(!json.contains("osPath"));
    }
    #[test]
    fn traversal_duplicates_and_foreign_selection_fail_closed() {
        let mut value = sample();
        value.files[0].relative_path = "../secret.txt".into();
        assert!(checked_layout(&value).is_err());
        value = sample();
        value.files.push(value.files[0].clone());
        assert!(checked_layout(&value).is_err());
        value = sample();
        value.secondary_path = Some("src/other.rs".into());
        assert!(checked_layout(&value).is_err());
        value = sample();
        value.files[0].saved_revision = "not a sha".into();
        assert!(checked_layout(&value).is_err());
    }
    #[test]
    fn tab_budget_and_untrusted_extra_fields_denied() {
        let mut value = sample();
        value.files = (0..49)
            .map(|i| RememberedSource {
                relative_path: format!("src/file{i}.rs"),
                saved_revision: "b".repeat(64),
                was_dirty: false,
            })
            .collect();
        assert!(checked_layout(&value).is_err());
        let mut json = serde_json::to_value(sample()).unwrap();
        json["sourceText"] = serde_json::json!("sensitive");
        assert!(serde_json::from_value::<WorkspaceLayout>(json).is_err());
    }
}
