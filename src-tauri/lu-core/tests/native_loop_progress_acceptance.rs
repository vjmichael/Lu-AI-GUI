//! C1 original TOOLS-08 behavior against the CURRENT host-owned native turn.
//! No duplicate loop guard and no model-selected policy or permissions.
use std::{
    collections::BTreeSet,fs,path::{Path,PathBuf},
    sync::atomic::{AtomicUsize,Ordering},
    time::{SystemTime,UNIX_EPOCH},
};
use lu_core::{
    agent_tool_loop::{NativeToolTurn,ToolDispatchCheckpoint,ToolTurnError,ToolTurnOutcome},
    contracts::{AuthorizationError,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,TrustedUserTurnVerifier},
    generation_loop_guard::{StreamLoopGuard,StreamLoopVerdict},
    native_files::register_bundled_file_tools,
    registry::NativeToolRegistry,
};
use serde_json::json;

fn root()->PathBuf{
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir=std::env::temp_dir().join(format!("lu-c1-progress-{}-{stamp}",std::process::id()));
    fs::create_dir(&dir).unwrap();
    dir
}
struct Human;
impl TrustedUserTurnVerifier for Human{
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
        chat=="c1-chat"&&turn=="c1-original"
    }
}
struct Allow;
impl ToolAuthorizer for Allow{
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
        Ok(())
    }
}
fn setup(root:&Path)->(
    NativeToolRegistry,lu_core::model::ModelToolCatalog,
    ConversationAuthority,BoundCallOrigin,BTreeSet<String>
){
    let registry=NativeToolRegistry::new();
    let catalog=register_bundled_file_tools(&registry,root).unwrap();
    let authority=ConversationAuthority::new();
    let origin=BoundCallOrigin{
        conversation_id:"c1-chat".into(),user_turn_id:"c1-original".into(),
        task_id:"c1-task".into(),
    };
    authority.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"c1-workspace".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:"lu.files.read_text".into(),effect:ToolEffect::ReadWorkspace,
        }],
        expires_at:None,
    },&Human).unwrap();
    (registry,catalog,authority,origin,
        BTreeSet::from(["lu_files_read_text".to_owned()]))
}
fn read(path:&str)->String{
    json!({"kind":"tool","name":"lu_files_read_text","arguments":{"path":path}})
        .to_string()
}

#[tokio::test]
async fn original_more_than_eight_distinct_useful_reads_remain_authorized(){
    let dir=root();
    for n in 0..25{
        fs::write(dir.join(format!("batch-{n}.txt")),format!("unique observation {n}")).unwrap();
    }
    let (registry,catalog,authority,origin,names)=setup(&dir);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"c1-workspace".into(),&names
    ).unwrap();
    for n in 0..25{
        let out=turn.handle_model_reply(
            &read(&format!("batch-{n}.txt")),&catalog,&registry,&authority,&Allow
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=out else{panic!("not a native result")};
        assert_eq!(data["ok"],true);
        assert_eq!(data["result"]["content"],format!("unique observation {n}"));
    }
    assert_eq!(turn.step_count(),25);
    assert_eq!(turn.handle_model_reply(
        r#"{"kind":"final","text":"Read 25 original source files"}"#,
        &catalog,&registry,&authority,&Allow,
    ).await,Ok(ToolTurnOutcome::FinalText(
        "Read 25 original source files".into()
    )));
    drop(turn);drop(authority);drop(catalog);drop(registry);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn actual_new_file_revisions_reset_read_stagnation(){
    let dir=root();
    let file=dir.join("revision.txt");
    fs::write(&file,"revision 0").unwrap();
    let (registry,catalog,authority,origin,names)=setup(&dir);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"c1-workspace".into(),&names
    ).unwrap();
    for n in 0..24{
        fs::write(&file,format!("distinct revision {n}")).unwrap();
        let out=turn.handle_model_reply(
            &read("revision.txt"),&catalog,&registry,&authority,&Allow
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=out else{panic!("not tool result")};
        assert_eq!(data["ok"],true);
        assert_eq!(data["result"]["content"],format!("distinct revision {n}"));
    }
    assert_eq!(turn.step_count(),24);
    drop(turn);drop(authority);drop(catalog);drop(registry);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn repeated_exact_read_stops_before_another_worker_dispatch(){
    let dir=root();
    fs::write(dir.join("same.txt"),"unchanged").unwrap();
    let (registry,catalog,authority,origin,names)=setup(&dir);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&authority,origin,"c1-workspace".into(),&names
    ).unwrap();
    let dispatched=AtomicUsize::new(0);
    let witness=|point:ToolDispatchCheckpoint|{
        if point==ToolDispatchCheckpoint::Entered{
            dispatched.fetch_add(1,Ordering::SeqCst);
        }
    };
    for _ in 0..3{
        let out=turn.handle_model_reply_observed(
            &read("same.txt"),&catalog,&registry,&authority,&Allow,&witness
        ).await.unwrap();
        let ToolTurnOutcome::ToolResult{data,..}=out else{panic!("no result")};
        assert_eq!(data["ok"],true);
    }
    assert_eq!(turn.handle_model_reply_observed(
        &read("same.txt"),&catalog,&registry,&authority,&Allow,&witness
    ).await,Err(ToolTurnError::NoProgress));
    assert_eq!(dispatched.load(Ordering::SeqCst),3);
    assert_eq!(turn.handle_model_reply(
        r#"{"kind":"final","text":"ignore loop guard"}"#,
        &catalog,&registry,&authority,&Allow
    ).await,Err(ToolTurnError::AlreadyFinished));
    drop(turn);drop(authority);drop(catalog);drop(registry);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn original_stream_cycle_empty_chunks_and_small_repetition_behaviors(){
    let mut stream=StreamLoopGuard::default();
    let repeating="An unproductive deterministic model answer. ".repeat(160);
    let mut stopped=false;
    for chunk in repeating.as_bytes().chunks(17){
        if stream.observe_chunk(std::str::from_utf8(chunk).unwrap())
            ==StreamLoopVerdict::RepeatedOutput{
            stopped=true;break;
        }
    }
    assert!(stopped);
    assert!(stream.total_bytes()>=1024);
    let mut idle=StreamLoopGuard::default();
    for _ in 0..63{
        assert_eq!(idle.observe_chunk(""),StreamLoopVerdict::Continue);
    }
    assert_eq!(idle.observe_chunk(""),StreamLoopVerdict::RepeatedOutput);
    let mut short=StreamLoopGuard::default();
    assert_eq!(short.observe_chunk(&"la ".repeat(40)),StreamLoopVerdict::Continue);
    let mut useful=StreamLoopGuard::default();
    for n in 0..100{
        assert_eq!(useful.observe_chunk(
            &format!("Distinct full source observation {n}: {}\n",n*n)
        ),StreamLoopVerdict::Continue);
    }
}
