//! Reviewed Observe & Learn publication into the existing ANSF store.
//! This is NOT automatic learning, training, or a second memory database.
//! Caller must be a trusted native human-review endpoint that independently
//! verifies the source test receipt and conversation ownership.
use crate::{
    ansf::{AnsfError,AnsfStore,EntryRole},
    native_observe_learn::{LearningProposal,Signal},
    tasks::{TaskRecord,TaskState},
};

#[derive(Debug,PartialEq,Eq)]
pub enum ObserveMemoryError {
    NotReviewed,
    InvalidEvidence,
    AlreadyPublished,
    Store(AnsfError),
}
impl From<AnsfError> for ObserveMemoryError {
    fn from(e:AnsfError)->Self{Self::Store(e)}
}
fn safe(s:&str)->bool{
    !s.is_empty()&&s.len()<=100&&
        s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
/// Trust boundary: this function is NOT callable by a model/renderer. A
/// real host must verify native GUI test receipts and require separate
/// explicit human approval before this call. It only archives enumerated
/// trusted outcomes; never screenshots, UI text, secrets or model weights.
pub(crate) fn publish_reviewed_test(
    ansf:&AnsfStore,
    task:&TaskRecord,
    chat_id:&str,
    proposal:&LearningProposal,
    human_approved:bool,
)->Result<i64,ObserveMemoryError>{
    if !human_approved || !proposal.human_review_required
       ||proposal.model_training_authorized{
        return Err(ObserveMemoryError::NotReviewed);
    }
    if !safe(chat_id)||!chat_id.starts_with("c-")||
       !safe(&proposal.session_id)||!proposal.session_id.starts_with("obs-")||
       task.task_id!=proposal.session_id||
       task.workspace_id.as_deref()!=Some(chat_id)||
       !matches!(task.state,TaskState::Running|TaskState::Paused|TaskState::Cancelled)||
       !(2..=32).contains(&proposal.evidence.len()){
        return Err(ObserveMemoryError::InvalidEvidence);
    }
    let mut seen_result=false;
    let mut previous_seq=0;
    for e in &proposal.evidence{
        if e.app_id!="lu-desktop"||e.sequence<=previous_seq||
            !matches!(e.signal,Signal::Checkpoint|Signal::TestPassed|Signal::TestFailed){
            return Err(ObserveMemoryError::InvalidEvidence);
        }
        if matches!(e.signal,Signal::TestPassed|Signal::TestFailed){seen_result=true;}
        previous_seq=e.sequence;
    }
    if !seen_result{return Err(ObserveMemoryError::InvalidEvidence);}
    let session_id=format!("observe-review-{}",proposal.session_id);
    let message_id=format!("approved-{}",proposal.session_id);
    // No new persistent store is introduced; ANSF is the existing memory
    // layer. Replays are rejected before starting another chapter.
    ansf.ensure_session(&session_id,chat_id,"Approved Lu GUI test evidence")?;
    if ansf.find_message(&session_id,&message_id)?.is_some(){
        return Err(ObserveMemoryError::AlreadyPublished);
    }
    ansf.start_chapter(&session_id,"Reviewed GUI test")?;
    let mut content=String::from(
        "Host-reviewed observation evidence only; not a screenshot, \
         UI automation grant, transcript or model training record.\n");
    for e in &proposal.evidence{
        let kind=match e.signal{
            Signal::Checkpoint=>"CHECKPOINT",
            Signal::TestPassed=>"TEST_PASSED",
            Signal::TestFailed=>"TEST_FAILED",
            _=>return Err(ObserveMemoryError::InvalidEvidence),
        };
        content.push_str(&format!("app=lu-desktop; sequence={}; outcome={kind}\n",e.sequence));
    }
    let id=ansf.append_entry(&session_id,&message_id,EntryRole::System,
        "Human-reviewed Lu GUI test evidence",&content)?;
    ansf.close_chapter(&session_id,"Reviewed native GUI test evidence")?;
    Ok(id)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        native_observe_learn::{Checkpoint,LearningProposal},
        tasks::TaskStore,
    };
    use std::path::Path;
    fn proposal()->LearningProposal{
        LearningProposal{
            session_id:"obs-scope".into(),
            evidence:vec![
                Checkpoint{app_id:"lu-desktop".into(),sequence:1,signal:Signal::Checkpoint},
                Checkpoint{app_id:"lu-desktop".into(),sequence:2,signal:Signal::TestPassed},
            ],
            human_review_required:true,model_training_authorized:false,
        }
    }
    #[test]
    fn reviewed_verified_outcome_uses_existing_ansf_and_rejects_replay(){
        let tasks=TaskStore::open(Path::new(":memory:")).unwrap();
        let ansf=AnsfStore::in_memory().unwrap();
        tasks.create("obs-scope",Some("c-owner"),None).unwrap();
        tasks.start("obs-scope").unwrap();
        let task=tasks.get("obs-scope").unwrap().unwrap();
        assert_eq!(publish_reviewed_test(&ansf,&task,"c-owner",&proposal(),false),
            Err(ObserveMemoryError::NotReviewed));
        assert_eq!(publish_reviewed_test(&ansf,&task,"c-other",&proposal(),true),
            Err(ObserveMemoryError::InvalidEvidence));
        let id=publish_reviewed_test(&ansf,&task,"c-owner",&proposal(),true).unwrap();
        let stored=ansf.read_entry("observe-review-obs-scope",id).unwrap().unwrap();
        assert!(stored.content.contains("TEST_PASSED"));
        assert!(!stored.content.contains("screenshots"));
        assert_eq!(publish_reviewed_test(&ansf,&task,"c-owner",&proposal(),true),
            Err(ObserveMemoryError::AlreadyPublished));
    }
    #[test]
    fn unverified_renderer_events_cannot_be_promoted_to_memory(){
        let tasks=TaskStore::open(Path::new(":memory:")).unwrap();
        let ansf=AnsfStore::in_memory().unwrap();
        tasks.create("obs-scope",Some("c-owner"),None).unwrap();
        tasks.start("obs-scope").unwrap();
        let record=tasks.get("obs-scope").unwrap().unwrap();
        let mut p=proposal();p.evidence[1].signal=Signal::ErrorIndicator;
        assert_eq!(publish_reviewed_test(&ansf,&record,"c-owner",&p,true),
            Err(ObserveMemoryError::InvalidEvidence));
        let mut p=proposal();p.model_training_authorized=true;
        assert_eq!(publish_reviewed_test(&ansf,&record,"c-owner",&p,true),
            Err(ObserveMemoryError::NotReviewed));
        let mut p=proposal();p.evidence[1].sequence=1;
        assert_eq!(publish_reviewed_test(&ansf,&record,"c-owner",&p,true),
            Err(ObserveMemoryError::InvalidEvidence));
    }
}
