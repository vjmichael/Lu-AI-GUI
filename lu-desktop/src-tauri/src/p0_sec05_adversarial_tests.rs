//! SEC-05 adversarial original-chat, journal and revocation fixtures.
//! All inputs and corruptions are made in disposable test-only profiles.
use super::*;
use std::{fs,path::PathBuf,sync::{Arc,Barrier},
    time::{SystemTime,UNIX_EPOCH}};
use crate::{
    p0_auth_custody::{HostCancelEvent,SCHEMA},
    reviewed_app_recovery::reconcile_reviewed_application_journals,
    reviewed_task_continuity::sec04_reviewed_turn_security,
};
use lu_core::tasks::TaskStore;
fn root()->PathBuf{
    let path=std::env::temp_dir().join(format!(
        "lu-sec05-host-{}-{}",std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn cancellation(chat:&str,index:u32)->HostCancelEvent{
    HostCancelEvent{
        schema:SCHEMA.into(),
        attempt_id:format!("au-{index:032x}"),
        chat_id:chat.into(),phase:"REQUESTED".into(),
        domains:vec![],authority_restored:false,
        automatic_retry_allowed:false,
        os_custody_independently_verified:false,
    }
}
#[test]
fn sec05_wrong_chat_origin_and_corrupted_original_task_never_leak_workers(){
    let root=root();
    let chats=ChatStore::open(&root).unwrap();
    let a=chats.create_chat().unwrap().chat_id;
    let b=chats.create_chat().unwrap().chat_id;
    let turn=format!("g-{}","a".repeat(32));
    chats.append_user_with_turn_id(&a,&turn,"Original A").unwrap();
    // Attacker supplies a syntactically valid original journal filename,
    // but its authenticated owner is another conversation.
    let path=root.join(format!("lu-reviewed-task-{turn}.sqlite"));
    let store=TaskStore::open(&path).unwrap();
    store.bind_reviewed_human_turn(&b,&turn).unwrap();
    store.create("secret-b-worker",Some(&turn),None).unwrap();
    store.start("secret-b-worker").unwrap();
    store.record_process_uncertain("secret-b-worker",
        "OS_CUSTODY_UNVERIFIED").unwrap();
    drop(store);
    let mismatched=sec04_reviewed_turn_security(&root,&a,&turn)
        .unwrap().unwrap();
    assert!(!mismatched.original_origin_bound);
    assert_eq!(mismatched.journal_status,"ORIGINAL_ORIGIN_MISMATCH");
    assert!(mismatched.interrupted_or_uncertain_tasks.is_empty());
    let background=TaskStore::open(
        &root.join("lu-background-tasks.sqlite")).unwrap();
    let view=for_original_chat(&root,&a,&chats,&background,&[]).unwrap();
    assert!(view.problems.iter().any(|item|
        item.reason=="ORIGINAL_AUTHORITY_BINDING_UNVERIFIED"));
    assert!(!view.problems.iter().any(|item|
        item.task_id.as_deref()==Some("secret-b-worker")));
    assert!(view.human_reconciliation_required);
    drop(background);drop(chats);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn sec05_corrupt_effect_journal_and_interrupted_startup_fail_closed(){
    let root=root();
    let chats=ChatStore::open(&root).unwrap();
    let chat=chats.create_chat().unwrap().chat_id;
    let turn=format!("g-{}","c".repeat(32));
    chats.append_user_with_turn_id(&chat,&turn,"Original recovered read").unwrap();
    let task=root.join(format!("lu-reviewed-task-{turn}.sqlite"));
    let effect=root.join(format!("lu-reviewed-effects-{turn}.sqlite"));
    let store=TaskStore::open(&task).unwrap();
    store.bind_reviewed_human_turn(&chat,&turn).unwrap();
    store.create("original-worker",Some(&turn),None).unwrap();
    store.start("original-worker").unwrap();
    store.record_process_uncertain("original-worker",
        "LINUX_GUARDIAN_LOST").unwrap();
    drop(store);
    // Deliberately corrupt the other original journal. SEC-04 must not
    // downgrade the whole turn to a successful empty result or replay it.
    fs::write(&effect,b"not-sqlite-original-effect").unwrap();
    assert!(reconcile_reviewed_application_journals(&root).is_err());
    let background=TaskStore::open(
        &root.join("lu-background-tasks.sqlite")).unwrap();
    let result=for_original_chat(&root,&chat,&chats,&background,&[]);
    assert!(result.is_err(),"damaged original effects must not claim clearance");
    let unavailable=SecurityRecoveryView::unavailable();
    assert!(unavailable.human_reconciliation_required);
    assert!(!unavailable.prior_authority_reusable);
    assert!(!unavailable.automatic_retry_allowed);
    assert!(unavailable.additional_problems_not_displayed);
    // The original immutable failure survives even though the effect
    // file is corrupted and cannot be reconciled.
    let tasks=TaskStore::open(&task).unwrap();
    assert_eq!(tasks.process_uncertainty("original-worker").unwrap()
        .as_deref(),Some("LINUX_GUARDIAN_LOST"));
    drop(tasks);drop(background);drop(chats);
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn sec05_symlinked_original_journal_cannot_read_external_target(){
    use std::os::unix::fs::symlink;
    let root=root();
    let chats=ChatStore::open(&root).unwrap();
    let chat=chats.create_chat().unwrap().chat_id;
    let turn=format!("g-{}","d".repeat(32));
    chats.append_user_with_turn_id(&chat,&turn,"Original only").unwrap();
    let target=root.join("unrelated-private-original.txt");
    fs::write(&target,b"DO NOT MODIFY UNRELATED DATA").unwrap();
    let fake=root.join(format!("lu-reviewed-task-{turn}.sqlite"));
    symlink(&target,&fake).unwrap();
    let background=TaskStore::open(
        &root.join("lu-background-tasks.sqlite")).unwrap();
    assert!(for_original_chat(&root,&chat,&chats,&background,&[]).is_err());
    assert!(reconcile_reviewed_application_journals(&root).is_err());
    assert_eq!(fs::read(&target).unwrap(),b"DO NOT MODIFY UNRELATED DATA");
    drop(background);drop(chats);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn sec05_32_concurrent_revoke_complete_collisions_and_cross_chat_isolation(){
    let root=root();
    let chats=Arc::new(ChatStore::open(&root).unwrap());
    let a=chats.create_chat().unwrap().chat_id;
    let b=chats.create_chat().unwrap().chat_id;
    let b_turn=format!("g-{}","f".repeat(32));
    chats.append_user_with_turn_id(&b,&b_turn,"Chat B original").unwrap();
    chats.append_progress(&b,&b_turn,"B output").unwrap();
    let b_before=chats.p0_auth_requested_watermark(&b).unwrap();
    for i in 0..32u32{
        let turn=format!("g-{i:032x}");
        chats.append_user_with_turn_id(&a,&turn,"Chat A original").unwrap();
        chats.append_progress(&a,&turn,"Original uncommitted A output").unwrap();
        let baseline=chats.p0_auth_requested_watermark(&a).unwrap();
        let barrier=Arc::new(Barrier::new(3));
        let t1=Arc::clone(&chats);let x1=Arc::clone(&barrier);
        let chat_a=a.clone();let turn_a=turn.clone();
        let finishing=std::thread::spawn(move||{
            x1.wait();
            t1.complete_reviewed_if_not_revoked(&chat_a,&turn_a,baseline)
        });
        let t2=Arc::clone(&chats);let x2=Arc::clone(&barrier);
        let chat_a=a.clone();
        let cancelling=std::thread::spawn(move||{
            x2.wait();
            t2.append_p0_auth_cancel(&cancellation(&chat_a,i+1))
        });
        barrier.wait();
        let _=finishing.join().unwrap();
        cancelling.join().unwrap().unwrap();
        assert!(chats.p0_auth_requested_watermark(&a).unwrap()>baseline);
        assert!(chats.complete_reviewed_if_not_revoked(&a,&turn,baseline)
            .is_err(),"old A authority must fail after committed revoke");
        assert_eq!(chats.p0_auth_requested_watermark(&b).unwrap(),b_before);
    }
    chats.complete_reviewed_if_not_revoked(&b,&b_turn,b_before).unwrap();
    let a_fresh=format!("g-{}","e".repeat(32));
    chats.append_user_with_turn_id(&a,&a_fresh,"Fresh distinct authorization").unwrap();
    chats.append_progress(&a,&a_fresh,"New output").unwrap();
    let fresh_baseline=chats.p0_auth_requested_watermark(&a).unwrap();
    chats.complete_reviewed_if_not_revoked(&a,&a_fresh,fresh_baseline).unwrap();
    drop(chats);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn sec05_saturated_cancellation_history_is_not_an_all_clear(){
    let root=root();
    let chats=ChatStore::open(&root).unwrap();
    let a=chats.create_chat().unwrap().chat_id;
    let turn=format!("g-{}","a".repeat(32));
    chats.append_user_with_turn_id(&a,&turn,"Original A").unwrap();
    for i in 0..17u32{
        chats.append_p0_auth_cancel(&cancellation(&a,i+1)).unwrap();
    }
    let recent=chats.p0_auth_cancel_history(&a).unwrap();
    assert_eq!(recent.len(),16);
    let background=TaskStore::open(
        &root.join("lu-background-tasks.sqlite")).unwrap();
    let output=for_original_chat(&root,&a,&chats,&background,&recent).unwrap();
    assert!(output.older_cancel_intents_not_inspected);
    assert!(output.human_reconciliation_required);
    assert!(!output.automatic_retry_allowed);
    assert!(output.pending_cancel_intents>0);
    drop(background);drop(chats);
    fs::remove_dir_all(root).unwrap();
}
