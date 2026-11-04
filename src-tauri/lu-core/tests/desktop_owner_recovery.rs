use lu_core::{
    desktop_owner::{DesktopOwner, OwnerError},
    journal::{EffectJournal, EffectState},
    read_approval_ledger::{ReadApprovalLedger, ReadDecision, ReadLedgerError},
};
use std::{fs, time::{SystemTime, UNIX_EPOCH}};

fn scratch() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "lu-owner-recovery-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ))
}

#[test]
fn second_owner_is_excluded_until_first_releases_os_sqlite_lock() {
    let root = scratch();
    fs::create_dir_all(&root).unwrap();
    let lock = root.join("owner.sqlite");
    let owner = DesktopOwner::acquire(&lock).unwrap();
    assert!(matches!(DesktopOwner::acquire(&lock), Err(OwnerError::AlreadyRunning)));
    drop(owner);
    let second_owner = DesktopOwner::acquire(&lock).unwrap();
    assert!(matches!(DesktopOwner::acquire(&lock), Err(OwnerError::AlreadyRunning)));
    drop(second_owner);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exclusive_owner_allows_once_only_reconciliation_without_approval_replay() {
    let root = scratch();
    fs::create_dir_all(&root).unwrap();
    let lock = root.join("owner.sqlite");
    let approvals_file = root.join("approvals.sqlite");
    let effects_file = root.join("effects.sqlite");
    let first = DesktopOwner::acquire(&lock).unwrap();
    let approvals = ReadApprovalLedger::open(&approvals_file).unwrap();
    approvals.stage("pending-1", "chat-one", "turn-one", "report.txt", 256).unwrap();
    approvals.stage("denied-1", "chat-one", "turn-two", "note.txt", 512).unwrap();
    approvals.decide("denied-1", "chat-one", "turn-two", "note.txt", 512, ReadDecision::Denied).unwrap();
    let journal = EffectJournal::open(&effects_file).unwrap();
    // Crash fixture: recorded interrupted in-flight effect. No native effect
    // or user grant is resurrected by either journal during recovery.
    let wire = lu_core::contracts::ToolInvocation {
        call_id: "native-effect-1".into(),
        tool_id: "lu.files.read_text".into(),
        workspace_id: Some("selection-a".into()),
        input: lu_core::contracts::ProtoPayload {
            type_url: "type.googleapis.com/lu.v1.ReadTextFileRequest".into(),
            bytes: vec![1, 2, 3],
        },
    };
    use lu_core::journal::PrepareOutcome;
    assert_eq!(journal.prepare(&wire).unwrap(), PrepareOutcome::New);
    journal.mark_executing(&wire.call_id).unwrap();
    assert!(matches!(DesktopOwner::acquire(&lock), Err(OwnerError::AlreadyRunning)));
    drop(approvals);
    drop(journal);
    drop(first);

    // New instance can recover only after the prior owner is gone.
    let second = DesktopOwner::acquire(&lock).unwrap();
    let approvals = ReadApprovalLedger::open(&approvals_file).unwrap();
    let journal = EffectJournal::open(&effects_file).unwrap();
    assert_eq!(approvals.reconcile_pending().unwrap(), 1);
    assert_eq!(journal.reconcile_interrupted().unwrap(), 1);
    assert_eq!(approvals.get("pending-1").unwrap().unwrap().decision, ReadDecision::Interrupted);
    assert_eq!(approvals.get("denied-1").unwrap().unwrap().decision, ReadDecision::Denied);
    assert_eq!(journal.get(&wire.call_id).unwrap().unwrap().state, EffectState::Uncertain);
    assert_eq!(approvals.decide(
        "pending-1", "chat-one", "turn-one", "report.txt", 256, ReadDecision::Approved
    ), Err(ReadLedgerError::Transition));
    assert!(matches!(journal.prepare(&wire).unwrap(), PrepareOutcome::Existing(EffectState::Uncertain)));
    assert_eq!(approvals.reconcile_pending().unwrap(), 0);
    assert_eq!(journal.reconcile_interrupted().unwrap(), 0);
    drop(approvals);
    drop(journal);
    drop(second);
    fs::remove_dir_all(root).unwrap();
}
