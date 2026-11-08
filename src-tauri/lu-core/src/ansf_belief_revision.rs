//! M4-P02/P03: source-anchored claims and conservative belief revision.
 //! Derived immutable rows in ANSF originals. Reading memory never authorizes tools.
use crate::ansf::{AnsfError,AnsfStore};
use crate::continuity::{ContinuityStore,VerifiedEffectEvidence};
use crate::journal::EffectJournal;
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};
use std::{collections::HashSet,time::{SystemTime,UNIX_EPOCH}};

const MAX_SCAN:usize=128;
fn ident(s:&str)->bool{
    (1..=128).contains(&s.len())&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.'))
}
fn bounded(s:&str,max:usize)->bool{
    !s.trim().is_empty()&&s.len()<=max&&!s.contains('\0')
}
fn sha(s:&str)->String{format!("{:x}",Sha256::digest(s.as_bytes()))}
fn now()->Result<u64,AnsfError>{
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)
       .map_err(|_|AnsfError::InvalidInput)?.as_secs())
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Claim{
    pub claim_id:String,
    pub session_id:String,
    pub source_entry_id:i64,
    pub project_id:String,
    pub task_id:String,
    pub subject_key:String,
    pub predicate_key:String,
    pub value:String,
    pub application_version:String,
    pub expires_unix:u64,
    pub supersedes_id:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ClaimReceipt{
    pub checkpoint_id:String,
    pub call_id:String,
    pub tool_id:String,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ClaimState{
    Reported,IndependentlyVerified,Contradicted,Stale,Retracted,Superseded
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AssessedClaim{
    pub claim:Claim,
    pub state:ClaimState,
    pub source_sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
struct Stored{
    claim:Claim,
    kind:String,
    proof:Option<ClaimReceipt>,
    receipt_sha:Option<String>,
    source_sha:String,
    retracted:bool,
    superseded:bool,
}
fn valid(c:&Claim)->bool{
    ident(&c.claim_id)&&ident(&c.session_id)&&c.source_entry_id>0&&
    ident(&c.project_id)&&ident(&c.task_id)&&ident(&c.subject_key)&&
    ident(&c.predicate_key)&&bounded(&c.value,1024)&&
    bounded(&c.application_version,128)&&
    c.supersedes_id.as_ref().is_none_or(|x|ident(x)&&x!=&c.claim_id)
}
pub(crate) fn initialize_claim_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_verified_claims(
           claim_id TEXT PRIMARY KEY NOT NULL,
           session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
           source_entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
           project_id TEXT NOT NULL,task_id TEXT NOT NULL,
           subject_key TEXT NOT NULL,predicate_key TEXT NOT NULL,
           value TEXT NOT NULL,application_version TEXT NOT NULL,
           expires_unix INTEGER NOT NULL,
           supersedes_id TEXT REFERENCES ansf_verified_claims(claim_id),
           kind TEXT NOT NULL CHECK(kind IN ('reported','verified')),
           checkpoint_id TEXT,call_id TEXT,tool_id TEXT,
           receipt_sha256 TEXT,source_sha256 TEXT NOT NULL,
           recorded_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS ansf_claim_scope
         ON ansf_verified_claims(session_id,project_id,task_id,subject_key,predicate_key);
         CREATE TABLE IF NOT EXISTS ansf_claim_retractions(
           claim_id TEXT PRIMARY KEY REFERENCES ansf_verified_claims(claim_id),
           correction_entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
           reason TEXT NOT NULL
         );
         CREATE TRIGGER IF NOT EXISTS ansf_claim_no_update
          BEFORE UPDATE ON ansf_verified_claims BEGIN
          SELECT RAISE(ABORT,'claims are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_claim_no_delete
          BEFORE DELETE ON ansf_verified_claims BEGIN
          SELECT RAISE(ABORT,'claims are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_claim_retract_no_update
          BEFORE UPDATE ON ansf_claim_retractions BEGIN
          SELECT RAISE(ABORT,'claim retractions are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_claim_retract_no_delete
          BEFORE DELETE ON ansf_claim_retractions BEGIN
          SELECT RAISE(ABORT,'claim retractions are immutable'); END;"
    )?;
    Ok(())
}
fn map_stored(r:&rusqlite::Row<'_>)->rusqlite::Result<Stored>{
    let cp:Option<String>=r.get(12)?;
    let call:Option<String>=r.get(13)?;
    let tool:Option<String>=r.get(14)?;
    let proof=match(cp,call,tool){
        (Some(checkpoint_id),Some(call_id),Some(tool_id))=>
            Some(ClaimReceipt{checkpoint_id,call_id,tool_id}),
        (None,None,None)=>None,
        _=>return Err(rusqlite::Error::InvalidQuery)
    };
    Ok(Stored{
        claim:Claim{
            claim_id:r.get(0)?,session_id:r.get(1)?,source_entry_id:r.get(2)?,
            project_id:r.get(3)?,task_id:r.get(4)?,subject_key:r.get(5)?,
            predicate_key:r.get(6)?,value:r.get(7)?,application_version:r.get(8)?,
            expires_unix:r.get::<_,i64>(9)? as u64,supersedes_id:r.get(10)?
        },
        kind:r.get(11)?,proof,receipt_sha:r.get(15)?,source_sha:r.get(16)?,
        retracted:r.get::<_,i64>(17)?!=0,superseded:r.get::<_,i64>(18)?!=0
    })
}
const COLS:&str="c.claim_id,c.session_id,c.source_entry_id,c.project_id,
 c.task_id,c.subject_key,c.predicate_key,c.value,c.application_version,
 c.expires_unix,c.supersedes_id,c.kind,c.checkpoint_id,c.call_id,
 c.tool_id,c.receipt_sha256,c.source_sha256,
 EXISTS(SELECT 1 FROM ansf_claim_retractions x WHERE x.claim_id=c.claim_id),
 EXISTS(SELECT 1 FROM ansf_verified_claims next WHERE next.supersedes_id=c.claim_id)";
impl AnsfStore{
    fn current_claim_source(&self,session:&str,entry:i64)
        ->Result<Option<(String,bool)>,AnsfError>{
        let Some(original)=self.read_entry(session,entry)? else{return Ok(None)};
        if !matches!(original.role.as_str(),"user"|"assistant"){return Ok(None)}
        let Some(head)=self.current_message(session,&original.message_id)? else {
            return Ok(None)
        };
        if head.entry.id!=entry{return Ok(None)}
        Ok(Some((sha(&original.content),original.role=="user")))
    }
    fn claim_receipt_sha(
        &self,c:&Claim,r:&ClaimReceipt,continuity:&ContinuityStore,
        journal:&EffectJournal,verify:&impl Fn(&VerifiedEffectEvidence)->bool
    )->Result<Option<String>,AnsfError>{
        let cp=continuity.latest_for_conversation(
            &c.project_id,&c.task_id,&c.session_id
        ).map_err(|_|AnsfError::InvalidInput)?;
        if cp.as_ref().is_none_or(|p|
            p.checkpoint_id!=r.checkpoint_id||
            !p.source_entry_ids.contains(&c.source_entry_id)||
            !p.blockers.is_empty()
        ){return Ok(None)}
        let effects=continuity.verified_effects_for_checkpoint(&r.checkpoint_id,journal)
            .map_err(|_|AnsfError::InvalidInput)?;
        Ok(effects.into_iter().find(|e|
            e.call_id==r.call_id&&e.tool_id==r.tool_id&&verify(e)
        ).map(|e|e.receipt_sha256))
    }
    fn append_claim(&self,c:&Claim,proof:Option<ClaimReceipt>,
        receipt_sha:Option<String>)->Result<(),AnsfError>{
        let current=now()?;
        if !valid(c)||proof.is_some()!=receipt_sha.is_some()||
            c.expires_unix<=current||
            c.expires_unix>current.saturating_add(365*24*3600){
            return Err(AnsfError::InvalidInput)
        }
        let (source_sha,is_human)=self.current_claim_source(
            &c.session_id,c.source_entry_id
        )?.ok_or(AnsfError::InvalidInput)?;
        if c.supersedes_id.is_some()&&!is_human{return Err(AnsfError::InvalidInput)}
        let kind=if proof.is_some(){"verified"}else{"reported"};
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        initialize_claim_schema(&conn)?;
        let tx=conn.transaction()?;
        if let Some(old)=&c.supersedes_id{
            let parent:Option<(String,String,String,String,String,i64)>=tx.query_row(
                "SELECT session_id,project_id,task_id,subject_key,predicate_key,source_entry_id
                 FROM ansf_verified_claims WHERE claim_id=?1",[old],|r|Ok((
                    r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?
                ))
            ).optional()?;
            match parent{
                Some((s,p,t,sub,pred,entry)) if s==c.session_id&&
                    p==c.project_id&&t==c.task_id&&sub==c.subject_key&&
                    pred==c.predicate_key&&c.source_entry_id>entry=>{},
                _=>return Err(AnsfError::InvalidInput)
            }
        }
        tx.execute(
            "INSERT OR IGNORE INTO ansf_verified_claims(
                claim_id,session_id,source_entry_id,project_id,task_id,
                subject_key,predicate_key,value,application_version,expires_unix,
                supersedes_id,kind,checkpoint_id,call_id,tool_id,receipt_sha256,source_sha256)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![c.claim_id,c.session_id,c.source_entry_id,c.project_id,c.task_id,
                c.subject_key,c.predicate_key,c.value,c.application_version,
                c.expires_unix as i64,c.supersedes_id,kind,
                proof.as_ref().map(|e|e.checkpoint_id.as_str()),
                proof.as_ref().map(|e|e.call_id.as_str()),
                proof.as_ref().map(|e|e.tool_id.as_str()),receipt_sha,source_sha]
        )?;
        let mut saved=tx.query_row(
            &format!("SELECT {COLS} FROM ansf_verified_claims c WHERE c.claim_id=?1"),
            [&c.claim_id],map_stored
        )?;
        // Immutable claim bytes stay idempotent after later retraction/revision.
        saved.retracted=false; saved.superseded=false;
        if saved!=(Stored{claim:c.clone(),kind:kind.to_owned(),
            proof,receipt_sha,source_sha,retracted:false,superseded:false}){
            return Err(AnsfError::InvalidInput)
        }
        tx.commit()?;
        Ok(())
    }
    /// Records source-backed assertion as *reported*, not true.
    pub fn record_reported_claim(&self,c:&Claim)->Result<(),AnsfError>{
        self.append_claim(c,None,None)
    }
    /// Requires independent effect postcondition validation in addition to
    /// the intact APPLIED journal receipt and current task checkpoint.
    pub fn record_verified_claim(&self,c:&Claim,proof:ClaimReceipt,
        continuity:&ContinuityStore,journal:&EffectJournal,
        independent_verify:impl Fn(&VerifiedEffectEvidence)->bool
    )->Result<(),AnsfError>{
        if !valid(c){return Err(AnsfError::InvalidInput)}
        let digest=self.claim_receipt_sha(c,&proof,continuity,journal,&independent_verify)?
            .ok_or(AnsfError::InvalidInput)?;
        self.append_claim(c,Some(proof),Some(digest))
    }
    /// A genuine new current USER original can retract a historical claim.
    pub fn retract_reported_or_verified_claim(&self,claim_id:&str,session_id:&str,
        correction_entry:i64,reason:&str)->Result<(),AnsfError>{
        if !ident(claim_id)||!ident(session_id)||correction_entry<=0||
            !bounded(reason,512){return Err(AnsfError::InvalidInput)}
        let (_,is_human)=self.current_claim_source(session_id,correction_entry)?
            .ok_or(AnsfError::InvalidInput)?;
        if !is_human{return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        initialize_claim_schema(&conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO ansf_claim_retractions(claim_id,correction_entry_id,reason)
             SELECT claim_id,?3,?4 FROM ansf_verified_claims
             WHERE claim_id=?1 AND session_id=?2",
            params![claim_id,session_id,correction_entry,reason]
        )?;
        let saved:Option<(i64,String)>=conn.query_row(
            "SELECT correction_entry_id,reason FROM ansf_claim_retractions
             WHERE claim_id=?1",[claim_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional()?;
        if saved!=Some((correction_entry,reason.to_owned())){
            return Err(AnsfError::InvalidInput)
        }
        Ok(())
    }
    /// Scan the FULL bounded set of claims on one exact subject+predicate.
    /// >128 results fails closed; contradictions are flagged, never voted away.
    pub fn assess_claims(&self,session:&str,project:&str,task:&str,subject:&str,
        predicate:&str,application_version:&str,continuity:&ContinuityStore,
        journal:&EffectJournal,independent_verify:impl Fn(&VerifiedEffectEvidence)->bool
    )->Result<Vec<AssessedClaim>,AnsfError>{
        if !ident(session)||!ident(project)||!ident(task)||!ident(subject)||
            !ident(predicate)||!bounded(application_version,128){
            return Err(AnsfError::InvalidInput)
        }
        let records={
            let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
            initialize_claim_schema(&conn)?;
            let mut stmt=conn.prepare(&format!(
                "SELECT {COLS} FROM ansf_verified_claims c
                 WHERE c.session_id=?1 AND c.project_id=?2 AND c.task_id=?3
                 AND c.subject_key=?4 AND c.predicate_key=?5
                 ORDER BY c.recorded_utc DESC,c.claim_id DESC LIMIT ?6"
            ))?;
            let rows=stmt.query_map(
                params![session,project,task,subject,predicate,(MAX_SCAN+1) as i64],
                map_stored
            )?.collect::<Result<Vec<_>,_>>()?;
            rows
        };
        if records.len()>MAX_SCAN{return Err(AnsfError::InvalidInput)}
        let mut out=Vec::with_capacity(records.len());
        let current=now()?;
        for r in records{
            let c=&r.claim;
            let state=if r.retracted{ClaimState::Retracted}
            else if r.superseded{ClaimState::Superseded}
            else if c.expires_unix<=current||
                c.application_version!=application_version||
                self.current_claim_source(session,c.source_entry_id)?
                    .map(|(hash,_)|hash)!=Some(r.source_sha.clone()){
                ClaimState::Stale
            }else if r.kind=="verified"{
                if let (Some(proof),Some(digest))=(&r.proof,&r.receipt_sha){
                    if self.claim_receipt_sha(c,proof,continuity,journal,
                        &independent_verify)?.as_deref()==Some(digest.as_str()){
                        ClaimState::IndependentlyVerified
                    }else{ClaimState::Stale}
                }else{ClaimState::Stale}
            }else{ClaimState::Reported};
            out.push(AssessedClaim{claim:r.claim,state,source_sha256:r.source_sha});
        }
        let distinct=out.iter().filter(|r|matches!(r.state,
            ClaimState::Reported|ClaimState::IndependentlyVerified)
        ).map(|r|r.claim.value.as_str()).collect::<HashSet<_>>();
        if distinct.len()>1{
            for row in &mut out{
                if matches!(row.state,ClaimState::Reported|ClaimState::IndependentlyVerified){
                    row.state=ClaimState::Contradicted
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::ansf::EntryRole;
    use crate::ansf_revisions::RevisionOrigin;
    use crate::continuity::TaskCheckpoint;
    use crate::contracts::{ToolInvocation,ProtoPayload};
    use std::{fs,path::PathBuf};
    fn fixture()->PathBuf{
        std::env::temp_dir().join(format!("lu-m4-belief-{}-{}",
          std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    fn claim(source:i64,id:&str,value:&str)->Claim{
        Claim{claim_id:id.into(),session_id:"chat-one".into(),
            source_entry_id:source,project_id:"lu".into(),task_id:"memory".into(),
            subject_key:"model".into(),predicate_key:"backend".into(),
            value:value.into(),application_version:"1.0".into(),
            expires_unix:now().unwrap()+3600,supersedes_id:None}
    }
    #[test]
    fn contradictory_reported_claims_are_never_treated_as_verified_and_correction_persists(){
        let path=fixture();fs::create_dir_all(&path).unwrap();
        let db=path.join("ansf.sqlite");
        let book=AnsfStore::open(&db).unwrap();
        book.create_session("chat-one","ws-one","Belief").unwrap();
        book.start_chapter("chat-one","Facts").unwrap();
        let first=book.append_entry("chat-one","one",EntryRole::User,"A","Backend is A").unwrap();
        let second=book.append_entry("chat-one","two",EntryRole::Assistant,"B","Backend is B").unwrap();
        let correction=book.append_entry("chat-one","three",EntryRole::User,"Correction","Revoke B").unwrap();
        let c1=claim(first,"first","A");let c2=claim(second,"second","B");
        let continuity=ContinuityStore::open(&path.join("continuity.sqlite")).unwrap();
        let journal=EffectJournal::open(&path.join("journal.sqlite")).unwrap();
        book.record_reported_claim(&c1).unwrap();
        book.record_reported_claim(&c2).unwrap();
        assert_eq!(book.assess_claims("chat-one","lu","memory","model","backend","1.0",
            &continuity,&journal,|_|false).unwrap().iter().filter(|x|
                x.state==ClaimState::Contradicted).count(),2);
        assert!(book.record_verified_claim(&c2,ClaimReceipt{
            checkpoint_id:"cp".into(),call_id:"call".into(),tool_id:"lu.verify".into()
        },&continuity,&journal,|_|true).is_err());
        book.retract_reported_or_verified_claim("second","chat-one",correction,"Wrong").unwrap();
        book.retract_reported_or_verified_claim("second","chat-one",correction,"Wrong").unwrap();
        assert!(book.retract_reported_or_verified_claim("second","chat-one",correction,"Different").is_err());
        assert_eq!(book.assess_claims("chat-one","lu","memory","model","backend","1.0",
            &continuity,&journal,|_|false).unwrap().iter().filter(|x|
                x.state==ClaimState::Reported).count(),1);
        drop(book);
        let restored=AnsfStore::open(&db).unwrap();
        assert_eq!(restored.assess_claims("chat-one","lu","memory","model","backend","1.0",
            &continuity,&journal,|_|false).unwrap().iter().filter(|x|
                x.state==ClaimState::Retracted).count(),1);
        drop(restored);drop(continuity);drop(journal);fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn verified_claim_rechecks_receipt_and_user_edit_becomes_stale(){
        let path=fixture();fs::create_dir_all(&path).unwrap();
        let book=AnsfStore::open(&path.join("ansf.sqlite")).unwrap();
        book.create_session("chat-one","ws-one","Verified").unwrap();
        book.start_chapter("chat-one","Facts").unwrap();
        let original=book.append_entry("chat-one","one",EntryRole::User,
            "Approved source","The actual worker passed").unwrap();
        let continuity=ContinuityStore::open(&path.join("continuity.sqlite")).unwrap();
        let journal=EffectJournal::open(&path.join("journal.sqlite")).unwrap();
        continuity.append_verified(&TaskCheckpoint{
            checkpoint_id:"cp-one".into(),project_id:"lu".into(),
            task_id:"memory".into(),conversation_id:"chat-one".into(),
            source_entry_ids:vec![original],completed_work:"Verified".into(),
            next_action:"Review".into(),blockers:vec![]
        },&book).unwrap();
        journal.prepare(&ToolInvocation{call_id:"call-one".into(),
            tool_id:"lu.test".into(),workspace_id:Some("ws-one".into()),
            input:ProtoPayload{type_url:"lu.test.Case".into(),bytes:vec![1]}
        }).unwrap();
        journal.mark_executing("call-one").unwrap();
        journal.mark_applied("call-one",b"verified native result").unwrap();
        continuity.link_verified_effect("cp-one","call-one",&journal).unwrap();
        let c=claim(original,"proof-one","passed");
        let proof=ClaimReceipt{checkpoint_id:"cp-one".into(),call_id:"call-one".into(),
            tool_id:"lu.test".into()};
        assert!(book.record_verified_claim(&c,proof.clone(),&continuity,&journal,
            |_|false).is_err());
        book.record_verified_claim(&c,proof,&continuity,&journal,|_|true).unwrap();
        assert_eq!(book.assess_claims("chat-one","lu","memory","model","backend","1.0",
            &continuity,&journal,|_|true).unwrap()[0].state,
            ClaimState::IndependentlyVerified);
        assert_eq!(book.assess_claims("chat-one","lu","memory","model","backend","1.0",
            &continuity,&journal,|_|false).unwrap()[0].state,ClaimState::Stale);
        book.append_edited_entry("chat-one","one","Changed","The worker failed",
            RevisionOrigin::VerifiedUser).unwrap();
        assert_eq!(book.assess_claims("chat-one","lu","memory","model","backend","1.0",
            &continuity,&journal,|_|true).unwrap()[0].state,ClaimState::Stale);
        drop(book);drop(journal);drop(continuity);fs::remove_dir_all(path).unwrap();
    }
}
