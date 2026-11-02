use lu_core::{
    host_session::{read_only_principal, HostSessionRegistry, SessionError},
    OwnershipStore, OwnershipStoreError, ResourceAction, ResourceDenial, ResourceRecord,
    Sensitivity,
};

#[test]
fn session_binding_is_host_only_scoped_and_revocable() {
    let path = std::env::temp_dir().join(format!(
        "lu-window-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = OwnershipStore::open(&path).unwrap();
    store
        .enroll(&ResourceRecord {
            conversation_id: "thread-1".into(),
            owner_profile_id: "alice".into(),
            project_id: "lu".into(),
            sensitivity: Sensitivity::Internal,
        })
        .unwrap();
    let sessions = HostSessionRegistry::new();
    assert_eq!(
        sessions.require_window_resource("main", "thread-1", ResourceAction::Read, &store),
        Err(SessionError::UnknownWindow)
    );
    sessions
        .bind_host_window("main", read_only_principal("alice".into(), "lu".into()))
        .unwrap();
    assert_eq!(
        sessions.bind_host_window("main", read_only_principal("mallory".into(), "lu".into())),
        Err(SessionError::InvalidWindow)
    );
    assert!(sessions
        .require_window_resource("main", "thread-1", ResourceAction::Read, &store)
        .is_ok());
    assert_eq!(
        sessions.require_window_resource("main", "thread-1", ResourceAction::Export, &store),
        Err(SessionError::Ownership(OwnershipStoreError::Denied(
            ResourceDenial::MissingCapability
        )))
    );
    sessions.unbind_host_window("main").unwrap();
    assert_eq!(
        sessions.require_window_resource("main", "thread-1", ResourceAction::Read, &store),
        Err(SessionError::UnknownWindow)
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}
