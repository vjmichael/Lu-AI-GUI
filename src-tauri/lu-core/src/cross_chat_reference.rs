//! Data-only links to other ANSF conversations. A link is never a new USER
//! message, tool grant or instruction. The trusted host must authorize reads
//! of BOTH the active conversation and the referenced source conversation.
//! This contract is not yet wired to Lu's desktop memory pane or prompts.
use crate::ansf::AnsfStore;

/// The host's principal-aware conversation access decision, not a model
/// capability. Caller must authenticate ownership of both conversations.
pub trait HostLinkAccess {
    fn may_read_link(&self, active_conversation: &str, source_conversation: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    InvalidReference,
    AccessDenied,
    SourceMissing,
    SourceUnavailable,
    ContentTooLarge,
}

/// References source entries by ID, never by duplicating the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossChatReference {
    active_conversation: String,
    source_conversation: String,
    source_entry_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkedTrust {
    /// Content is quoted data, not a user/system/tool role or new authority.
    UntrustedReferencedData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedContextData {
    reference: CrossChatReference,
    content: String,
}

impl LinkedContextData {
    pub fn trust(&self) -> LinkedTrust {
        LinkedTrust::UntrustedReferencedData
    }
    pub fn source(&self) -> &CrossChatReference {
        &self.reference
    }
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl CrossChatReference {
    pub fn new(
        active_conversation: impl Into<String>,
        source_conversation: impl Into<String>,
        source_entry_id: i64,
    ) -> Result<Self, LinkError> {
        let active_conversation = active_conversation.into();
        let source_conversation = source_conversation.into();
        let valid = |id: &str| !id.trim().is_empty() && id.len() <= 128 && !id.contains('\0');
        if !valid(&active_conversation) || !valid(&source_conversation) || source_entry_id <= 0 {
            return Err(LinkError::InvalidReference);
        }
        Ok(Self {
            active_conversation,
            source_conversation,
            source_entry_id,
        })
    }

    pub fn active_conversation(&self) -> &str {
        &self.active_conversation
    }
    pub fn source_conversation(&self) -> &str {
        &self.source_conversation
    }
    pub fn source_entry_id(&self) -> i64 {
        self.source_entry_id
    }

    /// Returns bounded, typed data only. Linking a historical 'user' or
    /// 'system' entry does NOT preserve that role in the active conversation.
    /// Source provenance remains with the referenced ANSF entry.
    pub fn resolve_for_context(
        &self,
        source: &AnsfStore,
        host_access: &dyn HostLinkAccess,
    ) -> Result<LinkedContextData, LinkError> {
        if !host_access.may_read_link(&self.active_conversation, &self.source_conversation) {
            return Err(LinkError::AccessDenied);
        }
        let entry = source
            .read_entry(&self.source_conversation, self.source_entry_id)
            .map_err(|_| LinkError::SourceUnavailable)?
            .ok_or(LinkError::SourceMissing)?;
        if entry.content.len() > 16 * 1024 {
            return Err(LinkError::ContentTooLarge);
        }
        Ok(LinkedContextData {
            reference: self.clone(),
            content: entry.content,
        })
    }
}
