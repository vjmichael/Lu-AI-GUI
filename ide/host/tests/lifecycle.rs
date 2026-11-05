use lu_ide_workspace::wire::proto::{self, workspace_reply::Outcome, workspace_request::Operation};
use lu_ide_workspace::{wire, IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Picker {
    selected: Option<PathBuf>,
    calls: AtomicUsize,
}
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.selected.take())
    }
}
fn temp() -> PathBuf {
    let random: u64 = rand::random();
    let dir =
        std::env::temp_dir().join(format!("lu-ide-test-{}-{random:016x}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}
fn request(op: Operation) -> Vec<u8> {
    wire::encode_request(&proto::WorkspaceRequest {
        protocol_version: wire::VERSION,
        request_id: "ui-1".into(),
        operation: Some(op),
    })
    .unwrap()
}
fn response(raw: &[u8]) -> proto::WorkspaceReply {
    wire::decode_reply(raw).unwrap()
}

#[test]
fn selects_closes_and_reopens_after_registry_restart_without_running_code() {
    let root = temp();
    let project = root.join("example");
    fs::create_dir(&project).unwrap();
    // Repo-controlled build script must NEVER execute during project intake.
    fs::write(
        project.join("build.rs"),
        "potentially malicious build script",
    )
    .unwrap();
    let catalog = root.join("state").join("projects.json");
    let mut picker = Picker {
        selected: Some(project.clone()),
        calls: AtomicUsize::new(0),
    };
    let id = {
        let ide = IdeWorkspaceRegistry::open(catalog.clone()).unwrap();
        let opened = response(
            &ide.handle_ui_request(
                &request(Operation::Select(proto::SelectDirectory {})),
                &mut picker,
            )
            .unwrap(),
        );
        let view = match opened.outcome.unwrap() {
            Outcome::Project(view) => view,
            other => panic!("expected project: {other:?}"),
        };
        assert!(view.active);
        assert!(view.available);
        assert_eq!(view.display_name, "example");
        assert!(wire::valid_project_id(&view.project_id));
        assert_eq!(picker.calls.load(Ordering::SeqCst), 1);
        assert_eq!(ide.projects().unwrap().len(), 1);
        ide.close(&view.project_id).unwrap();
        assert!(!ide.projects().unwrap()[0].active);
        view.project_id
    };
    let ide = IdeWorkspaceRegistry::open(catalog).unwrap();
    assert!(!ide.projects().unwrap()[0].active);
    assert!(ide.reopen(&id).unwrap().active);
    assert!(ide.reopen(&id).is_err()); // No duplicate live instance.
    assert!(!project.join("marker-executed").exists());
    // The UI response must not expose root paths.
    let raw = ide
        .handle_ui_request(
            &request(Operation::List(proto::ListProjects {})),
            &mut picker,
        )
        .unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains(&project.display().to_string()));
    assert_eq!(picker.calls.load(Ordering::SeqCst), 1); // Only Select opens picker.
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancelled_picker_does_not_register_or_write_catalog() {
    let root = temp();
    let catalog = root.join("projects.json");
    let ide = IdeWorkspaceRegistry::open(catalog.clone()).unwrap();
    let mut picker = Picker {
        selected: None,
        calls: AtomicUsize::new(0),
    };
    let reply = response(
        &ide.handle_ui_request(
            &request(Operation::Select(proto::SelectDirectory {})),
            &mut picker,
        )
        .unwrap(),
    );
    assert!(matches!(reply.outcome, Some(Outcome::Projects(_))));
    assert!(!catalog.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_or_changed_project_is_denied_on_reopen() {
    let root = temp();
    let project = root.join("selected");
    fs::create_dir(&project).unwrap();
    let ide = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let mut picker = Picker {
        selected: Some(project.clone()),
        calls: AtomicUsize::new(0),
    };
    let id = ide
        .register_from_native_picker(&mut picker)
        .unwrap()
        .unwrap()
        .project_id;
    ide.close(&id).unwrap();
    fs::rename(&project, root.join("moved")).unwrap();
    assert!(ide.reopen(&id).is_err());
    assert!(!ide.projects().unwrap()[0].available);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn persistence_failure_does_not_leave_live_workspace() {
    let root = temp();
    let project = root.join("selected");
    fs::create_dir(&project).unwrap();
    let catalog = root.join("catalog.json");
    let ide = IdeWorkspaceRegistry::open(catalog.clone()).unwrap();
    // A directory at catalog target prevents atomic replacement.
    fs::create_dir(&catalog).unwrap();
    let mut picker = Picker {
        selected: Some(project),
        calls: AtomicUsize::new(0),
    };
    assert!(ide.register_from_native_picker(&mut picker).is_err());
    assert!(ide.projects().unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_project_roots_and_catalogs_are_denied() {
    use std::os::unix::fs::symlink;
    let root = temp();
    let actual = root.join("actual");
    fs::create_dir(&actual).unwrap();
    let alias = root.join("alias");
    symlink(&actual, &alias).unwrap();
    let catalog = root.join("catalog.json");
    let ide = IdeWorkspaceRegistry::open(catalog.clone()).unwrap();
    let mut picker = Picker {
        selected: Some(alias),
        calls: AtomicUsize::new(0),
    };
    assert!(matches!(
        ide.register_from_native_picker(&mut picker),
        Err(IdeError::InvalidSelection)
    ));
    fs::write(root.join("real.json"), b"{}").unwrap();
    symlink(root.join("real.json"), &catalog).unwrap();
    assert!(IdeWorkspaceRegistry::open(catalog).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_ui_operation_cannot_supply_a_host_path() {
    let root = temp();
    let ide = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let mut picker = Picker {
        selected: None,
        calls: AtomicUsize::new(0),
    };
    let bad_id = request(Operation::Close(proto::CloseProject {
        project_id: "ide-00000000000000000000000000000000".into(),
    }));
    let reply = response(&ide.handle_ui_request(&bad_id, &mut picker).unwrap());
    assert!(matches!(reply.outcome, Some(Outcome::Failure(_))));
    assert_eq!(picker.calls.load(Ordering::SeqCst), 0);
    assert!(ide
        .handle_ui_request(b"../../tmp/hostile", &mut picker)
        .is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_registry_is_independent_from_lu_core() {
    let root = temp();
    let catalog = root.join("projects.json");
    let ide = IdeWorkspaceRegistry::open(catalog).unwrap();
    assert!(ide.projects().unwrap().is_empty());
    assert!(!root.join("lu_core.json").exists());
    fs::remove_dir_all(root).unwrap();
}
