//! A desktop confirmation cannot create a grant after its durable expiry
//! merely because approval entered the host before another SQLite writer
//! released the admission lock.
use ed25519_dalek::{Signer, SigningKey};
use lu_core::{
    CeGrantRequest, CePermissionError, CePermissionStore, CePublisherKeyEnrollment, ToolEffect,
};
use sha2::{Digest, Sha256};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn queued_approval_cannot_commit_after_grant_expires_during_sqlite_contention() {
    let root = std::env::temp_dir().join(format!(
        "lu_ce_approval_expiry_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = root.join("permissions.db");
    let store = CePermissionStore::open(&path).unwrap();
    let signing = SigningKey::from_bytes(&[79u8; 32]);
    store
        .enroll_publisher_key(&CePublisherKeyEnrollment {
            key_id: "approval-key".into(),
            publisher: "Publisher".into(),
            public_key: signing.verifying_key().to_bytes(),
            not_before: 0,
            expires_at: i64::MAX,
        })
        .unwrap();

    let artifact = b"signed-test-worker";
    let manifest = serde_json::json!({
        "format_version": 1,
        "id": "publisher.worker",
        "version": "1.0.0",
        "contract_version": 1,
        "publisher": "Publisher",
        "artifact_sha256": format!("{:x}", Sha256::digest(artifact)),
        "dependencies": [],
        "tools": ["publisher.worker.read"]
    })
    .to_string();
    let mut message = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
    message.extend_from_slice(&("approval-key".len() as u32).to_be_bytes());
    message.extend_from_slice(b"approval-key");
    message.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    message.extend_from_slice(manifest.as_bytes());
    let signature = signing.sign(&message);
    let envelope = serde_json::json!({
        "format_version": 1,
        "key_id": "approval-key",
        "manifest_json": manifest,
        "signature_hex": signature.to_bytes().iter()
            .map(|b| format!("{b:02x}")).collect::<String>()
    })
    .to_string();
    let package = store.verify_signed_package(&envelope, artifact).unwrap();

    // Independent SQLite writer holds the durable admission lock. The
    // contender begins while the grant is live, but must be evaluated at
    // actual commit admission, not at the pre-wait timestamp.
    let blocker = rusqlite::Connection::open(&path).unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let expiry = now_secs() + 3;
    let grant = CeGrantRequest {
        profile_id: "profile".into(),
        scope_id: "scope".into(),
        tool_id: "publisher.worker.read".into(),
        effect: ToolEffect::ReadWorkspace,
        approved_user_turn_id: "real-user-turn".into(),
        expires_at: expiry,
    };
    let contender = store.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let approval = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let outcome = contender.approve(&package, &grant);
        (outcome, package)
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    thread::sleep(Duration::from_millis(100));
    while now_secs() < expiry {
        thread::sleep(Duration::from_millis(25));
    }
    blocker.execute_batch("COMMIT").unwrap();

    let (result, package) = approval.join().unwrap();
    assert_eq!(result, Err(CePermissionError::InvalidGrant));
    assert_eq!(
        store.require(
            &package,
            "profile",
            "scope",
            "publisher.worker.read",
            ToolEffect::ReadWorkspace
        ),
        Err(CePermissionError::Denied)
    );
    drop(blocker);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
