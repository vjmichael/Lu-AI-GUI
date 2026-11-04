//! Persist only an explicitly user-picked document-directory override.
//! No model text, WebView pathname or recovered chat content may change it.
//! The OS Documents directory is used when no override exists.
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::{Path, PathBuf}, sync::Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentPreferenceError {
    InvalidDirectory,
    UnsupportedPath,
    Unavailable(String),
}
pub fn ordinary_directory(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {return false;};
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {return false;}
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 { return false; }
    }
    true
}
pub struct DocumentPreferences { db: Mutex<Connection> }
impl DocumentPreferences {
    pub fn open(database: &Path) -> Result<Self, DocumentPreferenceError> {
        let connection=Connection::open(database)
            .map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?;
        connection.pragma_update(None,"journal_mode","WAL")
            .map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?;
        connection.pragma_update(None,"synchronous","FULL")
            .map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_document_preferences(
                id INTEGER PRIMARY KEY CHECK (id=1),
                override_directory TEXT
             );
             INSERT OR IGNORE INTO lu_document_preferences(id,override_directory)
             VALUES (1,NULL);"
        ).map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?;
        Ok(Self{db:Mutex::new(connection)})
    }
    fn override_path(&self)->Result<Option<PathBuf>,DocumentPreferenceError>{
        let db=self.db.lock()
            .map_err(|_|DocumentPreferenceError::Unavailable("document preferences lock poisoned".into()))?;
        let raw:Option<String>=db.query_row(
            "SELECT override_directory FROM lu_document_preferences WHERE id=1",
            [], |r|r.get(0)
        ).optional().map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?
            .flatten();
        Ok(raw.map(PathBuf::from))
    }
    /// The operating system supplies the initial Documents location, respecting
    /// Windows Known Folders and Linux XDG user directories when available.
    /// Failure is explicit: no silent fallback to app data or Downloads.
    pub fn selected(&self, os_documents: &Path)->Result<PathBuf,DocumentPreferenceError>{
        let chosen=self.override_path()?.unwrap_or_else(||os_documents.to_path_buf());
        if !ordinary_directory(&chosen) {
            return Err(DocumentPreferenceError::InvalidDirectory);
        }
        chosen.canonicalize().map_err(|_|DocumentPreferenceError::InvalidDirectory)
    }
    /// The host invokes this ONLY after the user selects a folder in a native
    /// OS folder dialog. This function is not a Tauri/WebView setter.
    pub fn set_from_native_picker(&self, selected:&Path)->Result<PathBuf,DocumentPreferenceError>{
        if !ordinary_directory(selected) {return Err(DocumentPreferenceError::InvalidDirectory);}
        let canonical=selected.canonicalize()
            .map_err(|_|DocumentPreferenceError::InvalidDirectory)?;
        let text=canonical.to_str().ok_or(DocumentPreferenceError::UnsupportedPath)?;
        let db=self.db.lock()
            .map_err(|_|DocumentPreferenceError::Unavailable("document preferences lock poisoned".into()))?;
        db.execute("UPDATE lu_document_preferences SET override_directory=?1 WHERE id=1",
            params![text]).map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?;
        Ok(canonical)
    }
    pub fn reset_to_system_documents(&self, os_documents:&Path)
        ->Result<PathBuf,DocumentPreferenceError>{
        if !ordinary_directory(os_documents) {return Err(DocumentPreferenceError::InvalidDirectory);}
        let canonical=os_documents.canonicalize()
            .map_err(|_|DocumentPreferenceError::InvalidDirectory)?;
        let db=self.db.lock()
            .map_err(|_|DocumentPreferenceError::Unavailable("document preferences lock poisoned".into()))?;
        db.execute("UPDATE lu_document_preferences SET override_directory=NULL WHERE id=1",[])
            .map_err(|e|DocumentPreferenceError::Unavailable(e.to_string()))?;
        Ok(canonical)
    }
    pub fn is_custom(&self)->Result<bool,DocumentPreferenceError>{
        Ok(self.override_path()?.is_some())
    }
}

/// Validate a durable, independently recorded created-file link. The UI
/// supplies only a proposal ID and chat ID; never a pathname.
pub fn verified_created_folder(
    approval: &crate::write_approval_ledger::WriteDecisionRecord,
    effect: &crate::journal::EffectRecord,
) -> Result<PathBuf, DocumentPreferenceError> {
    use crate::{journal::EffectState, write_approval_ledger::WriteDecision};
    if approval.decision != WriteDecision::Approved ||
       approval.effect_call_id.as_deref() != Some(effect.call_id.as_str()) ||
       effect.tool_id != crate::native_create_new::TOOL_ID ||
       effect.state != EffectState::Applied {
        return Err(DocumentPreferenceError::InvalidDirectory);
    }
    let selected=Path::new(approval.selected_target.as_deref()
        .ok_or(DocumentPreferenceError::InvalidDirectory)?);
    if selected.file_name().and_then(|name|name.to_str()) != Some(&approval.file_name) {
        return Err(DocumentPreferenceError::InvalidDirectory);
    }
    let parent=selected.parent().ok_or(DocumentPreferenceError::InvalidDirectory)?;
    if !ordinary_directory(parent) {
        return Err(DocumentPreferenceError::InvalidDirectory);
    }
    Ok(parent.to_path_buf())
}
