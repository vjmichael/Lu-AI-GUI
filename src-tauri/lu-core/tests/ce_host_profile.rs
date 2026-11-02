//! Host-profile CE trust-store identity tests. Reusing the correct key ID
//! or copying a SQLite file must not transfer a consent grant across Lu profiles.
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signer, SigningKey};
use lu_core::{
    CeGrantRequest, CePermissionError, CePermissionStore, CePublisherKeyEnrollment, ToolEffect,
};
use sha2::{Digest, Sha256};

fn profile(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "lu_ce_profile_{}_{}_{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    }
    root
}
fn signing() -> SigningKey {
    SigningKey::from_bytes(&[68; 32])
}
fn enroll(store: &CePermissionStore) {
    store
        .enroll_publisher_key(&CePublisherKeyEnrollment {
            key_id: "publisher-key".into(),
            publisher: "Publisher".into(),
            public_key: signing().verifying_key().to_bytes(),
            not_before: 0,
            expires_at: i64::MAX,
        })
        .unwrap();
}
fn envelope() -> String {
    let artifact = b"worker";
    let manifest = serde_json::json!({
        "format_version":1, "id":"publisher.worker","version":"1.0.0",
        "contract_version":1, "publisher":"Publisher",
        "artifact_sha256":format!("{:x}",Sha256::digest(artifact)),
        "dependencies":[],"tools":["publisher.worker.read"]
    })
    .to_string();
    let mut message = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
    message.extend_from_slice(&(13u32).to_be_bytes());
    message.extend_from_slice(b"publisher-key");
    message.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    message.extend_from_slice(manifest.as_bytes());
    let signature = signing().sign(&message);
    serde_json::json!({
        "format_version":1,"key_id":"publisher-key",
        "manifest_json":manifest,
        "signature_hex":signature.to_bytes().iter()
            .map(|b|format!("{b:02x}")).collect::<String>()
    })
    .to_string()
}

fn private_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

#[test]
fn bound_profile_identity_persists_and_denies_cross_profile_grants() {
    let a = profile("a");
    let b = profile("b");
    let identity_a = CePermissionStore::host_profile_id_for_folder(&a).unwrap();
    let identity_b = CePermissionStore::host_profile_id_for_folder(&b).unwrap();
    assert_ne!(identity_a, identity_b);
    assert!(identity_a.starts_with("lu."));
    let store = CePermissionStore::open_for_host_profile(&a).unwrap();
    store.require_host_profile(&a, &identity_a).unwrap();
    assert_eq!(
        store.require_host_profile(&a, &identity_b),
        Err(CePermissionError::Denied)
    );
    assert_eq!(
        store.require_host_profile(&b, &identity_a),
        Err(CePermissionError::Denied)
    );
    enroll(&store);
    let package = store.verify_signed_package(&envelope(), b"worker").unwrap();
    let grant = CeGrantRequest {
        profile_id: identity_a.clone(),
        scope_id: "scope".into(),
        tool_id: "publisher.worker.read".into(),
        effect: ToolEffect::ReadWorkspace,
        approved_user_turn_id: "actual-human-turn".into(),
        expires_at: i64::MAX,
    };
    store.approve(&package, &grant).unwrap();
    store
        .require(
            &package,
            &identity_a,
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
        )
        .unwrap();
    assert_eq!(
        store.require(
            &package,
            &identity_b,
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace
        ),
        Err(CePermissionError::Denied)
    );
    let mut wrong_profile = grant.clone();
    wrong_profile.profile_id = identity_b.clone();
    wrong_profile.scope_id = "scope-two".into();
    assert_eq!(
        store.approve(&package, &wrong_profile),
        Err(CePermissionError::Denied)
    );
    drop(store);
    let reopened = CePermissionStore::open_for_host_profile(&a).unwrap();
    reopened.require_host_profile(&a, &identity_a).unwrap();
    reopened
        .require(
            &package,
            &identity_a,
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
        )
        .unwrap();
    drop(reopened);
    fs::remove_dir_all(a).unwrap();
    fs::remove_dir_all(b).unwrap();
}

#[test]
fn copied_foreign_database_and_alternate_path_are_not_approved() {
    let a = profile("original");
    let b = profile("copied");
    let store = CePermissionStore::open_for_host_profile(&a).unwrap();
    let identity = CePermissionStore::host_profile_id_for_folder(&a).unwrap();
    enroll(&store);
    drop(store);
    // A different file, even inside the same private profile, cannot be
    // passed to the native desktop consent adapter in place of its fixed DB.
    let alternate = CePermissionStore::open(&a.join("lu-ce").join("alternate.sqlite")).unwrap();
    assert_eq!(
        alternate.require_host_profile(&a, &identity),
        Err(CePermissionError::Denied)
    );
    drop(alternate);
    private_dir(&b.join("lu-ce"));
    fs::copy(
        a.join("lu-ce").join("permissions.sqlite"),
        b.join("lu-ce").join("permissions.sqlite"),
    )
    .unwrap();
    assert!(matches!(
        CePermissionStore::open_for_host_profile(&b),
        Err(CePermissionError::InsecureStorage)
    ));
    fs::remove_dir_all(a).unwrap();
    fs::remove_dir_all(b).unwrap();
}

#[test]
fn unbound_prior_permissions_are_not_silently_imported() {
    let root = profile("legacy");
    let subdir = root.join("lu-ce");
    private_dir(&subdir);
    let store = CePermissionStore::open(&subdir.join("permissions.sqlite")).unwrap();
    enroll(&store);
    drop(store);
    assert!(matches!(
        CePermissionStore::open_for_host_profile(&root),
        Err(CePermissionError::InsecureStorage)
    ));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlinked_profile_or_ce_subdirectory_is_rejected() {
    let source = profile("actual");
    let link_parent = profile("linkholder");
    let alias = link_parent.join("profile-link");
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    assert!(matches!(
        CePermissionStore::open_for_host_profile(&alias),
        Err(CePermissionError::InsecureStorage)
    ));
    let target = profile("target");
    std::os::unix::fs::symlink(target.join("lu-ce"), source.join("lu-ce")).unwrap();
    assert!(matches!(
        CePermissionStore::open_for_host_profile(&source),
        Err(CePermissionError::InsecureStorage)
    ));
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(link_parent).unwrap();
    fs::remove_dir_all(target).unwrap();
}
