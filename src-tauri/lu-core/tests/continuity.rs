use lu_core::{AnsfStore, ContinuityError, ContinuityStore, TaskCheckpoint};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp() -> PathBuf {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("lu-continuity-{}-{t}.sqlite", std::process::id()))
}
fn sample() -> TaskCheckpoint {
    TaskCheckpoint {
        checkpoint_id: "checkpoint-001".into(),
        project_id: "lu".into(),
        task_id: "memory-worker".into(),
        conversation_id: "thread-123".into(),
        source_entry_ids: vec![1, 2],
        completed_work: "Completed indexed import".into(),
        next_action: "Validate Windows runtime".into(),
        blockers: vec!["Smoke test".into()],
    }
}
#[test]
fn checkpoint_replay_and_restart_preserve_identity() {
    let db = temp();
    let first = sample();
    {
        let store = ContinuityStore::open(&db).unwrap();
        store.append(&first).unwrap();
        store.append(&first).unwrap();
        assert_eq!(
            store.latest("lu", "memory-worker").unwrap(),
            Some(first.clone())
        );
        let mut conflict = first.clone();
        conflict.next_action = "Unverified change".into();
        assert_eq!(
            store.append(&conflict),
            Err(ContinuityError::IdentityConflict)
        );
    }
    {
        let store = ContinuityStore::open(&db).unwrap();
        let mut next = first.clone();
        next.checkpoint_id = "checkpoint-002".into();
        next.completed_work = "Windows smoke test finished".into();
        store.append(&next).unwrap();
        assert_eq!(store.latest("lu", "memory-worker").unwrap(), Some(next));
        assert!(store
            .latest("another-project", "memory-worker")
            .unwrap()
            .is_none());
    }
    let _ = std::fs::remove_file(db);
}
#[test]
fn rejects_invalid_ids_and_unbounded_checkpoint_fields() {
    let db = temp();
    let store = ContinuityStore::open(&db).unwrap();
    let mut bad = sample();
    bad.project_id = "../other".into();
    assert_eq!(store.append(&bad), Err(ContinuityError::InvalidInput));
    bad = sample();
    bad.source_entry_ids = vec![0];
    assert_eq!(store.append(&bad), Err(ContinuityError::InvalidInput));
    bad = sample();
    bad.next_action = "x".repeat(4097);
    assert_eq!(store.append(&bad), Err(ContinuityError::InvalidInput));
    let _ = std::fs::remove_file(db);
}

#[test]
fn verified_append_rejects_missing_or_foreign_source_entries() {
    let db = temp();
    let ansf_path = temp();
    let store = ContinuityStore::open(&db).unwrap();
    let ansf = AnsfStore::open(&ansf_path).unwrap();
    ansf.create_session("thread-123", "lu", "Test").unwrap();
    let checkpoint = sample();
    assert_eq!(
        store.append_verified(&checkpoint, &ansf),
        Err(ContinuityError::InvalidInput)
    );
    assert!(store.latest("lu", "memory-worker").unwrap().is_none());
    drop(ansf);
    drop(store);
    let _ = std::fs::remove_file(db);
    let _ = std::fs::remove_file(ansf_path);
}

#[test]
fn recent_checkpoints_never_include_other_conversations() {
    let db = temp();
    let store = ContinuityStore::open(&db).unwrap();
    let first = sample();
    store.append(&first).unwrap();
    let mut unrelated = sample();
    unrelated.checkpoint_id = "checkpoint-foreign".into();
    unrelated.conversation_id = "thread-foreign".into();
    store.append(&unrelated).unwrap();
    let found = store.recent_for_conversation("thread-123", 20).unwrap();
    assert_eq!(found, vec![first]);
    assert!(store
        .recent_for_conversation("thread-missing", 20)
        .unwrap()
        .is_empty());
    assert_eq!(
        store.recent_for_conversation("thread-123", 31),
        Err(ContinuityError::InvalidInput)
    );
    drop(store);
    let _ = std::fs::remove_file(db);
}
