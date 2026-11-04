use lu_core::write_approval_ledger::{WriteApprovalLedger, WriteDecision, WriteLedgerError};
use std::{fs, time::{SystemTime, UNIX_EPOCH}};
fn scratch() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "lu-write-approval-{}-{}.sqlite", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ))
}
const SHA: &str="26c1a27202916dd84ea5322e4bd32f7bef59c432f1a851b7789c871f0413e7d";
#[test]
fn exact_review_target_and_content_hash_are_required_before_durable_approval() {
    let path=scratch();
    let db=WriteApprovalLedger::open(&path).unwrap();
    db.stage("pw-1","chat-a","generation-1","approved.txt",12,SHA).unwrap();
    assert!(db.approve("pw-1","chat-a","generation-1","approved.txt",12,SHA,
        "/lu-drafts/approved.txt","g-effect-1").is_err());
    db.choose_target("pw-1","chat-a","generation-1","/lu-drafts/approved.txt").unwrap();
    for (chat,name,sha,path) in [
        ("chat-b","approved.txt",SHA,"/lu-drafts/approved.txt"),
        ("chat-a","other.txt",SHA,"/lu-drafts/approved.txt"),
        ("chat-a","approved.txt",SHA,"/unapproved/approved.txt"),
        ("chat-a","approved.txt","aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","/lu-drafts/approved.txt")
    ] {
        assert_eq!(db.approve("pw-1",chat,"generation-1",name,12,sha,path,
            "g-effect-1"),Err(WriteLedgerError::Transition));
    }
    db.approve("pw-1","chat-a","generation-1","approved.txt",12,SHA,
        "/lu-drafts/approved.txt","g-effect-1").unwrap();
    let record=db.get("pw-1").unwrap().unwrap();
    assert_eq!(record.decision,WriteDecision::Approved);
    assert_eq!(record.effect_call_id.as_deref(),Some("g-effect-1"));
    assert_eq!(record.selected_target.as_deref(),Some("/lu-drafts/approved.txt"));
    assert!(db.approve("pw-1","chat-a","generation-1","approved.txt",12,SHA,
        "/lu-drafts/approved.txt","g-effect-1").is_err());
    drop(db);
    let again=WriteApprovalLedger::open(&path).unwrap();
    assert_eq!(again.get("pw-1").unwrap().unwrap(),record);
    assert_eq!(again.reconcile_pending().unwrap(),0);
    drop(again);
    fs::remove_file(path).unwrap();
}
#[test]
fn pending_and_selected_requests_are_interrupted_after_owner_restart_and_never_granted() {
    let path=scratch();
    let db=WriteApprovalLedger::open(&path).unwrap();
    db.stage("pw-pending","chat-a","model-a","one.txt",10,SHA).unwrap();
    db.stage("pw-selected","chat-a","model-b","two.txt",10,SHA).unwrap();
    db.choose_target("pw-selected","chat-a","model-b","/lu-drafts/two.txt").unwrap();
    db.stage("pw-denied","chat-b","model-c","deny.txt",10,SHA).unwrap();
    db.deny("pw-denied","chat-b").unwrap();
    drop(db);
    let again=WriteApprovalLedger::open(&path).unwrap();
    assert_eq!(again.reconcile_pending().unwrap(),2);
    assert_eq!(again.get("pw-pending").unwrap().unwrap().decision,WriteDecision::Interrupted);
    assert_eq!(again.get("pw-selected").unwrap().unwrap().decision,WriteDecision::Interrupted);
    assert_eq!(again.get("pw-denied").unwrap().unwrap().decision,WriteDecision::Denied);
    assert!(again.approve("pw-selected","chat-a","model-b","two.txt",10,SHA,
        "/lu-drafts/two.txt","g-effect-2").is_err());
    assert_eq!(again.reconcile_pending().unwrap(),0);
    assert_eq!(again.recent_for_chat("chat-a").unwrap().len(),2);
    drop(again);fs::remove_file(path).unwrap();
}
#[test]
fn duplicate_effect_binding_and_invalid_proposals_are_refused() {
    let path=scratch();
    let db=WriteApprovalLedger::open(&path).unwrap();
    for (name,len,sha) in [("../unsafe",12,SHA),("file.txt",16385,SHA),
        ("file.txt",1,"bad")] {
        assert_eq!(db.stage("pw-bad","chat-a","model-a",name,len,sha),
            Err(WriteLedgerError::Invalid));
    }
    for (id,name) in [("pw-a","a.txt"),("pw-b","b.txt")] {
        db.stage(id,"chat-a","model-a",name,12,SHA).unwrap();
        db.choose_target(id,"chat-a","model-a",&format!("/lu-drafts/{name}")).unwrap();
    }
    db.approve("pw-a","chat-a","model-a","a.txt",12,SHA,"/lu-drafts/a.txt","g-same").unwrap();
    assert!(db.approve("pw-b","chat-a","model-a","b.txt",12,SHA,
        "/lu-drafts/b.txt","g-same").is_err());
    assert_eq!(db.get("pw-b").unwrap().unwrap().decision,WriteDecision::Selected);
    drop(db);fs::remove_file(path).unwrap();
}
