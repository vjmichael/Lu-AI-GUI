use lu_core::host_resource_authority::{
    HostPrincipal, HostResourceAuthority, ResourceAction, ResourceDenial, ResourceRecord,
    Sensitivity,
};

fn principal() -> HostPrincipal {
    HostPrincipal {
        profile_id: "alice".into(),
        active_project_id: "lu".into(),
        export_enabled: false,
        training_enabled: false,
        maximum_sensitivity: Sensitivity::Confidential,
    }
}

#[test]
fn denies_foreign_project_profile_and_unknown_resources() {
    let mut authority = HostResourceAuthority::new();
    authority
        .enroll(ResourceRecord {
            conversation_id: "chat-1".into(),
            owner_profile_id: "alice".into(),
            project_id: "lu".into(),
            sensitivity: Sensitivity::Internal,
        })
        .unwrap();

    assert!(authority
        .require(&principal(), "chat-1", ResourceAction::Read)
        .is_ok());
    assert_eq!(
        authority.require(&principal(), "unlisted", ResourceAction::Read),
        Err(ResourceDenial::MissingRecord)
    );
    let mut foreign = principal();
    foreign.profile_id = "bob".into();
    assert_eq!(
        authority.require(&foreign, "chat-1", ResourceAction::Read),
        Err(ResourceDenial::WrongOwner)
    );
    let mut other_project = principal();
    other_project.active_project_id = "other".into();
    assert_eq!(
        authority.require(&other_project, "chat-1", ResourceAction::Read),
        Err(ResourceDenial::WrongProject)
    );
}

#[test]
fn read_permission_does_not_imply_training_or_export() {
    let mut authority = HostResourceAuthority::new();
    authority
        .enroll(ResourceRecord {
            conversation_id: "chat-1".into(),
            owner_profile_id: "alice".into(),
            project_id: "lu".into(),
            sensitivity: Sensitivity::Confidential,
        })
        .unwrap();
    for action in [ResourceAction::Export, ResourceAction::Train] {
        assert_eq!(
            authority.require(&principal(), "chat-1", action),
            Err(ResourceDenial::MissingCapability)
        );
    }
    let mut granted = principal();
    granted.export_enabled = true;
    assert!(authority
        .require(&granted, "chat-1", ResourceAction::Export)
        .is_ok());
    assert_eq!(
        authority.require(&granted, "chat-1", ResourceAction::Train),
        Err(ResourceDenial::MissingCapability)
    );
    granted.training_enabled = true;
    assert!(authority
        .require(&granted, "chat-1", ResourceAction::Train)
        .is_ok());
}

#[test]
fn sensitivity_and_enrollment_changes_fail_closed() {
    let mut authority = HostResourceAuthority::new();
    let protected = ResourceRecord {
        conversation_id: "chat-1".into(),
        owner_profile_id: "alice".into(),
        project_id: "lu".into(),
        sensitivity: Sensitivity::Restricted,
    };
    authority.enroll(protected.clone()).unwrap();
    assert_eq!(
        authority.require(&principal(), "chat-1", ResourceAction::Read),
        Err(ResourceDenial::SensitivityExceeded)
    );
    authority.enroll(protected.clone()).unwrap();
    let mut reassigned = protected;
    reassigned.owner_profile_id = "bob".into();
    assert_eq!(
        authority.enroll(reassigned),
        Err(ResourceDenial::WrongOwner)
    );
}
