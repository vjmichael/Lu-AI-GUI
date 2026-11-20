//! CP-11A-R2: fixed synthetic text-only benchmark of the current admitted model.
//! No tools, real conversation context, model loading, downloads or replay.
use crate::{active_checks::{CheckBody,CheckKind,CheckReason,CheckReport,CheckResult,CheckState,MODEL_CHECKS,finish},generation_queue::GenerationQueue,engine_memory};
use lu_core::{inference_bridge::BridgeError,inference_host::EnginePhase,inference_supervisor::{SupervisorStatus,TrustedInferenceSupervisor},inference_wire::decode_frame,persistent_llama_backend::PersistentLlamaBackend,wire::proto};
use serde::{Deserialize,Serialize};
use std::{sync::{Arc,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
const MAX_TOKENS:u32=128;
const CASE_LIMIT:Duration=Duration::from_secs(35);
const MAX_BYTES:usize=16*1024;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct InferenceMetrics {
    pub first_output_micros:Option<u64>,pub output_bytes:u32,pub output_pieces:u32,
    pub sampled_peak_ram_bytes:Option<u64>,pub ram_samples:u32,
    pub engine_cpu_micros:Option<u64>,
}
impl InferenceMetrics {
    pub fn valid(&self,elapsed:u64)->bool {
        self.output_bytes as usize<=MAX_BYTES && self.output_pieces<=self.output_bytes &&
        (self.output_bytes==0)==self.first_output_micros.is_none() &&
        self.first_output_micros.is_none_or(|n|n<=elapsed) &&
        (self.output_bytes==0)==(self.output_pieces==0) &&
        (self.ram_samples==0)==self.sampled_peak_ram_bytes.is_none() && self.ram_samples<=4096 &&
        self.sampled_peak_ram_bytes.is_none_or(|n|n>0)
    }
}
// Linux kernel per-process CPU clock includes all engine threads. Do not
// approximate CPU time from wall time or sample undocumented counters.
#[cfg(all(target_os="linux",target_pointer_width="64"))]
fn cpu_micros(pid:u32)->Option<u64>{
    use std::os::raw::{c_int,c_long};
    #[repr(C)] struct Timespec { sec:c_long,nsec:c_long }
    extern "C" {fn clock_getcpuclockid(pid:c_int,clock_id:*mut c_int)->c_int;fn clock_gettime(clock_id:c_int,value:*mut Timespec)->c_int;}
    let mut id=0;let mut value=Timespec{sec:0,nsec:0};
    if unsafe{clock_getcpuclockid(i32::try_from(pid).ok()?,&mut id)}!=0||unsafe{clock_gettime(id,&mut value)}!=0{return None}
    if !(0..1_000_000_000).contains(&value.nsec){return None}
    u64::try_from(value.sec).ok()?.checked_mul(1_000_000)?.checked_add(value.nsec as u64/1000)
}
#[cfg(not(all(target_os="linux",target_pointer_width="64")))]
fn cpu_micros(_:u32)->Option<u64>{None}
fn micros(started:Instant)->u64{started.elapsed().as_micros().min(u64::MAX as u128) as u64}
struct Case{prompt:&'static str,expected:String,history:Vec<proto::InferenceChatTurn>}
fn sample(kind:CheckKind)->Case {
    match kind {
        CheckKind::ModelExtract=>Case{prompt:"Sample inventory: Cedar has code LIMA-471. Birch has code VELA-208. What is Birch's code? Reply with the code only. /no_think",expected:"VELA-208".into(),history:vec![]},
        CheckKind::ModelArithmetic=>Case{prompt:"A sample shop has 17 boxes with 8 items each. It sells 19 items. How many remain? Reply only with the integer. /no_think",expected:(17*8-19).to_string(),history:vec![]},
        CheckKind::ModelHistory=>Case{prompt:"What is the current sample delivery city? Reply only with the city. /no_think",expected:"Oslo".into(),history:vec![proto::InferenceChatTurn{role:"user".into(),text:"The sample delivery city is Rome.".into()},proto::InferenceChatTurn{role:"assistant".into(),text:"Understood.".into()},proto::InferenceChatTurn{role:"user".into(),text:"Correction: the delivery city is Oslo, not Rome.".into()},proto::InferenceChatTurn{role:"assistant".into(),text:"I have noted the correction.".into()}]},
        _=>unreachable!("fixed benchmark cases only"),
    }
}
fn score(answer:&str,expected:&str)->bool { answer.trim()==expected }
fn same_model(a:&SupervisorStatus,b:&SupervisorStatus)->bool {
    a.session_id.is_some()&&a.session_id==b.session_id&&a.model_ref.is_some()&&a.model_ref==b.model_ref&&a.pid.is_some()&&a.pid==b.pid
}
fn attention(body:CheckBody)->CheckReport{finish(body,CheckState::Attention,Some(CheckReason::ModelUnavailable))}
fn stopped(body:CheckBody,reason:CheckReason)->CheckReport{finish(body,CheckState::Stopped,Some(reason))}
#[derive(Default)]
struct Samples{peak:Option<u64>,count:u32}
impl Samples{
    fn read(&mut self,pid:u32){if let Some(n)=engine_memory::sample_host_engine(pid).working_set_ram_bytes.filter(|n|*n>0){self.peak=Some(self.peak.unwrap_or(0).max(n));self.count+=1;}}
}
// Watch the exact reserved generation even when the server emits no chunks.
// Drop signals the monitor before scoped joining, including unwinding.
struct Done<'a>(&'a AtomicBool);
impl Drop for Done<'_>{fn drop(&mut self){self.0.store(true,Ordering::Release);}}
fn run_case(supervisor:&TrustedInferenceSupervisor<PersistentLlamaBackend>,snapshot:&SupervisorStatus,kind:CheckKind,
    stop:&(dyn Fn()->Option<CheckReason>+Sync))->Result<CheckResult,Option<CheckReason>>{
    let sid=snapshot.session_id.as_deref().ok_or(None)?;let pid=snapshot.pid.ok_or(None)?;
    if !supervisor.status().is_ok_and(|s|same_model(&s,snapshot)&&s.phase==EnginePhase::Ready){return Err(None)}
    let Case{prompt,expected,history}=sample(kind);
    let gid=crate::next_id("bench-");let rid=crate::next_id("req-");
    supervisor.reserve_generation_with_history(sid,&gid,&rid,prompt,MAX_TOKENS,&history).map_err(|_|None)?;
    let cpu_before=cpu_micros(pid);let started=Instant::now();let done=AtomicBool::new(false);
    let mut answer=String::new();let mut first=None;let mut pieces=0u32;let mut terminal=false;
    let (outcome,measurements,observed_stop)=std::thread::scope(|scope|{
        let monitor=scope.spawn(||{
            let mut samples=Samples::default();let mut last_sample=None;let mut stopped=None;
            loop {
                if let Some(reason)=stop().or_else(||(started.elapsed()>=CASE_LIMIT).then_some(CheckReason::TimeLimit)){
                    stopped=Some(reason);let _=supervisor.cancel(sid,&gid,&crate::next_id("req-"));break
                }
                if last_sample.is_none_or(|last:Instant|last.elapsed()>=Duration::from_millis(100))||done.load(Ordering::Acquire){
                    if supervisor.status().is_ok_and(|s|same_model(&s,snapshot)){samples.read(pid);}last_sample=Some(Instant::now());
                }
                if done.load(Ordering::Acquire){break}
                std::thread::sleep(Duration::from_millis(20));
            }(samples,stopped)
        });
        let guard=Done(&done);
        let outcome=supervisor.run_reserved_with_history(sid,&gid,&rid,prompt.into(),MAX_TOKENS,history,|wire|{
            let frame=decode_frame(&wire)?;
            if frame.request_id!=rid {return Err(BridgeError::Consumer)}
            let Some(proto::inference_frame::Body::Event(event))=frame.body else{return Err(BridgeError::Consumer)};
            if event.session_id!=sid||event.generation_id!=gid||terminal{return Err(BridgeError::Consumer)}
            match event.kind {
                Some(proto::inference_event::Kind::TextChunk(chunk))=>{
                    if chunk.text.is_empty()||chunk.chunk_index!=pieces||answer.len()+chunk.text.len()>MAX_BYTES{return Err(BridgeError::Consumer)}
                    first.get_or_insert_with(||micros(started));answer.push_str(&chunk.text);pieces+=1;
                },
                Some(proto::inference_event::Kind::Finished(end))=>{
                    if end.cancelled||end.generated_bytes!=answer.len() as u64||end.output_pieces!=pieces{return Err(BridgeError::Consumer)}terminal=true;
                },
                Some(proto::inference_event::Kind::State(_))|Some(proto::inference_event::Kind::Failure(_))=>(),
                _=>return Err(BridgeError::Consumer),
            }Ok(())
        });
        let elapsed=micros(started);let cpu=cpu_before.zip(cpu_micros(pid)).and_then(|(before,after)|after.checked_sub(before));drop(guard);
        let (samples,stopped)=monitor.join().expect("benchmark monitor");
        ((outcome,elapsed,cpu),samples,stopped)
    });
    if let Some(reason)=observed_stop.or_else(stop){return Err(Some(reason))}
    if outcome.0.is_err()||!terminal||!supervisor.status().is_ok_and(|s|same_model(&s,snapshot)&&s.phase==EnginePhase::Ready){return Err(None)}
    let passed=score(&answer,&expected);
    let inference=InferenceMetrics{first_output_micros:first,output_bytes:answer.len() as u32,output_pieces:pieces,sampled_peak_ram_bytes:measurements.peak,ram_samples:measurements.count,engine_cpu_micros:outcome.2};
    if !inference.valid(outcome.1){return Err(None)}
    Ok(CheckResult{kind,passed,verified_operations:u32::from(passed),elapsed_micros:outcome.1,inference:Some(inference)})
}
pub(crate) fn run(supervisor:&TrustedInferenceSupervisor<PersistentLlamaBackend>,queue:&Arc<GenerationQueue>,snapshot:&SupervisorStatus,
    stop:&(dyn Fn()->Option<CheckReason>+Sync),publish:&mut dyn FnMut(&CheckBody)->Result<(),String>)->Result<CheckReport,String>{
    let mut body=CheckBody{model_suite:true,..CheckBody::default()};
    if let Some(reason)=stop(){return Ok(stopped(body,reason))}
    let lease=match queue.try_benchmark(){Ok(l)=>l,Err(_)=>return Ok(stopped(body,CheckReason::ChatBusy))};
    let stop=||stop().or_else(||lease.yield_requested().then_some(CheckReason::ChatBusy));
    for kind in MODEL_CHECKS {
        if let Some(reason)=stop(){return Ok(stopped(body,reason))}
        match run_case(supervisor,snapshot,kind,&stop){
            Ok(result)=>{body.results.push(result);publish(&body)?;},
            Err(Some(reason))=>return Ok(stopped(body,reason)),
            Err(None)=>return Ok(attention(body)),
        }
    }
    if let Some(reason)=stop(){return Ok(stopped(body,reason))}
    if body.results.iter().all(|r|r.passed){Ok(finish(body,CheckState::Complete,None))}
    else{Ok(finish(body,CheckState::Attention,Some(CheckReason::CheckFailed)))}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn strict_independent_scoring(){for kind in MODEL_CHECKS{let c=sample(kind);assert!(score(&c.expected,&c.expected));assert!(!score(&format!("PASS {}",c.expected),&c.expected));assert!(!score("Rome",&c.expected));}assert_eq!(sample(CheckKind::ModelArithmetic).expected,"117");}
    #[test]fn metric_validation_and_failure_are_distinct(){
        let mut m=InferenceMetrics{first_output_micros:Some(22),output_bytes:4,output_pieces:2,sampled_peak_ram_bytes:Some(1000),ram_samples:2,engine_cpu_micros:Some(60)};
        assert!(m.valid(100));m.output_pieces=5;assert!(!m.valid(100));m.output_pieces=2;m.first_output_micros=Some(101);assert!(!m.valid(100));m.first_output_micros=Some(22);m.ram_samples=0;assert!(!m.valid(100));
        m.ram_samples=2;let mut body=CheckBody{model_suite:true,..CheckBody::default()};
        body.results=MODEL_CHECKS.iter().map(|k|CheckResult{kind:*k,passed:false,verified_operations:0,elapsed_micros:100,inference:Some(m.clone())}).collect();
        assert!(finish(body.clone(),CheckState::Attention,Some(CheckReason::CheckFailed)).valid());assert!(!finish(body.clone(),CheckState::Complete,None).valid());
        body.model_suite=false;assert!(!finish(body,CheckState::Running,None).valid());
    }
    #[test]fn lease_yields_to_real_queue(){let q=Arc::new(GenerationQueue::default());let l=q.try_benchmark().unwrap();assert!(q.has_jobs());assert!(q.try_benchmark().is_err());let (ticket,_)=q.enqueue("s-one","chat-one","g-one").unwrap();assert!(l.yield_requested());drop(l);ticket.wait_for_slot().unwrap();drop(ticket);assert!(q.try_benchmark().is_ok());}
}
