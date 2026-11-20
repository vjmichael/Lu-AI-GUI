//! CP-01E: per-invocation host observations for the existing DX v2 protocol.
//! This is an adapter, not an authorization or recovery journal. No content,
//! paths, conversation IDs, model output, or exception strings are collected.
use std::{time::Instant,sync::{Arc,Mutex}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum Checkpoint {
    NativeReceived = 4,
    SessionReady = 19,
    UserSaved = 2,
    WorkerEntered = 7,
    SavedAttested = 5,
    TaskAuthorized = 6,
    ModelEntered = 8,
    ModelReturned = 9,
    AssistantSaved = 12,
    ReplyObserved = 10,
    StreamClosed = 11,
    AssistantRendered = 13,
}

const ORDER: &[Checkpoint] = &[
    Checkpoint::NativeReceived, Checkpoint::SessionReady, Checkpoint::UserSaved,
    Checkpoint::WorkerEntered, Checkpoint::SavedAttested, Checkpoint::ModelEntered,
    Checkpoint::ModelReturned, Checkpoint::AssistantSaved, Checkpoint::ReplyObserved,
    Checkpoint::StreamClosed, Checkpoint::AssistantRendered,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observation {
    pub stage: i32,
    pub outcome: i32,
    pub failure_code: i32,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct CausalProgress {
    trace_ref: String,
    started: Instant,
    observations: Vec<Observation>,
    dropped: u32,
    failed: bool,
    // Cursor for the original CP-01 checkpoints; additional tool observations
    // never advance it. 0=idle, 1=dispatch entered, 2=returned but not audited.
    cursor: usize,
    tool_phase: u8,
    await_delivery: bool,
}

impl CausalProgress {
    pub fn new(trace_ref: String, started: Instant) -> Self {
        let mut trace = Self {
            trace_ref, started, observations: Vec::with_capacity(ORDER.len()),
            dropped: 0, failed: false, cursor: 1, tool_phase: 0, await_delivery:false,
        };
        // The trusted native wrapper minted the reference at its entry.
        trace.observations.push(Observation {
            stage: Checkpoint::NativeReceived as i32, outcome: 1,
            failure_code: 0, elapsed_ms: 0,
        });
        trace
    }

    pub fn trace_ref(&self) -> &str { &self.trace_ref }
    pub fn observations(&self) -> &[Observation] { &self.observations }
    pub fn dropped(&self) -> u32 { self.dropped }
    pub fn mark_lost(&mut self) { self.dropped = self.dropped.saturating_add(1); }
    pub fn terminal(&self) -> bool {
        self.failed || self.observations.last().is_some_and(|e|
            e.stage == Checkpoint::AssistantRendered as i32 ||
            (e.stage == Checkpoint::AssistantSaved as i32 && self.has_tool_authority() && !self.await_delivery))
    }

    pub fn awaiting_delivery(mut self)->Self { self.await_delivery=true; self }
    pub fn saved(&self)->bool { !self.failed && self.cursor>=8 && self.observations.iter().any(|e|e.stage==12&&e.outcome==2) }
    pub fn reply_seen(&self)->bool { self.observations.iter().any(|e|e.stage==10&&e.outcome==2) }
    /// A real queue/control cancellation proves the request was fenced, not
    /// worker exit or rollback. Avoid synthesizing either downstream event.
    pub fn observed_cancel(&mut self) {
        if self.terminal()||self.saved(){return}
        self.append(16,4,9); self.failed=true;
    }
    fn has_tool_authority(&self)->bool { self.observations.iter().any(|e|e.stage==6) }

    fn append(&mut self, stage: i32, outcome: i32, failure_code: i32) {
        if self.observations.len() >= 64 { self.mark_lost(); return; }
        let elapsed_ms = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        self.observations.push(Observation { stage, outcome, failure_code, elapsed_ms });
    }

    pub fn observe(&mut self, stage: Checkpoint) -> bool {
        // A duplicate renderer report is idempotent. A report before the
        // exact original host commit is not evidence and cannot be promoted.
        if matches!(stage, Checkpoint::ReplyObserved | Checkpoint::StreamClosed |
            Checkpoint::AssistantRendered) && !self.failed &&
            self.observations.iter().any(|e| e.stage == stage as i32) {
            return true;
        }
        if stage == Checkpoint::TaskAuthorized {
            if self.failed || self.cursor != 5 || self.has_tool_authority() { self.mark_lost(); return false; }
            self.append(6,2,0); return true;
        }
        if self.failed || ORDER.get(self.cursor) != Some(&stage) ||
            (matches!(stage,Checkpoint::ModelEntered|Checkpoint::ModelReturned) && self.tool_phase!=0) {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        let outcome = if matches!(stage, Checkpoint::WorkerEntered | Checkpoint::ModelEntered) { 1 } else { 2 };
        self.append(stage as i32, outcome, 0);
        self.cursor += 1;
        true
    }

    /// Only call at an actual error return from the original model dispatch.
    /// The cause remains unknown; this is not proof a worker exited.
    pub fn model_failed(&mut self) {
        self.model_failed_with(1);
    }
    pub fn model_failed_with(&mut self, code:i32) {
        if self.failed || ORDER.get(self.cursor) != Some(&Checkpoint::ModelReturned) {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.append(Checkpoint::ModelReturned as i32, if code==9 {4}else{3}, code);
        self.failed = true;
    }

    /// The actual dispatch boundary is not a typed result. Only the shared
    /// supervised loop can confirm a validated and durably audited envelope.
    pub fn tool_checkpoint(&mut self, point:lu_core::agent_tool_loop::ToolDispatchCheckpoint) {
        self.tool_checkpoint_at(point,6);
    }
    /// Host-authored search calls occur after authority but before inference.
    /// Never manufacture ModelEntered for a host-selected operation.
    pub fn host_tool_checkpoint(&mut self, point:lu_core::agent_tool_loop::ToolDispatchCheckpoint) {
        self.tool_checkpoint_at(point,5);
    }
    fn tool_checkpoint_at(&mut self,point:lu_core::agent_tool_loop::ToolDispatchCheckpoint,cursor:usize){
        use lu_core::agent_tool_loop::ToolDispatchCheckpoint::*;
        if self.failed || self.cursor != cursor || !self.has_tool_authority() || self.observations.len()>=60 { self.mark_lost(); return; }
        match (self.tool_phase,point) {
            (0,Entered)=>{self.append(14,1,0);self.tool_phase=1},
            (1,Returned)=>{self.append(15,1,0);self.tool_phase=2},
            (2,FeedbackAudited)=>{self.append(15,2,0);self.tool_phase=0},
            _=>self.mark_lost(),
        }
    }
    pub fn human_declined(&mut self) {
        if !self.failed { self.append(20,3,3); self.failed=true; }
    }
    /// A canonical refusal proves that the original nonce was fenced, but
    /// not which earlier native admission check failed.
    pub fn refused(&mut self) {
        if self.failed { return; }
        self.append(20, 3, 1);
        self.failed = true;
    }

    /// Generic failure/cancellation with no independently observed cause.
    /// Mark evidence incomplete; never infer worker termination or rollback.
    pub fn finish_incomplete(&mut self, cancelled: bool) {
        if self.terminal() || self.saved() { return; }
        self.dropped = self.dropped.saturating_add(1);
        self.append(21, if cancelled { 4 } else { 3 }, if cancelled { 9 } else { 1 });
        self.failed = true;
    }
}

/// The original native transaction uses the existing DX journal. A
/// dropped observation can only reduce assurance, never change tool authority.
pub(crate) struct NativeDiagnosticTrace {
    trace:Mutex<CausalProgress>,
    archive:Option<Arc<crate::diagnostic_archive::DiagnosticArchive>>,
    correlated:bool,
    inherited_prefix:bool,
}
impl NativeDiagnosticTrace {
    pub fn new(host:&crate::DesktopState)->Self {
        Self::correlated(host,None)
    }
    pub fn correlated(host:&crate::DesktopState,context:Option<(String,Instant)>)->Self {
        let correlated=context.is_some();
        let (reference,started)=context.unwrap_or_else(||(crate::next_id(""),Instant::now()));
        let trace=CausalProgress::new(reference,started);
        let trace=if correlated{trace.awaiting_delivery()}else{trace};
        let this=Self{trace:Mutex::new(trace),archive:host.perf01.archive(),correlated,inherited_prefix:false};
        this.record(|_|{});this
    }
    /// The negotiated route consumes the original parent CP-01 prefix. The
    /// parent no longer writes it. Existing native prefix checks are repeated
    /// for authorization but must not duplicate old observational events.
    pub fn from_parent(host:&crate::DesktopState,parent:Option<CausalProgress>)->Self {
        let Some(trace)=parent else{return Self::new(host)};
        Self::resume_parent(trace,host.perf01.archive())
    }
    pub(crate) fn resume_parent(trace:CausalProgress,archive:Option<Arc<crate::diagnostic_archive::DiagnosticArchive>>)->Self{
        Self{trace:Mutex::new(trace.awaiting_delivery()),archive,correlated:true,inherited_prefix:true}
    }
    pub fn record(&self,apply:impl FnOnce(&mut CausalProgress)) {
        if let Ok(mut trace)=self.trace.lock(){
            apply(&mut trace);
            if let Some(archive)=&self.archive{if !archive.persist(&trace){trace.mark_lost();}}
        }
    }
    pub fn point(&self,point:Checkpoint){self.record(|t|{
        if self.inherited_prefix && matches!(point,Checkpoint::SessionReady|Checkpoint::UserSaved|Checkpoint::WorkerEntered|Checkpoint::SavedAttested)
            && t.observations().iter().any(|e|e.stage==point as i32){return}
        t.observe(point);
    });}
    pub fn decline(&self){self.record(|t|t.human_declined());}
    pub fn lost(&self){self.record(|t|t.mark_lost());}
    pub fn incomplete(&self,cancelled:bool){self.record(|t|if cancelled{t.observed_cancel()}else{t.finish_incomplete(false)});}
    pub fn saved_response(&self,host:&crate::DesktopState,chat:&str,gid:&str,answer:&str,reader:bool){
        let seq=host.chats.require(chat).ok().and_then(|book|book.diagnostic_completed_assistant_seq(chat,gid,Some(answer),reader).ok().flatten());
        if let Some(seq)=seq {
            self.point(Checkpoint::AssistantSaved);
            if self.correlated {
                if let Ok(trace)=self.trace.lock(){
                    if trace.saved(){host.perf01.offer_native_delivery(chat,gid,seq,reader,trace.clone());}
                }
            }
        }else{self.lost()}
    }
    pub fn model_error(&self,error:&lu_core::supervised_tool_generation::NativeGenerationError)->&'static str{
        let (code,message)=native_failure(error);
        self.record(|t|t.model_failed_with(code));message
    }
}
impl Drop for NativeDiagnosticTrace {
    fn drop(&mut self){self.record(|t|t.finish_incomplete(false));}
}
fn native_failure(error:&lu_core::supervised_tool_generation::NativeGenerationError)->(i32,&'static str){
    use lu_core::supervised_tool_generation::native_generation_failure_class;
    match native_generation_failure_class(error){
        "cancelled"=>(9,"Cancelled inspection. Check the conversation before retrying."),
        "authority_rejected"|"policy_denied"=>(3,"The requested access was not authorized. Review the selected permission."),
        "inference_failed"=>(1,"The loaded model could not finish the inspection. Check the model and try again."),
        "journal_failed"=>(4,"Lu could not confirm a saved inspection record. Check the conversation and available storage."),
        "task_deadline"=>(8,"The inspection reached its time limit. Try a smaller request."),
        "model_reply_rejected"|"json_regeneration_exhausted"|"required_observation_missing"|"tool_contract_rejected"|"inference_event_invalid"=>(10,"Lu could not verify the model's inspection response. Try a clearer request or another model."),
        _=>(1,"The inspection did not complete. Check the conversation before trying again."),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn progress() -> CausalProgress {
        CausalProgress::new("0123456789abcdef0123456789abcdef".into(), Instant::now())
    }
    fn advance(t: &mut CausalProgress, stop: usize) {
        for stage in ORDER.iter().take(stop).skip(1) { assert!(t.observe(*stage)); }
    }
    #[test]
    fn cp12ar2closeout_host_search_tool_is_observed_before_model_and_must_be_audited(){
        use lu_core::agent_tool_loop::ToolDispatchCheckpoint::*;
        let mut t=progress();advance(&mut t,5);
        assert!(t.observe(Checkpoint::TaskAuthorized));
        t.host_tool_checkpoint(Entered);t.host_tool_checkpoint(Returned);
        assert!(!t.observe(Checkpoint::ModelEntered));
        t.host_tool_checkpoint(FeedbackAudited);
        assert!(t.observe(Checkpoint::ModelEntered));
        assert!(t.observe(Checkpoint::ModelReturned));
        assert_eq!(t.observations().iter().map(|e|e.stage).collect::<Vec<_>>(),vec![4,19,2,7,5,6,14,15,15,8,9]);
        assert_eq!(t.dropped(),1);
        let mut wrong=progress();advance(&mut wrong,5);assert!(wrong.observe(Checkpoint::TaskAuthorized));
        wrong.tool_checkpoint(Entered);assert_eq!(wrong.dropped(),1);
        assert!(!wrong.observations().iter().any(|e|e.stage==14));
    }
    #[test]
    fn host_completion_is_not_renderer_completion() {
        let mut t = progress(); advance(&mut t, 8);
        assert_eq!(t.observations().last().unwrap().stage, 12);
        assert!(t.observe(Checkpoint::ReplyObserved));
        assert_eq!(t.observations().len(), 9);
        assert!(t.observe(Checkpoint::ReplyObserved));
        assert_eq!(t.observations().len(), 9);
        assert_eq!(t.dropped(), 0);
    }
    #[test]
    fn premature_or_out_of_order_reply_cannot_complete_trace() {
        let mut t = progress();
        assert!(!t.observe(Checkpoint::ReplyObserved));
        assert_eq!(t.observations().len(), 1);
        advance(&mut t, 8);
        assert_eq!(t.dropped(), 1);
    }
    #[test]
    fn model_failure_preserves_exact_boundary_without_worker_exit() {
        let mut t = progress(); advance(&mut t, 6); t.model_failed();
        assert_eq!(t.observations().last().unwrap().stage, 9);
        assert_eq!(t.observations().last().unwrap().outcome, 3);
        assert!(!t.observe(Checkpoint::AssistantSaved));
        assert_eq!(t.dropped(), 1);
        assert!(!t.observations().iter().any(|e| e.stage == 17));
    }
    #[test]
    fn refusal_and_cancellation_are_distinct_and_bounded() {
        let mut t = progress(); t.refused();
        let mut c = progress(); c.finish_incomplete(true);
        assert_eq!(t.observations().last().unwrap().stage, 20);
        assert_eq!(c.observations().last().unwrap().failure_code, 9);
        assert_eq!(c.dropped(), 1);
        for _ in 0..1000 { c.finish_incomplete(true); }
        assert_eq!(c.observations().len(), 2);
    }
    #[test]
    fn independent_invocations_do_not_merge_or_share_checkpoint_state() {
        let mut a = progress(); advance(&mut a, 5);
        let b = CausalProgress::new("fedcba9876543210fedcba9876543210".into(), Instant::now());
        assert_ne!(a.trace_ref(), b.trace_ref());
        assert_eq!(b.observations().len(), 1);
        assert!(a.observations().windows(2).all(|w| w[0].elapsed_ms <= w[1].elapsed_ms));
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    #[test]
    fn two_renderer_milestones_are_required_after_reply() {
        let mut t = CausalProgress::new("0123456789abcdef0123456789abcdef".into(), Instant::now());
        for stage in ORDER.iter().take(9).skip(1) { assert!(t.observe(*stage)); }
        assert!(!t.terminal());
        assert!(t.observe(Checkpoint::StreamClosed));
        assert!(!t.terminal());
        assert!(t.observe(Checkpoint::AssistantRendered));
        assert!(t.terminal());
        assert!(t.observe(Checkpoint::ReplyObserved));
        assert!(t.observe(Checkpoint::AssistantRendered));
        assert_eq!(t.observations().len(), 11);
        assert_eq!(t.dropped(), 0);
    }
    #[test]
    fn out_of_order_render_and_storage_loss_never_prove_complete_evidence() {
        let mut t = CausalProgress::new("fedcba9876543210fedcba9876543210".into(), Instant::now());
        for stage in ORDER.iter().take(9).skip(1) { assert!(t.observe(*stage)); }
        assert!(!t.observe(Checkpoint::AssistantRendered));
        assert!(!t.terminal());
        t.mark_lost();
        assert!(t.observe(Checkpoint::StreamClosed));
        assert!(t.observe(Checkpoint::AssistantRendered));
        assert_eq!(t.dropped(), 2);
    }
}
