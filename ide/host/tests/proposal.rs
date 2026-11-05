use std::{fs, path::PathBuf};
use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use lu_ide_workspace::proposal::{ProposalEnvelope, ProposedEdit};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> { Ok(self.0.take()) }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-proposal-{}-{:016x}", std::process::id(), rand::random::<u64>()
    ));
    fs::create_dir_all(root.join("project/src")).unwrap();
    fs::write(root.join("project/src/first.rs"), "fn first() {}\n").unwrap();
    fs::write(root.join("project/src/second.rs"), "fn second() {}\n").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(root.join("project"))))
        .unwrap().unwrap().project_id;
    (root, registry, id)
}

#[test]
fn untrusted_two_file_proposal_reviews_without_writes() {
    let (root, registry, id) = fixture();
    let first = registry.preview_source_file(&id, "src/first.rs").unwrap();
    let second = registry.preview_source_file(&id, "src/second.rs").unwrap();
    let proposal = ProposalEnvelope {
        schema_version: 1,
        task_id: "task-01".into(),
        project_id: id,
        claimed_producer: "Lu".into(),
        rationale: "Only a hypothetical patch; no authorization".into(),
        edits: vec![
            ProposedEdit {
                relative_path: "src/first.rs".into(),
                expected_revision: first.revision,
                proposed_text: "fn first() { println!(\"proposal\"); }\n".into(),
            },
            ProposedEdit {
                relative_path: "src/second.rs".into(),
                expected_revision: second.revision,
                proposed_text: "fn second() { println!(\"proposal\"); }\n".into(),
            },
        ],
    };
    let json = serde_json::to_string(&proposal).unwrap();
    let imported = ProposalEnvelope::parse(&json).unwrap();
    let reviews = imported.review(&registry).unwrap();
    assert_eq!(reviews.len(), 2);
    assert_eq!(reviews[0].added_lines, 1);
    assert_eq!(reviews[1].added_lines, 1);
    assert_eq!(fs::read_to_string(root.join("project/src/first.rs")).unwrap(), first.text);
    assert_eq!(fs::read_to_string(root.join("project/src/second.rs")).unwrap(), second.text);
    assert!(!root.join("project/marker").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_one_file_causes_whole_review_request_to_fail_closed() {
    let (root, registry, id) = fixture();
    let first = registry.preview_source_file(&id, "src/first.rs").unwrap();
    let second = registry.preview_source_file(&id, "src/second.rs").unwrap();
    fs::write(root.join("project/src/second.rs"), "modified externally\n").unwrap();
    let envelope = ProposalEnvelope {
        schema_version: 1,
        task_id: "task-02".into(),
        project_id: id,
        claimed_producer: "Lu".into(),
        rationale: String::new(),
        edits: vec![
            ProposedEdit {
                relative_path: "src/first.rs".into(),
                expected_revision: first.revision,
                proposed_text: "new first\n".into(),
            },
            ProposedEdit {
                relative_path: "src/second.rs".into(),
                expected_revision: second.revision,
                proposed_text: "new second\n".into(),
            },
        ],
    };
    assert_eq!(envelope.review(&registry).unwrap_err().code(), "FILE_CHANGED_ON_DISK");
    assert_eq!(fs::read_to_string(root.join("project/src/first.rs")).unwrap(), first.text);
    assert_eq!(fs::read_to_string(root.join("project/src/second.rs")).unwrap(), "modified externally\n");
    fs::remove_dir_all(root).unwrap();
}
