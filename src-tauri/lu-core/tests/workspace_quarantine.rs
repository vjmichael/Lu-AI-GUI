use lu_core::{
    registry::NativeToolRegistry,
    workspace::WorkspaceHost,
    workspace_quarantine::{
        IntakeStatus, QuarantineError, WorkspaceQuarantine, MAX_STAGED_ARCHIVE_BYTES,
    },
};

#[test]
fn untrusted_workspace_is_staged_without_executable_registration() {
    let registry = NativeToolRegistry::new();
    let host = WorkspaceHost::new(registry.clone());
    let intake = WorkspaceQuarantine::new();
    let receipt = intake
        .stage("mythos.toaster", b"pretend untrusted DLC archive")
        .unwrap();
    assert_eq!(receipt.status, IntakeStatus::Quarantined);
    assert_eq!(receipt.sha256.len(), 64);
    assert_eq!(intake.get("mythos.toaster").unwrap(), Some(receipt));
    // Neither native tools nor workspace instances are created by staging.
    assert!(registry.tools().unwrap().is_empty());
    assert!(registry.modules().unwrap().is_empty());
    assert!(host.instances().unwrap().is_empty());
}

#[test]
fn rejection_is_terminal_and_cannot_be_replaced_under_same_identity() {
    let intake = WorkspaceQuarantine::new();
    let staged = intake
        .stage("mythos.toaster", b"dangerous archive")
        .unwrap();
    assert_eq!(
        intake
            .reject("mythos.toaster", "wrong hash", "suspected malicious")
            .unwrap_err(),
        QuarantineError::DigestMismatch
    );
    let rejected = intake
        .reject(
            "mythos.toaster",
            &staged.sha256,
            "attempted to request elevation",
        )
        .unwrap();
    assert_eq!(rejected.status, IntakeStatus::Rejected);
    assert!(rejected.reason.unwrap().contains("elevation"));
    assert_eq!(
        intake
            .reject("mythos.toaster", &staged.sha256, "retry")
            .unwrap_err(),
        QuarantineError::AlreadyRejected
    );
    assert_eq!(
        intake
            .stage("mythos.toaster", b"replacement payload")
            .unwrap_err(),
        QuarantineError::DuplicatePackageId
    );
}

#[test]
fn intake_rejects_path_names_empty_and_oversized_archives() {
    let intake = WorkspaceQuarantine::new();
    for name in [
        "",
        "../escape",
        "mythos/evil",
        "mythos..toaster",
        "Mythos.toaster",
    ] {
        assert_eq!(
            intake.stage(name, b"archive").unwrap_err(),
            QuarantineError::InvalidPackageId
        );
    }
    assert_eq!(
        intake.stage("mythos.toaster", b"").unwrap_err(),
        QuarantineError::ArchiveEmpty
    );
    assert_eq!(
        intake
            .stage("mythos.toaster", &vec![1u8; MAX_STAGED_ARCHIVE_BYTES + 1])
            .unwrap_err(),
        QuarantineError::ArchiveTooLarge
    );
    assert!(intake.list().unwrap().is_empty());
}

#[test]
fn a_digest_identifies_exact_bytes_but_does_not_grant_authority() {
    let intake = WorkspaceQuarantine::new();
    let original = intake.stage("example.workspace", b"archive-v1").unwrap();
    let different = WorkspaceQuarantine::new()
        .stage("example.workspace", b"archive-v2")
        .unwrap();
    assert_ne!(original.sha256, different.sha256);
    assert_eq!(original.status, IntakeStatus::Quarantined);
    assert_eq!(different.status, IntakeStatus::Quarantined);
}
