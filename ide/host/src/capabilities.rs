//! Wave A2: bounded, host-authored discovery of actual standalone IDE commands.
//! Metadata is NOT an executable dispatcher, authorization grant, or AI bridge.
use serde::Serialize;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDescriptor {
    pub id: &'static str,
    pub version: u32,
    pub command: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub effect: &'static str,
    pub input_fields: &'static [&'static str],
    pub result_type: &'static str,
    pub precondition: &'static str,
    pub evidence: &'static str,
    pub cancellation: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityCatalog {
    pub schema_version: u32,
    pub workspace: &'static str,
    pub connected_to_lu: bool,
    pub descriptions_only: bool,
    pub capabilities: Vec<CapabilityDescriptor>,
}

macro_rules! capability {
    ($id:literal, $command:literal, $title:literal, $description:literal,
     $effect:literal, [$($field:literal),* $(,)?], $result:literal,
     $precondition:literal, $evidence:literal) => {
        CapabilityDescriptor {
            id: $id,
            version: 1,
            command: $command,
            title: $title,
            description: $description,
            effect: $effect,
            input_fields: &[$($field),*],
            result_type: $result,
            precondition: $precondition,
            evidence: $evidence,
            cancellation: "single bounded host call; no background task",
        }
    };
}

/// Only real, registered Tauri operations are advertised. UI editor-buffer
/// actions (including search) are deliberately not misrepresented as host APIs.
pub fn list_capabilities() -> CapabilityCatalog {
    CapabilityCatalog {
        schema_version: SCHEMA_VERSION,
        workspace: "lu-ide-standalone",
        connected_to_lu: false,
        descriptions_only: true,
        capabilities: vec![
            capability!("project.list", "ide_list_projects", "List projects",
                "Read IDE-registered project summaries.", "read", [],
                "ProjectSummary[]", "IDE host available", "registered project summaries"),
            capability!("project.select", "ide_choose_project", "Choose project folder",
                "Open native OS directory picker and register selected project.", "catalog_write", [],
                "ProjectSummary[]", "explicit human picker selection", "updated project catalog"),
            capability!("project.open", "ide_open_project", "Open registered project",
                "Activate an existing project in the IDE.", "session_write", ["projectId"],
                "ProjectSummary[]", "registered project ID", "active project summary"),
            capability!("project.close", "ide_close_project", "Close project",
                "Close an open IDE project, without deleting its files.", "session_write", ["projectId"],
                "ProjectSummary[]", "open project ID", "inactive project summary"),
            capability!("project.unregister", "ide_unregister_project", "Remove project from IDE",
                "Remove project registration only; never remove files on disk.", "catalog_write", ["projectId"],
                "ProjectSummary[]", "registered project ID and user intent", "catalog without project"),
            capability!("source.list", "ide_list_source_directory", "List source directory",
                "Capability-scoped, bounded listing of one directory.", "read",
                ["projectId", "relativeDir"], "DirectoryListing",
                "registered and open project", "relative paths and truncation flag"),
            capability!("source.preview", "ide_preview_source_file", "Read source file",
                "Read bounded UTF-8 text and its saved SHA-256 revision.", "read",
                ["projectId", "relativePath"], "TextPreview",
                "open project and safe relative path", "text and saved source revision"),
            capability!("source.readSpan", "ide_read_source_span", "Read source lines",
                "Read exact numbered lines from a specified saved revision.", "read",
                ["projectId", "relativePath", "expectedRevision", "firstLine", "lastLine"],
                "SourceSpan", "open project and matching saved revision",
                "line text, UTF-8 byte offsets and newline identity"),
            capability!("source.review", "ide_review_source_change", "Review source change",
                "Calculate bounded diff preview without saving.", "read",
                ["projectId", "relativePath", "expectedRevision", "proposedText"],
                "ChangeReview", "matching saved revision and bounded proposed text",
                "diff hunks with exactness and truncation"),
            capability!("source.save", "ide_save_source_file", "Save source file",
                "Explicit host revision-checked file write; discovery does not authorize it.",
                "file_write", ["projectId", "relativePath", "expectedRevision", "text"],
                "TextPreview", "explicit human Save and matching saved revision",
                "post-write file content and revision"),
            capability!("proposal.import", "ide_import_proposal", "Import proposal",
                "Validate untrusted proposal as data in volatile inbox.", "session_write",
                ["proposalJson"], "ProposalSummary", "bounded valid proposal",
                "inbox summary; no filesystem write"),
            capability!("proposal.list", "ide_list_proposals", "List proposals",
                "Inspect volatile proposal inbox.", "read", [],
                "ProposalSummary[]", "IDE host available", "proposal summaries"),
            capability!("proposal.review", "ide_review_proposal", "Review proposal",
                "Read verified proposal details and bounded diffs.", "read",
                ["inboxId"], "ProposalDetail", "existing inbox item",
                "preview with exactness and truncation"),
            capability!("proposal.decide", "ide_decide_proposal", "Decide proposal",
                "Record human approval for staging only or rejection.", "session_write",
                ["inboxId", "expectedDigest", "approveForStaging"],
                "ProposalSummary", "human review and unchanged proposal digest",
                "recorded review decision; no file write"),
            capability!("proposal.stage", "ide_stage_proposal", "Stage proposal",
                "Return approved edit data for unsaved editor drafts only.", "read",
                ["inboxId", "expectedDigest"], "ApprovedStaging",
                "previous human approval and current source revisions",
                "validated staging data; no file write"),
            capability!("proposal.discard", "ide_discard_proposal", "Discard proposal",
                "Remove an inbox item without changing source files.", "session_write",
                ["inboxId"], "ProposalSummary[]", "existing inbox item",
                "updated volatile inbox"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn descriptors_are_unique_bounded_and_versioned() {
        let catalog = list_capabilities();
        assert_eq!(catalog.schema_version, 1);
        assert!(!catalog.connected_to_lu);
        assert!(catalog.descriptions_only);
        assert!(catalog.capabilities.len() <= 32);
        let mut ids = HashSet::new();
        let mut commands = HashSet::new();
        for item in &catalog.capabilities {
            assert_eq!(item.version, 1);
            assert!(ids.insert(item.id));
            assert!(commands.insert(item.command));
            assert!(item.command.starts_with("ide_"));
            assert!(!item.title.is_empty());
            assert!(!item.precondition.is_empty());
            assert!(!item.evidence.is_empty());
            assert!(matches!(item.effect, "read" | "session_write" | "catalog_write" | "file_write"));
        }
    }

    #[test]
    fn file_write_and_staging_are_never_conflated() {
        let items = list_capabilities().capabilities;
        let save = items.iter().find(|x| x.id == "source.save").unwrap();
        let stage = items.iter().find(|x| x.id == "proposal.stage").unwrap();
        assert_eq!(save.effect, "file_write");
        assert_eq!(stage.effect, "read");
        assert!(save.precondition.contains("explicit human Save"));
        assert!(stage.evidence.contains("no file write"));
    }
}
