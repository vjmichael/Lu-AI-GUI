use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root=std::env::temp_dir().join(format!(
        "lu-ide-dev03-{}-{:016x}",std::process::id(),rand::random::<u64>()
    ));
    let project=root.join("project");
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("Cargo.toml"),b"[package]\nname=\"example\"\nversion=\"0.1.0\"\n").unwrap();
    fs::write(project.join("pyproject.toml"),b"[tool.pytest.ini_options]\n").unwrap();
    fs::write(project.join("package.json"),b"{\"scripts\":{\"test\":\"dangerous shell\"}}").unwrap();
    fs::write(project.join("CMakeLists.txt"),b"execute_process(COMMAND unknown)\n").unwrap();
    fs::write(project.join("tests/example.rs"),b"#[test]\nfn sample(){}\n").unwrap();
    fs::write(project.join("tests/test_calc.py"),b"def test_one():\n  assert True\n").unwrap();
    fs::write(project.join("tests/unit.test.ts"),b"it('works',()=>{})\n").unwrap();
    fs::write(project.join("tests/RendererTest.cpp"),b"int main(){return 0;}\n").unwrap();
    let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id=registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    (root,registry,id)
}
#[test]
fn discovers_real_saved_manifests_without_using_untrusted_script_content() {
    let(root,registry,id)=fixture();
    let result=registry.discover_project_run_profiles(&id).unwrap();
    assert_eq!(result.schema_version,1);
    assert_eq!(result.project_id,id);
    assert_eq!(result.profiles.len(),7);
    assert!(!result.manifest_contents_exported);
    assert!(!result.process_started);
    assert!(!result.worker_grant_issued);
    assert!(!result.task_accepted);
    for p in &result.profiles {
        assert_eq!(p.manifest_state,"saved_file_sha_observed_not_validated");
        assert_eq!(p.manifest_sha256.as_deref().unwrap().len(),64);
        assert!(!p.execution_authorized);
        assert!(!p.title.contains("dangerous"));
    }
    let rust=&result.profiles[0];
    assert_eq!(rust.profile_id,"rust_check");
    assert_eq!(rust.existing_native_recipe_id,Some("rust_check"));
    assert_eq!(result.profiles[1].test_candidates,1);
    assert_eq!(result.profiles[2].test_candidates,1);
    assert_eq!(result.profiles[3].test_candidates,1);
    assert_eq!(result.profiles[4].test_candidates,0);
    assert_eq!(result.profiles[6].profile_id,"rust_build");
    assert_eq!(result.profiles[6].existing_native_recipe_id,Some("rust_build"));
    assert_eq!(result.profiles[2].test_hint_paths,vec!["tests/test_calc.py"]);
    assert_eq!(result.profiles[3].test_hint_paths,vec!["tests/unit.test.ts"]);
    assert!(result.profiles[2].existing_native_recipe_id.is_none());
    assert!(result.profiles[3].existing_native_recipe_id.is_none());
    assert!(result.profiles[4].existing_native_recipe_id.is_none());
    assert!(!format!("{result:?}").contains("dangerous shell"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn saved_manifest_sha_changes_on_disk_but_discovery_never_executes_it() {
    let(root,registry,id)=fixture();
    let before=registry.discover_project_run_profiles(&id).unwrap();
    fs::write(root.join("project/Cargo.toml"),b"[package]\nname=\"changed\"\nversion=\"1.0\"\n").unwrap();
    let after=registry.discover_project_run_profiles(&id).unwrap();
    assert_ne!(before.profiles[0].manifest_sha256,after.profiles[0].manifest_sha256);
    assert_eq!(after.profiles[0].manifest_sha256,after.profiles[1].manifest_sha256);
    assert!(!after.process_started);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_or_oversized_manifests_never_promote_execution() {
    let(root,registry,id)=fixture();
    fs::remove_file(root.join("project/Cargo.toml")).unwrap();
    fs::write(root.join("project/package.json"),vec![b'x';513*1024]).unwrap();
    let report=registry.discover_project_run_profiles(&id).unwrap();
    assert_eq!(report.profiles[0].manifest_state,"not_found");
    assert!(report.profiles[0].manifest_sha256.is_none());
    assert_eq!(report.profiles[3].manifest_state,"not_readable_or_unverifiable");
    assert!(report.profiles[3].manifest_sha256.is_none());
    assert!(!report.profiles[0].execution_authorized);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn bounded_conventional_test_hints_never_claim_complete_discovery() {
    let(root,registry,id)=fixture();
    for i in 0..36 {
        fs::write(root.join("project/tests").join(format!("test_{i}.py")),b"def test():pass\n")
            .unwrap();
    }
    let report=registry.discover_project_run_profiles(&id).unwrap();
    let py=&report.profiles[2];
    assert_eq!(py.test_candidates,37);
    assert_eq!(py.test_hint_paths.len(),20);
    assert_eq!(py.test_scan_state,"partial_conventional_tests_only");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn closed_and_unknown_project_are_denied() {
    let(root,registry,id)=fixture();
    assert!(registry.discover_project_run_profiles("ide-ffffffffffffffffffffffffffffffff").is_err());
    registry.close(&id).unwrap();
    assert!(registry.discover_project_run_profiles(&id).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn manifest_symlink_is_reported_unverifiable_without_following_it() {
    use std::os::unix::fs::symlink;
    let(root,registry,id)=fixture();
    fs::write(root.join("outside.toml"),b"SECRET_SCRIPT_CONTENT").unwrap();
    fs::remove_file(root.join("project/Cargo.toml")).unwrap();
    symlink(root.join("outside.toml"),root.join("project/Cargo.toml")).unwrap();
    let report=registry.discover_project_run_profiles(&id).unwrap();
    assert_eq!(report.profiles[0].manifest_state,"not_readable_or_unverifiable");
    assert!(report.profiles[0].manifest_sha256.is_none());
    fs::remove_dir_all(root).unwrap();
}
