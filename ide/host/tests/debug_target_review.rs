use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn commit_tree(path: &Path) {
    let repo = if let Ok(repo) = git2::Repository::open(path) {
        repo
    } else {
        git2::Repository::init(path).unwrap()
    };
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let oid = index.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();
    let signature = git2::Signature::now("Debug Fixture", "ide@example.invalid").unwrap();
    let previous = repo
        .head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parents: Vec<_> = previous.iter().collect();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "test",
        &tree,
        &parents,
    )
    .unwrap();
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-dev041-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        b"[package]\nname=\"debug_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        b"fn main() { panic!(\"must not execute on review\"); }\n",
    )
    .unwrap();
    commit_tree(&project);
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(project)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}
#[test]
fn exact_native_source_review_never_promotes_binary_or_debugger_rights() {
    let (root, registry, id) = fixture();
    let sha = registry
        .preview_source_file(&id, "Cargo.toml")
        .unwrap()
        .revision;
    let result = registry.review_project_debug_target(&id, &sha).unwrap();
    assert_eq!(result.project_id, id);
    assert_eq!(result.manifest_relative_path, "Cargo.toml");
    assert_eq!(result.clean_git_head.len(), 40);
    assert_eq!(result.review_fingerprint.len(), 64);
    assert!(result.target_executable.is_none());
    assert_eq!(result.binary_identity_state, "no_binary_sha_or_provenance");
    assert!(!result.binary_executed);
    assert!(!result.worker_grant_issued);
    assert!(!result.debugger_authorized);
    assert!(!result.installed_app_accepted);
    let exported = serde_json::to_string(&result).unwrap();
    assert!(!exported.contains("must not execute"));
    assert!(!exported.contains(root.to_str().unwrap()));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn dirty_stale_unregistered_and_closed_project_are_denied() {
    let (root, registry, id) = fixture();
    let sha = registry
        .preview_source_file(&id, "Cargo.toml")
        .unwrap()
        .revision;
    assert_eq!(
        registry
            .review_project_debug_target(&id, &"0".repeat(64))
            .unwrap_err(),
        "DEV041_STALE_MANIFEST"
    );
    assert_eq!(
        registry
            .review_project_debug_target(&id, "../../")
            .unwrap_err(),
        "DEV041_MANIFEST_SHA_INVALID"
    );
    fs::write(root.join("project/src/main.rs"), b"fn main(){}").unwrap();
    assert_eq!(
        registry.review_project_debug_target(&id, &sha).unwrap_err(),
        "DEV041_CLEAN_GIT_REQUIRED"
    );
    registry.close(&id).unwrap();
    assert_eq!(
        registry.review_project_debug_target(&id, &sha).unwrap_err(),
        "DEV041_PROJECT_NOT_ACTIVE"
    );
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn symlinked_cargo_manifest_cannot_become_debug_target() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    let sha = registry
        .preview_source_file(&id, "Cargo.toml")
        .unwrap()
        .revision;
    fs::write(root.join("secret.toml"), b"SECRET").unwrap();
    fs::remove_file(root.join("project/Cargo.toml")).unwrap();
    symlink(root.join("secret.toml"), root.join("project/Cargo.toml")).unwrap();
    commit_tree(&root.join("project"));
    assert_eq!(
        registry.review_project_debug_target(&id, &sha).unwrap_err(),
        "DEV041_MANIFEST_UNREADABLE"
    );
    fs::remove_dir_all(root).unwrap();
}
