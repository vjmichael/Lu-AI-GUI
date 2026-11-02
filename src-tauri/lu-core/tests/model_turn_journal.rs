use lu_core::{
    model_json_boundary::{JsonRepairStatus, MAX_BOUNDARY_JSON_BYTES},
    model_turn_journal::{ModelAuditError, ModelAuditKind, ModelTurnJournal},
};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn model_boundary_evidence_survives_reopen_and_remains_scoped() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("lu-model-journal-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let db = root.join("model-boundary.sqlite3");
    {
        let journal = ModelTurnJournal::open(&db).unwrap();
        journal
            .append(
                "conversation-a",
                "task-1",
                ModelAuditKind::ModelOutput,
                r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt",},}"#,
                None,
            )
            .unwrap();
        journal
            .append(
                "conversation-a",
                "task-1",
                ModelAuditKind::ParsedCandidate,
                "lu_files_read_text",
                Some(JsonRepairStatus::RemovedTrailingCommas),
            )
            .unwrap();
        journal
            .append(
                "conversation-a",
                "task-1",
                ModelAuditKind::ToolFeedback,
                r#"{"kind":"tool_result","response":{"ok":true}}"#,
                None,
            )
            .unwrap();
        journal
            .append(
                "conversation-b",
                "task-1",
                ModelAuditKind::Rejected,
                "MalformedModelReply",
                None,
            )
            .unwrap();
    }
    {
        let journal = ModelTurnJournal::open(&db).unwrap();
        let records = journal.events_for("conversation-a", "task-1").unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].kind, ModelAuditKind::ModelOutput);
        assert_eq!(records[1].kind, ModelAuditKind::ParsedCandidate);
        assert_eq!(records[1].repair.as_deref(), Some("RemovedTrailingCommas"));
        assert_eq!(records[2].kind, ModelAuditKind::ToolFeedback);
        assert!(journal
            .events_for("conversation-a", "task-2")
            .unwrap()
            .is_empty());
        assert_eq!(
            journal
                .events_for("conversation-b", "task-1")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            journal.append("", "task-1", ModelAuditKind::ModelOutput, "x", None),
            Err(ModelAuditError::InvalidOrigin)
        );
        assert_eq!(
            journal.append(
                "conversation-a",
                "task-1",
                ModelAuditKind::ModelOutput,
                &"x".repeat(MAX_BOUNDARY_JSON_BYTES + 1),
                None
            ),
            Err(ModelAuditError::OversizedEvent)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
