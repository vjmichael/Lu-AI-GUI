//! CONTEXT-01: recovery is conversation-scoped, context is not authority.
use lu_core::continuity::{ContinuityError, ContinuityStore, TaskCheckpoint};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

fn unique_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "lu-continuity-scope-{}-{}.sqlite",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
    ))
}

fn checkpoint(id: &str, chat: &str, next: &str) -> TaskCheckpoint {
    TaskCheckpoint {
        checkpoint_id: id.into(),
        project_id: "lu".into(),
        task_id: "v1".into(),
        conversation_id: chat.into(),
        source_entry_ids: vec![],
        completed_work: "persisted checkpoint".into(),
        next_action: next.into(),
        blockers: vec![],
    }
}

#[test]
fn scoped_resume_survives_restart_without_cross_chat_context_leakage() {
    let path = unique_path();
    let store = ContinuityStore::open(&path).unwrap();
    store.append(&checkpoint("cp-a1", "chat-a", "continue ANSF")).unwrap();
    store.append(&checkpoint("cp-b1", "chat-b", "review native tools")).unwrap();
    store.append(&checkpoint("cp-a2", "chat-a", "resume native tool tests")).unwrap();
    assert_eq!(
        store.latest_for_conversation("lu", "v1", "chat-a").unwrap().unwrap().checkpoint_id,
        "cp-a2"
    );
    assert_eq!(
        store.latest_for_conversation("lu", "v1", "chat-b").unwrap().unwrap().checkpoint_id,
        "cp-b1"
    );
    assert!(store.latest_for_conversation("lu", "v1", "chat-c").unwrap().is_none());
    assert!(store.latest_for_conversation("lu", "not-this-task", "chat-a").unwrap().is_none());
    drop(store);
    let reopened = ContinuityStore::open(&path).unwrap();
    let restored = reopened.latest_for_conversation("lu", "v1", "chat-a").unwrap().unwrap();
    assert_eq!(restored.next_action, "resume native tool tests");
    assert_eq!(restored.conversation_id, "chat-a");
    assert_eq!(reopened.recent_for_conversation("chat-a", 10).unwrap().len(), 2);
    drop(reopened);
    fs::remove_file(&path).unwrap();
}

#[test]
fn continuity_never_promotes_an_unverified_source_or_stolen_checkpoint_id() {
    let path = unique_path();
    let store = ContinuityStore::open(&path).unwrap();
    let first = checkpoint("cp-immutable", "chat-a", "original");
    store.append(&first).unwrap();
    assert_eq!(
        store.append(&checkpoint("cp-immutable", "chat-b", "forged")).unwrap_err(),
        ContinuityError::IdentityConflict
    );
    assert_eq!(
        store.latest_for_conversation("lu", "v1", "chat-a").unwrap().unwrap().next_action,
        "original"
    );
    assert!(store.latest_for_conversation("lu", "v1", "chat-b").unwrap().is_none());
    assert_eq!(
        store.latest_for_conversation("lu", "v1", "../chat-a").unwrap_err(),
        ContinuityError::InvalidInput
    );
    drop(store);
    fs::remove_file(&path).unwrap();
}
