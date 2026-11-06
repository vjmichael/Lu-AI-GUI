//! OBSERVE-AWARENESS-06 — lightweight awareness, notation and escalation
//! policy, inside the existing ObserveHost/TaskStore/ANSF architecture.
//!
//! This is not another agent process. Native event checks work without an
//! LLM/GPU. Optional tiny-model inference is *recommended* only when a host
//! confirms a separately authorized, available background model; it is not
//! launched here. Output from either model never grants tools or makes an
//! unverified GUI signal a test success/failure.
use std::collections::VecDeque;

const MAX_NOTES:usize=24;
const MAX_BRIEF_PER_MIN:usize=3;
const MAX_HANDOFF_PER_MIN:usize=1;
const MIN_NOTE_GAP_MS:u64=1500;
const MINUTE_MS:u64=60_000;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Source {
    /// A React/WebView hint is not OS-attested and cannot prove a GUI bug.
    RendererHint,
    /// A host-verified OS accessibility event. Not a native effect receipt.
    NativeAccessibility,
    /// A separately signed-off native test/effect receipt from the host.
    NativeTestReceipt,
}
impl Source {
    pub fn as_str(self)->&'static str{
        match self{
            Self::RendererHint=>"renderer_hint",
            Self::NativeAccessibility=>"native_accessibility",
            Self::NativeTestReceipt=>"native_test_receipt",
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Signal {
    UiChanged,
    ErrorIndicator,
    VerifiedTestFailure,
    VerifiedTestSuccess,
    StalledProgress,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum NoteKind {
    ErrorIndicator,
    UnexpectedUiState,
    VerifiedTestFailure,
    StalledProgress,
}
impl NoteKind {
    pub fn as_str(self)->&'static str{
        match self{
            Self::ErrorIndicator=>"error_indicator",
            Self::UnexpectedUiState=>"unexpected_ui_state",
            Self::VerifiedTestFailure=>"verified_test_failure",
            Self::StalledProgress=>"stalled_progress",
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Priority { Observe, Investigate }
impl Priority {
    pub fn as_str(self)->&'static str {
        match self{Self::Observe=>"observe",Self::Investigate=>"investigate"}
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NativeSignal {
    pub sequence:u64,
    pub signal:Signal,
    pub source:Source,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Note {
    pub sequence:u64,
    pub kind:NoteKind,
    pub source:Source,
    pub priority:Priority,
    /// No raw screenshot, app text, model response, window title or path.
    pub verified_failure:bool,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ReasoningRequest {
    /// May be scheduled only by the existing host if an authorized small
    /// model is already loaded and idle; not a model-load instruction.
    BriefSmallModel,
    /// Handoff is a *review request*, never automatic edit/build authority.
    MainModelReview,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AwarenessDecision {
    pub notation:Option<Note>,
    pub suggest:Option<ReasoningRequest>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum AwarenessError {
    NotActive, WrongSession, Replay, InvalidProvenance, Expired, InvalidScope,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Handoff {
    pub observation_session_id:String,
    pub chat_id:String,
    pub latest_sequence:u64,
    pub verified_failures:usize,
    pub unverified_indicators:usize,
    pub recommended_action:&'static str,
    pub requires_separate_tool_authorization:bool,
    pub main_model_may_treat_as_verified_failure:bool,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HostReadiness {
    pub small_model_preapproved:bool,
    pub small_model_loaded_idle:bool,
    pub interactive_turn_waiting:bool,
    pub main_model_preapproved_for_review:bool,
}
impl Default for HostReadiness {
    fn default()->Self{
        Self{
            small_model_preapproved:false,small_model_loaded_idle:false,
            interactive_turn_waiting:false,main_model_preapproved_for_review:false,
        }
    }
}
#[derive(Debug)]
pub struct AwarenessLane {
    session:String,
    chat:String,
    active:bool,
    last_sequence:u64,
    recent:VecDeque<(u64,Note)>,
    brief_times:VecDeque<u64>,
    handoff_times:VecDeque<u64>,
}
fn id(value:&str)->bool{
    !value.is_empty()&&value.len()<=128&&
      value.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'||c==b'_')
}
impl AwarenessLane {
    /// The existing host creates this only as part of a consented Observe
    /// session. This does not create its own scheduler or model process.
    pub fn new(session:&str,chat:&str)->Result<Self,AwarenessError>{
        if !id(session)||!session.starts_with("obs-")
            ||!id(chat)||!chat.starts_with("c-"){
            return Err(AwarenessError::InvalidScope);
        }
        Ok(Self{
            session:session.into(),chat:chat.into(),active:true,
            last_sequence:0,recent:VecDeque::new(),
            brief_times:VecDeque::new(),handoff_times:VecDeque::new(),
        })
    }
    pub fn stop(&mut self){
        self.active=false;
        self.recent.clear();
        self.brief_times.clear();
        self.handoff_times.clear();
    }
    fn trim(&mut self,now:u64){
        while self.recent.front().is_some_and(|(t,_)|
            now.saturating_sub(*t)>=MINUTE_MS){self.recent.pop_front();}
        while self.brief_times.front().is_some_and(|t|
            now.saturating_sub(*t)>=MINUTE_MS){self.brief_times.pop_front();}
        while self.handoff_times.front().is_some_and(|t|
            now.saturating_sub(*t)>=MINUTE_MS){self.handoff_times.pop_front();}
    }
    /// Trusted OS adapters must separately attest Source. Never deserialize
    /// Source::NativeTestReceipt from a renderer or model argument.
    pub fn ingest(
        &mut self,session:&str,chat:&str,event:NativeSignal,now_ms:u64,
        readiness:HostReadiness
    )->Result<AwarenessDecision,AwarenessError>{
        if !self.active{return Err(AwarenessError::NotActive);}
        if self.session!=session||self.chat!=chat{
            return Err(AwarenessError::WrongSession);
        }
        if event.sequence==0||event.sequence<=self.last_sequence{
            return Err(AwarenessError::Replay);
        }
        if event.source!=Source::NativeTestReceipt&&
            matches!(event.signal,Signal::VerifiedTestFailure|Signal::VerifiedTestSuccess){
            return Err(AwarenessError::InvalidProvenance);
        }
        self.last_sequence=event.sequence;
        self.trim(now_ms);
        let kind=match event.signal{
            Signal::ErrorIndicator=>Some(NoteKind::ErrorIndicator),
            Signal::VerifiedTestFailure=>Some(NoteKind::VerifiedTestFailure),
            Signal::StalledProgress=>Some(NoteKind::StalledProgress),
            Signal::UiChanged|Signal::VerifiedTestSuccess=>None,
        };
        let Some(kind)=kind else{
            return Ok(AwarenessDecision{notation:None,suggest:None});
        };
        let verified=kind==NoteKind::VerifiedTestFailure
            &&event.source==Source::NativeTestReceipt;
        let recent_count=self.recent.iter()
            .filter(|(_,n)|n.kind==kind).count();
        let note=Note{
            sequence:event.sequence,
            kind,source:event.source,
            priority:if verified||recent_count>=2{
                Priority::Investigate
            }else{Priority::Observe},
            verified_failure:verified,
        };
        let duplicate=self.recent.back().is_some_and(|(t,prior)|
            prior.kind==kind&&prior.source==event.source&&
            now_ms.saturating_sub(*t)<MIN_NOTE_GAP_MS);
        // Keep local signals inexpensive; repeated hints are never silently
        // upgraded to independently verified application failures.
        if duplicate {
            return Ok(AwarenessDecision{notation:None,suggest:None});
        }
        if self.recent.len()>=MAX_NOTES{self.recent.pop_front();}
        self.recent.push_back((now_ms,note.clone()));
        let can_reason=!readiness.interactive_turn_waiting;
        let suggest=if can_reason&&note.priority==Priority::Investigate
            &&readiness.main_model_preapproved_for_review
            &&self.handoff_times.len()<MAX_HANDOFF_PER_MIN{
            self.handoff_times.push_back(now_ms);
            Some(ReasoningRequest::MainModelReview)
        }else if can_reason
            &&readiness.small_model_preapproved
            &&readiness.small_model_loaded_idle
            &&self.brief_times.len()<MAX_BRIEF_PER_MIN{
            self.brief_times.push_back(now_ms);
            Some(ReasoningRequest::BriefSmallModel)
        }else{None};
        Ok(AwarenessDecision{notation:Some(note),suggest})
    }
    /// Bounded handoff content is structured provenance, NOT a model-authored
    /// prompt or a claimed GUI test verdict. Model tooling remains scoped by
    /// the existing Lu host policy; this structure grants no execution.
    pub fn handoff(&self,session:&str,chat:&str)
        ->Result<Option<Handoff>,AwarenessError>{
        if !self.active{return Err(AwarenessError::NotActive);}
        if self.session!=session||self.chat!=chat{
            return Err(AwarenessError::WrongSession);
        }
        if self.recent.is_empty(){return Ok(None);}
        let verified_failures=self.recent.iter()
            .filter(|(_,n)|n.verified_failure).count();
        let unverified_indicators=self.recent.iter()
            .filter(|(_,n)|!n.verified_failure).count();
        Ok(Some(Handoff{
            observation_session_id:self.session.clone(),
            chat_id:self.chat.clone(),latest_sequence:self.last_sequence,
            verified_failures,unverified_indicators,
            recommended_action:if verified_failures>0{
                "Review independently verified failed test"
            }else{"Investigate observed UI indicators; verification pending"},
            requires_separate_tool_authorization:true,
            main_model_may_treat_as_verified_failure:verified_failures>0,
        }))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn e(sequence:u64,signal:Signal,source:Source)->NativeSignal{
        NativeSignal{sequence,signal,source}
    }
    #[test]
    fn no_model_consumption_without_separate_readiness_and_grant(){
        let mut lane=AwarenessLane::new("obs-one","c-main").unwrap();
        let d=lane.ingest("obs-one","c-main",
            e(1,Signal::ErrorIndicator,Source::RendererHint),1000,
            HostReadiness::default()).unwrap();
        assert_eq!(d.suggest,None);
        assert_eq!(d.notation.unwrap().priority,Priority::Observe);
        let h=lane.handoff("obs-one","c-main").unwrap().unwrap();
        assert_eq!(h.verified_failures,0);
        assert_eq!(h.unverified_indicators,1);
        assert!(!h.main_model_may_treat_as_verified_failure);
    }
    #[test]
    fn explicit_small_model_inference_is_recommended_not_executed(){
        let mut lane=AwarenessLane::new("obs-one","c-main").unwrap();
        let ready=HostReadiness{
            small_model_preapproved:true,small_model_loaded_idle:true,
            ..HostReadiness::default()
        };
        assert_eq!(lane.ingest("obs-one","c-main",
            e(1,Signal::ErrorIndicator,Source::RendererHint),1000,ready)
            .unwrap().suggest,Some(ReasoningRequest::BriefSmallModel));
        assert_eq!(lane.ingest("obs-one","c-main",
            e(2,Signal::ErrorIndicator,Source::RendererHint),3000,
            HostReadiness{interactive_turn_waiting:true,..ready})
            .unwrap().suggest,None);
        assert_eq!(lane.ingest("obs-one","c-main",
            e(3,Signal::StalledProgress,Source::NativeAccessibility),5000,ready)
            .unwrap().suggest,Some(ReasoningRequest::BriefSmallModel));
    }
    #[test]
    fn renderer_cannot_forge_verified_test_failure(){
        let mut lane=AwarenessLane::new("obs-one","c-main").unwrap();
        assert_eq!(lane.ingest("obs-one","c-main",
            e(1,Signal::VerifiedTestFailure,Source::RendererHint),1000,
            HostReadiness::default()),Err(AwarenessError::InvalidProvenance));
        let result=lane.ingest("obs-one","c-main",
            e(1,Signal::VerifiedTestFailure,Source::NativeTestReceipt),1000,
            HostReadiness::default()).unwrap();
        assert!(result.notation.unwrap().verified_failure);
        let h=lane.handoff("obs-one","c-main").unwrap().unwrap();
        assert!(h.main_model_may_treat_as_verified_failure);
    }
    #[test]
    fn repeated_hints_escalate_suspicion_not_proof(){
        let mut lane=AwarenessLane::new("obs-one","c-main").unwrap();
        for i in 1..=3{
            let d=lane.ingest("obs-one","c-main",
                e(i,Signal::ErrorIndicator,Source::RendererHint),
                i*2000,HostReadiness::default()).unwrap();
            assert_eq!(d.notation.unwrap().priority,
                if i==3{Priority::Investigate}else{Priority::Observe});
        }
        assert_eq!(lane.handoff("obs-one","c-main").unwrap().unwrap()
            .verified_failures,0);
    }
    #[test]
    fn low_priority_ui_change_and_duplicate_hints_do_not_spam(){
        let mut lane=AwarenessLane::new("obs-one","c-main").unwrap();
        assert_eq!(lane.ingest("obs-one","c-main",
            e(1,Signal::UiChanged,Source::RendererHint),0,
            HostReadiness::default()).unwrap().notation,None);
        assert!(lane.ingest("obs-one","c-main",
            e(2,Signal::ErrorIndicator,Source::RendererHint),1000,
            HostReadiness::default()).unwrap().notation.is_some());
        assert_eq!(lane.ingest("obs-one","c-main",
            e(3,Signal::ErrorIndicator,Source::RendererHint),1001,
            HostReadiness::default()).unwrap().notation,None);
        assert_eq!(lane.ingest("obs-one","c-main",
            e(3,Signal::ErrorIndicator,Source::RendererHint),1002,
            HostReadiness::default()),Err(AwarenessError::Replay));
    }
    #[test]
    fn session_and_chat_are_isolated_and_stop_erases_volatile_evidence(){
        let mut lane=AwarenessLane::new("obs-one","c-main").unwrap();
        assert_eq!(lane.ingest("obs-two","c-main",
            e(1,Signal::ErrorIndicator,Source::RendererHint),1000,
            HostReadiness::default()),Err(AwarenessError::WrongSession));
        assert_eq!(lane.handoff("obs-one","c-other"),Err(AwarenessError::WrongSession));
        lane.stop();
        assert_eq!(lane.handoff("obs-one","c-main"),Err(AwarenessError::NotActive));
        assert_eq!(lane.ingest("obs-one","c-main",
            e(1,Signal::ErrorIndicator,Source::RendererHint),1000,
            HostReadiness::default()),Err(AwarenessError::NotActive));
    }
}
