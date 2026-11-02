//! Adversarial staged executable tests: no symlink traversal, no replacement
//! of an existing artifact and no silent adoption of a new inode with identical
//! signed bytes.
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signer, SigningKey};
use lu_core::{CeExecutableStager, CePublisherTrust, CeStageError, VerifiedCePackage};
use sha2::{Digest, Sha256};

fn fixture_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "lu-ce-staging-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    }
    root
}

fn artifact() -> Vec<u8> {
    #[cfg(target_os = "linux")]
    {
        b"\x7fELF-verified-test-worker".to_vec()
    }
    #[cfg(target_os = "windows")]
    {
        b"MZ-verified-test-worker".to_vec()
    }
    #[cfg(target_os = "macos")]
    {
        b"\xfe\xed\xfa\xcf-verified-test-worker".to_vec()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        b"native-verified-test-worker".to_vec()
    }
}

fn verified() -> VerifiedCePackage {
    let signer = SigningKey::from_bytes(&[73u8; 32]);
    let bytes = artifact();
    let mut trust = CePublisherTrust::new();
    trust
        .pin_key(
            "staging-key",
            "Publisher",
            signer.verifying_key().to_bytes(),
            0,
            1000,
        )
        .unwrap();
    let manifest = serde_json::json!({
        "format_version": 1,
        "id": "publisher.staging",
        "version": "1.0.0",
        "contract_version": 1,
        "publisher": "Publisher",
        "artifact_sha256": format!("{:x}", Sha256::digest(&bytes)),
        "dependencies": [],
        "tools": ["publisher.staging.run"]
    })
    .to_string();
    let mut signed = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
    signed.extend_from_slice(&("staging-key".len() as u32).to_be_bytes());
    signed.extend_from_slice(b"staging-key");
    signed.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    signed.extend_from_slice(manifest.as_bytes());
    let signature = signer.sign(&signed);
    let envelope = serde_json::json!({
        "format_version": 1,
        "key_id": "staging-key",
        "manifest_json": manifest,
        "signature_hex": signature
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    })
    .to_string();
    trust.verify_at(&envelope, &bytes, 200).unwrap()
}

#[test]
fn staging_is_reproducible_and_clones_keep_verified_file_pinned() {
    let root = fixture_root("repeat");
    let stager = CeExecutableStager::new(&root).unwrap();
    let package = verified();
    let original = stager.stage(package.clone()).unwrap();
    let duplicate = stager.stage(package).unwrap();
    assert!(original.verify_on_disk().is_ok());
    assert!(duplicate.verify_on_disk().is_ok());
    assert_eq!(original.host_path(), duplicate.host_path());
    let retained = original.clone();
    drop(original);
    assert!(retained.verify_on_disk().is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parallel_staging_never_overwrites_another_published_worker() {
    let root = fixture_root("parallel");
    let stager = Arc::new(CeExecutableStager::new(&root).unwrap());
    let package = verified();
    let mut threads = Vec::new();
    for _ in 0..8 {
        let stager = Arc::clone(&stager);
        let package = package.clone();
        threads.push(std::thread::spawn(move || stager.stage(package)));
    }
    let staged: Vec<_> = threads
        .into_iter()
        .map(|t| t.join().unwrap().unwrap())
        .collect();
    for value in &staged {
        assert_eq!(value.host_path(), staged[0].host_path());
        assert!(value.verify_on_disk().is_ok());
    }
    let contents: Vec<_> = fs::read_dir(staged[0].host_path().parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(contents.len(), 1, "no abandoned pending worker files");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn in_place_payload_edit_is_denied_even_when_size_matches() {
    let root = fixture_root("changed");
    let staged = CeExecutableStager::new(&root)
        .unwrap()
        .stage(verified())
        .unwrap();
    let path = staged.host_path();
    let mut changed = artifact();
    *changed.last_mut().unwrap() ^= 0x01;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(path, &changed).unwrap();
    assert_eq!(staged.verify_on_disk(), Err(CeStageError::ChangedOnDisk));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn replacement_with_identical_signed_bytes_is_denied_by_inode_pin() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let root = fixture_root("replacement");
    let staged = CeExecutableStager::new(&root)
        .unwrap()
        .stage(verified())
        .unwrap();
    let path = staged.host_path();
    let former_ino = fs::metadata(path).unwrap().ino();
    let replacement = path.with_extension("replacement");
    fs::write(&replacement, artifact()).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o500)).unwrap();
    fs::rename(&replacement, path).unwrap();
    assert_ne!(fs::metadata(path).unwrap().ino(), former_ino);
    assert_eq!(staged.verify_on_disk(), Err(CeStageError::ChangedOnDisk));
    // Even a new staging call may independently adopt the exact signed
    // bytes; the previous active runtime clone cannot change identity.
    assert!(CeExecutableStager::new(&root)
        .unwrap()
        .stage(verified())
        .unwrap()
        .verify_on_disk()
        .is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlinked_worker_and_digest_directory_are_never_accepted() {
    let root = fixture_root("symlinks");
    let outside = fixture_root("outside");
    let package = verified();
    let digest = package.artifact_sha256();
    let folder = root.join(digest);
    std::os::unix::fs::symlink(&outside, &folder).unwrap();
    let stager = CeExecutableStager::new(&root).unwrap();
    assert!(matches!(
        stager.stage(package.clone()),
        Err(CeStageError::InvalidRoot)
    ));
    fs::remove_file(&folder).unwrap();

    fs::create_dir(&folder).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
    let target = outside.join("file");
    fs::write(&target, artifact()).unwrap();
    std::os::unix::fs::symlink(&target, folder.join("worker")).unwrap();
    assert!(matches!(
        stager.stage(package),
        Err(CeStageError::ChangedOnDisk)
    ));
    assert_eq!(fs::read(&target).unwrap(), artifact());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn swapping_digest_directory_with_hardlink_to_same_worker_is_denied() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture_root("digest-identity");
    let staged = CeExecutableStager::new(&root)
        .unwrap()
        .stage(verified())
        .unwrap();
    let dir = staged.host_path().parent().unwrap();
    let displaced = root.join("displaced-digest");
    fs::rename(dir, &displaced).unwrap();
    fs::create_dir(dir).unwrap();
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
    fs::hard_link(displaced.join("worker"), staged.host_path()).unwrap();

    // The signed bytes and executable inode are unchanged; the parent
    // directory inode alone is different. This must not transfer authority.
    assert_eq!(fs::read(staged.host_path()).unwrap(), artifact());
    assert_eq!(staged.verify_on_disk(), Err(CeStageError::ChangedOnDisk));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn swapping_staging_root_with_hardlink_to_same_worker_is_denied() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture_root("root-identity");
    let staged = CeExecutableStager::new(&root)
        .unwrap()
        .stage(verified())
        .unwrap();
    let digest = staged.package().artifact_sha256().to_owned();
    let displaced = root.with_extension("displaced");
    fs::rename(&root, &displaced).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let replacement_dir = root.join(&digest);
    fs::create_dir(&replacement_dir).unwrap();
    fs::set_permissions(&replacement_dir, fs::Permissions::from_mode(0o700)).unwrap();
    fs::hard_link(displaced.join(&digest).join("worker"), staged.host_path()).unwrap();

    assert_eq!(fs::read(staged.host_path()).unwrap(), artifact());
    assert_eq!(staged.verify_on_disk(), Err(CeStageError::ChangedOnDisk));
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(displaced).unwrap();
}

#[cfg(unix)]
#[test]
fn writable_existing_worker_mode_is_denied_even_with_valid_signature() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture_root("writable-mode");
    let package = verified();
    let staged = CeExecutableStager::new(&root)
        .unwrap()
        .stage(package.clone())
        .unwrap();
    fs::set_permissions(staged.host_path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(staged.verify_on_disk(), Err(CeStageError::ChangedOnDisk));
    assert!(matches!(
        CeExecutableStager::new(&root).unwrap().stage(package),
        Err(CeStageError::ChangedOnDisk)
    ));
    fs::set_permissions(staged.host_path(), fs::Permissions::from_mode(0o500)).unwrap();
    assert!(staged.verify_on_disk().is_ok());
    fs::remove_dir_all(root).unwrap();
}
