//! Durable publisher trust regression tests: only an enrolled signer may
//! obtain a grant, rotation never inherits grants, and restart fails closed.
use ed25519_dalek::{Signer, SigningKey};
use lu_core::{
    CeGrantRequest, CePermissionError, CePermissionStore, CePublisherKeyEnrollment,
    CePublisherTrust, ToolEffect,
};
use sha2::{Digest, Sha256};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn root(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lu_trust_{}_{}_{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn enrollment(key_id: &str, signing: &SigningKey) -> CePublisherKeyEnrollment {
    CePublisherKeyEnrollment {
        key_id: key_id.into(),
        publisher: "Publisher".into(),
        public_key: signing.verifying_key().to_bytes(),
        not_before: 0,
        expires_at: i64::MAX,
    }
}

fn signed(key_id: &str, signing: &SigningKey) -> String {
    let artifact = b"trusted-worker";
    let manifest = serde_json::json!({
        "format_version":1,"id":"publisher.worker","version":"1.0.0",
        "contract_version":1,"publisher":"Publisher",
        "artifact_sha256":format!("{:x}",Sha256::digest(artifact)),
        "dependencies":[],"tools":["publisher.worker.read"]
    })
    .to_string();
    let mut message = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
    message.extend_from_slice(&(key_id.len() as u32).to_be_bytes());
    message.extend_from_slice(key_id.as_bytes());
    message.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    message.extend_from_slice(manifest.as_bytes());
    let signature = signing.sign(&message);
    serde_json::json!({
        "format_version":1, "key_id":key_id, "manifest_json":manifest,
        "signature_hex":signature.to_bytes().iter()
            .map(|b|format!("{b:02x}")).collect::<String>()
    })
    .to_string()
}

fn grant(scope: &str) -> CeGrantRequest {
    CeGrantRequest {
        profile_id: "profile".into(),
        scope_id: scope.into(),
        tool_id: "publisher.worker.read".into(),
        effect: ToolEffect::ReadWorkspace,
        approved_user_turn_id: "authenticated-turn".into(),
        expires_at: i64::MAX,
    }
}

#[test]
fn enrollment_signature_verification_rotation_and_restart_are_coherent() {
    let root = root("rotation");
    let path = root.join("permission.db");
    let old = SigningKey::from_bytes(&[21; 32]);
    let new = SigningKey::from_bytes(&[22; 32]);
    let old_envelope = signed("old-key", &old);
    let new_envelope = signed("new-key", &new);
    let store = CePermissionStore::open(&path).unwrap();
    store
        .enroll_publisher_key(&enrollment("old-key", &old))
        .unwrap();
    assert_eq!(
        store.enroll_publisher_key(&enrollment("old-key", &old)),
        Err(CePermissionError::AlreadyRecorded)
    );
    let original = store
        .verify_signed_package(&old_envelope, b"trusted-worker")
        .unwrap();
    store.approve(&original, &grant("scope-a")).unwrap();
    store.approve(&original, &grant("scope-b")).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let recorded = observed.clone();
    store
        .subscribe_revocations(Arc::new(move |event| {
            recorded.lock().unwrap().push(event.clone())
        }))
        .unwrap();

    assert_eq!(
        store
            .rotate_publisher_key("old-key", &enrollment("new-key", &new))
            .unwrap(),
        2
    );
    assert!(store.is_publisher_key_revoked("old-key").unwrap());
    assert!(store
        .verify_signed_package(&old_envelope, b"trusted-worker")
        .is_err());
    assert!(store
        .verify_signed_package(&new_envelope, b"trusted-worker")
        .is_ok());
    assert_eq!(observed.lock().unwrap().len(), 2);
    for scope in ["scope-a", "scope-b"] {
        assert_eq!(
            store.require(
                &original,
                "profile",
                scope,
                "publisher.worker.read",
                ToolEffect::ReadWorkspace
            ),
            Err(CePermissionError::Denied)
        );
    }
    let successor = store
        .verify_signed_package(&new_envelope, b"trusted-worker")
        .unwrap();
    assert_eq!(
        store.require(
            &successor,
            "profile",
            "scope-a",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace
        ),
        Err(CePermissionError::Denied),
        "rotation must not migrate a prior grant"
    );
    assert_eq!(
        store.approve(&successor, &grant("scope-a")),
        Err(CePermissionError::AlreadyRecorded),
        "the original user turn cannot approve the successor"
    );
    let mut renewed = grant("scope-a");
    renewed.approved_user_turn_id = "new-explicit-user-turn".into();
    store.approve(&successor, &renewed).unwrap();
    store
        .require(
            &successor,
            "profile",
            "scope-a",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
        )
        .unwrap();
    assert_eq!(store.revoke_publisher_key("new-key").unwrap(), 1);
    assert_eq!(store.revoke_publisher_key("new-key").unwrap(), 0);
    assert_eq!(
        store.enroll_publisher_key(&enrollment("new-key", &new)),
        Err(CePermissionError::AlreadyRecorded)
    );
    drop(store);
    let reopened = CePermissionStore::open(&path).unwrap();
    assert!(reopened.is_publisher_key_revoked("old-key").unwrap());
    assert!(reopened.is_publisher_key_revoked("new-key").unwrap());
    assert!(reopened
        .verify_signed_package(&new_envelope, b"trusted-worker")
        .is_err());
    assert_eq!(
        reopened.approve(&successor, &grant("scope-c")),
        Err(CePermissionError::Denied)
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn signer_public_key_binding_blocks_key_id_spoofing() {
    let root = root("spoof");
    let store = CePermissionStore::open(&root.join("permissions.db")).unwrap();
    let real = SigningKey::from_bytes(&[33; 32]);
    let impostor = SigningKey::from_bytes(&[34; 32]);
    store
        .enroll_publisher_key(&enrollment("publisher-key", &real))
        .unwrap();
    let spoofed_envelope = signed("publisher-key", &impostor);
    assert!(store
        .verify_signed_package(&spoofed_envelope, b"trusted-worker")
        .is_err());
    // Deliberately verify using a *different* in-memory trust identity:
    // a package carrying the same string ID cannot borrow the durable grant.
    let mut untrusted = CePublisherTrust::new();
    untrusted
        .pin_key(
            "publisher-key",
            "Publisher",
            impostor.verifying_key().to_bytes(),
            0,
            u64::MAX,
        )
        .unwrap();
    let spoofed_package = untrusted
        .verify(&spoofed_envelope, b"trusted-worker")
        .unwrap();
    assert_eq!(
        store.approve(&spoofed_package, &grant("scope")),
        Err(CePermissionError::Denied)
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn restart_recovers_grants_for_missing_or_expired_trust() {
    let root = root("recover");
    let path = root.join("permissions.db");
    let signing = SigningKey::from_bytes(&[36; 32]);
    let package_json = signed("key", &signing);
    let store = CePermissionStore::open(&path).unwrap();
    store
        .enroll_publisher_key(&enrollment("key", &signing))
        .unwrap();
    let package = store
        .verify_signed_package(&package_json, b"trusted-worker")
        .unwrap();
    store.approve(&package, &grant("scope")).unwrap();
    drop(store);
    // Simulate an interrupted migration with an inactive persisted identity.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE ce_publisher_keys SET state='retired' WHERE key_id='key'",
        [],
    )
    .unwrap();
    drop(conn);
    let recovered = CePermissionStore::open(&path).unwrap();
    assert_eq!(
        recovered.require(
            &package,
            "profile",
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace
        ),
        Err(CePermissionError::Denied)
    );
    assert_eq!(recovered.recover_publisher_trust().unwrap(), 0);
    assert!(recovered.is_publisher_key_revoked("key").unwrap());
    drop(recovered);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn grant_revocation_cannot_overtake_an_inflight_authorized_effect() {
    let root = root("lease_revoke");
    let path = root.join("permission.db");
    let signing = SigningKey::from_bytes(&[45; 32]);
    let store = CePermissionStore::open(&path).unwrap();
    store
        .enroll_publisher_key(&enrollment("key", &signing))
        .unwrap();
    let package = store
        .verify_signed_package(&signed("key", &signing), b"trusted-worker")
        .unwrap();
    store.approve(&package, &grant("scope")).unwrap();
    // A separate handle means a different SQLite connection. The IMMEDIATE
    // transaction fences even out-of-process revocation writers using this DB.
    let revoker = CePermissionStore::open(&path).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker_store = store.clone();
    let worker_package = package.clone();
    let worker = thread::spawn(move || {
        worker_store.with_authorized_grant(
            &worker_package,
            "profile",
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
            || {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                17_u32
            },
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (revoked_tx, revoked_rx) = mpsc::channel();
    let revoke_thread = thread::spawn(move || {
        let result = revoker.revoke(
            "profile",
            "publisher.worker",
            "scope",
            Some("publisher.worker.read"),
        );
        revoked_tx.send(result).unwrap();
    });
    // The second connection must not commit a revoke until the previously
    // authorized host effect releases its SQLite lease.
    assert!(revoked_rx.recv_timeout(Duration::from_millis(75)).is_err());
    release_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Ok(17));
    revoked_rx
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    revoke_thread.join().unwrap();
    assert_eq!(
        store.with_authorized_grant(
            &package,
            "profile",
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
            || 23
        ),
        Err(CePermissionError::Denied)
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn publisher_revocation_fences_cross_connection_operations() {
    let root = root("lease_key_revoke");
    let path = root.join("permission.db");
    let signing = SigningKey::from_bytes(&[46; 32]);
    let store = CePermissionStore::open(&path).unwrap();
    store
        .enroll_publisher_key(&enrollment("key", &signing))
        .unwrap();
    let package = store
        .verify_signed_package(&signed("key", &signing), b"trusted-worker")
        .unwrap();
    store.approve(&package, &grant("scope")).unwrap();
    let revoker = CePermissionStore::open(&path).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker_store = store.clone();
    let worker_package = package.clone();
    let worker = thread::spawn(move || {
        worker_store.with_authorized_grant(
            &worker_package,
            "profile",
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
            || {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            },
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (revoked_tx, revoked_rx) = mpsc::channel();
    let revoke_thread = thread::spawn(move || {
        revoked_tx
            .send(revoker.revoke_publisher_key("key"))
            .unwrap();
    });
    assert!(revoked_rx.recv_timeout(Duration::from_millis(75)).is_err());
    release_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Ok(()));
    assert_eq!(
        revoked_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(1)
    );
    revoke_thread.join().unwrap();
    assert_eq!(
        store.with_authorized_grant(
            &package,
            "profile",
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace,
            || 1
        ),
        Err(CePermissionError::Denied)
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
