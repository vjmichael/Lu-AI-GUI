//! AGENT-01D→native reviewed broker: non-dispatching exact-claim admission.
 //!
 //! The *existing* ReviewedApplicationTurnSession owns real model/native-tool
 //! registry and original-turn authorization. This bridge checks that an
 //! individual proposed step is still linked to an active (in-process) host
 //! owner and to the fresh user turn whose independent authority grants the
 //! exact reviewed Git/FFprobe read+execute pair. It never opens authority,
 //! selects files, invokes workers, or changes a scheduler state.
 //! All E1 OS/custody gates are still OPEN: this returns only preflight.
 use crate::{
    agent_host_scheduler::AgentHostScheduler,
    agent_plan_ledger::AgentPlanLedger,
    agent_plan_revisions::{AgentPlanRevisionStore,HostPlanReferenceVerifier},
    agent_deliverable_receipts::{AgentDeliverableReceipts,HistoricalDeliverable},
    conversation_authority::{BoundCallOrigin,ConversationAuthority,TrustedUserTurnVerifier},
 };
 #[derive(Clone,Copy,Debug,PartialEq,Eq)]
 pub enum ReviewedBrokerOperation{
    GitHead,GitStatus,GitBranch,GitHistory,MediaFacts,
 }
 impl ReviewedBrokerOperation{
    pub fn native_ids(self)->(&'static str,&'static str){
        match self{
            Self::GitHead=>("lu.reviewed.git.head","lu.reviewed.git.repo-read"),
            Self::GitStatus=>("lu.reviewed.git.status","lu.reviewed.git.repo-read"),
            Self::GitBranch=>("lu.reviewed.git.branch","lu.reviewed.git.repo-read"),
            Self::GitHistory=>("lu.reviewed.git.history","lu.reviewed.git.repo-read"),
            Self::MediaFacts=>("lu.reviewed.media.facts","lu.reviewed.media.file-read"),
        }
    }
 }
 #[derive(Clone,Debug,PartialEq,Eq)]
 pub struct ScheduledBrokerBinding{
    pub claim_id:String,
    pub revision_sha256:String,
    pub step_index:u8,
    pub reviewed_execute_id:&'static str,
    pub reviewed_read_id:&'static str,
    /// A name or preflight is not proof that binaries, resources, OS custody
    /// or independently checked source facts are available on this host.
    pub dispatch_status:&'static str,
 }
 #[derive(Clone,Debug,PartialEq,Eq)]
 pub enum BridgeError{
    NotHostOwned,WrongHumanTurn,StaleRevision,NotProposed,
    NoOriginalAuthority,WrongCapability,UnverifiedSource,
    BrokerNotRegistered,PlanStepNotReviewedOperation,OutcomeNotIndependentlyVerified,
 }
 // AGENT-01F: a reviewed Git/FFprobe read is not a generic plan-step
 // executor. Inspecting sources and independently witnessing a source fact
 // are the only current step classes the broker can serve. A PRESENT or
 // HUMAN_REVIEW step must not silently become a native execution request.
 fn supported_reviewed_step(step:&crate::agent_plan_revisions::RevisionStep)->bool{
    use crate::agent_plan_ledger::PlanStepKind;
    use crate::agent_plan_revisions::ExpectedDeliverable;
    matches!(
        (step.kind,step.expected),
        (PlanStepKind::IndependentlyCheck,ExpectedDeliverable::IndependentWitness)
    ) && !step.deliverable_id.is_empty()
 }
 #[allow(clippy::too_many_arguments)]
 pub fn preflight_reviewed_broker(
    scheduler:&AgentHostScheduler,
    ledger:&AgentPlanLedger,
    revisions:&AgentPlanRevisionStore,
    sources:&dyn HostPlanReferenceVerifier,
    authority:&ConversationAuthority,
    original_plan:&str,chat:&str,origin_turn:&str,
    owner_id:&str,epoch:u64,revision_sha:&str,claim_id:&str,step:u8,
    broker_origin:&BoundCallOrigin,operation:ReviewedBrokerOperation,
 )->Result<ScheduledBrokerBinding,BridgeError>{
    let saved=ledger.snapshot(original_plan,chat,origin_turn)
        .map_err(|_|BridgeError::NotHostOwned)?
        .ok_or(BridgeError::NotHostOwned)?;
    if matches!(saved.state.as_str(),"CANCEL_REQUESTED"|"NEEDS_REVIEW"){
        return Err(BridgeError::NotHostOwned)
    }
    let now=revisions.latest(original_plan,chat)
        .map_err(|_|BridgeError::StaleRevision)?
        .ok_or(BridgeError::StaleRevision)?;
    if now.revision_sha256!=revision_sha||now.review_state!="APPROVED_PLAN_ONLY"{
        return Err(BridgeError::StaleRevision)
    }
    if !now.steps.get(step as usize).is_some_and(supported_reviewed_step){
        return Err(BridgeError::PlanStepNotReviewedOperation)
    }
    if !sources.goal_event_is_current(chat,&now.goal_id,&now.goal_event_id)
        ||now.assumption_sources.iter().any(|x|
            !sources.assumption_source_is_readable(chat,x)){
        return Err(BridgeError::UnverifiedSource)
    }
    if broker_origin.conversation_id!=chat
        ||broker_origin.user_turn_id!=broker_origin.task_id
        ||broker_origin.user_turn_id==origin_turn
        ||Some(broker_origin.user_turn_id.as_str())==now.review_turn_id.as_deref(){
        return Err(BridgeError::WrongHumanTurn)
    }
    scheduler.check_live_claim(
        original_plan,chat,origin_turn,owner_id,epoch,
        revision_sha,claim_id,step,&broker_origin.user_turn_id
    ).map_err(|_|BridgeError::NotHostOwned)?;
    let view=scheduler.view(original_plan,chat,origin_turn,ledger)
        .map_err(|_|BridgeError::NotHostOwned)?
        .ok_or(BridgeError::NotHostOwned)?;
    if view.state!="OWNED"||view.revision_sha!=revision_sha
        ||view.epoch!=1{
        return Err(BridgeError::NotHostOwned)
    }
    let found=view.claims.iter().filter(|row|
        row.step_index==step&&row.revision_sha==revision_sha
        &&row.claim_id==claim_id).collect::<Vec<_>>();
    if found.len()!=1 || found[0].claim_state!="PROPOSED_NO_EXECUTION"{
        return Err(BridgeError::NotProposed)
    }
    if found[0].original_request_turn!=broker_origin.user_turn_id{
        return Err(BridgeError::WrongHumanTurn)
    }
    let (execute_id,read_id)=operation.native_ids();
    let allowed=authority.permitted_tool_ids(broker_origin)
        .map_err(|_|BridgeError::NoOriginalAuthority)?;
    if !allowed.contains(execute_id)||!allowed.contains(read_id){
        return Err(BridgeError::WrongCapability)
    }
    Ok(ScheduledBrokerBinding{
        claim_id:claim_id.into(),revision_sha256:revision_sha.into(),
        step_index:step,reviewed_execute_id:execute_id,reviewed_read_id:read_id,
        dispatch_status:"WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE",
    })
 }
 /// Only an independently verified new worker effect can satisfy a reviewed
 /// plan's expected outcome. Fresh-step original human identity is DISTINCT
 /// from the original plan request, and the plan remains non-executable.
 fn call_matches_exact_binding(call:&str,binding:&ScheduledBrokerBinding)->bool{
     let suffix=match binding.reviewed_execute_id{
         "lu.reviewed.git.head"=>"-gh",
         "lu.reviewed.git.status"=>"-gs",
         "lu.reviewed.git.branch"=>"-gb",
         "lu.reviewed.git.history"=>"-gc",
         "lu.reviewed.media.facts"=>"-fm",
         _=>return false
     };
     call.len()>suffix.len()&&call.len()<=128&&call.ends_with(suffix)
 }
 #[allow(clippy::too_many_arguments)]
 pub fn link_verified_scheduled_worker_outcome(
     scheduler:&AgentHostScheduler,
     ledger:&AgentPlanLedger,
     revisions:&AgentPlanRevisionStore,
     receipts:&AgentDeliverableReceipts,
     sources:&dyn HostPlanReferenceVerifier,
     human:&dyn TrustedUserTurnVerifier,
     profile:&std::path::Path,
     plan:&str,chat:&str,original_turn:&str,owner_id:&str,epoch:u64,
     binding:&ScheduledBrokerBinding,worker_origin:&BoundCallOrigin,
     worker_call_id:&str,
 )->Result<HistoricalDeliverable,BridgeError>{
     if worker_origin.conversation_id!=chat
         ||worker_origin.user_turn_id!=worker_origin.task_id
         ||worker_origin.user_turn_id==original_turn
         ||!call_matches_exact_binding(worker_call_id,binding){
         return Err(BridgeError::WrongHumanTurn)
     }
     receipts.record_verified_execution_turn(
         plan,chat,original_turn,&worker_origin.user_turn_id,
         &binding.revision_sha256,binding.step_index,
         &binding.claim_id,owner_id,epoch,worker_call_id,profile,
         ledger,revisions,sources,scheduler,human
     ).map_err(|_|BridgeError::OutcomeNotIndependentlyVerified)?;
     scheduler.acknowledge_historical_receipt(
         plan,chat,original_turn,owner_id,epoch,
         &binding.revision_sha256,binding.step_index,&binding.claim_id,
         profile,ledger,revisions,receipts,sources
     ).map_err(|_|BridgeError::OutcomeNotIndependentlyVerified)?;
     receipts.history(plan,chat,original_turn,profile,ledger,revisions,sources)
         .map_err(|_|BridgeError::OutcomeNotIndependentlyVerified)?
         .into_iter().find(|item|
             item.revision_sha256==binding.revision_sha256
             &&item.step_index==binding.step_index
             &&item.effect_call_id==worker_call_id
             &&item.tool_id==binding.reviewed_execute_id
             &&item.status=="VERIFIED_AT_OBSERVATION"
         ).ok_or(BridgeError::OutcomeNotIndependentlyVerified)
 }
 /// Prove an independently registered, per-turn *actual* broker session
 /// offers exactly the reviewed tool behind the host's non-dispatching claim.
 /// This method does not call run_journaled_observed.
 pub fn verify_actual_broker_session(
    session:&crate::native_reviewed_app_tool::ReviewedApplicationTurnSession,
    binding:&ScheduledBrokerBinding,
 )->Result<(),BridgeError>{
    let capabilities=session.capabilities()
        .map_err(|_|BridgeError::BrokerNotRegistered)?;
    if !capabilities.iter().any(|row|
        row.native_tool_id==binding.reviewed_execute_id
        &&row.effect=="execute_code"
        &&row.availability_claim=="native_contract_registered_and_turn_granted_only"){
        return Err(BridgeError::BrokerNotRegistered)
    }
    // The read capability is intentionally not model-visible in this
    // registry: the reviewed broker itself checks its host-only read grant
    // at every native worker launch; preflight_reviewed_broker checks it too.
    Ok(())
 }
 #[cfg(test)]
 mod tests{
    #[test]
    fn independent_result_link_requires_the_exact_reviewed_operation(){
        let binding=ScheduledBrokerBinding{
            claim_id:String::from("q-"),revision_sha256:String::from("rev"),
            step_index:0,reviewed_execute_id:"lu.reviewed.git.head",
            reviewed_read_id:"lu.reviewed.git.repo-read",
            dispatch_status:"WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE",
        };
        assert!(call_matches_exact_binding("host-call-gh",&binding));
        assert!(!call_matches_exact_binding("host-call-gs",&binding));
        assert!(!call_matches_exact_binding("host-call-fm",&binding));
    }
    #[test]
    fn current_reviewed_broker_cannot_dispatch_presentation_or_human_review_steps(){
        use super::supported_reviewed_step;
        use crate::agent_plan_ledger::PlanStepKind;
        use crate::agent_plan_revisions::{RevisionStep,ExpectedDeliverable};
        let build=|kind,expected|RevisionStep{
            kind,depends_on:None,deliverable_id:"d-actual".into(),expected
        };
        assert!(!supported_reviewed_step(&build(
            PlanStepKind::CollectSource,ExpectedDeliverable::SourceReferences)));
        assert!(supported_reviewed_step(&build(
            PlanStepKind::IndependentlyCheck,ExpectedDeliverable::IndependentWitness)));
        for kind in [PlanStepKind::Present,PlanStepKind::HumanReview,PlanStepKind::Analyze]{
            assert!(!supported_reviewed_step(&build(
                kind,ExpectedDeliverable::Explanation)));
        }
        assert!(!supported_reviewed_step(&build(
            PlanStepKind::Inspect,ExpectedDeliverable::HumanDecision)));
    }

    use super::*;
    use crate::{
        agent_host_scheduler::{ScheduleBudget,HostFreshStepConsent},
        agent_plan_ledger::{PlanStepKind,PlanStepSpec},
        agent_plan_revisions::{
            RevisionProposal,RevisionReason,RevisionStep,ExpectedDeliverable,
            HostPlanReviewVerifier,HumanDecision,
        },
        conversation_authority::{TaskAuthority,AuthorizedCapability,TrustedUserTurnVerifier},
        contracts::ToolEffect,
    };
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn ident(prefix:&str,n:u64)->String{format!("{prefix}{n:032x}")}
    struct Human;
    impl TrustedUserTurnVerifier for Human{
        fn verify_user_turn(&self,_chat:&str,_turn:&str)->bool{true}
    }
    struct Source(bool);
    impl HostPlanReferenceVerifier for Source{
        fn goal_event_belongs_to_chat(&self,_c:&str,_g:&str,_e:&str)->bool{true}
        fn goal_event_is_current(&self,_c:&str,_g:&str,_e:&str)->bool{self.0}
        fn assumption_source_is_readable(&self,_c:&str,_s:&str)->bool{self.0}
    }
    struct Review;
    impl HostPlanReviewVerifier for Review{
        fn has_exact_human_decision(
            &self,_c:&str,_t:&str,_p:&str,_s:&str,_decision:HumanDecision
        )->bool{true}
    }
    struct Consent;
    impl HostFreshStepConsent for Consent{
        fn explicitly_requested_step(
            &self,_c:&str,_t:&str,_p:&str,_s:&str,
            _i:u8,_b:&ScheduleBudget,
        )->bool{true}
    }
    fn budget()->ScheduleBudget{
        ScheduleBudget{
            max_claims:1,model_token_ceiling:1000,tool_call_ceiling:1,
            wall_clock_ceiling_ms:1000,output_ceiling_bytes:1024
        }
    }
    #[test]
    fn actual_conversation_authority_not_planner_approval_controls_broker_capabilities(){
        let root=std::env::temp_dir().join(format!(
            "lu-agent-broker-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let file=root.join("plans.sqlite");
        let (plan,chat,original,owner,fresh)=(
            ident("p-",1),ident("c-",2),ident("g-",3),
            ident("h-",4),ident("g-",5)
        );
        let ledger=AgentPlanLedger::open(&file).unwrap();
        ledger.register(&plan,&chat,&original,&[
            PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:None}
        ],&Human).unwrap();
        let revisions=AgentPlanRevisionStore::open(&file).unwrap();
        let p=RevisionProposal{
            goal_id:ident("goal-",6),goal_event_id:ident("m-",7),
            assumption_sources:vec![],reason:RevisionReason::Initial,
            steps:vec![RevisionStep{
                kind:PlanStepKind::IndependentlyCheck,depends_on:None,
                deliverable_id:ident("d-",8),
                expected:ExpectedDeliverable::IndependentWitness
            }]
        };
        let rev=revisions.append_proposal(
            &plan,&chat,&original,&ident("v-",9),None,&p,&Human,&Source(true)
        ).unwrap();
        revisions.record_review(
            &plan,&chat,&ident("g-",10),&rev.revision_sha256,
            HumanDecision::Approve,&Human,&Review,&Source(true)
        ).unwrap();
        let scheduler=AgentHostScheduler::open(&file).unwrap();
        scheduler.claim_host(
            &plan,&chat,&original,&owner,&rev.revision_sha256,
            budget(),&ledger,&revisions,&Source(true)
        ).unwrap();
        let claim=ident("q-",11);
        let receipts=crate::agent_deliverable_receipts::
            AgentDeliverableReceipts::open(&file).unwrap();
        scheduler.propose_step(
            &plan,&chat,&original,&owner,1,&rev.revision_sha256,
            0,&claim,&fresh,budget(),&ledger,&revisions,&Source(true),
            &Human,&Consent,&root,&receipts
        ).unwrap();
        let authority=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:chat.clone(),user_turn_id:fresh.clone(),task_id:fresh.clone()
        };
        let pre=||preflight_reviewed_broker(
            &scheduler,&ledger,&revisions,&Source(true),&authority,
            &plan,&chat,&original,&owner,1,&rev.revision_sha256,&claim,0,
            &origin,ReviewedBrokerOperation::GitHead
        );
        assert_eq!(pre(),Err(BridgeError::NoOriginalAuthority));
        authority.open_user_task(TaskAuthority{
            conversation_id:chat.clone(),user_turn_id:fresh.clone(),
            task_id:fresh.clone(),execution_scope_id:fresh.clone(),
            capabilities:vec![
                AuthorizedCapability{
                    tool_id:"lu.reviewed.git.head".into(),
                    effect:ToolEffect::ExecuteCode
                },
                AuthorizedCapability{
                    tool_id:"lu.reviewed.git.repo-read".into(),
                    effect:ToolEffect::ReadWorkspace
                }
            ],expires_at:None,
        },&Human).unwrap();
        assert_eq!(pre().unwrap().dispatch_status,
            "WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE");
        assert_eq!(preflight_reviewed_broker(
            &scheduler,&ledger,&revisions,&Source(true),&authority,
            &plan,&chat,&original,&owner,1,&rev.revision_sha256,&claim,0,
            &origin,ReviewedBrokerOperation::MediaFacts
        ),Err(BridgeError::WrongCapability));
        assert_eq!(preflight_reviewed_broker(
            &scheduler,&ledger,&revisions,&Source(false),&authority,
            &plan,&chat,&original,&owner,1,&rev.revision_sha256,&claim,0,
            &origin,ReviewedBrokerOperation::GitHead
        ),Err(BridgeError::UnverifiedSource));
        assert_eq!(preflight_reviewed_broker(
            &scheduler,&ledger,&revisions,&Source(true),&authority,
            &plan,&chat,&original,&owner,1,&rev.revision_sha256,&claim,0,
            &BoundCallOrigin{
                conversation_id:chat.clone(),
                user_turn_id:ident("g-",99),task_id:ident("g-",99)
            },ReviewedBrokerOperation::GitHead
        ),Err(BridgeError::NotHostOwned));
        // A fresh scheduler object has the same SQLite row but no live
        // original-owner fence. It cannot bind a broker after restart.
        let reopened=AgentHostScheduler::open(&file).unwrap();
        assert_eq!(preflight_reviewed_broker(
            &reopened,&ledger,&revisions,&Source(true),&authority,
            &plan,&chat,&original,&owner,1,&rev.revision_sha256,&claim,0,
            &origin,ReviewedBrokerOperation::GitHead
        ),Err(BridgeError::NotHostOwned));
        drop(reopened);
        authority.revoke_task(&fresh).unwrap();
        assert_eq!(pre(),Err(BridgeError::WrongCapability));
        drop(receipts);drop(scheduler);drop(revisions);drop(ledger);
        fs::remove_dir_all(root).unwrap();
    }
 }
