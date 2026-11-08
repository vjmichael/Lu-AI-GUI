//! M4-P01: verified procedural memory as a *derived projection* in the
//! original ANSF book. No duplicate transcript, executable recipes, claims
//! sourced from assistant text alone, or historical permission grants.
//! Callers MUST authenticate book ownership and independently attest the
//! real effect outcome; no model-facing write command is registered.
use crate::ansf::{AnsfError,AnsfStore};
use crate::continuity::{ContinuityStore,VerifiedEffectEvidence};
use crate::journal::EffectJournal;
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};

const MAX_STEPS:usize=16;
const MAX_HITS:usize=16;
fn id(s:&str)->bool{
    (1..=128).contains(&s.len()) &&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
fn label(s:&str)->bool{
    !s.trim().is_empty()&&s.len()<=240&&!s.contains('\0')
}
fn text(s:&str)->bool{
    !s.trim().is_empty()&&s.len()<=4096&&!s.contains('\0')
}
fn sha(s:&str)->String{format!("{:x}",Sha256::digest(s.as_bytes()))}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ProcedureCandidate{
    pub procedure_id:String,
    pub project_id:String,
    pub task_id:String,
    pub session_id:String,
    pub source_entry_id:i64,
    pub checkpoint_id:String,
    pub call_id:String,
    pub tool_id:String,
    pub tool_version:String,
    pub application_version:String,
    pub title:String,
    pub prerequisites:Vec<String>,
    pub steps:Vec<String>,
    pub failure_conditions:Vec<String>,
    /// A successor must have its own independently verified new effect.
    pub replaces_id:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VerifiedProcedure{
    pub candidate:ProcedureCandidate,
    pub source_sha256:String,
    pub receipt_sha256:String,
}
fn valid(c:&ProcedureCandidate)->bool{
    id(&c.procedure_id)&&id(&c.project_id)&&id(&c.task_id)&&
    id(&c.session_id)&&c.source_entry_id>0&&id(&c.checkpoint_id)&&
    id(&c.call_id)&&id(&c.tool_id)&&label(&c.tool_version)&&
    label(&c.application_version)&&label(&c.title)&&
    c.prerequisites.len()<=MAX_STEPS&&
    !c.steps.is_empty()&&c.steps.len()<=MAX_STEPS&&
    c.failure_conditions.len()<=MAX_STEPS&&
    c.prerequisites.iter().all(|s|text(s))&&
    c.steps.iter().all(|s|text(s))&&
    c.failure_conditions.iter().all(|s|text(s))&&
    c.replaces_id.as_ref().is_none_or(|s|id(s)&&s!=&c.procedure_id)
}
pub(crate) fn initialize_procedure_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_verified_procedures(
           procedure_id TEXT PRIMARY KEY NOT NULL,
           session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
           source_entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
           project_id TEXT NOT NULL,
           task_id TEXT NOT NULL,
           checkpoint_id TEXT NOT NULL,
           call_id TEXT NOT NULL,
           tool_id TEXT NOT NULL,
           tool_version TEXT NOT NULL,
           application_version TEXT NOT NULL,
           title TEXT NOT NULL,
           prerequisites_json TEXT NOT NULL,
           steps_json TEXT NOT NULL,
           failure_conditions_json TEXT NOT NULL,
           replaces_id TEXT REFERENCES ansf_verified_procedures(procedure_id),
           source_sha256 TEXT NOT NULL,
           receipt_sha256 TEXT NOT NULL,
           recorded_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS ansf_verified_procedures_scope
           ON ansf_verified_procedures(session_id,project_id,task_id,tool_id);
         CREATE TABLE IF NOT EXISTS ansf_procedure_invalidations(
           procedure_id TEXT PRIMARY KEY REFERENCES ansf_verified_procedures(procedure_id),
           correction_entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
           reason TEXT NOT NULL,
           observed_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TRIGGER IF NOT EXISTS ansf_procedures_no_update
           BEFORE UPDATE ON ansf_verified_procedures BEGIN
           SELECT RAISE(ABORT,'verified procedures are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_procedures_no_delete
           BEFORE DELETE ON ansf_verified_procedures BEGIN
           SELECT RAISE(ABORT,'verified procedures are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_procedure_invalidations_no_update
           BEFORE UPDATE ON ansf_procedure_invalidations BEGIN
           SELECT RAISE(ABORT,'procedure invalidations are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_procedure_invalidations_no_delete
           BEFORE DELETE ON ansf_procedure_invalidations BEGIN
           SELECT RAISE(ABORT,'procedure invalidations are immutable'); END;"
    )?;
    Ok(())
}
fn parse_json(json:String)->rusqlite::Result<Vec<String>>{
    serde_json::from_str(&json).map_err(|_|rusqlite::Error::InvalidQuery)
}
fn map_record(r:&rusqlite::Row<'_>)->rusqlite::Result<VerifiedProcedure>{
    Ok(VerifiedProcedure{
        candidate:ProcedureCandidate{
            procedure_id:r.get(0)?,session_id:r.get(1)?,source_entry_id:r.get(2)?,
            project_id:r.get(3)?,task_id:r.get(4)?,checkpoint_id:r.get(5)?,
            call_id:r.get(6)?,tool_id:r.get(7)?,tool_version:r.get(8)?,
            application_version:r.get(9)?,title:r.get(10)?,
            prerequisites:parse_json(r.get(11)?)?,
            steps:parse_json(r.get(12)?)?,
            failure_conditions:parse_json(r.get(13)?)?,
            replaces_id:r.get(14)?,
        },
        source_sha256:r.get(15)?,receipt_sha256:r.get(16)?
    })
}
const FIELDS:&str="procedure_id,session_id,source_entry_id,project_id,task_id,
                    checkpoint_id,call_id,tool_id,tool_version,application_version,
                    title,prerequisites_json,steps_json,failure_conditions_json,
                    replaces_id,source_sha256,receipt_sha256";

impl AnsfStore{
    fn source_is_current(&self,session:&str,entry_id:i64)->Result<Option<String>,AnsfError>{
        let Some(entry)=self.read_entry(session,entry_id)? else {return Ok(None)};
        if !matches!(entry.role.as_str(),"user"|"assistant"){return Ok(None)}
        let Some(head)=self.current_message(session,&entry.message_id)? else {
            return Ok(None)
        };
        if head.entry.id!=entry_id{return Ok(None)}
        Ok(Some(sha(&entry.content)))
    }

    /// Promotion requires a currently sourced task checkpoint, an APPLIED
    /// journal receipt that still matches its stored digest, AND a separate
    /// host-owned independent outcome verifier. Merely returning APPLIED from
    /// a tool is never enough. No procedure is executable by reading this DB.
    pub fn promote_verified_procedure(
        &self,candidate:&ProcedureCandidate,
        continuity:&ContinuityStore,journal:&EffectJournal,
        independent_verify:impl FnOnce(&VerifiedEffectEvidence)->bool
    )->Result<VerifiedProcedure,AnsfError>{
        if !valid(candidate){return Err(AnsfError::InvalidInput)}
        let c=candidate;
        let source_sha=self.source_is_current(&c.session_id,c.source_entry_id)?
            .ok_or(AnsfError::InvalidInput)?;
        let cp=continuity.latest_for_conversation(
            &c.project_id,&c.task_id,&c.session_id
        ).map_err(|_|AnsfError::InvalidInput)?
            .filter(|p|p.checkpoint_id==c.checkpoint_id&&
                p.source_entry_ids.contains(&c.source_entry_id))
            .ok_or(AnsfError::InvalidInput)?;
        if cp.blockers.iter().any(|s|!s.trim().is_empty()){
            return Err(AnsfError::InvalidInput)
        }
        let evidence=continuity.verified_effects_for_checkpoint(
            &c.checkpoint_id,journal
        ).map_err(|_|AnsfError::InvalidInput)?
        .into_iter().find(|v|v.call_id==c.call_id&&v.tool_id==c.tool_id)
        .ok_or(AnsfError::InvalidInput)?;
        if !independent_verify(&evidence){return Err(AnsfError::InvalidInput)}
        let record=VerifiedProcedure{
            candidate:c.clone(),source_sha256:source_sha,
            receipt_sha256:evidence.receipt_sha256
        };
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        if let Some(replaces)=&c.replaces_id{
            let older:Option<(String,String)>=tx.query_row(
                "SELECT session_id,project_id FROM ansf_verified_procedures
                 WHERE procedure_id=?1",[replaces],|r|Ok((r.get(0)?,r.get(1)?))
            ).optional()?;
            if older.as_ref()!=Some(&(c.session_id.clone(),c.project_id.clone())){
                return Err(AnsfError::InvalidInput)
            }
        }
        tx.execute(
            "INSERT OR IGNORE INTO ansf_verified_procedures
             (procedure_id,session_id,source_entry_id,project_id,task_id,
              checkpoint_id,call_id,tool_id,tool_version,application_version,
              title,prerequisites_json,steps_json,failure_conditions_json,
              replaces_id,source_sha256,receipt_sha256)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![c.procedure_id,c.session_id,c.source_entry_id,
                c.project_id,c.task_id,c.checkpoint_id,c.call_id,c.tool_id,
                c.tool_version,c.application_version,c.title,
                serde_json::to_string(&c.prerequisites).map_err(|_|AnsfError::InvalidInput)?,
                serde_json::to_string(&c.steps).map_err(|_|AnsfError::InvalidInput)?,
                serde_json::to_string(&c.failure_conditions).map_err(|_|AnsfError::InvalidInput)?,
                c.replaces_id,record.source_sha256,record.receipt_sha256]
        )?;
        let saved=tx.query_row(
            &format!("SELECT {FIELDS} FROM ansf_verified_procedures WHERE procedure_id=?1"),
            [&c.procedure_id],map_record
        )?;
        if saved!=record{return Err(AnsfError::InvalidInput)}
        tx.commit()?;
        Ok(saved)
    }

    /// Record a new human-confirmed correction as an append-only invalidation.
    /// The trusted caller must authenticate the human and source-book owner.
    pub fn invalidate_verified_procedure(
        &self,procedure_id:&str,session_id:&str,
        correction_entry_id:i64,reason:&str
    )->Result<(),AnsfError>{
        if !id(procedure_id)||!id(session_id)||correction_entry_id<=0||
            !text(reason){return Err(AnsfError::InvalidInput)}
        let correction=self.read_entry(session_id,correction_entry_id)?
            .ok_or(AnsfError::InvalidInput)?;
        if correction.role!="user"||
            self.source_is_current(session_id,correction_entry_id)?.is_none(){
            return Err(AnsfError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let changed=conn.execute(
            "INSERT OR IGNORE INTO ansf_procedure_invalidations
             (procedure_id,correction_entry_id,reason)
             SELECT procedure_id,?3,?4 FROM ansf_verified_procedures
             WHERE procedure_id=?1 AND session_id=?2",
            params![procedure_id,session_id,correction_entry_id,reason]
        )?;
        let saved:Option<(i64,String)>=conn.query_row(
            "SELECT correction_entry_id,reason FROM ansf_procedure_invalidations
             WHERE procedure_id=?1",[procedure_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional()?;
        if changed==0 && saved.is_none(){return Err(AnsfError::InvalidInput)}
        if saved!=Some((correction_entry_id,reason.to_owned())){
            return Err(AnsfError::InvalidInput)
        }
        Ok(())
    }

    /// Exact version-matched and source-current read. Independently rechecks
    /// journal receipt and host validator each time. Missing/stale facts are
    /// omitted rather than silently elevated to verified "knowledge".
    pub fn reusable_verified_procedures(
        &self,session_id:&str,project_id:&str,task_id:&str,
        tool_id:&str,tool_version:&str,application_version:&str,
        limit:usize,continuity:&ContinuityStore,journal:&EffectJournal,
        independent_verify:impl Fn(&VerifiedEffectEvidence)->bool
    )->Result<Vec<VerifiedProcedure>,AnsfError>{
        if !id(session_id)||!id(project_id)||!id(task_id)||!id(tool_id)||
            !label(tool_version)||!label(application_version)||
            limit==0||limit>MAX_HITS{return Err(AnsfError::InvalidInput)}
        let stored={
            let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
            let mut stmt=conn.prepare(
                &format!("SELECT {FIELDS} FROM ansf_verified_procedures p
                    WHERE p.session_id=?1 AND p.project_id=?2 AND p.task_id=?3
                      AND p.tool_id=?4 AND p.tool_version=?5 AND p.application_version=?6
                      AND NOT EXISTS(SELECT 1 FROM ansf_procedure_invalidations x
                                     WHERE x.procedure_id=p.procedure_id)
                      AND NOT EXISTS(SELECT 1 FROM ansf_verified_procedures successors
                                     WHERE successors.replaces_id=p.procedure_id)
                    ORDER BY p.recorded_utc DESC,p.procedure_id DESC LIMIT ?7")
            )?;
            stmt.query_map(params![session_id,project_id,task_id,tool_id,
                tool_version,application_version,limit as i64],map_record)?
                .collect::<Result<Vec<_>,_>>()?
        };
        let mut verified=Vec::new();
        for record in stored{
            let c=&record.candidate;
            if self.source_is_current(session_id,c.source_entry_id)?.as_deref()!=
                Some(record.source_sha256.as_str()){continue}
            let current=continuity.latest_for_conversation(
                project_id,task_id,session_id
            ).map_err(|_|AnsfError::InvalidInput)?;
            if current.as_ref().is_none_or(|cp|
                cp.checkpoint_id!=c.checkpoint_id||
                !cp.source_entry_ids.contains(&c.source_entry_id)||
                !cp.blockers.is_empty()
            ){continue}
            let receipts=continuity.verified_effects_for_checkpoint(
                &c.checkpoint_id,journal
            ).map_err(|_|AnsfError::InvalidInput)?;
            if let Some(e)=receipts.iter().find(|e|e.call_id==c.call_id&&
                e.tool_id==c.tool_id&&e.receipt_sha256==record.receipt_sha256){
                if independent_verify(e){verified.push(record)}
            }
        }
        Ok(verified)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::ansf::EntryRole;
    use crate::continuity::TaskCheckpoint;
    use crate::contracts::{ToolInvocation,ProtoPayload};
    use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
    fn root()->PathBuf{std::env::temp_dir().join(format!("lu-m4-{}-{}",
        std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap().as_nanos()))}
    #[test]
    fn editing_a_procedural_source_revokes_retrieval_without_rewriting_original(){
        use crate::ansf_revisions::RevisionOrigin;
        let root=root();fs::create_dir_all(&root).unwrap();
        let book=AnsfStore::open(&root.join("book.sqlite")).unwrap();
        book.create_session("c-test","ws-test","Revision proof").unwrap();
        book.start_chapter("c-test","Work").unwrap();
        let src=book.append_entry("c-test","original",EntryRole::User,
            "Original","First trusted version").unwrap();
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        let journal=EffectJournal::open(&root.join("journal.sqlite")).unwrap();
        let cp=TaskCheckpoint{checkpoint_id:"cp-rev".into(),project_id:"lu".into(),
            task_id:"review".into(),conversation_id:"c-test".into(),
            source_entry_ids:vec![src],completed_work:"Completed".into(),
            next_action:"Proceed only after revalidation".into(),blockers:vec![]};
        continuity.append_verified(&cp,&book).unwrap();
        journal.prepare(&ToolInvocation{
            call_id:"call-rev".into(),tool_id:"lu.verify".into(),
            workspace_id:Some("ws-test".into()),
            input:ProtoPayload{type_url:"lu.test.Verification".into(),bytes:vec![3]}
        }).unwrap();
        journal.mark_executing("call-rev").unwrap();
        journal.mark_applied("call-rev",b"independently tested postcondition").unwrap();
        continuity.link_verified_effect("cp-rev","call-rev",&journal).unwrap();
        let p=ProcedureCandidate{
            procedure_id:"proc-rev".into(),project_id:"lu".into(),task_id:"review".into(),
            session_id:"c-test".into(),source_entry_id:src,
            checkpoint_id:"cp-rev".into(),call_id:"call-rev".into(),
            tool_id:"lu.verify".into(),tool_version:"1.0".into(),
            application_version:"1.0".into(),title:"Original version".into(),
            prerequisites:vec![],steps:vec!["Read observed result".into()],
            failure_conditions:vec![],replaces_id:None,
        };
        book.promote_verified_procedure(&p,&continuity,&journal,|_|true).unwrap();
        let mut conflicting=p.clone();conflicting.steps=vec!["Wrong new meaning".into()];
        assert!(book.promote_verified_procedure(&conflicting,&continuity,&journal,|_|true).is_err());
        assert_eq!(book.reusable_verified_procedures("c-test","lu","review",
            "lu.verify","1.0","1.0",2,&continuity,&journal,|_|true).unwrap().len(),1);
        let revised=book.append_edited_entry("c-test","original","Changed",
            "This earlier instruction has been replaced",
            RevisionOrigin::VerifiedUser).unwrap().unwrap();
        assert!(revised.revised_entry_id>src);
        assert!(book.read_entry("c-test",src).unwrap().is_some());
        assert!(book.promote_verified_procedure(&p,&continuity,&journal,|_|true).is_err());
        assert!(book.reusable_verified_procedures("c-test","lu","review",
            "lu.verify","1.0","1.0",2,&continuity,&journal,|_|true).unwrap().is_empty());
        drop(book);drop(journal);drop(continuity);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn uncertain_effect_never_promotes_even_if_checkpoint_mentions_the_source(){
        let root=root();fs::create_dir_all(&root).unwrap();
        let book=AnsfStore::open(&root.join("book.sqlite")).unwrap();
        book.create_session("c-failed","w-failed","Interrupted").unwrap();
        book.start_chapter("c-failed","Interrupted work").unwrap();
        let src=book.append_entry("c-failed","msg-failed",EntryRole::User,
            "Instruction","User requested a cautious workflow").unwrap();
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        let journal=EffectJournal::open(&root.join("journal.sqlite")).unwrap();
        continuity.append_verified(&TaskCheckpoint{
            checkpoint_id:"cp-failed".into(),project_id:"lu".into(),
            task_id:"danger".into(),conversation_id:"c-failed".into(),
            source_entry_ids:vec![src],completed_work:"Not completed".into(),
            next_action:"Reconcile".into(),blockers:vec![],
        },&book).unwrap();
        journal.prepare(&ToolInvocation{
            call_id:"call-failed".into(),tool_id:"lu.write".into(),
            workspace_id:Some("w-failed".into()),
            input:ProtoPayload{type_url:"lu.test.Write".into(),bytes:vec![5]}
        }).unwrap();
        journal.mark_executing("call-failed").unwrap();
        journal.mark_uncertain("call-failed").unwrap();
        assert!(continuity.link_verified_effect("cp-failed","call-failed",&journal).is_err());
        let p=ProcedureCandidate{
            procedure_id:"proc-failed".into(),project_id:"lu".into(),
            task_id:"danger".into(),session_id:"c-failed".into(),
            source_entry_id:src,checkpoint_id:"cp-failed".into(),
            call_id:"call-failed".into(),tool_id:"lu.write".into(),
            tool_version:"1.0".into(),application_version:"1.0".into(),
            title:"Not independently verified".into(),prerequisites:vec![],
            steps:vec!["Attempt effect again".into()],failure_conditions:vec![],
            replaces_id:None,
        };
        assert!(book.promote_verified_procedure(&p,&continuity,&journal,|_|true).is_err());
        assert!(book.reusable_verified_procedures("c-failed","lu","danger",
            "lu.write","1.0","1.0",3,&continuity,&journal,|_|true).unwrap().is_empty());
        drop(book);drop(continuity);drop(journal);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn promotion_requires_original_and_real_verified_effect_then_rechecks_corrections(){
        let root=root();fs::create_dir_all(&root).unwrap();
        let book=AnsfStore::open(&root.join("book.sqlite")).unwrap();
        book.create_session("chat-one","workspace-one","M4 proof").unwrap();
        book.start_chapter("chat-one","Evidence").unwrap();
        let src=book.append_entry("chat-one","msg-one",EntryRole::User,
            "Workflow","User wants the reviewed build workflow").unwrap();
        let correction=book.append_entry("chat-one","msg-two",EntryRole::User,
            "Correction","Do not reuse the previous workflow").unwrap();
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        let journal=EffectJournal::open(&root.join("journal.sqlite")).unwrap();
        let cp=TaskCheckpoint{checkpoint_id:"cp-one".into(),project_id:"lu".into(),
            task_id:"build".into(),conversation_id:"chat-one".into(),
            source_entry_ids:vec![src],completed_work:"Validated build".into(),
            next_action:"Review result".into(),blockers:vec![]};
        continuity.append_verified(&cp,&book).unwrap();
        let c=ProcedureCandidate{
            procedure_id:"procedure-one".into(),project_id:"lu".into(),
            task_id:"build".into(),session_id:"chat-one".into(),source_entry_id:src,
            checkpoint_id:"cp-one".into(),call_id:"call-one".into(),
            tool_id:"lu.build".into(),tool_version:"2.1".into(),
            application_version:"0.1".into(),title:"Verified build".into(),
            prerequisites:vec!["Pinned compiler is present".into()],
            steps:vec!["Review current compilation intent".into()],
            failure_conditions:vec!["Compiler version changed".into()],
            replaces_id:None,
        };
        assert!(book.promote_verified_procedure(&c,&continuity,&journal,|_|true).is_err());
        journal.prepare(&ToolInvocation{
            call_id:"call-one".into(),tool_id:"lu.build".into(),
            workspace_id:Some("workspace-one".into()),
            input:ProtoPayload{type_url:"lu.test.Build".into(),bytes:vec![1]}
        }).unwrap();
        journal.mark_executing("call-one").unwrap();
        journal.mark_applied("call-one",b"verified native receipt").unwrap();
        continuity.link_verified_effect("cp-one","call-one",&journal).unwrap();
        assert!(book.promote_verified_procedure(&c,&continuity,&journal,|_|false).is_err());
        let stored=book.promote_verified_procedure(&c,&continuity,&journal,|_|true).unwrap();
        assert_eq!(stored.candidate,c);
        assert_eq!(book.promote_verified_procedure(&c,&continuity,&journal,|_|true).unwrap(),stored);
        assert_eq!(book.reusable_verified_procedures("chat-one","lu","build",
            "lu.build","2.1","0.1",4,&continuity,&journal,|_|true).unwrap(),vec![stored]);
        assert!(book.reusable_verified_procedures("chat-one","lu","build",
            "lu.build","9.0","0.1",4,&continuity,&journal,|_|true).unwrap().is_empty());
        assert!(book.reusable_verified_procedures("chat-one","lu","build",
            "lu.build","2.1","0.1",4,&continuity,&journal,|_|false).unwrap().is_empty());
        book.invalidate_verified_procedure("procedure-one","chat-one",correction,
            "New user correction requires review").unwrap();
        assert!(book.reusable_verified_procedures("chat-one","lu","build",
            "lu.build","2.1","0.1",4,&continuity,&journal,|_|true).unwrap().is_empty());
        drop(book);
        let reopened=AnsfStore::open(&root.join("book.sqlite")).unwrap();
        assert!(reopened.reusable_verified_procedures("chat-one","lu","build",
            "lu.build","2.1","0.1",4,&continuity,&journal,|_|true).unwrap().is_empty());
        drop(reopened);drop(continuity);drop(journal);
        fs::remove_dir_all(root).unwrap();
    }
}
