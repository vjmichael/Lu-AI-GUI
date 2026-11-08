//! AGENT-01F stage 1: host-owned K/I/A/F plan admission, DRAFT ONLY.
//! No model, WebView, document or old turn may mint an action grant.
//! Original human goal + user turn are the authoritative input; a proposed
//! step graph is merely data for later human review, NOT an executable plan.
use std::{fs,fs::OpenOptions,path::{Path,PathBuf}};
use rand::{rngs::OsRng,RngCore};
use crate::{chat_store::ChatStore,agent_plan_review::OriginalChatGoalSources};
use lu_core::{
    ansf_goals::project_goal_events,
    agent_plan_ledger::{AgentPlanLedger,PlanStepKind,PlanStepSpec},
    agent_plan_revisions::{AgentPlanRevisionStore,RevisionProposal,
        RevisionReason,RevisionStep,ExpectedDeliverable},
};

#[derive(Clone,Copy,Debug)]
pub struct ProposedStep{
    pub kind:PlanStepKind,
    pub depends_on:Option<u8>,
    pub expected:ExpectedDeliverable,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct DraftPlanReceipt{
    pub plan_id:String,
    pub revision_sha256:String,
    pub review_state:&'static str,
    pub execution_authority:&'static str,
}
fn host_id(prefix:&str)->String{
    let mut bytes=[0u8;16];
    OsRng.fill_bytes(&mut bytes);
    let mut id=String::from(prefix);
    for byte in bytes{
        use std::fmt::Write;
        let _=write!(id,"{byte:02x}");
    }
    id
}
fn original_plan_file(root:&Path)->Result<PathBuf,String>{
    let meta=fs::symlink_metadata(root)
        .map_err(|_|"Host profile unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("Host profile must be a real directory".into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Linked Windows profile refused".into())
        }
    }
    let file=root.join("lu-agent-plans.sqlite");
    match fs::symlink_metadata(&file){
        Ok(_)=>{},
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
            OpenOptions::new().write(true).create_new(true).open(&file)
                .map_err(|_|"Cannot create unique host plan journal")?;
        },
        Err(_)=>return Err("Cannot inspect host plan journal".into()),
    }
    crate::agent_plan_review::existing_plan_file(root)
}
/// Stage a proposal in the host-selected profile only. This is intentionally
/// NOT exported as a model tool or Tauri command: an actual model-turn ingress
/// and idempotent admission protocol remain separate implementation tasks.
pub fn stage_initial_draft(
    profile:&Path,chats:&ChatStore,chat:&str,source_turn:&str,
    steps:&[ProposedStep],
)->Result<DraftPlanReceipt,String>{
    if steps.is_empty()||steps.len()>32||
        steps.iter().enumerate().any(|(i,s)|
            s.depends_on.is_some_and(|p|p as usize>=i)){
        return Err("Invalid bounded plan graph".into())
    }
    if chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(source_turn)
        ||chats.trusted_user_turn_text(chat,source_turn).is_none(){
        return Err("No current original authenticated user turn".into())
    }
    let goal=project_goal_events(&chats.memory_events(chat)?)
        .map_err(|_|"Invalid saved ANSF goal state")?;
    if goal.questions.iter().any(|q|q.status=="pending"){
        return Err("Current goal has unresolved clarification".into())
    }
    let goal_id=goal.goal_id.ok_or("No saved current goal")?;
    let goal_event_id=goal.revision_event_id.ok_or("No saved current goal event")?;
    let path=original_plan_file(profile)?;
    let plan_id=host_id("p-");
    let revision_id=host_id("v-");
    let baseline=steps.iter().map(|s|PlanStepSpec{
        kind:s.kind,depends_on:s.depends_on
    }).collect::<Vec<_>>();
    let ledger=AgentPlanLedger::open(&path)
        .map_err(|e|format!("Cannot open host plan ledger: {e:?}"))?;
    ledger.register(&plan_id,chat,source_turn,&baseline,chats)
        .map_err(|e|format!("Original plan admission refused: {e:?}"))?;
    let proposal=RevisionProposal{
        goal_id,goal_event_id,assumption_sources:Vec::new(),
        reason:RevisionReason::Initial,
        steps:steps.iter().map(|s|RevisionStep{
            kind:s.kind,depends_on:s.depends_on,
            deliverable_id:host_id("d-"),expected:s.expected,
        }).collect()
    };
    let source=OriginalChatGoalSources{chats};
    let revisions=AgentPlanRevisionStore::open(&path)
        .map_err(|e|format!("Cannot inspect plan revisions: {e:?}"))?;
    let recorded=revisions.append_proposal(
        &plan_id,chat,source_turn,&revision_id,None,&proposal,chats,&source)
        .map_err(|e|format!("Original plan revision refused: {e:?}"))?;
    // A failed second phase leaves at most an inert DRAFT (not a retryable
    // effect), and a changed goal fails at the existing revision verifier.
    Ok(DraftPlanReceipt{
        plan_id,revision_sha256:recorded.revision_sha256,
        review_state:"PENDING_HUMAN_REVIEW",
        execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::{
        ansf_goals::{GoalEvent,GoalEventKind},
        agent_plan_ledger::AgentPlanLedger,
        agent_plan_revisions::AgentPlanRevisionStore,
    };
    use std::time::{SystemTime,UNIX_EPOCH};
    fn id(prefix:&str,n:u8)->String{format!("{prefix}{n:032x}")}
    fn setup()->(PathBuf,ChatStore,String,String,GoalEvent){
        let root=std::env::temp_dir().join(format!(
            "lu-agent01f-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let turn=id("g-",1);
        chats.append_user_with_turn_id(&chat,&turn,
            "Make a source-backed technical report").unwrap();
        let goal=GoalEvent{
            event_id:id("m-",3),goal_id:id("goal-",2),question_id:None,
            kind:GoalEventKind::SetGoal,
            text:"Produce a verified technical report".into(),
            choice:None,comment:None
        };
        chats.append_goal_memory_event(&chat,goal.clone()).unwrap();
        (root,chats,chat,turn,goal)
    }
    fn steps()->Vec<ProposedStep>{
        vec![
            ProposedStep{kind:PlanStepKind::CollectSource,depends_on:None,
                expected:ExpectedDeliverable::SourceReferences},
            ProposedStep{kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0),
                expected:ExpectedDeliverable::IndependentWitness},
            ProposedStep{kind:PlanStepKind::Present,depends_on:Some(1),
                expected:ExpectedDeliverable::Explanation},
        ]
    }
    #[test]
    fn real_saved_turn_and_current_goal_stage_only_reviewable_draft(){
        let (root,chats,chat,turn,_)=setup();
        let receipt=stage_initial_draft(&root,&chats,&chat,&turn,&steps()).unwrap();
        assert_eq!(receipt.review_state,"PENDING_HUMAN_REVIEW");
        assert_eq!(receipt.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        let path=root.join("lu-agent-plans.sqlite");
        let plan=AgentPlanLedger::open(&path).unwrap()
            .snapshot(&receipt.plan_id,&chat,&turn).unwrap().unwrap();
        assert_eq!(plan.state,"DRAFT");
        assert_eq!(plan.continuation,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        assert!(reopened.trusted_user_turn_text(&chat,&turn).is_some());
        let revision=AgentPlanRevisionStore::open(&path).unwrap()
            .latest(&receipt.plan_id,&chat).unwrap().unwrap();
        assert_eq!(revision.review_state,"PENDING_HUMAN_REVIEW");
        assert_eq!(revision.revision_sha256,receipt.revision_sha256);
        let _=fs::remove_dir_all(root);
    }
    #[test]
    fn cross_chat_unverified_turn_and_pending_clarification_are_denied(){
        let (root,chats,chat,turn,goal)=setup();
        let other=chats.create_chat().unwrap().chat_id;
        assert!(stage_initial_draft(&root,&chats,&other,&turn,&steps()).is_err());
        assert!(stage_initial_draft(&root,&chats,&chat,&id("g-",99),&steps()).is_err());
        chats.append_goal_memory_event(&chat,GoalEvent{
            event_id:id("m-",4),goal_id:goal.goal_id,
            question_id:Some(id("q-",5)),kind:GoalEventKind::AskQuestion,
            text:"Which evidence source?".into(),choice:None,comment:None
        }).unwrap();
        assert!(stage_initial_draft(&root,&chats,&chat,&turn,&steps()).is_err());
        assert!(!root.join("lu-agent-plans.sqlite").exists());
        let _=fs::remove_dir_all(root);
    }
    #[test]
    fn invalid_steps_and_superseded_human_turn_do_not_write(){
        let (root,chats,chat,turn,_)=setup();
        assert!(stage_initial_draft(&root,&chats,&chat,&turn,&[]).is_err());
        assert!(stage_initial_draft(&root,&chats,&chat,&turn,&[
            ProposedStep{kind:PlanStepKind::Analyze,depends_on:Some(0),
                expected:ExpectedDeliverable::StructuredFindings},
        ]).is_err());
        chats.append_user_with_turn_id(&chat,&id("g-",6),"Correction").unwrap();
        assert!(stage_initial_draft(&root,&chats,&chat,&turn,&steps()).is_err());
        assert!(!root.join("lu-agent-plans.sqlite").exists());
        let _=fs::remove_dir_all(root);
    }
}
