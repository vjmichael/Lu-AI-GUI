//! Inert, bounded write proposals from completed model turns. The model
//! never selects the destination directory or issues an effect grant.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::Mutex};

pub const MAX_WRITE_PREVIEW_BYTES: usize = 16 * 1024;
const MAX_PENDING: usize = 8;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProposedWrite {
    pub kind: String,
    pub file_name: String,
    pub purpose: String,
    pub content: String,
}
impl ProposedWrite {
    pub fn digest(&self) -> String {
        format!("{:x}", Sha256::digest(self.content.as_bytes()))
    }
    pub fn bytes(&self) -> usize { self.content.len() }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteProposalError {
    Invalid, Full, Duplicate, Unknown, WrongConversation, NotSelected, Unavailable,
}
#[derive(Debug, Clone)]
struct PendingWrite {
    chat_id: String,
    generation_id: String,
    proposal: ProposedWrite,
    directory: Option<PathBuf>,
}
#[derive(Default)]
pub struct WriteProposalQueue { pending: Mutex<HashMap<String, PendingWrite>> }

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 &&
    s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn valid_file(s: &str) -> bool {
    !s.is_empty() && s.len() <= 255 && s != "." && s != ".."
    && !s.starts_with(".lu-stage-")
    && !s.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
}
impl WriteProposalQueue {
    pub fn stage(
        &self, id: &str, chat: &str, generation: &str, raw: &str
    ) -> Result<ProposedWrite, WriteProposalError> {
        if !valid_id(id) || !valid_id(chat) || !valid_id(generation)
            || raw.len() > MAX_WRITE_PREVIEW_BYTES + 4096 {
            return Err(WriteProposalError::Invalid);
        }
        let p: ProposedWrite = serde_json::from_str(raw)
            .map_err(|_| WriteProposalError::Invalid)?;
        if p.kind != "propose_create_file" || !valid_file(&p.file_name)
            || p.content.len() > MAX_WRITE_PREVIEW_BYTES || p.purpose.trim().is_empty()
            || p.purpose.len() > 256 || p.purpose.chars().any(char::is_control) {
            return Err(WriteProposalError::Invalid);
        }
        // The model proposes both filename and UTF-8 bytes. The host, not
        // the extension alone, validates supported structured text.
        lu_core::write_format::identify_text_format(&p.file_name, &p.content)
            .map_err(|_| WriteProposalError::Invalid)?;
        let mut entries = self.pending.lock().map_err(|_| WriteProposalError::Unavailable)?;
        if entries.contains_key(id) { return Err(WriteProposalError::Duplicate); }
        if entries.len() >= MAX_PENDING { return Err(WriteProposalError::Full); }
        entries.insert(id.into(), PendingWrite {
            chat_id: chat.into(), generation_id: generation.into(),
            proposal: p.clone(), directory: None,
        });
        Ok(p)
    }
    pub fn choose_directory(
        &self, id: &str, chat: &str, path: &Path
    ) -> Result<(String, String, ProposedWrite), WriteProposalError> {
        let metadata = fs::symlink_metadata(path).map_err(|_| WriteProposalError::Invalid)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(WriteProposalError::Invalid);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(WriteProposalError::Invalid);
            }
        }
        let canonical = path.canonicalize().map_err(|_| WriteProposalError::Invalid)?;
        let mut entries = self.pending.lock().map_err(|_| WriteProposalError::Unavailable)?;
        let entry = entries.get_mut(id).ok_or(WriteProposalError::Unknown)?;
        if entry.chat_id != chat { return Err(WriteProposalError::WrongConversation); }
        entry.directory = Some(canonical.clone());
        Ok((canonical.join(&entry.proposal.file_name).to_string_lossy().into_owned(),
            entry.generation_id.clone(), entry.proposal.clone()))
    }
    /// Host-only immutable snapshot used to construct the final native OS
    /// confirmation dialog before any grant is opened or proposal consumed.
    pub fn review_details(
        &self, id: &str, chat: &str
    ) -> Result<(String, PathBuf, ProposedWrite), WriteProposalError> {
        let entries = self.pending.lock().map_err(|_| WriteProposalError::Unavailable)?;
        let entry = entries.get(id).ok_or(WriteProposalError::Unknown)?;
        if entry.chat_id != chat { return Err(WriteProposalError::WrongConversation); }
        let directory = entry.directory.clone().ok_or(WriteProposalError::NotSelected)?;
        Ok((entry.generation_id.clone(), directory, entry.proposal.clone()))
    }

    /// Take exactly once only after host-bound directory selection. The
    /// ledger/effect journal must still admit the approval independently.
    pub fn take_confirmed(
        &self, id: &str, chat: &str
    ) -> Result<(String, PathBuf, ProposedWrite), WriteProposalError> {
        let mut entries = self.pending.lock().map_err(|_| WriteProposalError::Unavailable)?;
        let entry = entries.get(id).ok_or(WriteProposalError::Unknown)?;
        if entry.chat_id != chat { return Err(WriteProposalError::WrongConversation); }
        if entry.directory.is_none() { return Err(WriteProposalError::NotSelected); }
        let taken = entries.remove(id).ok_or(WriteProposalError::Unknown)?;
        Ok((taken.generation_id, taken.directory.unwrap(), taken.proposal))
    }
    pub fn dismiss(&self, id: &str, chat: &str) -> Result<String, WriteProposalError> {
        let mut entries = self.pending.lock().map_err(|_| WriteProposalError::Unavailable)?;
        let entry = entries.get(id).ok_or(WriteProposalError::Unknown)?;
        if entry.chat_id != chat { return Err(WriteProposalError::WrongConversation); }
        Ok(entries.remove(id).unwrap().generation_id)
    }
}
