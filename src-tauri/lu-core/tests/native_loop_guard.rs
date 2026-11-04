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
    loop_guard::{LoopGuard, LoopPolicy, LoopReason, LoopVerdict},
    model::ModelToolCatalog,
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

#[test]
fn exact_repeat_blocks_third_and_kills_after_three_consecutive_blocks() {
    let mut guard=LoopGuard::default();
    let args=arg(json!({"path":"secret.txt","max_bytes":64}));
    assert_eq!(guard.check("lu_files_read_text",&args),LoopVerdict::Allow);
    assert_eq!(guard.check("lu_files_read_text",&args),LoopVerdict::Allow);
    assert_eq!(guard.check("lu_files_read_text",&args),
        LoopVerdict::Block(LoopReason::ExactRepeat));
    assert_eq!(guard.check("lu_files_read_text",&args),
        LoopVerdict::Block(LoopReason::ExactRepeat));
    assert_eq!(guard.check("lu_files_read_text",&args),
        LoopVerdict::Kill(LoopReason::ExactRepeat));
    assert_eq!(guard.attempted(),5);
    assert_eq!(guard.consecutive_blocks(),3);
    assert!(!LoopReason::ExactRepeat.message().contains("secret.txt"));
}

#[test]
fn pingpong_needs_real_alternation_and_not_merely_two_different_tools() {
    let mut guard=LoopGuard::default();
    for (tool,n) in [
        ("a",1),("b",2),("a",3),("b",4),
    ]{
        assert_eq!(guard.check(tool,&arg(json!({"path":n}))),LoopVerdict::Allow);
    }
    assert_eq!(guard.check("a",&arg(json!({"path":5}))),
        LoopVerdict::Block(LoopReason::PingPong));
    assert_eq!(guard.check("a",&arg(json!({"path":6}))),
        LoopVerdict::Block(LoopReason::PingPong));
    assert_eq!(guard.check("a",&arg(json!({"path":7}))),
        LoopVerdict::Kill(LoopReason::PingPong));
}

#[test]
fn similar_arguments_warn_but_different_progress_is_allowed() {
    let mut guard=LoopGuard::default();
    for query in ["alpha","beta","gamma"]{
        let args=arg(json!({
            "path":"work","query":query,"max_files":32,"max_matches":40
        }));
        assert_eq!(guard.check("lu_files_search_text",&args),LoopVerdict::Allow);
    }
    let result=guard.check("lu_files_search_text",&arg(json!({
        "path":"work","query":"delta","max_files":32,"max_matches":40
    })));
    assert_eq!(result,LoopVerdict::Warn(LoopReason::SimilarArguments));
    assert_eq!(guard.consecutive_blocks(),0);
    assert_eq!(guard.attempted(),4);
}

#[test]
fn original_total_call_counter_is_independent_of_five_entry_window() {
    let mut guard=LoopGuard::default();
    for i in 1..20 {
        let args=arg(json!({"path":format!("different-{i}")}));
        assert_eq!(guard.check("read",&args),LoopVerdict::Allow);
    }
    assert_eq!(guard.check("read",&arg(json!({"path":"different-20"}))),
        LoopVerdict::Warn(LoopReason::ExcessiveCalls));
    assert_eq!(guard.attempted(),20);
}

#[test]
fn policy_is_host_owned_bounded_and_per_turn() {
    let mut broken=LoopPolicy::default();
    broken.window_size=3;
    assert!(LoopGuard::new(broken).is_err());
    broken=LoopPolicy::default();
    broken.kill_after_blocks=0;
    assert!(LoopGuard::new(broken).is_err());
    broken=LoopPolicy::default();
    broken.similarity_threshold=f64::NAN;
    assert!(LoopGuard::new(broken).is_err());
    let first=LoopGuard::default();
    let second=LoopGuard::default();
    assert_eq!(first.attempted(),0);
    assert_eq!(second.attempted(),0);
}

#[tokio::test]
async fn real_native_dispatch_blocks_repetition_before_effect_or_authorizer() {
    let root=sandbox();
    let original=fs::read(root.join("a.txt")).unwrap();
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"loop-scope".into(),&allowed
    ).unwrap();
    let events=AtomicUsize::new(0);
    let observer=|stage:ToolDispatchCheckpoint|{
        if stage==ToolDispatchCheckpoint::Entered {
            events.fetch_add(1,Ordering::SeqCst);
        }
    };
    let call=frame("lu_files_read_text",json!({"path":"a.txt"}));
    for i in 1..=4 {
        let result=turn.handle_model_reply_observed(
            &call,&catalog,&registry,&authority,&Allow,&observer
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult {data,..}= &result else {
            panic!("missing typed tool feedback");
        };
        if i<=2 {
            assert_eq!(data["ok"],true);
        } else {
            assert_eq!(data["ok"],false);
            assert_eq!(data["error"]["code"],"LOOP_DETECTED");
            assert_eq!(data["error"]["retryable"],false);
            assert!(!data.to_string().contains("a.txt"),
                "guard response must never echo path arguments");
            let serialized=NativeToolTurn::model_feedback(&result).unwrap();
            parse_host_tool_feedback_boundary(&serialized,MAX_MODEL_FRAME_BYTES).unwrap();
        }
    }
    assert_eq!(events.load(Ordering::SeqCst),2,"only first two calls dispatched");
    assert_eq!(turn.step_count(),4);
    assert_eq!(turn.loop_attempt_count(),4);
    assert_eq!(turn.handle_model_reply_observed(
        &call,&catalog,&registry,&authority,&Allow,&observer
    ).await,Err(ToolTurnError::LoopTerminated));
    assert_eq!(events.load(Ordering::SeqCst),2);
    assert_eq!(turn.step_count(),4,"kill cannot issue a new native call");
    assert_eq!(turn.handle_model_reply(
        r#"{"kind":"final","text":"I will continue"}"#,
        &catalog,&registry,&authority,&Allow
    ).await,Err(ToolTurnError::AlreadyFinished));
    assert_eq!(fs::read(root.join("a.txt")).unwrap(),original);
    drop(turn);drop(authority);drop(catalog);drop(registry);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn real_native_pingpong_blocks_without_issuing_fifth_call() {
    let root=sandbox();
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"loop-scope".into(),&allowed
    ).unwrap();
    for (name,args) in [
        ("lu_files_read_text",json!({"path":"a.txt"})),
        ("lu_files_list_directory",json!({"limit":10})),
        ("lu_files_read_text",json!({"path":"b.txt"})),
        ("lu_files_list_directory",json!({"limit":11})),
    ]{
        let outcome=turn.handle_model_reply(
            &frame(name,args),&catalog,&registry,&authority,&Allow
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult {data,..}=outcome else {
            panic!("missing real native result")
        };
        assert_eq!(data["ok"],true);
    }
    let output=turn.handle_model_reply(
        &frame("lu_files_read_text",json!({"path":"c.txt"})),
        &catalog,&registry,&authority,&Allow
    ).await.unwrap();
    let ToolTurnOutcome::ToolResult{data,..}=output else {panic!("missing synthetic block")};
    assert_eq!(data["ok"],false);
    assert_eq!(data["error"]["code"],"LOOP_DETECTED");
    assert!(data["error"]["message"].as_str().unwrap().contains("Alternating"));
    drop(turn);drop(authority);drop(catalog);drop(registry);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn real_native_similarity_warning_does_not_modify_outer_feedback_envelope() {
    let root=sandbox();
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"loop-scope".into(),&allowed
    ).unwrap();
    for (index,phrase) in ["alpha","beta","gamma","delta"].iter().enumerate() {
        let output=turn.handle_model_reply(
            &frame("lu_files_search_text",json!({
                "path":"","query":phrase,"max_files":8,"max_matches":8
            })),&catalog,&registry,&authority,&Allow
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=&output else {
            panic!("no native result")
        };
        assert_eq!(data["ok"],true);
        if index==3 {
            assert!(data["result"]["_lu_loop_warning"].as_str().unwrap()
                .contains("repeatedly called"));
        } else {
            assert!(data["result"].get("_lu_loop_warning").is_none());
        }
        parse_host_tool_feedback_boundary(
            &NativeToolTurn::model_feedback(&output).unwrap(),MAX_MODEL_FRAME_BYTES
        ).unwrap();
    }
    drop(turn);drop(authority);drop(catalog);drop(registry);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn a_new_host_owned_turn_starts_with_empty_loop_history() {
    let root=sandbox();
    let (registry,catalog,authority,origin,allowed)=setup(&root);
    let call=frame("lu_files_read_text",json!({"path":"a.txt"}));
    for _turn in 0..2 {
        let mut turn=NativeToolTurn::new_read_only(
            &catalog,&registry,&authority,origin.clone(),"loop-scope".into(),&allowed
        ).unwrap();
        assert_eq!(turn.loop_attempt_count(),0);
        let output=turn.handle_model_reply(
            &call,&catalog,&registry,&authority,&Allow
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=output else {panic!("missing read")};
        assert_eq!(data["ok"],true);
        assert_eq!(turn.loop_attempt_count(),1);
    }
    fs::remove_dir_all(root).unwrap();
}
