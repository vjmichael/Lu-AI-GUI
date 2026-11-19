//! CP-06D. Real registry, exact typed authorization, effect journal, and
//! authenticated ANSF original events. Native confirmation is a test-host
//! decision: this suite does not simulate success of an installed OS dialog.
use std::{collections::VecDeque,fs,path::{Path,PathBuf},sync::{Arc,atomic::{AtomicBool,AtomicU64,Ordering}},io::Read};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{ansf::AnsfStore,ansf_live_chat::{LiveEvent,LiveKind},conversation_authority::TrustedUserTurnVerifier,
    inference_host::Cancellation,journal::{EffectJournal,EffectState},model_transfer_turn::{TransferIntent,TransferAction,TransferProposal},
    model_turn_journal::{ModelAuditKind as K,ModelTurnJournal},native_file_transfer::source_digest,
    supervised_tool_generation::{ModelStepGenerator,NativeGenerationError}};
use serde_json::Value;
use sha2::{Digest,Sha256};
static SEQ:AtomicU64=AtomicU64::new(0);
const ORIGINAL:&str="source.txt";const TARGET:&str="reviewed-new.txt";const CHAT:&str="c-transfer";
const TURN:&str="g-00000000000000000000000000000001";const EFFECT:&str="effect-transfer-00000001";
const COPY:&str="Copy the file source.txt";const MOVE:&str="Move the file source.txt to a new name";
const COPY_SELECTION:&str=r#"{"selection":"copy_file"}"#;const MOVE_SELECTION:&str=r#"{"selection":"move_file"}"#;const DONE:&str=r#"{"status":"verified_completion"}"#;
fn event(n:u32,kind:LiveKind,text:&str)->LiveEvent{LiveEvent{event_id:format!("e-{n:032x}"),kind,text:text.into(),user_turn_id:(kind==LiveKind::User).then(||TURN.into()),legacy_turn_id:None,generation_id:(kind!=LiveKind::User).then(||TURN.into()),call_id:None,tool_id:None}}
struct Fixture{base:PathBuf,folder:PathBuf,dir:Arc<Dir>,book:AnsfStore,audit:ModelTurnJournal,effects:EffectJournal,original:String,live:AtomicBool}
impl Fixture{
    fn new(original:&str,base:Option<PathBuf>)->Self{
        let base=base.unwrap_or_else(||std::env::temp_dir().join(format!("lu-cp06d-{}-{}",std::process::id(),SEQ.fetch_add(1,Ordering::Relaxed))));
        fs::create_dir_all(&base).unwrap();let folder=base.join("approved");fs::create_dir(&folder).unwrap();fs::write(folder.join(ORIGINAL),b"Original fixture content.\n").unwrap();
        let dir=Arc::new(Dir::open_ambient_dir(&folder,ambient_authority()).unwrap());
        let book=AnsfStore::open(&base.join("chat.sqlite")).unwrap();book.create_session(CHAT,"workspace","Transfer").unwrap();book.start_chapter(CHAT,"Original").unwrap();book.append_host_chat_event(CHAT,&event(1,LiveKind::User,original)).unwrap();
        let audit=ModelTurnJournal::open(&base.join("audit.sqlite")).unwrap();let effects=EffectJournal::open(&base.join("effects.sqlite")).unwrap();
        Self{base,folder,dir,book,audit,effects,original:original.into(),live:AtomicBool::new(true)}
    }
    fn live(&self)->Result<(),String>{if self.live.load(Ordering::SeqCst)&&self.verify_user_turn(CHAT,TURN){Ok(())}else{Err("Cancelled fixture".into())}}
    fn propose(&self,model:&mut dyn ModelStepGenerator)->Result<Option<TransferProposal>,String>{TransferIntent::parse(&self.original).unwrap().propose(CHAT,TURN,self,model,&Cancellation::default(),&self.audit,&||self.live())}
    fn review(&self,proposal:TransferProposal)->lu_core::model_transfer_turn::ReviewedModelTransfer{let (bytes,sha)=source_digest(&self.dir,ORIGINAL).unwrap();proposal.review(Arc::clone(&self.dir),ORIGINAL,TARGET,bytes,&sha,&self.audit).unwrap()}
    fn assert_verified(&self,action:TransferAction,answer:&str,origin:&str){
        let events=self.audit.events_for(CHAT,TURN).unwrap();assert!(events.iter().all(|e|e.kind!=K::FinalAnswer));let feedback:Value=serde_json::from_str(&events.iter().find(|e|e.kind==K::ToolFeedback).unwrap().payload).unwrap();
        let final_row:Value=serde_json::from_str(&events.iter().find(|e|e.kind==K::EvidencePresentation).unwrap().payload).unwrap();assert_eq!(final_row["origin"],origin);assert_eq!(final_row["answer"],answer);assert_eq!(final_row["effect_call_id"],EFFECT);
        let row=self.effects.get(EFFECT).unwrap().unwrap();assert_eq!(row.state,EffectState::Applied);let bytes=row.receipt.unwrap();let sha=format!("{:x}",Sha256::digest(&bytes));assert_eq!(feedback["receipt_sha256"],sha);assert_eq!(final_row["receipt_sha256"],sha);assert!(self.effects.independent_verified_receipt(EFFECT,action.native_id(),&sha).unwrap());
        assert_eq!(fs::read(self.folder.join(TARGET)).unwrap(),b"Original fixture content.\n");assert_eq!(self.folder.join(ORIGINAL).exists(),action==TransferAction::Copy);if action==TransferAction::Copy{assert_eq!(fs::read(self.folder.join(ORIGINAL)).unwrap(),b"Original fixture content.\n")}
        let reopened=ModelTurnJournal::open(&self.base.join("audit.sqlite")).unwrap();assert_eq!(events,reopened.events_for(CHAT,TURN).unwrap());let ef=EffectJournal::open(&self.base.join("effects.sqlite")).unwrap();assert!(ef.independent_verified_receipt(EFFECT,action.native_id(),&sha).unwrap());
    }
    fn finish(&self,answer:&str){self.book.append_host_chat_event(CHAT,&event(2,LiveKind::AssistantProgress,answer)).unwrap();self.book.append_host_chat_event(CHAT,&event(3,LiveKind::AssistantCompleted,"")).unwrap();let b=AnsfStore::open(&self.base.join("chat.sqlite")).unwrap();assert_eq!(b.generation_terminal_kind(CHAT,TURN).unwrap(),Some(LiveKind::AssistantCompleted));assert_eq!(b.authenticated_chat_turn(CHAT,TURN).unwrap().unwrap().0,self.original);}
    fn cleanup(self){let path=self.base.clone();drop(self);fs::remove_dir_all(path).unwrap();}
}
impl TrustedUserTurnVerifier for Fixture{fn verify_user_turn(&self,c:&str,t:&str)->bool{c==CHAT&&t==TURN&&self.book.authenticated_chat_turn(c,t).ok().flatten().is_some_and(|v|v.0==self.original)&&self.book.latest_authenticated_chat_turn(c).ok().flatten().is_some_and(|v|v.0==TURN&&v.1==self.original)}}
struct Scripted(VecDeque<String>);
impl Scripted{fn new(v:&[&str])->Self{Self(v.iter().map(|s|s.to_string()).collect())}}
impl ModelStepGenerator for Scripted{fn generate_step(&mut self,_:&str,_:u32,_:&Cancellation)->Result<String,NativeGenerationError>{Ok(self.0.pop_front().expect("unexpected extra inference"))}}
#[test]
fn cp06d_exact_shapes_and_inert_selection(){
    assert_eq!(TransferIntent::parse(COPY).unwrap().action(),TransferAction::Copy);assert_eq!(TransferIntent::parse(MOVE).unwrap().action(),TransferAction::Move);
    for s in ["Copy the file ../source.txt","Copy the file source.txt to /other","Move the file source.txt","Copy the file source.txt and delete it","How do I copy the file source.txt?","Do not copy a file","Copy the file source.txt without tools","Copy the file source.txt to target.txt"]{assert!(TransferIntent::parse(s).is_none(),"{s}")}
    for bad in [r#"{"selection":null}"#,MOVE_SELECTION,r#"{"selection":"copy_file","path":"evil"}"#,r#"{"selection":"copy_file","approved":true}"#,r#"{"selection":"copy_file","selection":"move_file"}"#]{let f=Fixture::new(COPY,None);assert!(f.propose(&mut Scripted::new(&[bad])).unwrap().is_none());assert!(!f.folder.join(TARGET).exists());assert!(f.effects.get(EFFECT).unwrap().is_none());f.cleanup()}
}
#[tokio::test]
async fn cp06d_confirmed_copy_move_and_verified_fallback(){
    for (original,selection,action,status,mode) in [(COPY,COPY_SELECTION,TransferAction::Copy,DONE,"model_selected_verified_effect_host_rendered"),(MOVE,MOVE_SELECTION,TransferAction::Move,r#"{"status":"unverified"}"#,"host_verified_effect_fallback")]{
        let f=Fixture::new(original,None);let mut model=Scripted::new(&[selection,status]);let p=f.propose(&mut model).unwrap().unwrap();assert!(!f.folder.join(TARGET).exists());let reviewed=f.review(p);assert!(!f.folder.join(TARGET).exists());
        let outcome=reviewed.execute(true,EFFECT,&f,&f.effects,&f.audit,&||f.live()).await.unwrap().unwrap();let answer=outcome.present(&mut model,&Cancellation::default(),&f.audit,&f.effects,&||f.live()).unwrap();f.assert_verified(action,&answer,mode);f.finish(&answer);f.cleanup();
    }
}
#[tokio::test]
async fn cp06d_refusal_stale_source_target_and_revocation_do_not_publish(){
    for case in ["decline","wrong-picker","changed","existing","revoked","replay"]{
        let f=Fixture::new(COPY,None);let mut model=Scripted::new(&[COPY_SELECTION]);let p=f.propose(&mut model).unwrap().unwrap();
        if case=="wrong-picker"{assert!(!p.matches_leaf("wrong.txt"));let (bytes,sha)=source_digest(&f.dir,ORIGINAL).unwrap();fs::write(f.folder.join("wrong.txt"),b"Original fixture content.\n").unwrap();assert!(p.review(Arc::clone(&f.dir),"wrong.txt",TARGET,bytes,&sha,&f.audit).is_err());}
        else{let review=f.review(p);if case=="changed"{fs::write(f.folder.join(ORIGINAL),b"modified").unwrap()}if case=="existing"{fs::write(f.folder.join(TARGET),b"existing data").unwrap()}if case=="revoked"{f.live.store(false,Ordering::SeqCst)}
            if case=="replay"{assert!(f.propose(&mut Scripted::new(&[COPY_SELECTION])).is_err());}
            let r=review.execute(!matches!(case,"decline"|"replay"),EFFECT,&f,&f.effects,&f.audit,&||f.live()).await;if matches!(case,"decline"|"replay"){assert!(r.unwrap().is_none())}else{assert!(r.is_err(),"{case}")}
        }
        let rows=f.audit.events_for(CHAT,TURN).unwrap();assert!(rows.iter().all(|e|e.kind!=K::EvidencePresentation&&e.kind!=K::ToolFeedback));if case=="existing"{assert_eq!(fs::read(f.folder.join(TARGET)).unwrap(),b"existing data")}else{assert!(!f.folder.join(TARGET).exists())}assert!(f.folder.join(ORIGINAL).exists());assert!(!f.effects.get(EFFECT).unwrap().is_some_and(|e|e.state==EffectState::Applied));f.cleanup();
    }
}
#[tokio::test]
async fn cp06d_post_effect_stop_retains_receipt_without_false_completion(){
    let f=Fixture::new(COPY,None);let mut model=Scripted::new(&[COPY_SELECTION]);let p=f.propose(&mut model).unwrap().unwrap();let result=f.review(p).execute(true,EFFECT,&f,&f.effects,&f.audit,&||f.live()).await.unwrap().unwrap();f.live.store(false,Ordering::SeqCst);
    assert!(result.present(&mut model,&Cancellation::default(),&f.audit,&f.effects,&||f.live()).is_err());assert!(f.folder.join(TARGET).exists());assert_eq!(f.effects.get(EFFECT).unwrap().unwrap().state,EffectState::Applied);let rows=f.audit.events_for(CHAT,TURN).unwrap();assert!(rows.iter().any(|e|e.kind==K::ToolFeedback));assert!(rows.iter().all(|e|e.kind!=K::EvidencePresentation&&e.kind!=K::FinalAnswer));f.book.append_host_chat_event(CHAT,&event(2,LiveKind::AssistantCancelled,"")).unwrap();assert_eq!(f.book.generation_terminal_kind(CHAT,TURN).unwrap(),Some(LiveKind::AssistantCancelled));f.cleanup();
}
#[tokio::test]
async fn cp06d_replaced_audit_and_existing_effect_id_are_not_authority(){
    let f=Fixture::new(COPY,None);let mut model=Scripted::new(&[COPY_SELECTION]);let p=f.propose(&mut model).unwrap().unwrap();let other=ModelTurnJournal::open(&f.base.join("replacement.sqlite")).unwrap();let (size,sha)=source_digest(&f.dir,ORIGINAL).unwrap();assert!(p.review(Arc::clone(&f.dir),ORIGINAL,TARGET,size,&sha,&other).is_err());assert!(!f.folder.join(TARGET).exists());drop(other);f.cleanup();
    let f=Fixture::new(COPY,None);let mut model=Scripted::new(&[COPY_SELECTION]);let p=f.propose(&mut model).unwrap().unwrap();let result=f.review(p).execute(true,EFFECT,&f,&f.effects,&f.audit,&||f.live()).await.unwrap().unwrap();
    let other=ModelTurnJournal::open(&f.base.join("replacement.sqlite")).unwrap();assert!(result.present(&mut model,&Cancellation::default(),&other,&f.effects,&||f.live()).is_err());drop(other);
    let g=Fixture::new(COPY,None);let p=g.propose(&mut Scripted::new(&[COPY_SELECTION])).unwrap().unwrap();assert!(g.review(p).execute(true,EFFECT,&g,&f.effects,&g.audit,&||g.live()).await.is_err());assert!(!g.folder.join(TARGET).exists());g.cleanup();f.cleanup();
}
fn hash(p:&Path)->String{let mut h=Sha256::new();let mut f=fs::File::open(p).unwrap();let mut b=[0;65536];loop{let n=f.read(&mut b).unwrap();if n==0{break}h.update(&b[..n]);}format!("{:x}",h.finalize())}
#[tokio::test]
#[ignore="requires pinned local llama-server, genuine GGUF and test-host native review"]
async fn cp06d_genuine_model_copy_move_and_decline(){
    use lu_core::{inference_supervisor::TrustedInferenceSupervisor,model_admission::ModelAdmissionManager,persistent_llama_backend::{PersistentEngineConfig,PersistentLlamaBackend},supervised_tool_generation::SupervisorModelStep};
    use std::{num::NonZeroUsize,time::Duration};
    let exe=PathBuf::from(std::env::var("LU_CP06D_SERVER").unwrap());let gguf=PathBuf::from(std::env::var("LU_CP06D_GGUF").unwrap());let esha=std::env::var("LU_CP06D_SERVER_SHA256").unwrap();let msha=std::env::var("LU_CP06D_GGUF_SHA256").unwrap();assert_eq!(hash(&exe),esha);assert_eq!(hash(&gguf),msha);
    let pin:[u8;32]=(0..esha.len()).step_by(2).map(|i|u8::from_str_radix(&esha[i..i+2],16).unwrap()).collect::<Vec<_>>().try_into().unwrap();let auth="deadbeef".repeat(8);
    let mut manager=ModelAdmissionManager::new();let admitted=manager.admit(gguf.parent().unwrap(),gguf.file_name().unwrap().to_str().unwrap()).unwrap().clone();
    let supervisor=TrustedInferenceSupervisor::new("llama-cpu".into(),8192,vec![("model-1".into(),admitted)],move||PersistentLlamaBackend::new(PersistentEngineConfig{executable:exe.clone(),expected_sha256:pin,threads:NonZeroUsize::new(3).unwrap(),context_tokens:8192,startup_timeout:Duration::from_secs(120),request_timeout:Duration::from_secs(120),api_key:auth.clone(),load_observer:None})).unwrap();
    let sid="cp06d-transfer";supervisor.load("model-1",sid.into(),"cp06d-load").unwrap();assert!(supervisor.status().unwrap().pid.is_some());let base=PathBuf::from(std::env::var("LU_CP06D_EVIDENCE").unwrap());
    for (i,(tag,original,action,approve)) in [("copy",COPY,TransferAction::Copy,true),("move",MOVE,TransferAction::Move,true),("decline",COPY,TransferAction::Copy,false)].into_iter().enumerate(){
        let f=Fixture::new(original,Some(base.join(tag)));let mut model=SupervisorModelStep::new(&supervisor,sid.into(),format!("transfer-{i}")).unwrap();let p=f.propose(&mut model).unwrap().expect("genuine inert model proposal");assert_eq!(p.action(),action);assert!(!f.folder.join(TARGET).exists());
        let r=f.review(p).execute(approve,EFFECT,&f,&f.effects,&f.audit,&||f.live()).await.unwrap();if let Some(r)=r{assert!(approve);let answer=r.present(&mut model,&Cancellation::default(),&f.audit,&f.effects,&||f.live()).unwrap();f.assert_verified(action,&answer,"model_selected_verified_effect_host_rendered");f.finish(&answer);println!("CP06D_VERIFIED {tag} {answer}");}else{assert!(!approve);assert!(f.effects.get(EFFECT).unwrap().is_none());assert!(!f.folder.join(TARGET).exists());assert!(f.folder.join(ORIGINAL).exists());f.finish("File operation declined. No file was changed.");println!("CP06D_DECLINED no effect");}
        for e in f.audit.events_for(CHAT,TURN).unwrap(){println!("CP06D_AUDIT {tag} {:?} {}",e.kind,e.payload);}
    }
    supervisor.unload(sid,"cp06d-unload").unwrap();assert!(supervisor.status().unwrap().pid.is_none());
}
