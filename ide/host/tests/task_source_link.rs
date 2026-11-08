use lu_ide_workspace::{
    continuity::{ContinuityError, DevelopmentLedger},
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String, DevelopmentLedger) {
    let root=std::env::temp_dir().join(format!("lu-ide-c7-{}-{:016x}",
        std::process::id(),rand::random::<u64>()));
    let project=root.join("source-project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"),b"fn main() {}\n").unwrap();
    fs::write(project.join("src/second.rs"),b"fn second() {}\n").unwrap();
    let registry=IdeWorkspaceRegistry::open(root.join("projects.json")).unwrap();
    let id=registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    let ledger=DevelopmentLedger::new(root.join("development-ledger.jsonl")).unwrap();
    (root,registry,id,ledger)
}
fn task(registry:&IdeWorkspaceRegistry,ledger:&DevelopmentLedger,id:&str) ->
    lu_ide_workspace::task_evidence::TaskSummary
{
    ledger.record_task_update(registry,id,"C7","Source relation",
        "in_progress","Check local edits",
        vec!["source_review".into(),"exact_head_ci".into()]).unwrap()
}

#[test]
fn real_registered_source_link_persists_across_journal_restart() {
    let(root,registry,id,ledger)=fixture();
    let before=task(&registry,&ledger,&id);
    let preview=registry.preview_source_file(&id,"src/main.rs").unwrap();
    let linked=ledger.record_task_source(&registry,&id,"C7",
        "src/main.rs",&preview.revision,&before.update_event_id).unwrap();
    assert_eq!(linked.source_links.len(),1);
    assert_eq!(linked.source_links[0].relative_path,"src/main.rs");
    assert_eq!(linked.source_links[0].saved_revision,preview.revision);
    assert_eq!(linked.source_links[0].source,"native_registered_source_read");
    assert_eq!(linked.task_acceptance,"not_verified");
    assert_eq!(linked.acceptance_gates[0].state,"not_verified");
    drop(ledger);
    let ledger=DevelopmentLedger::new(root.join("development-ledger.jsonl")).unwrap();
    let context=ledger.context(&registry,&id).unwrap();
    assert_eq!(context.tasks.len(),1);
    assert_eq!(context.tasks[0].source_links.len(),1);
    assert_eq!(context.tasks[0].source_links[0].bound_update_event_id,
        before.update_event_id);
    let raw=fs::read_to_string(root.join("development-ledger.jsonl")).unwrap();
    assert!(!raw.contains("fn main()"));
    assert!(!raw.contains(root.to_str().unwrap()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_task_update_and_changed_source_are_never_associated() {
    let(root,registry,id,ledger)=fixture();
    let first=task(&registry,&ledger,&id);
    let preview=registry.preview_source_file(&id,"src/main.rs").unwrap();
    let revised=task(&registry,&ledger,&id);
    assert_eq!(ledger.record_task_source(&registry,&id,"C7","src/main.rs",
        &preview.revision,&first.update_event_id).unwrap_err().code(),
        ContinuityError::TaskStale.code());
    fs::write(root.join("source-project/src/main.rs"),b"changed source\n").unwrap();
    assert_eq!(ledger.record_task_source(&registry,&id,"C7","src/main.rs",
        &preview.revision,&revised.update_event_id).unwrap_err().code(),
        ContinuityError::TaskStale.code());
    assert!(ledger.context(&registry,&id).unwrap().tasks[0].source_links.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn prior_source_links_are_historical_after_a_new_task_revision() {
    let(root,registry,id,ledger)=fixture();
    let first=task(&registry,&ledger,&id);
    let preview=registry.preview_source_file(&id,"src/main.rs").unwrap();
    ledger.record_task_source(&registry,&id,"C7","src/main.rs",
        &preview.revision,&first.update_event_id).unwrap();
    let revised=task(&registry,&ledger,&id);
    assert_eq!(revised.source_links.len(),1);
    assert_ne!(revised.source_links[0].bound_update_event_id,
        revised.update_event_id);
    // Explicitly relink the source to the new task update.
    let again=ledger.record_task_source(&registry,&id,"C7","src/main.rs",
        &preview.revision,&revised.update_event_id).unwrap();
    assert_eq!(again.source_links.len(),1);
    assert_eq!(again.source_links[0].bound_update_event_id,
        revised.update_event_id);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_link_does_not_accept_escape_or_files_from_closed_project() {
    let(root,registry,id,ledger)=fixture();
    let first=task(&registry,&ledger,&id);
    let sha=registry.preview_source_file(&id,"src/main.rs").unwrap().revision;
    for path in ["../secret.txt","/etc/passwd","src/../../secret","src\\main.rs"] {
        assert_eq!(ledger.record_task_source(&registry,&id,"C7",path,&sha,
            &first.update_event_id).unwrap_err().code(),
            ContinuityError::InvalidInput.code());
    }
    registry.close(&id).unwrap();
    assert!(ledger.record_task_source(&registry,&id,"C7","src/main.rs",&sha,
        &first.update_event_id).is_err());
    assert!(root.join("source-project/src/main.rs").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn at_most_twelve_unique_file_links_but_same_source_relink_is_allowed() {
    let(root,registry,id,ledger)=fixture();
    let first=task(&registry,&ledger,&id);
    for i in 0..13 {
        let path=format!("src/file{i}.rs");
        fs::write(root.join("source-project").join(&path),
            format!("const N: usize = {i};\n")).unwrap();
        let sha=registry.preview_source_file(&id,&path).unwrap().revision;
        let response=ledger.record_task_source(&registry,&id,"C7",&path,
            &sha,&first.update_event_id);
        if i < 12 { assert!(response.is_ok()); }
        else { assert!(response.is_err()); }
    }
    let path="src/file0.rs";
    let sha=registry.preview_source_file(&id,path).unwrap().revision;
    let linked=ledger.record_task_source(&registry,&id,"C7",path,
        &sha,&first.update_event_id).unwrap();
    assert_eq!(linked.source_links.len(),12);
    assert_eq!(linked.task_acceptance,"not_verified");
    fs::remove_dir_all(root).unwrap();
}
