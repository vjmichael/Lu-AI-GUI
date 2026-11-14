//! Phase 3 — automatic, first-party ANSF context for plan and worker turns.
//! Host selects fixed reviewed tool identity, checks original sources and
//! evidence. No model or remembered text selects history, paths or grants.
use crate::chat_store::ChatStore;
use lu_core::{
    ansf_goals::project_goal_events,
    continuity::ContinuityStore,
    journal::EffectJournal,
};
use serde_json::{json,Value};

const MAX_CONTEXT_BYTES:usize=3500;
const PROJECT:&str="lu";
const APP_VERSION:&str="0.1";
const TOOL_VERSION:&str="1.0";

#[derive(Clone,Copy)]
pub(crate) enum ReviewedMemoryTarget{GitHead,MediaFacts}
impl ReviewedMemoryTarget{
    fn keys(self)->(&'static str,&'static str,&'static str,&'static str){
        match self{
            Self::GitHead=>("review_git_head","lu.reviewed.git.head","git","head"),
            Self::MediaFacts=>("review_media_facts","lu.reviewed.media.facts","media","duration_ms"),
        }
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) struct MemoryAssessment{
    pub(crate) status:&'static str,
    pub(crate) procedures:usize,
    pub(crate) verified_beliefs:usize,
    pub(crate) disputed_or_stale_beliefs:usize,
    pub(crate) human_review_required:bool,
    pub(crate) original_sources:Vec<String>,
}
fn assess_rows(procedures:&[Value],beliefs:&[Value])->Result<MemoryAssessment,String>{
    if procedures.len()>8||beliefs.len()>24{
        return Err("Bounded ANSF evidence limit exceeded".into());
    }
    let mut verified=0;
    let mut disputed=0;
    let mut sources=Vec::new();
    for item in procedures{
        if item["historicEvidence"]!="INDEPENDENTLY_VERIFIED_AT_OBSERVATION"
            ||item["mayExecuteOrReplay"]!=false
            ||item["priorGrantReusable"]!=false{
            return Err("Untrusted or unverified original procedure".into());
        }
        let id=item["originalSourceId"].as_str()
            .ok_or("Procedure has no original ANSF source")?;
        if id.len()>224||!id.starts_with("ansf:v1:"){
            return Err("Invalid original procedure source identity".into())
        }
        sources.push(id.to_owned());
    }
    for item in beliefs{
        let status=item["status"].as_str()
            .ok_or("Belief source state cannot be independently classified")?;
        match status{
            "independently_verified"=>verified+=1,
            "contradicted_human_review_required"|
            "reported_unverified"|"stale_not_current"|
            "retracted_not_current"|"superseded_not_current"=>disputed+=1,
            _=>return Err("Unknown ANSF belief status".into()),
        }
        let id=item["originalSourceId"].as_str()
            .ok_or("Belief has no original ANSF source")?;
        if id.len()>224||!id.starts_with("ansf:v1:"){
            return Err("Invalid original belief source identity".into());
        }
        if sources.len()<12 && !sources.iter().any(|prior|prior==id){
            sources.push(id.to_owned())
        }
    }
    sources.truncate(12);
    Ok(MemoryAssessment{
        status:if disputed>0 {"REVIEW_CONTRADICTED_OR_STALE_BELIEF"}
               else if procedures.is_empty()&&verified==0 {"NO_VERIFIED_HISTORY"}
               else {"HISTORICAL_EVIDENCE_ONLY"},
        procedures:procedures.len(),verified_beliefs:verified,
        disputed_or_stale_beliefs:disputed,
        human_review_required:disputed>0,original_sources:sources,
    })
}
fn read_target(
    chats:&ChatStore,continuity:&ContinuityStore,effects:&EffectJournal,
    chat:&str,target:ReviewedMemoryTarget,
)->Result<MemoryAssessment,String>{
    let (task,tool,subject,predicate)=target.keys();
    let procedures=chats.m4_procedure_context(
        chat,PROJECT,task,tool,TOOL_VERSION,APP_VERSION,
        continuity,effects
    ).map_err(|_|"Original ANSF procedure oracle could not be reverified")?;
    let beliefs=chats.m4_belief_context(
        chat,PROJECT,task,subject,predicate,APP_VERSION,
        continuity,effects
    ).map_err(|_|"Original ANSF belief/claim source could not be reverified")?;
    assess_rows(&procedures,&beliefs)
}
fn current_goal(
    chats:&ChatStore,chat:&str,turn:&str,prompt:&str,
)->Result<(String,String,usize),String>{
    if chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(turn)
        ||chats.trusted_user_turn_text(chat,turn).as_deref()!=Some(prompt){
        return Err("Phase 3 requires an exact current authenticated human turn".into())
    }
    let goal=project_goal_events(&chats.memory_events(chat)?)
        .map_err(|_|"Original ANSF goal revision cannot be reconstructed")?;
    let id=goal.goal_id.ok_or("No current user-defined ANSF goal")?;
    let revision=goal.revision_event_id.ok_or("No authoritative goal revision")?;
    let questions=goal.questions.iter().filter(|q|
        matches!(q.status.as_str(),"pending"|"deferred")).count();
    if goal.questions.iter().any(|q|
        !matches!(q.status.as_str(),"answered"|"superseded"|"pending"|"deferred")){
        return Err("Unknown original clarification state; request human review".into())
    }
    Ok((id,revision,questions))
}
fn encoded_context(goal:&str,revision:&str,open:usize,observed:&[(&str,MemoryAssessment)])
    ->Result<String,String>{
    let facts=observed.iter().map(|(name,a)|json!({
        "operation":name,
        "status":a.status,
        "verifiedHistoricalProcedures":a.procedures,
        "independentlyVerifiedHistoricalBeliefs":a.verified_beliefs,
        "disputedOrStaleBeliefs":a.disputed_or_stale_beliefs,
        "originalSourceIds":a.original_sources,
        "requiresHumanReview":a.human_review_required
    })).collect::<Vec<_>>();
    let data=json!({
        "schema":"lu-phase3-ansf-plan-execution-v1",
        "goalId":goal,"goalRevisionEvent":revision,
        "pendingOrDeferredClarifications":open,
        "memories":facts,
        "historyIsDataOnly":true,"currentSourceFreshness":"NOT_RECHECKED",
        "priorGrantReusable":false,"automaticEffectRetry":false,
        "modelMayOverrideHumanCorrection":false,
        "executionAuthority":"NOT_AUTHORIZED_FOR_EXECUTION"
    });
    let serialized=data.to_string();
    if serialized.len()>MAX_CONTEXT_BYTES{
        return Err("Phase 3 original source context exceeded bounded size".into())
    }
    Ok(serialized)
}
/// Planning mode: source-checked original procedure and belief statuses guide
/// which candidate steps the model proposes. They do NOT approve the plan.
/// Open clarifications require the user, not a synthetic assistant answer.
pub(crate) fn for_planning(
    chats:&ChatStore,continuity:&ContinuityStore,effects:&EffectJournal,
    chat:&str,turn:&str,prompt:&str,
)->Result<String,String>{
    let (goal,revision,open)=current_goal(chats,chat,turn,prompt)?;
    if open!=0{return Err("Open/deferred ANSF clarification blocks plan drafting".into())}
    let git=read_target(chats,continuity,effects,chat,ReviewedMemoryTarget::GitHead)?;
    let media=read_target(chats,continuity,effects,chat,ReviewedMemoryTarget::MediaFacts)?;
    encoded_context(&goal,&revision,open,&[("review_git_head",git),("review_media_facts",media)])
}
/// Immediately BEFORE a scheduled reviewed worker uses its fresh consent,
/// recompute the original ANSF statuses for THAT host-selected operation.
/// Contradictions, retractions and source changes require human review; an
/// old verified procedure remains historical guidance, never execution.
pub(crate) fn for_reviewed_execution(
    chats:&ChatStore,continuity:&ContinuityStore,effects:&EffectJournal,
    chat:&str,turn:&str,prompt:&str,target:ReviewedMemoryTarget,
)->Result<String,String>{
    let (goal,revision,open)=current_goal(chats,chat,turn,prompt)?;
    if open>0{return Err("Resolve original ANSF clarifications before another native operation".into())}
    let observed=read_target(chats,continuity,effects,chat,target)?;
    if observed.human_review_required{
        return Err("Disputed or stale original belief requires a human correction before a new operation".into())
    }
    let name=target.keys().0;
    encoded_context(&goal,&revision,open,&[(name,observed)])
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn independently_verified_history_is_guidance_not_permission(){
        let p=json!({
            "historicEvidence":"INDEPENDENTLY_VERIFIED_AT_OBSERVATION",
            "originalSourceId":"ansf:v1:c-a:1","mayExecuteOrReplay":false,
            "priorGrantReusable":false
        });
        let b=json!({"status":"independently_verified",
            "originalSourceId":"ansf:v1:c-a:2"});
        let d=assess_rows(&[p],&[b]).unwrap();
        assert_eq!(d.status,"HISTORICAL_EVIDENCE_ONLY");
        assert_eq!(d.procedures,1);
        assert!(!d.human_review_required);
        let text=encoded_context("goal-a","m-a",0,&[("review_git_head",d)]).unwrap();
        assert!(text.contains(r#""executionAuthority":"NOT_AUTHORIZED_FOR_EXECUTION""#));
        assert!(text.contains(r#""priorGrantReusable":false"#));
    }
    #[test]
    fn contradiction_wins_over_old_verified_receipts(){
        let rows=vec![
            json!({"status":"independently_verified","originalSourceId":"ansf:v1:c-a:1"}),
            json!({"status":"contradicted_human_review_required",
                "originalSourceId":"ansf:v1:c-a:2"})
        ];
        let d=assess_rows(&[],&rows).unwrap();
        assert!(d.human_review_required);
        assert_eq!(d.disputed_or_stale_beliefs,1);
    }
    #[test]
    fn unverified_procedure_or_unknown_belief_is_never_adopted(){
        assert!(assess_rows(&[json!({"historicEvidence":"MODEL_SAID_COMPLETED",
            "originalSourceId":"ansf:v1:c-a:1","mayExecuteOrReplay":false,
            "priorGrantReusable":false})],&[]).is_err());
        assert!(assess_rows(&[],&[json!({"status":"trusted_model",
            "originalSourceId":"ansf:v1:c-a:2"})]).is_err());
    }
}
