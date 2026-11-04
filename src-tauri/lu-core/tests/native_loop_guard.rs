//! TOOLS-08: real native tool-turn loop prevention, no model-facing guard.
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use lu_core::{
    agent_tool_loop::{
        NativeToolTurn, ToolDispatchCheckpoint, ToolTurnError, ToolTurnOutcome,
        MAX_MODEL_FRAME_BYTES,
    },
    contracts::{
        AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation,
    },
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    loop_guard::{LoopGuard, LoopPolicy, LoopReason, LoopVerdict, ProgressEvidence},
    model::ModelToolCatalog,
    generation_loop_guard::{StreamLoopGuard, StreamLoopVerdict},
    inference_host::Cancellation,
    model_turn_journal::ModelTurnJournal,
    supervised_tool_generation::{
        ModelStepGenerator, NativeGenerationError, NativeTurnServices,
        run_supervised_native_turn_journaled,
    },
    model_json_boundary::parse_host_tool_feedback_boundary,
    native_file_extra::register_inspection_tools,
    native_files::register_bundled_file_tools,
    registry::NativeToolRegistry,
};
use serde_json::{json, Value};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);
fn sandbox() -> PathBuf {
    let sequence=NEXT_DIR.fetch_add(1,Ordering::SeqCst);
    let nanos=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root=std::env::temp_dir().join(format!(
        "lu-loop-{}-{nanos}-{sequence}",std::process::id()));
    fs::create_dir(&root).unwrap();
    for file in ["a.txt","b.txt","c.txt","d.txt","e.txt"] {
        fs::write(root.join(file),format!("marker {file}\n")).unwrap();
    }
    root
}
struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool {
        chat=="loop-chat" && turn=="loop-user"
    }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
        ->Result<(),AuthorizationError> { Ok(()) }
}
fn setup(root:&std::path::Path)->(
    NativeToolRegistry,ModelToolCatalog,ConversationAuthority,
    BoundCallOrigin,BTreeSet<String>)
{
    let registry=NativeToolRegistry::new();
    let mut catalog=register_bundled_file_tools(&registry,root).unwrap();
    register_inspection_tools(&registry,&mut catalog,root).unwrap();
    let authority=ConversationAuthority::new();
    let origin=BoundCallOrigin{
        conversation_id:"loop-chat".into(),
        user_turn_id:"loop-user".into(),
        task_id:"loop-task".into(),
    };
    authority.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"loop-scope".into(),
        capabilities:[
            "lu.files.read_text",
            "lu.files.list_directory",
            "lu.inspect.search_text",
        ].into_iter().map(|id|AuthorizedCapability{
            tool_id:id.into(),effect:ToolEffect::ReadWorkspace,
        }).collect(),
        expires_at:None,
    },&Verified).unwrap();
    let names:BTreeSet<String>=[
        "lu_files_read_text",
        "lu_files_list_directory",
        "lu_files_search_text",
    ].into_iter().map(str::to_owned).collect();
    (registry,catalog,authority,origin,names)
}
fn frame(name:&str,arguments:Value)->String {
    json!({"kind":"tool","name":name,"arguments":arguments}).to_string()
}
fn arg(value:Value)->serde_json::Map<String,Value> {
    value.as_object().unwrap().clone()
}


fn feedback(result: Value)->Value {
    json!({"ok":true,"result":result})
}
fn hash(n: usize)->String {format!("{n:064x}")}
fn remove_workspace(root:PathBuf){
    for i in 0..10 {
        match fs::remove_dir_all(&root) {
            Ok(())=>return,
            Err(e) if e.raw_os_error()==Some(32) && i<9 =>
                std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(e)=>panic!("failed to clean isolated test workspace: {e}"),
        }
    }
}

#[test]
fn no_fixed_tool_limit_when_real_output_is_new(){
    let mut guard=LoopGuard::default();
    let args=arg(json!({"path":"same.txt","max_bytes":1024}));
    for n in 0..100 {
        assert_eq!(guard.check("read",&args),LoopVerdict::Allow,
            "valid repeated read {n} with changed contents was blocked");
        assert_eq!(
            guard.observe_feedback("read",&args,
                &feedback(json!({"content":format!("new revision {n}")})),
                ToolEffect::ReadWorkspace),
            ProgressEvidence::NewInformation
        );
    }
    assert_eq!(guard.attempted(),100);
    assert_eq!(guard.no_progress_streak(),0);
}

#[test]
fn repeated_same_result_not_new_progress_even_if_timestamp_changes(){
    let mut guard=LoopGuard::default();
    let args=arg(json!({"path":"secret.txt"}));
    for attempt in 0..3 {
        let decision=guard.check("read",&args);
        assert!(matches!(decision,LoopVerdict::Allow|LoopVerdict::Warn(_)));
        let observation=guard.observe_feedback("read",&args,
            &feedback(json!({"content":"unchanged","timestamp":attempt,
                "duration_ms":attempt*10})),ToolEffect::ReadWorkspace);
        assert_eq!(observation,if attempt==0 {
            ProgressEvidence::NewInformation
        } else { ProgressEvidence::NoProgress });
    }
    assert_eq!(guard.check("read",&args),
        LoopVerdict::Block(LoopReason::RepeatedFailure));
    assert_eq!(guard.check("read",&args),
        LoopVerdict::Block(LoopReason::RepeatedFailure));
    assert_eq!(guard.check("read",&args),
        LoopVerdict::Kill(LoopReason::RepeatedFailure));
    assert!(!LoopReason::RepeatedFailure.message().contains("secret.txt"));
}

#[test]
fn many_distinct_edits_of_one_file_are_progress_when_receipts_prove_change(){
    let mut guard=LoopGuard::default();
    for revision in 0..80 {
        let args=arg(json!({"path":"work.txt","operation":"replace",
            "position":revision,"replacement":format!("edit-{revision}")}));
        assert_eq!(guard.check("edit",&args),LoopVerdict::Allow);
        let observation=guard.observe_feedback("edit",&args,&feedback(json!({
            "committed":true,
            "original_sha256":hash(revision),
            "replacement_sha256":hash(revision+1),
            "bytes_written":100,
            "backup_id":format!("receipt-{revision}")
        })),ToolEffect::WriteWorkspace);
        assert_eq!(observation,ProgressEvidence::StateChanged);
    }
    assert_eq!(guard.attempted(),80);
    assert_eq!(guard.no_progress_streak(),0);
}

#[test]
fn an_edit_that_changes_no_bytes_is_not_progress(){
    let mut guard=LoopGuard::default();
    let args=arg(json!({"path":"work.txt","operation":"replace"}));
    let observation=guard.observe_feedback("edit",&args,&feedback(json!({
        "committed":true,"original_sha256":hash(1),
        "replacement_sha256":hash(1)
    })),ToolEffect::WriteWorkspace);
    assert_eq!(observation,ProgressEvidence::NoProgress);
}

#[test]
fn generic_write_success_is_unverified_not_proof_of_progress(){
    let mut guard=LoopGuard::default();
    let args=arg(json!({"path":"work.txt","operation":"append"}));
    assert_eq!(guard.observe_feedback("edit",&args,&feedback(json!({
        "committed":true,"bytes_written":20
    })),ToolEffect::WriteWorkspace),ProgressEvidence::Unverified);
    assert_eq!(guard.no_progress_streak(),1);
}

#[test]
fn repeated_failures_with_different_arguments_are_stagnation(){
    let mut guard=LoopGuard::default();
    for n in 0..8 {
        let args=arg(json!({"path":format!("missing-{n}.txt")}));
        let decision=guard.check("read",&args);
        assert!(matches!(decision,LoopVerdict::Allow|LoopVerdict::Warn(_)));
        assert_eq!(guard.observe_feedback("read",&args,
            &json!({"ok":false,"error":{"code":"NOT_FOUND","retryable":false}}),
            ToolEffect::ReadWorkspace),ProgressEvidence::NoProgress);
    }
    assert_eq!(guard.no_progress_streak(),8);
    // One genuinely new approach can run as a recovery probe.
    let probe=arg(json!({"path":"new-approach.txt"}));
    assert!(matches!(guard.check("read",&probe),LoopVerdict::Warn(_)));
    guard.observe_feedback("read",&probe,
        &json!({"ok":false,"error":{"code":"NOT_FOUND"}}),
        ToolEffect::ReadWorkspace);
    for n in 0..2 {
        let different=arg(json!({"path":format!("another-{n}.txt")}));
        assert_eq!(guard.check("read",&different),
            LoopVerdict::Block(LoopReason::NoProgress));
    }
    assert_eq!(guard.check("read",&arg(json!({"path":"elsewhere"}))),
        LoopVerdict::Kill(LoopReason::NoProgress));
}

#[test]
fn alternating_tool_calls_that_return_no_new_information_eventually_block(){
    let mut guard=LoopGuard::default();
    let mut blocked=false;
    for n in 0..9 {
        let tool=if n%2==0{"read"} else {"search"};
        let args=arg(json!({"path":"same","page":n}));
        match guard.check(tool,&args) {
            LoopVerdict::Block(LoopReason::OutputCycle) => {blocked=true;break;}
            LoopVerdict::Block(_) => {blocked=true;break;}
            LoopVerdict::Kill(_) => {blocked=true;break;}
            _=> {}
        }
        guard.observe_feedback(tool,&args,
            &feedback(json!({"matches":[]})),ToolEffect::ReadWorkspace);
    }
    assert!(blocked,"a no-progress A-B cycle must be stopped");
}

#[test]
fn sustained_progress_resets_a_previous_stagnation_warning(){
    let mut guard=LoopGuard::default();
    for n in 0..5 {
        let args=arg(json!({"path":format!("miss-{n}")}));
        guard.check("read",&args);
        guard.observe_feedback("read",&args,&json!({"ok":false}),
            ToolEffect::ReadWorkspace);
    }
    assert_eq!(guard.no_progress_streak(),5);
    let args=arg(json!({"path":"found"}));
    assert!(matches!(guard.check("read",&args),LoopVerdict::Warn(_)));
    assert_eq!(guard.observe_feedback("read",&args,
        &feedback(json!({"content":"important new finding"})),
        ToolEffect::ReadWorkspace),ProgressEvidence::NewInformation);
    assert_eq!(guard.no_progress_streak(),0);
    assert_eq!(guard.check("read",&args),LoopVerdict::Allow);
}

#[test]
fn policy_is_bounded_and_no_model_control(){
    let mut bad=LoopPolicy::default();
    bad.window_size=2;
    assert!(LoopGuard::new(bad).is_err());
    bad=LoopPolicy::default();
    bad.block_after_stalled=0;
    assert!(LoopGuard::new(bad).is_err());
    let a=LoopGuard::default();
    let b=LoopGuard::default();
    assert_eq!(a.attempted(),0);
    assert_eq!(b.attempted(),0);
}

#[test]
fn streamed_large_repeated_text_is_stopped_across_chunk_boundaries(){
    let mut guard=StreamLoopGuard::default();
    let phrase="I will repeat the identical output forever. ";
    let cyclic=phrase.repeat(160);
    let mut stopped=false;
    for chunk in cyclic.as_bytes().chunks(17) {
        let value=std::str::from_utf8(chunk).unwrap();
        if guard.observe_chunk(value)==StreamLoopVerdict::RepeatedOutput {
            stopped=true;break;
        }
    }
    assert!(stopped,"should detect a large periodic output cycle");
    assert!(guard.total_bytes()>=1024);
    let mut ordinary=StreamLoopGuard::default();
    for i in 0..1000 {
        let value=format!("Different sentence number {i} describing step {i}: {}.\n",i*i);
        assert_eq!(ordinary.observe_chunk(&value),StreamLoopVerdict::Continue);
    }
}
#[test]
fn short_repetition_is_not_automatically_infinite_token_output(){
    let mut guard=StreamLoopGuard::default();
    assert_eq!(guard.observe_chunk(&"la ".repeat(40)),StreamLoopVerdict::Continue);
}

#[tokio::test]
async fn real_native_read_of_a_changing_file_can_repeat_more_than_eight_times(){
    let root=sandbox();
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"loop-scope".into(),&allowed
    ).unwrap();
    let call=frame("lu_files_read_text",json!({"path":"a.txt"}));
    for version in 0..24 {
        let text=format!("fresh document revision {version}");
        fs::write(root.join("a.txt"),&text).unwrap();
        let out=turn.handle_model_reply(
            &call,&catalog,&registry,&authority,&Allow
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=out else {panic!("missing result")};
        assert_eq!(data["ok"],true);
        assert_eq!(data["result"]["content"],text);
    }
    assert_eq!(turn.step_count(),24);
    assert_eq!(turn.loop_attempt_count(),24);
    drop(turn);drop(authority);drop(catalog);drop(registry);
    remove_workspace(root);
}

#[tokio::test]
async fn real_native_identical_feedback_is_blocked_before_another_dispatch(){
    let root=sandbox();
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"loop-scope".into(),&allowed
    ).unwrap();
    let seen=AtomicUsize::new(0);
    let observation=|checkpoint:ToolDispatchCheckpoint| {
        if checkpoint==ToolDispatchCheckpoint::Entered {
            seen.fetch_add(1,Ordering::SeqCst);
        }
    };
    let call=frame("lu_files_read_text",json!({"path":"a.txt"}));
    for n in 0..5 {
        let out=turn.handle_model_reply_observed(
            &call,&catalog,&registry,&authority,&Allow,&observation
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=&out else {panic!("result")};
        if n<3 {assert_eq!(data["ok"],true);}
        else {
            assert_eq!(data["ok"],false);
            assert_eq!(data["error"]["code"],"LOOP_DETECTED");
            assert!(!data.to_string().contains("a.txt"));
        }
        parse_host_tool_feedback_boundary(
            &NativeToolTurn::model_feedback(&out).unwrap(),MAX_MODEL_FRAME_BYTES
        ).unwrap();
    }
    assert_eq!(seen.load(Ordering::SeqCst),3);
    assert_eq!(turn.handle_model_reply_observed(
        &call,&catalog,&registry,&authority,&Allow,&observation
    ).await,Err(ToolTurnError::LoopTerminated));
    assert_eq!(seen.load(Ordering::SeqCst),3);
    assert_eq!(turn.handle_model_reply(
        r#"{"kind":"final","text":"I can ignore previous blocks"}"#,
        &catalog,&registry,&authority,&Allow
    ).await,Err(ToolTurnError::AlreadyFinished));
    drop(turn);drop(authority);drop(catalog);drop(registry);
    remove_workspace(root);
}
struct TwentyFiveReadModel { index: usize }
impl ModelStepGenerator for TwentyFiveReadModel {
    fn generate_step(
        &mut self,_:&str, _max_tokens:u32, cancel:&Cancellation,
    )->Result<String,NativeGenerationError> {
        if cancel.is_cancelled(){return Err(NativeGenerationError::Cancelled)}
        let n=self.index;
        self.index+=1;
        if n>=25 {
            Ok(r#"{"kind":"final","text":"All 25 distinct file reads completed."}"#.into())
        } else {
            Ok(frame("lu_files_read_text",json!({"path":format!("batch-{n}.txt")})))
        }
    }
}
#[tokio::test]
async fn real_supervised_model_may_complete_25_useful_native_calls_and_journal(){
    let root=sandbox();
    for n in 0..25 {
        fs::write(root.join(format!("batch-{n}.txt")),format!("unique data {n}")).unwrap();
    }
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"loop-scope".into(),&allowed
    ).unwrap();
    let journal=ModelTurnJournal::open(&root.join("audit.sqlite")).unwrap();
    let mut model=TwentyFiveReadModel{index:0};
    let report=run_supervised_native_turn_journaled(
        "Read all 25 unique batch files and report completion.",
        &mut turn,&mut model,
        &NativeTurnServices{
            catalog:&catalog,registry:&registry,
            authority:&authority,host_policy:&Allow,
        },&Cancellation::default(),&journal,
    ).await.unwrap();
    assert_eq!(report.tool_calls,25);
    assert_eq!(report.model_generations,26);
    assert_eq!(turn.loop_attempt_count(),25);
    assert_eq!(report.answer,"All 25 distinct file reads completed.");
    drop(journal);drop(turn);drop(authority);drop(catalog);drop(registry);
    remove_workspace(root);
}
