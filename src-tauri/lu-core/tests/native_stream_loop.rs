//! Streaming generation loop regression with Lu's real Protobuf inference
//! supervisor and fake deterministic backend (NOT a physical GGUF acceptance).
use std::{
    fs,
    path::PathBuf,
    sync::{Arc,atomic::{AtomicU64,AtomicUsize,Ordering}},
    time::{SystemTime, UNIX_EPOCH},
};

use lu_core::{
    inference_host::{Cancellation,EngineError,GenerationRequest,InferenceBackend,TokenChunk},
    inference_supervisor::TrustedInferenceSupervisor,
    model_admission::{AdmittedModel,ModelAdmissionManager},
    supervised_tool_generation::{
        ModelStepGenerator,NativeGenerationError,SupervisorModelStep,
    },
};
static IDS:AtomicU64=AtomicU64::new(0);
fn model_dir()->PathBuf {
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let i=IDS.fetch_add(1,Ordering::Relaxed);
    let root=std::env::temp_dir().join(format!(
        "lu-stream-cycle-{}-{stamp}-{i}",std::process::id()));
    fs::create_dir(&root).unwrap();
    let mut gguf=Vec::new();
    gguf.extend_from_slice(b"GGUF");
    gguf.extend_from_slice(&3u32.to_le_bytes());
    gguf.extend_from_slice(&1u64.to_le_bytes());
    gguf.extend_from_slice(&1u64.to_le_bytes());
    fs::write(root.join("tiny.gguf"),gguf).unwrap();
    root
}
struct RepeatingFakeBackend {
    emitted:Arc<AtomicUsize>,
    repeat:bool,
}
impl InferenceBackend for RepeatingFakeBackend {
    fn load(&mut self,_:&AdmittedModel)->Result<(),String>{Ok(())}
    fn unload(&mut self){}
    fn generate(
        &mut self,_:&GenerationRequest,cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>,
    )->Result<(),String>{
        for i in 0..800 {
            if cancel.is_cancelled(){return Err("cancelled".into())}
            let text=if self.repeat{
                "The same sentence is repeating without useful output. ".into()
            } else {
                format!("Chunk {i}: a different observation and a unique value {}.\n",i*i)
            };
            self.emitted.fetch_add(1,Ordering::SeqCst);
            emit(TokenChunk{text,index:i}).map_err(|_|"host stopped stream".to_owned())?;
            if !self.repeat && i>=29 { break }
        }
        Ok(())
    }
}
#[test]
fn actual_protobuf_supervisor_cuts_repeated_stream_before_exhaustion(){
    let root=model_dir();
    let mut manager=ModelAdmissionManager::new();
    let admitted=manager.admit(&root,"tiny.gguf").unwrap().clone();
    let emitted=Arc::new(AtomicUsize::new(0));
    let observed=Arc::clone(&emitted);
    let supervisor=TrustedInferenceSupervisor::new(
        "repeat-backend".into(),512,vec![("demo".into(),admitted)],
        move || Ok(RepeatingFakeBackend{emitted:Arc::clone(&observed),repeat:true}),
    ).unwrap();
    supervisor.load("demo","s-loop".into(),"load-loop").unwrap();
    let mut model=SupervisorModelStep::new(
        &supervisor,"s-loop".into(),"turn-loop".into()).unwrap();
    assert_eq!(model.generate_step("Describe the image",512,&Cancellation::default()),
        Err(NativeGenerationError::TokenLoop));
    assert!(emitted.load(Ordering::SeqCst) < 150,
        "the real streaming adapter must interrupt before hundreds of repeated chunks");
    drop(model);drop(supervisor);drop(manager);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn actual_protobuf_supervisor_allows_diverse_stream(){
    let root=model_dir();
    let mut manager=ModelAdmissionManager::new();
    let admitted=manager.admit(&root,"tiny.gguf").unwrap().clone();
    let emitted=Arc::new(AtomicUsize::new(0));
    let observed=Arc::clone(&emitted);
    let supervisor=TrustedInferenceSupervisor::new(
        "diverse-backend".into(),512,vec![("demo".into(),admitted)],
        move || Ok(RepeatingFakeBackend{emitted:Arc::clone(&observed),repeat:false}),
    ).unwrap();
    supervisor.load("demo","s-diverse".into(),"load-diverse").unwrap();
    let mut model=SupervisorModelStep::new(
        &supervisor,"s-diverse".into(),"turn-diverse".into()).unwrap();
    let result=model.generate_step("Provide diverse information",512,&Cancellation::default())
        .expect("diverse token stream should not be rejected");
    assert!(result.contains("Chunk 29"));
    assert_eq!(emitted.load(Ordering::SeqCst),30);
    drop(model);drop(supervisor);drop(manager);
    fs::remove_dir_all(root).unwrap();
}
