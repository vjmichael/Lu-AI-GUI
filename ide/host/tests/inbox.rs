use std::{fs, path::PathBuf};

use lu_ide_workspace::{
    inbox::{IdeProposalInbox, ReviewStatus},
    proposal::{ProposalEnvelope, ProposedEdit},
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-inbox-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::create_dir_all(root.join("project/src")).unwrap();
    fs::write(root.join("project/src/one.py"), "x = 1\n").unwrap();
    fs::write(root.join("project/src/two.py"), "y = 2\n").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(root.join("project"))))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}
fn envelope(registry: &IdeWorkspaceRegistry, project_id: &str) -> ProposalEnvelope {
    let one = registry
        .preview_source_file(project_id, "src/one.py")
        .unwrap();
    let two = registry
        .preview_source_file(project_id, "src/two.py")
        .unwrap();
    ProposalEnvelope {
        schema_version: 1,
        task_id: "review-01".into(),
        project_id: project_id.to_owned(),
        claimed_producer: "Lu".into(), // untrusted string, not proof of origin
        rationale: "Inspect first; no automatic application".into(),
        edits: vec![
            ProposedEdit {
                relative_path: "src/one.py".into(),
                expected_revision: one.revision,
                proposed_text: "x = 123\n".into(),
            },
            ProposedEdit {
                relative_path: "src/two.py".into(),
                expected_revision: two.revision,
                proposed_text: "y = 456\n".into(),
            },
        ],
    }
}

#[test]
fn import_review_and_approve_are_data_only_no_project_writes() {
    let (root, registry, id) = fixture();
    let inbox = IdeProposalInbox::default();
    let json = serde_json::to_string(&envelope(&registry, &id)).unwrap();
    let received = inbox.import(&registry, &json).unwrap();
    assert_eq!(received.status, ReviewStatus::Pending);
    assert_eq!(received.edit_count, 2);
    assert_eq!(received.digest.len(), 64);
    assert_eq!(inbox.list().unwrap().len(), 1);
    let detail = inbox.detail(&registry, &received.inbox_id).unwrap();
    assert_eq!(detail.reviews.len(), 2);
    assert!(detail.reviews.iter().all(|r| !r.truncated));
    let decided = inbox
        .decide(&registry, &received.inbox_id, &received.digest, true)
        .unwrap();
    assert_eq!(decided.status, ReviewStatus::ApprovedForStaging);
    let staged = inbox.staging(&registry, &received.inbox_id, &received.digest).unwrap();
    assert_eq!(staged.edits.len(), 2);
    assert_eq!(staged.edits[0].proposed_text, "x = 123\n");
    assert_eq!(staged.edits[1].proposed_text, "y = 456\n");
    assert_eq!(
        fs::read_to_string(root.join("project/src/one.py")).unwrap(),
        "x = 1\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("project/src/two.py")).unwrap(),
        "y = 2\n"
    );
    assert_eq!(
        inbox
            .decide(&registry, &received.inbox_id, &received.digest, true)
            .unwrap_err()
            .code(),
        "PROPOSAL_STATE_CHANGED"
    );
    inbox.discard(&received.inbox_id).unwrap();
    assert!(inbox.list().unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_source_blocks_approval_and_retains_original_files() {
    let (root, registry, id) = fixture();
    let inbox = IdeProposalInbox::default();
    let received = inbox
        .import(
            &registry,
            &serde_json::to_string(&envelope(&registry, &id)).unwrap(),
        )
        .unwrap();
    fs::write(root.join("project/src/two.py"), "externally updated\n").unwrap();
    assert_eq!(
        inbox
            .decide(&registry, &received.inbox_id, &received.digest, true)
            .unwrap_err()
            .code(),
        "FILE_CHANGED_ON_DISK"
    );
    assert_eq!(inbox.list().unwrap()[0].status, ReviewStatus::Pending);
    assert_eq!(
        fs::read_to_string(root.join("project/src/one.py")).unwrap(),
        "x = 1\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("project/src/two.py")).unwrap(),
        "externally updated\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn wrong_digest_and_duplicate_decisions_are_denied() {
    let (root, registry, id) = fixture();
    let inbox = IdeProposalInbox::default();
    let received = inbox
        .import(
            &registry,
            &serde_json::to_string(&envelope(&registry, &id)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        inbox
            .decide(&registry, &received.inbox_id, &"f".repeat(64), true)
            .unwrap_err()
            .code(),
        "PROPOSAL_STATE_CHANGED"
    );
    let decision = inbox
        .decide(&registry, &received.inbox_id, &received.digest, false)
        .unwrap();
    assert_eq!(decision.status, ReviewStatus::Rejected);
    assert_eq!(
        inbox
            .decide(&registry, &received.inbox_id, &received.digest, true)
            .unwrap_err()
            .code(),
        "PROPOSAL_STATE_CHANGED"
    );
    assert_eq!(
        inbox.detail(&registry, "made-up").unwrap_err().code(),
        "PROPOSAL_NOT_FOUND"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incomplete_preview_cannot_be_approved_for_staging() {
    let (root, registry, id) = fixture();
    let mut doc = envelope(&registry, &id);
    doc.edits.truncate(1);
    doc.edits[0].proposed_text = "new value\n".repeat(350);
    let inbox = IdeProposalInbox::default();
    let received = inbox
        .import(&registry, &serde_json::to_string(&doc).unwrap())
        .unwrap();
    let detail = inbox.detail(&registry, &received.inbox_id).unwrap();
    assert!(detail.reviews[0].truncated);
    assert_eq!(
        inbox
            .decide(&registry, &received.inbox_id, &received.digest, true)
            .unwrap_err()
            .code(),
        "PROPOSAL_PREVIEW_INCOMPLETE"
    );
    assert_eq!(
        fs::read_to_string(root.join("project/src/one.py")).unwrap(),
        "x = 1\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn proposal_queue_is_volatile_and_never_persists_untrusted_code() {
    let (root, registry, id) = fixture();
    let inbox = IdeProposalInbox::default();
    inbox
        .import(
            &registry,
            &serde_json::to_string(&envelope(&registry, &id)).unwrap(),
        )
        .unwrap();
    assert_eq!(inbox.list().unwrap().len(), 1);
    drop(inbox);
    let new_process_queue = IdeProposalInbox::default();
    assert!(new_process_queue.list().unwrap().is_empty());
    let catalog = fs::read_to_string(root.join("catalog.json")).unwrap();
    assert!(!catalog.contains("claimedProducer"));
    assert!(!catalog.contains("x = 123"));
    fs::remove_dir_all(root).unwrap();
}
