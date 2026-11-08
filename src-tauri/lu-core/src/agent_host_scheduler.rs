//! AGENT-01D — bounded, host-fenced, NON-DISPATCHING scheduling decisions.
//!
//! It is not a worker launcher or authorization broker. Review of a plan,
//! even together with a fresh step request, never authorizes OS execution.
//! Native worker launch, process reaping and source revision custody require
//! independently accepted E1 gates. An abandoned session is quarantined,
//! NOT timed-out into a new owner and NOT silently replayed.
use std::{collections::BTreeSet,path::Path,sync::Mutex};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use crate::{
    agent_plan_ledger::{AgentPlanLedger,PlanSnapshot},
    agent_plan_revisions::{AgentPlanRevisionStore,RevisionSnapshot,HostPlanReferenceVerifier},
    agent_deliverable_receipts::AgentDeliverableReceipts,
    conversation_authority::TrustedUserTurnVerifier,
};

const MAX_CLAIMS:u8=8;
const MAX_TOKENS:u32=8192;
const MAX_TOOLS:u8=8;
const MAX_WALL_MS:u32=120_000;
const MAX_OUTPUT_BYTES:u32=8*1024*1024;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ScheduleBudget{
    /// Reservation caps, NOT proof of OS memory, time, token or CPU isolation.
    pub max_claims:u8,
    pub model_token_ceiling:u32,
    pub tool_call_ceiling:u8,
    pub wall_clock_ceiling_ms:u32,
    pub output_ceiling_bytes:u32,
}
impl ScheduleBudget{
    pub fn bounded(&self)->bool{
        (1..=MAX_CLAIMS).contains(&self.max_claims)
        &&(1..=MAX_TOKENS).contains(&self.model_token_ceiling)
        &&(1..=MAX_TOOLS).contains(&self.tool_call_ceiling)
        &&(100..=MAX_WALL_MS).contains(&self.wall_clock_ceiling_ms)
        &&(1..=MAX_OUTPUT_BYTES).contains(&self.output_ceiling_bytes)
    }
    fn fits(&self,request:&Self)->bool{
        request.bounded() && request.max_claims==1
        &&request.model_token_ceiling<=self.model_token_ceiling
        &&request.tool_call_ceiling<=self.tool_call_ceiling
        &&request.wall_clock_ceiling_ms<=self.wall_clock_ceiling_ms
        &&request.output_ceiling_bytes<=self.output_ceiling_bytes
    }
}

/// This *additional* host-only proof is independent of "approved the plan".
/// The user must have made a fresh, explicit request for this exact step and
/// its bounded resource reservation. A model-generated turn cannot supply it.
pub trait HostFreshStepConsent:Send+Sync{
    fn explicitly_requested_step(
        &self,chat:&str,request_turn:&str,plan:&str,revision_sha:&str,
        step_index:u8,budget:&ScheduleBudget,
    )->bool;
}
/// A caller may quarantine an old owner only after verifying the old HOST
/// process stopped. This does NOT prove old worker descendants are reaped.
pub trait HostStoppedWitness:Send+Sync{
    fn old_host_stopped(&self,owner_id:&str)->bool;
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ScheduleError{
    Database(String),Poisoned,InvalidId,InvalidBudget,UnknownPlan,
    WrongOrigin,UnreviewedRevision,StaleRevision,StaleOwner,OwnerBusy,
    NoFreshConsent,DependencyUnverified,AlreadyClaimed,LimitReached,
    InvalidTransition,ReviewRequired,NoProof,CorruptState,
}
impl From<rusqlite::Error> for ScheduleError{
    fn from(error:rusqlite::Error)->Self{Self::Database(error.to_string())}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum WriteStatus{New,AlreadyRecorded}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SessionReceipt{
    pub epoch:u64,
    pub write:WriteStatus,
    /// NO OS, tool or model authority emerges from session ownership.
    pub execution:&'static str,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ScheduledProposal{
    pub plan_id:String,
    pub revision_sha:String,
    pub step_index:u8,
    pub claim_id:String,
    pub original_request_turn:String,
    pub claim_state:String,
    pub epoch:u64,
    pub budget:ScheduleBudget,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SchedulerView{
    pub state:String,
    pub epoch:u64,
    pub revision_sha:String,
    pub requested_claims:u8,
    pub claims:Vec<ScheduledProposal>,
    pub execution:&'static str,
}
pub struct AgentHostScheduler{
    db:Mutex<Connection>,
    /// Deliberately NOT persisted. A restarted process knowing an old owner
    /// ID cannot impersonate the original supervising host.
    live_owners:Mutex<BTreeSet<(String,String)>>,
}
fn exact(value:&str,prefix:&str)->bool{
    value.len()==prefix.len()+32&&value.starts_with(prefix)
        &&value[prefix.len()..].bytes().all(|b|
            b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn sha(value:&str)->bool{
    value.len()==64&&value.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn scope(plan:&str,chat:&str,turn:&str)->bool{
    exact(plan,"p-")&&exact(chat,"c-")&&exact(turn,"g-")
}
fn reviewable(
    ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
    sources:&dyn HostPlanReferenceVerifier,
    plan:&str,chat:&str,origin_turn:&str,
)->Result<(PlanSnapshot,RevisionSnapshot),ScheduleError>{
    let plan_view=ledger.snapshot(plan,chat,origin_turn)
        .map_err(|_|ScheduleError::WrongOrigin)?
        .ok_or(ScheduleError::UnknownPlan)?;
    if matches!(plan_view.state.as_str(),"CANCEL_REQUESTED"|"NEEDS_REVIEW"){
        return Err(ScheduleError::ReviewRequired)
    }
    let rev=revisions.latest(plan,chat)
        .map_err(|_|ScheduleError::WrongOrigin)?
        .ok_or(ScheduleError::UnreviewedRevision)?;
    if rev.review_state!="APPROVED_PLAN_ONLY"||
        !sources.goal_event_is_current(chat,&rev.goal_id,&rev.goal_event_id)||
        rev.assumption_sources.iter().any(|s|
            !sources.assumption_source_is_readable(chat,s)){
        return Err(ScheduleError::UnreviewedRevision)
    }
    Ok((plan_view,rev))
}
fn identity_in_tx(tx:&rusqlite::Transaction<'_>,plan:&str,chat:&str,origin_turn:&str)
    ->Result<String,ScheduleError>{
    let result:Option<(String,String,String)>=tx.query_row(
        "SELECT conversation_id,human_turn_id,state FROM lu_agent_plans
         WHERE plan_id=?1",params![plan],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
    ).optional()?;
    let Some((c,t,state))=result else{return Err(ScheduleError::UnknownPlan)};
    if c!=chat||t!=origin_turn{return Err(ScheduleError::WrongOrigin)}
    if !matches!(state.as_str(),
        "DRAFT"|"OBSERVATIONS_ONLY"|"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
        return Err(ScheduleError::CorruptState)
    }
    Ok(state)
}
fn origin_in_tx(tx:&rusqlite::Transaction<'_>,plan:&str,chat:&str,origin_turn:&str)
    ->Result<String,ScheduleError>{
    let state=identity_in_tx(tx,plan,chat,origin_turn)?;
    if matches!(state.as_str(),"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
        return Err(ScheduleError::ReviewRequired)
    }
    Ok(state)
}
fn revision_in_tx(tx:&rusqlite::Transaction<'_>,plan:&str,rev:&str)
    ->Result<(),ScheduleError>{
    let latest:Option<(String,i64)>=tx.query_row(
        "SELECT revision_sha256,revision_index FROM lu_agent_plan_revisions
         WHERE plan_id=?1 ORDER BY revision_index DESC LIMIT 1",
        params![plan],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional()?;
    let Some((actual,index))=latest else{return Err(ScheduleError::UnreviewedRevision)};
    if actual!=rev{return Err(ScheduleError::StaleRevision)}
    let approved:Option<String>=tx.query_row(
        "SELECT decision FROM lu_agent_plan_revision_reviews
         WHERE plan_id=?1 AND revision_index=?2",
        params![plan,index],|r|r.get(0)
    ).optional()?;
    if approved.as_deref()!=Some("APPROVE_PLAN_ONLY"){
        return Err(ScheduleError::UnreviewedRevision)
    }
    Ok(())
}
#[derive(Clone,Debug)]
struct OwnerRow{
    owner:String,
    epoch:u64,
    revision:String,
    state:String,
    max_claims:u8,
    remaining:ScheduleBudget,
}
fn owned(tx:&rusqlite::Transaction<'_>,plan:&str,owner:&str,epoch:u64)
    ->Result<OwnerRow,ScheduleError>{
    let row:Option<(String,i64,String,String,i64,i64,i64,i64,i64)>=tx.query_row(
        "SELECT owner_id,epoch,revision_sha256,state,max_claims,
                token_ceiling,tool_ceiling,wall_ceiling_ms,output_ceiling
         FROM lu_agent_schedule_owners WHERE plan_id=?1",
        params![plan],|r|Ok((
            r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,
            r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?
        ))
    ).optional()?;
    let Some((id,raw_epoch,revision,state,max_claims,tokens,tools,wall,output))=row
        else{return Err(ScheduleError::StaleOwner)};
    if id!=owner||raw_epoch<0||raw_epoch as u64!=epoch{
        return Err(ScheduleError::StaleOwner)
    }
    if !matches!(state.as_str(),"OWNED"|"CANCEL_REQUESTED"|"RECOVERY_REQUIRED"|"RELEASED"){
        return Err(ScheduleError::CorruptState)
    }
    let bounded=ScheduleBudget{
        max_claims:u8::try_from(max_claims).map_err(|_|ScheduleError::CorruptState)?,
        model_token_ceiling:u32::try_from(tokens).map_err(|_|ScheduleError::CorruptState)?,
        tool_call_ceiling:u8::try_from(tools).map_err(|_|ScheduleError::CorruptState)?,
        wall_clock_ceiling_ms:u32::try_from(wall).map_err(|_|ScheduleError::CorruptState)?,
        output_ceiling_bytes:u32::try_from(output).map_err(|_|ScheduleError::CorruptState)?,
    };
    if !bounded.bounded(){return Err(ScheduleError::CorruptState)}
    Ok(OwnerRow{
        owner:id,epoch,revision,state,
        max_claims:bounded.max_claims,remaining:bounded
    })
}
impl AgentHostScheduler{
    /// Same host-owned plan SQLite file as AGENT-01A/B/C. WAL FULL ensures
    /// the schedule intent is durable; it does NOT atomically cover external
    /// OS effects or the per-human-turn TaskStore and EffectJournal databases.
    pub fn open(file:&Path)->Result<Self,ScheduleError>{
        let db=Connection::open(file)?;
        db.pragma_update(None,"foreign_keys","ON")?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_agent_schedule_owners(
               plan_id TEXT PRIMARY KEY REFERENCES lu_agent_plans(plan_id),
               owner_id TEXT NOT NULL,
               epoch INTEGER NOT NULL CHECK(epoch=1),
               revision_sha256 TEXT NOT NULL,
               state TEXT NOT NULL CHECK(state IN
                 ('OWNED','CANCEL_REQUESTED','RECOVERY_REQUIRED','RELEASED')),
               max_claims INTEGER NOT NULL CHECK(max_claims BETWEEN 1 AND 8),
               token_ceiling INTEGER NOT NULL CHECK(token_ceiling BETWEEN 1 AND 8192),
               tool_ceiling INTEGER NOT NULL CHECK(tool_ceiling BETWEEN 1 AND 8),
               wall_ceiling_ms INTEGER NOT NULL CHECK(wall_ceiling_ms BETWEEN 100 AND 120000),
               output_ceiling INTEGER NOT NULL CHECK(output_ceiling BETWEEN 1 AND 8388608),
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS lu_agent_schedule_proposals(
               plan_id TEXT NOT NULL REFERENCES lu_agent_plans(plan_id),
               revision_sha256 TEXT NOT NULL,
               step_index INTEGER NOT NULL CHECK(step_index BETWEEN 0 AND 31),
               claim_id TEXT NOT NULL UNIQUE,
               owner_id TEXT NOT NULL,
               epoch INTEGER NOT NULL CHECK(epoch=1),
               fresh_human_turn_id TEXT NOT NULL,
               state TEXT NOT NULL CHECK(state IN
                 ('PROPOSED_NO_EXECUTION','OBSERVED_VERIFIED_HISTORICAL',
                  'CANCEL_INTENT_UNCERTAIN','ABANDONED_UNCERTAIN')),
               token_reservation INTEGER NOT NULL,
               tool_reservation INTEGER NOT NULL,
               wall_reservation_ms INTEGER NOT NULL,
               output_reservation INTEGER NOT NULL,
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(plan_id,revision_sha256,step_index),
               UNIQUE(plan_id,fresh_human_turn_id)
             );
             CREATE TRIGGER IF NOT EXISTS lu_schedule_proposal_no_delete
               BEFORE DELETE ON lu_agent_schedule_proposals
               BEGIN SELECT RAISE(ABORT,'Cannot erase durable schedule intent'); END;
             CREATE TRIGGER IF NOT EXISTS lu_schedule_owner_no_delete
               BEFORE DELETE ON lu_agent_schedule_owners
               BEGIN SELECT RAISE(ABORT,'Cannot erase durable supervisor fence'); END;
             CREATE TRIGGER IF NOT EXISTS lu_schedule_owner_immutable
               BEFORE UPDATE OF owner_id,epoch,revision_sha256,max_claims,
                   token_ceiling,tool_ceiling,wall_ceiling_ms,output_ceiling
                   ON lu_agent_schedule_owners
               BEGIN SELECT RAISE(ABORT,'Cannot change host ownership or limits'); END;
             CREATE TRIGGER IF NOT EXISTS lu_schedule_owner_state_guard
               BEFORE UPDATE OF state ON lu_agent_schedule_owners
               WHEN NOT (
                   OLD.state='OWNED' AND NEW.state IN
                     ('CANCEL_REQUESTED','RECOVERY_REQUIRED','RELEASED')
                   OR OLD.state='CANCEL_REQUESTED'
                     AND NEW.state='RECOVERY_REQUIRED'
               )
               BEGIN SELECT RAISE(ABORT,'Illegal supervisor transition'); END;
             CREATE TRIGGER IF NOT EXISTS lu_schedule_proposal_immutable
               BEFORE UPDATE OF plan_id,revision_sha256,step_index,
                   claim_id,owner_id,epoch,fresh_human_turn_id,
                   token_reservation,tool_reservation,
                   wall_reservation_ms,output_reservation
                   ON lu_agent_schedule_proposals
               BEGIN SELECT RAISE(ABORT,'Cannot rebind step proposal identity'); END;
             CREATE TRIGGER IF NOT EXISTS lu_schedule_proposal_transition
               BEFORE UPDATE OF state ON lu_agent_schedule_proposals
               WHEN NOT(
                   OLD.state='PROPOSED_NO_EXECUTION'
                   AND NEW.state IN(
                     'OBSERVED_VERIFIED_HISTORICAL',
                     'CANCEL_INTENT_UNCERTAIN','ABANDONED_UNCERTAIN')
               )
               BEGIN SELECT RAISE(ABORT,'Illegal proposal transition'); END;"
        )?;
        Ok(Self{
            db:Mutex::new(db),
            live_owners:Mutex::new(BTreeSet::new()),
        })
    }
    fn require_live_owner(&self,plan:&str,owner:&str)
        ->Result<(),ScheduleError>{
        let live=self.live_owners.lock()
            .map_err(|_|ScheduleError::Poisoned)?;
        if !live.contains(&(plan.into(),owner.into())){
            return Err(ScheduleError::ReviewRequired)
        }
        Ok(())
    }
    fn forget_live_owner(&self,plan:&str,owner:&str)
        ->Result<(),ScheduleError>{
        let mut live=self.live_owners.lock()
            .map_err(|_|ScheduleError::Poisoned)?;
        live.remove(&(plan.into(),owner.into()));
        Ok(())
    }
    /// Bind ONE host for this plan's full lifetime; no automatic lease
    /// expiry/takeover. A new human revision uses a fresh plan, not a process
    /// ID or old executable grant. Persistent sessions are never force-stolen.
    #[allow(clippy::too_many_arguments)]
    pub fn claim_host(
        &self,plan:&str,chat:&str,turn:&str,owner_id:&str,
        revision_sha:&str,budget:ScheduleBudget,
        ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
        sources:&dyn HostPlanReferenceVerifier,
    )->Result<SessionReceipt,ScheduleError>{
        if !scope(plan,chat,turn)||!exact(owner_id,"h-")||!sha(revision_sha){
            return Err(ScheduleError::InvalidId)
        }
        if !budget.bounded(){return Err(ScheduleError::InvalidBudget)}
        let (_,revision)=reviewable(ledger,revisions,sources,plan,chat,turn)?;
        if revision.revision_sha256!=revision_sha{
            return Err(ScheduleError::StaleRevision)
        }
        let mut db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        origin_in_tx(&tx,plan,chat,turn)?;
        revision_in_tx(&tx,plan,revision_sha)?;
        let existing:Option<String>=tx.query_row(
            "SELECT owner_id FROM lu_agent_schedule_owners WHERE plan_id=?1",
            params![plan],|r|r.get(0)
        ).optional()?;
        if existing.is_some(){
            // Same owner ID cannot be inherited after a process restart.
            self.require_live_owner(plan,owner_id)?;
            let owner=owned(&tx,plan,owner_id,1)?;
            if owner.state!="OWNED"||owner.revision!=revision_sha
                ||owner.remaining!=budget{
                return Err(ScheduleError::OwnerBusy)
            }
            return Ok(SessionReceipt{epoch:1,write:WriteStatus::AlreadyRecorded,
                execution:"NOT_AUTHORIZED_FOR_EXECUTION"})
        }
        tx.execute(
            "INSERT INTO lu_agent_schedule_owners
             (plan_id,owner_id,epoch,revision_sha256,state,max_claims,
              token_ceiling,tool_ceiling,wall_ceiling_ms,output_ceiling)
             VALUES(?1,?2,1,?3,'OWNED',?4,?5,?6,?7,?8)",
            params![plan,owner_id,revision_sha,
                budget.max_claims,budget.model_token_ceiling,
                budget.tool_call_ceiling,budget.wall_clock_ceiling_ms,
                budget.output_ceiling_bytes]
        )?;
        tx.commit()?;
        let mut live=self.live_owners.lock()
            .map_err(|_|ScheduleError::Poisoned)?;
        live.insert((plan.into(),owner_id.into()));
        Ok(SessionReceipt{epoch:1,write:WriteStatus::New,
            execution:"NOT_AUTHORIZED_FOR_EXECUTION"})
    }
    /// Only proposes an additional step in durable SQLite. Requires a new
    /// saved human turn plus a host-only exact step/budget consent witness.
    /// Neither this proposal nor its owner may dispatch a native application.
    #[allow(clippy::too_many_arguments)]
    pub fn propose_step(
        &self,plan:&str,chat:&str,origin_turn:&str,owner:&str,
        epoch:u64,revision_sha:&str,step:u8,claim_id:&str,
        fresh_turn:&str,request:ScheduleBudget,
        ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
        sources:&dyn HostPlanReferenceVerifier,
        human:&dyn TrustedUserTurnVerifier,
        consent:&dyn HostFreshStepConsent,
        profile:&Path,receipts:&AgentDeliverableReceipts,
    )->Result<WriteStatus,ScheduleError>{
        if !scope(plan,chat,origin_turn)||!exact(owner,"h-")
            ||!exact(claim_id,"q-")||!exact(fresh_turn,"g-")
            ||!sha(revision_sha)||epoch!=1{
            return Err(ScheduleError::InvalidId)
        }
        if !request.bounded()||request.max_claims!=1{
            return Err(ScheduleError::InvalidBudget)
        }
        self.require_live_owner(plan,owner)?;
        let (view,revision)=reviewable(ledger,revisions,sources,plan,chat,origin_turn)?;
        if revision.revision_sha256!=revision_sha{return Err(ScheduleError::StaleRevision)}
        if fresh_turn==origin_turn || Some(fresh_turn)==revision.review_turn_id.as_deref()
            ||fresh_turn==revision.source_human_turn_id{
            return Err(ScheduleError::NoFreshConsent)
        }
        if !human.verify_user_turn(chat,fresh_turn)
            ||!consent.explicitly_requested_step(
                chat,fresh_turn,plan,revision_sha,step,&request){
            return Err(ScheduleError::NoFreshConsent)
        }
        let expected=revision.steps.get(step as usize)
            .ok_or(ScheduleError::InvalidTransition)?;
        if view.steps.get(step as usize).is_none()
            ||view.steps[step as usize].state!="PLANNED"{
            return Err(ScheduleError::InvalidTransition)
        }
        // Do not trust the previously cached scheduler witness alone.
        // Reopen the ORIGINAL task/effect journal via AGENT-01C and require
        // its historical, independently checked receipt STILL agrees.
        // Even this cross-database observation never becomes an OS permit.
        if let Some(parent)=expected.depends_on{
            let live=receipts.history(
                plan,chat,origin_turn,profile,ledger,revisions,sources
            ).map_err(|_|ScheduleError::DependencyUnverified)?;
            if !live.iter().any(|row|row.revision_sha256==revision_sha
                &&row.step_index==parent
                &&row.status=="VERIFIED_AT_OBSERVATION"){
                return Err(ScheduleError::DependencyUnverified)
            }
        }
        // An AGENT-01A OBSERVED_UNVERIFIED task does not satisfy a dependency.
        let mut db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        origin_in_tx(&tx,plan,chat,origin_turn)?;
        revision_in_tx(&tx,plan,revision_sha)?;
        let current=owned(&tx,plan,owner,epoch)?;
        if current.state!="OWNED"{return Err(ScheduleError::OwnerBusy)}
        if current.revision!=revision_sha{return Err(ScheduleError::StaleRevision)}
        if !current.remaining.fits(&request){return Err(ScheduleError::InvalidBudget)}
        // The previous dependency must have a separately checked historical
        // witness. This is a database correlation, not OS descendant proof.
        if let Some(parent)=expected.depends_on{
            let previous=tx.query_row(
                "SELECT state FROM lu_agent_schedule_proposals
                 WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
                params![plan,revision_sha,parent],|r|r.get::<_,String>(0)
            ).optional()?;
            if previous.as_deref()!=Some("OBSERVED_VERIFIED_HISTORICAL"){
                return Err(ScheduleError::DependencyUnverified)
            }
        }
        let existing:Option<(String,String,String,i64,i64,i64,i64,i64)>=tx.query_row(
            "SELECT claim_id,fresh_human_turn_id,state,
                    token_reservation,tool_reservation,wall_reservation_ms,
                    output_reservation,epoch
             FROM lu_agent_schedule_proposals
             WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
            params![plan,revision_sha,step],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))
        ).optional()?;
        if let Some((old_id,old_turn,state,tokens,tools,wall,output,old_epoch))=existing{
            if old_id!=claim_id||old_turn!=fresh_turn||old_epoch!=epoch as i64
                ||tokens!=i64::from(request.model_token_ceiling)
                ||tools!=i64::from(request.tool_call_ceiling)
                ||wall!=i64::from(request.wall_clock_ceiling_ms)
                ||output!=i64::from(request.output_ceiling_bytes){
                return Err(ScheduleError::AlreadyClaimed)
            }
            return if matches!(state.as_str(),
                "PROPOSED_NO_EXECUTION"|"OBSERVED_VERIFIED_HISTORICAL"){
                Ok(WriteStatus::AlreadyRecorded)
            }else{Err(ScheduleError::ReviewRequired)}
        }
        let sums:(i64,i64,i64,i64,i64)=tx.query_row(
            "SELECT COUNT(*),COALESCE(SUM(token_reservation),0),
                    COALESCE(SUM(tool_reservation),0),
                    COALESCE(SUM(wall_reservation_ms),0),
                    COALESCE(SUM(output_reservation),0)
             FROM lu_agent_schedule_proposals WHERE plan_id=?1",
            params![plan],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        )?;
        if sums.0>=i64::from(current.max_claims)
            ||sums.1+i64::from(request.model_token_ceiling)
                >i64::from(current.remaining.model_token_ceiling)
            ||sums.2+i64::from(request.tool_call_ceiling)
                >i64::from(current.remaining.tool_call_ceiling)
            ||sums.3+i64::from(request.wall_clock_ceiling_ms)
                >i64::from(current.remaining.wall_clock_ceiling_ms)
            ||sums.4+i64::from(request.output_ceiling_bytes)
                >i64::from(current.remaining.output_ceiling_bytes){
            return Err(ScheduleError::LimitReached)
        }
        let pending:i64=tx.query_row(
            "SELECT COUNT(*) FROM lu_agent_schedule_proposals
             WHERE plan_id=?1 AND state='PROPOSED_NO_EXECUTION'",
            params![plan],|r|r.get(0)
        )?;
        if pending!=0{return Err(ScheduleError::OwnerBusy)}
        // A second request-turn may not be recycled as a different step.
        tx.execute(
            "INSERT INTO lu_agent_schedule_proposals
             (plan_id,revision_sha256,step_index,claim_id,owner_id,
              epoch,fresh_human_turn_id,state,token_reservation,
              tool_reservation,wall_reservation_ms,output_reservation)
             VALUES(?1,?2,?3,?4,?5,?6,?7,
                    'PROPOSED_NO_EXECUTION',?8,?9,?10,?11)",
            params![plan,revision_sha,step,claim_id,owner,epoch,
                fresh_turn,request.model_token_ceiling,
                request.tool_call_ceiling,request.wall_clock_ceiling_ms,
                request.output_ceiling_bytes]
        )?;
        tx.commit()?;
        Ok(WriteStatus::New)
    }
    /// Mark a proposal as HISTORICALLY observed, ONLY by checking the
    /// AGENT-01C independent result still exists in original task/effect
    /// SQLite. Never returns a worker permit or advances Lu TaskStore.
    #[allow(clippy::too_many_arguments)]
    pub fn acknowledge_historical_receipt(
        &self,plan:&str,chat:&str,origin_turn:&str,owner:&str,
        epoch:u64,revision_sha:&str,step:u8,claim_id:&str,
        profile:&Path,ledger:&AgentPlanLedger,
        revisions:&AgentPlanRevisionStore,
        receipts:&AgentDeliverableReceipts,
        sources:&dyn HostPlanReferenceVerifier,
    )->Result<WriteStatus,ScheduleError>{
        if !scope(plan,chat,origin_turn)||!exact(owner,"h-")
            ||!exact(claim_id,"q-")||!sha(revision_sha)||epoch!=1{
            return Err(ScheduleError::InvalidId)
        }
        self.require_live_owner(plan,owner)?;
        let (_,revision)=reviewable(ledger,revisions,sources,plan,chat,origin_turn)?;
        if revision.revision_sha256!=revision_sha{
            return Err(ScheduleError::StaleRevision)
        }
        let history=receipts.history(
            plan,chat,origin_turn,profile,ledger,revisions,sources
        ).map_err(|_|ScheduleError::NoProof)?;
        if !history.iter().any(|row|
            row.revision_sha256==revision_sha &&row.step_index==step
            &&row.status=="VERIFIED_AT_OBSERVATION"){
            return Err(ScheduleError::NoProof)
        }
        let mut db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        origin_in_tx(&tx,plan,chat,origin_turn)?;
        revision_in_tx(&tx,plan,revision_sha)?;
        let current=owned(&tx,plan,owner,epoch)?;
        if current.state!="OWNED"||current.revision!=revision_sha{
            return Err(ScheduleError::StaleOwner)
        }
        let row:Option<(String,String)>=tx.query_row(
            "SELECT claim_id,state FROM lu_agent_schedule_proposals
             WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
            params![plan,revision_sha,step],
            |r|Ok((r.get(0)?,r.get(1)?))
        ).optional()?;
        let Some((old_id,state))=row else{return Err(ScheduleError::NoProof)};
        if old_id!=claim_id{return Err(ScheduleError::AlreadyClaimed)}
        if state=="OBSERVED_VERIFIED_HISTORICAL"{
            return Ok(WriteStatus::AlreadyRecorded)
        }
        if state!="PROPOSED_NO_EXECUTION"{
            return Err(ScheduleError::ReviewRequired)
        }
        let changed=tx.execute(
            "UPDATE lu_agent_schedule_proposals
             SET state='OBSERVED_VERIFIED_HISTORICAL'
             WHERE plan_id=?1 AND revision_sha256=?2
               AND step_index=?3 AND state='PROPOSED_NO_EXECUTION'",
            params![plan,revision_sha,step]
        )?;
        if changed!=1{return Err(ScheduleError::CorruptState)}
        tx.commit()?;
        Ok(WriteStatus::New)
    }
    /// A cancellation request blocks all new scheduling decisions. The
    /// still-proposed step becomes UNCERTAIN; no process/descendant kill,
    /// worker cancellation result, or source state is fabricated.
    pub fn request_cancel(
        &self,plan:&str,chat:&str,turn:&str,owner:&str,epoch:u64,
    )->Result<WriteStatus,ScheduleError>{
        if !scope(plan,chat,turn)||!exact(owner,"h-")||epoch!=1{
            return Err(ScheduleError::InvalidId)
        }
        self.require_live_owner(plan,owner)?;
        let mut db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        origin_in_tx(&tx,plan,chat,turn)?;
        let state=owned(&tx,plan,owner,epoch)?;
        if state.state=="CANCEL_REQUESTED"{
            return Ok(WriteStatus::AlreadyRecorded)
        }
        if state.state!="OWNED"{return Err(ScheduleError::InvalidTransition)}
        tx.execute(
            "UPDATE lu_agent_schedule_owners
             SET state='CANCEL_REQUESTED' WHERE plan_id=?1",
            params![plan]
        )?;
        tx.execute(
            "UPDATE lu_agent_schedule_proposals
             SET state='CANCEL_INTENT_UNCERTAIN'
             WHERE plan_id=?1 AND state='PROPOSED_NO_EXECUTION'",
            params![plan]
        )?;
        tx.commit()?;
        // Keep only the current-process identity so duplicate cancellation
        // notices remain idempotent; OWNED admission stays blocked by state.
        Ok(WriteStatus::New)
    }
    /// Caller must independently verify the prior HOST ended. Never
    /// authorizes ownership transfer or asserts old workers were terminated.
    /// No time-based lease reaping: an outage never turns into unsafe replay.
    pub fn quarantine_abandoned(
        &self,plan:&str,chat:&str,turn:&str,
        previous_owner:&str,witness:&dyn HostStoppedWitness,
    )->Result<WriteStatus,ScheduleError>{
        if !scope(plan,chat,turn)||!exact(previous_owner,"h-"){
            return Err(ScheduleError::InvalidId)
        }
        if !witness.old_host_stopped(previous_owner){
            return Err(ScheduleError::ReviewRequired)
        }
        let mut db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Quarantine remains available if the original plan was already
        // cancelled or marked uncertain; never grants new execution.
        identity_in_tx(&tx,plan,chat,turn)?;
        let state=owned(&tx,plan,previous_owner,1)?;
        if state.state=="RECOVERY_REQUIRED"{
            return Ok(WriteStatus::AlreadyRecorded)
        }
        if state.state=="RELEASED"{return Err(ScheduleError::InvalidTransition)}
        tx.execute(
            "UPDATE lu_agent_schedule_owners
             SET state='RECOVERY_REQUIRED' WHERE plan_id=?1",params![plan]
        )?;
        tx.execute(
            "UPDATE lu_agent_schedule_proposals
             SET state='ABANDONED_UNCERTAIN'
             WHERE plan_id=?1 AND state='PROPOSED_NO_EXECUTION'",
            params![plan]
        )?;
        tx.commit()?;
        // Abandoned owner cannot keep mutating this schedule instance.
        self.forget_live_owner(plan,previous_owner)?;
        Ok(WriteStatus::New)
    }
    /// Clean release only when all planning intents have a verified *past*
    /// witness. A released plan cannot be reused to impersonate a new run;
    /// start a NEW plan with explicit human authority instead.
    pub fn release_without_dispatch(
        &self,plan:&str,chat:&str,turn:&str,owner:&str,epoch:u64,
    )->Result<(),ScheduleError>{
        if !scope(plan,chat,turn)||!exact(owner,"h-")||epoch!=1{
            return Err(ScheduleError::InvalidId)
        }
        self.require_live_owner(plan,owner)?;
        let mut db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        origin_in_tx(&tx,plan,chat,turn)?;
        let state=owned(&tx,plan,owner,epoch)?;
        if state.state!="OWNED"{return Err(ScheduleError::InvalidTransition)}
        let unresolved:i64=tx.query_row(
            "SELECT COUNT(*) FROM lu_agent_schedule_proposals
             WHERE plan_id=?1 AND state!='OBSERVED_VERIFIED_HISTORICAL'",
            params![plan],|r|r.get(0)
        )?;
        if unresolved!=0{return Err(ScheduleError::ReviewRequired)}
        tx.execute(
            "UPDATE lu_agent_schedule_owners SET state='RELEASED'
             WHERE plan_id=?1",params![plan]
        )?;
        tx.commit()?;
        self.forget_live_owner(plan,owner)?;
        Ok(())
    }
    /// Authenticated read-only host progress. Claim IDs are host generated
    /// opaque references; no process IDs, stdout or private source paths.
    pub fn view(
        &self,plan:&str,chat:&str,turn:&str,
        ledger:&AgentPlanLedger,
    )->Result<Option<SchedulerView>,ScheduleError>{
        if !scope(plan,chat,turn){return Err(ScheduleError::InvalidId)}
        ledger.snapshot(plan,chat,turn)
            .map_err(|_|ScheduleError::WrongOrigin)?
            .ok_or(ScheduleError::UnknownPlan)?;
        let db=self.db.lock().map_err(|_|ScheduleError::Poisoned)?;
        let owner:Option<(String,i64,String)>=db.query_row(
            "SELECT state,epoch,revision_sha256
             FROM lu_agent_schedule_owners WHERE plan_id=?1",
            params![plan],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
        ).optional()?;
        let Some((state,epoch,rev))=owner else{return Ok(None)};
        if epoch!=1||!sha(&rev)||!matches!(state.as_str(),
            "OWNED"|"RELEASED"|"CANCEL_REQUESTED"|"RECOVERY_REQUIRED"){
            return Err(ScheduleError::CorruptState)
        }
        let mut stmt=db.prepare(
            "SELECT revision_sha256,step_index,claim_id,fresh_human_turn_id,
                    state,epoch,token_reservation,tool_reservation,
                    wall_reservation_ms,output_reservation
             FROM lu_agent_schedule_proposals WHERE plan_id=?1
             ORDER BY step_index LIMIT 9"
        )?;
        let rows=stmt.query_map(params![plan],|r|Ok((
            r.get::<_,String>(0)?,r.get::<_,i64>(1)?,
            r.get::<_,String>(2)?,r.get::<_,String>(3)?,
            r.get::<_,String>(4)?,r.get::<_,i64>(5)?,
            r.get::<_,i64>(6)?,r.get::<_,i64>(7)?,
            r.get::<_,i64>(8)?,r.get::<_,i64>(9)?
        )))?;
        let mut claims=Vec::new();
        for row in rows{
            let (revision,index,id,request,state,claim_epoch,tokens,tools,wall,output)=row?;
            if claims.len()>=MAX_CLAIMS as usize
                ||!sha(&revision)||!(0..32).contains(&index)
                ||!exact(&id,"q-")||!exact(&request,"g-")||claim_epoch!=epoch
                ||!matches!(state.as_str(),
                    "PROPOSED_NO_EXECUTION"|"OBSERVED_VERIFIED_HISTORICAL"|
                    "CANCEL_INTENT_UNCERTAIN"|"ABANDONED_UNCERTAIN"){
                return Err(ScheduleError::CorruptState)
            }
            let budget=ScheduleBudget{
                max_claims:1,
                model_token_ceiling:u32::try_from(tokens)
                    .map_err(|_|ScheduleError::CorruptState)?,
                tool_call_ceiling:u8::try_from(tools)
                    .map_err(|_|ScheduleError::CorruptState)?,
                wall_clock_ceiling_ms:u32::try_from(wall)
                    .map_err(|_|ScheduleError::CorruptState)?,
                output_ceiling_bytes:u32::try_from(output)
                    .map_err(|_|ScheduleError::CorruptState)?,
            };
            if !budget.bounded(){return Err(ScheduleError::CorruptState)}
            claims.push(ScheduledProposal{
                plan_id:plan.into(),revision_sha:revision,
                step_index:index as u8,claim_id:id,
                original_request_turn:request,claim_state:state,
                epoch:epoch as u64,budget
            });
        }
        Ok(Some(SchedulerView{
            state,epoch:epoch as u64,revision_sha:rev,
            requested_claims:claims.len() as u8,claims,
            execution:"NOT_AUTHORIZED_FOR_EXECUTION",
        }))
    }
}


#[cfg(test)]
mod tests{
    use super::*;
    use rusqlite::params;
    use crate::{
        agent_plan_ledger::{PlanStepKind,PlanStepSpec},
        agent_plan_revisions::{
            RevisionProposal,RevisionReason,RevisionStep,ExpectedDeliverable,
            HostPlanReviewVerifier,HumanDecision,
        },
        contracts::{ProtoPayload,ToolInvocation},
        journal::{EffectJournal,IndependentOutcomeVerdict},
        tasks::TaskStore,
    };
    use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
    struct Human;
    impl TrustedUserTurnVerifier for Human{
        fn verify_user_turn(&self,_chat:&str,_turn:&str)->bool{true}
    }
    struct Sources(bool);
    impl HostPlanReferenceVerifier for Sources{
        fn goal_event_belongs_to_chat(&self,_chat:&str,_goal:&str,_event:&str)->bool{true}
        fn goal_event_is_current(&self,_chat:&str,_goal:&str,_event:&str)->bool{self.0}
        fn assumption_source_is_readable(&self,_chat:&str,_source:&str)->bool{self.0}
    }
    struct Review;
    impl HostPlanReviewVerifier for Review{
        fn has_exact_human_decision(
            &self,_chat:&str,_turn:&str,_plan:&str,_sha:&str,_decision:HumanDecision
        )->bool{true}
    }
    struct Fresh(bool);
    impl HostFreshStepConsent for Fresh{
        fn explicitly_requested_step(
            &self,_chat:&str,_turn:&str,_plan:&str,_rev:&str,
            _step:u8,_budget:&ScheduleBudget,
        )->bool{self.0}
    }
    struct Stopped(bool);
    impl HostStoppedWitness for Stopped{
        fn old_host_stopped(&self,_owner:&str)->bool{self.0}
    }
    fn id(prefix:&str,n:u64)->String{format!("{prefix}{n:032x}")}
    fn session_budget()->ScheduleBudget{
        ScheduleBudget{
            max_claims:2,model_token_ceiling:1000,tool_call_ceiling:2,
            wall_clock_ceiling_ms:2000,output_ceiling_bytes:2048
        }
    }
    fn step_budget()->ScheduleBudget{
        ScheduleBudget{
            max_claims:1,model_token_ceiling:400,tool_call_ceiling:1,
            wall_clock_ceiling_ms:1000,output_ceiling_bytes:1024
        }
    }
    struct Fixture{
        root:PathBuf,db:PathBuf,plan:String,chat:String,
        origin:String,revision:String,owner:String
    }
    fn fixture()->Fixture{
        let root=std::env::temp_dir().join(format!(
            "lu-agent01d-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let db=root.join("plans.sqlite");
        let (plan,chat,origin,owner)=(
            id("p-",1),id("c-",2),id("g-",3),id("h-",4)
        );
        let steps=[
            PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:None},
            PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0)},
        ];
        let ledger=AgentPlanLedger::open(&db).unwrap();
        ledger.register(&plan,&chat,&origin,&steps,&Human).unwrap();
        let revisions=AgentPlanRevisionStore::open(&db).unwrap();
        let proposal=RevisionProposal{
            goal_id:id("goal-",5),goal_event_id:id("m-",6),
            assumption_sources:vec![],reason:RevisionReason::Initial,
            steps:vec![
                RevisionStep{
                    kind:PlanStepKind::IndependentlyCheck,depends_on:None,
                    deliverable_id:id("d-",7),expected:ExpectedDeliverable::IndependentWitness,
                },
                RevisionStep{
                    kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0),
                    deliverable_id:id("d-",8),expected:ExpectedDeliverable::IndependentWitness,
                },
            ],
        };
        let appended=revisions.append_proposal(
            &plan,&chat,&origin,&id("v-",9),None,
            &proposal,&Human,&Sources(true)
        ).unwrap();
        revisions.record_review(
            &plan,&chat,&id("g-",10),&appended.revision_sha256,
            HumanDecision::Approve,&Human,&Review,&Sources(true)
        ).unwrap();
        Fixture{root,db,plan,chat,origin,
            revision:appended.revision_sha256,owner}
    }
    fn opened(f:&Fixture)->(
        AgentPlanLedger,AgentPlanRevisionStore,
        AgentDeliverableReceipts,AgentHostScheduler
    ){
        (
            AgentPlanLedger::open(&f.db).unwrap(),
            AgentPlanRevisionStore::open(&f.db).unwrap(),
            AgentDeliverableReceipts::open(&f.db).unwrap(),
            AgentHostScheduler::open(&f.db).unwrap()
        )
    }
    fn claim(f:&Fixture,s:&AgentHostScheduler,
        a:&AgentPlanLedger,b:&AgentPlanRevisionStore)->SessionReceipt{
        s.claim_host(&f.plan,&f.chat,&f.origin,&f.owner,&f.revision,
            session_budget(),a,b,&Sources(true)).unwrap()
    }
    #[test]
    fn only_one_durable_host_can_own_a_plan_and_nothing_is_executable(){
        let f=fixture();
        let (a,b,c,s)=opened(&f);
        let first=claim(&f,&s,&a,&b);
        assert_eq!(first.write,WriteStatus::New);
        assert_eq!(first.epoch,1);
        assert_eq!(first.execution,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(claim(&f,&s,&a,&b).write,WriteStatus::AlreadyRecorded);
        assert_eq!(s.claim_host(
            &f.plan,&f.chat,&f.origin,&id("h-",99),&f.revision,
            session_budget(),&a,&b,&Sources(true)
        ),Err(ScheduleError::ReviewRequired));
        assert_eq!(s.claim_host(
            &f.plan,&id("c-",88),&f.origin,&id("h-",88),&f.revision,
            session_budget(),&a,&b,&Sources(true)
        ),Err(ScheduleError::WrongOrigin));
        assert_eq!(s.view(&f.plan,&f.chat,&f.origin,&a).unwrap().unwrap().state,
            "OWNED");
        let db=Connection::open(&f.db).unwrap();
        assert!(db.execute(
            "UPDATE lu_agent_schedule_owners SET owner_id=?1 WHERE plan_id=?2",
            params![id("h-",99),&f.plan]).is_err());
        assert!(db.execute(
            "DELETE FROM lu_agent_schedule_owners WHERE plan_id=?1",
            params![&f.plan]).is_err());
        drop(db);
        drop(s);drop(c);drop(b);drop(a);
        let (a,b,c,s)=opened(&f);
        assert_eq!(s.view(&f.plan,&f.chat,&f.origin,&a).unwrap().unwrap().epoch,1);
        assert_eq!(s.claim_host(
            &f.plan,&f.chat,&f.origin,&id("h-",99),&f.revision,
            session_budget(),&a,&b,&Sources(true)
        ),Err(ScheduleError::ReviewRequired));
        assert_eq!(s.claim_host(
            &f.plan,&f.chat,&f.origin,&f.owner,&f.revision,
            session_budget(),&a,&b,&Sources(true)
        ),Err(ScheduleError::ReviewRequired));
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&id("q-",70),&id("g-",71),step_budget(),&a,&b,
            &Sources(true),&Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::ReviewRequired));
        drop(s);drop(c);drop(b);drop(a);fs::remove_dir_all(f.root).unwrap();
    }
    #[test]
    fn fresh_human_consent_and_requested_resource_ceiling_are_required(){
        let f=fixture();let (a,b,c,s)=opened(&f);
        claim(&f,&s,&a,&b);
        let q=id("q-",20);
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&q,&f.origin,step_budget(),&a,&b,&Sources(true),
            &Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::NoFreshConsent));
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&q,&id("g-",21),step_budget(),&a,&b,&Sources(true),
            &Human,&Fresh(false),&f.root,&c
        ),Err(ScheduleError::NoFreshConsent));
        let mut oversized=step_budget();
        oversized.model_token_ceiling=10_000;
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&q,&id("g-",21),oversized,&a,&b,&Sources(true),
            &Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::InvalidBudget));
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&q,&id("g-",21),step_budget(),&a,&b,&Sources(true),
            &Human,&Fresh(true),&f.root,&c
        ),Ok(WriteStatus::New));
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&q,&id("g-",21),step_budget(),&a,&b,&Sources(true),
            &Human,&Fresh(true),&f.root,&c
        ),Ok(WriteStatus::AlreadyRecorded));
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&id("q-",22),&id("g-",23),step_budget(),&a,&b,&Sources(true),
            &Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::AlreadyClaimed));
        // A model cannot get the scheduler to skip dependencies by claiming
        // AGENT-01A "OBSERVED_UNVERIFIED" is a completed outcome.
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            1,&id("q-",24),&id("g-",25),step_budget(),&a,&b,&Sources(true),
            &Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::DependencyUnverified));
        let view=s.view(&f.plan,&f.chat,&f.origin,&a).unwrap().unwrap();
        assert_eq!(view.claims.len(),1);
        assert_eq!(view.claims[0].claim_state,"PROPOSED_NO_EXECUTION");
        assert_eq!(view.execution,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(s);drop(c);drop(b);drop(a);fs::remove_dir_all(f.root).unwrap();
    }
    #[test]
    fn cancel_intent_and_host_quarantine_do_not_reap_or_replay_workers(){
        let f=fixture();let (a,b,c,s)=opened(&f);
        claim(&f,&s,&a,&b);
        s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,0,
            &id("q-",27),&id("g-",28),step_budget(),
            &a,&b,&Sources(true),&Human,&Fresh(true),&f.root,&c
        ).unwrap();
        assert_eq!(s.quarantine_abandoned(
            &f.plan,&f.chat,&f.origin,&f.owner,&Stopped(false)
        ),Err(ScheduleError::ReviewRequired));
        assert_eq!(s.request_cancel(
            &f.plan,&f.chat,&f.origin,&f.owner,1
        ),Ok(WriteStatus::New));
        assert_eq!(s.request_cancel(
            &f.plan,&f.chat,&f.origin,&f.owner,1
        ),Ok(WriteStatus::AlreadyRecorded));
        assert_eq!(s.release_without_dispatch(
            &f.plan,&f.chat,&f.origin,&f.owner,1
        ),Err(ScheduleError::InvalidTransition));
        assert_eq!(s.quarantine_abandoned(
            &f.plan,&f.chat,&f.origin,&f.owner,&Stopped(true)
        ),Ok(WriteStatus::New));
        assert_eq!(s.quarantine_abandoned(
            &f.plan,&f.chat,&f.origin,&f.owner,&Stopped(true)
        ),Ok(WriteStatus::AlreadyRecorded));
        let projection=s.view(&f.plan,&f.chat,&f.origin,&a).unwrap().unwrap();
        assert_eq!(projection.state,"RECOVERY_REQUIRED");
        assert_eq!(projection.claims[0].claim_state,"CANCEL_INTENT_UNCERTAIN");
        // Status stays uncertain even when host stopped. It never claims a
        // child process was terminated.
        drop(s);drop(c);drop(b);drop(a);
        let (a,b,c,s)=opened(&f);
        assert_eq!(s.view(&f.plan,&f.chat,&f.origin,&a)
            .unwrap().unwrap().state,"RECOVERY_REQUIRED");
        assert_eq!(s.claim_host(
            &f.plan,&f.chat,&f.origin,&id("h-",99),&f.revision,
            session_budget(),&a,&b,&Sources(true)
        ),Err(ScheduleError::ReviewRequired));
        drop(s);drop(c);drop(b);drop(a);fs::remove_dir_all(f.root).unwrap();
    }
    #[test]
    fn stale_goal_and_stale_plan_revision_cannot_receive_a_step(){
        let f=fixture();let (a,b,c,s)=opened(&f);
        assert_eq!(s.claim_host(
            &f.plan,&f.chat,&f.origin,&f.owner,&f.revision,
            session_budget(),&a,&b,&Sources(false)
        ),Err(ScheduleError::UnreviewedRevision));
        claim(&f,&s,&a,&b);
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,0,
            &id("q-",32),&id("g-",33),step_budget(),
            &a,&b,&Sources(false),&Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::UnreviewedRevision));
        let mut changed=RevisionProposal{
            goal_id:id("goal-",5),goal_event_id:id("m-",34),
            assumption_sources:vec![],reason:RevisionReason::GoalChanged,
            steps:vec![
                RevisionStep{
                    kind:PlanStepKind::IndependentlyCheck,depends_on:None,
                    deliverable_id:id("d-",7),expected:ExpectedDeliverable::IndependentWitness
                },
                RevisionStep{
                    kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0),
                    deliverable_id:id("d-",8),expected:ExpectedDeliverable::IndependentWitness
                },
            ],
        };
        let next=b.append_proposal(
            &f.plan,&f.chat,&id("g-",35),&id("v-",36),
            Some(&f.revision),&changed,&Human,&Sources(true)
        ).unwrap();
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,0,
            &id("q-",32),&id("g-",33),step_budget(),
            &a,&b,&Sources(true),&Human,&Fresh(true),&f.root,&c
        ),Err(ScheduleError::StaleRevision));
        assert_ne!(next.revision_sha256,f.revision);
        // No old owner's automatic adoption of a changed goal.
        drop(s);drop(c);drop(b);drop(a);fs::remove_dir_all(f.root).unwrap();
    }
    #[test]
    fn no_independent_receipt_means_no_completion_and_no_next_step(){
        let f=fixture();let (a,b,c,s)=opened(&f);
        claim(&f,&s,&a,&b);
        s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&id("q-",41),&id("g-",42),step_budget(),
            &a,&b,&Sources(true),&Human,&Fresh(true),&f.root,&c
        ).unwrap();
        assert_eq!(s.acknowledge_historical_receipt(
            &f.plan,&f.chat,&f.origin,&f.owner,1,
            &f.revision,0,&id("q-",41),&f.root,&a,&b,&c,&Sources(true)
        ),Err(ScheduleError::NoProof));
        assert_eq!(s.release_without_dispatch(
            &f.plan,&f.chat,&f.origin,&f.owner,1
        ),Err(ScheduleError::ReviewRequired));
        drop(s);drop(c);drop(b);drop(a);fs::remove_dir_all(f.root).unwrap();
    }
    fn tool(id:&str,turn:&str)->ToolInvocation{
        ToolInvocation{
            call_id:id.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some(turn.into()),
            input:ProtoPayload{type_url:"synthetic-fixture-only".into(),bytes:vec![1]}
        }
    }
    #[test]
    fn independently_checked_historical_receipt_unlocks_only_a_new_proposal(){
        let f=fixture();let (a,b,c,s)=opened(&f);
        claim(&f,&s,&a,&b);
        s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            0,&id("q-",50),&id("g-",51),step_budget(),
            &a,&b,&Sources(true),&Human,&Fresh(true),&f.root,&c
        ).unwrap();
        // Actual production TaskStore + EffectJournal, but these are FIXTURE
        // bytes: they are NOT an OS/Git/FFprobe child process acceptance.
        let worker="call-for-fixture-gh";
        let outer="call-for-fixture";
        let task=TaskStore::open(
            &f.root.join(format!("lu-reviewed-task-{}.sqlite",f.origin))
        ).unwrap();
        task.create(worker,Some(&f.origin),None).unwrap();
        task.start(worker).unwrap();
        task.complete(worker).unwrap();
        let effect=EffectJournal::open(
            &f.root.join(format!("lu-reviewed-effects-{}.sqlite",f.origin))
        ).unwrap();
        for id in [outer,worker]{
            effect.prepare(&tool(id,&f.origin)).unwrap();
            effect.mark_executing(id).unwrap();
            effect.mark_applied(id,if id==worker{b"worker receipt"}else{b"outer receipt"})
                .unwrap();
        }
        effect.record_independent_outcome(
            worker,b"worker receipt",IndependentOutcomeVerdict::Verified,
            "GIT_HEAD_DIRECT_V1",Some(&"a".repeat(64))
        ).unwrap();
        assert!(c.record_observed(
            &f.plan,&f.chat,&f.origin,&f.revision,0,worker,
            &f.root,&a,&b,&Sources(true)
        ).is_ok());
        assert_eq!(s.acknowledge_historical_receipt(
            &f.plan,&f.chat,&f.origin,&f.owner,1,
            &f.revision,0,&id("q-",50),&f.root,&a,&b,&c,&Sources(true)
        ),Ok(WriteStatus::New));
        assert_eq!(s.acknowledge_historical_receipt(
            &f.plan,&f.chat,&f.origin,&f.owner,1,
            &f.revision,0,&id("q-",50),&f.root,&a,&b,&c,&Sources(true)
        ),Ok(WriteStatus::AlreadyRecorded));
        // A second proposed step requires another verified and distinct
        // human request; never old-effect replay.
        assert_eq!(s.propose_step(
            &f.plan,&f.chat,&f.origin,&f.owner,1,&f.revision,
            1,&id("q-",52),&id("g-",53),step_budget(),
            &a,&b,&Sources(true),&Human,&Fresh(true),&f.root,&c
        ),Ok(WriteStatus::New));
        assert_eq!(s.view(&f.plan,&f.chat,&f.origin,&a)
            .unwrap().unwrap().requested_claims,2);
        assert_eq!(s.release_without_dispatch(
            &f.plan,&f.chat,&f.origin,&f.owner,1
        ),Err(ScheduleError::ReviewRequired));
        drop(effect);drop(task);drop(s);drop(c);drop(b);drop(a);
        fs::remove_dir_all(f.root).unwrap();
    }
}
