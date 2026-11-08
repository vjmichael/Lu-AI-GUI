//! AGENT-01C — host-only observed, independently cross-checked deliverables.
 //!
 //! A journal APPLIED state is not a successful result, a surviving artifact,
 //! or proof that a descendant process was reaped. A VERIFIED independent
 //! observation is historical and cannot grant new execution permission.
 //! This module never runs a tool, parses model assertions, or resumes work.
 use std::{fs,path::{Path,PathBuf},sync::Mutex};
 use rusqlite::{params,Connection,OpenFlags,OptionalExtension,TransactionBehavior};
 use sha2::{Digest,Sha256};
 use crate::{
     agent_plan_ledger::{AgentPlanLedger,PlanStepKind},
     agent_plan_revisions::{AgentPlanRevisionStore,ExpectedDeliverable},
 };
 const MAX_JOURNAL:u64=64*1024*1024;
 const MAX_RECEIPT:usize=32*1024;
 const MAX_WITNESSES:usize=32;
 #[derive(Clone,Debug,PartialEq,Eq)]
 pub enum DeliverableError{
     Database(String),Poisoned,InvalidId,WrongOrigin,UnapprovedPlan,
     StaleRevision,UnsupportedDeliverable,UnverifiedEffect,
     MissingJournal,InvalidJournal,ConflictingReceipt,LimitReached,
 }
 impl From<rusqlite::Error> for DeliverableError{
     fn from(e:rusqlite::Error)->Self{Self::Database(e.to_string())}
 }
 #[derive(Clone,Copy,Debug,PartialEq,Eq)]
 pub enum ReceiptWrite{New,AlreadyRecorded}
 #[derive(Clone,Debug,PartialEq,Eq)]
 pub struct HistoricalDeliverable{
     pub revision_sha256:String,
     pub step_index:u8,
     pub deliverable_id:String,
     pub tool_id:String,
     pub effect_call_id:String,
     pub receipt_sha256:String,
     pub independent_method:String,
     pub observed_sha256:String,
     pub status:&'static str,
     /// Even when an observation is verified, the scheduler is not authorized.
     pub continuation:&'static str,
 }
 pub struct AgentDeliverableReceipts{db:Mutex<Connection>}
 fn exact(id:&str,prefix:&str)->bool{
     id.len()==prefix.len()+32&&id.starts_with(prefix)
         &&id[prefix.len()..].bytes().all(|b|
             b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
 }
 fn sha_ok(value:&str)->bool{
     value.len()==64&&value.bytes().all(|b|
         b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
 }
 fn worker_operation(call:&str)->Option<(&str,&'static str,&'static str)>{
     const ALLOWED:[(&str,&str,&str);5]=[
         ("-gh","lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"),
         ("-gs","lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
         ("-gb","lu.reviewed.git.branch","GIT_BRANCH_DIRECT_V1"),
         ("-gc","lu.reviewed.git.history","GIT_HISTORY_LOOSE_DAG_V1"),
         ("-fm","lu.reviewed.media.facts","WAV_PCM_DIRECT_V1"),
     ];
     for (suffix,tool,method) in ALLOWED{
         if let Some(outer)=call.strip_suffix(suffix){
             if !outer.is_empty()&&outer.len()<=115{
                 return Some((outer,tool,method))
             }
         }
     }
     None
 }
 fn method_matches(tool:&str,method:&str)->bool{
     match tool{
         "lu.reviewed.git.head"=>method=="GIT_HEAD_DIRECT_V1",
         "lu.reviewed.git.status"=>method=="GIT_STATUS_INDEX_TREE_V1",
         "lu.reviewed.git.branch"=>method=="GIT_BRANCH_DIRECT_V1",
         "lu.reviewed.git.history"=>matches!(method,
             "GIT_HISTORY_LOOSE_DAG_V1"|"GIT_HISTORY_OBJECT_DAG_V2"),
         "lu.reviewed.media.facts"=>matches!(method,
             "WAV_PCM_DIRECT_V1"|"MP4_BOXES_DIRECT_V1"),
         _=>false,
     }
 }
 fn safe_file(path:&Path)->Result<bool,DeliverableError>{
     let metadata=match fs::symlink_metadata(path){
         Ok(meta)=>meta,
         Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
         Err(_)=>return Err(DeliverableError::InvalidJournal),
     };
     if !metadata.is_file()||metadata.file_type().is_symlink()
         ||metadata.len()>MAX_JOURNAL{
         return Err(DeliverableError::InvalidJournal)
     }
     #[cfg(windows)]
     {
         use std::os::windows::fs::MetadataExt;
         if metadata.file_attributes()&0x400!=0{
             return Err(DeliverableError::InvalidJournal)
         }
     }
     Ok(true)
 }
 fn readonly(path:&Path)->Result<Connection,DeliverableError>{
     if !safe_file(path)?{
         return Err(DeliverableError::MissingJournal)
     }
     for suffix in ["-wal","-shm"]{
         let mut other=path.as_os_str().to_os_string();
         other.push(suffix);
         let _=safe_file(&PathBuf::from(other))?;
     }
     Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY
         |OpenFlags::SQLITE_OPEN_NO_MUTEX)
         .map_err(|_|DeliverableError::InvalidJournal)
 }
 #[derive(Clone,Debug)]
 struct VerifiedObservation{
     tool_id:String,receipt_sha256:String,method:String,observed_sha256:String
 }
 fn observe_existing(turn:&str,call:&str,profile:&Path)
     ->Result<VerifiedObservation,DeliverableError>{
     let Some((outer,expected_tool,_))=worker_operation(call) else{
         return Err(DeliverableError::InvalidId)
     };
     // These filenames are derived only from the already persisted human turn.
     // The host selects the profile directory; model data cannot select paths.
     let effects=readonly(&profile.join(format!("lu-reviewed-effects-{turn}.sqlite")))?;
     let tasks=readonly(&profile.join(format!("lu-reviewed-task-{turn}.sqlite")))?;
     let get_effect=|id:&str|->Result<(String,String,Option<Vec<u8>>),DeliverableError>{
         effects.query_row(
             "SELECT tool_id,state,
                CASE WHEN LENGTH(receipt) BETWEEN 1 AND 32768
                     THEN receipt ELSE NULL END
              FROM lu_effects WHERE call_id=?1",params![id],
             |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
         ).optional()?.ok_or(DeliverableError::UnverifiedEffect)
     };
     let (outer_tool,outer_state,_)=get_effect(outer)?;
     if outer_tool!=expected_tool||outer_state!="APPLIED"{
         return Err(DeliverableError::UnverifiedEffect)
     }
     let (inner_tool,state,receipt)=get_effect(call)?;
     if inner_tool!=expected_tool||state!="APPLIED"{
         return Err(DeliverableError::UnverifiedEffect)
     }
     let Some(receipt)=receipt else{
         return Err(DeliverableError::UnverifiedEffect)
     };
     if receipt.is_empty()||receipt.len()>MAX_RECEIPT{
         return Err(DeliverableError::UnverifiedEffect)
     }
     let task:Option<(String,Option<String>)>=tasks.query_row(
         "SELECT state,workspace_id FROM lu_tasks WHERE task_id=?1",
         params![call],|r|Ok((r.get(0)?,r.get(1)?))
     ).optional()?;
     if task!=Some(("COMPLETED".into(),Some(turn.into()))){
         return Err(DeliverableError::UnverifiedEffect)
     }
     let independent:Option<(String,String,String,Option<String>)>=
         effects.query_row(
             "SELECT receipt_sha256,verdict,method,observed_sha256
              FROM lu_independent_outcomes WHERE call_id=?1",
             params![call],|r|Ok((
                 r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?
             ))
         ).optional()?;
     let Some((digest,verdict,method,observed))=independent else{
         return Err(DeliverableError::UnverifiedEffect)
     };
     let Some(observed)=observed else{
         return Err(DeliverableError::UnverifiedEffect)
     };
     if verdict!="VERIFIED"||!method_matches(&inner_tool,&method)
         ||!sha_ok(&digest)||!sha_ok(&observed)
         ||digest!=format!("{:x}",Sha256::digest(&receipt)){
         return Err(DeliverableError::UnverifiedEffect)
     }
     Ok(VerifiedObservation{
         tool_id:inner_tool,receipt_sha256:digest,
         method,observed_sha256:observed
     })
 }
 impl AgentDeliverableReceipts{
     /// The caller owns the plan database path. Existing revision plans are
     /// checked through production AGENT-01A/B APIs before any record is made.
     pub fn open(plan_db:&Path)->Result<Self,DeliverableError>{
         let db=Connection::open(plan_db)?;
         db.pragma_update(None,"foreign_keys","ON")?;
         db.pragma_update(None,"journal_mode","WAL")?;
         db.pragma_update(None,"synchronous","FULL")?;
         db.execute_batch(
             "CREATE TABLE IF NOT EXISTS lu_agent_deliverable_receipts(
               plan_id TEXT NOT NULL REFERENCES lu_agent_plans(plan_id),
               revision_sha256 TEXT NOT NULL,
               step_index INTEGER NOT NULL CHECK(step_index BETWEEN 0 AND 31),
               deliverable_id TEXT NOT NULL,
               original_turn_id TEXT NOT NULL,
               effect_call_id TEXT NOT NULL,
               tool_id TEXT NOT NULL,
               receipt_sha256 TEXT NOT NULL CHECK(length(receipt_sha256)=64),
               independent_method TEXT NOT NULL,
               observed_sha256 TEXT NOT NULL CHECK(length(observed_sha256)=64),
               recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(plan_id,revision_sha256,step_index),
               UNIQUE(original_turn_id,effect_call_id)
              );
              CREATE TRIGGER IF NOT EXISTS lu_agent_deliverable_no_update
                BEFORE UPDATE ON lu_agent_deliverable_receipts
                BEGIN SELECT RAISE(ABORT,'Immutable deliverable witness'); END;
              CREATE TRIGGER IF NOT EXISTS lu_agent_deliverable_no_delete
                BEFORE DELETE ON lu_agent_deliverable_receipts
                BEGIN SELECT RAISE(ABORT,'Immutable deliverable witness'); END;"
         )?;
         Ok(Self{db:Mutex::new(db)})
     }
     /// Attach only a historical independent OS-effect observation. The host
     /// supplies the profile path, not the model or an arbitrary WebView call.
     /// This does not mark a plan step completed or continue a task.
     #[allow(clippy::too_many_arguments)]
     pub fn record_observed(
         &self,plan:&str,chat:&str,turn:&str,revision_sha:&str,
         step_index:u8,call_id:&str,profile:&Path,
         ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
     )->Result<ReceiptWrite,DeliverableError>{
         if !exact(plan,"p-")||!exact(chat,"c-")||!exact(turn,"g-")
             ||!sha_ok(revision_sha)||call_id.len()>128||call_id.is_empty(){
             return Err(DeliverableError::InvalidId)
         }
         let plan_view=ledger.snapshot(plan,chat,turn)
             .map_err(|_|DeliverableError::WrongOrigin)?
             .ok_or(DeliverableError::WrongOrigin)?;
         if matches!(plan_view.state.as_str(),"CANCEL_REQUESTED"|"NEEDS_REVIEW"){
             return Err(DeliverableError::UnapprovedPlan)
         }
         let rev=revisions.latest(plan,chat)
             .map_err(|_|DeliverableError::WrongOrigin)?
             .ok_or(DeliverableError::UnapprovedPlan)?;
         if rev.revision_sha256!=revision_sha{
             return Err(DeliverableError::StaleRevision)
         }
         if rev.review_state!="APPROVED_PLAN_ONLY"{
             return Err(DeliverableError::UnapprovedPlan)
         }
         let step=rev.steps.get(step_index as usize)
             .ok_or(DeliverableError::UnsupportedDeliverable)?;
         if step.expected!=ExpectedDeliverable::IndependentWitness
             ||step.kind!=PlanStepKind::IndependentlyCheck{
             return Err(DeliverableError::UnsupportedDeliverable)
         }
         if step_index as usize>=plan_view.steps.len(){
             return Err(DeliverableError::UnsupportedDeliverable)
         }
         let observed=observe_existing(turn,call_id,profile)?;
         let mut db=self.db.lock().map_err(|_|DeliverableError::Poisoned)?;
         let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
         // Recheck the exact parent revision still exists at this commit
         // point. The host must exclude concurrent plan owners at deployment.
         let current:Option<String>=tx.query_row(
             "SELECT revision_sha256 FROM lu_agent_plan_revisions
              WHERE plan_id=?1 ORDER BY revision_index DESC LIMIT 1",
             params![plan],|r|r.get(0)
         ).optional()?;
         if current.as_deref()!=Some(revision_sha){
             return Err(DeliverableError::StaleRevision)
         }
         let existing:Option<(String,String,String,String,String,String,String)>=
             tx.query_row(
                 "SELECT deliverable_id,original_turn_id,effect_call_id,tool_id,
                         receipt_sha256,independent_method,observed_sha256
                  FROM lu_agent_deliverable_receipts
                  WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
                 params![plan,revision_sha,step_index],
                 |r|Ok((
                     r.get(0)?,r.get(1)?,r.get(2)?,
                     r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?
                 ))
             ).optional()?;
         if let Some((deliverable,original,effect,tool,receipt,method,observed_hash))=existing{
             if deliverable!=step.deliverable_id||original!=turn||effect!=call_id
                 ||tool!=observed.tool_id||receipt!=observed.receipt_sha256
                 ||method!=observed.method||observed_hash!=observed.observed_sha256{
                 return Err(DeliverableError::ConflictingReceipt)
             }
             return Ok(ReceiptWrite::AlreadyRecorded)
         }
         let count:i64=tx.query_row(
             "SELECT COUNT(*) FROM lu_agent_deliverable_receipts
              WHERE plan_id=?1",params![plan],|r|r.get(0)
         )?;
         if count as usize>=MAX_WITNESSES{return Err(DeliverableError::LimitReached)}
         let result=tx.execute(
             "INSERT INTO lu_agent_deliverable_receipts(
               plan_id,revision_sha256,step_index,deliverable_id,
               original_turn_id,effect_call_id,tool_id,receipt_sha256,
               independent_method,observed_sha256)
              VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
             params![plan,revision_sha,step_index,
                 step.deliverable_id,turn,call_id,observed.tool_id,
                 observed.receipt_sha256,observed.method,observed.observed_sha256]
         );
         if result.is_err(){return Err(DeliverableError::ConflictingReceipt)}
         tx.commit()?;
         Ok(ReceiptWrite::New)
     }
     /// Historical evidence is re-read from original task/effect journals,
     /// never inferred solely from this derived table. A missing or changed
     /// oracle makes the display NEEDS_RECONCILIATION, not successful work.
     pub fn history(
         &self,plan:&str,chat:&str,turn:&str,profile:&Path,
         ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
     )->Result<Vec<HistoricalDeliverable>,DeliverableError>{
         if !exact(plan,"p-")||!exact(chat,"c-")||!exact(turn,"g-"){
             return Err(DeliverableError::InvalidId)
         }
         ledger.snapshot(plan,chat,turn)
             .map_err(|_|DeliverableError::WrongOrigin)?
             .ok_or(DeliverableError::WrongOrigin)?;
         let revs=revisions.history(plan,chat)
             .map_err(|_|DeliverableError::WrongOrigin)?;
         let latest=revs.last().map(|r|r.revision_sha256.as_str());
         let db=self.db.lock().map_err(|_|DeliverableError::Poisoned)?;
         let mut stmt=db.prepare(
             "SELECT revision_sha256,step_index,deliverable_id,original_turn_id,
                     effect_call_id,tool_id,receipt_sha256,independent_method,
                     observed_sha256 FROM lu_agent_deliverable_receipts
              WHERE plan_id=?1 ORDER BY recorded_at,step_index LIMIT 33"
         )?;
         let rows=stmt.query_map(params![plan],|r|Ok((
             r.get::<_,String>(0)?,r.get::<_,i64>(1)?,
             r.get::<_,String>(2)?,r.get::<_,String>(3)?,
             r.get::<_,String>(4)?,r.get::<_,String>(5)?,
             r.get::<_,String>(6)?,r.get::<_,String>(7)?,
             r.get::<_,String>(8)?
         )))?;
         let mut history=Vec::new();
         for row in rows{
             let (rev,index,deliverable,origin,call,tool,receipt,method,observed)=row?;
             if history.len()>=MAX_WITNESSES{
                 return Err(DeliverableError::InvalidJournal)
             }
             if origin!=turn||!sha_ok(&rev)||!sha_ok(&receipt)
                 ||!sha_ok(&observed)||index<0||index>31{
                 return Err(DeliverableError::InvalidJournal)
             }
             let revision=revs.iter().find(|r|r.revision_sha256==rev)
                 .ok_or(DeliverableError::InvalidJournal)?;
             let step=revision.steps.get(index as usize)
                 .ok_or(DeliverableError::InvalidJournal)?;
             if step.expected!=ExpectedDeliverable::IndependentWitness
                 ||step.kind!=PlanStepKind::IndependentlyCheck
                 ||step.deliverable_id!=deliverable{
                 return Err(DeliverableError::InvalidJournal)
             }
             let status=if latest!=Some(rev.as_str()){
                 "SUPERSEDED_HISTORICAL_WITNESS"
             }else{
                 match observe_existing(turn,&call,profile){
                     Ok(check) if check.tool_id==tool
                         &&check.receipt_sha256==receipt
                         &&check.method==method
                         &&check.observed_sha256==observed
                         &&revision.review_state=="APPROVED_PLAN_ONLY"=>
                             "VERIFIED_AT_OBSERVATION",
                     _=>"NEEDS_RECONCILIATION",
                 }
             };
             history.push(HistoricalDeliverable{
                 revision_sha256:rev,step_index:index as u8,
                 deliverable_id:deliverable,tool_id:tool,effect_call_id:call,
                 receipt_sha256:receipt,independent_method:method,
                 observed_sha256:observed,status,
                 continuation:"NOT_AUTHORIZED_FOR_EXECUTION",
             });
         }
         Ok(history)
     }
 }


#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        agent_plan_ledger::{PlanStepSpec,HostObservation},
        agent_plan_revisions::{
            RevisionProposal,RevisionReason,RevisionStep,HumanDecision,
            HostPlanReferenceVerifier,HostPlanReviewVerifier,
        },
        contracts::{ProtoPayload,ToolInvocation},
        journal::{EffectJournal,IndependentOutcomeVerdict},
        tasks::TaskStore,
        conversation_authority::TrustedUserTurnVerifier,
    };
    use std::time::{SystemTime,UNIX_EPOCH};
    struct Human;
    impl TrustedUserTurnVerifier for Human{
        fn verify_user_turn(&self,_chat:&str,_turn:&str)->bool{true}
    }
    struct Sources;
    impl HostPlanReferenceVerifier for Sources{
        fn goal_event_belongs_to_chat(&self,_chat:&str,_g:&str,_e:&str)->bool{true}
        fn goal_event_is_current(&self,_chat:&str,_g:&str,_e:&str)->bool{true}
        fn assumption_source_is_readable(&self,_chat:&str,_s:&str)->bool{true}
    }
    struct Review;
    impl HostPlanReviewVerifier for Review{
        fn has_exact_human_decision(
            &self,_chat:&str,_turn:&str,_p:&str,_h:&str,_d:HumanDecision
        )->bool{true}
    }
    fn id(p:&str,n:u64)->String{format!("{p}{n:032x}")}
    fn location()->PathBuf{
        let root=std::env::temp_dir().join(format!(
            "lu-agent01c-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
    struct Fixture{
        profile:PathBuf,plan:String,chat:String,turn:String,
        revision:String,step:u8,call:String,
    }
    fn invocation(id:&str,turn:&str)->ToolInvocation{
        ToolInvocation{
            call_id:id.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some(turn.into()),
            input:ProtoPayload{type_url:"fixture-evidence-not-os-proof".into(),bytes:vec![1]}
        }
    }
    /// These are fabricated FIXTURE receipts, not real OS worker proof.
    /// The tests verify custody-join invariants, never Windows Job Objects.
    fn fixture(verified:bool,wrong_method:bool)->Fixture{
        let profile=location();
        let (plan,chat,turn,review_turn)=(
            id("p-",21),id("c-",22),id("g-",23),id("g-",24)
        );
        let revision=id("v-",25);
        let step=0;
        let call="host-call-gh".to_string();
        let planfile=profile.join("agent-plans.sqlite");
        let ledger=AgentPlanLedger::open(&planfile).unwrap();
        ledger.register(&plan,&chat,&turn,&[
            PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:None}
        ],&Human).unwrap();
        let revisions=AgentPlanRevisionStore::open(&planfile).unwrap();
        let proposal=RevisionProposal{
            goal_id:id("goal-",26),goal_event_id:id("m-",27),
            assumption_sources:vec![],reason:RevisionReason::Initial,
            steps:vec![RevisionStep{
                kind:PlanStepKind::IndependentlyCheck,depends_on:None,
                deliverable_id:id("d-",28),
                expected:ExpectedDeliverable::IndependentWitness,
            }]
        };
        let recorded=revisions.append_proposal(&plan,&chat,&turn,&revision,
            None,&proposal,&Human,&Sources).unwrap();
        revisions.record_review(&plan,&chat,&review_turn,
            &recorded.revision_sha256,HumanDecision::Approve,
            &Human,&Review,&Sources).unwrap();
        let tasks=TaskStore::open(
            &profile.join(format!("lu-reviewed-task-{turn}.sqlite"))
        ).unwrap();
        tasks.create(&call,Some(&turn),None).unwrap();
        tasks.start(&call).unwrap();
        tasks.complete(&call).unwrap();
        let effect=EffectJournal::open(
            &profile.join(format!("lu-reviewed-effects-{turn}.sqlite"))
        ).unwrap();
        for x in ["host-call",call.as_str()]{
            effect.prepare(&invocation(x,&turn)).unwrap();
            effect.mark_executing(x).unwrap();
            effect.mark_applied(x,
                if x==call{"worker receipt".as_bytes()}else{"outer receipt".as_bytes()}
            ).unwrap();
        }
        if verified{
            let observed=id("",29);
            let method=if wrong_method{
                "GIT_BRANCH_DIRECT_V1"
            }else{"GIT_HEAD_DIRECT_V1"};
            effect.record_independent_outcome(
                &call,b"worker receipt",IndependentOutcomeVerdict::Verified,
                method,Some(&observed.repeat(2))
            ).unwrap();
        }
        drop(effect);drop(tasks);drop(revisions);drop(ledger);
        Fixture{
            profile,plan,chat,turn,
            revision:recorded.revision_sha256,step,call
        }
    }
    fn stores(f:&Fixture)->(
        AgentPlanLedger,AgentPlanRevisionStore,AgentDeliverableReceipts
    ){
        let path=f.profile.join("agent-plans.sqlite");
        (
            AgentPlanLedger::open(&path).unwrap(),
            AgentPlanRevisionStore::open(&path).unwrap(),
            AgentDeliverableReceipts::open(&path).unwrap()
        )
    }
    #[test]
    fn matching_real_sqlite_receipt_and_independent_source_is_historical_not_execution(){
        let f=fixture(true,false);
        let (ledger,revisions,receipts)=stores(&f);
        assert_eq!(receipts.record_observed(
            &f.plan,&f.chat,&f.turn,&f.revision,f.step,&f.call,
            &f.profile,&ledger,&revisions
        ),Ok(ReceiptWrite::New));
        assert_eq!(receipts.record_observed(
            &f.plan,&f.chat,&f.turn,&f.revision,f.step,&f.call,
            &f.profile,&ledger,&revisions
        ),Ok(ReceiptWrite::AlreadyRecorded));
        let list=receipts.history(
            &f.plan,&f.chat,&f.turn,&f.profile,&ledger,&revisions
        ).unwrap();
        assert_eq!(list.len(),1);
        assert_eq!(list[0].status,"VERIFIED_AT_OBSERVATION");
        assert_eq!(list[0].continuation,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(list[0].independent_method,"GIT_HEAD_DIRECT_V1");
        assert_eq!(receipts.history(&f.plan,&id("c-",99),
            &f.turn,&f.profile,&ledger,&revisions),
            Err(DeliverableError::WrongOrigin));
        drop(receipts);drop(revisions);drop(ledger);
        let (ledger,revisions,receipts)=stores(&f);
        assert_eq!(receipts.history(&f.plan,&f.chat,&f.turn,
            &f.profile,&ledger,&revisions).unwrap()[0].status,
            "VERIFIED_AT_OBSERVATION");
        drop(receipts);drop(revisions);drop(ledger);
        fs::remove_dir_all(f.profile).unwrap();
    }
    #[test]
    fn absent_independent_check_or_wrong_method_cannot_verify_deliverable(){
        for (verified,wrong_method) in [(false,false),(true,true)]{
            let f=fixture(verified,wrong_method);
            let (ledger,revisions,receipts)=stores(&f);
            assert_eq!(receipts.record_observed(
                &f.plan,&f.chat,&f.turn,&f.revision,f.step,&f.call,
                &f.profile,&ledger,&revisions
            ),Err(DeliverableError::UnverifiedEffect));
            drop(receipts);drop(revisions);drop(ledger);
            fs::remove_dir_all(f.profile).unwrap();
        }
    }
    #[test]
    fn changed_independent_witness_requires_reconciliation_after_restart(){
        let f=fixture(true,false);
        let (ledger,revisions,receipts)=stores(&f);
        receipts.record_observed(
            &f.plan,&f.chat,&f.turn,&f.revision,f.step,
            &f.call,&f.profile,&ledger,&revisions
        ).unwrap();
        let path=f.profile.join(format!("lu-reviewed-effects-{}.sqlite",f.turn));
        let db=Connection::open(path).unwrap();
        db.execute(
            "UPDATE lu_independent_outcomes SET observed_sha256=?1
             WHERE call_id=?2",params!["f".repeat(64),f.call]
        ).unwrap();
        drop(db);
        assert_eq!(receipts.history(
            &f.plan,&f.chat,&f.turn,&f.profile,&ledger,&revisions
        ).unwrap()[0].status,"NEEDS_RECONCILIATION");
        assert_eq!(receipts.record_observed(
            &f.plan,&f.chat,&f.turn,&f.revision,f.step,&f.call,
            &f.profile,&ledger,&revisions
        ),Err(DeliverableError::ConflictingReceipt));
        drop(receipts);drop(revisions);drop(ledger);
        fs::remove_dir_all(f.profile).unwrap();
    }
    #[test]
    fn cancellation_intent_blocks_new_receipt_and_does_not_claim_os_cleanup(){
        let f=fixture(true,false);
        let (ledger,revisions,receipts)=stores(&f);
        ledger.request_cancel(&f.plan,&f.chat,&f.turn,
            &id("e-",50)).unwrap();
        assert_eq!(receipts.record_observed(
            &f.plan,&f.chat,&f.turn,&f.revision,f.step,&f.call,
            &f.profile,&ledger,&revisions
        ),Err(DeliverableError::UnapprovedPlan));
        assert_eq!(ledger.snapshot(&f.plan,&f.chat,&f.turn)
            .unwrap().unwrap().continuation,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(receipts);drop(revisions);drop(ledger);
        fs::remove_dir_all(f.profile).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_effect_journal_is_not_trusted_as_verification(){
        use std::os::unix::fs::symlink;
        let f=fixture(true,false);
        let original=f.profile.join(format!("lu-reviewed-effects-{}.sqlite",f.turn));
        let moved=f.profile.join("actual-effect.sqlite");
        fs::rename(&original,&moved).unwrap();
        symlink(&moved,&original).unwrap();
        let (ledger,revisions,receipts)=stores(&f);
        assert_eq!(receipts.record_observed(
            &f.plan,&f.chat,&f.turn,&f.revision,f.step,&f.call,
            &f.profile,&ledger,&revisions
        ),Err(DeliverableError::InvalidJournal));
        drop(receipts);drop(revisions);drop(ledger);
        fs::remove_dir_all(f.profile).unwrap();
    }
}
