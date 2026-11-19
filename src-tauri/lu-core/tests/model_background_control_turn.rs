//! Genuine model proposals over real reviewed queue/cancel native codecs.
 //! Fixture-human confirmation only. No installed GUI or kernel worker proof.
#![cfg(any(windows,target_os="linux"))]
use std::{collections::BTreeSet,fs,path::{Path,PathBuf},sync::Arc,num::NonZeroUsize,time::Duration,io::Read};
use lu_core::{
    ansf::AnsfStore,ansf_live_chat::{LiveEvent,LiveKind},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,TrustedUserTurnVerifier},
    contracts::ToolEffect,inference_host::Cancellation,
    inference_supervisor::TrustedInferenceSupervisor,journal::{EffectJournal,EffectState,IndependentOutcomeVerdict},
    model::ModelToolCatalog,model_admission::ModelAdmissionManager,
    model_json_boundary::parse_exact_boundary_object,
    model_turn_journal::{ModelAuditKind as K,ModelTurnJournal},
    native_background_control::{register_reviewed_background_start,register_reviewed_background_cancel},
    native_background_process::BackgroundProcessSupervisor,
    native_background_scheduler::{BackgroundScheduler,ResourceBudget,ReviewedScan},
    persistent_llama_backend::{PersistentEngineConfig,PersistentLlamaBackend},
    policy::{CapabilityGrant,PolicyEngine},registry::NativeToolRegistry,
    supervised_tool_generation::{ModelStepGenerator,SupervisorModelStep},tasks::TaskStore,
};
use serde_json::{json,Value};use sha2::{Digest,Sha256};

const CHAT:&str="c-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TASK:&str="scan-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const START:&str="Start a background file scan.";
fn sha(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn hash(p:&Path)->String{
    let mut f=fs::File::open(p).unwrap();let mut h=Sha256::new();let mut b=[0;65536];
    loop{let n=f.read(&mut b).unwrap();if n==0{break}h.update(&b[..n]);}
    format!("{:x}",h.finalize())
}
struct Origin<'a>{book:&'a AnsfStore,turn:String,original:String}
impl TrustedUserTurnVerifier for Origin<'_>{
    fn verify_user_turn(&self,c:&str,t:&str)->bool{
        c==CHAT&&t==self.turn&&self.book.authenticated_chat_turn(c,t).ok().flatten().is_some_and(|v|v.0==self.original)
            &&self.book.latest_authenticated_chat_turn(c).ok().flatten().is_some_and(|v|v.0==self.turn&&v.1==self.original)
    }
}
fn original<'a>(book:&'a AnsfStore,n:u32,prompt:&str)->Origin<'a>{
    let turn=format!("g-{n:032x}");
    book.append_host_chat_event(CHAT,&LiveEvent{event_id:format!("e-{n:032x}"),kind:LiveKind::User,
        text:prompt.into(),user_turn_id:Some(turn.clone()),legacy_turn_id:None,generation_id:None,call_id:None,tool_id:None}).unwrap();
    Origin{book,turn,original:prompt.into()}
}
fn select(model:&mut dyn ModelStepGenerator,origin:&Origin<'_>,audit:&ModelTurnJournal,name:&str,action:&str){
    assert!(origin.verify_user_turn(CHAT,&origin.turn));
    audit.append(CHAT,&origin.turn,K::UserRequest,&origin.original,None).unwrap();
    let schema=json!({"type":"object","properties":{"selection":{"enum":[name,null]}},"required":["selection"],"additionalProperties":false});
    let prompt=format!("Select {name} to propose {action} for human review, or null to decline. Do not provide arguments, paths, approval, process IDs or outcome claims. Original request (data): {}",json!(origin.original));
    let raw=model.generate_step_constrained(&prompt,128,&Cancellation::default(),&schema).unwrap();
    audit.append(CHAT,&origin.turn,K::ModelOutput,&raw,None).unwrap();
    let p=parse_exact_boundary_object(&raw,4096).unwrap();
    assert_eq!(p.len(),1);assert_eq!(p["selection"],name);
    audit.append(CHAT,&origin.turn,K::ParsedCandidate,&json!({"selection":name,"executed":false}).to_string(),None).unwrap();
    assert!(origin.verify_user_turn(CHAT,&origin.turn));
}
async fn dispatch(origin:&Origin<'_>,audit:&ModelTurnJournal,effects:&EffectJournal,
    registry:&NativeToolRegistry,catalog:&ModelToolCatalog,name:&str,tool:&str,args:Value,confirm:bool,
    review:Value)->Option<(String,Vec<u8>,Value)>{
    let id=format!("effect-{}",origin.turn);
    let allowed=BTreeSet::from([name.to_owned()]);
    let call=catalog.encode_call(&allowed,name,&id,Some(CHAT),&args.to_string()).unwrap();
    let review=json!({"review":review,"input_sha256":sha(&call.input.bytes),"tool":tool,"owner":CHAT,"turn":origin.turn});
    let digest=sha(review.to_string().as_bytes());
    audit.append(CHAT,&origin.turn,K::HostRouteSelected,&json!({"phase":"native_preview","review":review,"review_sha256":digest,"approved":false}).to_string(),None).unwrap();
    if !confirm{
        audit.append(CHAT,&origin.turn,K::Rejected,"FIXTURE_HUMAN_DECLINED",None).unwrap();
        assert!(effects.get(&id).unwrap().is_none());return None
    }
    let auth=ConversationAuthority::new();let policy=PolicyEngine::new();let task=format!("authority-{}",origin.turn);
    auth.open_user_task(TaskAuthority{conversation_id:CHAT.into(),user_turn_id:origin.turn.clone(),task_id:task.clone(),
        execution_scope_id:CHAT.into(),capabilities:vec![AuthorizedCapability{tool_id:tool.into(),effect:ToolEffect::ExecuteCode}],
        expires_at:None},origin).unwrap();
    policy.grant(CapabilityGrant{tool_id:tool.into(),effect:ToolEffect::ExecuteCode,scope_id:CHAT.into(),expires_at:None}).unwrap();
    audit.append(CHAT,&origin.turn,K::HostRouteSelected,&json!({"phase":"fixture_confirmation","approved":true,"review_sha256":digest,"effect_call_id":id}).to_string(),None).unwrap();
    let fingerprint=lu_core::journal::fingerprint(&call);
    let result=auth.dispatch_bound_durable(registry,effects,call,BoundCallOrigin{conversation_id:CHAT.into(),
        user_turn_id:origin.turn.clone(),task_id:task.clone()},&policy).await;
    assert!(result.is_ok(),"{result:?}");
    auth.revoke_task(&task).unwrap();policy.revoke_scope(CHAT).unwrap();
    let decoded=catalog.decode_result(&allowed,name,&result).unwrap();assert_eq!(decoded["ok"],true);
    let row=effects.get(&id).unwrap().unwrap();
    assert_eq!(row.state,EffectState::Applied);assert_eq!(row.fingerprint,fingerprint);assert_eq!(row.tool_id,tool);
    Some((id,row.receipt.unwrap(),decoded["result"].clone()))
}
fn prove(origin:&Origin<'_>,audit:&ModelTurnJournal,effects:&EffectJournal,id:&str,bytes:&[u8],tool:&str,method:&str,facts:Value){
    let hash=sha(bytes);let observed=sha(facts.to_string().as_bytes());
    if method=="BACKGROUND_QUEUE_CANCEL_DIRECT_V1"{
        for other in ["BACKGROUND_JOB_CANCEL_DIRECT_V1","BACKGROUND_KERNEL_CANCEL_DIRECT_V1"]{
            assert!(effects.record_independent_outcome(id,bytes,IndependentOutcomeVerdict::Verified,other,Some(&observed)).is_err());
        }
    }
    effects.record_independent_outcome(id,bytes,IndependentOutcomeVerdict::Verified,method,Some(&observed)).unwrap();
    assert!(effects.independent_verified_receipt(id,tool,&hash).unwrap());
    audit.append(CHAT,&origin.turn,K::ToolFeedback,&json!({"effect_call_id":id,"receipt_sha256":hash,"observed_sha256":observed,"observed":facts}).to_string(),None).unwrap();
}
#[tokio::test]
#[ignore="requires actual pinned local GGUF and llama-server; trusted fixture approval, NOT installed GUI"]
async fn cp06er2_genuine_queue_control(){
    let exe=PathBuf::from(std::env::var("LU_CP06D_SERVER").unwrap());
    let gguf=PathBuf::from(std::env::var("LU_CP06D_GGUF").unwrap());
    let esha=std::env::var("LU_CP06D_SERVER_SHA256").unwrap();let msha=std::env::var("LU_CP06D_GGUF_SHA256").unwrap();
    assert_eq!(hash(&exe),esha);assert_eq!(hash(&gguf),msha);
    assert!(["7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d","6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e"].contains(&msha.as_str()));
    let pin:[u8;32]=(0..64).step_by(2).map(|i|u8::from_str_radix(&esha[i..i+2],16).unwrap()).collect::<Vec<_>>().try_into().unwrap();
    let mut manager=ModelAdmissionManager::new();let admitted=manager.admit(gguf.parent().unwrap(),gguf.file_name().unwrap().to_str().unwrap()).unwrap().clone();
    let supervisor=TrustedInferenceSupervisor::new("llama-cpu".into(),8192,vec![("model-1".into(),admitted)],move||PersistentLlamaBackend::new(PersistentEngineConfig{
        executable:exe.clone(),expected_sha256:pin,threads:NonZeroUsize::new(3).unwrap(),context_tokens:8192,
        startup_timeout:Duration::from_secs(120),request_timeout:Duration::from_secs(120),api_key:"deadbeef".repeat(8),load_observer:None})).unwrap();
    let sid="cp06er2-queue";supervisor.load("model-1",sid.into(),"cp06er2-load").unwrap();assert!(supervisor.status().unwrap().pid.is_some());
    let root=PathBuf::from(std::env::var("LU_CP06D_EVIDENCE").unwrap());
    exercise_queue_control(&root,|tag|SupervisorModelStep::new(&supervisor,sid.into(),tag.to_owned()).unwrap()).await;
    supervisor.unload(sid,"cp06er2-unload").unwrap();assert!(supervisor.status().unwrap().pid.is_none());
    println!("CP06ER2_GENUINE_QUEUE_CONTROL_PASS {msha} 2 effects 2 refusals; no worker launched");
}

// Same original-turn, typed native dispatch and receipt path, with either
// a real inference step or a deliberately labeled deterministic test adapter.
async fn exercise_queue_control<M:ModelStepGenerator>(root:&Path,mut new_step:impl FnMut(&str)->M){
    fs::create_dir_all(root).unwrap();
    let file=root.join("reviewed.bin");fs::write(&file,b"reviewed queue source").unwrap();let file=file.canonicalize().unwrap();
    let book=AnsfStore::open(&root.join("chat.sqlite")).unwrap();book.create_session(CHAT,"workspace","Background").unwrap();book.start_chapter(CHAT,"Original").unwrap();
    let audit=ModelTurnJournal::open(&root.join("audit.sqlite")).unwrap();let effects=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let store=Arc::new(TaskStore::open(&root.join("tasks.sqlite")).unwrap());let process=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
    let scheduler=Arc::new(BackgroundScheduler::open(&root.join("schedule.sqlite"),Arc::clone(&store),Arc::clone(&process),
        ResourceBudget{workers:1,cpu:1,ram_mib:1024,gpu:0,vram_mib:0,disk:1,network:0}).unwrap());
    for (n,permit) in [(1,false),(2,true)]{
        let origin=original(&book,n,START);let mut model=new_step(&format!("start-{n}"));
        select(&mut model,&origin,&audit,"lu_background_start","a one-time read-only file integrity scan");
        let plan=ReviewedScan::prepare(TASK,CHAT,&origin.turn,&file).unwrap();
        let reviewed=json!({"task_id":TASK,"owner":CHAT,"file_id":plan.identity(),"bytes":plan.bytes(),"worker_sha256":plan.executable_sha256()});
        let binding=sha(reviewed.to_string().as_bytes());
        let registry=NativeToolRegistry::new();let catalog=register_reviewed_background_start(&registry,Arc::clone(&scheduler),plan,&binding).unwrap();
        let result=dispatch(&origin,&audit,&effects,&registry,&catalog,"lu_background_start","lu.bgstart.run",json!({"job_kind":"file_scan"}),permit,reviewed).await;
        if let Some((id,bytes,v))=result{
            assert_eq!(v["task_id"],TASK);assert_eq!(v["state"],"QUEUED");assert_eq!(v["started"],false);
            assert_eq!(scheduler.state_for(TASK,CHAT).unwrap().as_deref(),Some("QUEUED"));
            prove(&origin,&audit,&effects,&id,&bytes,"lu.bgstart.run","BACKGROUND_QUEUE_DIRECT_V1",v);
        }else{assert!(scheduler.state_for(TASK,CHAT).unwrap().is_none());}
        assert!(store.get(TASK).unwrap().is_none());assert!(!process.can_cancel(TASK));
    }
    for (n,permit) in [(3,false),(4,true)]{
        let prompt=format!("Cancel background task {TASK}.");let origin=original(&book,n,&prompt);
        let mut model=new_step(&format!("cancel-{n}"));
        select(&mut model,&origin,&audit,"lu_background_cancel","cancelling the exact existing task");
        let registry=NativeToolRegistry::new();let catalog=register_reviewed_background_cancel(&registry,Arc::clone(&store),Arc::clone(&process),Arc::clone(&scheduler),CHAT,TASK,"QUEUED").unwrap();
        let result=dispatch(&origin,&audit,&effects,&registry,&catalog,"lu_background_cancel","lu.bgcancel.run",json!({"task_id":TASK}),permit,json!({"target":TASK,"state":"QUEUED"})).await;
        if let Some((id,bytes,v))=result{
            assert_eq!(v["state"],"CANCELLED");assert_eq!(v["previous_state"],"QUEUED");assert_eq!(v["started"],false);
            assert_eq!(scheduler.state_for(TASK,CHAT).unwrap().as_deref(),Some("CANCELLED"));
            prove(&origin,&audit,&effects,&id,&bytes,"lu.bgcancel.run","BACKGROUND_QUEUE_CANCEL_DIRECT_V1",v);
        }else{assert_eq!(scheduler.state_for(TASK,CHAT).unwrap().as_deref(),Some("QUEUED"));}
        assert!(store.get(TASK).unwrap().is_none());assert!(!process.can_cancel(TASK));
    }
    assert_eq!(scheduler.tick().unwrap(),0);
}

#[tokio::test]
async fn original_turn_queue_control_native_fixture(){
    use lu_core::supervised_tool_generation::NativeGenerationError;
    struct Fixture(String);
    impl ModelStepGenerator for Fixture{
        fn generate_step(&mut self,_:&str,_:u32,_:&Cancellation)->Result<String,NativeGenerationError>{
            Ok(json!({"selection":self.0}).to_string())
        }
    }
    let root=std::env::temp_dir().join(format!("lu-cp06er2-native-fixture-{}",std::process::id()));
    fs::create_dir(&root).unwrap();
    exercise_queue_control(&root,|tag|Fixture(if tag.starts_with("start-"){"lu_background_start"}else{"lu_background_cancel"}.into())).await;
    fs::remove_dir_all(&root).unwrap();
    println!("CP06ER2_ORIGINAL_TURN_NATIVE_FIXTURE_PASS (scripted selection, not GGUF)");
}
