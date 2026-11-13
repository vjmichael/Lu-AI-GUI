use lu_core::read_approval_ledger::{ReadApprovalLedger, ReadDecision, ReadLedgerError};
use std::{fs, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};

// Windows timestamps can share the same system-clock tick across parallel tests.
// Make independent SQLite files unique even when pid and nanoseconds coincide.
static NEXT_LEDGER_PATH: AtomicU64 = AtomicU64::new(0);
fn path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "lu-read-approval-{}-{}-{}.sqlite", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        NEXT_LEDGER_PATH.fetch_add(1, Ordering::Relaxed)))
}
#[test]
fn approval_decisions_persist_reject_replay_and_do_not_mint_permission() {
    let p=path();
    let db=ReadApprovalLedger::open(&p).unwrap();
    db.stage("p-01","chat-a","g-01","report.txt",1024).unwrap();
    assert_eq!(db.stage("p-01","chat-a","g-01","report.txt",1024),Err(ReadLedgerError::Duplicate));
    assert_eq!(db.decide("p-01","chat-b","g-01","report.txt",1024,ReadDecision::Approved),Err(ReadLedgerError::Conflict));
    assert_eq!(db.decide("p-01","chat-a","g-02","report.txt",1024,ReadDecision::Approved),Err(ReadLedgerError::Conflict));
    assert_eq!(db.decide("p-01","chat-a","g-01","other.txt",1024,ReadDecision::Approved),Err(ReadLedgerError::Conflict));
    db.decide("p-01","chat-a","g-01","report.txt",1024,ReadDecision::Approved).unwrap();
    db.link_effect("p-01","chat-a","g-01","g-effect-01").unwrap();
    assert_eq!(db.link_effect("p-01","chat-a","g-01","g-effect-01"),Err(ReadLedgerError::Transition));
    assert_eq!(db.get("p-01").unwrap().unwrap().effect_call_id.as_deref(),Some("g-effect-01"));
    assert_eq!(db.decide("p-01","chat-a","g-01","report.txt",1024,ReadDecision::Denied),Err(ReadLedgerError::Transition));
    assert_eq!(db.get("p-01").unwrap().unwrap().decision,ReadDecision::Approved);
    drop(db);
    let reopened=ReadApprovalLedger::open(&p).unwrap();
    assert_eq!(reopened.get("p-01").unwrap().unwrap().decision,ReadDecision::Approved);
    assert_eq!(reopened.get("p-01").unwrap().unwrap().effect_call_id.as_deref(),Some("g-effect-01"));
    assert_eq!(reopened.reconcile_pending().unwrap(),0);
    drop(reopened);
    fs::remove_file(p).unwrap();
}
#[test]
fn crash_recovery_marks_pending_interrupted_and_no_reapproval() {
    let p=path();
    let db=ReadApprovalLedger::open(&p).unwrap();
    db.stage("p-10","chat-a","g-10","one.txt",64).unwrap();
    db.stage("p-11","chat-b","g-11","two.txt",4096).unwrap();
    db.decide("p-11","chat-b","g-11","two.txt",4096,ReadDecision::Denied).unwrap();
    drop(db);
    let reopened=ReadApprovalLedger::open(&p).unwrap();
    assert_eq!(reopened.reconcile_pending().unwrap(),1);
    assert_eq!(reopened.get("p-10").unwrap().unwrap().decision,ReadDecision::Interrupted);
    assert_eq!(reopened.get("p-11").unwrap().unwrap().decision,ReadDecision::Denied);
    assert_eq!(reopened.decide("p-10","chat-a","g-10","one.txt",64,ReadDecision::Approved),Err(ReadLedgerError::Transition));
    assert_eq!(reopened.reconcile_pending().unwrap(),0);
    drop(reopened);
    fs::remove_file(p).unwrap();
}
#[test]
fn invalid_approval_and_untrusted_targets_fail_closed() {
    let p=path();
    let db=ReadApprovalLedger::open(&p).unwrap();
    assert_eq!(db.stage("p-invalid","chat-a","g-1","../secret",64),Err(ReadLedgerError::Invalid));
    assert_eq!(db.stage("p-invalid","chat-a","g-1","good.txt",0),Err(ReadLedgerError::Invalid));
    assert_eq!(db.stage("p-invalid","chat-a","g-1","good.txt",65537),Err(ReadLedgerError::Invalid));
    assert_eq!(db.decide("missing","chat-a","g-1","good.txt",64,ReadDecision::Approved),Err(ReadLedgerError::Missing));
    drop(db);
    fs::remove_file(p).unwrap();
}

#[test]
fn a_read_approval_can_never_bind_to_another_conversation_or_duplicate_effect() {
    let p=path();
    let db=ReadApprovalLedger::open(&p).unwrap();
    db.stage("p-a", "chat-a", "model-a", "one.txt", 1024).unwrap();
    db.stage("p-b", "chat-b", "model-b", "two.txt", 1024).unwrap();
    assert_eq!(db.link_effect("p-a","chat-a","model-a","g-1"),Err(ReadLedgerError::Transition));
    db.decide("p-a","chat-a","model-a","one.txt",1024,ReadDecision::Approved).unwrap();
    db.decide("p-b","chat-b","model-b","two.txt",1024,ReadDecision::Approved).unwrap();
    assert_eq!(db.link_effect("p-a","chat-b","model-a","g-1"),Err(ReadLedgerError::Transition));
    assert_eq!(db.link_effect("p-a","chat-a","model-b","g-1"),Err(ReadLedgerError::Transition));
    db.link_effect("p-a","chat-a","model-a","g-1").unwrap();
    assert!(db.link_effect("p-b","chat-b","model-b","g-1").is_err(), "one effect cannot be authorized by two approvals");
    assert_eq!(db.get("p-b").unwrap().unwrap().effect_call_id,None);
    drop(db);
    fs::remove_file(p).unwrap();
}

#[test]
fn upgrades_early_approval_schema_without_erasing_old_decisions() {
    use rusqlite::Connection;
    let p = path();
    let conn = Connection::open(&p).unwrap();
    conn.execute_batch(
        "CREATE TABLE lu_read_approvals (
            proposal_id TEXT PRIMARY KEY NOT NULL,
            chat_id TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            file_name TEXT NOT NULL,
            max_bytes INTEGER NOT NULL,
            state TEXT NOT NULL
        );
        INSERT INTO lu_read_approvals
            (proposal_id,chat_id,generation_id,file_name,max_bytes,state)
        VALUES ('p-old','chat-a','g-old','old.txt',512,'APPROVED');"
    ).unwrap();
    drop(conn);
    let upgraded = ReadApprovalLedger::open(&p).unwrap();
    assert_eq!(upgraded.get("p-old").unwrap().unwrap().decision,ReadDecision::Approved);
    assert_eq!(upgraded.get("p-old").unwrap().unwrap().effect_call_id,None);
    upgraded.link_effect("p-old","chat-a","g-old","g-effect-migrated").unwrap();
    assert_eq!(
        upgraded.get("p-old").unwrap().unwrap().effect_call_id.as_deref(),
        Some("g-effect-migrated")
    );
    drop(upgraded);
    fs::remove_file(p).unwrap();
}

#[test]
fn parallel_read_approval_test_paths_never_alias() {
    let workers: Vec<_> = (0..64)
        .map(|_| std::thread::spawn(path))
        .collect();
    let paths: std::collections::HashSet<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("ledger fixture thread"))
        .collect();
    assert_eq!(paths.len(), 64, "independent approvals must never share a SQLite test file");
}
