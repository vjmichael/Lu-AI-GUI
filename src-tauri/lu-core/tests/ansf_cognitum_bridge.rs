use lu_core::ansf::{AnsfError, AnsfStore, EntryRole};
use lu_core::ansf_cognitum_bridge::AnsfReadAuthorizer;
use lu_core::ansf_revisions::RevisionOrigin;

struct Allow;
impl AnsfReadAuthorizer for Allow {
    fn may_read(&self, workspace_id: &str, session_id: &str) -> bool {
        workspace_id == "workspace" && session_id == "chat"
    }
}
struct Deny;
impl AnsfReadAuthorizer for Deny {
    fn may_read(&self, _workspace_id: &str, _session_id: &str) -> bool { false }
}

#[test]
fn rust_fts5_returns_current_revisions_with_host_scope() {
    let store = AnsfStore::in_memory().unwrap();
    store.create_session("chat", "workspace", "Memory").unwrap();
    store.start_chapter("chat", "Topic").unwrap();
    store.create_session("other", "different", "Other").unwrap();
    store.start_chapter("other", "Topic").unwrap();

    let root = store.append_entry("chat", "msg", EntryRole::User,
        "Legacy engine", "The zeppelin engine was old").unwrap();
    store.append_entry("other", "msg2", EntryRole::User,
        "Private engine", "Secret zeppelin engine").unwrap();
    let corrected = store.append_edited_entry("chat", "msg",
        "New engine", "The jet engine is new",
        RevisionOrigin::VerifiedUser).unwrap().unwrap();

    assert!(store.search_cognitum_evidence(
        "workspace", "chat", "zeppelin", 10, &Allow
    ).unwrap().is_empty());
    let hits = store.search_cognitum_evidence(
        "workspace", "chat", "jet", 10, &Allow
    ).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].original_entry_id, root);
    assert_eq!(hits[0].entry_id, corrected.revised_entry_id);
    assert_eq!(hits[0].revision_number, 1);
    assert!(hits[0].citation_handle.starts_with("ansf:chat:"));
    assert_eq!(store.search_cognitum_evidence(
        "different", "chat", "jet", 10, &Allow
    ).unwrap(), Vec::new());
    assert_eq!(store.search_cognitum_evidence(
        "workspace", "chat", "jet", 10, &Deny
    ), Err(AnsfError::InvalidInput));
}

#[test]
fn rust_fts5_treats_operators_as_literal_tokens() {
    let store = AnsfStore::in_memory().unwrap();
    store.create_session("chat", "workspace", "Memory").unwrap();
    store.start_chapter("chat", "Topic").unwrap();
    store.append_entry("chat", "msg", EntryRole::User,
        "Accepted", "The book uses plain words").unwrap();
    assert!(store.search_cognitum_evidence(
        "workspace", "chat", "missing OR book", 10, &Allow
    ).unwrap().is_empty());
    assert!(store.search_cognitum_evidence(
        "workspace", "chat", "book", 10, &Allow
    ).unwrap().len() == 1);
    assert_eq!(store.search_cognitum_evidence(
        "workspace", "chat", "", 10, &Allow
    ), Err(AnsfError::InvalidInput));
}
