use lu_core::{
    HostPrincipal, OwnershipStore, OwnershipStoreError, ResourceAction, ResourceDenial,
    ResourceRecord, Sensitivity,
};

#[test]
fn survives_reopen_and_denies_reassignment_and_cross_profile() {
    let path = std::env::temp_dir().join(format!(
        "lu-owner-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let record = ResourceRecord {
        conversation_id: "thread-a".into(),
        owner_profile_id: "user-a".into(),
        project_id: "project-a".into(),
        sensitivity: Sensitivity::Confidential,
    };
    let store = OwnershipStore::open(&path).unwrap();
    store.enroll(&record).unwrap();
    drop(store);

    let reopened = OwnershipStore::open(&path).unwrap();
    let principal = HostPrincipal {
        profile_id: "user-a".into(),
        active_project_id: "project-a".into(),
        export_enabled: false,
        training_enabled: false,
        maximum_sensitivity: Sensitivity::Confidential,
    };
    assert!(reopened
        .require(&principal, "thread-a", ResourceAction::Read)
        .is_ok());
    assert_eq!(
        reopened.require(&principal, "thread-a", ResourceAction::Train),
        Err(OwnershipStoreError::Denied(
            ResourceDenial::MissingCapability
        ))
    );
    let mut attacker = principal.clone();
    attacker.profile_id = "user-b".into();
    assert_eq!(
        reopened.require(&attacker, "thread-a", ResourceAction::Read),
        Err(OwnershipStoreError::Denied(ResourceDenial::WrongOwner))
    );
    let mut reassignment = record;
    reassignment.owner_profile_id = "user-b".into();
    assert_eq!(
        reopened.enroll(&reassignment),
        Err(OwnershipStoreError::Denied(ResourceDenial::WrongOwner))
    );
    assert_eq!(
        reopened.require(&principal, "nonexistent", ResourceAction::Read),
        Err(OwnershipStoreError::Denied(ResourceDenial::MissingRecord))
    );
    drop(reopened);
    let _ = std::fs::remove_file(path);
}
