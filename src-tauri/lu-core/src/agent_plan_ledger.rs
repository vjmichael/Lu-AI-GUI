//! AGENT-01A: durable plan/step intent WITHOUT native execution authority.
//!
//! Only the host records a bounded dependency plan after verifying its exact
//! original human chat turn. An OBSERVED step is NOT an independently verified
//! result, an OS process completion, a reaping receipt, or a permission grant.
//! This module has no worker spawn, tool registry, automatic retry or resume.
use std::{path::Path,sync::Mutex};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use crate::conversation_authority::TrustedUserTurnVerifier;

const MAX_STEPS:usize=32;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PlanStepKind{
    CollectSource,Inspect,Analyze,IndependentlyCheck,HumanReview,Present,
}
impl PlanStepKind{
    pub(crate) fn as_str(self)->&'static str{
        match self{
            Self::CollectSource=>"COLLECT_SOURCE",
            Self::Inspect=>"INSPECT",
            Self::Analyze=>"ANALYZE",
            Self::IndependentlyCheck=>"INDEPENDENT_CHECK",
            Self::HumanReview=>"HUMAN_REVIEW",
            Self::Present=>"PRESENT",
        }
    }
    pub(crate) fn parse(s:&str)->Option<Self>{
        match s{
            "COLLECT_SOURCE"=>Some(Self::CollectSource),
            "INSPECT"=>Some(Self::Inspect),
            "ANALYZE"=>Some(Self::Analyze),
            "INDEPENDENT_CHECK"=>Some(Self::IndependentlyCheck),
            "HUMAN_REVIEW"=>Some(Self::HumanReview),
            "PRESENT"=>Some(Self::Present),
            _=>None,
        }
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct PlanStepSpec{
    pub kind:PlanStepKind,
    /// A backward-only index prevents cycles and unbounded fanout.
    /// Observing the dependency is not proof that it succeeded.
    pub depends_on:Option<u8>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum HostObservation{
    /// Host saw some event; it has NOT established an OS effect or outcome.
    UnverifiedEvidence,
    /// Host found uncertainty, bad evidence or an unaccounted-for effect.
    ReconciliationNeeded,
}
impl HostObservation{
    fn code(self)->&'static str{
        match self{
            Self::UnverifiedEvidence=>"UNVERIFIED_EVIDENCE",
            Self::ReconciliationNeeded=>"NEEDS_RECONCILIATION",
        }
    }
    fn next_state(self)->&'static str{
        match self{
            Self::UnverifiedEvidence=>"OBSERVED_UNVERIFIED",
            Self::ReconciliationNeeded=>"NEEDS_REVIEW",
        }
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PlanWrite{New,AlreadyRecorded}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum PlanLedgerError{
    Database(String),
    Poisoned,
    InvalidId,
    InvalidPlan,
    UnverifiedHumanTurn,
    ConflictingId,
    WrongOrigin,
    InvalidTransition,
    DamagedState,
}
impl From<rusqlite::Error> for PlanLedgerError{
    fn from(error:rusqlite::Error)->Self{Self::Database(error.to_string())}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct PlannedStep{
    pub index:u8,
    pub kind:PlanStepKind,
    pub depends_on:Option<u8>,
    pub state:String,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct PlanSnapshot{
    pub plan_id:String,
    pub conversation_id:String,
    pub human_turn_id:String,
    pub state:String,
    pub steps:Vec<PlannedStep>,
    /// This ledger never grants a native execution or continuation permit.
    pub continuation:&'static str,
}
pub struct AgentPlanLedger{db:Mutex<Connection>}
fn exact_id(id:&str,prefix:&str)->bool{
    id.len()==prefix.len()+32&&id.starts_with(prefix)
    &&id[prefix.len()..].bytes().all(|b|
        b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn valid_origin(plan:&str,chat:&str,turn:&str)->bool{
    exact_id(plan,"p-")&&exact_id(chat,"c-")&&exact_id(turn,"g-")
}
fn valid_steps(steps:&[PlanStepSpec])->bool{
    !steps.is_empty()&&steps.len()<=MAX_STEPS
        &&steps.iter().enumerate().all(|(index,step)|
            step.depends_on.is_none_or(|parent|(parent as usize)<index))
}
impl AgentPlanLedger{
    pub fn open(path:&Path)->Result<Self,PlanLedgerError>{
        let db=Connection::open(path)?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_agent_plans(
                plan_id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                human_turn_id TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN
                    ('DRAFT','OBSERVATIONS_ONLY','NEEDS_REVIEW','CANCEL_REQUESTED')),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS lu_agent_plan_steps(
                plan_id TEXT NOT NULL,
                step_index INTEGER NOT NULL CHECK(step_index BETWEEN 0 AND 31),
                kind TEXT NOT NULL CHECK(kind IN
                    ('COLLECT_SOURCE','INSPECT','ANALYZE','INDEPENDENT_CHECK','HUMAN_REVIEW','PRESENT')),
                depends_on INTEGER,
                state TEXT NOT NULL CHECK(state IN
                    ('PLANNED','OBSERVED_UNVERIFIED','NEEDS_REVIEW')),
                PRIMARY KEY(plan_id,step_index),
                FOREIGN KEY(plan_id) REFERENCES lu_agent_plans(plan_id)
             );
             CREATE TABLE IF NOT EXISTS lu_agent_plan_events(
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                plan_id TEXT NOT NULL,
                step_index INTEGER,
                host_event_id TEXT UNIQUE,
                event TEXT NOT NULL CHECK(event IN
                    ('OBSERVED_UNVERIFIED','NEEDS_REVIEW','CANCEL_REQUESTED','RECOVERED_NEEDS_REVIEW')),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE INDEX IF NOT EXISTS lu_agent_plan_events_plan
                 ON lu_agent_plan_events(plan_id,sequence);"
        )?;
        Ok(Self{db:Mutex::new(db)})
    }
    /// The caller is the trusted host. The verifier must authenticate a real
    /// SAVED human user turn, never model-generated claims or UI-supplied paths.
    /// Idempotent plans must have identical origin, step sequence and edges.
    pub fn register(
        &self,plan:&str,chat:&str,turn:&str,
        steps:&[PlanStepSpec],verifier:&dyn TrustedUserTurnVerifier,
    )->Result<PlanWrite,PlanLedgerError>{
        if !valid_origin(plan,chat,turn){return Err(PlanLedgerError::InvalidId)}
        if !valid_steps(steps){return Err(PlanLedgerError::InvalidPlan)}
        if !verifier.verify_user_turn(chat,turn){
            return Err(PlanLedgerError::UnverifiedHumanTurn)
        }
        let mut db=self.db.lock().map_err(|_|PlanLedgerError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing:Option<(String,String)>=tx.query_row(
            "SELECT conversation_id,human_turn_id FROM lu_agent_plans
             WHERE plan_id=?1",params![plan],
            |row|Ok((row.get(0)?,row.get(1)?))
        ).optional()?;
        if let Some((old_chat,old_turn))=existing{
            if old_chat!=chat||old_turn!=turn{
                return Err(PlanLedgerError::ConflictingId)
            }
            let mut statement=tx.prepare(
                "SELECT step_index,kind,depends_on FROM lu_agent_plan_steps
                 WHERE plan_id=?1 ORDER BY step_index")?;
            let iter=statement.query_map(params![plan],|row|Ok((
                row.get::<_,i64>(0)?,row.get::<_,String>(1)?,
                row.get::<_,Option<i64>>(2)?
            )))?;
            let mut recorded=Vec::new();
            for r in iter{recorded.push(r?);}
            if recorded.len()!=steps.len()||
                !recorded.iter().zip(steps).enumerate().all(|(i,(old,expected))|
                    old.0==i as i64
                    &&old.1==expected.kind.as_str()
                    &&old.2==expected.depends_on.map(i64::from)){
                return Err(PlanLedgerError::ConflictingId)
            }
            return Ok(PlanWrite::AlreadyRecorded)
        }
        tx.execute(
            "INSERT INTO lu_agent_plans
             (plan_id,conversation_id,human_turn_id,state)
             VALUES (?1,?2,?3,'DRAFT')",
            params![plan,chat,turn]
        )?;
        for (index,step) in steps.iter().enumerate(){
            tx.execute(
                "INSERT INTO lu_agent_plan_steps
                 (plan_id,step_index,kind,depends_on,state)
                 VALUES (?1,?2,?3,?4,'PLANNED')",
                params![plan,index as i64,step.kind.as_str(),
                    step.depends_on.map(i64::from)]
            )?;
        }
        tx.commit()?;
        Ok(PlanWrite::New)
    }
    /// A host observation is a journal fact only. Not a tool execution
    /// receipt, independently checked result or authorization.
    ///
    /// A host event ID must be fresh and exact; duplicate identical deliveries
    /// are idempotent, conflicts cannot be overwritten across plans or steps.
    pub fn observe(
        &self,plan:&str,chat:&str,turn:&str,index:u8,
        event_id:&str,event:HostObservation,
    )->Result<PlanWrite,PlanLedgerError>{
        if !valid_origin(plan,chat,turn)||!exact_id(event_id,"e-"){
            return Err(PlanLedgerError::InvalidId)
        }
        let mut db=self.db.lock().map_err(|_|PlanLedgerError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Check original saved-turn scope BEFORE even an idempotent event
        // lookup: a guessed event token is never an authorization bypass.
        let state=self_state(&tx,plan,chat,turn)?;
        let existing:Option<(String,i64,String)>=tx.query_row(
            "SELECT plan_id,step_index,event FROM lu_agent_plan_events
             WHERE host_event_id=?1",params![event_id],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
        ).optional()?;
        if let Some((id,step,code))=existing{
            if id==plan&&step==i64::from(index)&&code==event.code(){
                return Ok(PlanWrite::AlreadyRecorded)
            }
            return Err(PlanLedgerError::ConflictingId)
        }
        if state=="CANCEL_REQUESTED"||state=="NEEDS_REVIEW"{
            return Err(PlanLedgerError::InvalidTransition)
        }
        let previous:Option<(String,Option<i64>)>=tx.query_row(
            "SELECT state,depends_on FROM lu_agent_plan_steps
             WHERE plan_id=?1 AND step_index=?2",
            params![plan,i64::from(index)],
            |r|Ok((r.get(0)?,r.get(1)?))
        ).optional()?;
        let Some((before,dependency))=previous else{
            return Err(PlanLedgerError::InvalidPlan)
        };
        if before!="PLANNED"{
            return Err(PlanLedgerError::InvalidTransition)
        }
        if event==HostObservation::UnverifiedEvidence{
            if let Some(parent)=dependency{
                let state:String=tx.query_row(
                    "SELECT state FROM lu_agent_plan_steps
                     WHERE plan_id=?1 AND step_index=?2",
                    params![plan,parent],|r|r.get(0)
                )?;
                if state!="OBSERVED_UNVERIFIED"{
                    return Err(PlanLedgerError::InvalidTransition)
                }
            }
        }
        tx.execute(
            "UPDATE lu_agent_plan_steps SET state=?1
             WHERE plan_id=?2 AND step_index=?3 AND state='PLANNED'",
            params![event.next_state(),plan,i64::from(index)]
        )?;
        tx.execute(
            "INSERT INTO lu_agent_plan_events
             (plan_id,step_index,host_event_id,event) VALUES (?1,?2,?3,?4)",
            params![plan,i64::from(index),event_id,event.code()]
        )?;
        let new_plan_state=if event==HostObservation::ReconciliationNeeded{
            "NEEDS_REVIEW"
        }else{"OBSERVATIONS_ONLY"};
        tx.execute(
            "UPDATE lu_agent_plans
             SET state=?1,updated_at=CURRENT_TIMESTAMP WHERE plan_id=?2",
            params![new_plan_state,plan]
        )?;
        tx.commit()?;
        Ok(PlanWrite::New)
    }
    /// This requests cancellation only: it NEVER claims workers stopped.
    /// No side effects are launched and subsequent observations are denied.
    pub fn request_cancel(
        &self,plan:&str,chat:&str,turn:&str,event_id:&str
    )->Result<PlanWrite,PlanLedgerError>{
        if !valid_origin(plan,chat,turn)||!exact_id(event_id,"e-"){
            return Err(PlanLedgerError::InvalidId)
        }
        let mut db=self.db.lock().map_err(|_|PlanLedgerError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state=self_state(&tx,plan,chat,turn)?;
        let prior:Option<(String,String)>=tx.query_row(
            "SELECT plan_id,event FROM lu_agent_plan_events WHERE host_event_id=?1",
            params![event_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional()?;
        if let Some((id,code))=prior{
            return if id==plan&&code=="CANCEL_REQUESTED"{
                Ok(PlanWrite::AlreadyRecorded)
            }else{Err(PlanLedgerError::ConflictingId)}
        }
        if state=="CANCEL_REQUESTED"{return Err(PlanLedgerError::InvalidTransition)}
        tx.execute(
            "UPDATE lu_agent_plans SET state='CANCEL_REQUESTED',
             updated_at=CURRENT_TIMESTAMP WHERE plan_id=?1",params![plan]
        )?;
        tx.execute(
            "INSERT INTO lu_agent_plan_events
             (plan_id,step_index,host_event_id,event)
             VALUES (?1,NULL,?2,'CANCEL_REQUESTED')",
            params![plan,event_id]
        )?;
        tx.commit()?;
        Ok(PlanWrite::New)
    }
    /// ONLY after the prior host is known to have stopped; still does not
    /// prove its OS workers/descendants stopped, and never resumes anything.
    pub fn reconcile_abandoned_observations(&self)
        ->Result<usize,PlanLedgerError>{
        let mut db=self.db.lock().map_err(|_|PlanLedgerError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Capture ONLY formerly unverified observations as recovered.
        // Existing explicit NEEDS_REVIEW rows must not be mislabeled as a
        // restart interruption or acquire a fabricated recovery event.
        tx.execute(
            "INSERT INTO lu_agent_plan_events
             (plan_id,step_index,host_event_id,event)
             SELECT plan_id,step_index,NULL,'RECOVERED_NEEDS_REVIEW'
             FROM lu_agent_plan_steps
             WHERE state='OBSERVED_UNVERIFIED'",[]
        )?;
        let count=tx.execute(
            "UPDATE lu_agent_plan_steps SET state='NEEDS_REVIEW'
             WHERE state='OBSERVED_UNVERIFIED'",[]
        )?;
        tx.execute(
            "UPDATE lu_agent_plans SET state='NEEDS_REVIEW',
             updated_at=CURRENT_TIMESTAMP
             WHERE plan_id IN (
                SELECT plan_id FROM lu_agent_plan_steps
                WHERE state='NEEDS_REVIEW'
             ) AND state!='CANCEL_REQUESTED'",[]
        )?;
        tx.commit()?;
        Ok(count)
    }
    /// Host-readable snapshot only. No executable next-step is returned.
    /// Wrong-chat/turn requests are denied before any task details leak.
    pub fn snapshot(
        &self,plan:&str,chat:&str,turn:&str
    )->Result<Option<PlanSnapshot>,PlanLedgerError>{
        if !valid_origin(plan,chat,turn){return Err(PlanLedgerError::InvalidId)}
        let db=self.db.lock().map_err(|_|PlanLedgerError::Poisoned)?;
        let origin:Option<(String,String,String)>=db.query_row(
            "SELECT conversation_id,human_turn_id,state FROM lu_agent_plans
             WHERE plan_id=?1",params![plan],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
        ).optional()?;
        let Some((stored_chat,stored_turn,state))=origin else{return Ok(None)};
        if stored_chat!=chat||stored_turn!=turn{
            return Err(PlanLedgerError::WrongOrigin)
        }
        if !matches!(state.as_str(),
            "DRAFT"|"OBSERVATIONS_ONLY"|"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
            return Err(PlanLedgerError::DamagedState)
        }
        let mut stmt=db.prepare(
            "SELECT step_index,kind,depends_on,state
             FROM lu_agent_plan_steps
             WHERE plan_id=?1 ORDER BY step_index LIMIT 33"
        )?;
        let iter=stmt.query_map(params![plan],|r|Ok((
            r.get::<_,i64>(0)?,r.get::<_,String>(1)?,
            r.get::<_,Option<i64>>(2)?,r.get::<_,String>(3)?
        )))?;
        let mut steps=Vec::new();
        for row in iter{
            let (index,kind,depends_on,status)=row?;
            if index!=steps.len() as i64||steps.len()>=MAX_STEPS
                ||!matches!(status.as_str(),
                    "PLANNED"|"OBSERVED_UNVERIFIED"|"NEEDS_REVIEW"){
                return Err(PlanLedgerError::DamagedState)
            }
            let Some(kind)=PlanStepKind::parse(&kind) else{
                return Err(PlanLedgerError::DamagedState)
            };
            let dependency=depends_on.map(|v|u8::try_from(v)
                .map_err(|_|PlanLedgerError::DamagedState)).transpose()?;
            if dependency.is_some_and(|p|p as usize>=steps.len()){
                return Err(PlanLedgerError::DamagedState)
            }
            steps.push(PlannedStep{
                index:index as u8,kind,depends_on:dependency,state:status
            });
        }
        if steps.is_empty(){return Err(PlanLedgerError::DamagedState)}
        Ok(Some(PlanSnapshot{
            plan_id:plan.into(),conversation_id:chat.into(),
            human_turn_id:turn.into(),state,steps,
            continuation:"NOT_AUTHORIZED_FOR_EXECUTION",
        }))
    }
}
fn self_state(
    tx:&rusqlite::Transaction<'_>,plan:&str,chat:&str,turn:&str
)->Result<String,PlanLedgerError>{
    let row:Option<(String,String,String)>=tx.query_row(
        "SELECT conversation_id,human_turn_id,state
         FROM lu_agent_plans WHERE plan_id=?1",params![plan],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
    ).optional()?;
    let Some((original_chat,original_turn,state))=row else{
        return Err(PlanLedgerError::InvalidPlan)
    };
    if original_chat!=chat||original_turn!=turn{
        return Err(PlanLedgerError::WrongOrigin)
    }
    if !matches!(state.as_str(),
        "DRAFT"|"OBSERVATIONS_ONLY"|"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
        return Err(PlanLedgerError::DamagedState)
    }
    Ok(state)
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::time::{SystemTime,UNIX_EPOCH};
    struct Source{allow:bool}
    impl TrustedUserTurnVerifier for Source{
        fn verify_user_turn(&self,_chat:&str,_turn:&str)->bool{self.allow}
    }
    fn ids()->(String,String,String){
        (format!("p-{:032x}",20),format!("c-{:032x}",21),
         format!("g-{:032x}",22))
    }
    fn path()->std::path::PathBuf{
        std::env::temp_dir().join(format!(
            "lu-agent01-plan-{}-{}.sqlite",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn specs()->Vec<PlanStepSpec>{
        vec![
            PlanStepSpec{kind:PlanStepKind::CollectSource,depends_on:None},
            PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0)},
            PlanStepSpec{kind:PlanStepKind::Present,depends_on:Some(1)}
        ]
    }
    #[test]
    fn host_verified_plan_dag_persists_and_rejects_cross_chat_rebinding(){
        let file=path();let (p,c,t)=ids();
        {
            let ledger=AgentPlanLedger::open(&file).unwrap();
            assert_eq!(ledger.register(&p,&c,&t,&specs(),&Source{allow:false}),
                Err(PlanLedgerError::UnverifiedHumanTurn));
            assert_eq!(ledger.register(&p,&c,&t,&specs(),&Source{allow:true}),
                Ok(PlanWrite::New));
            assert_eq!(ledger.register(&p,&c,&t,&specs(),&Source{allow:true}),
                Ok(PlanWrite::AlreadyRecorded));
            assert_eq!(ledger.register(&p,&format!("c-{:032x}",99),
                &t,&specs(),&Source{allow:true}),Err(PlanLedgerError::ConflictingId));
            assert_eq!(ledger.snapshot(&p,&format!("c-{:032x}",99),&t),
                Err(PlanLedgerError::WrongOrigin));
            let mut changed=specs();
            changed[2].depends_on=Some(0);
            assert_eq!(ledger.register(&p,&c,&t,&changed,&Source{allow:true}),
                Err(PlanLedgerError::ConflictingId));
        }
        let reopened=AgentPlanLedger::open(&file).unwrap();
        let view=reopened.snapshot(&p,&c,&t).unwrap().unwrap();
        assert_eq!(view.steps.len(),3);
        assert_eq!(view.state,"DRAFT");
        assert_eq!(view.continuation,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(reopened);
        let _=std::fs::remove_file(&file);
    }
    #[test]
    fn rejects_cycles_unbounded_tasks_and_wrong_direction(){
        let file=path();let ledger=AgentPlanLedger::open(&file).unwrap();
        let (p,c,t)=ids();let auth=Source{allow:true};
        assert_eq!(ledger.register(&p,&c,&t,&[],&auth),
            Err(PlanLedgerError::InvalidPlan));
        let mut cycle=specs();cycle[1].depends_on=Some(2);
        assert_eq!(ledger.register(&p,&c,&t,&cycle,&auth),
            Err(PlanLedgerError::InvalidPlan));
        let too_many=vec![PlanStepSpec{
            kind:PlanStepKind::Inspect,depends_on:None
        };33];
        assert_eq!(ledger.register(&p,&c,&t,&too_many,&auth),
            Err(PlanLedgerError::InvalidPlan));
        assert_eq!(ledger.register("../", &c,&t,&specs(),&auth),
            Err(PlanLedgerError::InvalidId));
        drop(ledger);let _=std::fs::remove_file(file);
    }
    #[test]
    fn observations_are_not_execution_proof_and_are_idempotent(){
        let file=path();let ledger=AgentPlanLedger::open(&file).unwrap();
        let (p,c,t)=ids();
        ledger.register(&p,&c,&t,&specs(),&Source{allow:true}).unwrap();
        let e=format!("e-{:032x}",3);
        assert_eq!(ledger.observe(&p,&c,&t,1,&e,
            HostObservation::UnverifiedEvidence),
            Err(PlanLedgerError::InvalidTransition));
        ledger.observe(&p,&c,&t,0,&e,HostObservation::UnverifiedEvidence).unwrap();
        assert_eq!(ledger.observe(&p,&c,&t,0,&e,
            HostObservation::UnverifiedEvidence),Ok(PlanWrite::AlreadyRecorded));
        assert_eq!(ledger.observe(&p,&format!("c-{:032x}",98),
            &t,0,&e,HostObservation::UnverifiedEvidence),
            Err(PlanLedgerError::WrongOrigin));
        assert_eq!(ledger.observe(&p,&c,&t,1,&e,
            HostObservation::UnverifiedEvidence),Err(PlanLedgerError::ConflictingId));
        let b=format!("e-{:032x}",4);
        ledger.observe(&p,&c,&t,1,&b,HostObservation::UnverifiedEvidence).unwrap();
        let snapshot=ledger.snapshot(&p,&c,&t).unwrap().unwrap();
        assert_eq!(snapshot.state,"OBSERVATIONS_ONLY");
        assert_eq!(snapshot.steps[0].state,"OBSERVED_UNVERIFIED");
        assert_eq!(snapshot.continuation,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(ledger);let _=std::fs::remove_file(file);
    }
    #[test]
    fn restart_reconciliation_does_not_claim_worker_death_or_retry(){
        let file=path();let (p,c,t)=ids();
        {
            let ledger=AgentPlanLedger::open(&file).unwrap();
            ledger.register(&p,&c,&t,&specs(),&Source{allow:true}).unwrap();
            ledger.observe(&p,&c,&t,0,&format!("e-{:032x}",7),
                HostObservation::UnverifiedEvidence).unwrap();
        }
        let resumed=AgentPlanLedger::open(&file).unwrap();
        assert_eq!(resumed.reconcile_abandoned_observations().unwrap(),1);
        assert_eq!(resumed.reconcile_abandoned_observations().unwrap(),0);
        let view=resumed.snapshot(&p,&c,&t).unwrap().unwrap();
        assert_eq!(view.state,"NEEDS_REVIEW");
        assert_eq!(view.steps[0].state,"NEEDS_REVIEW");
        assert_eq!(resumed.observe(&p,&c,&t,1,&format!("e-{:032x}",8),
            HostObservation::UnverifiedEvidence),Err(PlanLedgerError::InvalidTransition));
        drop(resumed);let _=std::fs::remove_file(file);
    }
    #[test]
    fn explicit_cancel_intent_blocks_further_plan_steps_but_not_os_processes(){
        let file=path();let (p,c,t)=ids();
        let ledger=AgentPlanLedger::open(&file).unwrap();
        ledger.register(&p,&c,&t,&specs(),&Source{allow:true}).unwrap();
        let e=format!("e-{:032x}",9);
        assert_eq!(ledger.request_cancel(&p,&c,&t,&e),Ok(PlanWrite::New));
        assert_eq!(ledger.request_cancel(&p,&c,&t,&e),
            Ok(PlanWrite::AlreadyRecorded));
        assert_eq!(ledger.snapshot(&p,&c,&t).unwrap().unwrap().state,
            "CANCEL_REQUESTED");
        assert_eq!(ledger.observe(&p,&c,&t,0,&format!("e-{:032x}",10),
            HostObservation::UnverifiedEvidence),Err(PlanLedgerError::InvalidTransition));
        drop(ledger);let _=std::fs::remove_file(file);
    }
}
