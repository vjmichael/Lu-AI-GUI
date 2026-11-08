//! AGENT-01B: immutable, source-referenced plan proposals and human decisions.
//!
//! The original AGENT-01A plan and task SQLite are authoritative for identity,
//! NOT for execution. Every revision remains a non-executable PROPOSAL.
//! Approval is a separate, exact-hash human review; it does not authorize any
//! native tool, replay an effect, resume a worker, or prove output correctness.
use std::{collections::BTreeSet,path::Path,sync::Mutex};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use sha2::{Digest,Sha256};
use crate::{
    agent_plan_ledger::PlanStepKind,
    ansf_source_ids::AnsfSourceId,
    conversation_authority::TrustedUserTurnVerifier,
};

const MAX_STEPS:usize=32;
const MAX_REVISIONS:usize=64;
const MAX_ASSUMPTIONS:usize=16;
const MAX_SOURCE_REF:usize=224;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ExpectedDeliverable{
    SourceReferences,
    StructuredFindings,
    IndependentWitness,
    HumanDecision,
    ArtifactReference,
    Explanation,
}
impl ExpectedDeliverable{
    fn code(self)->&'static str{
        match self{
            Self::SourceReferences=>"SOURCE_REFERENCES",
            Self::StructuredFindings=>"STRUCTURED_FINDINGS",
            Self::IndependentWitness=>"INDEPENDENT_WITNESS",
            Self::HumanDecision=>"HUMAN_DECISION",
            Self::ArtifactReference=>"ARTIFACT_REFERENCE",
            Self::Explanation=>"EXPLANATION",
        }
    }
    fn parse(s:&str)->Option<Self>{
        match s{
            "SOURCE_REFERENCES"=>Some(Self::SourceReferences),
            "STRUCTURED_FINDINGS"=>Some(Self::StructuredFindings),
            "INDEPENDENT_WITNESS"=>Some(Self::IndependentWitness),
            "HUMAN_DECISION"=>Some(Self::HumanDecision),
            "ARTIFACT_REFERENCE"=>Some(Self::ArtifactReference),
            "EXPLANATION"=>Some(Self::Explanation),
            _=>None
        }
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum RevisionReason{
    Initial,GoalChanged,AssumptionsChanged,DependenciesChanged,
    DeliverablesChanged,ScopeChanged,Correction,
}
impl RevisionReason{
    fn code(self)->&'static str{
        match self{
            Self::Initial=>"INITIAL",Self::GoalChanged=>"GOAL_CHANGED",
            Self::AssumptionsChanged=>"ASSUMPTIONS_CHANGED",
            Self::DependenciesChanged=>"DEPENDENCIES_CHANGED",
            Self::DeliverablesChanged=>"DELIVERABLES_CHANGED",
            Self::ScopeChanged=>"SCOPE_CHANGED",
            Self::Correction=>"CORRECTION",
        }
    }
    fn parse(s:&str)->Option<Self>{
        match s{
            "INITIAL"=>Some(Self::Initial),
            "GOAL_CHANGED"=>Some(Self::GoalChanged),
            "ASSUMPTIONS_CHANGED"=>Some(Self::AssumptionsChanged),
            "DEPENDENCIES_CHANGED"=>Some(Self::DependenciesChanged),
            "DELIVERABLES_CHANGED"=>Some(Self::DeliverablesChanged),
            "SCOPE_CHANGED"=>Some(Self::ScopeChanged),
            "CORRECTION"=>Some(Self::Correction),
            _=>None
        }
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RevisionStep{
    pub kind:PlanStepKind,
    pub depends_on:Option<u8>,
    /// Stable expected-output ID; this is not evidence that output exists.
    pub deliverable_id:String,
    pub expected:ExpectedDeliverable,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RevisionProposal{
    pub goal_id:String,
    pub goal_event_id:String,
    /// Stable ANSF source locators only. Their contents are never copied here.
    pub assumption_sources:Vec<String>,
    pub steps:Vec<RevisionStep>,
    pub reason:RevisionReason,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum HumanDecision{Approve,Reject}
impl HumanDecision{
    fn code(self)->&'static str{
        match self{
            Self::Approve=>"APPROVE_PLAN_ONLY",Self::Reject=>"REJECT_PLAN",
        }
    }
}
/// A real host must verify that these references are still accessible in the
/// user's original conversation scope. Parseable references ALONE do not pass.
/// Model-proposed citation strings never confer this permission.
pub trait HostPlanReferenceVerifier:Send+Sync{
    fn goal_event_belongs_to_chat(
        &self,conversation_id:&str,goal_id:&str,goal_event_id:&str
    )->bool;
    fn assumption_source_is_readable(
        &self,conversation_id:&str,stable_ansf_source_id:&str
    )->bool;
}
/// The saved human-turn verifier alone cannot prove that its text explicitly
/// approved THIS SHA-256 revision. The host must match a second durable,
/// explicit review action bound to the exact revision hash and decision.
/// No current Lu WebView/model call may manufacture this authorization.
pub trait HostPlanReviewVerifier:Send+Sync{
    fn has_exact_human_decision(
        &self,conversation_id:&str,review_turn_id:&str,
        plan_id:&str,revision_sha256:&str,decision:HumanDecision,
    )->bool;
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum RevisionError{
    Database(String),Poisoned,InvalidId,InvalidProposal,
    UnverifiedHumanTurn,UnverifiedSource,UnverifiedReview,
    UnknownPlan,WrongOrigin,StaleParent,DuplicateConflict,
    InvalidTransition,CorruptHistory,LimitReached,
}
impl From<rusqlite::Error> for RevisionError{
    fn from(e:rusqlite::Error)->Self{Self::Database(e.to_string())}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum RecordState{New,AlreadyRecorded}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AppendReceipt{
    pub revision_index:u8,
    pub revision_sha256:String,
    pub record_state:RecordState,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RevisionSnapshot{
    pub revision_index:u8,
    pub revision_id:String,
    pub source_human_turn_id:String,
    pub goal_id:String,
    pub goal_event_id:String,
    pub assumption_sources:Vec<String>,
    pub steps:Vec<RevisionStep>,
    pub reason:RevisionReason,
    pub parent_sha256:Option<String>,
    pub revision_sha256:String,
    /// PENDING_HUMAN_REVIEW, APPROVED_PLAN_ONLY, or REJECTED_PLAN.
    pub review_state:&'static str,
    pub review_turn_id:Option<String>,
    pub execution_authority:&'static str,
}
pub struct AgentPlanRevisionStore{db:Mutex<Connection>}
fn exact(id:&str,prefix:&str)->bool{
    id.len()==prefix.len()+32&&id.starts_with(prefix)
        &&id[prefix.len()..].bytes().all(|b|
            b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn digest_put(hash:&mut Sha256,bytes:&[u8]){
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn hash_revision(
    plan:&str,revision_id:&str,source_turn:&str,
    parent:Option<&str>,proposal:&RevisionProposal,
)->String{
    let mut hash=Sha256::new();
    digest_put(&mut hash,b"Lu AGENT-01B plan revision V1");
    for text in [
        plan,revision_id,source_turn,parent.unwrap_or(""),
        &proposal.goal_id,&proposal.goal_event_id,proposal.reason.code()
    ]{digest_put(&mut hash,text.as_bytes())}
    digest_put(&mut hash,&(proposal.assumption_sources.len() as u32).to_le_bytes());
    for source in &proposal.assumption_sources{
        digest_put(&mut hash,source.as_bytes())
    }
    digest_put(&mut hash,&(proposal.steps.len() as u32).to_le_bytes());
    for step in &proposal.steps{
        digest_put(&mut hash,step.kind.as_str().as_bytes());
        digest_put(&mut hash,&[step.depends_on.unwrap_or(u8::MAX)]);
        digest_put(&mut hash,step.deliverable_id.as_bytes());
        digest_put(&mut hash,step.expected.code().as_bytes());
    }
    format!("{:x}",hash.finalize())
}
fn valid_sha(sha:&str)->bool{
    sha.len()==64&&sha.bytes().all(|b|
        b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn valid_proposal(p:&RevisionProposal)->bool{
    if !exact(&p.goal_id,"goal-")||!exact(&p.goal_event_id,"m-")
        ||p.steps.is_empty()||p.steps.len()>MAX_STEPS
        ||p.assumption_sources.len()>MAX_ASSUMPTIONS{
        return false
    }
    let mut assumption_ids=BTreeSet::new();
    for source in &p.assumption_sources{
        if source.len()>MAX_SOURCE_REF
            ||AnsfSourceId::parse(source).is_err()
            ||!assumption_ids.insert(source.as_str()){return false}
    }
    let mut deliverable_ids=BTreeSet::new();
    p.steps.iter().enumerate().all(|(i,s)|
        s.depends_on.is_none_or(|dep|(dep as usize)<i)
        &&exact(&s.deliverable_id,"d-")
        &&deliverable_ids.insert(s.deliverable_id.as_str()))
}
fn plan_origin(
    db:&Connection,plan:&str,chat:&str
)->Result<(String,String),RevisionError>{
    let row:Option<(String,String,String)>=db.query_row(
        "SELECT conversation_id,human_turn_id,state
         FROM lu_agent_plans WHERE plan_id=?1",params![plan],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
    ).optional()?;
    let Some((original_chat,turn,state))=row else{
        return Err(RevisionError::UnknownPlan)
    };
    if original_chat!=chat{return Err(RevisionError::WrongOrigin)}
    if !exact(&turn,"g-")||!matches!(state.as_str(),
        "DRAFT"|"OBSERVATIONS_ONLY"|"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
        return Err(RevisionError::CorruptHistory)
    }
    Ok((original_chat,turn))
}
fn read_history(db:&Connection,plan:&str)->Result<Vec<RevisionSnapshot>,RevisionError>{
    let mut statement=db.prepare(
        "SELECT revision_index,revision_id,source_human_turn_id,
                goal_id,goal_event_id,reason,parent_sha256,revision_sha256
         FROM lu_agent_plan_revisions
         WHERE plan_id=?1 ORDER BY revision_index LIMIT 65"
    )?;
    let rows=statement.query_map(params![plan],|r|Ok((
        r.get::<_,i64>(0)?,r.get::<_,String>(1)?,
        r.get::<_,String>(2)?,r.get::<_,String>(3)?,
        r.get::<_,String>(4)?,r.get::<_,String>(5)?,
        r.get::<_,Option<String>>(6)?,r.get::<_,String>(7)?
    )))?;
    let mut revisions=Vec::new();
    let mut expected_parent:Option<String>=None;
    for row in rows{
        let (ordinal,id,turn,goal_id,goal_event_id,reason,parent,sha)=row?;
        if ordinal!=revisions.len() as i64+1||revisions.len()>=MAX_REVISIONS
            ||!exact(&id,"v-")||!exact(&turn,"g-")
            ||parent!=expected_parent||!valid_sha(&sha){
            return Err(RevisionError::CorruptHistory)
        }
        let Some(reason)=RevisionReason::parse(&reason) else{
            return Err(RevisionError::CorruptHistory)
        };
        if (ordinal==1)!=(reason==RevisionReason::Initial){
            return Err(RevisionError::CorruptHistory)
        }
        let mut q=db.prepare(
            "SELECT source_id FROM lu_agent_plan_revision_assumptions
             WHERE plan_id=?1 AND revision_index=?2 ORDER BY ordinal"
        )?;
        let mut assumptions=Vec::new();
        let matches=q.query_map(params![plan,ordinal],|r|r.get::<_,String>(0))?;
        for source in matches{assumptions.push(source?);}
        let mut q=db.prepare(
            "SELECT step_index,kind,depends_on,deliverable_id,expected
             FROM lu_agent_plan_revision_steps
             WHERE plan_id=?1 AND revision_index=?2 ORDER BY step_index"
        )?;
        let mut steps=Vec::new();
        let rows=q.query_map(params![plan,ordinal],|r|Ok((
            r.get::<_,i64>(0)?,r.get::<_,String>(1)?,
            r.get::<_,Option<i64>>(2)?,r.get::<_,String>(3)?,
            r.get::<_,String>(4)?
        )))?;
        for row in rows{
            let (index,kind,dep,deliverable,expected)=row?;
            if index!=steps.len() as i64{return Err(RevisionError::CorruptHistory)}
            let dependency=dep.map(|v|u8::try_from(v)
                .map_err(|_|RevisionError::CorruptHistory)).transpose()?;
            let Some(kind)=PlanStepKind::parse(&kind) else{
                return Err(RevisionError::CorruptHistory)
            };
            let Some(expected)=ExpectedDeliverable::parse(&expected) else{
                return Err(RevisionError::CorruptHistory)
            };
            steps.push(RevisionStep{kind,depends_on:dependency,
                deliverable_id:deliverable,expected});
        }
        let proposal=RevisionProposal{
            goal_id:goal_id.clone(),goal_event_id:goal_event_id.clone(),
            assumption_sources:assumptions.clone(),steps:steps.clone(),reason
        };
        if !valid_proposal(&proposal)
            ||hash_revision(plan,&id,&turn,parent.as_deref(),&proposal)!=sha{
            return Err(RevisionError::CorruptHistory)
        }
        let mut reviews=db.prepare(
            "SELECT human_turn_id,decision FROM lu_agent_plan_revision_reviews
             WHERE plan_id=?1 AND revision_index=?2 LIMIT 2"
        )?;
        let rows=reviews.query_map(params![plan,ordinal],
            |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
        let mut human_review=None;
        for record in rows{
            if human_review.is_some(){return Err(RevisionError::CorruptHistory)}
            human_review=Some(record?);
        }
        let (review_state,review_turn)=match human_review{
            None=>("PENDING_HUMAN_REVIEW",None),
            Some((turn,decision))=>{
                if !exact(&turn,"g-")||turn==id{
                    return Err(RevisionError::CorruptHistory)
                }
                match decision.as_str(){
                    "APPROVE_PLAN_ONLY"=>("APPROVED_PLAN_ONLY",Some(turn)),
                    "REJECT_PLAN"=>("REJECTED_PLAN",Some(turn)),
                    _=>return Err(RevisionError::CorruptHistory)
                }
            }
        };
        revisions.push(RevisionSnapshot{
            revision_index:ordinal as u8,revision_id:id,
            source_human_turn_id:turn,
            goal_id,goal_event_id,assumption_sources:assumptions,
            steps,reason,parent_sha256:parent,
            revision_sha256:sha.clone(),review_state,review_turn_id:review_turn,
            execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"
        });
        expected_parent=Some(sha);
    }
    Ok(revisions)
}
impl AgentPlanRevisionStore{
    /// Open the same host-owned SQLite file as AGENT-01A. This does not
    /// create a plan or a trusted human turn if either is absent.
    pub fn open(path:&Path)->Result<Self,RevisionError>{
        let db=Connection::open(path)?;
        db.pragma_update(None,"foreign_keys","ON")?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_agent_plan_revisions(
               plan_id TEXT NOT NULL REFERENCES lu_agent_plans(plan_id),
               revision_index INTEGER NOT NULL CHECK(revision_index BETWEEN 1 AND 64),
               revision_id TEXT NOT NULL UNIQUE,
               source_human_turn_id TEXT NOT NULL,
               goal_id TEXT NOT NULL,
               goal_event_id TEXT NOT NULL,
               reason TEXT NOT NULL CHECK(reason IN
                 ('INITIAL','GOAL_CHANGED','ASSUMPTIONS_CHANGED',
                  'DEPENDENCIES_CHANGED','DELIVERABLES_CHANGED',
                  'SCOPE_CHANGED','CORRECTION')),
               parent_sha256 TEXT,
               revision_sha256 TEXT NOT NULL CHECK(length(revision_sha256)=64),
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(plan_id,revision_index)
             );
             CREATE TABLE IF NOT EXISTS lu_agent_plan_revision_steps(
               plan_id TEXT NOT NULL,
               revision_index INTEGER NOT NULL,
               step_index INTEGER NOT NULL CHECK(step_index BETWEEN 0 AND 31),
               kind TEXT NOT NULL,
               depends_on INTEGER,
               deliverable_id TEXT NOT NULL,
               expected TEXT NOT NULL,
               PRIMARY KEY(plan_id,revision_index,step_index),
               FOREIGN KEY(plan_id,revision_index)
                 REFERENCES lu_agent_plan_revisions(plan_id,revision_index)
             );
             CREATE TABLE IF NOT EXISTS lu_agent_plan_revision_assumptions(
               plan_id TEXT NOT NULL,
               revision_index INTEGER NOT NULL,
               ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 15),
               source_id TEXT NOT NULL,
               PRIMARY KEY(plan_id,revision_index,ordinal),
               FOREIGN KEY(plan_id,revision_index)
                 REFERENCES lu_agent_plan_revisions(plan_id,revision_index)
             );
             CREATE TABLE IF NOT EXISTS lu_agent_plan_revision_reviews(
               plan_id TEXT NOT NULL,
               revision_index INTEGER NOT NULL,
               human_turn_id TEXT NOT NULL,
               decision TEXT NOT NULL CHECK(decision IN
                 ('APPROVE_PLAN_ONLY','REJECT_PLAN')),
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(plan_id,revision_index),
               FOREIGN KEY(plan_id,revision_index)
                 REFERENCES lu_agent_plan_revisions(plan_id,revision_index)
             );
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_no_update
               BEFORE UPDATE ON lu_agent_plan_revisions
               BEGIN SELECT RAISE(ABORT,'Immutable plan revision'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_no_delete
               BEFORE DELETE ON lu_agent_plan_revisions
               BEGIN SELECT RAISE(ABORT,'Immutable plan revision'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_steps_no_update
               BEFORE UPDATE ON lu_agent_plan_revision_steps
               BEGIN SELECT RAISE(ABORT,'Immutable revision steps'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_steps_no_delete
               BEFORE DELETE ON lu_agent_plan_revision_steps
               BEGIN SELECT RAISE(ABORT,'Immutable revision steps'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_sources_no_update
               BEFORE UPDATE ON lu_agent_plan_revision_assumptions
               BEGIN SELECT RAISE(ABORT,'Immutable assumption refs'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_sources_no_delete
               BEFORE DELETE ON lu_agent_plan_revision_assumptions
               BEGIN SELECT RAISE(ABORT,'Immutable assumption refs'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_review_no_update
               BEFORE UPDATE ON lu_agent_plan_revision_reviews
               BEGIN SELECT RAISE(ABORT,'Immutable human review'); END;
             CREATE TRIGGER IF NOT EXISTS lu_agent_rev_review_no_delete
               BEFORE DELETE ON lu_agent_plan_revision_reviews
               BEGIN SELECT RAISE(ABORT,'Immutable human review'); END;"
        )?;
        Ok(Self{db:Mutex::new(db)})
    }
    /// The host can record a model-produced proposal only as DRAFT.
    /// The saved human turn authenticates provenance, NOT approval. References
    /// are checked independently by the host. expected_parent_sha256 is a
    /// compare-and-swap guard against concurrent assumption/goal changes.
    #[allow(clippy::too_many_arguments)]
    pub fn append_proposal(
        &self,plan:&str,chat:&str,source_turn:&str,revision_id:&str,
        expected_parent_sha256:Option<&str>,proposal:&RevisionProposal,
        human:&dyn TrustedUserTurnVerifier,
        source:&dyn HostPlanReferenceVerifier,
    )->Result<AppendReceipt,RevisionError>{
        if !exact(plan,"p-")||!exact(chat,"c-")
            ||!exact(source_turn,"g-")||!exact(revision_id,"v-"){
            return Err(RevisionError::InvalidId)
        }
        if !valid_proposal(proposal)
            ||expected_parent_sha256.is_some_and(|s|!valid_sha(s)){
            return Err(RevisionError::InvalidProposal)
        }
        if !human.verify_user_turn(chat,source_turn){
            return Err(RevisionError::UnverifiedHumanTurn)
        }
        if !source.goal_event_belongs_to_chat(
            chat,&proposal.goal_id,&proposal.goal_event_id)
            ||proposal.assumption_sources.iter().any(|s|
                !source.assumption_source_is_readable(chat,s)){
            return Err(RevisionError::UnverifiedSource)
        }
        let mut db=self.db.lock().map_err(|_|RevisionError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (_,original_turn)=plan_origin(&tx,plan,chat)?;
        let recorded=read_history(&tx,plan)?;
        if let Some(existing)=recorded.iter().find(|r|r.revision_id==revision_id){
            let expected=hash_revision(
                plan,revision_id,source_turn,existing.parent_sha256.as_deref(),
                proposal
            );
            if existing.revision_sha256!=expected
                ||existing.parent_sha256.as_deref()!=expected_parent_sha256{
                return Err(RevisionError::DuplicateConflict)
            }
            return Ok(AppendReceipt{revision_index:existing.revision_index,
                revision_sha256:existing.revision_sha256.clone(),
                record_state:RecordState::AlreadyRecorded})
        }
        let last=recorded.last();
        if last.is_some_and(|s|s.revision_sha256.as_str()!=
            expected_parent_sha256.unwrap_or("")){
            return Err(RevisionError::StaleParent)
        }
        if last.is_none()&&expected_parent_sha256.is_some(){
            return Err(RevisionError::StaleParent)
        }
        if recorded.len()>=MAX_REVISIONS{return Err(RevisionError::LimitReached)}
        if recorded.is_empty(){
            if proposal.reason!=RevisionReason::Initial||source_turn!=original_turn{
                return Err(RevisionError::InvalidProposal)
            }
            let mut stmt=tx.prepare(
                "SELECT step_index,kind,depends_on FROM lu_agent_plan_steps
                 WHERE plan_id=?1 ORDER BY step_index"
            )?;
            let rows=stmt.query_map(params![plan],|r|Ok((
                r.get::<_,i64>(0)?,r.get::<_,String>(1)?,
                r.get::<_,Option<i64>>(2)?
            )))?;
            let mut baseline=Vec::new();
            for r in rows{baseline.push(r?);}
            if baseline.len()!=proposal.steps.len()
                ||!baseline.iter().zip(&proposal.steps).enumerate().all(
                    |(i,(row,step))|row.0==i as i64
                    &&row.1==step.kind.as_str()
                    &&row.2==step.depends_on.map(i64::from)){
                return Err(RevisionError::InvalidProposal)
            }
        }else if proposal.reason==RevisionReason::Initial{
            return Err(RevisionError::InvalidProposal)
        }
        let parent=last.map(|x|x.revision_sha256.as_str());
        let digest=hash_revision(plan,revision_id,source_turn,parent,proposal);
        let n=(recorded.len()+1) as i64;
        tx.execute(
            "INSERT INTO lu_agent_plan_revisions
             (plan_id,revision_index,revision_id,source_human_turn_id,
              goal_id,goal_event_id,reason,parent_sha256,revision_sha256)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![plan,n,revision_id,source_turn,proposal.goal_id,
                proposal.goal_event_id,proposal.reason.code(),parent,digest]
        )?;
        for (i,step) in proposal.steps.iter().enumerate(){
            tx.execute(
                "INSERT INTO lu_agent_plan_revision_steps
                 (plan_id,revision_index,step_index,kind,depends_on,
                  deliverable_id,expected)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![plan,n,i as i64,step.kind.as_str(),
                    step.depends_on.map(i64::from),
                    step.deliverable_id,step.expected.code()]
            )?;
        }
        for (i,source) in proposal.assumption_sources.iter().enumerate(){
            tx.execute(
                "INSERT INTO lu_agent_plan_revision_assumptions
                 (plan_id,revision_index,ordinal,source_id)
                 VALUES(?1,?2,?3,?4)",
                params![plan,n,i as i64,source]
            )?;
        }
        tx.commit()?;
        Ok(AppendReceipt{revision_index:n as u8,revision_sha256:digest,
            record_state:RecordState::New})
    }
    /// A distinct, persisted human turn must explicitly review the EXACT
    /// proposal digest through a separate trusted host decision source.
    /// APPROVED_PLAN_ONLY never grants tools, steps or worker lifetimes.
    pub fn record_review(
        &self,plan:&str,chat:&str,review_turn:&str,
        revision_sha256:&str,decision:HumanDecision,
        human:&dyn TrustedUserTurnVerifier,
        reviewed:&dyn HostPlanReviewVerifier,
    )->Result<RecordState,RevisionError>{
        if !exact(plan,"p-")||!exact(chat,"c-")
            ||!exact(review_turn,"g-")||!valid_sha(revision_sha256){
            return Err(RevisionError::InvalidId)
        }
        if !human.verify_user_turn(chat,review_turn){
            return Err(RevisionError::UnverifiedHumanTurn)
        }
        if !reviewed.has_exact_human_decision(
            chat,review_turn,plan,revision_sha256,decision){
            return Err(RevisionError::UnverifiedReview)
        }
        let mut db=self.db.lock().map_err(|_|RevisionError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        plan_origin(&tx,plan,chat)?;
        let history=read_history(&tx,plan)?;
        let current=history.last().ok_or(RevisionError::InvalidTransition)?;
        if current.revision_sha256!=revision_sha256
            ||review_turn==current.source_human_turn_id{
            return Err(RevisionError::StaleParent)
        }
        if let Some(old)=current.review_turn_id.as_deref(){
            if old==review_turn && current.review_state==match decision{
                HumanDecision::Approve=>"APPROVED_PLAN_ONLY",
                HumanDecision::Reject=>"REJECTED_PLAN"
            }{return Ok(RecordState::AlreadyRecorded)}
            return Err(RevisionError::DuplicateConflict)
        }
        tx.execute(
            "INSERT INTO lu_agent_plan_revision_reviews
             (plan_id,revision_index,human_turn_id,decision)
             VALUES(?1,?2,?3,?4)",
            params![plan,current.revision_index as i64,review_turn,decision.code()]
        )?;
        tx.commit()?;
        Ok(RecordState::New)
    }
    /// The complete bounded, hash-checked history; earlier revisions and
    /// reviews are never erased when a new model suggestion arrives.
    pub fn history(
        &self,plan:&str,chat:&str
    )->Result<Vec<RevisionSnapshot>,RevisionError>{
        if !exact(plan,"p-")||!exact(chat,"c-"){
            return Err(RevisionError::InvalidId)
        }
        let db=self.db.lock().map_err(|_|RevisionError::Poisoned)?;
        plan_origin(&db,plan,chat)?;
        read_history(&db,plan)
    }
    pub fn latest(
        &self,plan:&str,chat:&str
    )->Result<Option<RevisionSnapshot>,RevisionError>{
        Ok(self.history(plan,chat)?.pop())
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::agent_plan_ledger::{AgentPlanLedger,PlanStepSpec};
    use std::time::{SystemTime,UNIX_EPOCH};
    struct Human{granted:bool}
    impl TrustedUserTurnVerifier for Human{
        fn verify_user_turn(&self,_chat:&str,_turn:&str)->bool{self.granted}
    }
    struct Sources{allowed:bool}
    impl HostPlanReferenceVerifier for Sources{
        fn goal_event_belongs_to_chat(&self,_chat:&str,_goal:&str,_event:&str)->bool{
            self.allowed
        }
        fn assumption_source_is_readable(&self,_chat:&str,_source:&str)->bool{
            self.allowed
        }
    }
    struct Review{allowed:bool}
    impl HostPlanReviewVerifier for Review{
        fn has_exact_human_decision(
            &self,_chat:&str,_turn:&str,_plan:&str,_hash:&str,_decision:HumanDecision
        )->bool{self.allowed}
    }
    fn id(prefix:&str,n:u8)->String{format!("{prefix}{n:032x}")}
    fn fixture()->(std::path::PathBuf,String,String,String){
        let db=std::env::temp_dir().join(format!(
            "lu-agent01b-{}-{}.sqlite",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let (p,c,t)=(id("p-",1),id("c-",2),id("g-",3));
        let ledger=AgentPlanLedger::open(&db).unwrap();
        ledger.register(&p,&c,&t,&[
            PlanStepSpec{kind:PlanStepKind::CollectSource,depends_on:None},
            PlanStepSpec{kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0)},
            PlanStepSpec{kind:PlanStepKind::Present,depends_on:Some(1)},
        ],&Human{granted:true}).unwrap();
        drop(ledger);
        (db,p,c,t)
    }
    fn proposal(goal:u8,event:u8)->RevisionProposal{
        RevisionProposal{
            goal_id:id("goal-",goal),goal_event_id:id("m-",event),
            assumption_sources:Vec::new(),
            reason:RevisionReason::Initial,
            steps:vec![
                RevisionStep{kind:PlanStepKind::CollectSource,depends_on:None,
                    deliverable_id:id("d-",1),expected:ExpectedDeliverable::SourceReferences},
                RevisionStep{kind:PlanStepKind::IndependentlyCheck,depends_on:Some(0),
                    deliverable_id:id("d-",2),expected:ExpectedDeliverable::IndependentWitness},
                RevisionStep{kind:PlanStepKind::Present,depends_on:Some(1),
                    deliverable_id:id("d-",3),expected:ExpectedDeliverable::Explanation},
            ]
        }
    }
    #[test]
    fn initial_revision_survives_restart_and_hash_chain_rejects_stale_parent(){
        let (path,p,c,t)=fixture();
        let a=proposal(1,1);
        let receipt={
            let store=AgentPlanRevisionStore::open(&path).unwrap();
            let r=store.append_proposal(&p,&c,&t,&id("v-",4),None,&a,
                &Human{granted:true},&Sources{allowed:true}).unwrap();
            assert_eq!(r.record_state,RecordState::New);
            assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",4),None,&a,
                &Human{granted:true},&Sources{allowed:true}).unwrap().record_state,
                RecordState::AlreadyRecorded);
            r
        };
        let store=AgentPlanRevisionStore::open(&path).unwrap();
        assert_eq!(store.latest(&p,&c).unwrap().unwrap().review_state,
            "PENDING_HUMAN_REVIEW");
        let mut b=proposal(1,2);
        b.reason=RevisionReason::GoalChanged;
        assert_eq!(store.append_proposal(&p,&c,&id("g-",5),&id("v-",6),
            Some(&"0".repeat(64)),&b,&Human{granted:true},
            &Sources{allowed:true}),Err(RevisionError::StaleParent));
        let second=store.append_proposal(&p,&c,&id("g-",5),&id("v-",6),
            Some(&receipt.revision_sha256),&b,&Human{granted:true},
            &Sources{allowed:true}).unwrap();
        assert_eq!(second.revision_index,2);
        assert_eq!(store.history(&p,&c).unwrap().len(),2);
        assert_eq!(store.latest(&p,&c).unwrap().unwrap().parent_sha256,
            Some(receipt.revision_sha256));
        drop(store);let _=std::fs::remove_file(path);
    }
    #[test]
    fn human_turn_and_reference_checks_are_not_model_authority(){
        let (path,p,c,t)=fixture();let a=proposal(1,1);
        let store=AgentPlanRevisionStore::open(&path).unwrap();
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&a,
            &Human{granted:false},&Sources{allowed:true}),
            Err(RevisionError::UnverifiedHumanTurn));
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&a,
            &Human{granted:true},&Sources{allowed:false}),
            Err(RevisionError::UnverifiedSource));
        assert_eq!(store.append_proposal(&p,&id("c-",99),&t,&id("v-",1),
            None,&a,&Human{granted:true},&Sources{allowed:true}),
            Err(RevisionError::WrongOrigin));
        let bad=RevisionProposal{goal_id:"../escape".into(),..a.clone()};
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&bad,
            &Human{granted:true},&Sources{allowed:true}),
            Err(RevisionError::InvalidProposal));
        let mut dup=a.clone();
        dup.steps[1].deliverable_id=id("d-",1);
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&dup,
            &Human{granted:true},&Sources{allowed:true}),
            Err(RevisionError::InvalidProposal));
        drop(store);let _=std::fs::remove_file(path);
    }
    #[test]
    fn exact_human_review_does_not_grant_execution_or_rewrite_revision(){
        let (path,p,c,t)=fixture();
        let store=AgentPlanRevisionStore::open(&path).unwrap();
        let r=store.append_proposal(&p,&c,&t,&id("v-",1),None,&proposal(1,1),
            &Human{granted:true},&Sources{allowed:true}).unwrap();
        let review_turn=id("g-",10);
        assert_eq!(store.record_review(&p,&c,&review_turn,&r.revision_sha256,
            HumanDecision::Approve,&Human{granted:true},&Review{allowed:false}),
            Err(RevisionError::UnverifiedReview));
        assert_eq!(store.record_review(&p,&c,&t,&r.revision_sha256,
            HumanDecision::Approve,&Human{granted:true},&Review{allowed:true}),
            Err(RevisionError::StaleParent));
        assert_eq!(store.record_review(&p,&c,&review_turn,&r.revision_sha256,
            HumanDecision::Approve,&Human{granted:true},&Review{allowed:true}),
            Ok(RecordState::New));
        assert_eq!(store.record_review(&p,&c,&review_turn,&r.revision_sha256,
            HumanDecision::Approve,&Human{granted:true},&Review{allowed:true}),
            Ok(RecordState::AlreadyRecorded));
        let old=store.latest(&p,&c).unwrap().unwrap();
        assert_eq!(old.review_state,"APPROVED_PLAN_ONLY");
        assert_eq!(old.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(store.record_review(&p,&c,&id("g-",11),&r.revision_sha256,
            HumanDecision::Reject,&Human{granted:true},&Review{allowed:true}),
            Err(RevisionError::DuplicateConflict));
        let db=Connection::open(&path).unwrap();
        assert!(db.execute("UPDATE lu_agent_plan_revisions SET goal_id='spoof'
            WHERE plan_id=?1",params![p]).is_err());
        assert!(db.execute("DELETE FROM lu_agent_plan_revision_reviews
            WHERE plan_id=?1",params![p]).is_err());
        drop(db);drop(store);let _=std::fs::remove_file(path);
    }
    #[test]
    fn sources_and_revision_payload_validate_immutable_dependencies(){
        let (path,p,c,t)=fixture();
        let store=AgentPlanRevisionStore::open(&path).unwrap();
        let mut a=proposal(1,1);
        a.steps[1].depends_on=Some(2);
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&a,
            &Human{granted:true},&Sources{allowed:true}),
            Err(RevisionError::InvalidProposal));
        a=proposal(1,1);
        let source=AnsfSourceId::new("book-1",1).unwrap().stable_source_id().unwrap();
        a.assumption_sources.push(source.clone());
        a.assumption_sources.push(source);
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&a,
            &Human{granted:true},&Sources{allowed:true}),
            Err(RevisionError::InvalidProposal));
        a.assumption_sources.pop();
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&a,
            &Human{granted:true},&Sources{allowed:false}),
            Err(RevisionError::UnverifiedSource));
        let r=store.append_proposal(&p,&c,&t,&id("v-",1),None,&a,
            &Human{granted:true},&Sources{allowed:true}).unwrap();
        assert_eq!(store.latest(&p,&c).unwrap().unwrap()
            .assumption_sources.len(),1);
        let mut b=a.clone();b.goal_event_id=id("m-",2);
        assert_eq!(store.append_proposal(&p,&c,&t,&id("v-",1),None,&b,
            &Human{granted:true},&Sources{allowed:true}),
            Err(RevisionError::DuplicateConflict));
        assert_eq!(r.revision_index,1);
        drop(store);let _=std::fs::remove_file(path);
    }
}
