//! OBSERVE-LEARN-01: privacy-first event-driven native decision policy.
//! OS adapters, screen capture, model dispatch and learning persistence are
//! separate, unfinished host integrations. No raw pixels, text or keylogging.
use std::collections::{BTreeSet,VecDeque};
const WINDOW:u64=60_000;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Mode {Off,Active,Paused}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Signal {Focus,UiChanged,ErrorIndicator,Checkpoint,TestPassed,TestFailed}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Tier {Brief,Deep}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Decision {LocalOnly,RecommendInference{tier:Tier,sequence:u64,redacted_snapshot_permitted:bool}}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Error {MissingConsent,InvalidScope,Expired,OutsideApp,SnapshotDenied,Replay,InvalidTransition}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Consent {
    pub session_id:String,
    pub app_ids:Vec<String>,
    pub expires_ms:u64,
    pub allow_redacted_snapshot:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NativeEvent {
    // Only the trusted OS adapter may construct real events; model text is
    // never an app identity, window grant or event-source attestation.
    pub app_id:String,
    pub sequence:u64,
    pub signal:Signal,
    pub requests_redacted_snapshot:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Checkpoint {pub app_id:String,pub sequence:u64,pub signal:Signal}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LearningProposal {
    pub session_id:String,
    pub evidence:Vec<Checkpoint>,
    pub human_review_required:bool,
    pub model_training_authorized:bool,
}
#[derive(Debug)]
pub struct Controller {
    mode:Mode,
    scope:Option<Consent>,
    last_seq:u64,
    last_change:Option<u64>,
    event_times:VecDeque<u64>,
    reasoning:VecDeque<(u64,Tier)>,
    checkpoints:VecDeque<Checkpoint>,
    learning_review:bool,
}
impl Default for Controller {
    fn default()->Self {Self{
        mode:Mode::Off,scope:None,last_seq:0,last_change:None,
        event_times:VecDeque::new(),reasoning:VecDeque::new(),
        checkpoints:VecDeque::new(),learning_review:false,
    }}
}
fn safe_id(s:&str)->bool {
    !s.is_empty()&&s.len()<=128&&
      s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
impl Controller {
    pub fn mode(&self)->Mode {self.mode}
    /// Existing native watchdog calls this even if the UI is silent.
    pub fn tick(&mut self,now:u64)->Result<Mode,Error>{
        if self.mode!=Mode::Off{self.check_expiry(now)?;}
        Ok(self.mode)
    }
    /// Must be invoked ONLY by trusted native host UI after explicit user
    /// consent. No Tauri/model-facing start command is provided in this slice.
    pub fn start(&mut self,grant:Consent,now:u64)->Result<(),Error>{
        if self.mode!=Mode::Off{return Err(Error::InvalidTransition);}
        if !safe_id(&grant.session_id)||!grant.session_id.starts_with("obs-")||
            grant.app_ids.is_empty()||grant.app_ids.len()>8||
            grant.app_ids.iter().any(|x|!safe_id(x))||
            now>=grant.expires_ms||grant.expires_ms-now>7_200_000{
            return Err(Error::InvalidScope);
        }
        let unique:BTreeSet<_>=grant.app_ids.iter().collect();
        if unique.len()!=grant.app_ids.len(){return Err(Error::InvalidScope);}
        *self=Self::default();
        self.scope=Some(grant);self.mode=Mode::Active;
        Ok(())
    }
    pub fn pause(&mut self)->Result<(),Error>{
        if self.mode!=Mode::Active{return Err(Error::InvalidTransition);}
        self.mode=Mode::Paused;Ok(())
    }
    pub fn resume(&mut self,now:u64)->Result<(),Error>{
        if self.mode!=Mode::Paused{return Err(Error::InvalidTransition);}
        self.check_expiry(now)?;self.mode=Mode::Active;Ok(())
    }
    /// Wipes volatile session and evidence; restart never auto-restores grant.
    pub fn stop(&mut self){*self=Self::default();}
    fn check_expiry(&mut self,now:u64)->Result<(),Error>{
        if self.scope.as_ref().is_some_and(|x|now>=x.expires_ms){
            self.stop();return Err(Error::Expired);
        }
        Ok(())
    }
    pub fn grant_learning_review(&mut self,consented:bool)->Result<(),Error>{
        if self.mode==Mode::Off{return Err(Error::MissingConsent);}
        self.learning_review=consented;
        if !consented{self.checkpoints.clear();}
        Ok(())
    }
    /// Host-only event decision. Returns no OS actions, prompt text, raw
    /// screen/audio, memory writes or authority to operate the computer.
    pub fn ingest(&mut self,event:NativeEvent,now:u64,model_ready_idle:bool)
       ->Result<Decision,Error>{
        if self.mode!=Mode::Active{return Err(Error::MissingConsent);}
        self.check_expiry(now)?;
        let scope=self.scope.as_ref().ok_or(Error::MissingConsent)?;
        if !scope.app_ids.contains(&event.app_id){return Err(Error::OutsideApp);}
        if event.sequence==0||event.sequence<=self.last_seq{
            return Err(Error::Replay);
        }
        if event.requests_redacted_snapshot&&!scope.allow_redacted_snapshot{
            return Err(Error::SnapshotDenied);
        }
        self.last_seq=event.sequence;
        // A trusted independent test harness must issue TestPassed/Failed.
        // Seeing an error indicator alone never verifies an actual test failure.
        if matches!(event.signal,Signal::Checkpoint|Signal::TestPassed|Signal::TestFailed){
            if self.checkpoints.len()>=32{self.checkpoints.pop_front();}
            self.checkpoints.push_back(Checkpoint{
                app_id:event.app_id.clone(),sequence:event.sequence,signal:event.signal
            });
        }
        while self.event_times.front().is_some_and(|t|now.saturating_sub(*t)>=WINDOW){
            self.event_times.pop_front();
        }
        if self.event_times.len()>=120{return Ok(Decision::LocalOnly);}
        self.event_times.push_back(now);
        if event.signal==Signal::Focus{return Ok(Decision::LocalOnly);}
        let rapid=self.last_change.is_some_and(|t|now.saturating_sub(t)<250);
        self.last_change=Some(now);
        if rapid||!model_ready_idle{return Ok(Decision::LocalOnly);}
        while self.reasoning.front().is_some_and(|(t,_)|now.saturating_sub(*t)>=WINDOW){
            self.reasoning.pop_front();
        }
        let tier=if event.signal==Signal::TestFailed{Tier::Deep}else{Tier::Brief};
        let used=self.reasoning.iter().filter(|(_,t)|*t==tier).count();
        let limit=if tier==Tier::Deep{1}else{6};
        if used>=limit{return Ok(Decision::LocalOnly);}
        self.reasoning.push_back((now,tier));
        Ok(Decision::RecommendInference{
            tier,sequence:event.sequence,
            redacted_snapshot_permitted:event.requests_redacted_snapshot
                &&scope.allow_redacted_snapshot,
        })
    }
    /// Proposed learning is inert until independent human review. This does
    /// not write ANSF memory, export samples or train model weights.
    pub fn learning_proposal(&mut self,now:u64)->Result<LearningProposal,Error>{
        self.check_expiry(now)?;
        if self.mode==Mode::Off||!self.learning_review{
            return Err(Error::MissingConsent);
        }
        if self.checkpoints.len()<2||
           !self.checkpoints.iter().any(|x|
               matches!(x.signal,Signal::TestPassed|Signal::TestFailed)){
            return Err(Error::InvalidTransition);
        }
        Ok(LearningProposal{
            session_id:self.scope.as_ref().ok_or(Error::MissingConsent)?.session_id.clone(),
            evidence:self.checkpoints.iter().cloned().collect(),
            human_review_required:true,model_training_authorized:false,
        })
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn grant(shots:bool)->Consent{Consent{
        session_id:"obs-test".into(),app_ids:vec!["lu-gui".into()],
        expires_ms:120_000,allow_redacted_snapshot:shots,
    }}
    fn ev(seq:u64,signal:Signal)->NativeEvent{NativeEvent{
        app_id:"lu-gui".into(),sequence:seq,signal,requests_redacted_snapshot:false,
    }}
    #[test]
    fn consent_scope_and_snapshots_fail_closed(){
        let mut o=Controller::default();
        assert_eq!(o.mode(),Mode::Off);
        assert_eq!(o.ingest(ev(1,Signal::UiChanged),1,true),Err(Error::MissingConsent));
        o.start(grant(false),0).unwrap();
        let mut other=ev(1,Signal::UiChanged);other.app_id="other".into();
        assert_eq!(o.ingest(other,1,true),Err(Error::OutsideApp));
        let mut shot=ev(1,Signal::UiChanged);shot.requests_redacted_snapshot=true;
        assert_eq!(o.ingest(shot,1,true),Err(Error::SnapshotDenied));
        assert!(matches!(o.ingest(ev(1,Signal::UiChanged),300,true).unwrap(),
            Decision::RecommendInference{tier:Tier::Brief,..}));
    }
    #[test]
    fn idle_model_and_bursts_never_force_inference(){
        let mut o=Controller::default();o.start(grant(true),0).unwrap();
        assert_eq!(o.ingest(ev(1,Signal::ErrorIndicator),300,false).unwrap(),
            Decision::LocalOnly);
        let mut shot=ev(2,Signal::ErrorIndicator);shot.requests_redacted_snapshot=true;
        assert!(matches!(o.ingest(shot,600,true).unwrap(),
            Decision::RecommendInference{redacted_snapshot_permitted:true,..}));
        assert_eq!(o.ingest(ev(3,Signal::UiChanged),601,true).unwrap(),
            Decision::LocalOnly);
    }
    #[test]
    fn bounded_brief_deep_rate_and_replay(){
        let mut o=Controller::default();o.start(grant(false),0).unwrap();
        for seq in 1..=8 {
            let d=o.ingest(ev(seq,Signal::ErrorIndicator),seq*300,true).unwrap();
            assert_eq!(matches!(d,Decision::RecommendInference{..}),seq<=6);
        }
        assert_eq!(o.ingest(ev(8,Signal::TestFailed),2700,true),
            Err(Error::Replay));
        assert!(matches!(o.ingest(ev(9,Signal::TestFailed),3000,true).unwrap(),
            Decision::RecommendInference{tier:Tier::Deep,..}));
        assert_eq!(o.ingest(ev(10,Signal::TestFailed),3300,true).unwrap(),
            Decision::LocalOnly);
    }
    #[test]
    fn reviewed_learning_not_equivalent_to_training(){
        let mut o=Controller::default();o.start(grant(false),0).unwrap();
        o.ingest(ev(1,Signal::Checkpoint),300,false).unwrap();
        assert_eq!(o.learning_proposal(400),Err(Error::MissingConsent));
        o.grant_learning_review(true).unwrap();
        assert_eq!(o.learning_proposal(400),Err(Error::InvalidTransition));
        o.ingest(ev(2,Signal::TestPassed),600,false).unwrap();
        let draft=o.learning_proposal(700).unwrap();
        assert_eq!(draft.evidence.len(),2);
        assert!(draft.human_review_required&&!draft.model_training_authorized);
        o.grant_learning_review(false).unwrap();
        assert_eq!(o.learning_proposal(800),Err(Error::MissingConsent));
    }
    #[test]
    fn pause_stop_expiry_revoke_nonpersistent_session(){
        let mut o=Controller::default();o.start(grant(false),0).unwrap();
        o.pause().unwrap();
        assert_eq!(o.ingest(ev(1,Signal::UiChanged),20,true),Err(Error::MissingConsent));
        o.resume(300).unwrap();
        assert_eq!(o.ingest(ev(1,Signal::TestFailed),120_000,true),Err(Error::Expired));
        assert_eq!(o.mode(),Mode::Off);
        o.start(grant(false),1).unwrap();o.stop();
        assert_eq!(o.learning_proposal(2),Err(Error::MissingConsent));
    }
}
