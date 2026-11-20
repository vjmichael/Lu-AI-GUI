//! Data-only, untrusted future coding proposal envelope (version 1).
//!
//! No model, process, network, or IDE write authorization is exposed here.
//! "claimedProducer" is a label supplied by an untrusted sender, NOT identity
//! authentication; no execution/apply API exists for an envelope.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{
    browser::{validate_relative, BrowserError, MAX_TEXT_BYTES},
    review::ChangeReview,
    wire, IdeWorkspaceRegistry,
};

const MAX_PROPOSAL_BYTES: usize = 600 * 1024;
const MAX_EDIT_FILES: usize = 4;
const MAX_TOTAL_EDIT_BYTES: usize = 512 * 1024;
const MAX_RATIONALE_BYTES: usize = 2_048;

#[derive(Debug)]
pub enum ProposalError {
    InvalidEnvelope,
    UnsupportedVersion,
    InvalidTask,
    InvalidProject,
    InvalidTarget,
    InvalidRevision,
    TooLarge,
    DuplicateTarget,
    Review(BrowserError),
}
impl ProposalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidEnvelope => "PROPOSAL_INVALID",
            Self::UnsupportedVersion => "PROPOSAL_UNSUPPORTED_VERSION",
            Self::InvalidTask => "PROPOSAL_INVALID_TASK",
            Self::InvalidProject => "PROJECT_NOT_FOUND",
            Self::InvalidTarget => "INVALID_RELATIVE_PATH",
            Self::InvalidRevision => "INVALID_FILE_REVISION",
            Self::TooLarge => "PROPOSAL_TOO_LARGE",
            Self::DuplicateTarget => "PROPOSAL_DUPLICATE_TARGET",
            Self::Review(error) => error.code(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposedEdit {
    pub relative_path: String,
    pub expected_revision: String,
    pub proposed_text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposalEnvelope {
    pub schema_version: u16,
    pub task_id: String,
    pub project_id: String,
    /// Not authentication; an adversary can freely supply this claim.
    pub claimed_producer: String,
    pub rationale: String,
    pub edits: Vec<ProposedEdit>,
}

impl ProposalEnvelope {
    /// Import a bounded JSON proposal strictly as data. Unknown fields
    /// (including "command", "approved" or "autoApply") are rejected.
    pub fn parse(json: &str) -> Result<Self, ProposalError> {
        if json.len() > MAX_PROPOSAL_BYTES {
            return Err(ProposalError::TooLarge);
        }
        let envelope: Self =
            serde_json::from_str(json).map_err(|_| ProposalError::InvalidEnvelope)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn validate(&self) -> Result<(), ProposalError> {
        if self.schema_version != 1 {
            return Err(ProposalError::UnsupportedVersion);
        }
        if self.task_id.is_empty()
            || self.task_id.len() > 64
            || !self
                .task_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || self.claimed_producer.is_empty()
            || self.claimed_producer.len() > 64
            || self.claimed_producer.chars().any(char::is_control)
        {
            return Err(ProposalError::InvalidTask);
        }
        if !wire::valid_project_id(&self.project_id) {
            return Err(ProposalError::InvalidProject);
        }
        if self.rationale.len() > MAX_RATIONALE_BYTES
            || self.edits.is_empty()
            || self.edits.len() > MAX_EDIT_FILES
        {
            return Err(ProposalError::TooLarge);
        }
        let mut total = 0usize;
        let mut unique = HashSet::new();
        for edit in &self.edits {
            if validate_relative(&edit.relative_path, false).is_err() {
                return Err(ProposalError::InvalidTarget);
            }
            if !unique.insert(edit.relative_path.as_str()) {
                return Err(ProposalError::DuplicateTarget);
            }
            if edit.expected_revision.len() != 64
                || !edit
                    .expected_revision
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(ProposalError::InvalidRevision);
            }
            if edit.proposed_text.as_bytes().contains(&0)
                || edit.proposed_text.len() as u64 > MAX_TEXT_BYTES
            {
                return Err(ProposalError::TooLarge);
            }
            total = total.saturating_add(edit.proposed_text.len());
            if total > MAX_TOTAL_EDIT_BYTES {
                return Err(ProposalError::TooLarge);
            }
        }
        Ok(())
    }

    /// Review every edit against a current, open project. This is intentionally
    /// read-only and may fail after an earlier preview was created if the disk
    /// changed; even a successful preview confers NO permission to write.
    pub fn review(
        &self,
        registry: &IdeWorkspaceRegistry,
    ) -> Result<Vec<ChangeReview>, ProposalError> {
        self.validate()?;
        self.edits
            .iter()
            .map(|edit| {
                registry
                    .review_source_change(
                        &self.project_id,
                        &edit.relative_path,
                        &edit.expected_revision,
                        &edit.proposed_text,
                    )
                    .map_err(ProposalError::Review)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProposalEnvelope {
        ProposalEnvelope {
            schema_version: 1,
            task_id: "local-draft-1".into(),
            project_id: "ide-0123456789abcdef0123456789abcdef".into(),
            claimed_producer: "Lu".into(),
            rationale: "Explain a possible improvement, but never auto-apply".into(),
            edits: vec![ProposedEdit {
                relative_path: "src/main.rs".into(),
                expected_revision: "a".repeat(64),
                proposed_text: "fn main() {}\n".into(),
            }],
        }
    }

    #[test]
    fn valid_envelope_is_data_not_an_authorization() {
        let json = serde_json::to_string(&sample()).unwrap();
        let parsed = ProposalEnvelope::parse(&json).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.claimed_producer, "Lu");
    }

    #[test]
    fn reject_commands_and_self_approval_unknown_fields() {
        let json = serde_json::to_string(&sample()).unwrap();
        let modified = json.replacen(
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"autoApply\":true",
            1,
        );
        assert_eq!(
            ProposalEnvelope::parse(&modified).unwrap_err().code(),
            "PROPOSAL_INVALID"
        );
        let modified = json.replacen(
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"command\":\"cargo test\"",
            1,
        );
        assert_eq!(
            ProposalEnvelope::parse(&modified).unwrap_err().code(),
            "PROPOSAL_INVALID"
        );
    }

    #[test]
    fn deny_duplicate_and_escaped_paths() {
        let mut proposal = sample();
        proposal.edits[0].relative_path = "../outside".into();
        assert_eq!(
            proposal.validate().unwrap_err().code(),
            "INVALID_RELATIVE_PATH"
        );
        proposal.edits[0].relative_path = "src/main.rs".into();
        proposal.edits.push(ProposedEdit {
            relative_path: "src/main.rs".into(),
            expected_revision: "a".repeat(64),
            proposed_text: String::new(),
        });
        assert_eq!(
            proposal.validate().unwrap_err().code(),
            "PROPOSAL_DUPLICATE_TARGET"
        );
    }

    #[test]
    fn deny_invalid_version_and_unbounded_payload() {
        let mut proposal = sample();
        proposal.schema_version = 2;
        assert_eq!(
            proposal.validate().unwrap_err().code(),
            "PROPOSAL_UNSUPPORTED_VERSION"
        );
        proposal.schema_version = 1;
        proposal.edits[0].proposed_text = "x".repeat(MAX_TEXT_BYTES as usize + 1);
        assert_eq!(
            proposal.validate().unwrap_err().code(),
            "PROPOSAL_TOO_LARGE"
        );
        assert_eq!(
            ProposalEnvelope::parse(&"x".repeat(MAX_PROPOSAL_BYTES + 1))
                .unwrap_err()
                .code(),
            "PROPOSAL_TOO_LARGE"
        );
    }
}
