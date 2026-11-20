//! CP-12B-R3: independent approved model classifications of one count-only
//! snapshot. No chat history, general tool router, or model load.
use crate::{dx_ipc::{ReviewedSummary, Summary}, DesktopState};
use lu_core::{inference_bridge::BridgeError,inference_host::EnginePhase,inference_supervisor::SupervisorStatus,inference_wire::decode_frame,wire::proto};
use std::{sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
const LIMIT:Duration=Duration::from_secs(35);
const TOKENS:u32=64;
const MAX_BYTES:usize=4096;
#[derive(Default)]
pub(crate) struct ReviewControl{active:Mutex<Option<Arc<AtomicBool>>>,closed:AtomicBool}
pub(crate) struct ReviewLease<'a>{control:&'a ReviewControl,stop:Arc<AtomicBool>}
impl ReviewControl{
    pub(crate) fn begin(&self)->Result<ReviewLease<'_>,String>{
        let mut active=self.active.lock().map_err(|_|"Review unavailable")?;
        if self.closed.load(Ordering::Acquire)||active.is_some(){return Err("A review is already running or unavailable.".into())}
        let stop=Arc::new(AtomicBool::new(false));*active=Some(stop.clone());Ok(ReviewLease{control:self,stop})
    }
    pub(crate) fn stop(&self)->bool{
        self.active.lock().ok().and_then(|a|a.as_ref().map(|s|!s.swap(true,Ordering::AcqRel))).unwrap_or(false)
    }
    pub(crate) fn shutdown(&self){self.closed.store(true,Ordering::Release);self.stop();}
}
impl ReviewLease<'_>{
    pub(crate) fn stopped(&self)->bool{self.stop.load(Ordering::Acquire)||self.control.closed.load(Ordering::Acquire)}
    // Serialize publication with cancellation; stop cannot be overtaken by a late invitation.
    pub(crate) fn publish<T>(&self, f:impl FnOnce()->Result<T,String>)->Result<T,String>{
        let active=self.control.active.lock().map_err(|_|"Review unavailable")?;
        if self.stopped()||!active.as_ref().is_some_and(|s|Arc::ptr_eq(s,&self.stop)){return Err("Review stopped.".into())} f()
    }
}
impl Drop for ReviewLease<'_>{fn drop(&mut self){if let Ok(mut a)=self.control.active.lock(){if a.as_ref().is_some_and(|s|Arc::ptr_eq(s,&self.stop)){*a=None;}}}}
struct Done<'a>(&'a AtomicBool);
impl Drop for Done<'_>{fn drop(&mut self){self.0.store(true,Ordering::Release);}}
fn prompt(s:&Summary)->String{
    format!("Classify this troubleshooting summary. Observed failures: {}. Uncertain records: {}. Unavailable records: {}. Partially confirmed: {}. Confirmed deliveries: {}.\nChoose the FIRST applicable rule in this order: F if observed failures > 0; U if uncertain or unavailable > 0; P if partially confirmed > 0; D if confirmed deliveries > 0; N if all counts are zero. Return exactly one letter F, U, P, D or N. No explanation. /no_think",s.observed_failure,s.incomplete,s.unavailable,s.bounded,s.complete)
}
pub(crate) fn ready(owner:&DesktopState)->Result<SupervisorStatus,String>{
    let s=owner.supervisor.status().map_err(|_|"Model unavailable")?;
    if s.phase!=EnginePhase::Ready||s.session_id.is_none()||s.pid.is_none()||s.model_ref.is_none()||s.generation_id.is_some(){return Err("Load a model and wait until Lu is ready before reviewing.".into())} Ok(s)
}
/// The model never sees the peer's proposed finding or peer-generated strings.
pub(crate) fn run(owner:&DesktopState,review:&ReviewLease<'_>,snapshot:&SupervisorStatus,summary:Summary)->Result<ReviewedSummary,String>{
    if !summary.valid(){return Err("Diagnostic summary is invalid.".into())}
    let _fence=owner.model_profile_admission.try_lock().map_err(|_|"Model is changing")?;
    let queue=owner.generation_queue.try_benchmark()?;
    if review.stopped()||queue.yield_requested()||!owner.supervisor.status().is_ok_and(|s|s==*snapshot){return Err("Review stopped or model changed.".into())}
    let pressure=||lu_core::native_resource_pressure::available_ram().is_none_or(|m|m<256);
    if pressure(){return Err("Not enough memory to review now.".into())}
    let supervisor=&owner.supervisor;
    let sid=snapshot.session_id.as_deref().ok_or("Model unavailable")?;
    let gid=crate::next_id("dx-review-");let rid=crate::next_id("req-");let prompt=prompt(&summary);
    supervisor.reserve_generation_with_history(sid,&gid,&rid,&prompt,TOKENS,&[]).map_err(|_|"Review could not start")?;
    let deadline=Instant::now()+LIMIT;let done=AtomicBool::new(false);
    let mut text=String::new();let mut pieces=0u32;let mut terminal=false;
    let (outcome,stopped)=std::thread::scope(|scope|{
        let monitor=scope.spawn(||loop{
            if done.load(Ordering::Acquire){break false}
            if review.stopped()||pressure()||Instant::now()>=deadline{
                let _=supervisor.cancel(sid,&gid,&crate::next_id("req-"));break true
            }
            std::thread::sleep(Duration::from_millis(25));
        });
        let guard=Done(&done);
        let outcome=supervisor.run_reserved_with_history(sid,&gid,&rid,prompt,TOKENS,vec![],|wire|{
            let frame=decode_frame(&wire)?;
            if frame.request_id!=rid{return Err(BridgeError::Consumer)}
            let Some(proto::inference_frame::Body::Event(event))=frame.body else{return Err(BridgeError::Consumer)};
            if event.session_id!=sid||event.generation_id!=gid||terminal{return Err(BridgeError::Consumer)}
            match event.kind{
                Some(proto::inference_event::Kind::TextChunk(chunk))=>{
                    if chunk.text.is_empty()||chunk.chunk_index!=pieces||text.len()+chunk.text.len()>MAX_BYTES{return Err(BridgeError::Consumer)}
                    text.push_str(&chunk.text);pieces+=1;
                },
                Some(proto::inference_event::Kind::Finished(end))=>{
                    if end.cancelled||end.generated_bytes!=text.len() as u64||end.output_pieces!=pieces{return Err(BridgeError::Consumer)}terminal=true;
                },
                Some(proto::inference_event::Kind::State(_))|Some(proto::inference_event::Kind::Failure(_))=>(),
                _=>return Err(BridgeError::Consumer),
            } Ok(())
        });
        drop(guard);(outcome,monitor.join().unwrap_or(true))
    });
    if stopped||review.stopped()||queue.yield_requested()||Instant::now()>=deadline{return Err("Review stopped; no finding was accepted.".into())}
    if outcome.is_err()||!terminal||!owner.supervisor.status().is_ok_and(|s|s==*snapshot){return Err("The model review did not finish.".into())}
    ReviewedSummary::checked(summary,&text).ok_or_else(||"The model's finding did not match the evidence. No unverified finding was accepted.".into())
}
#[cfg(test)]
mod tests{
    use super::*;use crate::dx_ipc::Finding;
    #[test]fn cp12br3_finite_evidence_rules(){
        let mut s=Summary{complete:0,observed_failure:0,bounded:0,incomplete:0,unavailable:0};
        for (field,letter,kind) in [(0,"N",Finding::Empty),(1,"D",Finding::Delivered),(2,"P",Finding::Partial),(3,"U",Finding::Uncertain),(4,"F",Finding::Failure)]{
            match field{1=>s.complete=1,2=>s.bounded=1,3=>s.unavailable=1,4=>s.observed_failure=1,_=>()}
            assert_eq!(ReviewedSummary::checked(s.clone(),letter).unwrap().finding,kind);
            assert!(ReviewedSummary::checked(s.clone(),"F or D").is_none());
            for invalid in ["N","D","P","U","F"].into_iter().filter(|x|*x!=letter){assert!(ReviewedSummary::checked(s.clone(),invalid).is_none());}
        }
        s.complete=65;assert!(Finding::expected(&s).is_none());
    }
    #[test]fn cp12br3_cancellation_and_admission_are_fenced(){
        let c=ReviewControl::default();let r=c.begin().unwrap();assert!(c.begin().is_err());assert!(r.publish(||Ok(42)).is_ok());assert!(c.stop());assert!(r.publish::<()>(||panic!("late publication")).is_err());drop(r);let r=c.begin().unwrap();c.shutdown();assert!(r.stopped());drop(r);assert!(c.begin().is_err());
    }
}
