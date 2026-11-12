//! PERF-01 P0: low-overhead host-measured chat routing/latency receipts.
//! No model prompt, raw paths, private source text, environment dump, or
//! hypothetical GPU/prefill/token-rate measurements are persisted here.
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::Mutex,
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
    pub queue_wait_ms:Option<u64>,
    pub context_assembly_ms:Option<u64>,
    pub model_dispatch_to_first_chunk_ms:Option<u64>,
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
        route:route.into(),reason_code:reason_code.into(),
        terminal_state:"INCOMPLETE".into(),
        evidence_scope:"HOST_OBSERVED_ONLY".into(),
        source_head:source_head(),
        operating_system:std::env::consts::OS.into(),
        architecture:std::env::consts::ARCH.into(),
        model_loaded_at_entry,model_started_by_this_route:None,
        observed_engine_starts_during_turn:None,
        observed_engine_starts_since_host_start:None,
        model_ref:None,backend_family:None,context_limit_tokens:None,
        queue_wait_ms:None,context_assembly_ms:None,
        model_dispatch_to_first_chunk_ms:None,overall_latency_ms:0,
        original_source_bytes_scanned:None,original_entries_scanned:None,
        source_result_complete:None,streamed_utf8_bytes:0,
        prefill_tokens_per_second:None,decode_tokens_per_second:None,
        gpu_used_percent:None,cache_hit:None,
    }
}
#[derive(Default)]
pub(crate) struct PerfLedger {
    recent:Mutex<VecDeque<PerfReceipt>>,
}
impl PerfLedger{
    pub fn record(&self,row:PerfReceipt)->Result<(),String>{
        // Never print raw source content; performance is a diagnostic,
        // not an authority record or proof of verified task completion.
        let mut q=self.recent.lock().map_err(|_|"Perf telemetry lock unavailable")?;
        q.push_back(row);
        while q.len()>MAX_RECEIPTS{q.pop_front();}
        Ok(())
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
}
impl ModelTrace{
    pub fn new(chat_id:&str,generation_id:&str,started:Instant,model_loaded:bool)
        ->Self{
        Self{
            started,queue_entered:Instant::now(),context_entered:None,
            dispatch_entered:None,
            row:base(chat_id,generation_id,"MODEL_INFERENCE",
                "MODEL_REASONING_REQUIRED",model_loaded),
        }
    }
    pub fn with_model(mut self,model_ref:Option<String>)->Self{
        self.row.model_ref=model_ref;
        self.row.backend_family=Some("llama.cpp".into());
        self.row.context_limit_tokens=Some(4096);
        self
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
        }
        self.row.streamed_utf8_bytes=self.row.streamed_utf8_bytes
            .saturating_add(bytes as u64);
    }
    pub fn with_engine_start_events(mut self,before:u64,after:u64)->Self{
        self.row=self.row.with_engine_start_events(before,after);
        self
    }
    pub fn finish(mut self,terminal:&str)->PerfReceipt{
        self.row.terminal_state=terminal.into();
        self.row.overall_latency_ms=elapsed_ms(self.started);
        self.row
    }
}

#[cfg(test)]
mod tests{
    use super::*;
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
    fn queued_model_timing_is_host_observed_not_fake_token_throughput(){
        let mut span=ModelTrace::new("c","g",Instant::now(),true)
            .with_model(Some("model-opaque".into()));
        span.queued();
        span.acquired_slot();
        span.context_ready();
        span.dispatched();
        span.stream_chunk(10);
        span.stream_chunk(20);
        let receipt=span.with_engine_start_events(2,4).finish("COMPLETED");
        assert_eq!(receipt.streamed_utf8_bytes,30);
        assert_eq!(receipt.observed_engine_starts_during_turn,Some(2));
        assert_eq!(receipt.observed_engine_starts_since_host_start,Some(4));
        assert_eq!(receipt.model_ref.as_deref(),Some("model-opaque"));
        assert_eq!(receipt.backend_family.as_deref(),Some("llama.cpp"));
        assert_eq!(receipt.context_limit_tokens,Some(4096));
        assert_eq!(receipt.model_started_by_this_route,None);
        assert!(receipt.queue_wait_ms.is_some());
        assert!(receipt.context_assembly_ms.is_some());
        assert!(receipt.model_dispatch_to_first_chunk_ms.is_some());
        assert!(receipt.decode_tokens_per_second.is_none());
        assert_eq!(receipt.evidence_scope,"HOST_OBSERVED_ONLY");
    }
}
