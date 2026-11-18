//! SEC-05 adversarial SQLite/in-memory custody regressions.
//! Test-only: no model, renderer, OS effect replay or execution authority.
use super::*;
use std::{fs,path::Path,time::{SystemTime,UNIX_EPOCH}};
fn dir()->std::path::PathBuf{
    let p=std::env::temp_dir().join(format!(
        "lu-sec05-task-{}-{}",std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn sec05_sqlite_writer_cannot_forge_verified_terminal_or_erase_uncertain_task(){
    let root=dir();
    let path=root.join("original-task.sqlite");
    {
        let store=TaskStore::open(&path).unwrap();
        store.create("sec05-worker",Some("c-a"),None).unwrap();
        store.start("sec05-worker").unwrap();
        store.record_process_uncertain(
            "sec05-worker","LINUX_GUARDIAN_LOST").unwrap();
        let attacker=Connection::open(&path).unwrap();
        attacker.pragma_update(None,"foreign_keys","ON").unwrap();
        for sql in [
            "UPDATE lu_task_custody_uncertainty SET reason='OS_CUSTODY_UNVERIFIED'",
            "DELETE FROM lu_task_custody_uncertainty",
            "UPDATE lu_tasks SET state='COMPLETED' WHERE task_id='sec05-worker'",
            "UPDATE lu_tasks SET state='RUNNING' WHERE task_id='sec05-worker'",
            "UPDATE lu_tasks SET state='CANCELLED' WHERE task_id='sec05-worker'",
            "UPDATE lu_tasks SET state='TIMED_OUT' WHERE task_id='sec05-worker'",
            "DELETE FROM lu_tasks WHERE task_id='sec05-worker'",
            "UPDATE lu_tasks SET task_id='sec05-forged' WHERE task_id='sec05-worker'",
            "UPDATE lu_tasks SET workspace_id='c-other' WHERE task_id='sec05-worker'",
            "UPDATE lu_task_events SET event='COMPLETED' WHERE event='PROCESS_UNCERTAIN'",
            "DELETE FROM lu_task_events WHERE event='PROCESS_UNCERTAIN'",
        ]{
            assert!(attacker.execute_batch(sql).is_err(),
                "direct SQL mutation must not clear original custody uncertainty: {sql}");
        }
        assert!(store.complete("sec05-worker").is_err());
        assert!(store.cancelled("sec05-worker").is_err());
        assert!(store.timed_out("sec05-worker").is_err());
        assert_eq!(store.reconcile_interrupted().unwrap(),1);
        assert!(store.resume("sec05-worker").is_err());
        assert_eq!(store.process_uncertainty("sec05-worker").unwrap()
            .as_deref(),Some("LINUX_GUARDIAN_LOST"));
    }
    let restored=TaskStore::open(&path).unwrap();
    assert_eq!(restored.process_uncertainty("sec05-worker").unwrap()
        .as_deref(),Some("LINUX_GUARDIAN_LOST"));
    assert_eq!(restored.get("sec05-worker").unwrap().unwrap().state,
        TaskState::Interrupted);
    assert!(restored.resume("sec05-worker").is_err());
    drop(restored);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn sec05_bounded_workspace_coverage_never_misattributes_or_discards_unknowns(){
    let store=TaskStore::open(Path::new(":memory:")).unwrap();
    let a=format!("c-{}","a".repeat(32));
    let b=format!("c-{}","b".repeat(32));
    for i in 0..74{
        let id=format!("sec05-adversary-{i:03}");
        store.create(&id,Some(&a),None).unwrap();
        store.start(&id).unwrap();
        store.record_process_uncertain(
            &id,"OS_CUSTODY_UNVERIFIED").unwrap();
    }
    for i in 0..4{
        let id=format!("sec05-other-{i:03}");
        store.create(&id,Some(&b),None).unwrap();
        store.start(&id).unwrap();
        store.record_process_uncertain(
            &id,"OS_CUSTODY_UNVERIFIED").unwrap();
    }
    let (a_rows,more)=store.security_recovery_rows(Some(&a),64).unwrap();
    assert_eq!(a_rows.len(),64);
    assert!(more,"74 original events must not masquerade as complete 64-row window");
    assert!(a_rows.iter().all(|x|x.task_id.starts_with("sec05-adversary-")));
    let (b_rows,more)=store.security_recovery_rows(Some(&b),64).unwrap();
    assert_eq!(b_rows.len(),4);
    assert!(!more);
    assert!(b_rows.iter().all(|x|x.task_id.starts_with("sec05-other-")));
    assert!(store.security_recovery_rows(Some("../escape"),64).is_err());
    assert!(store.security_recovery_rows(None,0).is_err());
}
#[test]
fn sec05_interrupted_without_custody_row_is_not_automatic_restart_permission(){
    let root=dir();
    let path=root.join("original.sqlite");
    let store=TaskStore::open(&path).unwrap();
    store.create("sec05-no-ack",Some("c-one"),None).unwrap();
    store.start("sec05-no-ack").unwrap();
    assert_eq!(store.reconcile_interrupted().unwrap(),1);
    assert!(store.resume("sec05-no-ack").is_err());
    assert_eq!(store.process_uncertainty("sec05-no-ack").unwrap(),None);
    let (rows,more)=store.security_recovery_rows(Some("c-one"),64).unwrap();
    assert_eq!(rows.len(),1);assert!(!more);
    assert_eq!(rows[0].state,TaskState::Interrupted);
    drop(store);
    let again=TaskStore::open(&path).unwrap();
    assert!(again.resume("sec05-no-ack").is_err());
    assert_eq!(again.reconcile_interrupted().unwrap(),0);
    drop(again);
    fs::remove_dir_all(root).unwrap();
}
