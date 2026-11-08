use lu_core::{
    continuity::{ContinuityStore,TaskCheckpoint},
    journal::{EffectJournal,EffectState},
    contracts::{ToolInvocation,ProtoPayload}
};
use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
fn root()->PathBuf{
    let p=std::env::temp_dir().join(format!(
        "lu-ansf-effect-proof-{}-{}",std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&p).unwrap();p
}
fn effect(id:&str)->ToolInvocation{
    ToolInvocation{
        call_id:id.into(),tool_id:"lu.core.file_write".into(),
        workspace_id:Some("local-dev".into()),
        input:ProtoPayload{
            type_url:"type.googleapis.com/lu.v1.WriteRequest".into(),
            bytes:b"expected-input".to_vec()
        }
    }
}
fn checkpoint()->TaskCheckpoint{
    TaskCheckpoint{
        checkpoint_id:"checkpoint-proof-01".into(),
        project_id:"lu".into(),task_id:"nsis".into(),
        conversation_id:"c-source".into(),
        source_entry_ids:vec![1],
        completed_work:"Package was verified".into(),
        next_action:"Package the app".into(),blockers:vec![]
    }
}
#[test]
fn only_applied_receipts_can_prove_work_across_restarts(){
    let folder=root();
    let (state,work)= (folder.join("effects.sqlite"),folder.join("continuity.sqlite"));
    {
        let effects=EffectJournal::open(&state).unwrap();
        let continuity=ContinuityStore::open(&work).unwrap();
        continuity.append(&checkpoint()).unwrap();
        assert!(continuity.link_verified_effect("checkpoint-proof-01","missing",&effects).is_err());
        effects.prepare(&effect("call-001")).unwrap();
        assert!(continuity.link_verified_effect("checkpoint-proof-01","call-001",&effects).is_err());
        effects.mark_executing("call-001").unwrap();
        assert!(continuity.link_verified_effect("checkpoint-proof-01","call-001",&effects).is_err());
        effects.mark_applied("call-001",b"real protobuf receipt").unwrap();
        let saved=continuity.link_verified_effect("checkpoint-proof-01","call-001",&effects).unwrap();
        assert_eq!(saved.tool_id,"lu.core.file_write");
        assert_eq!(saved.receipt_bytes,b"real protobuf receipt".len() as u64);
        assert_eq!(saved.receipt_sha256.len(),64);
        assert_eq!(continuity.link_verified_effect("checkpoint-proof-01","call-001",&effects).unwrap(),saved);
        assert!(continuity.link_verified_effect("checkpoint-missing","call-001",&effects).is_err());
        assert_eq!(continuity.verified_effects_for_checkpoint("checkpoint-proof-01",&effects).unwrap(),vec![saved]);
        effects.prepare(&effect("call-002")).unwrap();
        effects.mark_executing("call-002").unwrap();
    }
    {
        let effects=EffectJournal::open(&state).unwrap();
        effects.reconcile_interrupted().unwrap();
        assert_eq!(effects.get("call-002").unwrap().unwrap().state,EffectState::Uncertain);
        let continuity=ContinuityStore::open(&work).unwrap();
        assert!(continuity.link_verified_effect("checkpoint-proof-01","call-002",&effects).is_err());
        assert_eq!(continuity.verified_effects_for_checkpoint("checkpoint-proof-01",&effects).unwrap().len(),1);
        assert!(continuity.verified_effects_for_checkpoint("missing",&effects).unwrap().is_empty());
    }
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn context_change_monitor_detects_new_task_assumptions_without_replay(){
    use lu_core::continuity::compare_task_checkpoints;
    let folder=root();
    let store=ContinuityStore::open(&folder.join("ccm.sqlite")).unwrap();
    let before=checkpoint();
    store.append(&before).unwrap();
    let mut after=before.clone();
    after.checkpoint_id="checkpoint-proof-02".into();
    after.conversation_id="c-new-chat".into();
    after.source_entry_ids=vec![4];
    after.next_action="Run new Windows installer acceptance".into();
    after.blockers=vec!["Windows machine unavailable".into()];
    store.append(&after).unwrap();
    let pair=store.recent_for_task("lu","nsis").unwrap();
    assert_eq!(pair.len(),2);
    let review=compare_task_checkpoints(&pair[1],&pair[0]).unwrap();
    assert!(review.needs_review);
    assert!(review.next_action_changed);
    assert!(review.source_evidence_changed);
    assert_eq!(review.added_blockers,vec!["Windows machine unavailable"]);
    assert!(!review.completed_work_changed);
    assert!(compare_task_checkpoints(&pair[1],&before).is_err());
    assert!(store.recent_for_task("../lu","nsis").is_err());
    drop(store);
    fs::remove_dir_all(folder).unwrap();
}
