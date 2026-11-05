//! R3.5: IDE-owned, in-memory review inbox for untrusted coding proposals.
//!
//! Importing, reviewing and approving a proposal NEVER writes source files,
//! grants a filesystem capability, executes commands, or connects to Lu.
//! "Approved for staging" is a review decision, NOT apply authorization.
//! Future authenticated Lu integration must have its own policy boundary.
use std::{collections::BTreeMap, sync::Mutex};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    fresh_project_id,
    proposal::{ProposalEnvelope, ProposalError},
    review::ChangeReview,
    IdeWorkspaceRegistry,
};

const MAX_INBOX_ITEMS: usize = 16;

#[derive(Debug)]
pub enum InboxError {
    Proposal(ProposalError),
    Unavailable,
    Full,
    NotFound,
    StateChanged,
    PreviewIncomplete,
}
impl InboxError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Proposal(error) => error.code(),
            Self::Unavailable => "INBOX_UNAVAILABLE",
            Self::Full => "INBOX_FULL",
            Self::NotFound => "PROPOSAL_NOT_FOUND",
            Self::StateChanged => "PROPOSAL_STATE_CHANGED",
            Self::PreviewIncomplete => "PROPOSAL_PREVIEW_INCOMPLETE",
        }
    }
}
impl From<ProposalError> for InboxError {
    fn from(error: ProposalError) -> Self {
        Self::Proposal(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    ApprovedForStaging,
    Rejected,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalSummary {
    pub inbox_id: String,
    pub task_id: String,
    pub project_id: String,
    /// Claimed identity from untrusted input, never a verified peer.
    pub claimed_producer: String,
    pub rationale: String,
    pub edit_count: usize,
    pub digest: String,
    pub status: ReviewStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalDetail {
    pub summary: ProposalSummary,
    pub reviews: Vec<ChangeReview>,
}

struct Entry {
    envelope: ProposalEnvelope,
    digest: String,
    status: ReviewStatus,
}

impl Entry {
    fn summary(&self, inbox_id: &str) -> ProposalSummary {
        ProposalSummary {
            inbox_id: inbox_id.to_owned(),
            task_id: self.envelope.task_id.clone(),
            project_id: self.envelope.project_id.clone(),
            claimed_producer: self.envelope.claimed_producer.clone(),
            rationale: self.envelope.rationale.clone(),
            edit_count: self.envelope.edits.len(),
            digest: self.digest.clone(),
            status: self.status,
        }
    }

    fn review(&self, registry: &IdeWorkspaceRegistry) -> Result<Vec<ChangeReview>, InboxError> {
        self.envelope.review(registry).map_err(Into::into)
    }
}

/// This queue belongs to the standalone native IDE process, not Lu or the
/// WebView. Entries are intentionally volatile in R3.5: no source content
/// survives app restart until encrypted, policy-governed persistence exists.
pub struct IdeProposalInbox {
    items: Mutex<BTreeMap<String, Entry>>,
}
impl Default for IdeProposalInbox {
    fn default() -> Self {
        Self {
            items: Mutex::new(BTreeMap::new()),
        }
    }
}

impl IdeProposalInbox {
    pub fn import(
        &self,
        registry: &IdeWorkspaceRegistry,
        raw_json: &str,
    ) -> Result<ProposalSummary, InboxError> {
        let envelope = ProposalEnvelope::parse(raw_json)?;
        // Validate the whole proposal against current, open project content
        // BEFORE committing it to the in-memory inbox. This has no disk effects.
        envelope.review(registry)?;
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|_| InboxError::Proposal(ProposalError::InvalidEnvelope))?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let mut items = self.items.lock().map_err(|_| InboxError::Unavailable)?;
        if items.len() >= MAX_INBOX_ITEMS {
            return Err(InboxError::Full);
        }
        let id = fresh_project_id();
        let entry = Entry {
            envelope,
            digest,
            status: ReviewStatus::Pending,
        };
        let summary = entry.summary(&id);
        items.insert(id, entry);
        Ok(summary)
    }

    pub fn list(&self) -> Result<Vec<ProposalSummary>, InboxError> {
        let items = self.items.lock().map_err(|_| InboxError::Unavailable)?;
        Ok(items.iter().map(|(id, entry)| entry.summary(id)).collect())
    }

    pub fn detail(
        &self,
        registry: &IdeWorkspaceRegistry,
        inbox_id: &str,
    ) -> Result<ProposalDetail, InboxError> {
        let items = self.items.lock().map_err(|_| InboxError::Unavailable)?;
        let entry = items.get(inbox_id).ok_or(InboxError::NotFound)?;
        let reviews = entry.review(registry)?;
        Ok(ProposalDetail {
            summary: entry.summary(inbox_id),
            reviews,
        })
    }

    /// A separately requested, digest-bound HUMAN review decision. The
    /// "approved_for_staging" state is informational and cannot apply changes.
    /// The IDE must recheck revisions on every review and later manual save.
    pub fn decide(
        &self,
        registry: &IdeWorkspaceRegistry,
        inbox_id: &str,
        expected_digest: &str,
        approve_for_staging: bool,
    ) -> Result<ProposalSummary, InboxError> {
        let mut items = self.items.lock().map_err(|_| InboxError::Unavailable)?;
        let entry = items.get_mut(inbox_id).ok_or(InboxError::NotFound)?;
        if entry.status != ReviewStatus::Pending || entry.digest != expected_digest {
            return Err(InboxError::StateChanged);
        }
        if approve_for_staging {
            let reviews = entry.review(registry)?;
            if reviews
                .iter()
                .any(|review| review.truncated || !review.counts_exact)
            {
                return Err(InboxError::PreviewIncomplete);
            }
            entry.status = ReviewStatus::ApprovedForStaging;
        } else {
            entry.status = ReviewStatus::Rejected;
        }
        Ok(entry.summary(inbox_id))
    }

    pub fn discard(&self, inbox_id: &str) -> Result<(), InboxError> {
        let mut items = self.items.lock().map_err(|_| InboxError::Unavailable)?;
        if items.remove(inbox_id).is_none() {
            return Err(InboxError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_queue_is_empty_and_does_not_confer_apply_rights() {
        let inbox = IdeProposalInbox::default();
        assert!(inbox.list().unwrap().is_empty());
        assert_eq!(
            inbox.discard("missing").unwrap_err().code(),
            "PROPOSAL_NOT_FOUND"
        );
    }
}
