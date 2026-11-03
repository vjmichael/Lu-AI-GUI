//! Model-proposed read intent is untrusted data, never an authorization grant.
//!
//! The approval path requires the trusted host to bind a model turn and then
//! obtain an explicit OS file-picker selection. An ambiguous, replayed or
//! mismatched proposal is denied before a native tool can be invoked.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::Mutex,
};

const MAX_PENDING: usize = 16;
const MAX_PROPOSAL_BYTES: usize = 2048;
const MAX_INSPECT_BYTES: u32 = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProposedRead {
    pub kind: String,
    pub file_name: String,
    pub purpose: String,
    pub max_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalError {
    Invalid,
    Full,
    Duplicate,
    Unknown,
    WrongConversation,
    WrongOrigin,
    TargetMismatch,
    Unavailable,
}

#[derive(Debug, Clone)]
struct Pending {
    conversation_id: String,
    generation_id: String,
    proposal: ProposedRead,
}

#[derive(Default)]
pub struct ReadApprovalQueue {
    pending: Mutex<HashMap<String, Pending>>,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "." && name != ".."
        && !name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

impl ReadApprovalQueue {
    /// Stage only the exact JSON tool proposal from a verified assistant turn.
    /// The host MUST verify that `generation_id` is a completed/owned model
    /// reply for `conversation_id`. This function does not make that claim.
    pub fn stage(
        &self,
        proposal_id: &str,
        conversation_id: &str,
        generation_id: &str,
        raw: &str,
    ) -> Result<ProposedRead, ProposalError> {
        if !valid_id(proposal_id)
            || !valid_id(conversation_id)
            || !valid_id(generation_id)
            || raw.len() > MAX_PROPOSAL_BYTES
        {
            return Err(ProposalError::Invalid);
        }
        let proposal: ProposedRead =
            serde_json::from_str(raw).map_err(|_| ProposalError::Invalid)?;
        if proposal.kind != "propose_read_file"
            || !valid_name(&proposal.file_name)
            || proposal.purpose.trim().is_empty()
            || proposal.purpose.len() > 256
            || proposal.purpose.chars().any(char::is_control)
            || proposal.max_bytes == 0
            || proposal.max_bytes > MAX_INSPECT_BYTES
        {
            return Err(ProposalError::Invalid);
        }
        let mut guard = self.pending.lock().map_err(|_| ProposalError::Unavailable)?;
        if guard.contains_key(proposal_id) {
            return Err(ProposalError::Duplicate);
        }
        if guard.len() >= MAX_PENDING {
            return Err(ProposalError::Full);
        }
        guard.insert(
            proposal_id.to_string(),
            Pending {
                conversation_id: conversation_id.to_string(),
                generation_id: generation_id.to_string(),
                proposal: proposal.clone(),
            },
        );
        Ok(proposal)
    }

    /// This consumes the pending proposal exactly once, INCLUDING a denied
    /// approval. A new request must be staged to retry a cancelled/mismatched
    /// selection. A native OS dialog (not the model/WebView) chooses `selected`.
    ///
    /// File identity must be revalidated at the actual read boundary; the
    /// returned path is not a durable handle and is not itself a capability.
    pub fn approve_selected(
        &self,
        proposal_id: &str,
        conversation_id: &str,
        generation_id: &str,
        selected: &Path,
    ) -> Result<ProposedRead, ProposalError> {
        let entry = self
            .pending
            .lock()
            .map_err(|_| ProposalError::Unavailable)?
            .remove(proposal_id)
            .ok_or(ProposalError::Unknown)?;
        if entry.conversation_id != conversation_id {
            return Err(ProposalError::WrongConversation);
        }
        if entry.generation_id != generation_id {
            return Err(ProposalError::WrongOrigin);
        }
        let name = selected.file_name().and_then(|s| s.to_str())
            .ok_or(ProposalError::TargetMismatch)?;
        if name != entry.proposal.file_name {
            return Err(ProposalError::TargetMismatch);
        }
        let metadata = fs::symlink_metadata(selected).map_err(|_| ProposalError::TargetMismatch)?;
        if !metadata.file_type().is_file() {
            return Err(ProposalError::TargetMismatch);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(ProposalError::TargetMismatch);
            }
        }
        Ok(entry.proposal)
    }

    pub fn dismiss(&self, proposal_id: &str) -> Result<(), ProposalError> {
        self.pending
            .lock()
            .map_err(|_| ProposalError::Unavailable)?
            .remove(proposal_id)
            .ok_or(ProposalError::Unknown)?;
        Ok(())
    }

    pub fn clear_conversation(&self, chat: &str) -> Result<usize, ProposalError> {
        let mut entries = self.pending.lock().map_err(|_| ProposalError::Unavailable)?;
        let before = entries.len();
        entries.retain(|_, entry| entry.conversation_id != chat);
        Ok(before - entries.len())
    }
}
