//! PERF-01 P0: low-overhead host-measured chat routing/latency receipts.
//! No model prompt, raw paths, private source text, environment dump, or
//! hypothetical GPU/prefill/token-rate measurements are persisted here.
use serde::Serialize;
use crate::causal_diagnostic::{CausalProgress,Checkpoint};
use lu_core::diagnostic_localization;
use crate::diagnostic_archive::{DiagnosticArchive,wire_trace};
use std::{
    collections::VecDeque,
    sync::{Arc,Mutex},
    time::Instant,
};

const MAX_RECEIPTS:usize=128;
const MAX_CHAT_RECEIPTS:usize=24;

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct PerfReceipt {
    pub chat_id:String,
    pub generation_id:String,
    pub route:String,
    pub scheduler_class:Option<String>,
    pub reason_code:String,
    pub terminal_state:String,
    pub evidence_scope:String,
    pub source_head:Option<String>,
    pub operating_system:String,
    pub architecture:String,
    pub model_loaded_at_entry:bool,
    pub model_started_by_this_route:Option<bool>,
    // Original llama backend engine_spawned callback events. Not GPU telemetry.
    pub observed_engine_starts_during_turn:Option<u64>,
    pub observed_engine_starts_since_host_start:Option<u64>,
    pub model_ref:Option<String>,
    pub backend_family:Option<String>,
    pub context_limit_tokens:Option<u32>,
    // Actual scheduler publication, not a claim of SQLite cross-store atomicity.
    pub queue_published:Option<bool>,
    pub catalog_reconciliation_required:bool,
    pub admitted_image_count:Option<u32>,
    pub image_stage:Option<&'static str>,
    pub image_failure_code:Option<&'static str>,
    pub image_delivery:Option<lu_core::multimodal_http::ImageDeliveryReceipt>,
    // Private transient correlation; the exposed finding has only fixed
    // stage/region/category vocabulary and no trace reference.
    #[serde(skip)]
    cp01_trace:Option<CausalProgress>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub cp01_finding:Option<diagnostic_localization::DiagnosticFinding>,
    pub queue_wait_ms:Option<u64>,
    pub context_assembly_ms:Option<u64>,
    pub model_dispatch_to_first_chunk_ms:Option<u64>,
    // Wall-clock measurements from original Send to genuine model chunk and
    // first successfully enqueued Tauri UI event. Not a painted-frame claim.
    pub turn_to_first_chunk_ms:Option<u64>,
    pub turn_to_first_webview_forward_ms:Option<u64>,
    pub overall_latency_ms:u64,
    pub original_source_bytes_scanned:Option<u64>,
    pub original_entries_scanned:Option<u32>,
    pub source_result_complete:Option<bool>,
    pub streamed_utf8_bytes:u64,
    // NOT measured: prefill tokens/s, decode tokens/s, cache hit-rate,
    // actual GPU load, VRAM resident allocation, instantaneous CPU load.
    pub prefill_tokens_per_second:Option<f64>,
    pub decode_tokens_per_second:Option<f64>,
    pub gpu_used_percent:Option<f64>,
    pub cache_hit:Option<bool>,
    /// Native host admitted the current turn for scoped reuse; a true value
    /// is NOT proof that llama.cpp actually reused even one KV token.
    pub cache_scope_admitted:Option<bool>,
}
fn elapsed_ms(start:Instant)->u64 {
    start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
fn source_head()->Option<String> {
    // The build must supply its ORIGINAL exact source SHA to claim identity.
    // A dev build without that variable reports None, never a guessed SHA.
    let value=option_env!("LU_PERF01_SOURCE_HEAD")?;
    (value.len()==40&&value.bytes().all(|b|b.is_ascii_hexdigit()))
        .then(||value.to_ascii_lowercase())
}
fn base(chat_id:&str,generation_id:&str,route:&str,reason_code:&str,
        model_loaded_at_entry:bool)->PerfReceipt{
    PerfReceipt{
        chat_id:chat_id.into(),generation_id:generation_id.into(),
        route:route.into(),scheduler_class:None,
        reason_code:reason_code.into(),
        terminal_state:"INCOMPLETE".into(),
        evidence_scope:"HOST_OBSERVED_ONLY".into(),
        source_head:source_head(),
        operating_system:std::env::consts::OS.into(),
        architecture:std::env::consts::ARCH.into(),
        model_loaded_at_entry,model_started_by_this_route:None,
        observed_engine_starts_during_turn:None,
        observed_engine_starts_since_host_start:None,
        model_ref:None,backend_family:None,context_limit_tokens:None,
        queue_published:None,admitted_image_count:None,catalog_reconciliation_required:false,
        image_stage:None,image_failure_code:None,image_delivery:None,
        cp01_trace:None,cp01_finding:None,
        queue_wait_ms:None,context_assembly_ms:None,
        model_dispatch_to_first_chunk_ms:None,
        turn_to_first_chunk_ms:None,turn_to_first_webview_forward_ms:None,
        overall_latency_ms:0,
        original_source_bytes_scanned:None,original_entries_scanned:None,
        source_result_complete:None,streamed_utf8_bytes:0,
        prefill_tokens_per_second:None,decode_tokens_per_second:None,
        gpu_used_percent:None,cache_hit:None,
        cache_scope_admitted:None,
    }
}
#[derive(Clone,Debug,Serialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub(crate) struct DeliveryWitness {
    pub chat_id:String,pub generation_id:String,pub diagnostic_ref:String,
    pub assistant_seq:i64,
    #[serde(skip)] pub reader:bool,
}
struct NativeDelivery { witness:DeliveryWitness, trace:CausalProgress }
#[derive(Default)]
pub(crate) struct PerfLedger {
    recent:Mutex<VecDeque<PerfReceipt>>,
    native_delivery:Mutex<VecDeque<NativeDelivery>>,
    archive:Option<Arc<DiagnosticArchive>>,
}
impl PerfLedger{
    pub fn with_archive(archive:Option<Arc<DiagnosticArchive>>)->Self{
        Self{archive,..Self::default()}
    }
    pub fn archive(&self)->Option<Arc<DiagnosticArchive>>{self.archive.clone()}
    pub fn record(&self,mut row:PerfReceipt)->Result<(),String>{
        row.refresh_cp01_finding();
        // Never print raw source content; performance is a diagnostic,
        // not an authority record or proof of verified task completion.
        let mut q=self.recent.lock().map_err(|_|"Perf telemetry lock unavailable")?;
        q.push_back(row);
        while q.len()>MAX_RECEIPTS{q.pop_front();}
        Ok(())
    }
    pub fn exact_turn(&self,chat_id:&str,generation_id:&str)->Result<Option<PerfReceipt>,String>{
        let q=self.recent.lock().map_err(|_|"Perf telemetry lock unavailable")?;
        Ok(q.iter().rev().find(|v|v.chat_id==chat_id&&v.generation_id==generation_id).cloned())
    }
    pub fn exact_causal_turn(&self,chat_id:&str,generation_id:&str,trace_ref:&str)
        ->Option<PerfReceipt>{
        let q=self.recent.lock().ok()?;
        q.iter().rev().find(|row|row.chat_id==chat_id&&row.generation_id==generation_id
            &&row.cp01_trace.as_ref().is_some_and(|t|t.trace_ref()==trace_ref)).cloned()
    }
    pub fn offer_native_delivery(&self,chat:&str,gid:&str,seq:i64,reader:bool,trace:CausalProgress){
        if seq<=0||!trace.saved(){return}
        if let Ok(mut q)=self.native_delivery.try_lock(){
            let witness=DeliveryWitness{chat_id:chat.into(),generation_id:gid.into(),diagnostic_ref:trace.trace_ref().into(),assistant_seq:seq,reader};
            if q.iter().any(|r|r.witness.diagnostic_ref==witness.diagnostic_ref){return}
            q.push_back(NativeDelivery{witness,trace});
            while q.len()>64{q.pop_front();}
        }
    }
    pub fn text_delivery(&self,chats:&crate::chat_store::ChatStore,chat:&str,reference:&str)->Option<DeliveryWitness>{
        if let Some(w)=self.native_delivery.try_lock().ok().and_then(|q|q.iter().find(|r|r.witness.chat_id==chat&&r.witness.diagnostic_ref==reference).map(|r|r.witness.clone())){return Some(w)}
        let gid=self.recent.try_lock().ok()?.iter().find(|r|r.chat_id==chat&&r.terminal_state=="COMPLETED"&&r.cp01_trace.as_ref().is_some_and(|t|t.trace_ref()==reference&&t.saved()))?.generation_id.clone();
        let seq=chats.require(chat).ok()?.diagnostic_completed_assistant_seq(chat,&gid,None,false).ok().flatten()?;
        Some(DeliveryWitness{chat_id:chat.into(),generation_id:gid,diagnostic_ref:reference.into(),assistant_seq:seq,reader:false})
    }
    pub fn observe_text_reply(&self,witness:&DeliveryWitness,delivered:bool)->bool{
        if let Ok(mut q)=self.native_delivery.try_lock(){
            if let Some(row)=q.iter_mut().find(|r|&r.witness==witness){
                return record_text_ack(&mut row.trace,self.archive.as_deref(),delivered);
            }
        }
        let Ok(mut q)=self.recent.try_lock() else{return false};
        let Some(row)=q.iter_mut().find(|r|r.chat_id==witness.chat_id&&r.generation_id==witness.generation_id&&r.terminal_state=="COMPLETED"&&r.cp01_trace.as_ref().is_some_and(|t|t.trace_ref()==witness.diagnostic_ref))else{return false};
        let Some(trace)=row.cp01_trace.as_mut()else{return false};
        let ok=record_text_ack(trace,self.archive.as_deref(),delivered);
        row.refresh_cp01_finding();ok
    }
    // Reports are observational only. Never let a stale attempt mutate a
    // newer row, and never block inference/cancellation on telemetry.
    pub fn observe_cp01_reply(&self,chat_id:&str,generation_id:&str,trace_ref:&str,delivered:bool)->bool{
        let Ok(mut q)=self.recent.try_lock() else{return false};
        let Some(row)=q.iter_mut().rev().find(|row|row.chat_id==chat_id&&
            row.generation_id==generation_id&&row.terminal_state=="COMPLETED"&&
            row.cp01_trace.as_ref().is_some_and(|t|t.trace_ref()==trace_ref))
            else{return false};
        let Some(trace)=row.cp01_trace.as_mut() else{return false};
        let mut accepted=trace.observe(Checkpoint::ReplyObserved);
        if delivered&&accepted{
            accepted=trace.observe(Checkpoint::StreamClosed)&&trace.observe(Checkpoint::AssistantRendered);
        }
        if let Some(archive)=self.archive.as_ref(){
            if !archive.persist(trace){trace.mark_lost();}
        }
        row.refresh_cp01_finding();
        accepted
    }
    pub fn recent(&self,chat_id:&str,limit:usize)
        ->Result<Vec<PerfReceipt>,String>{
        if limit==0||limit>MAX_CHAT_RECEIPTS{
            return Err("Invalid bounded performance receipt limit".into());
        }
        let q=self.recent.lock().map_err(|_|"Perf telemetry lock unavailable")?;
        Ok(q.iter().rev().filter(|v|v.chat_id==chat_id)
            .take(limit).cloned().collect())
    }
}
fn record_text_ack(trace:&mut CausalProgress,archive:Option<&DiagnosticArchive>,delivered:bool)->bool{
    if !trace.saved() || (delivered&&!trace.reply_seen()){return false}
    let accepted=if delivered{trace.observe(Checkpoint::StreamClosed)&&trace.observe(Checkpoint::AssistantRendered)}else{trace.observe(Checkpoint::ReplyObserved)};
    if let Some(archive)=archive{if !archive.persist(trace){trace.mark_lost();return false}}
    accepted
}
pub(crate) fn cpu_receipt(
    chat_id:&str,generation_id:&str,reason_code:&str,
    start:Instant,model_loaded_at_entry:bool,
    source_bytes:Option<usize>,entries:Option<usize>,complete:Option<bool>,
    terminal_state:&str
)->PerfReceipt{
    let mut result=base(chat_id,generation_id,"CPU_DETERMINISTIC",
        reason_code,model_loaded_at_entry);
    result.terminal_state=terminal_state.into();
    result.model_started_by_this_route=Some(false);
    result.overall_latency_ms=elapsed_ms(start);
    result.original_source_bytes_scanned=source_bytes.map(|v|v as u64);
    result.original_entries_scanned=entries.map(|v|v.min(u32::MAX as usize) as u32);
    result.source_result_complete=complete;
    result
}
impl PerfReceipt {
    fn refresh_cp01_finding(&mut self){
        self.cp01_finding=self.cp01_trace.as_ref().and_then(|source|{
            let trace=wire_trace(source);
            diagnostic_localization::localize_transactional_turn(&trace).ok()
        });
    }
    pub(crate) fn with_engine_start_events(mut self, before:u64,after:u64)->Self{
        self.observed_engine_starts_during_turn=
            Some(after.saturating_sub(before));
        self.observed_engine_starts_since_host_start=Some(after);
        self
    }
}
pub(crate) struct ModelTrace{
    started:Instant,
    queue_entered:Instant,
    context_entered:Option<Instant>,
    dispatch_entered:Option<Instant>,
    row:PerfReceipt,
    archive:Option<Arc<DiagnosticArchive>>,
}
impl ModelTrace{
    pub fn new(chat_id:&str,generation_id:&str,started:Instant,model_loaded:bool)
        ->Self{
        Self{
            started,queue_entered:Instant::now(),context_entered:None,
            dispatch_entered:None,
            row:base(chat_id,generation_id,"MODEL_INFERENCE",
                "MODEL_REASONING_REQUIRED",model_loaded),
            archive:None,
        }
    }
    pub fn with_cp01_trace(mut self,trace_ref:String,started:Instant,
        archive:Option<Arc<DiagnosticArchive>>)->Self{
        self.row.cp01_trace=Some(CausalProgress::new(trace_ref,started));
        self.archive=archive;
        self
    }
    /// Transfer the one invocation's causal prefix, not a second trace or grant.
    pub fn take_causal_progress(&mut self)->Option<CausalProgress>{self.row.cp01_trace.take()}
    pub fn persist_cp01(&mut self){
        if let (Some(trace),Some(archive))=(self.row.cp01_trace.as_mut(),self.archive.as_ref()){
            if !archive.persist(trace){trace.mark_lost();}
        }
    }
    pub fn causal_checkpoint(&mut self,stage:Checkpoint){
        if let Some(trace)=self.row.cp01_trace.as_mut(){trace.observe(stage);}
        // SessionReady/UserSaved occur inside the attachment admission gate.
        // Flush directly after releasing the gate.
        if !matches!(stage,Checkpoint::SessionReady|Checkpoint::UserSaved){self.persist_cp01();}
    }
    pub fn causal_cancelled(&mut self){
        if let Some(trace)=self.row.cp01_trace.as_mut(){trace.observed_cancel();}
        self.persist_cp01();
    }
    pub fn causal_saved_response(&mut self,chats:&crate::chat_store::ChatStore,chat:&str,gid:&str,answer:&str){
        if self.row.cp01_trace.is_none(){return}
        if chats.require(chat).ok().and_then(|book|book.diagnostic_completed_assistant_seq(chat,gid,Some(answer),false).ok().flatten()).is_some(){self.causal_checkpoint(Checkpoint::AssistantSaved)}
        else if let Some(trace)=self.row.cp01_trace.as_mut(){trace.mark_lost();self.persist_cp01();}
    }
    pub fn causal_model_failed(&mut self){
        if let Some(trace)=self.row.cp01_trace.as_mut(){trace.model_failed();}
        self.persist_cp01();
    }
    pub fn causal_refused(&mut self){
        if let Some(trace)=self.row.cp01_trace.as_mut(){trace.refused();}
    }
    pub fn with_background_origin(mut self,background:bool)->Self{
        let label=if background {
            "BACKGROUND_TERMINAL"
        } else {
            "FOREGROUND_HUMAN"
        };
        self.row.scheduler_class=Some(label.to_owned());
        self
    }
    pub fn with_model(mut self,model_ref:Option<String>)->Self{
        self.row.model_ref=model_ref;
        self.row.backend_family=Some("llama.cpp".into());
        self.row.context_limit_tokens=Some(4096);
        self
    }
    pub fn cache_scope_admitted(&mut self,admitted:bool){
        self.row.cache_scope_admitted=Some(admitted);
    }
    pub fn catalog_reconciliation_required(&mut self){self.row.catalog_reconciliation_required=true;}
    pub fn admission_publication(&mut self,published:bool,image_count:usize){
        self.row.queue_published=Some(published);
        self.row.admitted_image_count=Some(image_count.min(u32::MAX as usize) as u32);
    }
    pub fn image_stage(&mut self,stage:&'static str){self.row.image_stage=Some(stage);}
    pub fn image_failure(&mut self,code:&'static str){self.row.image_failure_code=Some(code);}
    pub fn finalize_image_outcome(&mut self,phase:&str){
        if phase=="completed"{return}
        if phase=="cancelled"{self.row.image_failure_code=Some("CP01_IMAGE_CANCELLED");return}
        if self.row.image_failure_code.is_none(){
            self.row.image_failure_code=Some(match self.row.image_stage{
                Some("QUEUE_ADMISSION")=>"CP01_IMAGE_QUEUE_FAILED",
                Some("SAVED_IMAGE_READBACK")=>"CP01_IMAGE_SAVED_SOURCE_FAILED",
                Some("CONTEXT_ASSEMBLY")=>"CP01_IMAGE_CONTEXT_FAILED",
                Some("EXACT_IMAGE_RESERVATION")=>"CP01_IMAGE_RESERVATION_FAILED",
                Some("ORIGINAL_RESPONSE_COMMIT"|"ORIGINAL_RESPONSE_SAVED")=>"CP01_IMAGE_JOURNAL_FAILED",
                _=>"CP01_IMAGE_INSUFFICIENT_BACKEND_EVIDENCE",
            });
        }
    }
    pub fn image_delivery(&mut self,receipt:Option<lu_core::multimodal_http::ImageDeliveryReceipt>){
        if let Some(r)=receipt.as_ref(){
            self.row.image_failure_code=r.failure_code;
        }
        self.row.image_delivery=receipt;
    }
    pub fn queued(&mut self){self.queue_entered=Instant::now();}
    pub fn acquired_slot(&mut self){
        self.row.queue_wait_ms=Some(elapsed_ms(self.queue_entered));
        self.context_entered=Some(Instant::now());
    }
    pub fn context_ready(&mut self){
        self.row.context_assembly_ms=self.context_entered.map(elapsed_ms);
    }
    pub fn dispatched(&mut self){self.dispatch_entered=Some(Instant::now());}
    pub fn stream_chunk(&mut self,bytes:usize){
        if self.row.model_dispatch_to_first_chunk_ms.is_none(){
            self.row.model_dispatch_to_first_chunk_ms=self.dispatch_entered.map(elapsed_ms);
            self.row.turn_to_first_chunk_ms=Some(elapsed_ms(self.started));
        }
        self.row.streamed_utf8_bytes=self.row.streamed_utf8_bytes
            .saturating_add(bytes as u64);
    }
    pub fn chunk_forwarded_to_webview(&mut self) {
        if self.row.turn_to_first_webview_forward_ms.is_none(){
            self.row.turn_to_first_webview_forward_ms=Some(elapsed_ms(self.started));
        }
    }
    pub fn with_engine_start_events(mut self,before:u64,after:u64)->Self{
        self.row=self.row.with_engine_start_events(before,after);
        self
    }
    pub fn finish(mut self,terminal:&str)->PerfReceipt{
        if terminal!="COMPLETED"{
            if let Some(trace)=self.row.cp01_trace.as_mut(){
                trace.finish_incomplete(terminal=="CANCELLED");
            }
        }
        self.persist_cp01();
        self.row.terminal_state=terminal.into();
        self.row.overall_latency_ms=elapsed_ms(self.started);
        self.row
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn cp12ar2_negotiated_route_transfers_single_original_trace(){
        use lu_core::agent_tool_loop::ToolDispatchCheckpoint::*;
        let archive=Arc::new(DiagnosticArchive::open(std::path::Path::new(":memory:")).unwrap());
        let ledger=PerfLedger::with_archive(Some(archive.clone()));
        let reference="123456789abcdef0123456789abcdef0";
        let start=Instant::now();
        let mut parent=ModelTrace::new("c-a","g-a",start,true).with_cp01_trace(reference.into(),start,ledger.archive());
        for step in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,Checkpoint::SavedAttested]{parent.causal_checkpoint(step)}
        let native=crate::causal_diagnostic::NativeDiagnosticTrace::resume_parent(parent.take_causal_progress().unwrap(),ledger.archive());
        assert!(parent.take_causal_progress().is_none());
        // Real route rechecks original host authorization; do not repeat
        // already persisted observational checkpoints on the inherited trace.
        for step in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,Checkpoint::SavedAttested,Checkpoint::TaskAuthorized,Checkpoint::ModelEntered]{native.point(step)}
        native.record(|t|{t.tool_checkpoint(Entered);t.tool_checkpoint(Returned);t.tool_checkpoint(FeedbackAudited);});
        native.point(Checkpoint::ModelReturned);native.point(Checkpoint::AssistantSaved);
        native.record(|t|{assert_eq!(t.trace_ref(),reference);assert_eq!(t.dropped(),0);assert!(t.saved());ledger.offer_native_delivery("c-a","g-a",42,false,t.clone())});
        ledger.record(parent.finish("COMPLETED")).unwrap();
        drop(native);
        let w=DeliveryWitness{chat_id:"c-a".into(),generation_id:"g-a".into(),diagnostic_ref:reference.into(),assistant_seq:42,reader:false};
        assert!(!ledger.observe_text_reply(&w,true));assert!(ledger.observe_text_reply(&w,false));assert!(ledger.observe_text_reply(&w,true));
        let report=serde_json::to_value(archive.report().unwrap()).unwrap();
        assert_eq!(report["findings"].as_array().unwrap().len(),1);
        assert_eq!(report["findings"][0]["finding"]["evidenceStrength"],"complete");
        assert_eq!(report["unavailableRecords"],0);
    }
    #[test]
    fn cp12ar1_native_delivery_is_exact_and_requires_prior_reply() {
        use lu_core::agent_tool_loop::ToolDispatchCheckpoint::*;
        let archive=Arc::new(DiagnosticArchive::open(std::path::Path::new(":memory:")).unwrap());
        let ledger=PerfLedger::with_archive(Some(archive.clone()));
        let mut trace=CausalProgress::new("123456789abcdef0123456789abcdef0".into(),Instant::now()).awaiting_delivery();
        for cp in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,Checkpoint::SavedAttested,Checkpoint::TaskAuthorized,Checkpoint::ModelEntered]{assert!(trace.observe(cp));}
        trace.tool_checkpoint(Entered);trace.tool_checkpoint(Returned);trace.tool_checkpoint(FeedbackAudited);
        assert!(trace.observe(Checkpoint::ModelReturned));assert!(trace.observe(Checkpoint::AssistantSaved));
        assert!(archive.persist(&trace));assert!(!trace.terminal());
        let witness=DeliveryWitness{chat_id:"private-chat".into(),generation_id:"private-generation".into(),diagnostic_ref:trace.trace_ref().into(),assistant_seq:42,reader:true};
        ledger.offer_native_delivery(&witness.chat_id,&witness.generation_id,42,true,trace);
        assert!(!ledger.observe_text_reply(&witness,true));
        for wrong in [DeliveryWitness{assistant_seq:43,..witness.clone()},DeliveryWitness{reader:false,..witness.clone()},DeliveryWitness{chat_id:"other".into(),..witness.clone()},DeliveryWitness{generation_id:"old".into(),..witness.clone()}]{assert!(!ledger.observe_text_reply(&wrong,false));}
        assert!(ledger.observe_text_reply(&witness,false));
        assert!(ledger.observe_text_reply(&witness,true));assert!(ledger.observe_text_reply(&witness,true));
        let report=serde_json::to_value(archive.report().unwrap()).unwrap();
        assert_eq!(report["findings"][0]["recordingState"],"sealed");
        assert_eq!(report["findings"][0]["finding"]["evidenceStrength"],"complete");
        for value in [&witness.chat_id,&witness.generation_id,&witness.diagnostic_ref]{assert!(!report.to_string().contains(value));}
    }
    #[test]
    fn cp01_exact_attempt_delivery_persists_only_sanitized_findings(){
        let archive=Arc::new(DiagnosticArchive::open(std::path::Path::new(":memory:")).unwrap());
        let ledger=PerfLedger::with_archive(Some(Arc::clone(&archive)));
        let chat="private-chat-marker";
        let generation="private-generation-marker";
        let trace_id="0123456789abcdef0123456789abcdef";
        let newer_id="fedcba9876543210fedcba9876543210";
        let start=Instant::now();
        let mut trace=ModelTrace::new(chat,generation,start,true)
            .with_cp01_trace(trace_id.into(),start,ledger.archive());
        for stage in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,
            Checkpoint::SavedAttested,Checkpoint::ModelEntered,Checkpoint::ModelReturned,
            Checkpoint::AssistantSaved] {
            trace.causal_checkpoint(stage);
        }
        ledger.record(trace.finish("COMPLETED")).unwrap();
        let mut newer=ModelTrace::new(chat,generation,Instant::now(),true)
            .with_cp01_trace(newer_id.into(),Instant::now(),ledger.archive());
        newer.causal_refused();
        ledger.record(newer.finish("FAILED")).unwrap();
        assert!(!ledger.observe_cp01_reply("other",generation,trace_id,true));
        assert!(!ledger.observe_cp01_reply(chat,"other",trace_id,true));
        assert!(!ledger.observe_cp01_reply(chat,generation,newer_id,true));
        assert!(ledger.observe_cp01_reply(chat,generation,trace_id,false));
        let original=ledger.exact_causal_turn(chat,generation,trace_id).unwrap();
        assert_eq!(original.cp01_finding.unwrap().first_unconfirmed,"stream_closed");
        assert!(ledger.observe_cp01_reply(chat,generation,trace_id,true));
        assert!(ledger.observe_cp01_reply(chat,generation,trace_id,true));
        let original=ledger.exact_causal_turn(chat,generation,trace_id).unwrap();
        assert_eq!(original.cp01_finding.unwrap().evidence_strength,
            diagnostic_localization::EvidenceStrength::Complete);
        assert_eq!(ledger.exact_causal_turn(chat,generation,newer_id).unwrap().terminal_state,"FAILED");
        let report=serde_json::to_value(archive.report().unwrap()).unwrap();
        let json=report.to_string();
        for secret in [chat,generation,trace_id,newer_id] {
            assert!(!json.contains(secret),"private identity leaked into support report");
        }
        assert_eq!(report["unavailableRecords"],0);
        assert_eq!(report["findings"].as_array().unwrap().len(),2);
        assert!(report["findings"].as_array().unwrap().iter().any(|row|
            row["recordingState"]=="sealed" && row["finding"]["evidenceStrength"]=="complete"));
    }
    #[test]
    fn bounded_ledger_is_scoped_and_never_exposes_other_chat_samples(){
        let ledger=PerfLedger::default();
        for i in 0..180{
            let chat=if i%2==0{"a"}else{"b"};
            ledger.record(cpu_receipt(chat,&format!("g-{i}"),
                "EXACT_CHAT_LITERAL",Instant::now(),false,
                Some(42),Some(1),Some(true),"COMPLETED")).unwrap();
        }
        let a=ledger.recent("a",24).unwrap();
        assert_eq!(a.len(),24);
        assert!(a.iter().all(|r|r.chat_id=="a"
            &&r.model_started_by_this_route==Some(false)));
        assert!(a.iter().all(|r|r.prefill_tokens_per_second.is_none()
            &&r.decode_tokens_per_second.is_none()&&r.gpu_used_percent.is_none()));
        assert_eq!(ledger.recent("missing",2).unwrap().len(),0);
        assert!(ledger.recent("a",25).is_err());
    }
    #[test]
    fn background_and_foreground_are_distinct_in_latency_receipts(){
        let background=ModelTrace::new("chat","g",Instant::now(),true)
            .with_background_origin(true).finish("CANCELLED");
        let foreground=ModelTrace::new("chat","h",Instant::now(),true)
            .with_background_origin(false).finish("COMPLETED");
        assert_eq!(background.scheduler_class.as_deref(),Some("BACKGROUND_TERMINAL"));
        assert_eq!(foreground.scheduler_class.as_deref(),Some("FOREGROUND_HUMAN"));
        assert!(background.turn_to_first_chunk_ms.is_none());
        assert!(background.turn_to_first_webview_forward_ms.is_none());
    }
    #[test]
    fn absent_or_failed_first_webview_chunk_is_not_reported_as_visible(){
        let mut trace=ModelTrace::new("c","g",Instant::now(),true);
        trace.queued();
        trace.acquired_slot();
        trace.context_ready();
        trace.dispatched();
        trace.stream_chunk(512);
        let receipt=trace.finish("FAILED");
        assert!(receipt.turn_to_first_chunk_ms.is_some());
        assert!(receipt.turn_to_first_webview_forward_ms.is_none());
        assert!(receipt.decode_tokens_per_second.is_none());
    }
    #[test]
    fn queued_model_timing_is_host_observed_not_fake_token_throughput(){
        let mut span=ModelTrace::new("c","g",Instant::now(),true)
            .with_model(Some("model-opaque".into()))
            .with_background_origin(false);
        span.queued();
        span.cache_scope_admitted(true);
        span.acquired_slot();
        span.context_ready();
        span.dispatched();
        span.stream_chunk(10);
        span.chunk_forwarded_to_webview();
        span.stream_chunk(20);
        let receipt=span.with_engine_start_events(2,4).finish("COMPLETED");
        assert_eq!(receipt.streamed_utf8_bytes,30);
        assert_eq!(receipt.observed_engine_starts_during_turn,Some(2));
        assert_eq!(receipt.observed_engine_starts_since_host_start,Some(4));
        assert_eq!(receipt.model_ref.as_deref(),Some("model-opaque"));
        assert_eq!(receipt.scheduler_class.as_deref(),Some("FOREGROUND_HUMAN"));
        assert_eq!(receipt.backend_family.as_deref(),Some("llama.cpp"));
        assert_eq!(receipt.context_limit_tokens,Some(4096));
        assert_eq!(receipt.model_started_by_this_route,None);
        assert!(receipt.queue_wait_ms.is_some());
        assert!(receipt.context_assembly_ms.is_some());
        assert!(receipt.model_dispatch_to_first_chunk_ms.is_some());
        assert!(receipt.turn_to_first_chunk_ms.is_some());
        assert!(receipt.turn_to_first_webview_forward_ms.is_some());
        assert!(receipt.turn_to_first_webview_forward_ms
            >=receipt.turn_to_first_chunk_ms);
        assert!(receipt.decode_tokens_per_second.is_none());
        assert_eq!(receipt.evidence_scope,"HOST_OBSERVED_ONLY");
    }
}
