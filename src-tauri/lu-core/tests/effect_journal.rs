use lu_core::{
    contracts::{ProtoPayload, ToolInvocation},
    journal::{EffectJournal, EffectState, JournalError, PrepareOutcome},
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("lu-journal-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}
fn call(id: &str) -> ToolInvocation {
    ToolInvocation {
        call_id: id.into(),
        tool_id: "lu.core.file_write".into(),
        workspace_id: Some("session-1".into()),
        input: ProtoPayload {
            type_url: "type.googleapis.com/lu.v1.WriteRequest".into(),
            bytes: b"target + contents".to_vec(),
        },
    }
}
#[test]
fn effect_survives_restart_and_applied_receipt_is_not_reexecuted() {
    let root = dir();
    let db = root.join("effects.sqlite");
    let request = call("call-1");
    {
        let journal = EffectJournal::open(&db).unwrap();
        assert_eq!(journal.prepare(&request).unwrap(), PrepareOutcome::New);
        journal.mark_executing("call-1").unwrap();
        journal.mark_applied("call-1", b"protobuf-reply").unwrap();
        assert!(matches!(
            journal.mark_executing("call-1"),
            Err(JournalError::InvalidTransition)
        ));
    }
    {
        let journal = EffectJournal::open(&db).unwrap();
        assert_eq!(
            journal.prepare(&request).unwrap(),
            PrepareOutcome::Existing(EffectState::Applied)
        );
        let receipt = journal.get("call-1").unwrap().unwrap().receipt.unwrap();
        assert_eq!(receipt, b"protobuf-reply");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn interrupted_running_effect_is_uncertain_never_automatically_replayed() {
    let root = dir();
    let db = root.join("effects.sqlite");
    {
        let journal = EffectJournal::open(&db).unwrap();
        journal.prepare(&call("call-2")).unwrap();
        journal.mark_executing("call-2").unwrap();
    }
    let journal = EffectJournal::open(&db).unwrap();
    assert_eq!(journal.reconcile_interrupted().unwrap(), 1);
    assert_eq!(
        journal.prepare(&call("call-2")).unwrap(),
        PrepareOutcome::Existing(EffectState::Uncertain)
    );
    assert!(matches!(
        journal.mark_executing("call-2"),
        Err(JournalError::InvalidTransition)
    ));
    drop(journal);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn duplicate_call_id_with_changed_payload_is_rejected() {
    let root = dir();
    let db = root.join("effects.sqlite");
    let journal = EffectJournal::open(&db).unwrap();
    journal.prepare(&call("duplicate")).unwrap();
    let mut changed = call("duplicate");
    changed.input.bytes.push(0);
    assert_eq!(
        journal.prepare(&changed),
        Err(JournalError::DuplicateIdConflict)
    );
    drop(journal);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn transitions_fail_closed_and_prepared_can_be_identified() {
    let root = dir();
    let db = root.join("effects.sqlite");
    let journal = EffectJournal::open(&db).unwrap();
    journal.prepare(&call("prepared")).unwrap();
    assert_eq!(journal.reconcile_interrupted().unwrap(), 0);
    assert_eq!(
        journal.prepare(&call("prepared")).unwrap(),
        PrepareOutcome::Existing(EffectState::Prepared)
    );
    assert!(matches!(
        journal.mark_applied("prepared", b"reply"),
        Err(JournalError::InvalidTransition)
    ));
    journal.mark_executing("prepared").unwrap();
    journal.mark_uncertain("prepared").unwrap();
    assert!(matches!(
        journal.mark_applied("prepared", b"reply"),
        Err(JournalError::InvalidTransition)
    ));
    drop(journal);
    let _ = fs::remove_dir_all(root);
}
