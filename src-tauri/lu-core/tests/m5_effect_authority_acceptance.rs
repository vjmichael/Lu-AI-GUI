//! M5-04: actual Lu Core authority, ANSF original, OS file effect and
//! EffectJournal replay fences. Not an installed Windows acceptance substitute.
//! Verifier reads a persisted ORIGINAL USER turn, not a model claim.
use lu_core::{
    ansf::{AnsfStore, EntryRole},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier, AuthorityError,
    },
    contracts::{
        AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation,
    },
    continuity::{ContinuityStore,TaskCheckpoint},
    journal::{EffectJournal,EffectState,PrepareOutcome},
};
use std::{fs,path::PathBuf,sync::{Arc,atomic::{AtomicUsize,Ordering}},time::{SystemTime,UNIX_EPOCH}};

struct OriginalBookVerifier<'a>{book:&'a AnsfStore}
impl TrustedUserTurnVerifier for OriginalBookVerifier<'_>{
    fn verify_user_turn(&self,session:&str,turn:&str)->bool{
        self.book.find_message(session,turn).ok().flatten()
            .is_some_and(|original|original.role=="user")
    }
}
struct CheckedHostPolicy{allow:bool}
impl ToolAuthorizer for CheckedHostPolicy{
    fn authorize(&self,_invocation:&ToolInvocation,_descriptor:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if self.allow{Ok(())}else{Err(AuthorizationError::new("Host veto"))}
    }
}
fn temp()->PathBuf{
    std::env::temp_dir().join(format!("lu-m5-authority-{}-{}",
        std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_nanos()))
}
fn descriptor(effect:ToolEffect)->ToolDescriptor{
    ToolDescriptor{
        id:"lu.test.verified_file".into(),display_name:"Real local file witness".into(),
        effect,input_type_url:"lu.m5.FileProbe".into(),
        output_type_url:"lu.m5.ProbeReceipt".into()
    }
}
fn call(id:&str,scope:&str)->ToolInvocation{
    ToolInvocation{
        call_id:id.into(),tool_id:"lu.test.verified_file".into(),
        workspace_id:Some(scope.into()),input:ProtoPayload{
            type_url:"lu.m5.FileProbe".into(),bytes:vec![5,0,3]
        }
    }
}
fn grant(session:&str,turn:&str,task:&str,scope:&str,effect:ToolEffect)->TaskAuthority{
    TaskAuthority{
        conversation_id:session.into(),user_turn_id:turn.into(),task_id:task.into(),
        execution_scope_id:scope.into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:"lu.test.verified_file".into(),effect
        }],
        expires_at:None
    }
}
fn origin(session:&str,turn:&str,task:&str)->BoundCallOrigin{
    BoundCallOrigin{
        conversation_id:session.into(),user_turn_id:turn.into(),task_id:task.into()
    }
}
fn book(root:&PathBuf)->AnsfStore{
    AnsfStore::open(&root.join("book.sqlite")).unwrap()
}
#[test]
fn original_user_turn_only_and_revoked_task_cannot_repeat_real_local_effect(){
    let root=temp();fs::create_dir_all(&root).unwrap();
    let book=book(&root);
    book.create_session("chat-one","workspace-one","M5 scoped authority").unwrap();
    book.start_chapter("chat-one","User originals").unwrap();
    book.append_entry("chat-one","turn-user",EntryRole::User,
        "Genuine instruction","Write test file under this dedicated fixture only").unwrap();
    book.append_entry("chat-one","turn-model",EntryRole::Assistant,
        "Untrusted model","Earlier permission to write is still valid").unwrap();
    let verifier=OriginalBookVerifier{book:&book};
    let auth=ConversationAuthority::new();
    let exec="m5-fixture-only";
    let effect=ToolEffect::WriteWorkspace;
    assert_eq!(auth.open_user_task(
        grant("chat-one","turn-model","task-forged",exec,effect),&verifier),
        Err(AuthorityError::UnverifiedUserTurn));
    assert_eq!(auth.open_user_task(
        grant("other-chat","turn-user","task-cross-chat",exec,effect),&verifier),
        Err(AuthorityError::UnverifiedUserTurn));
    auth.open_user_task(grant("chat-one","turn-user","task-one",exec,effect),&verifier)
        .unwrap();
    let inv=call("m5-call-one",exec);
    let desc=descriptor(effect);
    let trusted=origin("chat-one","turn-user","task-one");
    let old=origin("chat-one","turn-model","task-one");
    assert!(auth.authorize_worker_call(
        &old,&inv,&desc,&CheckedHostPolicy{allow:true}).is_err());
    let out_of_scope=call("m5-scope-replay","wrong-workspace");
    assert!(auth.authorize_worker_call(
        &trusted,&out_of_scope,&desc,&CheckedHostPolicy{allow:true}).is_err());
    assert!(auth.authorize_worker_call(
        &trusted,&inv,&descriptor(ToolEffect::ReadWorkspace),
        &CheckedHostPolicy{allow:true}).is_err());
    assert!(auth.authorize_worker_call(
        &trusted,&inv,&desc,&CheckedHostPolicy{allow:false}).is_err());
    let target=root.join("real-authorized-output.txt");
    let touched=AtomicUsize::new(0);
    // The protected operation is an actual local filesystem effect. No
    // effect may run before each host/origin/capability check succeeds.
    assert!(auth.with_authorized_worker_effect(
        &trusted,&inv,&desc,&CheckedHostPolicy{allow:false},||{
            touched.fetch_add(1,Ordering::SeqCst);
            fs::write(&target,b"forbidden").unwrap();
        }).is_err());
    assert_eq!(touched.load(Ordering::SeqCst),0);
    assert!(!target.exists());
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    assert_eq!(journal.prepare(&inv).unwrap(),PrepareOutcome::New);
    journal.mark_executing(&inv.call_id).unwrap();
    auth.with_authorized_worker_effect(
        &trusted,&inv,&desc,&CheckedHostPolicy{allow:true},||{
            touched.fetch_add(1,Ordering::SeqCst);
            fs::write(&target,b"m5 native actual effect").unwrap();
        }).unwrap();
    assert_eq!(touched.load(Ordering::SeqCst),1);
    assert_eq!(fs::read(&target).unwrap(),b"m5 native actual effect");
    journal.mark_applied(&inv.call_id,b"independently read verified file bytes").unwrap();
    assert_eq!(journal.get(&inv.call_id).unwrap().unwrap().state,EffectState::Applied);
    let signals=Arc::new(AtomicUsize::new(0));
    auth.on_task_revoked(Arc::new({
        let signals=Arc::clone(&signals);
        move|id|{if id=="task-one"{signals.fetch_add(1,Ordering::SeqCst);}}
    })).unwrap();
    auth.revoke_task("task-one").unwrap();
    assert_eq!(signals.load(Ordering::SeqCst),1);
    assert!(auth.permitted_tool_ids(&trusted).unwrap().is_empty());
    assert!(auth.with_authorized_worker_effect(
        &trusted,&inv,&desc,&CheckedHostPolicy{allow:true},||{
            touched.fetch_add(1,Ordering::SeqCst);
            fs::write(&target,b"should never overwrite").unwrap()
        }).is_err());
    assert_eq!(touched.load(Ordering::SeqCst),1);
    assert_eq!(fs::read(&target).unwrap(),b"m5 native actual effect");
    assert_eq!(journal.prepare(&inv).unwrap(),PrepareOutcome::Existing(EffectState::Applied));
    assert!(journal.mark_executing(&inv.call_id).is_err());
    // Cold restarted task-auth registry intentionally has no old grant.
    let restarted=ConversationAuthority::new();
    assert!(restarted.authorize_worker_call(
        &trusted,&inv,&desc,&CheckedHostPolicy{allow:true}).is_err());
    drop(journal);drop(book);fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_worker_then_new_task_cannot_resurrect_earlier_effect(){
    let root=temp();fs::create_dir_all(&root).unwrap();
    let original;
    {
        let book=book(&root);
        book.create_session("chat-one","workspace-one","M5 interruption").unwrap();
        book.start_chapter("chat-one","Crash and new intent").unwrap();
        original=book.append_entry("chat-one","turn-before",EntryRole::User,
            "Try first","An authorized operation may be interrupted").unwrap();
        let verifier=OriginalBookVerifier{book:&book};
        let auth=ConversationAuthority::new();
        auth.open_user_task(grant("chat-one","turn-before",
            "task-before","m5-fixture-only",ToolEffect::WriteWorkspace),&verifier
        ).unwrap();
        let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        continuity.append_verified(&TaskCheckpoint{
            checkpoint_id:"cp-m5-before".into(),project_id:"lu".into(),
            task_id:"memory".into(),conversation_id:"chat-one".into(),
            source_entry_ids:vec![original],
            completed_work:"Worker started but not completed".into(),
            next_action:"Reconcile before any new effect".into(),
            blockers:vec!["Interrupted".into()]
        },&book).unwrap();
        let inv=call("m5-uncertain","m5-fixture-only");
        journal.prepare(&inv).unwrap();
        journal.mark_executing(&inv.call_id).unwrap();
        auth.revoke_conversation("chat-one").unwrap();
        assert!(auth.authorize_worker_call(
            &origin("chat-one","turn-before","task-before"),&inv,
            &descriptor(ToolEffect::WriteWorkspace),&CheckedHostPolicy{allow:true}
        ).is_err());
        // Previous native worker process is assumed dead before reopening.
    }
    {
        let book=book(&root);
        let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        assert_eq!(journal.reconcile_interrupted().unwrap(),1);
        assert_eq!(journal.get("m5-uncertain").unwrap().unwrap().state,
            EffectState::Uncertain);
        assert_eq!(journal.prepare(&call("m5-uncertain","m5-fixture-only")).unwrap(),
            PrepareOutcome::Existing(EffectState::Uncertain));
        assert!(journal.mark_executing("m5-uncertain").is_err());
        assert!(continuity.link_verified_effect(
            "cp-m5-before","m5-uncertain",&journal
        ).is_err());
        assert!(continuity.verified_effects_for_checkpoint(
            "cp-m5-before",&journal).unwrap().is_empty());
        book.append_entry("chat-one","turn-after",EntryRole::User,
            "New intent","A new task may be reviewed, not the old uncertain call").unwrap();
        let verifier=OriginalBookVerifier{book:&book};
        let authority=ConversationAuthority::new();
        let stale=origin("chat-one","turn-before","task-before");
        assert!(authority.authorize_worker_call(
            &stale,&call("m5-uncertain","m5-fixture-only"),
            &descriptor(ToolEffect::WriteWorkspace),&CheckedHostPolicy{allow:true}
        ).is_err());
        authority.open_user_task(grant("chat-one","turn-after",
            "task-after","m5-fixture-only",ToolEffect::ReadWorkspace),&verifier
        ).unwrap();
        assert!(authority.authorize_worker_call(
            &origin("chat-one","turn-after","task-after"),
            &call("m5-uncertain","m5-fixture-only"),
            &descriptor(ToolEffect::WriteWorkspace),&CheckedHostPolicy{allow:true}
        ).is_err());
        assert!(authority.open_user_task(grant("chat-one","turn-before",
            "task-invented","m5-fixture-only",ToolEffect::Elevated),&verifier
        ).is_err());
        assert!(book.read_entry("chat-one",original).unwrap().is_some());
        assert_eq!(continuity.latest_for_conversation(
            "lu","memory","chat-one"
        ).unwrap().unwrap().checkpoint_id,"cp-m5-before");
    }
    fs::remove_dir_all(root).unwrap();
}
