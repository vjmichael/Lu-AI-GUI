//! AGENT-01E: bounded, read-only per-conversation human plan timeline.
 //!
 //! Only host-selected profile file; NO WebView-supplied SQLite paths, plan
 //! IDs or approval tokens. No writes, auto-retries or worker launches.
 //! Persisted approval, a broker receipt link, and completed OS execution are
 //! different facts. Never claim present-day output/source/process custody.
 use std::{fs,path::{Path,PathBuf}};
 use rusqlite::{params,Connection,OpenFlags,OptionalExtension};
 use serde::Serialize;
 use crate::chat_store::ChatStore;
 const MAX_FILE:u64=64*1024*1024;
 const MAX_PLANS:usize=16;
 const MAX_STEPS:usize=32;
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct AgentStepProgress{
     pub step_index:u8,
     pub purpose:String,
     pub expected_deliverable:String,
     pub depends_on:Option<u8>,
     pub schedule_state:String,
     pub historical_witness_linked:bool,
 }
 /// AGENT-01H: read-only, bounded review of the next step and blockers.
 /// A historical link is NOT live verification or executable authority.
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct AgentContinuationPreview{
     pub candidate_step_index:Option<u8>,
     pub status:&'static str,
     pub required_action:&'static str,
     pub live_source_reverified:bool,
     pub execution_authority:&'static str,
 }
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct AgentPlanProgress{
     pub plan_id:String,
     pub original_human_turn_id:String,
     pub plan_state:String,
     pub latest_revision:u8,
     /// Exact host-bound revision for an OPTIONAL native confirmation.
     /// The SHA itself is never a capability token.
     pub revision_sha256:Option<String>,
     pub revision_review:String,
     /// Host source verification is not available for nonempty assumptions
     /// in this first native review UI, so approval is withheld.
     pub pending_source_checks:u8,
     pub supervisor_state:String,
     pub step_count:u8,
     pub steps:Vec<AgentStepProgress>,
     pub next_safe_action:&'static str,
     pub continuation_preview:AgentContinuationPreview,
     pub execution_authority:&'static str,
 }
 // This is a pure read-only state classification, not a model or OS action.
 // Even the "manual review" branch must recheck current ANSF goal, prior
 // independent worker/oracle evidence, exact resource and a NEW human turn
 // in the native broker. Previous ownership is never reused after restart.
 fn continuation_preview(
     plan_state:&str,review:&str,owner_state:&str,
     pending_sources:u8,steps:&[AgentStepProgress],
 )->AgentContinuationPreview{
     let make=|candidate_step_index,status,required_action|
         AgentContinuationPreview{
             candidate_step_index,status,required_action,
             live_source_reverified:false,
             execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
         };
     if matches!(plan_state,"CANCEL_REQUESTED"|"NEEDS_REVIEW")
         ||matches!(owner_state,"CANCEL_REQUESTED"|"RECOVERY_REQUIRED"){
         return make(None,"STOP_RECOVERY_REQUIRED",
             "Inspect task, effect, independent oracle and OS custody. Never automatically replay.")
     }
     if owner_state=="STALE_PLAN_REVISION"{
         return make(None,"STOP_STALE_REVISION",
             "The plan or goal changed. Request a fresh source-backed human plan review.")
     }
     if owner_state=="OWNED"{
         return make(None,"STOP_OWNER_CUSTODY_UNCERTAIN",
             "A host owns this plan; observe or independently reconcile it, never reassign.")
     }
     if owner_state=="RELEASED"{
         return make(None,"STOP_RELEASED_OWNER",
             "The earlier owner is closed. Reusing its claim or review for a later step is prohibited.")
     }
     if owner_state!="NOT_CLAIMED"{
         return make(None,"STOP_UNRECOGNIZED_OWNER",
             "Unrecognized scheduler history cannot authorize anything.")
     }
     if review!="APPROVED_PLAN_ONLY"{
         return make(None,"STOP_PLAN_REVIEW",
             "Review the exact immutable plan first. Plan approval alone does not authorize a tool.")
     }
     if pending_sources>0{
         return make(None,"STOP_SOURCE_RECHECK",
             "Source assumptions require independent review before any step.")
     }
     if steps.is_empty(){
         return make(None,"STOP_NO_STEPS",
             "There is no independently reviewable plan step.")
     }
     if steps.iter().any(|s|matches!(s.schedule_state.as_str(),
         "INTERRUPTED_NEEDS_RECONCILIATION"|"CANCELLED_NEEDS_RECONCILIATION")){
         return make(None,"STOP_UNCERTAIN_STEP",
             "An earlier step is uncertain. Inspect real journals before another grant.")
     }
     if steps.iter().any(|s|s.schedule_state=="WAITING_FOR_BROKER_ACCEPTANCE"){
         return make(None,"STOP_OUTSTANDING_STEP",
             "A prior step has no independently accepted result; no subsequent consent.")
     }
     if steps.iter().any(|s|s.schedule_state=="HISTORICAL_WITNESS_RECORDED"
         &&!s.historical_witness_linked){
         return make(None,"STOP_MISSING_WITNESS",
             "A historical step lacks a witness link; never infer that work succeeded.")
     }
     let Some(next)=steps.iter().find(|s|s.schedule_state=="NOT_SCHEDULED")else{
         return make(None,"HISTORICAL_ONLY",
             "All steps are historical; this does not confer new execution authority.")
     };
     if let Some(parent)=next.depends_on{
         if parent>=next.step_index{
             return make(None,"STOP_INVALID_DEPENDENCY",
                 "Invalid graph requires a fresh reviewed plan revision.")
         }
         let Some(previous)=steps.get(parent as usize)else{
             return make(None,"STOP_INVALID_DEPENDENCY",
                 "Missing dependency requires a fresh reviewed plan revision.")
         };
         if previous.schedule_state!="HISTORICAL_WITNESS_RECORDED"
             ||!previous.historical_witness_linked{
             return make(Some(next.step_index),"STOP_DEPENDENCY_UNVERIFIED",
                 "Earlier prerequisite lacks historical independent evidence.")
         }
         return make(Some(next.step_index),"PREVIEW_DEPENDENCY_RECHECK",
             "Historical receipts require fresh independent re-verification and a NEW human step request. No automatic continuation.")
     }
     if next.purpose=="INDEPENDENT_CHECK"
         &&next.expected_deliverable=="INDEPENDENT_WITNESS"{
         return make(Some(next.step_index),"MANUAL_ONE_STEP_REVIEW_ONLY",
             "A FIRST independent check may ask for a separate native picker and fresh human step confirmation. The host still verifies the goal and broker.")
     }
     make(Some(next.step_index),"STOP_UNSUPPORTED_STEP_TYPE",
         "Current reviewed Git/FFprobe does not implement this step type. Never substitute a different tool.")
 }
 fn valid(id:&str,prefix:&str)->bool{
     id.len()==prefix.len()+32&&id.starts_with(prefix)
         &&id[prefix.len()..].bytes().all(|b|
             b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
 }
 fn safe(path:&Path)->Result<Option<fs::Metadata>,String>{
     let meta=match fs::symlink_metadata(path){
         Ok(m)=>m,
         Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
         Err(_)=>return Err("Plan progress file inaccessible".into())
     };
     if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_FILE{
         return Err("Plan history must be a bounded regular file".into())
     }
     #[cfg(windows)]
     {
         use std::os::windows::fs::MetadataExt;
         if meta.file_attributes()&0x400!=0{
             return Err("Linked Windows plan history refused".into())
         }
     }
     Ok(Some(meta))
 }
 fn exists(db:&Connection,table:&str)->Result<bool,String>{
     let count:i64=db.query_row(
         "SELECT COUNT(*) FROM sqlite_master
          WHERE type='table' AND name=?1",
         params![table],|r|r.get(0)
     ).map_err(|_|"Plan-history schema cannot be inspected")?;
     Ok(count==1)
 }
 fn open_readonly(root:&Path)->Result<Option<Connection>,String>{
     let file=root.join("lu-agent-plans.sqlite");
     if safe(&file)?.is_none(){return Ok(None)}
     for suffix in ["-wal","-shm"]{
         let mut sidecar=file.as_os_str().to_os_string();
         sidecar.push(suffix);
         safe(&PathBuf::from(sidecar))?;
     }
     Connection::open_with_flags(
         file,OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX
     ).map(Some).map_err(|_|"Plan-history SQLite is unreadable".into())
 }
 /// Authenticate the chosen chat before querying its original human turns.
 /// Skips an entry whose saved USER turn cannot be independently verified;
 /// never relies on model-generated role labels.
 pub fn read_chat_progress(
     root:&Path,chat:&str,chats:&ChatStore
 )->Result<Vec<AgentPlanProgress>,String>{
     if !valid(chat,"c-"){return Err("Invalid chat identity".into())}
     chats.history_window(chat,None,"older",1)?;
     let Some(db)=open_readonly(root)? else{return Ok(Vec::new())};
     for table in [
         "lu_agent_plans","lu_agent_plan_revisions",
         "lu_agent_plan_revision_steps","lu_agent_plan_revision_reviews",
         "lu_agent_plan_revision_assumptions",
         "lu_agent_schedule_owners","lu_agent_schedule_proposals",
         "lu_agent_deliverable_receipts",
     ]{
         if !exists(&db,table)?{
             // This might be a partially migrated journal. Do not invent
             // plans from an incomplete schema or infer readiness.
             return Err("Agent progress journal is incomplete".into())
         }
     }
     let mut stmt=db.prepare(
         "SELECT plan_id,human_turn_id,state FROM lu_agent_plans
          WHERE conversation_id=?1 ORDER BY created_at DESC,plan_id
          LIMIT 17"
     ).map_err(|_|"Cannot read plan list")?;
     let rows=stmt.query_map(params![chat],|r|Ok((
         r.get::<_,String>(0)?,
         r.get::<_,String>(1)?,
         r.get::<_,String>(2)?
     ))).map_err(|_|"Cannot query plan rows")?;
     let mut plans=Vec::new();
     for row in rows{
         if plans.len()>=MAX_PLANS{
             return Err("Too many plans in this history view".into())
         }
         let (plan,turn,state)=row.map_err(|_|"Invalid plan entry")?;
         if !valid(&plan,"p-")||!valid(&turn,"g-")
             ||!matches!(state.as_str(),
                 "DRAFT"|"OBSERVATIONS_ONLY"|"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
             return Err("Damaged plan identity or state".into())
         }
         if chats.trusted_user_turn_text(chat,&turn).is_none(){continue}
         let revision:Option<(i64,String)>=db.query_row(
             "SELECT revision_index,revision_sha256
              FROM lu_agent_plan_revisions
              WHERE plan_id=?1 ORDER BY revision_index DESC LIMIT 1",
             params![plan],|r|Ok((r.get(0)?,r.get(1)?))
         ).optional().map_err(|_|"Plan revision lookup failed")?;
         let Some((revision_number,revision_sha))=revision else{
             // This plan was only registered, not yet reviewed.
             plans.push(AgentPlanProgress{
                 plan_id:plan,original_human_turn_id:turn,plan_state:state,
                 latest_revision:0,revision_sha256:None,
                 revision_review:"NO_PROPOSAL".into(),pending_source_checks:0,
                 supervisor_state:"NOT_CLAIMED".into(),step_count:0,
                 steps:Vec::new(),
                 continuation_preview:continuation_preview(
                     &state,"NO_PROPOSAL","NOT_CLAIMED",0,&[]
                 ),
                 next_safe_action:"Prepare a plan proposal for human review. Do not execute.",
                 execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
             });continue
         };
         if !(1..=64).contains(&revision_number)
             ||revision_sha.len()!=64||
             !revision_sha.bytes().all(|b|b.is_ascii_hexdigit()
                 &&!b.is_ascii_uppercase()){
             return Err("Damaged plan revision".into())
         }
         let review:Option<String>=db.query_row(
             "SELECT decision FROM lu_agent_plan_revision_reviews
              WHERE plan_id=?1 AND revision_index=?2",
             params![plan,revision_number],|r|r.get(0)
         ).optional().map_err(|_|"Human review history unavailable")?;
         let review=match review.as_deref(){
             Some("APPROVE_PLAN_ONLY")=>"APPROVED_PLAN_ONLY",
             Some("REJECT_PLAN")=>"REJECTED_PLAN",
             None=>"PENDING_HUMAN_REVIEW",
             _=>return Err("Damaged human review decision".into())
         };
         let count:i64=db.query_row(
             "SELECT COUNT(*) FROM lu_agent_plan_revision_assumptions
              WHERE plan_id=?1 AND revision_index=?2",
             params![plan,revision_number],|r|r.get(0)
         ).map_err(|_|"Assumption-source references unavailable")?;
         if !(0..=16).contains(&count){
             return Err("Too many assumption-source references".into())
         }
         let owner:Option<(String,String)>=db.query_row(
             "SELECT revision_sha256,state FROM lu_agent_schedule_owners
              WHERE plan_id=?1",
             params![plan],|r|Ok((r.get(0)?,r.get(1)?))
         ).optional().map_err(|_|"Scheduler owner history unreadable")?;
         let owner_state=match owner.as_ref(){
             None=>"NOT_CLAIMED",
             Some((rev,_)) if rev!=&revision_sha=>"STALE_PLAN_REVISION",
             Some((_,s)) if matches!(s.as_str(),
                "OWNED"|"CANCEL_REQUESTED"|"RECOVERY_REQUIRED"|"RELEASED")=>s,
             _=>return Err("Unknown scheduler history state".into())
         };
         let mut step_stmt=db.prepare(
             "SELECT step_index,kind,depends_on,expected
              FROM lu_agent_plan_revision_steps
              WHERE plan_id=?1 AND revision_index=?2
              ORDER BY step_index LIMIT 33"
         ).map_err(|_|"Expected deliverable list inaccessible")?;
         let step_rows=step_stmt.query_map(params![plan,revision_number],|r|Ok((
             r.get::<_,i64>(0)?,r.get::<_,String>(1)?,
             r.get::<_,Option<i64>>(2)?,r.get::<_,String>(3)?
         ))).map_err(|_|"Cannot read steps")?;
         let mut steps=Vec::new();
         for row in step_rows{
             if steps.len()>=MAX_STEPS{return Err("Too many plan steps".into())}
             let (index,kind,dep,expected)=row.map_err(|_|"Damaged step entry")?;
             if index!=steps.len() as i64||!matches!(kind.as_str(),
                 "COLLECT_SOURCE"|"INSPECT"|"ANALYZE"|"INDEPENDENT_CHECK"|
                 "HUMAN_REVIEW"|"PRESENT") ||!matches!(expected.as_str(),
                 "SOURCE_REFERENCES"|"STRUCTURED_FINDINGS"|
                 "INDEPENDENT_WITNESS"|"HUMAN_DECISION"|
                 "ARTIFACT_REFERENCE"|"EXPLANATION"){
                 return Err("Unexpected step or deliverable type".into())
             }
             let depends_on=dep.map(|p|u8::try_from(p)
                 .map_err(|_|"Invalid plan step dependency")).transpose()?;
             if depends_on.is_some_and(|p|p as usize>=steps.len()){
                 return Err("Invalid step ordering".into())
             }
             let record:Option<String>=db.query_row(
                 "SELECT state FROM lu_agent_schedule_proposals
                  WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
                 params![plan,revision_sha,index],|r|r.get(0)
             ).optional().map_err(|_|"Cannot read scheduled step state")?;
             let step_state=match record.as_deref(){
                 Some("PROPOSED_NO_EXECUTION")=>"WAITING_FOR_BROKER_ACCEPTANCE",
                 Some("OBSERVED_VERIFIED_HISTORICAL")=>"HISTORICAL_WITNESS_RECORDED",
                 Some("ABANDONED_UNCERTAIN")=>"INTERRUPTED_NEEDS_RECONCILIATION",
                 Some("CANCEL_INTENT_UNCERTAIN")=>"CANCELLED_NEEDS_RECONCILIATION",
                 None=>"NOT_SCHEDULED",
                 _=>return Err("Damaged scheduler state".into())
             };
             let witness:i64=db.query_row(
                 "SELECT COUNT(*) FROM lu_agent_deliverable_receipts
                  WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
                 params![plan,revision_sha,index],|r|r.get(0)
             ).map_err(|_|"Cannot inspect historical witness reference")?;
             if !(0..=1).contains(&witness){
                 return Err("Duplicate conflicting witness".into())
             }
             steps.push(AgentStepProgress{
                 step_index:index as u8,purpose:kind,depends_on,
                 expected_deliverable:expected,
                 schedule_state:step_state.into(),
                 historical_witness_linked:witness==1,
             });
         }
         let next=if matches!(owner_state,
             "RECOVERY_REQUIRED"|"CANCEL_REQUESTED")
             ||state=="NEEDS_REVIEW"||state=="CANCEL_REQUESTED"{
             "Inspect uncertainty and request an independent recovery review; no automatic replay."
         }else if owner_state=="STALE_PLAN_REVISION"{
             "The goal/plan changed. Review a fresh plan revision before new work."
         }else if review!="APPROVED_PLAN_ONLY"{
             "Review the current plan proposal. Approval alone never authorizes tools."
         }else if owner_state=="OWNED"{
             "One exact reviewed step has been claimed by a host. It may be running or interrupted: check the original task/effect journals and independent source evidence. Never retry automatically."
         }else if owner_state=="RELEASED"{
             "Earlier host ownership is released; no new execution follows without explicit current human review and fresh authority."
         }else{
             "A fresh human request, source validation, reviewed broker grant and verified OS custody are still required."
         };
         let preview=continuation_preview(
             &state,review,owner_state,count as u8,&steps
         );
         plans.push(AgentPlanProgress{
             plan_id:plan,original_human_turn_id:turn,plan_state:state,
             latest_revision:revision_number as u8,
             revision_sha256:Some(revision_sha),
             revision_review:review.into(),
             pending_source_checks:count as u8,
             supervisor_state:owner_state.into(),
             step_count:steps.len() as u8,steps,
             next_safe_action:next,continuation_preview:preview,
             execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
         });
     }
     Ok(plans)
 }


#[cfg(test)]
mod tests{
    use super::*;
    use std::time::{SystemTime,UNIX_EPOCH};
    fn fixture()->(PathBuf,ChatStore,String,String,String){
        let root=std::env::temp_dir().join(format!(
            "lu-agent01e-progress-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let turn=format!("g-{:032x}",7);
        let plan=format!("p-{:032x}",8);
        chats.append_user_with_turn_id(
            &chat,&turn,"Please review this multi-step plan").unwrap();
        let db=Connection::open(root.join("lu-agent-plans.sqlite")).unwrap();
        // Minimal schema fixture, but the read_chat_progress implementation
        // and saved HUMAN ChatStore verifier are the real production code.
        db.execute_batch(
            "CREATE TABLE lu_agent_plans(
                plan_id TEXT,conversation_id TEXT,human_turn_id TEXT,
                state TEXT,created_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE lu_agent_plan_revisions(
                plan_id TEXT,revision_index INTEGER,revision_sha256 TEXT);
             CREATE TABLE lu_agent_plan_revision_steps(
                plan_id TEXT,revision_index INTEGER,step_index INTEGER,
                kind TEXT,depends_on INTEGER,expected TEXT);
             CREATE TABLE lu_agent_plan_revision_reviews(
                plan_id TEXT,revision_index INTEGER,decision TEXT);
             CREATE TABLE lu_agent_plan_revision_assumptions(
                plan_id TEXT,revision_index INTEGER,source_id TEXT);
             CREATE TABLE lu_agent_schedule_owners(
                plan_id TEXT,revision_sha256 TEXT,state TEXT);
             CREATE TABLE lu_agent_schedule_proposals(
                plan_id TEXT,revision_sha256 TEXT,step_index INTEGER,state TEXT);
             CREATE TABLE lu_agent_deliverable_receipts(
                plan_id TEXT,revision_sha256 TEXT,step_index INTEGER);"
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plans(plan_id,conversation_id,human_turn_id,state)
             VALUES(?1,?2,?3,'DRAFT')",
            params![plan,chat,turn]).unwrap();
        let digest="a".repeat(64);
        db.execute(
            "INSERT INTO lu_agent_plan_revisions VALUES(?1,1,?2)",
            params![plan,digest]).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plan_revision_steps VALUES(
                ?1,1,0,'INDEPENDENT_CHECK',NULL,'INDEPENDENT_WITNESS')",
            params![plan]).unwrap();
        (root,chats,chat,turn,plan)
    }
    #[test]
    fn genuine_saved_human_turn_scopes_read_only_plan_and_pending_review(){
        let (root,chats,chat,turn,plan)=fixture();
        let before=read_chat_progress(&root,&chat,&chats).unwrap();
        assert_eq!(before.len(),1);
        assert_eq!(before[0].plan_id,plan);
        assert_eq!(before[0].original_human_turn_id,turn);
        assert_eq!(before[0].revision_review,"PENDING_HUMAN_REVIEW");
        assert_eq!(before[0].steps[0].schedule_state,"NOT_SCHEDULED");
        assert_eq!(before[0].execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        let other=chats.create_chat().unwrap().chat_id;
        assert!(read_chat_progress(&root,&other,&chats).unwrap().is_empty());
        drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(read_chat_progress(&root,&chat,&reopened).unwrap().len(),1);
        drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recorded_human_approval_and_interruption_never_mark_worker_proven(){
        let (root,chats,chat,_,plan)=fixture();
        let db=Connection::open(root.join("lu-agent-plans.sqlite")).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plan_revision_reviews VALUES(
                ?1,1,'APPROVE_PLAN_ONLY')",params![plan]).unwrap();
        db.execute(
            "INSERT INTO lu_agent_schedule_owners VALUES(
                ?1,?2,'RECOVERY_REQUIRED')",
            params![plan,"a".repeat(64)]).unwrap();
        db.execute(
            "INSERT INTO lu_agent_schedule_proposals VALUES(
                ?1,?2,0,'ABANDONED_UNCERTAIN')",
            params![plan,"a".repeat(64)]).unwrap();
        db.execute(
            "INSERT INTO lu_agent_deliverable_receipts VALUES(
                ?1,?2,0)",params![plan,"a".repeat(64)]).unwrap();
        drop(db);
        let result=read_chat_progress(&root,&chat,&chats).unwrap();
        assert_eq!(result[0].revision_review,"APPROVED_PLAN_ONLY");
        assert_eq!(result[0].supervisor_state,"RECOVERY_REQUIRED");
        assert_eq!(result[0].steps[0].schedule_state,
            "INTERRUPTED_NEEDS_RECONCILIATION");
        assert!(result[0].steps[0].historical_witness_linked);
        assert!(result[0].next_safe_action.contains("no automatic replay"));
        assert_eq!(result[0].execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn real_approved_first_step_is_advisory_not_an_execution_grant(){
        let (root,chats,chat,_,plan)=fixture();
        let db=Connection::open(root.join("lu-agent-plans.sqlite")).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plan_revision_reviews VALUES(?1,1,'APPROVE_PLAN_ONLY')",
            params![plan]
        ).unwrap();
        drop(db);
        let plan_view=read_chat_progress(&root,&chat,&chats).unwrap();
        let next=&plan_view[0].continuation_preview;
        assert_eq!(next.status,"MANUAL_ONE_STEP_REVIEW_ONLY");
        assert_eq!(next.candidate_step_index,Some(0));
        assert!(!next.live_source_reverified);
        assert_eq!(next.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn released_owner_never_becomes_a_reusable_step_grant_after_restart(){
        let (root,chats,chat,_,plan)=fixture();
        let path=root.join("lu-agent-plans.sqlite");
        let db=Connection::open(&path).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plan_revision_reviews VALUES(?1,1,'APPROVE_PLAN_ONLY')",
            params![plan]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plan_revision_steps VALUES(
                ?1,1,1,'INDEPENDENT_CHECK',0,'INDEPENDENT_WITNESS')",
            params![plan]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_schedule_owners VALUES(?1,?2,'RELEASED')",
            params![plan,"a".repeat(64)]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_schedule_proposals VALUES(
                ?1,?2,0,'OBSERVED_VERIFIED_HISTORICAL')",
            params![plan,"a".repeat(64)]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_deliverable_receipts VALUES(?1,?2,0)",
            params![plan,"a".repeat(64)]
        ).unwrap();
        drop(db);
        let state=read_chat_progress(&root,&chat,&chats).unwrap();
        assert_eq!(state[0].steps.len(),2);
        assert_eq!(state[0].steps[0].schedule_state,"HISTORICAL_WITNESS_RECORDED");
        assert_eq!(state[0].continuation_preview.status,"STOP_RELEASED_OWNER");
        assert_eq!(state[0].continuation_preview.candidate_step_index,None);
        drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        let replay=read_chat_progress(&root,&chat,&reopened).unwrap();
        assert_eq!(replay[0].continuation_preview.status,"STOP_RELEASED_OWNER");
        assert!(!replay[0].continuation_preview.live_source_reverified);
        drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn historical_witness_link_cannot_override_uncertain_scheduler_custody(){
        let (root,chats,chat,_,plan)=fixture();
        let db=Connection::open(root.join("lu-agent-plans.sqlite")).unwrap();
        db.execute(
            "INSERT INTO lu_agent_plan_revision_reviews VALUES(?1,1,'APPROVE_PLAN_ONLY')",
            params![plan]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_schedule_owners VALUES(?1,?2,'RECOVERY_REQUIRED')",
            params![plan,"a".repeat(64)]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_schedule_proposals VALUES(
                ?1,?2,0,'ABANDONED_UNCERTAIN')",
            params![plan,"a".repeat(64)]
        ).unwrap();
        db.execute(
            "INSERT INTO lu_agent_deliverable_receipts VALUES(?1,?2,0)",
            params![plan,"a".repeat(64)]
        ).unwrap();
        drop(db);
        let view=read_chat_progress(&root,&chat,&chats).unwrap();
        assert_eq!(view[0].continuation_preview.status,"STOP_RECOVERY_REQUIRED");
        assert_eq!(view[0].continuation_preview.candidate_step_index,None);
        assert_eq!(view[0].continuation_preview.execution_authority,
            "NOT_AUTHORIZED_FOR_EXECUTION");
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cached_dependency_or_unsupported_step_never_authorizes_next_worker(){
        let prior=AgentStepProgress{
            step_index:0,purpose:"INDEPENDENT_CHECK".into(),
            expected_deliverable:"INDEPENDENT_WITNESS".into(),
            depends_on:None,schedule_state:"HISTORICAL_WITNESS_RECORDED".into(),
            historical_witness_linked:true,
        };
        let next=AgentStepProgress{
            step_index:1,purpose:"INDEPENDENT_CHECK".into(),
            expected_deliverable:"INDEPENDENT_WITNESS".into(),
            depends_on:Some(0),schedule_state:"NOT_SCHEDULED".into(),
            historical_witness_linked:false,
        };
        let preview=continuation_preview(
            "DRAFT","APPROVED_PLAN_ONLY","NOT_CLAIMED",0,&[prior.clone(),next]
        );
        assert_eq!(preview.status,"PREVIEW_DEPENDENCY_RECHECK");
        assert_eq!(preview.candidate_step_index,Some(1));
        assert!(!preview.live_source_reverified);
        assert_eq!(preview.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        let no_witness=continuation_preview(
            "DRAFT","APPROVED_PLAN_ONLY","NOT_CLAIMED",0,
            &[AgentStepProgress{historical_witness_linked:false,..prior}]
        );
        assert_eq!(no_witness.status,"STOP_MISSING_WITNESS");
        let unsupported=continuation_preview(
            "DRAFT","APPROVED_PLAN_ONLY","NOT_CLAIMED",0,
            &[AgentStepProgress{
                step_index:0,purpose:"PRESENT".into(),
                expected_deliverable:"EXPLANATION".into(),depends_on:None,
                schedule_state:"NOT_SCHEDULED".into(),historical_witness_linked:false,
            }]
        );
        assert_eq!(unsupported.status,"STOP_UNSUPPORTED_STEP_TYPE");
    }
    #[test]
    fn inconsistent_schema_is_error_not_fabricated_empty_history(){
        let (root,chats,chat,_,_)=fixture();
        let db=Connection::open(root.join("lu-agent-plans.sqlite")).unwrap();
        db.execute("DROP TABLE lu_agent_schedule_proposals",[]).unwrap();
        drop(db);
        assert!(read_chat_progress(&root,&chat,&chats).is_err());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_plan_database_is_rejected(){
        use std::os::unix::fs::symlink;
        let (root,chats,chat,_,_)=fixture();
        let path=root.join("lu-agent-plans.sqlite");
        let moved=root.join("actual-plan.sqlite");
        fs::rename(&path,&moved).unwrap();
        symlink(&moved,&path).unwrap();
        assert!(read_chat_progress(&root,&chat,&chats).is_err());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
}
